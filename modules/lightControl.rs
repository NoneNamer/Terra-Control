use crate::modules::config::LightControlConfig;

use std::thread;
use std::time::{Duration, Instant};
use chrono::Local;
use rppal::gpio::{Gpio, OutputPin};
use rusqlite::{params, Connection, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use log::{info, warn};

// Structure for the light controller with overheat protection
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
    pub fn new(config: LightControlConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let gpio = Gpio::new()?;
        Ok(LightController {
            uv1: gpio.get(config.uv_relay1)?.into_output(),
            uv2: gpio.get(config.uv_relay2)?.into_output(),
            heat: gpio.get(config.heat_relay)?.into_output(),
            overheat_temp: config.overheat_temp,
            overheat_time: Duration::from_secs(config.overheat_time),
            last_overheat: None,
            current_temp: 0.0,
            is_overheating: AtomicBool::new(false),
        })
    }

    pub fn set_uv1(&mut self, state: bool) {
        if state {
            self.uv1.set_high();
        } else {
            self.uv1.set_low();
        }
    }

    pub fn set_uv2(&mut self, state: bool) {
        if state {
            self.uv2.set_high();
        } else {
            self.uv2.set_low();
        }
    }

    // Control heat with overheat protection
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
    
    // Set heat directly (internal use only)
    fn set_heat(&mut self, state: bool) {
        if state {
            self.heat.set_high();
        } else {
            self.heat.set_low();
        }
    }
    
    // Update the current temperature (called from getData)
    pub fn update_temperature(&mut self, temp: f32) {
        self.current_temp = temp;
        
        // If temperature is too high, trigger overheat protection
        if temp >= self.overheat_temp as f32 {
            if !self.is_overheating.load(Ordering::SeqCst) {
                self.control_heat(false); // This will activate overheat protection
            }
        }
    }
    
    // Check if the system is currently in overheat state
    pub fn is_overheating(&self) -> bool {
        self.is_overheating.load(Ordering::SeqCst)
    }
    
    // Get current temperature
    pub fn get_temperature(&self) -> f32 {
        self.current_temp
    }
    
    // Get overheat time remaining (in seconds), if any
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

// Function to update lights based on schedule and sensor data
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

// Helper function to check if a time is between two other times
fn is_time_between(time: &str, start: &str, end: &str) -> bool {
    time >= start && time <= end
}