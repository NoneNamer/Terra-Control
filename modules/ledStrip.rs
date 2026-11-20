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
