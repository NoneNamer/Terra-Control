use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, NaiveDateTime};

/// Default configuration values
#[derive(Deserialize)]
pub struct DefaultConfig {
    pub def_uv1_start: String,
    pub def_uv1_end: String,
    pub def_uv2_start: String,
    pub def_uv2_end: String,
    pub def_heat_start: String,
    pub def_heat_end: String,
    pub def_led_R: i32,
    pub def_led_G: i32,
    pub def_led_B: i32,
    pub def_led_CW: i32,
    pub def_led_WW: i32,
}

/// Weekly schedule for lighting and heating
#[derive(Debug, Serialize, Deserialize)]
pub struct Schedule {
    pub week_number: i32,
    pub uv1_start: String,
    pub uv1_end: String,
    pub uv2_start: String,
    pub uv2_end: String,
    pub heat_start: String,
    pub heat_end: String,
    pub led_start: String,
    pub led_end: String,
    pub led_r: i32,
    pub led_g: i32,
    pub led_b: i32,
    pub led_cw: i32,
    pub led_ww: i32,
}

/// Manual override settings for LED control
#[derive(Debug, Serialize, Deserialize)]
pub struct Override {
    pub id: i32,
    pub red: Option<i32>,
    pub green: Option<i32>,
    pub blue: Option<i32>,
    pub cool_white: Option<i32>,
    pub warm_white: Option<i32>,
    pub active: bool,
}

/// Historical sensor data record
#[derive(Debug, Serialize, Deserialize)]
pub struct Data {
    pub id: i32,
    pub timestamp: String,
    pub temp_basking1: Option<f32>,
    pub temp_basking2: Option<f32>,
    pub temp_cool: Option<f32>,
    pub humidity: Option<f32>,
    pub time_heat: Option<String>,
    pub overheat: Option<String>,
}

/// Current sensor readings from all sensors
#[derive(Debug)]
pub struct SensorReadings {
    pub timestamp: chrono::NaiveDateTime,
    pub basking_temp: Option<f32>,
    pub control_temp: Option<f32>,
    pub cool_temp: Option<f32>,
    pub humidity: Option<f32>,
}

/// Real-time sensor readings with control temperature
#[derive(Debug)]
pub struct CurrentReadings {
    pub timestamp: DateTime<Utc>,
    pub basking_temp: f32,
    pub control_temp: f32,
    pub cool_temp: f32,
    pub humidity: f32,
}

impl CurrentReadings {
    /// Creates a new CurrentReadings instance with default values
    pub fn new() -> Self {
        Self {
            timestamp: Utc::now(),
            basking_temp: 0.0,
            control_temp: 0.0,
            cool_temp: 0.0,
            humidity: 0.0,
        }
    }
}

/// System log entry
#[derive(Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
}

/// RGBWW color representation for LED control
#[derive(Debug, Clone, Copy)]
pub struct RGBWW {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub ww: u8,
    pub cw: u8,
}

impl RGBWW {
    /// Creates an RGBWW struct with all values set to 0 (off)
    pub fn off() -> Self {
        Self { r: 0, g: 0, b: 0, ww: 0, cw: 0 }
    }

    /// Creates an RGBWW struct from a comma-separated string
    pub fn from_str(s: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 5 {
            return Err("LED values must be in format R,G,B,WW,CW".into());
        }
        Ok(Self {
            r: parts[0].parse()?,
            g: parts[1].parse()?,
            b: parts[2].parse()?,
            ww: parts[3].parse()?,
            cw: parts[4].parse()?,
        })
    }
}

/// Natural light preset for different times of day
#[derive(Clone, Copy)]
pub struct LightPreset {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub ww: u8,
    pub cw: u8,
}

impl LightPreset {
    /// Creates a new LightPreset with specified RGBWW values
    pub fn new(r: u8, g: u8, b: u8, ww: u8, cw: u8) -> Self {
        Self { r, g, b, ww, cw }
    }
    
    /// Interpolates between two light presets by a given factor
    pub fn interpolate(&self, other: &LightPreset, factor: f32) -> Self {
        let factor = factor.clamp(0.0, 1.0);
        let r = self.r as f32 * (1.0 - factor) + other.r as f32 * factor;
        let g = self.g as f32 * (1.0 - factor) + other.g as f32 * factor;
        let b = self.b as f32 * (1.0 - factor) + other.b as f32 * factor;
        let ww = self.ww as f32 * (1.0 - factor) + other.ww as f32 * factor;
        let cw = self.cw as f32 * (1.0 - factor) + other.cw as f32 * factor;
        
        Self {
            r: r as u8,
            g: g as u8,
            b: b as u8,
            ww: ww as u8,
            cw: cw as u8,
        }
    }
    
    /// Converts the preset to an RGBWW struct
    pub fn to_rgbww(&self) -> RGBWW {
        RGBWW {
            r: self.r,
            g: self.g,
            b: self.b,
            ww: self.ww,
            cw: self.cw,
        }
    }
}