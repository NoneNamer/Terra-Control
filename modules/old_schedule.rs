use rusqlite::{params, Connection, Result};
use chrono::{Datelike, Local, NaiveTime};
use crate::modules::config::ScheduleConfig; // Import from config.rs
use std::fs;
use std::path::Path;

/// Schedule Database Handler
pub struct Schedule {
    conn: Connection,
}

impl Schedule {
    /// Initialize or Verify the Schedule Database
    pub fn initialize(config_path: &str, db_path: &str) -> Result<()> {
        // Load default values from config.toml
        let config_content = fs::read_to_string(config_path)?;
        let config: ScheduleConfig = toml::from_str(&config_content)?;

        // Check if the database exists
        if !Path::new(db_path).exists() {
            println!("Database not found. Creating a new one...");
            Self::create_schedule_db(db_path, &config)?;
        } else {
            println!("Database found. No action required.");
        }

        Ok(())
    }

    /// Create a New Schedule Database
    fn create_schedule_db(db_path: &str, config: &ScheduleConfig) -> Result<()> {
        let conn = Connection::open(db_path)?;

        // Create the schedule table
        conn.execute(
            "CREATE TABLE schedule (
                week_number INTEGER NOT NULL PRIMARY KEY,
                uv1_start TIME NOT NULL,
                uv1_end TIME NOT NULL,
                uv2_start TIME NOT NULL,
                uv2_end TIME NOT NULL,
                heat_start TIME NOT NULL,
                heat_end TIME NOT NULL,
                led_r INTEGER NOT NULL,
                led_g INTEGER NOT NULL,
                led_b INTEGER NOT NULL,
                led_cw INTEGER NOT NULL,
                led_ww INTEGER NOT NULL
            )",
            [],
        )?;

        // Insert 52 weeks of default values
        let mut stmt = conn.prepare(
            "INSERT INTO schedule (
                week_number, uv1_start, uv1_end, uv2_start, uv2_end,
                heat_start, heat_end, led_r, led_g, led_b, led_cw, led_ww
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
        )?;

        for week in 1..=52 {
            stmt.execute(params![
                week,
                config.def_uv1_start,
                config.def_uv1_end,
                config.def_uv2_start,
                config.def_uv2_end,
                config.def_heat_start,
                config.def_heat_end,
                config.def_led_R,
                config.def_led_G,
                config.def_led_B,
                config.def_led_CW,
                config.def_led_WW
            ])?;
        }

        println!("Database created and populated with default values.");
        Ok(())
    }

    /// Connect to the Schedule Database
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        Ok(Schedule { conn })
    }

    /// Get Current Schedule for Relays
    pub fn get_current_schedule(&self) -> Result<(bool, bool, bool)> {
        let now = Local::now();
        let mut current_week = now.iso_week().week(); // Current ISO week
        let current_time = now.time(); // Current time

        // Fallback to week 52 if current_week exceeds 52
        if current_week > 52 {
            current_week = 52;
        }

        let mut stmt = self.conn.prepare(
            "SELECT uv1_start, uv1_end, uv2_start, uv2_end, heat_start, heat_end 
             FROM schedule WHERE week_number = ?1"
        )?;
        
        let mut rows = stmt.query(params![current_week])?;
        if let Some(row) = rows.next()? {
            let uv1_start: String = row.get(0)?;
            let uv1_end: String = row.get(1)?;
            let uv2_start: String = row.get(2)?;
            let uv2_end: String = row.get(3)?;
            let heat_start: String = row.get(4)?;
            let heat_end: String = row.get(5)?;
            
            let uv1_active = is_time_in_range(&current_time, &uv1_start, &uv1_end);
            let uv2_active = is_time_in_range(&current_time, &uv2_start, &uv2_end);
            let heat_active = is_time_in_range(&current_time, &heat_start, &heat_end);

            return Ok((uv1_active, uv2_active, heat_active));
        }

        Ok((false, false, false))
    }

    /// Get Current RGB LED Values
    pub fn get_rgb_values(&self) -> Result<(i32, i32, i32, i32, i32)> {
        let now = Local::now();
        let mut current_week = now.iso_week().week();

        if current_week > 52 {
            current_week = 52;
        }

        let mut stmt = self.conn.prepare(
            "SELECT led_r, led_g, led_b, led_cw, led_ww FROM schedule WHERE week_number = ?1"
        )?;

        let mut rows = stmt.query(params![current_week])?;
        if let Some(row) = rows.next()? {
            let led_r: i32 = row.get(0)?;
            let led_g: i32 = row.get(1)?;
            let led_b: i32 = row.get(2)?;
            let led_cw: i32 = row.get(3)?;
            let led_ww: i32 = row.get(4)?;

            return Ok((led_r, led_g, led_b, led_cw, led_ww));
        }

        Ok((0, 0, 0, 0, 0))
    }
}

/// Helper function to check if the current time is in the specified range
fn is_time_in_range(current_time: &NaiveTime, start: &str, end: &str) -> bool {
    let start_time = NaiveTime::parse_from_str(start, "%H:%M:%S").unwrap_or_default();
    let end_time = NaiveTime::parse_from_str(end, "%H:%M:%S").unwrap_or_default();
    *current_time >= start_time && *current_time <= end_time
}
