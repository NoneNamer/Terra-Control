use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::modules::gpio::{LEDStrip, RGBWW, RelayController, RelayType};
use crate::modules::config::Config;
use chrono::{Local, NaiveTime};

/// Controls the LED strip with power management via relay.
///
/// This struct manages an LED strip with RGBWW (Red, Green, Blue, Warm White, Cool White)
/// capabilities, with power control through a relay to save energy when LEDs are not in use.
pub struct LEDController {
    led_strip: Option<LEDStrip>,
    relay_controller: Arc<Mutex<RelayController>>,
    power_state: bool,
}

/// Natural light presets for different times of day.
///
/// Represents a specific color configuration for the LED strip that mimics
/// natural lighting conditions at different times of day (morning, noon, evening).
#[derive(Clone, Copy)]
pub struct LightPreset {
    r: u8,
    g: u8,
    b: u8,
    ww: u8,
    cw: u8,
}

impl LightPreset {
    /// Creates a new LightPreset with specified RGBWW values.
    ///
    /// # Arguments
    ///
    /// * `r` - Red component (0-255)
    /// * `g` - Green component (0-255)
    /// * `b` - Blue component (0-255)
    /// * `ww` - Warm white component (0-255)
    /// * `cw` - Cool white component (0-255)
    ///
    /// # Returns
    ///
    /// A new LightPreset with the specified values
    fn new(r: u8, g: u8, b: u8, ww: u8, cw: u8) -> Self {
        Self { r, g, b, ww, cw }
    }
    
    /// Creates a morning light preset from configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The application configuration containing light settings
    ///
    /// # Returns
    ///
    /// A LightPreset with morning lighting values
    fn from_config_morning(config: &Config) -> Self {
        Self {
            r: config.led.morning_r,
            g: config.led.morning_g,
            b: config.led.morning_b,
            ww: config.led.morning_ww,
            cw: config.led.morning_cw,
        }
    }
    
    /// Creates a noon light preset from configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The application configuration containing light settings
    ///
    /// # Returns
    ///
    /// A LightPreset with noon lighting values
    fn from_config_noon(config: &Config) -> Self {
        Self {
            r: config.led.noon_r,
            g: config.led.noon_g,
            b: config.led.noon_b,
            ww: config.led.noon_ww,
            cw: config.led.noon_cw,
        }
    }
    
    /// Creates an evening light preset from configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The application configuration containing light settings
    ///
    /// # Returns
    ///
    /// A LightPreset with evening lighting values
    fn from_config_evening(config: &Config) -> Self {
        Self {
            r: config.led.evening_r,
            g: config.led.evening_g,
            b: config.led.evening_b,
            ww: config.led.evening_ww,
            cw: config.led.evening_cw,
        }
    }
    
    /// Interpolates between two light presets by a given factor.
    ///
    /// Used to smoothly transition between lighting conditions (e.g., morning to noon).
    ///
    /// # Arguments
    ///
    /// * `other` - The target preset to interpolate towards
    /// * `factor` - A value between 0.0 and 1.0 that determines how far to interpolate
    ///              (0.0 = this preset, 1.0 = other preset)
    ///
    /// # Returns
    ///
    /// A new LightPreset representing the interpolated values
    fn interpolate(&self, other: &LightPreset, factor: f32) -> Self {
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
    
    /// Converts the preset to an RGBWW struct for use with the LED controller.
    ///
    /// # Returns
    ///
    /// An RGBWW struct with the preset's color values
    fn to_rgbww(&self) -> RGBWW {
        RGBWW {
            r: self.r,
            g: self.g,
            b: self.b,
            ww: self.ww,
            cw: self.cw,
        }
    }
}

// Default presets for different times of day (fallbacks if config doesn't have values)
const MORNING_PRESET: LightPreset = LightPreset { r: 255, g: 180, b: 100, ww: 200, cw: 50 };
const NOON_PRESET: LightPreset = LightPreset { r: 255, g: 240, b: 220, ww: 50, cw: 255 };
const EVENING_PRESET: LightPreset = LightPreset { r: 255, g: 140, b: 50, ww: 255, cw: 0 };

impl LEDController {
    /// Creates a new LED controller with power management.
    ///
    /// # Arguments
    ///
    /// * `relay_controller` - Reference to the relay controller for power management
    ///
    /// # Returns
    ///
    /// A new LEDController instance
    pub fn new(relay_controller: Arc<Mutex<RelayController>>) -> Self {
        Self {
            led_strip: None,
            relay_controller,
            power_state: false,
        }
    }

