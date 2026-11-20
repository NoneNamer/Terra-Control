//! SQLite access via sqlx (runtime-checked queries). The schema lives in `migrations/`.

use crate::modules::config::Config;
use crate::modules::models::{
    LedMode, LedPresets, LedSettings, Readings, Rgbww, ScheduleRow, WeekSchedule, TIMESTAMP_FORMAT,
};
use anyhow::{Context, Result};
use chrono::NaiveDate;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{FromRow, SqlitePool};
use std::str::FromStr;

/// Opens (and creates if missing) the database and applies all pending migrations.
pub async fn connect(url: &str) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)
        .with_context(|| format!("invalid database URL '{url}'"))?
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .with_context(|| format!("failed to open database '{url}'"))?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to apply database migrations")?;
    Ok(pool)
}

// ===== Schedule =====

pub async fn get_schedule(pool: &SqlitePool) -> Result<Vec<ScheduleRow>> {
    Ok(sqlx::query_as::<_, ScheduleRow>("SELECT * FROM schedule ORDER BY week")
        .fetch_all(pool)
        .await?)
}

pub async fn get_schedule_week(pool: &SqlitePool, week: i64) -> Result<Option<ScheduleRow>> {
    Ok(sqlx::query_as::<_, ScheduleRow>("SELECT * FROM schedule WHERE week = ?")
        .bind(week)
        .fetch_optional(pool)
        .await?)
}

/// Inserts or replaces the row of `row.week`. The row must be validated by the caller.
pub async fn upsert_schedule_week(pool: &SqlitePool, row: &ScheduleRow) -> Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO schedule
            (week, uv1_on, uv1_off, uv2_on, uv2_off, heat_on, heat_off, led_on, led_off,
             led_r, led_g, led_b, led_ww, led_cw)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(row.week)
    .bind(&row.uv1_on)
    .bind(&row.uv1_off)
    .bind(&row.uv2_on)
    .bind(&row.uv2_off)
    .bind(&row.heat_on)
    .bind(&row.heat_off)
    .bind(&row.led_on)
    .bind(&row.led_off)
    .bind(row.led_r)
    .bind(row.led_g)
    .bind(row.led_b)
    .bind(row.led_ww)
    .bind(row.led_cw)
    .execute(pool)
    .await?;
    Ok(())
}

/// Schedule in force for `week`: the database row if present and valid, else the config default.
pub async fn effective_schedule(pool: &SqlitePool, config: &Config, week: u32) -> WeekSchedule {
    match get_schedule_week(pool, week as i64).await {
        Ok(Some(row)) => match row.to_week_schedule() {
            Ok(schedule) => return schedule,
            Err(e) => eprintln!("Schedule row for week {week} is invalid ({e}), using default"),
        },
        Ok(None) => {}
        Err(e) => eprintln!("Failed to read schedule for week {week} ({e:#}), using default"),
    }
    config.default_schedule()
}

pub async fn delete_schedule_week(pool: &SqlitePool, week: i64) -> Result<bool> {
    let result = sqlx::query("DELETE FROM schedule WHERE week = ?").bind(week).execute(pool).await?;
    Ok(result.rows_affected() > 0)
}

// ===== Readings =====

/// Relay states stored together with a measurement.
#[derive(Debug, Clone, Copy)]
pub struct RelaySnapshot {
    pub uv1_on: bool,
    pub uv2_on: bool,
    pub heat_on: bool,
    pub overheat: bool,
}

/// Rounds to 0.1 in f64 so f32 artefacts (22.899999…) don't end up in the database.
fn round1(v: Option<f32>) -> Option<f64> {
    v.map(|x| (x as f64 * 10.0).round() / 10.0)
}

pub async fn insert_reading(pool: &SqlitePool, r: &Readings, relays: RelaySnapshot) -> Result<()> {
    sqlx::query(
        "INSERT INTO readings
            (timestamp, basking_temp, cool_temp, enclosure_temp, air_temp, humidity,
             uv1_on, uv2_on, heat_on, overheat)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(r.timestamp.format(TIMESTAMP_FORMAT).to_string())
    .bind(round1(r.basking_temp))
    .bind(round1(r.cool_temp))
    .bind(round1(r.enclosure_temp))
    .bind(round1(r.air_temp))
    .bind(round1(r.humidity))
    .bind(relays.uv1_on)
    .bind(relays.uv2_on)
    .bind(relays.heat_on)
    .bind(relays.overheat)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, FromRow)]
pub struct ReadingRow {
    pub timestamp: String,
    pub basking_temp: Option<f64>,
    pub cool_temp: Option<f64>,
    pub enclosure_temp: Option<f64>,
    pub air_temp: Option<f64>,
    pub humidity: Option<f64>,
    pub uv1_on: bool,
    pub uv2_on: bool,
    pub heat_on: bool,
    pub overheat: bool,
}

