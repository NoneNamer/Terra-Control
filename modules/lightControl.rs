//! Logic for UV1, UV2 and the heat spot: schedule evaluation, overheat protection and the
//! re-strike pause of the UV lamps. No hardware access here; relays are switched through
//! `gpio::Hardware`.
//!
//! Re-strike pause: a UV lamp that was switched off stays off for `restrike_pause_secs`
//! (hot metal-halide / T5 lamps must cool down before igniting again). The switching times are
//! stored in the database, see `restore_restrike` for how a restart is handled.

use crate::modules::config::LightControlConfig;
use crate::modules::gpio::{Relay, SharedHardware};
use crate::modules::logs::{self, Level};
use crate::modules::models::WeekSchedule;
use crate::modules::state::AppState;
use crate::modules::storage;
use anyhow::Result;
use chrono::{Datelike, Local, NaiveTime};
use serde::Serialize;
use std::time::{Duration, Instant};

/// Requested on/off state of the lamps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct LampStates {
    pub uv1: bool,
    pub uv2: bool,
    pub heat: bool,
}

impl LampStates {
    pub fn from_schedule(schedule: &WeekSchedule, t: NaiveTime) -> Self {
        let on = |w: Option<crate::modules::models::TimeWindow>| w.is_some_and(|w| w.contains(t));
        LampStates { uv1: on(schedule.uv1), uv2: on(schedule.uv2), heat: on(schedule.heat) }
    }
}

/// Why the heat spot is currently not allowed to run.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum HeatBlock {
    /// No basking temperature measured yet since start
    NoReading,
    /// Basking sensor failed or its last value is too old
    SensorFault,
    /// Basking temperature at or above the overheat threshold
    Overheat { temp: f32 },
    /// Overheat cooldown still running
    Cooldown { remaining_secs: u64 },
}

/// Remaining re-strike pause per UV lamp in seconds (None = may switch on).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RestrikeStatus {
    pub uv1: Option<u64>,
    pub uv2: Option<u64>,
}

pub struct LightController {
    overheat_temp: f32,
    cooldown: Duration,
    restrike_pause: Duration,
    /// A basking value older than this counts as a sensor fault
    max_reading_age: Duration,
    desired: LampStates,
    basking_temp: Option<f32>,
    last_reading: Option<Instant>,
    sensor_failed: bool,
    cooldown_until: Option<Instant>,
    /// UV1 / UV2 must stay off until these instants
    restrike_until: [Option<Instant>; 2],
}

/// Index into `restrike_until`; None for relays without a re-strike pause.
fn uv_index(relay: Relay) -> Option<usize> {
    match relay {
        Relay::Uv1 => Some(0),
        Relay::Uv2 => Some(1),
        _ => None,
    }
}

impl LightController {
    pub fn new(config: &LightControlConfig, sensor_interval: Duration) -> Self {
        LightController {
            overheat_temp: config.overheat_temp,
            cooldown: Duration::from_secs(config.overheat_cooldown_secs),
            restrike_pause: Duration::from_secs(config.restrike_pause_secs),
            max_reading_age: sensor_interval * 3,
            desired: LampStates::default(),
            basking_temp: None,
            last_reading: None,
            sensor_failed: false,
            cooldown_until: None,
            restrike_until: [None, None],
        }
    }

    pub fn restrike_pause(&self) -> Duration {
        self.restrike_pause
    }

    /// Blocks a UV lamp until `until` (used after a restart). Ignored for other relays.
    pub fn block_restrike(&mut self, relay: Relay, until: Instant) {
        if let Some(i) = uv_index(relay) {
            self.restrike_until[i] = Some(until);
        }
    }

    fn restrike_remaining(&self, relay: Relay, now: Instant) -> Option<Duration> {
        let until = self.restrike_until[uv_index(relay)?]?;
        (now < until).then(|| until - now)
    }

    /// Remaining re-strike pause of both UV lamps.
    pub fn restrike_status(&self, now: Instant) -> RestrikeStatus {
        // Round up so a running pause never shows as 0 s
        let secs = |r| self.restrike_remaining(r, now).map(|d: Duration| d.as_secs() + 1);
        RestrikeStatus { uv1: secs(Relay::Uv1), uv2: secs(Relay::Uv2) }
    }

    pub fn desired(&self) -> LampStates {
        self.desired
    }

    pub fn set_desired(&mut self, desired: LampStates) {
        self.desired = desired;
    }