    /// Initializes the LED controller.
    ///
    /// Sets up the LED strip and ensures it's in a known state.
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error
    pub async fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
        // First, turn on the power relay
        self.power_on().await?;
        
        // Wait a moment for the power to stabilize
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Initialize the LED strip
        self.led_strip = Some(LEDStrip::new()?);
        
        Ok(())
    }

    /// Powers on the LED strip via relay.
    ///
    /// Turns on power to the LED strip and waits for it to initialize.
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error
    pub async fn power_on(&mut self) -> Result<(), Box<dyn Error>> {
        let mut relay = self.relay_controller.lock().await;
        relay.turn_on(RelayType::LED);
        self.power_state = true;
        Ok(())
    }

    /// Powers off the LED strip via relay.
    ///
    /// Turns off power to the LED strip to save energy when not in use.
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error
    pub async fn power_off(&mut self) -> Result<(), Box<dyn Error>> {
        // First turn off all LEDs if the strip is initialized
        if let Some(ref mut strip) = self.led_strip {
            strip.set_all(RGBWW::off());
            strip.show()?;
        }
        
        // Then turn off the power relay
        let mut relay = self.relay_controller.lock().await;
        relay.turn_off(RelayType::LED);
        self.power_state = false;
        
        Ok(())
    }

    /// Sets the LED strip color.
    ///
    /// Powers on the strip if needed and sets the specified color.
    ///
    /// # Arguments
    ///
    /// * `color` - The RGBWW color to set
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error
    pub async fn set_color(&mut self, color: RGBWW) -> Result<(), Box<dyn Error>> {
        // If the strip is powered off, power it on first
        if !self.power_state {
            self.power_on().await?;
            
            // Initialize the strip if needed
            if self.led_strip.is_none() {
                self.led_strip = Some(LEDStrip::new()?);
            }
        }
        
        // Set the color
        if let Some(ref mut strip) = self.led_strip {
            strip.set_all(color);
            strip.show()?;
        } else {
            return Err("LED strip not initialized".into());
        }
        
        Ok(())
    }

    /// Sets the LED color components individually.
    ///
    /// # Arguments
    ///
    /// * `r` - Red component (0-255)
    /// * `g` - Green component (0-255)
    /// * `b` - Blue component (0-255)
    /// * `ww` - Warm white component (0-255)
    /// * `cw` - Cool white component (0-255)
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error
    pub async fn set_rgbww(&mut self, r: u8, g: u8, b: u8, ww: u8, cw: u8) -> Result<(), Box<dyn Error>> {
        let color = RGBWW { r, g, b, ww, cw };
        self.set_color(color).await
    }

    /// Sets the LED color from a string representation.
    ///
    /// # Arguments
    ///
    /// * `color_str` - A string in the format "r,g,b,ww,cw"
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error
    pub async fn set_color_from_str(&mut self, color_str: &str) -> Result<(), Box<dyn Error>> {
        let color = RGBWW::from_str(color_str)?;
        self.set_color(color).await
    }

    /// Checks if the LED strip is currently powered on.
    ///
    /// # Returns
    ///
    /// True if powered on, False otherwise
    pub fn is_powered_on(&self) -> bool {
        self.power_state
    }
}