/// All readings from `start` to `end` (both inclusive, whole days).
pub async fn get_readings(pool: &SqlitePool, start: NaiveDate, end: NaiveDate) -> Result<Vec<ReadingRow>> {
    let from = format!("{} 00:00:00", start.format("%Y-%m-%d"));
    let to = format!("{} 23:59:59", end.format("%Y-%m-%d"));
    Ok(sqlx::query_as::<_, ReadingRow>(
        "SELECT timestamp, basking_temp, cool_temp, enclosure_temp, air_temp, humidity,
                uv1_on, uv2_on, heat_on, overheat
         FROM readings WHERE timestamp BETWEEN ? AND ? ORDER BY timestamp",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?)
}

// ===== LED settings =====

#[derive(FromRow)]
struct LedSettingsRow {
    enabled: bool,
    mode: String,
    r: i64,
    g: i64,
    b: i64,
    ww: i64,
    cw: i64,
    brightness: i64,
    season_weight: f64,
}

#[derive(FromRow)]
struct PresetRow {
    name: String,
    r: i64,
    g: i64,
    b: i64,
    ww: i64,
    cw: i64,
}

fn clamp_u8(v: i64) -> u8 {
    v.clamp(0, 255) as u8
}

/// Loads the LED settings; on first start they are created from `config.toml`.
pub async fn load_led_settings(pool: &SqlitePool, config: &Config) -> Result<LedSettings> {
    let row = sqlx::query_as::<_, LedSettingsRow>(
        "SELECT enabled, mode, r, g, b, ww, cw, brightness, season_weight FROM led_settings WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?;
    match row {
        Some(row) => Ok(LedSettings {
            enabled: row.enabled,
            mode: LedMode::parse(&row.mode).map_err(anyhow::Error::msg)?,
            manual_color: Rgbww {
                r: clamp_u8(row.r),
                g: clamp_u8(row.g),
                b: clamp_u8(row.b),
                ww: clamp_u8(row.ww),
                cw: clamp_u8(row.cw),
            },
            brightness: row.brightness.clamp(0, 100) as u8,
            season_weight: row.season_weight as f32,
        }),
        None => {
            let settings = LedSettings {
                enabled: true,
                mode: config.led.default_mode,
                manual_color: config.schedule_default.led_color,
                brightness: config.led.default_brightness,
                season_weight: config.led.season_weight,
            };
            save_led_settings(pool, &settings).await?;
            Ok(settings)
        }
    }
}

pub async fn save_led_settings(pool: &SqlitePool, s: &LedSettings) -> Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO led_settings (id, enabled, mode, r, g, b, ww, cw, brightness, season_weight)
         VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(s.enabled)
    .bind(s.mode.as_str())
    .bind(s.manual_color.r)
    .bind(s.manual_color.g)
    .bind(s.manual_color.b)
    .bind(s.manual_color.ww)
    .bind(s.manual_color.cw)
    .bind(s.brightness)
    .bind(s.season_weight)
    .execute(pool)
    .await?;
    Ok(())
}

/// Loads the colour presets; missing presets are created from `config.toml`.
pub async fn load_led_presets(pool: &SqlitePool, config: &Config) -> Result<LedPresets> {
    let rows = sqlx::query_as::<_, PresetRow>("SELECT name, r, g, b, ww, cw FROM led_presets")
        .fetch_all(pool)
        .await?;
    let mut presets = config.led.presets();
    let mut found = 0;
    for row in rows {
        let color = Rgbww {
            r: clamp_u8(row.r),
            g: clamp_u8(row.g),
            b: clamp_u8(row.b),
            ww: clamp_u8(row.ww),
            cw: clamp_u8(row.cw),
        };
        match row.name.as_str() {
            "morning" => presets.morning = color,
            "noon" => presets.noon = color,
            "evening" => presets.evening = color,
            _ => continue,
        }
        found += 1;
    }
    if found < 3 {
        save_led_presets(pool, &presets).await?;
    }
    Ok(presets)
}

pub async fn save_led_presets(pool: &SqlitePool, p: &LedPresets) -> Result<()> {
    let mut tx = pool.begin().await?;
    for (name, c) in [("morning", p.morning), ("noon", p.noon), ("evening", p.evening)] {
        sqlx::query("INSERT OR REPLACE INTO led_presets (name, r, g, b, ww, cw) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(name)
            .bind(c.r)
            .bind(c.g)
            .bind(c.b)
            .bind(c.ww)
            .bind(c.cw)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}
