use rusqlite::{params, Connection, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;

pub fn initialize_schedule_db(config_path: &str, db_path: &str) -> Result<()> {
    // Load default values from config.toml
    let config_content = fs::read_to_string(config_path)?;
    let config: ScheduleConfig = toml::from_str(&config_content)?;

    // Check if the database exists
    if !Path::new(db_path).exists() {
        println!("Database not found. Creating a new one...");
        create_schedule_db(db_path, &config)?;
    } else {
        println!("Database found. No action required.");
    }

    Ok(())
}

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