/// Calculates a natural light color based on the time of day.
///
/// This function interpolates between morning, noon, and evening light presets
/// based on the current time, and also factors in seasonal variations.
///
/// # Arguments
///
/// * `current_time` - The current time in 24-hour format (HH:MM)
/// * `morning_time` - The morning time in 24-hour format (HH:MM)
/// * `noon_time` - The noon time in 24-hour format (HH:MM)
/// * `evening_time` - The evening time in 24-hour format (HH:MM)
/// * `season_color` - A tuple of (r,g,b,ww,cw) representing seasonal color adjustment
/// * `season_weight` - A factor (0.0-1.0) for how strongly to apply seasonal adjustment
/// * `config` - The application configuration
///
/// # Returns
///
/// A Result containing a tuple of (r,g,b,ww,cw) representing the calculated color
pub fn calculate_natural_light(
    current_time: &str,
    morning_time: &str,
    noon_time: &str,
    evening_time: &str,
    season_color: &(u8, u8, u8, u8, u8),
    season_weight: f32,
    config: &Config
) -> Result<(u8, u8, u8, u8, u8), Box<dyn Error>> {
    // Parse the times
    let current = NaiveTime::parse_from_str(current_time, "%H:%M")?;
    let morning = NaiveTime::parse_from_str(morning_time, "%H:%M")?;
    let noon = NaiveTime::parse_from_str(noon_time, "%H:%M")?;
    let evening = NaiveTime::parse_from_str(evening_time, "%H:%M")?;
    
    // Create season preset from the season color
    let season_preset = LightPreset::new(
        season_color.0,
        season_color.1,
        season_color.2,
        season_color.3,
        season_color.4
    );
    
    // Get time presets from config if available
    let morning_preset = LightPreset::from_config_morning(config);
    let noon_preset = LightPreset::from_config_noon(config);
    let evening_preset = LightPreset::from_config_evening(config);
    
    // Initialize with morning preset
    let mut time_preset = morning_preset;
    let mut interpolation_factor = 0.0;
    
    // Calculate interpolation based on current time
    if current >= morning && current < noon {
        // Morning to noon transition
        let morning_seconds = morning.num_seconds_from_midnight() as f32;
        let noon_seconds = noon.num_seconds_from_midnight() as f32;
        let current_seconds = current.num_seconds_from_midnight() as f32;
        
        interpolation_factor = (current_seconds - morning_seconds) / (noon_seconds - morning_seconds);
        time_preset = morning_preset.interpolate(&noon_preset, interpolation_factor);
    } else if current >= noon && current < evening {
        // Noon to evening transition
        let noon_seconds = noon.num_seconds_from_midnight() as f32;
        let evening_seconds = evening.num_seconds_from_midnight() as f32;
        let current_seconds = current.num_seconds_from_midnight() as f32;
        
        interpolation_factor = (current_seconds - noon_seconds) / (evening_seconds - noon_seconds);
        time_preset = noon_preset.interpolate(&evening_preset, interpolation_factor);
    } else {
        // Evening or early morning - use evening preset
        time_preset = evening_preset;
    }
    
    // Blend time-based preset with season preset
    let final_preset = time_preset.interpolate(&season_preset, season_weight);
    
    // Return as a tuple
    Ok((
        final_preset.r,
        final_preset.g,
        final_preset.b,
        final_preset.ww,
        final_preset.cw
    ))
}