    /// Feeds a new basking measurement (None = sensor failed).
    /// Returns a message if the safety state changed in a way worth logging.
    pub fn update_temperature(&mut self, temp: Option<f32>, now: Instant) -> Option<(Level, String)> {
        let was_failed = self.sensor_failed;
        self.sensor_failed = temp.is_none();
        if let Some(t) = temp {
            self.basking_temp = Some(t);
            self.last_reading = Some(now);
            if t >= self.overheat_temp {
                let starting = self.cooldown_until.map_or(true, |until| now >= until);
                // Every reading above the limit restarts the cooldown
                self.cooldown_until = Some(now + self.cooldown);
                if starting {
                    return Some((
                        Level::Error,
                        format!(
                            "OVERHEAT: basking spot {t:.1} °C >= {:.1} °C, heat spot blocked for {} min",
                            self.overheat_temp,
                            self.cooldown.as_secs() / 60
                        ),
                    ));
                }
            }
        }
        match (was_failed, self.sensor_failed) {
            (false, true) => Some((Level::Error, "Basking sensor failed, heat spot blocked".into())),
            (true, false) => Some((Level::Info, "Basking sensor is delivering values again".into())),
            _ => None,
        }
    }

    /// Returns why the heat spot must stay off, or None if it may run.
    pub fn heat_block(&self, now: Instant) -> Option<HeatBlock> {
        let Some(last) = self.last_reading else {
            return Some(HeatBlock::NoReading);
        };
        if self.sensor_failed || now.duration_since(last) > self.max_reading_age {
            return Some(HeatBlock::SensorFault);
        }
        if let Some(t) = self.basking_temp.filter(|t| *t >= self.overheat_temp) {
            return Some(HeatBlock::Overheat { temp: t });
        }
        if let Some(until) = self.cooldown_until.filter(|until| now < *until) {
            return Some(HeatBlock::Cooldown { remaining_secs: (until - now).as_secs() });
        }
        None
    }

    /// True while an overheat or its cooldown blocks the heat spot.
    pub fn is_overheat(&self, now: Instant) -> bool {
        matches!(self.heat_block(now), Some(HeatBlock::Overheat { .. } | HeatBlock::Cooldown { .. }))
    }

    /// The lamp states that will actually be written (desired states minus safety blocks).
    pub fn effective(&self, now: Instant) -> LampStates {
        let mut states = self.desired;
        if self.heat_block(now).is_some() {
            states.heat = false;
        }
        if self.restrike_remaining(Relay::Uv1, now).is_some() {
            states.uv1 = false;
        }
        if self.restrike_remaining(Relay::Uv2, now).is_some() {
            states.uv2 = false;
        }
        states
    }

    /// Writes the effective states to the relays and starts the re-strike pause of every UV
    /// lamp that goes off. Returns the relays that changed.
    pub fn apply(&mut self, hw: &SharedHardware, now: Instant) -> Result<Vec<(Relay, bool)>> {
        let target = self.effective(now);
        let mut hw = hw.lock();
        let mut changed = Vec::new();
        for (relay, on) in [(Relay::Uv1, target.uv1), (Relay::Uv2, target.uv2), (Relay::Heat, target.heat)] {
            let was_on = hw.relay(relay);
            // Write every cycle so the output is re-asserted even if something reset it
            hw.set_relay(relay, on)?;
            if was_on != on {
                changed.push((relay, on));
                if was_on {
                    self.block_restrike(relay, now + self.restrike_pause);
                }
            }
        }
        Ok(changed)
    }
}

/// Restores the re-strike pause after a (re)start from the last stored UV switching state.
///
/// - Last state "off" at time T: the lamp stays blocked until T + pause.
/// - Last state "on": the program ended without switching the lamp off (crash, power loss), so
///   the lamp may have gone dark just now. To be safe the full pause starts at program start.
/// - No stored state (first start): no pause.
/// - A stored time in the future (wrong clock): full pause from now.
///
/// Returns log messages for every lamp that is blocked.
pub fn restore_restrike(
    light: &mut LightController,
    stored: &[storage::LampStateRow],
    now_wall: chrono::DateTime<chrono::Utc>,
    now: Instant,
) -> Vec<String> {
    let pause = light.restrike_pause();
    let mut messages = Vec::new();
    for relay in [Relay::Uv1, Relay::Uv2] {
        let Some(row) = stored.iter().find(|r| r.relay == relay.name()) else { continue };
        let remaining = if row.is_on {
            Some(pause)
        } else {
            match (now_wall - row.changed_at).to_std() {
                Ok(elapsed) if elapsed >= pause => None,
                Ok(elapsed) => Some(pause - elapsed),
                Err(_) => Some(pause), // stored time lies in the future
            }
        };
        if let Some(remaining) = remaining {
            light.block_restrike(relay, now + remaining);
            let reason = if row.is_on { "was on at shutdown/power loss" } else { "switched off recently" };
            messages.push(format!(
                "{} re-strike pause after start: {} min ({reason})",
                relay.name(),
                remaining.as_secs().div_ceil(60)
            ));
        }
    }
    messages
}

