use crate::modules::models::{Data, Override, Schedule};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::error::Error;

/// Initializes the SQLite database connection and sets up required tables.
///
/// This function:
/// 1. Creates a connection pool to the SQLite database
/// 2. Creates all necessary tables if they don't exist, including:
///    - `schedule`: For weekly lighting/heating, now with LED start/end times.
///    - `sensor_history`: For historical sensor readings, matching the `Data` model.
///    - `led_override`: A single-row table to store the current manual LED color override.
///    - `logs`: For system events.

pub async fn initialize_db() -> Result<SqlitePool, Box<dyn Error>> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite:data.db")
        .await?;

    // Create tables if they don't exist
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS schedule (
            week_number INTEGER PRIMARY KEY,
            uv1_start TEXT NOT NULL,
            uv1_end TEXT NOT NULL,
            uv2_start TEXT NOT NULL,
            uv2_end TEXT NOT NULL,
            heat_start TEXT NOT NULL,
            heat_end TEXT NOT NULL,
            led_start TEXT NOT NULL,
            led_end TEXT NOT NULL,
            led_r INTEGER NOT NULL,
            led_g INTEGER NOT NULL,
            led_b INTEGER NOT NULL,
            led_cw INTEGER NOT NULL,
            led_ww INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Create the sensor history table to match the 'Data' struct
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sensor_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            temp_basking1 REAL,
            temp_basking2 REAL,
            temp_cool REAL,
            humidity REAL,
            time_heat TEXT,
            overheat TEXT
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Create a simple table for the LED override state, matching the 'Override' struct
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS led_override (
            id INTEGER PRIMARY KEY,
            red INTEGER,
            green INTEGER,
            blue INTEGER,
            cool_white INTEGER,
            warm_white INTEGER,
            active BOOLEAN NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Insert a default, inactive override state so the row always exists
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO led_override (id, red, green, blue, cool_white, warm_white, active)
        VALUES (1, 255, 0, 0, 0, 0, 0)
        "#,
    )
    .execute(&pool)
    .await?;

    // Create logs table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            level TEXT NOT NULL,
            message TEXT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

impl Schedule {
    pub async fn get_schedules(pool: &SqlitePool) -> Result<Vec<Schedule>, sqlx::Error> {
        let schedules = sqlx::query_as!(
            Schedule,
            r#"
            SELECT * FROM schedule
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(schedules)
    }
}

impl Override {
    pub async fn get_led_override(pool: &SqlitePool) -> Result<Option<Override>, sqlx::Error> {
        let led_override = sqlx::query_as!(
            Override,
            r#"
            SELECT * FROM led_override WHERE id = 1
            "#
        )
        .fetch_optional(pool)
        .await?;

        Ok(led_override)
    }
}

impl Data {
    pub async fn get_history_for_month(
        pool: &SqlitePool,
        month: String,
    ) -> Result<Vec<History>, sqlx::Error> {
        let history = sqlx::query_as!(
            Data,
            r#"
            SELECT id, timestamp, temp_basking1, temp_basking2, temp_cool, humidity, time_heat, overheat 
            FROM sensor_history
            WHERE strftime('%Y-%m', timestamp) = ?
            ORDER BY timestamp
            "#,
            month
        )
        .fetch_all(pool)
        .await?;

        Ok(history)
    }
}