/// Updates the LED strip based on schedule and database settings.
///
/// This function is called periodically to:
/// 1. Check the current time against the configured schedule
/// 2. Retrieve manual settings from the database
/// 3. Calculate the appropriate colors for the current time of day
/// 4. Update the LED strip or power it off during night hours
///
/// # Arguments
///
/// * `db_pool` - Database connection for retrieving settings
/// * `led_controller` - Reference to the LED controller
/// * `config` - Application configuration
///
/// # Returns
///
/// A Result indicating success or an error
pub async fn update_leds(
    db_pool: &rusqlite::Connection,
    led_controller: &Arc<Mutex<LEDController>>,
    config: &Config
) -> Result<(), Box<dyn Error>> {
    // Get current time
    let now = Local::now();
    let current_time = now.format("%H:%M").to_string();
    
    // Try to get schedule from database first
    let schedule_result = db_pool.query_row(
        "SELECT led_start, led_end, led_r, led_g, led_b, led_cw, led_ww FROM schedule WHERE ? BETWEEN week_start AND week_end",
        [now.format("%Y-%m-%d").to_string()],
        |row| {
            Ok((
                row.get::<_, String>(0)?, // led_start
                row.get::<_, String>(1)?, // led_end
                row.get::<_, i32>(2)? as u8, // led_r
                row.get::<_, i32>(3)? as u8, // led_g
                row.get::<_, i32>(4)? as u8, // led_b
                row.get::<_, i32>(5)? as u8, // led_cw
                row.get::<_, i32>(6)? as u8, // led_ww
            ))
        }
    );
    
    // Get led settings from database
    let led_settings_result = db_pool.query_row(
        "SELECT r, g, b, ww, cw, enabled, override, season_weight FROM led_settings WHERE id = 1",
        [],
        |row| {
            Ok((
                row.get::<_, i32>(0)? as u8,  // r
                row.get::<_, i32>(1)? as u8,  // g
                row.get::<_, i32>(2)? as u8,  // b
                row.get::<_, i32>(3)? as u8,  // ww
                row.get::<_, i32>(4)? as u8,  // cw
                row.get::<_, bool>(5)?,       // enabled
                row.get::<_, i32>(6)? != 0,   // override (true if using manual settings)
                row.get::<_, f32>(7)?         // season_weight
            ))
        }
    );
    
    // Check if LEDs should be enabled based on schedule
    let (leds_enabled, morning_time, evening_time) = match &schedule_result {
        Ok((start, end, _, _, _, _, _)) => {
            // Check if current time is between start and end
            (current_time >= *start && current_time <= *end, start.clone(), end.clone())
        },
        Err(_) => (true, "07:00".to_string(), "19:00".to_string()) // Default if no schedule
    };
    
    // Fixed noon time
    let noon_time = "12:00".to_string();
    
    let mut controller = led_controller.lock().await;
    
    match led_settings_result {
        Ok((r, g, b, ww, cw, enabled, override_natural, season_weight)) => {
            if enabled && leds_enabled {
                // Get the season color from the schedule
                let season_color = match &schedule_result {
                    Ok((_, _, sr, sg, sb, scw, sww)) => (*sr, *sg, *sb, *sww, *scw),
                    Err(_) => (r, g, b, ww, cw), // Use manual settings as fallback
                };
                
                if override_natural {
                    // Use manual settings
                    controller.set_rgbww(r, g, b, ww, cw).await?;
                } else {
                    // Calculate natural light colors based on time of day and season
                    let (calc_r, calc_g, calc_b, calc_ww, calc_cw) = calculate_natural_light(
                        &current_time,
                        &morning_time,
                        &noon_time,
                        &evening_time,
                        &season_color,
                        season_weight,
                        config
                    )?;
                    
                    controller.set_rgbww(calc_r, calc_g, calc_b, calc_ww, calc_cw).await?;
                }
            } else {
                controller.power_off().await?;
            }
        },
        Err(_) => {
            // Use defaults from config if no settings found
            let (r, g, b, ww, cw, enabled) = (
                config.db.def_led_R as u8,
                config.db.def_led_G as u8,
                config.db.def_led_B as u8,
                config.db.def_led_WW as u8,
                config.db.def_led_CW as u8,
                true // Default to enabled
            );
            
            if enabled && leds_enabled {
                controller.set_rgbww(r, g, b, ww, cw).await?;
            } else {
                controller.power_off().await?;
            }
        }
    }
    
    Ok(())
}

/// Retrieves LED settings from the database.
///
/// # Arguments
///
/// * `db` - Database connection
/// * `config` - Application configuration
///
/// # Returns
///
/// A Result containing a tuple of (r,g,b,ww,cw,enabled) settings
fn get_led_settings(
    db: &rusqlite::Connection,
    config: &Config
) -> Result<(u8, u8, u8, u8, u8, bool), Box<dyn Error>> {
    // Try to get settings from database
    let result = db.query_row(
        "SELECT r, g, b, ww, cw, enabled FROM led_settings WHERE id = 1",
        [],
        |row| {
            Ok((
                row.get::<_, i32>(0)? as u8,
                row.get::<_, i32>(1)? as u8,
                row.get::<_, i32>(2)? as u8,
                row.get::<_, i32>(3)? as u8,
                row.get::<_, i32>(4)? as u8,
                row.get::<_, bool>(5)?
            ))
        }
    );
    
    match result {
        Ok(settings) => Ok(settings),
        Err(_) => {
            // Use defaults from config
            Ok((
                config.db.def_led_R as u8,
                config.db.def_led_G as u8,
                config.db.def_led_B as u8,
                config.db.def_led_WW as u8,
                config.db.def_led_CW as u8,
                true // Default to enabled
            ))
        }
    }
} 