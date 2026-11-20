//! Loading and validation of `config.toml`.

use crate::modules::models::{LedMode, LedPresets, Rgbww, TimeWindow, WeekSchedule};
use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// Default location of the configuration file (relative to the working directory).
pub const CONFIG_PATH: &str = "config.toml";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub main: MainConfig,
    pub hardware: HardwareConfig,
    pub relays: RelayConfig,
    pub sensors: SensorConfig,
    pub thresholds: Thresholds,
    pub light_control: LightControlConfig,
    pub led: LedConfig,
    pub web: WebConfig,
    pub storage: StorageConfig,
    pub schedule_default: ScheduleDefault,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MainConfig {
    /// Commissioning mode: schedule paused, lamps only switched manually
    pub setup_mode: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareConfig {
    /// Use the simulation backend (forced on non-Linux targets)
    pub simulate: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelayConfig {
    pub uv1: u8,
    pub uv2: u8,
    pub heat: u8,
    /// Optional LED power supply relay (to be retrofitted)
    pub led_psu: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensorConfig {
    pub basking_id: String,
    pub cool_id: String,
    pub enclosure_id: String,
    #[allow(dead_code)] // used by the SHT3x driver (phase 2)
    pub sht3x_address: u16,
    pub interval_secs: u64,
    pub retries: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thresholds {
    pub basking_max: f32,
    pub cool_min: f32,
    pub cool_max: f32,
    pub humidity_min: f32,
    pub humidity_max: f32,
    pub enclosure_max: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightControlConfig {
    pub overheat_temp: f32,
    pub overheat_cooldown_secs: u64,
    pub restrike_pause_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedConfig {
    pub data_pin_strip1: u8,
    pub data_pin_strip2: u8,
    #[allow(dead_code)] // used by the WS2805 driver (phase 2)
    pub ic_count_strip1: usize,
    #[allow(dead_code)]
    pub ic_count_strip2: usize,
    pub default_mode: LedMode,
    pub default_brightness: u8,
    pub season_weight: f32,
    pub morning: Rgbww,
    pub noon: Rgbww,
    pub evening: Rgbww,
}

impl LedConfig {
    pub fn presets(&self) -> LedPresets {
        LedPresets { morning: self.morning, noon: self.noon, evening: self.evening }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebConfig {
    pub address: String,
    pub port: u16,
    pub static_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    pub database: String,
    pub storage_days: u32,
}

/// Schedule used for weeks without their own database row.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleDefault {
    pub uv1_on: Option<String>,
    pub uv1_off: Option<String>,
    pub uv2_on: Option<String>,
    pub uv2_off: Option<String>,
    pub heat_on: Option<String>,
    pub heat_off: Option<String>,
    pub led_on: Option<String>,
    pub led_off: Option<String>,
    pub led_color: Rgbww,
}

impl ScheduleDefault {
    pub fn to_week_schedule(&self) -> Result<WeekSchedule, String> {
        let w = |name, on: &Option<String>, off: &Option<String>| {
            TimeWindow::parse_pair(name, on.as_deref(), off.as_deref())
        };
        Ok(WeekSchedule {
            uv1: w("uv1", &self.uv1_on, &self.uv1_off)?,
            uv2: w("uv2", &self.uv2_on, &self.uv2_off)?,
            heat: w("heat", &self.heat_on, &self.heat_off)?,
            led: w("led", &self.led_on, &self.led_off)?,
            led_color: self.led_color,
        })
    }
}

impl Config {
    /// Reads, parses and validates the configuration file.
    pub fn load(path: &str) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read configuration file '{path}'"))?;
        let config: Config = toml::from_str(&text)
            .with_context(|| format!("failed to parse configuration file '{path}'"))?;
        config.validate().with_context(|| format!("invalid configuration in '{path}'"))?;
        Ok(config)
    }

    /// The validated default schedule (validity is checked in `validate`).
    pub fn default_schedule(&self) -> WeekSchedule {
        self.schedule_default
            .to_week_schedule()
            .expect("schedule_default is validated on load")
    }

    pub fn validate(&self) -> Result<()> {
        // GPIO pins: valid BCM numbers, no pin used twice
        let mut pins = vec![
            ("relays.uv1", self.relays.uv1),
            ("relays.uv2", self.relays.uv2),
            ("relays.heat", self.relays.heat),
            ("led.data_pin_strip1", self.led.data_pin_strip1),
            ("led.data_pin_strip2", self.led.data_pin_strip2),
        ];
        if let Some(pin) = self.relays.led_psu {
            pins.push(("relays.led_psu", pin));
        }
        for (i, (name, pin)) in pins.iter().enumerate() {
            if *pin > 27 {
                bail!("{name}: GPIO {pin} does not exist (0-27)");
            }
            if let Some((other, _)) = pins[..i].iter().find(|(_, p)| p == pin) {
                bail!("GPIO {pin} is used by both {other} and {name}");
            }
        }

        let s = &self.sensors;
        let ids = [("basking_id", &s.basking_id), ("cool_id", &s.cool_id), ("enclosure_id", &s.enclosure_id)];
        for (i, (name, id)) in ids.iter().enumerate() {
            if id.trim().is_empty() {
                bail!("sensors.{name} must not be empty");
            }
            if ids[..i].iter().any(|(_, other)| other == id) {
                bail!("sensors.{name}: 1-Wire ID '{id}' is assigned twice");
            }
        }
        if s.interval_secs < 10 {
            bail!("sensors.interval_secs must be at least 10 (got {})", s.interval_secs);
        }
        if s.retries == 0 {
            bail!("sensors.retries must be at least 1");
        }

        let lc = &self.light_control;
        if !(30.0..=60.0).contains(&lc.overheat_temp) {
            bail!("light_control.overheat_temp must be 30-60 °C (got {})", lc.overheat_temp);
        }
        if lc.overheat_cooldown_secs < 900 {
            bail!("light_control.overheat_cooldown_secs must be at least 900 (got {})", lc.overheat_cooldown_secs);
        }
        if lc.restrike_pause_secs < 900 {
            bail!("light_control.restrike_pause_secs must be at least 900 (got {})", lc.restrike_pause_secs);
        }

        if self.led.default_brightness > 100 {
            bail!("led.default_brightness must be 0-100 (got {})", self.led.default_brightness);
        }
        if !(0.0..=1.0).contains(&self.led.season_weight) {
            bail!("led.season_weight must be 0.0-1.0 (got {})", self.led.season_weight);
        }

        if self.web.address.trim().is_empty() {
            bail!("web.address must not be empty");
        }
        if self.web.port == 0 {
            bail!("web.port must not be 0");
        }
        if self.storage.storage_days == 0 {
            bail!("storage.storage_days must be at least 1");
        }

        if let Err(e) = self.schedule_default.to_week_schedule() {
            bail!("schedule_default: {e}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_config_is_valid() {
        Config::load(CONFIG_PATH).expect("config.toml must load");
    }
}
