//! Logic for the LED base / winter lighting. Both strips always show the same colour.
//!
//! On/off comes from the weekly schedule (`led` window). In natural mode the colour follows
//! morning -> noon -> evening across that window and is mixed with the weekly base colour
//! (`season_weight`). Manual mode replaces the colour only; the schedule still switches.

use crate::modules::gpio::{Relay, SharedHardware};
use crate::modules::logs::{self, Level};
use crate::modules::models::{LedMode, LedPresets, LedSettings, Rgbww, WeekSchedule};
use crate::modules::state::AppState;
use crate::modules::storage;
use anyhow::Result;
use chrono::{Datelike, Local, NaiveTime};
use std::time::Duration;

/// Colour of the daily curve at `progress` (0.0 = LED on time, 1.0 = LED off time).
pub fn natural_color(presets: &LedPresets, progress: f32) -> Rgbww {
    let p = progress.clamp(0.0, 1.0);
    if p < 0.5 {
        presets.morning.mix(&presets.noon, p * 2.0)
    } else {
        presets.noon.mix(&presets.evening, (p - 0.5) * 2.0)
    }
}

pub struct LedController {
    settings: LedSettings,
    presets: LedPresets,
    output: Rgbww,
}

impl LedController {
    pub fn new(settings: LedSettings, presets: LedPresets) -> Self {
        LedController { settings, presets, output: Rgbww::OFF }
    }

    pub fn settings(&self) -> LedSettings {
        self.settings
    }

    pub fn set_settings(&mut self, settings: LedSettings) {
        self.settings = settings;
    }

    pub fn presets(&self) -> LedPresets {
        self.presets
    }

    pub fn set_presets(&mut self, presets: LedPresets) {
        self.presets = presets;
    }

    /// Colour currently written to the strips.
    pub fn output(&self) -> Rgbww {
        self.output
    }

    /// Colour the strips should show now. In setup mode the schedule window is ignored.
    pub fn target_color(&self, schedule: &WeekSchedule, t: NaiveTime, setup_mode: bool) -> Rgbww {
        if !self.settings.enabled {
            return Rgbww::OFF;
        }
        let progress = schedule.led.and_then(|w| w.progress(t));
        if progress.is_none() && !setup_mode {
            return Rgbww::OFF;
        }
        let color = match self.settings.mode {
            LedMode::Manual => self.settings.manual_color,
            LedMode::Natural => natural_color(&self.presets, progress.unwrap_or(0.5))
                .mix(&schedule.led_color, self.settings.season_weight),
        };
        color.scaled(self.settings.brightness)
    }

    /// Writes `color` to the strips and switches the optional LED PSU relay.
    pub fn apply(&mut self, hw: &SharedHardware, color: Rgbww) -> Result<()> {
        let mut hw = hw.lock();
        if color.is_off() {
            hw.write_leds(color)?;
            hw.set_relay(Relay::LedPsu, false)?;
        } else {
            hw.set_relay(Relay::LedPsu, true)?;
            hw.write_leds(color)?;
        }
        self.output = color;
        Ok(())
    }
}

/// One cycle of the LED loop: compute the target colour and write it.
pub async fn update_leds(state: &AppState) -> Result<()> {
    let now = Local::now();
    let schedule = storage::effective_schedule(&state.pool, &state.config, now.iso_week().week()).await;
    let (was_off, color) = {
        let mut led = state.led.lock().await;
        let was_off = led.output().is_off();
        let color = led.target_color(&schedule, now.time(), state.config.main.setup_mode);
        led.apply(&state.hw, color)?;
        (was_off, color)
    };
    // Natural mode changes the colour slightly every cycle, so only on/off is logged
    if was_off != color.is_off() {
        let msg = if color.is_off() { "LEDs switched off".to_string() } else { format!("LEDs switched on ({color:?})") };
        logs::log(&state.pool, Level::Info, &msg).await;
    }
    Ok(())
}

