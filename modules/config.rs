// modules/config.rs
use std::fs;
use std::error::Error;
use toml;
use chrono::NaiveTime;

//top level config struct
#[derive(Debug, Deserialize)]
pub struct Config {
    pub main: MainConfig,
    pub gpio: GpioConfig,
    pub db: ScheduleConfig,
    pub web: WebConfig, 
    pub light_control: LightControlConfig,
    pub get_data: GetDataConfig,
    pub led: LedConfig,
}

//main config struct
#[derive(Debug, Deserialize)]
pub struct MainConfig {
    pub debug: bool,
}

//GPIO struct
#[derive(Debug, Deserialize)]
pub struct GpioConfig {
    pub uv_relay1: u8,
    pub uv_relay2: u8,
    pub heat_relay: u8,
    pub led_relay: u8,
    pub ic_count: Option<usize>,
    pub ds18b20_bus: Option<u8>,
    pub dht22_pin: Option<u8>,
    pub veml6075_uv1: u8,
    pub veml6075_uv2: u8,
}

//lightControl struct
#[derive(Deserialize)]
pub struct LightControlConfig {
    pub overheat_temp: u8,
    pub overheat_time: u64, // Time in seconds
}

// New GetDataConfig struct
#[derive(Debug, Clone, Deserialize)]
pub struct GetDataConfig {
    pub retry: u8,              // Number of retries for failed sensor readings
    pub interval: Option<u64>,  // Interval in seconds for data collection (default: 60)
    pub backup_sensor: bool,    // Whether to use DHT22 as backup for overheat detection
    pub storage_days: Option<u32>, // How many days of data to keep (for automatic cleanup)
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

// LED configuration
#[derive(Debug, Clone, Deserialize)]
pub struct LedConfig {
    pub default_mode: String,                     // Either "manual" or "natural"
    pub default_brightness: u8,                   // 0-100% brightness
    pub season_weight: f32,                       // 0.0 - 1.0 weight of season color

    // Natural light presets
    pub morning_r: u8,
    pub morning_g: u8,
    pub morning_b: u8,
    pub morning_ww: u8,
    pub morning_cw: u8,

    pub noon_r: u8,
    pub noon_g: u8,
    pub noon_b: u8,
    pub noon_ww: u8,
    pub noon_cw: u8,

    pub evening_r: u8,
    pub evening_g: u8,
    pub evening_b: u8,
    pub evening_ww: u8,
    pub evening_cw: u8,
}

//validation logic
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        self.main.validate()?;
        self.get_data.validate()?;
        self.db.validate()?;
        self.web.validate()?;
        self.light_control.validate()?;
        self.led.validate()?;
        Ok(())
    }
}

impl MainConfig {
    pub fn validate(&self) -> Result<(), String> {
        // No specific validation needed since debug is a boolean
        Ok(())
    }
}

impl GpioConfig {
    pub fn load() -> Self {
        let config_str = std::fs::read_to_string("config.toml")
            .expect("Failed to read config.toml");
        
        let config: toml::Value = toml::from_str(&config_str)
            .expect("Failed to parse config.toml");
        
        let gpio = config.get("gpio")
            .expect("Missing [gpio] section in config.toml");
        
        Self {
            uv_relay1: gpio.get("uv_relay1")
                .and_then(|v| v.as_integer())
                .map(|v| v as u8)
                .expect("Missing or invalid uv_relay1 in config"),
                
            uv_relay2: gpio.get("uv_relay2")
                .and_then(|v| v.as_integer())
                .map(|v| v as u8)
                .expect("Missing or invalid uv_relay2 in config"),
                
            heat_relay: gpio.get("heat_relay")
                .and_then(|v| v.as_integer())
                .map(|v| v as u8)
                .expect("Missing or invalid heat_relay in config"),
                
            led_relay: gpio.get("led_relay")
                .and_then(|v| v.as_integer())
                .map(|v| v as u8)
                .expect("Missing or invalid led_relay in config"),
                
            ic_count: gpio.get("ic_count")
                .and_then(|v| v.as_integer())
                .map(|v| v as usize),
                
            ds18b20_bus: gpio.get("ds18b20_bus")
                .and_then(|v| v.as_integer())
                .map(|v| v as u8),
                
            dht22_pin: gpio.get("dht22_pin")
                .and_then(|v| v.as_integer())
                .map(|v| v as u8),
            
            veml6075_uv1: gpio.get("veml6075_uv1")
                .and_then(|v| v.as_integer())
                .map(|v| v as u8)
                .expect("Missing or invalid veml6075_uv1 in config"),
            
            veml6075_uv2: gpio.get("veml6075_uv2")
                .and_then(|v| v.as_integer())
                .map(|v| v as u8)
                .expect("Missing or invalid veml6075_uv2 in config"),
        }
    }
    
    pub fn validate(&self) -> Result<(), String> {
        // Validate GPIO pin numbers
        if self.uv_relay1 > 27 {
            return Err(format!("Invalid UV1 relay GPIO pin: {}", self.uv_relay1));
        }
        
        if self.uv_relay2 > 27 {
            return Err(format!("Invalid UV2 relay GPIO pin: {}", self.uv_relay2));
        }
        
        if self.heat_relay > 27 {
            return Err(format!("Invalid heat relay GPIO pin: {}", self.heat_relay));
        }
        
        if self.led_relay > 27 {
            return Err(format!("Invalid LED relay GPIO pin: {}", self.led_relay));
        }
        
        // Check for pin conflicts
        let pins = vec![self.uv_relay1, self.uv_relay2, self.heat_relay, self.led_relay];
        for i in 0..pins.len() {
            for j in i+1..pins.len() {
                if pins[i] == pins[j] {
                    return Err(format!("GPIO pin conflict: Pin {} used multiple times", pins[i]));
                }
            }
        }
        
        Ok(())
    }
}

impl LightControlConfig {
    pub fn validate(&self) -> Result<(), String> {

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

impl GetDataConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.retry == 0 {
            return Err("Retry count must be at least 1".into());
        }
        
        if let Some(interval) = self.interval {
            if interval < 10 {
                return Err(format!("Interval must be at least 10 seconds (got {})", interval));
            }
        }
        
        if let Some(days) = self.storage_days {
            if days < 1 {
                return Err(format!("Storage days must be at least 1 (got {})", days));
            }
        }
        
        Ok(())
    }
}

impl LedConfig {
    pub fn validate(&self) -> Result<(), String> {
        // Validate weight is between 0 and 1
        if self.season_weight < 0.0 || self.season_weight > 1.0 {
            return Err(format!("Season weight must be between 0.0 and 1.0, got: {}", self.season_weight));
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