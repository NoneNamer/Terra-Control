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
        // Ensure all pins are non-zero (or use another logic as per your requirements)
        if self.ds18b20_bus == 0 {
            return Err("Missing / invalid value in get_data: ds18b20_bus".to_string());
        }
        if self.dht22 == 0 {
            return Err("Missing / invalid value in get_data: dht22".to_string());
        }
        if self.veml6075_uv1 == 0 {
            return Err("Missing / invalid value in get_data: veml6075_uv1".to_string());
        }
        if self.veml6075_uv2 == 0 {
            return Err("Missing / invalid value in get_data: veml6075_uv2".to_string());
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