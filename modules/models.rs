//! Shared data types used by the logic modules, the storage layer and the web API.

use chrono::{NaiveDateTime, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};

/// Time format used in the schedule ("HH:MM").
pub const TIME_FORMAT: &str = "%H:%M";
/// Timestamp format used in the database (local time).
pub const TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// One colour value for the 5-channel WS2805 LEDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rgbww {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub ww: u8,
    pub cw: u8,
}

impl Rgbww {
    pub const OFF: Rgbww = Rgbww { r: 0, g: 0, b: 0, ww: 0, cw: 0 };

    pub fn is_off(&self) -> bool {
        *self == Self::OFF
    }

    /// Linear interpolation between `self` (factor 0.0) and `other` (factor 1.0).
    pub fn mix(&self, other: &Rgbww, factor: f32) -> Rgbww {
        let f = factor.clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| (a as f32 * (1.0 - f) + b as f32 * f).round() as u8;
        Rgbww {
            r: lerp(self.r, other.r),
            g: lerp(self.g, other.g),
            b: lerp(self.b, other.b),
            ww: lerp(self.ww, other.ww),
            cw: lerp(self.cw, other.cw),
        }
    }

    /// Scales all channels by a brightness in percent (0-100).
    pub fn scaled(&self, brightness_percent: u8) -> Rgbww {
        let f = brightness_percent.min(100) as f32 / 100.0;
        let scale = |v: u8| (v as f32 * f).round() as u8;
        Rgbww {
            r: scale(self.r),
            g: scale(self.g),
            b: scale(self.b),
            ww: scale(self.ww),
            cw: scale(self.cw),
        }
    }
}

/// Daily on/off window. If `off` is earlier than `on`, the window spans midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeWindow {
    pub on: NaiveTime,
    pub off: NaiveTime,
}

impl TimeWindow {
    /// Parses an optional pair of "HH:MM" strings. Both missing = no window (load off all day).
    pub fn parse_pair(
        name: &str,
        on: Option<&str>,
        off: Option<&str>,
    ) -> Result<Option<TimeWindow>, String> {
        match (on, off) {
            (None, None) => Ok(None),
            (Some(on), Some(off)) => {
                let on = parse_time(on).map_err(|e| format!("{name}_on: {e}"))?;
                let off = parse_time(off).map_err(|e| format!("{name}_off: {e}"))?;
                if on == off {
                    return Err(format!("{name}: on and off time must differ"));
                }
                Ok(Some(TimeWindow { on, off }))
            }
            _ => Err(format!("{name}: on and off time must both be set or both be empty")),
        }
    }

    /// True if `t` lies within [on, off).
    pub fn contains(&self, t: NaiveTime) -> bool {
        if self.on < self.off {
            t >= self.on && t < self.off
        } else {
            t >= self.on || t < self.off
        }
    }

    /// Position of `t` inside the window: 0.0 at `on`, 1.0 at `off`. None outside the window.
    pub fn progress(&self, t: NaiveTime) -> Option<f32> {
        if !self.contains(t) {
            return None;
        }
        let day = 24 * 3600;
        let secs = |x: NaiveTime| x.num_seconds_from_midnight() as i64;
        let length = (secs(self.off) - secs(self.on)).rem_euclid(day);
        let elapsed = (secs(t) - secs(self.on)).rem_euclid(day);
        Some(elapsed as f32 / length as f32)
    }
}

pub fn parse_time(s: &str) -> Result<NaiveTime, String> {
    NaiveTime::parse_from_str(s.trim(), TIME_FORMAT)
        .map_err(|_| format!("invalid time '{s}', expected HH:MM"))
}

fn format_time(t: Option<TimeWindow>, on: bool) -> Option<String> {
    t.map(|w| if on { w.on } else { w.off }.format(TIME_FORMAT).to_string())
}

/// Validated schedule for one calendar week. `None` windows mean "off all day".
#[derive(Debug, Clone, PartialEq)]
pub struct WeekSchedule {
    pub uv1: Option<TimeWindow>,
    pub uv2: Option<TimeWindow>,
    pub heat: Option<TimeWindow>,
    pub led: Option<TimeWindow>,
    pub led_color: Rgbww,
}

