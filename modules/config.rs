// modules/config.rs
use std::fs;
use std::error::Error;
use toml;
use chrono::NaiveTime;

//top level config struct
#[derive(Debug, Deserialize)]
pub struct Config {
    pub main: MainConfig,
    pub get_data: GetDataConfig,
    pub db: ScheduleConfig,
    pub web: WebConfig, 
}

//main config struct
#[derive(Debug, Deserialize)]
pub struct MainConfig {
    pub fixed_mode: bool,
}
//getData struct
#[derive(Debug, Deserialize)]
pub struct GetDataConfig {
    pub ds18b20_bus: u8,
    pub dht22: u8,
    pub veml6075_uv1: u8,
    pub veml6075_uv2: u8,
}
//lightControl struct
#[derive(Deserialize)]
pub struct LightControlConfig {
    pub uv_relay1: u8,
    pub uv_relay2: u8,
    pub heat_relay: u8,
    pub overheat_temp: u8,
    pub overheat_time: u64, // Time in seconds
}
// web config struct
#[derive(Debug, Deserialize)]
pub struct WebConfig {
    pub address: String,    // Web server address (e.g., "127.0.0.1")
    pub port: u16,          // Web server port (e.g., 8080)
}
//schedule struct
#[derive(Debug, Deserialize)]
pub struct ScheduleConfig {
    pub def_uv1_start: String,
    pub def_uv1_end: String,
    pub def_uv2_start: String,
    pub def_uv2_end: String,
    pub def_heat_start: String,
    pub def_heat_end: String,
    pub def_led_R: i32,
    pub def_led_G: i32,
    pub def_led_B: i32,
    pub def_led_WW: i32,
    pub def_led_CW: i32,
}

//validation logic
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        self.main.validate()?;
        self.get_data.validate()?;
        self.db.validate()?;
        self.web.validate()?;
        self.light_control.validate()?;
        Ok(())
    }
}

impl MainConfig {
    pub fn validate(&self) -> Result<(), String> {
        // No specific validation needed since fixed_mode is a boolean
        Ok(())
    }
}

impl GetDataConfig {
    pub fn validate(&self) -> Result<(), String> {
        // Validate GPIO pins
        if !(0..=27).contains(&self.ds18b20_bus) {
            return Err(format!("Invalid GPIO number for ds18b20_bus: {}", self.ds18b20_bus));
        }
        if !(0..=27).contains(&self.dht22) {
            return Err(format!("Invalid GPIO number for dht22: {}", self.dht22));
        }
        if !(0..=27).contains(&self.veml6075_uv1) {
            return Err(format!("Invalid GPIO number for veml6075_uv1: {}", self.veml6075_uv1));
        }
        if !(0..=27).contains(&self.veml6075_uv2) {
            return Err(format!("Invalid GPIO number for veml6075_uv2: {}", self.veml6075_uv2));
        }
        Ok(())
    }
}

impl LightControlConfig {
    pub fn validate(&self) -> Result<(), String> {
            // Validate GPIO pin numbers (assume valid range is 0-27 for Raspberry Pi GPIOs)
            if !(0..=27).contains(&self.uv_relay1) {
                return Err(format!("Invalid GPIO number for uv_relay1: {}", self.uv_relay1));
            }
            if !(0..=27).contains(&self.uv_relay2) {
                return Err(format!("Invalid GPIO number for uv_relay2: {}", self.uv_relay2));
            }
            if !(0..=27).contains(&self.heat_relay) {
                return Err(format!("Invalid GPIO number for heat_relay: {}", self.heat_relay));
            }

            // Validate overheat_temp (0-60 °C)
            if !(0..=60).contains(&self.overheat_temp) {
                return Err(format!(
                    "Invalid overheat_temp: {}. Must be in the range 0-60°C.",
                    self.overheat_temp
                ));
            }

            // Validate overheat_time (minimum 15 minutes = 900 seconds)
            if self.overheat_time < 900 {
                return Err(format!(
                    "Invalid overheat_time: {} seconds. Must be at least 900 seconds (15 minutes).",
                    self.overheat_time
                ));
            }

            Ok(())
    }
}

impl ScheduleConfig {
    pub fn validate(&self) -> Result<(), String> {
        // Check time formats for mandatory fields
        for (field_name, value) in &[
            ("def_uv1_start", &self.def_uv1_start),
            ("def_uv1_end", &self.def_uv1_end),
            ("def_uv2_start", &self.def_uv2_start),
            ("def_uv2_end", &self.def_uv2_end),
            ("def_heat_start", &self.def_heat_start),
            ("def_heat_end", &self.def_heat_end),
        ] {
            if Self::validate_time_format(value).is_err() {
                return Err(format!("Missing / invalid value in db: {}", field_name));
            }
        }

        // Check LED intensity ranges
        for (field_name, &value) in &[
            ("def_led_R", self.def_led_R),
            ("def_led_G", self.def_led_G),
            ("def_led_B", self.def_led_B),
            ("def_led_WW", self.def_led_WW),
            ("def_led_CW", self.def_led_CW),
        ] {
            if value < 0 || value > 255 {
                return Err(format!("Missing / invalid value in db: {}", field_name));
            }
        }

        Ok(())
    }

    fn validate_time_format(time: &str) -> Result<(), ConfigError> {
    chrono::NaiveTime::parse_from_str(time, "%H:%M").map_err(|_| 
        ConfigError::ValidationError("Invalid time format".to_string()))?;
    Ok(())
    }
}

impl WebConfig {
    pub fn validate(&self) -> Result<(), String> {
        // Ensure that the address is non-empty
        if self.address.is_empty() {
            return Err("Web server address cannot be empty".to_string());
        }

        // Ensure the port is within valid range
        if self.port == 0 || self.port > 65535 {
            return Err("Invalid port number".to_string());
        }

        Ok(())
    }
}

impl Config {
    pub fn load(config_path: &str) -> Result<Self, String> {
        // Read and parse the config file
        let config_str = std::fs::read_to_string(config_path)
            .map_err(|_| "Failed to read configuration file".to_string())?;
        let config: Config = toml::de::from_str(&config_str)
            .map_err(|_| "Failed to parse configuration file".to_string())?;

        // Validate the loaded configuration
        config.validate()?;
        Ok(config)
    }
}