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

impl WeekSchedule {
    /// Rules beyond the format: UV and heat must not run over midnight (an "off" time before
    /// the "on" time is almost always a typo and would heat the terrarium at night).
    /// The LED window may span midnight.
    pub fn validate(&self) -> Result<(), String> {
        for (name, window) in [("uv1", self.uv1), ("uv2", self.uv2), ("heat", self.heat)] {
            if let Some(w) = window {
                if w.off < w.on {
                    return Err(format!(
                        "{name}: off time {} is before on time {} (UV and heat must not run over midnight)",
                        w.off.format(TIME_FORMAT),
                        w.on.format(TIME_FORMAT)
                    ));
                }
            }
        }
        Ok(())
    }
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
        let schedule = WeekSchedule {
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
        };
        schedule.validate()?;
        Ok(schedule)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> NaiveTime {
        parse_time(s).unwrap()
    }

    fn window(on: &str, off: &str) -> TimeWindow {
        TimeWindow::parse_pair("x", Some(on), Some(off)).unwrap().unwrap()
    }

    #[test]
    fn window_contains_start_but_not_end() {
        let w = window("06:30", "19:00");
        assert!(!w.contains(t("06:29")));
        assert!(w.contains(t("06:30")));
        assert!(w.contains(t("18:59")));
        assert!(!w.contains(t("19:00")));
    }

    #[test]
    fn window_can_span_midnight() {
        let w = window("22:00", "02:00");
        assert!(w.contains(t("23:30")));
        assert!(w.contains(t("00:00")));
        assert!(w.contains(t("01:59")));
        assert!(!w.contains(t("02:00")));
        assert!(!w.contains(t("12:00")));
        assert_eq!(w.progress(t("00:00")), Some(0.5));
    }

    #[test]
    fn window_progress() {
        let w = window("06:00", "18:00");
        assert_eq!(w.progress(t("06:00")), Some(0.0));
        assert_eq!(w.progress(t("12:00")), Some(0.5));
        assert_eq!(w.progress(t("15:00")), Some(0.75));
        assert_eq!(w.progress(t("18:00")), None);
        assert_eq!(w.progress(t("05:00")), None);
    }

    #[test]
    fn time_pairs_must_be_complete_and_valid() {
        assert_eq!(TimeWindow::parse_pair("uv1", None, None), Ok(None));
        assert!(TimeWindow::parse_pair("uv1", Some("06:00"), None).is_err());
        assert!(TimeWindow::parse_pair("uv1", Some("25:00"), Some("10:00")).is_err());
        assert!(TimeWindow::parse_pair("uv1", Some("06:00"), Some("06:00")).is_err());
        assert!(TimeWindow::parse_pair("uv1", Some("6:5"), Some("10:00")).is_ok(), "chrono accepts 6:5");
    }

    fn row() -> ScheduleRow {
        ScheduleRow {
            week: 12,
            uv1_on: Some("06:30".into()),
            uv1_off: Some("19:00".into()),
            uv2_on: Some("".into()),
            uv2_off: Some(" ".into()),
            heat_on: None,
            heat_off: None,
            led_on: Some("6:00".into()),
            led_off: Some("20:00".into()),
            led_r: 1,
            led_g: 2,
            led_b: 3,
            led_ww: 4,
            led_cw: 5,
        }
    }

    #[test]
    fn schedule_row_validation_and_normalisation() {
        let s = row().to_week_schedule().unwrap();
        assert_eq!(s.uv1, Some(window("06:30", "19:00")));
        assert_eq!(s.uv2, None, "empty strings from forms mean off");
        assert_eq!(s.heat, None);
        assert_eq!(s.led_color, Rgbww { r: 1, g: 2, b: 3, ww: 4, cw: 5 });

        let normalised = ScheduleRow::from_week_schedule(12, &s);
        assert_eq!(normalised.led_on.as_deref(), Some("06:00"));
        assert_eq!(normalised.uv2_on, None);
        assert_eq!(normalised.to_week_schedule().unwrap(), s);

        let mut bad = row();
        bad.week = 54;
        assert!(bad.to_week_schedule().is_err());
        let mut bad = row();
        bad.led_cw = 256;
        assert!(bad.to_week_schedule().is_err());
        let mut bad = row();
        bad.heat_on = Some("10:00".into());
        assert!(bad.to_week_schedule().is_err(), "half a time pair");
    }

    #[test]
    fn colour_mixing_and_brightness() {
        let a = Rgbww { r: 0, g: 100, b: 200, ww: 255, cw: 10 };
        let b = Rgbww { r: 100, g: 100, b: 0, ww: 0, cw: 20 };
        assert_eq!(a.mix(&b, 0.0), a);
        assert_eq!(a.mix(&b, 1.0), b);
        assert_eq!(a.mix(&b, 0.5), Rgbww { r: 50, g: 100, b: 100, ww: 128, cw: 15 });
        assert_eq!(a.scaled(100), a);
        assert_eq!(a.scaled(0), Rgbww::OFF);
        assert_eq!(a.scaled(50), Rgbww { r: 0, g: 50, b: 100, ww: 128, cw: 5 });
        assert_eq!(a.scaled(200), a, "brightness is capped at 100 %");
    }

    #[test]
    fn uv_and_heat_must_not_run_over_midnight() {
        let mut r = row();
        r.heat_on = Some("19:00".into());
        r.heat_off = Some("06:30".into());
        let err = r.to_week_schedule().unwrap_err();
        assert!(err.contains("heat"), "{err}");

        let mut r = row();
        r.uv1_on = Some("22:00".into());
        r.uv1_off = Some("02:00".into());
        assert!(r.to_week_schedule().is_err());

        // A night light over midnight is fine for the LEDs
        let mut r = row();
        r.led_on = Some("22:00".into());
        r.led_off = Some("02:00".into());
        assert!(r.to_week_schedule().is_ok());
    }
}