/// Applies the current lamp states, logs every relay change and stores UV changes
/// (needed for the re-strike pause after a restart).
pub async fn apply_and_log(state: &AppState) -> Result<()> {
    let changed = {
        let mut light = state.light.lock().await;
        light.apply(&state.hw, Instant::now())?
    };
    for (relay, on) in changed {
        logs::log(&state.pool, Level::Info, &format!("{} switched {}", relay.name(), if on { "on" } else { "off" })).await;
        if uv_index(relay).is_some() {
            if let Err(e) = storage::save_lamp_state(&state.pool, relay.name(), on, chrono::Utc::now()).await {
                logs::log(&state.pool, Level::Error, &format!("Failed to store {} state: {e:#}", relay.name())).await;
            }
        }
    }
    Ok(())
}

/// One cycle of the light loop: evaluate the schedule (unless in setup mode) and switch.
pub async fn update_lights(state: &AppState) -> Result<()> {
    if !state.config.main.setup_mode {
        let now = Local::now();
        let schedule = storage::effective_schedule(&state.pool, &state.config, now.iso_week().week()).await;
        let desired = LampStates::from_schedule(&schedule, now.time());
        state.light.lock().await.set_desired(desired);
    }
    apply_and_log(state).await
}

/// Background task: runs `update_lights` every 30 s.
pub async fn run(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        if let Err(e) = update_lights(&state).await {
            logs::log(&state.pool, Level::Error, &format!("Light update failed: {e:#}")).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::gpio::{Climate, Hardware};
    use crate::modules::models::{parse_time, Rgbww, TimeWindow};
    use chrono::{TimeZone, Utc};

    const INTERVAL: Duration = Duration::from_secs(60);
    const PAUSE: Duration = Duration::from_secs(900);
    const COOLDOWN: Duration = Duration::from_secs(1800);

    /// Records relay states only.
    #[derive(Default)]
    struct FakeHw {
        relays: [bool; 4],
    }

    fn idx(r: Relay) -> usize {
        match r {
            Relay::Uv1 => 0,
            Relay::Uv2 => 1,
            Relay::Heat => 2,
            Relay::LedPsu => 3,
        }
    }

    impl Hardware for FakeHw {
        fn backend(&self) -> &'static str {
            "fake"
        }
        fn set_relay(&mut self, relay: Relay, on: bool) -> Result<()> {
            self.relays[idx(relay)] = on;
            Ok(())
        }
        fn relay(&self, relay: Relay) -> bool {
            self.relays[idx(relay)]
        }
        fn write_leds(&mut self, _: Rgbww) -> Result<()> {
            Ok(())
        }
        fn read_onewire(&mut self, _: &str) -> Result<f32> {
            anyhow::bail!("not used")
        }
        fn read_climate(&mut self) -> Result<Climate> {
            anyhow::bail!("not used")
        }
    }

    fn controller() -> LightController {
        let cfg = LightControlConfig {
            overheat_temp: 50.0,
            overheat_cooldown_secs: COOLDOWN.as_secs(),
            restrike_pause_secs: PAUSE.as_secs(),
        };
        LightController::new(&cfg, INTERVAL)
    }

    fn hw() -> SharedHardware {
        SharedHardware::new(Box::new(FakeHw::default()))
    }

    fn all_on() -> LampStates {
        LampStates { uv1: true, uv2: true, heat: true }
    }

    fn relays(hw: &SharedHardware) -> (bool, bool, bool) {
        let hw = hw.lock();
        (hw.relay(Relay::Uv1), hw.relay(Relay::Uv2), hw.relay(Relay::Heat))
    }

    // ----- schedule -----

    #[test]
    fn lamp_states_follow_the_schedule() {
        let w = |on, off| Some(TimeWindow { on: parse_time(on).unwrap(), off: parse_time(off).unwrap() });
        let schedule = WeekSchedule {
            uv1: w("06:30", "19:00"),
            uv2: None, // brumation: off all day
            heat: w("10:00", "14:30"),
            led: w("06:00", "20:00"),
            led_color: Rgbww::OFF,
        };
        let at = |t| LampStates::from_schedule(&schedule, parse_time(t).unwrap());
        assert_eq!(at("06:29"), LampStates { uv1: false, uv2: false, heat: false });
        assert_eq!(at("06:30"), LampStates { uv1: true, uv2: false, heat: false });
        assert_eq!(at("12:00"), LampStates { uv1: true, uv2: false, heat: true });
        assert_eq!(at("14:30"), LampStates { uv1: true, uv2: false, heat: false });
        assert_eq!(at("19:00"), LampStates::default());
    }

    // ----- heat safety -----

    #[test]
    fn heat_stays_off_until_the_first_reading() {
        let (mut light, hw, t0) = (controller(), hw(), Instant::now());
        light.set_desired(all_on());
        assert_eq!(light.heat_block(t0), Some(HeatBlock::NoReading));
        light.apply(&hw, t0).unwrap();
        assert_eq!(relays(&hw), (true, true, false));

        light.update_temperature(Some(30.0), t0);
        assert_eq!(light.heat_block(t0), None);
        light.apply(&hw, t0).unwrap();
        assert_eq!(relays(&hw), (true, true, true));
    }

    #[test]
    fn sensor_failure_switches_heat_off_and_recovers() {
        let (mut light, hw, t0) = (controller(), hw(), Instant::now());
        light.set_desired(all_on());
        light.update_temperature(Some(30.0), t0);
        light.apply(&hw, t0).unwrap();
        assert!(relays(&hw).2);

        let msg = light.update_temperature(None, t0 + INTERVAL);
        assert!(matches!(msg, Some((Level::Error, _))));
        assert_eq!(light.heat_block(t0 + INTERVAL), Some(HeatBlock::SensorFault));
        light.apply(&hw, t0 + INTERVAL).unwrap();
        assert_eq!(relays(&hw), (true, true, false), "UV keeps running, heat goes off");

        let msg = light.update_temperature(Some(31.0), t0 + INTERVAL * 2);
        assert!(matches!(msg, Some((Level::Info, _))));
        assert_eq!(light.heat_block(t0 + INTERVAL * 2), None);
    }

    #[test]
    fn stale_reading_counts_as_sensor_fault() {
        let (mut light, t0) = (controller(), Instant::now());
        light.update_temperature(Some(30.0), t0);
        assert_eq!(light.heat_block(t0 + INTERVAL * 3), None);
        assert_eq!(light.heat_block(t0 + INTERVAL * 3 + Duration::from_secs(1)), Some(HeatBlock::SensorFault));
    }

    #[test]
    fn overheat_blocks_heat_for_the_cooldown() {
        let (mut light, hw, t0) = (controller(), hw(), Instant::now());
        light.set_desired(all_on());
        light.update_temperature(Some(45.0), t0);
        light.apply(&hw, t0).unwrap();
        assert!(relays(&hw).2);

        // Reaching the threshold switches the heat spot off at once
        let msg = light.update_temperature(Some(50.0), t0);
        assert!(matches!(msg, Some((Level::Error, ref m)) if m.contains("OVERHEAT")));
        assert_eq!(light.heat_block(t0), Some(HeatBlock::Overheat { temp: 50.0 }));
        assert!(light.is_overheat(t0));
        light.apply(&hw, t0).unwrap();
        assert_eq!(relays(&hw), (true, true, false));

        // Temperature drops, but the cooldown keeps the spot off
        let t1 = t0 + INTERVAL;
        light.update_temperature(Some(40.0), t1);
        assert!(matches!(light.heat_block(t1), Some(HeatBlock::Cooldown { .. })));
        assert!(light.is_overheat(t1));

        // After the cooldown the spot may run again
        let mut t = t1;
        while t < t0 + COOLDOWN {
            t += INTERVAL;
            light.update_temperature(Some(40.0), t);
        }
        assert_eq!(light.heat_block(t), None);
        light.apply(&hw, t).unwrap();
        assert!(relays(&hw).2);
    }

    #[test]
    fn every_reading_above_the_limit_restarts_the_cooldown() {
        let (mut light, t0) = (controller(), Instant::now());
        assert!(light.update_temperature(Some(51.0), t0).is_some());
        // Second high reading: no new log message, but the cooldown starts over
        let t1 = t0 + Duration::from_secs(600);
        assert!(light.update_temperature(Some(52.0), t1).is_none());
        let t2 = t1 + INTERVAL;
        light.update_temperature(Some(40.0), t2);
        match light.heat_block(t2) {
            Some(HeatBlock::Cooldown { remaining_secs }) => {
                assert_eq!(remaining_secs, (COOLDOWN - INTERVAL).as_secs())
            }
            other => panic!("expected cooldown, got {other:?}"),
        }
    }

    // ----- re-strike pause -----

    #[test]
    fn uv_lamp_waits_for_the_restrike_pause() {
        let (mut light, hw, t0) = (controller(), hw(), Instant::now());
        light.set_desired(LampStates { uv1: true, uv2: true, heat: false });
        light.apply(&hw, t0).unwrap();
        assert_eq!(relays(&hw), (true, true, false));

        // UV1 goes off (e.g. schedule end), then is requested again right away
        light.set_desired(LampStates { uv1: false, uv2: true, heat: false });
        let changed = light.apply(&hw, t0).unwrap();
        assert_eq!(changed, vec![(Relay::Uv1, false)]);
        light.set_desired(LampStates { uv1: true, uv2: true, heat: false });
        let t1 = t0 + PAUSE - Duration::from_secs(1);
        light.apply(&hw, t1).unwrap();
        assert_eq!(relays(&hw), (false, true, false), "UV1 must stay off during the pause");
        assert_eq!(light.restrike_status(t1), RestrikeStatus { uv1: Some(2), uv2: None });

        light.apply(&hw, t0 + PAUSE).unwrap();
        assert_eq!(relays(&hw), (true, true, false));
        assert_eq!(light.restrike_status(t0 + PAUSE), RestrikeStatus::default());
    }

    #[test]
    fn heat_spot_has_no_restrike_pause() {
        let (mut light, hw, t0) = (controller(), hw(), Instant::now());
        light.update_temperature(Some(30.0), t0);
        light.set_desired(LampStates { uv1: false, uv2: false, heat: true });
        light.apply(&hw, t0).unwrap();
        light.set_desired(LampStates::default());
        light.apply(&hw, t0).unwrap();
        light.set_desired(LampStates { uv1: false, uv2: false, heat: true });
        light.apply(&hw, t0 + Duration::from_secs(1)).unwrap();
        assert!(relays(&hw).2);
    }

    fn row(relay: &str, is_on: bool, changed_at: chrono::DateTime<Utc>) -> storage::LampStateRow {
        storage::LampStateRow { relay: relay.into(), is_on, changed_at }
    }

    #[test]
    fn restrike_pause_survives_a_restart() {
        let wall = Utc.with_ymd_and_hms(2026, 11, 20, 12, 0, 0).unwrap();
        let t0 = Instant::now();
        let minutes = |m| chrono::Duration::minutes(m);

        // UV1 was on when the program died, UV2 was switched off 5 min ago
        let mut light = controller();
        let rows = [row("UV1", true, wall - minutes(60)), row("UV2", false, wall - minutes(5))];
        let msgs = restore_restrike(&mut light, &rows, wall, t0);
        assert_eq!(msgs.len(), 2);
        assert_eq!(light.restrike_status(t0), RestrikeStatus { uv1: Some(901), uv2: Some(601) });

        // Switched off long ago, or never stored: no pause
        let mut light = controller();
        let msgs = restore_restrike(&mut light, &[row("UV1", false, wall - minutes(16))], wall, t0);
        assert!(msgs.is_empty());
        assert_eq!(light.restrike_status(t0), RestrikeStatus::default());

        // Stored time in the future (clock was wrong): full pause
        let mut light = controller();
        restore_restrike(&mut light, &[row("UV2", false, wall + minutes(30))], wall, t0);
        assert_eq!(light.restrike_status(t0), RestrikeStatus { uv1: None, uv2: Some(901) });

        // The restored pause really keeps the lamp off
        let hw = hw();
        let mut light = controller();
        restore_restrike(&mut light, &[row("UV1", true, wall)], wall, t0);
        light.set_desired(LampStates { uv1: true, uv2: true, heat: false });
        light.apply(&hw, t0).unwrap();
        assert_eq!(relays(&hw), (false, true, false));
        light.apply(&hw, t0 + PAUSE).unwrap();
        assert_eq!(relays(&hw), (true, true, false));
    }
}