/// Schedule row as stored in the database and exchanged with the web API.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ScheduleRow {
    #[serde(default)]
    pub week: i64,
    pub uv1_on: Option<String>,
    pub uv1_off: Option<String>,
    pub uv2_on: Option<String>,
    pub uv2_off: Option<String>,
    pub heat_on: Option<String>,
    pub heat_off: Option<String>,
    pub led_on: Option<String>,
    pub led_off: Option<String>,
    pub led_r: i64,
    pub led_g: i64,
    pub led_b: i64,
    pub led_ww: i64,
    pub led_cw: i64,
}

impl ScheduleRow {
    /// Validates the row and converts it into a `WeekSchedule`.
    pub fn to_week_schedule(&self) -> Result<WeekSchedule, String> {
        if !(1..=53).contains(&self.week) {
            return Err(format!("week must be 1-53, got {}", self.week));
        }
        let channel = |name: &str, v: i64| {
            u8::try_from(v).map_err(|_| format!("{name} must be 0-255, got {v}"))
        };
        // Treat empty strings from HTML forms like missing values
        fn clean(s: &Option<String>) -> Option<&str> {
            s.as_deref().filter(|s| !s.trim().is_empty())
        }
        let window = |name: &str, on: &Option<String>, off: &Option<String>| {
            TimeWindow::parse_pair(name, clean(on), clean(off))
        };
        Ok(WeekSchedule {
            uv1: window("uv1", &self.uv1_on, &self.uv1_off)?,
            uv2: window("uv2", &self.uv2_on, &self.uv2_off)?,
            heat: window("heat", &self.heat_on, &self.heat_off)?,
            led: window("led", &self.led_on, &self.led_off)?,
            led_color: Rgbww {
                r: channel("led_r", self.led_r)?,
                g: channel("led_g", self.led_g)?,
                b: channel("led_b", self.led_b)?,
                ww: channel("led_ww", self.led_ww)?,
                cw: channel("led_cw", self.led_cw)?,
            },
        })
    }

    pub fn from_week_schedule(week: i64, s: &WeekSchedule) -> Self {
        ScheduleRow {
            week,
            uv1_on: format_time(s.uv1, true),
            uv1_off: format_time(s.uv1, false),
            uv2_on: format_time(s.uv2, true),
            uv2_off: format_time(s.uv2, false),
            heat_on: format_time(s.heat, true),
            heat_off: format_time(s.heat, false),
            led_on: format_time(s.led, true),
            led_off: format_time(s.led, false),
            led_r: s.led_color.r as i64,
            led_g: s.led_color.g as i64,
            led_b: s.led_color.b as i64,
            led_ww: s.led_color.ww as i64,
            led_cw: s.led_color.cw as i64,
        }
    }
}

/// One set of sensor values. `None` = sensor could not be read.
#[derive(Debug, Clone, Serialize)]
pub struct Readings {
    pub timestamp: NaiveDateTime,
    pub basking_temp: Option<f32>,
    pub cool_temp: Option<f32>,
    pub enclosure_temp: Option<f32>,
    pub air_temp: Option<f32>,
    pub humidity: Option<f32>,
}

/// LED operating mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LedMode {
    /// Daily colour curve from the presets, mixed with the weekly base colour
    Natural,
    /// Fixed manual colour (still switched on/off by the schedule)
    Manual,
}

impl LedMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            LedMode::Natural => "natural",
            LedMode::Manual => "manual",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "natural" => Ok(LedMode::Natural),
            "manual" => Ok(LedMode::Manual),
            other => Err(format!("invalid LED mode '{other}', expected natural or manual")),
        }
    }
}

/// The three colour presets of the daily curve.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LedPresets {
    pub morning: Rgbww,
    pub noon: Rgbww,
    pub evening: Rgbww,
}

/// Persistent LED settings (single row in `led_settings`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LedSettings {
    pub enabled: bool,
    pub mode: LedMode,
    pub manual_color: Rgbww,
    pub brightness: u8,
    pub season_weight: f32,
}
