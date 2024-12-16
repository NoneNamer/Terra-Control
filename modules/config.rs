// modules/config.rs
use std::fs;
use std::error::Error;
use toml;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub pin_config: PinConfig,
    pub sensor_config: SensorConfig,
}

#[derive(Debug, Deserialize)]
pub struct PinConfig {
    pub ds18b20_bus_pin: u8,
    pub dht22_pin: u8,
    pub veml6075_uv1_pin: u8,
    pub veml6075_uv2_pin: u8,
}

#[derive(Debug, Deserialize)]
pub struct SensorConfig {
    pub ds18b20_resolution: u8,
    pub dht22_polling_interval: u64,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn Error>> {
        let config_str = fs::read_to_string("config.toml")?;
        let config: Config = toml::de::from_str(&config_str)?;
        Ok(config)
    }
}