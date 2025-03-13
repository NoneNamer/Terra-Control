use crate::modules::config::LightControlConfig;

use std::thread;
use std::time::{Duration, Instant};
use chrono::Local;
use rppal::gpio::{Gpio, OutputPin};
use rusqlite::{params, Connection, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use log::{info, warn};

/// Structure for the light controller with overheat protection.
///
/// This struct manages the UV lights and heat lamp for the terrarium,
/// including safety features that prevent dangerous overheating conditions.
pub struct LightController {
    uv1: OutputPin,
    uv2: OutputPin,
    heat: OutputPin,
    overheat_temp: u8,
    overheat_time: Duration,
    last_overheat: Option<Instant>,
    current_temp: f32,          // Current temperature from sensor
    is_overheating: AtomicBool, // Atomic flag for thread-safe access
}

//gpio logic with overheat protection
impl LightController {
    /// Creates a new LightController with the specified configuration.
    ///
    /// Initializes GPIO pins for controlling UV lights and heat lamp,
    /// and sets up overheat protection parameters.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the light controller containing
    ///              pin assignments and safety thresholds
    ///
    /// # Returns
    ///
    /// A Result containing either the new LightController or an error
    pub fn new(config: LightControlConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let gpio = Gpio::new()?;
        Ok(LightController {
            uv1: gpio.get(config.uv_relay1)?.into_output(),
            uv2: gpio.get(config.uv_relay2)?.into_output(),
            heat: gpio.get(config.heat_relay)?.into_output(),
            overheat_temp: config.overheat_temp,
            overheat_time: Duration::from_secs(config.overheat_cooldown_seconds as u64),
            last_overheat: None,
            current_temp: 0.0,
            is_overheating: AtomicBool::new(false),
        })
    }

    /// Controls the first UV light.
    ///
    /// # Arguments
    ///
    /// * `state` - True to turn on, False to turn off
    pub fn set_uv1(&mut self, state: bool) {
        if state {
            self.uv1.set_high();
        } else {
            self.uv1.set_low();
        }
    }

    /// Controls the second UV light.
    ///
    /// # Arguments
    ///
    /// * `state` - True to turn on, False to turn off
    pub fn set_uv2(&mut self, state: bool) {
        if state {
            self.uv2.set_high();
        } else {
            self.uv2.set_low();
        }
    }

    /// Safely controls the heat lamp with overheat protection.
    ///
    /// This method will:
    /// 1. Check if the system is in an overheat condition
    /// 2. If overheating, it will block attempts to turn on the heat lamp
    /// 3. Update the overheat state based on current temperature and cooldown
    ///
    /// # Arguments
    ///
    /// * `state` - True to turn on, False to turn off
    pub fn control_heat(&mut self, state: bool) {
        // Check for overheat condition
        if self.current_temp >= self.overheat_temp as f32 {
            // Set overheat flag
            self.is_overheating.store(true, Ordering::SeqCst);
            
            // Turn off heat
            self.set_heat(false);
            
            // Record overheat time
            self.last_overheat = Some(Instant::now());
            
            warn!("OVERHEAT PROTECTION ACTIVATED: Temperature ({:.1}°C) exceeds threshold ({} °C)",
                  self.current_temp, self.overheat_temp);
                  
            return;
        }
        
        // Check if we're in the cooldown period after an overheat
        if let Some(last_overheat) = self.last_overheat {
            if last_overheat.elapsed() >= self.overheat_time {
                // Cooldown period is over
                self.last_overheat = None;
                self.is_overheating.store(false, Ordering::SeqCst);
                self.set_heat(state);
                
                if state {
                    info!("Overheat cooldown period complete. Heat enabled.");
                }
            } else {
                // Still in cooldown period
                self.set_heat(false);
            }
        } else {
            // Normal operation
            self.set_heat(state);
        }
    }
    
    /// Internal function to directly control the heat lamp relay.
    ///
    /// # Arguments
    ///
    /// * `state` - True to turn on, False to turn off
    fn set_heat(&mut self, state: bool) {
        if state {
            self.heat.set_high();
        } else {
            self.heat.set_low();
        }
    }
    
    /// Updates the current temperature reading and checks for overheat conditions.
    ///
    /// This method is called periodically with new temperature readings and
    /// will trigger overheat protection if the temperature exceeds safe limits.
    ///
    /// # Arguments
    ///
    /// * `temp` - The current temperature from the sensor
    pub fn update_temperature(&mut self, temp: f32) {
        self.current_temp = temp;
        
        // If temperature is too high, trigger overheat protection
        if temp >= self.overheat_temp as f32 {
            if !self.is_overheating.load(Ordering::SeqCst) {
                self.control_heat(false); // This will activate overheat protection
            }
        }
    }
    
    /// Checks if the system is currently in an overheat state.
    ///
    /// # Returns
    ///
    /// True if the system is overheating, False otherwise
    pub fn is_overheating(&self) -> bool {
        self.is_overheating.load(Ordering::SeqCst)
    }
    
    /// Gets the current temperature reading.
    ///
    /// # Returns
    ///
    /// The most recent temperature reading in degrees
    pub fn get_temperature(&self) -> f32 {
        self.current_temp
    }
    
    /// Gets the remaining time in the overheat cooldown period.
    ///
    /// # Returns
    ///
    /// Some(seconds) if in cooldown, None if not in cooldown
    pub fn get_overheat_cooldown_remaining(&self) -> Option<u64> {
        self.last_overheat.map(|time| {
            let elapsed = time.elapsed();
            if elapsed >= self.overheat_time {
                0
            } else {
                (self.overheat_time.as_secs() - elapsed.as_secs()).min(self.overheat_time.as_secs())
            }
        })
    }
}

/// Updates the light control system based on schedule and current settings.
///
/// This function is called periodically to:
/// 1. Check the current time against the configured schedule
/// 2. Check the database for manual overrides
/// 3. Update UV lights and heat lamp accordingly
/// 4. Handle safety conditions like overheat protection
///
/// # Arguments
///
/// * `db` - Database connection for retrieving settings
/// * `light_controller` - Reference to the light controller
/// * `config` - Application configuration containing schedules
///
/// # Returns
///
/// A Result indicating success or an error
pub async fn update_lights(
    db: &rusqlite::Connection,
    light_controller: &Arc<tokio::sync::Mutex<LightController>>,
    config: &crate::modules::config::Config
) -> Result<(), Box<dyn std::error::Error>> {
    // Get current time
    let now = Local::now();
    let current_time = now.format("%H:%M").to_string();
    
    // Get current schedule from DB
    let mut stmt = db.prepare("SELECT uv1_start, uv1_end, uv2_start, uv2_end, heat_start, heat_end FROM schedule WHERE ? BETWEEN week_start AND week_end")?;
    let schedule = stmt.query_row(params![now.format("%Y-%m-%d").to_string()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?
        ))
    });
    
    // Update relays based on schedule
    let mut controller = light_controller.lock().await;
    
    // Get schedule times (or use defaults if no schedule found)
    let (uv1_start, uv1_end, uv2_start, uv2_end, heat_start, heat_end) = match schedule {
        Ok(s) => s,
        Err(_) => (
            config.db.def_uv1_start.clone(),
            config.db.def_uv1_end.clone(),
            config.db.def_uv2_start.clone(),
            config.db.def_uv2_end.clone(),
            config.db.def_heat_start.clone(),
            config.db.def_heat_end.clone()
        )
    };
    
    // Check if we're within the scheduled times and update relays
    controller.set_uv1(is_time_between(&current_time, &uv1_start, &uv1_end));
    controller.set_uv2(is_time_between(&current_time, &uv2_start, &uv2_end));
    
    // Heat is controlled with overheat protection
    controller.control_heat(is_time_between(&current_time, &heat_start, &heat_end));
    
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