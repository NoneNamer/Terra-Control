//! Logic for UV1, UV2 and the heat spot: schedule evaluation and overheat protection.
//! No hardware access here; relays are switched through `gpio::Hardware`.

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

pub struct LightController {
    overheat_temp: f32,
    cooldown: Duration,
    /// A basking value older than this counts as a sensor fault
    max_reading_age: Duration,
    desired: LampStates,
    basking_temp: Option<f32>,
    last_reading: Option<Instant>,
    sensor_failed: bool,
    cooldown_until: Option<Instant>,
}

impl LightController {
    pub fn new(config: &LightControlConfig, sensor_interval: Duration) -> Self {
        LightController {
            overheat_temp: config.overheat_temp,
            cooldown: Duration::from_secs(config.overheat_cooldown_secs),
            max_reading_age: sensor_interval * 3,
            desired: LampStates::default(),
            basking_temp: None,
            last_reading: None,
            sensor_failed: false,
            cooldown_until: None,
        }
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
        states
    }

    /// Writes the effective states to the relays. Returns the relays that changed.
    pub fn apply(&self, hw: &SharedHardware, now: Instant) -> Result<Vec<(Relay, bool)>> {
        let target = self.effective(now);
        let mut hw = hw.lock();
        let mut changed = Vec::new();
        for (relay, on) in [(Relay::Uv1, target.uv1), (Relay::Uv2, target.uv2), (Relay::Heat, target.heat)] {
            if hw.relay(relay) != on {
                changed.push((relay, on));
            }
            // Write every cycle so the output is re-asserted even if something reset it
            hw.set_relay(relay, on)?;
        }
        Ok(changed)
    }
}

/// Applies the current lamp states and logs every relay change.
pub async fn apply_and_log(state: &AppState) -> Result<()> {
    let changed = {
        let light = state.light.lock().await;
        light.apply(&state.hw, Instant::now())?
    };
    for (relay, on) in changed {
        logs::log(&state.pool, Level::Info, &format!("{} switched {}", relay.name(), if on { "on" } else { "off" })).await;
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
