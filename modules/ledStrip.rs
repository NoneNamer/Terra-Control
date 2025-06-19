use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::modules::gpio::{LEDStrip, RelayController, RelayType};
use crate::modules::config::Config;
use crate::modules::models::{LightPreset, RGBWW};
use chrono::{Local, NaiveTime};
use sqlx::SqlitePool;

/// Controls the LED strip with power management via relay.
///
/// This struct manages an LED strip with RGBWW (Red, Green, Blue, Warm White, Cool White)
/// capabilities, with power control through a relay to save energy when LEDs are not in use.
pub struct LEDController {
    led_strip: Option<LEDStrip>,
    relay_controller: Arc<Mutex<RelayController>>,
    power_state: bool,
}

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

    /// Fades the LED strip from its current color to a target color over a specified duration.
    ///
    /// # Arguments
    ///
    /// * `target_color` - The final RGBWW color to fade to
    /// * `duration_secs` - The duration of the fade in seconds
    /// * `steps` - The number of steps to use for the fade (higher = smoother)
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error
    pub async fn fade_to(&mut self, target_color: RGBWW, duration_secs: u32, steps: u32) -> Result<(), Box<dyn Error>> {
        // Ensure the strip is powered on
        if !self.power_state {
            self.power_on().await?;
        }

        // Get current color
        let current_color = if let Some(ref strip) = self.led_strip {
            strip.get_current_color()
        } else {
            RGBWW::off()
        };

        // Calculate step duration
        let step_duration = duration_secs as f32 / steps as f32;
        let step_ms = (step_duration * 1000.0) as u64;

        // Perform the fade
        for step in 0..=steps {
            let factor = step as f32 / steps as f32;
            
            // Interpolate between current and target color
            let r = (current_color.r as f32 * (1.0 - factor) + target_color.r as f32 * factor) as u8;
            let g = (current_color.g as f32 * (1.0 - factor) + target_color.g as f32 * factor) as u8;
            let b = (current_color.b as f32 * (1.0 - factor) + target_color.b as f32 * factor) as u8;
            let ww = (current_color.ww as f32 * (1.0 - factor) + target_color.ww as f32 * factor) as u8;
            let cw = (current_color.cw as f32 * (1.0 - factor) + target_color.cw as f32 * factor) as u8;

            let color = RGBWW { r, g, b, ww, cw };
            
            // Set the color
            if let Some(ref mut strip) = self.led_strip {
                strip.set_all(color);
                strip.show()?;
            }

            // Wait for next step
            tokio::time::sleep(tokio::time::Duration::from_millis(step_ms)).await;
        }

        Ok(())
    }

    /// Fades the LED strip to off over a specified duration.
    ///
    /// # Arguments
    ///
    /// * `duration_secs` - The duration of the fade in seconds
    /// * `steps` - The number of steps to use for the fade (higher = smoother)
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error
    pub async fn fade_out(&mut self, duration_secs: u32, steps: u32) -> Result<(), Box<dyn Error>> {
        self.fade_to(RGBWW::off(), duration_secs, steps).await?;
        self.power_off().await?;
        Ok(())
    }

    /// Fades the LED strip from off to a target color over a specified duration.
    ///
    /// # Arguments
    ///
    /// * `target_color` - The final RGBWW color to fade to
    /// * `duration_secs` - The duration of the fade in seconds
    /// * `steps` - The number of steps to use for the fade (higher = smoother)
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error
    pub async fn fade_in(&mut self, target_color: RGBWW, duration_secs: u32, steps: u32) -> Result<(), Box<dyn Error>> {
        self.power_on().await?;
        self.fade_to(target_color, duration_secs, steps).await
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
    db_pool: &SqlitePool,
    led_controller: &Arc<Mutex<LEDController>>,
    config: &Config
) -> Result<(), Box<dyn Error>> {
    // Get current time
    let now = Local::now();
    let current_time = now.format("%H:%M").to_string();
    
    // Get current week number (1-52)
    let week_number = now.iso_week().week() as i32;
    
    // Try to get schedule from database first
    let schedule_result = sqlx::query!(
        "SELECT led_start, led_end, led_r, led_g, led_b, led_cw, led_ww 
         FROM schedule 
         WHERE week_number = $1",
        week_number
    )
    .fetch_optional(db_pool)
    .await?;
    
    // Get led settings from database
    let led_settings = sqlx::query!(
        "SELECT r, g, b, ww, cw, enabled, override, season_weight 
         FROM led_settings 
         WHERE id = 1"
    )
    .fetch_one(db_pool)
    .await?;
    
    // Process the results
    let (r, g, b, ww, cw, enabled, override_settings, season_weight) = (
        led_settings.r as u8,
        led_settings.g as u8,
        led_settings.b as u8,
        led_settings.ww as u8,
        led_settings.cw as u8,
        led_settings.enabled != 0,
        led_settings.override != 0,
        led_settings.season_weight
    );
    
    // Decide whether to use scheduled or manual settings
    let mut controller = led_controller.lock().await;
    
    if override_settings {
        // Use manual settings from led_settings table
        if enabled {
            controller.set_rgbww(r, g, b, ww, cw).await?;
        } else {
            controller.set_off().await?;
        }
    } else {
        // Use schedule-based settings if available
        if let Some(schedule) = schedule_result {
            let (led_start, led_end, led_r, led_g, led_b, led_cw, led_ww) = (
                schedule.led_start,
                schedule.led_end,
                schedule.led_r as u8,
                schedule.led_g as u8,
                schedule.led_b as u8,
                schedule.led_cw as u8,
                schedule.led_ww as u8
            );
            
            if is_time_between(&current_time, &led_start, &led_end) {
                controller.set_rgbww(led_r, led_g, led_b, led_cw, led_ww).await?;
            } else {
                controller.set_off().await?;
            }
        } else {
            // Use default values from config
            controller.set_rgbww(
                config.db.def_led_R as u8,
                config.db.def_led_G as u8,
                config.db.def_led_B as u8,
                config.db.def_led_CW as u8,
                config.db.def_led_WW as u8
            ).await?;
        }
    }
    
    Ok(())
}

/// Checks if the current time is between two specified times.
///
/// # Arguments
///
/// * `time` - The time to check
/// * `start` - The start time in 24-hour format (HH:MM)
/// * `end` - The end time in 24-hour format (HH:MM)
///
/// # Returns
///
/// True if the time is between start and end, False otherwise
fn is_time_between(time: &str, start: &str, end: &str) -> bool {
    time >= start && time <= end
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