/// Background task: runs `update_leds` every 30 s.
pub async fn run(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        if let Err(e) = update_leds(&state).await {
            logs::log(&state.pool, Level::Error, &format!("LED update failed: {e:#}")).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::models::{parse_time, TimeWindow};

    fn presets() -> LedPresets {
        LedPresets {
            morning: Rgbww { r: 200, g: 100, b: 0, ww: 200, cw: 0 },
            noon: Rgbww { r: 100, g: 100, b: 100, ww: 100, cw: 200 },
            evening: Rgbww { r: 250, g: 50, b: 0, ww: 250, cw: 0 },
        }
    }

    fn schedule(led: Option<(&str, &str)>) -> WeekSchedule {
        WeekSchedule {
            uv1: None,
            uv2: None,
            heat: None,
            led: led.map(|(on, off)| TimeWindow { on: parse_time(on).unwrap(), off: parse_time(off).unwrap() }),
            led_color: Rgbww { r: 0, g: 0, b: 255, ww: 0, cw: 0 },
        }
    }

    fn settings(mode: LedMode, brightness: u8, season_weight: f32) -> LedSettings {
        LedSettings {
            enabled: true,
            mode,
            manual_color: Rgbww { r: 10, g: 20, b: 30, ww: 40, cw: 50 },
            brightness,
            season_weight,
        }
    }

    fn t(s: &str) -> NaiveTime {
        parse_time(s).unwrap()
    }

    #[test]
    fn daily_curve_runs_morning_noon_evening() {
        let p = presets();
        assert_eq!(natural_color(&p, 0.0), p.morning);
        assert_eq!(natural_color(&p, 0.5), p.noon);
        assert_eq!(natural_color(&p, 1.0), p.evening);
        // Halfway between morning and noon
        assert_eq!(natural_color(&p, 0.25), Rgbww { r: 150, g: 100, b: 50, ww: 150, cw: 100 });
        // Out-of-range progress is clamped
        assert_eq!(natural_color(&p, -1.0), p.morning);
        assert_eq!(natural_color(&p, 2.0), p.evening);
    }

    #[test]
    fn leds_follow_the_schedule_window() {
        let led = LedController::new(settings(LedMode::Natural, 100, 0.0), presets());
        let s = schedule(Some(("06:00", "18:00")));
        assert_eq!(led.target_color(&s, t("05:59"), false), Rgbww::OFF);
        assert_eq!(led.target_color(&s, t("06:00"), false), presets().morning);
        assert_eq!(led.target_color(&s, t("12:00"), false), presets().noon);
        assert_eq!(led.target_color(&s, t("18:00"), false), Rgbww::OFF);
        // No LED window this week: off all day
        assert_eq!(led.target_color(&schedule(None), t("12:00"), false), Rgbww::OFF);
    }

    #[test]
    fn weekly_colour_is_mixed_in_by_season_weight() {
        let led = LedController::new(settings(LedMode::Natural, 100, 0.5), presets());
        let c = led.target_color(&schedule(Some(("06:00", "18:00"))), t("12:00"), false);
        // noon (100,100,100,100,200) mixed 50 % with weekly blue (0,0,255,0,0)
        assert_eq!(c, Rgbww { r: 50, g: 50, b: 178, ww: 50, cw: 100 });
    }

    #[test]
    fn manual_mode_replaces_the_colour_and_brightness_scales() {
        let led = LedController::new(settings(LedMode::Manual, 50, 0.3), presets());
        let s = schedule(Some(("06:00", "18:00")));
        assert_eq!(led.target_color(&s, t("12:00"), false), Rgbww { r: 5, g: 10, b: 15, ww: 20, cw: 25 });
        // The schedule still switches the LEDs off
        assert_eq!(led.target_color(&s, t("20:00"), false), Rgbww::OFF);
    }

    #[test]
    fn disabled_leds_stay_off_and_setup_mode_ignores_the_window() {
        let mut off = settings(LedMode::Manual, 100, 0.0);
        off.enabled = false;
        let s = schedule(Some(("06:00", "18:00")));
        assert_eq!(LedController::new(off, presets()).target_color(&s, t("12:00"), true), Rgbww::OFF);

        let led = LedController::new(settings(LedMode::Manual, 100, 0.0), presets());
        assert_eq!(led.target_color(&s, t("23:00"), true), settings(LedMode::Manual, 100, 0.0).manual_color);
        // Natural mode outside the window shows the noon colour in setup mode
        let led = LedController::new(settings(LedMode::Natural, 100, 0.0), presets());
        assert_eq!(led.target_color(&schedule(None), t("23:00"), true), presets().noon);
    }
}
