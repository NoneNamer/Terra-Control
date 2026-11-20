//! Logging to the `logs` table, to `logs/<date>.log` and to stdout, plus log / data export.

use crate::modules::models::TIMESTAMP_FORMAT;
use crate::modules::storage::ReadingRow;
use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::io::Write;
use std::path::Path;

pub const LOG_DIR: &str = "logs";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Level {
    Info,
    Warning,
    Error,
}

impl Level {
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Info => "INFO",
            Level::Warning => "WARNING",
            Level::Error => "ERROR",
        }
    }

    /// Parses a filter value from the web API ("info", "warning", "error"); anything else = all.
    pub fn parse_filter(s: &str) -> Option<Level> {
        match s.to_ascii_lowercase().as_str() {
            "info" => Some(Level::Info),
            "warning" => Some(Level::Warning),
            "error" => Some(Level::Error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

/// Logs a message to stdout, the log file and the database.
/// Logging never fails the caller; problems are reported on stderr.
pub async fn log(pool: &SqlitePool, level: Level, message: &str) {
    let now = Local::now();
    let timestamp = now.format(TIMESTAMP_FORMAT).to_string();
    println!("[{timestamp}] [{}] {message}", level.as_str());

    if let Err(e) = append_to_file(&now.format("%Y-%m-%d").to_string(), &timestamp, level, message) {
        eprintln!("Failed to write log file: {e:#}");
    }

    let result = sqlx::query("INSERT INTO logs (timestamp, level, message) VALUES (?, ?, ?)")
        .bind(&timestamp)
        .bind(level.as_str())
        .bind(message)
        .execute(pool)
        .await;
    if let Err(e) = result {
        eprintln!("Failed to write log to database: {e}");
    }
}

fn append_to_file(date: &str, timestamp: &str, level: Level, message: &str) -> Result<()> {
    std::fs::create_dir_all(LOG_DIR)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(Path::new(LOG_DIR).join(format!("{date}.log")))?;
    writeln!(file, "[{timestamp}] [{}] {message}", level.as_str())?;
    Ok(())
}

/// Latest log entries, newest first.
pub async fn get_log_entries(pool: &SqlitePool, filter: Option<Level>, limit: i64) -> Result<Vec<LogEntry>> {
    let limit = limit.clamp(1, 1000);
    let entries = match filter {
        Some(level) => {
            sqlx::query_as::<_, LogEntry>(
                "SELECT timestamp, level, message FROM logs WHERE level = ? ORDER BY id DESC LIMIT ?",
            )
            .bind(level.as_str())
            .bind(limit)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query_as::<_, LogEntry>("SELECT timestamp, level, message FROM logs ORDER BY id DESC LIMIT ?")
                .bind(limit)
                .fetch_all(pool)
                .await?
        }
    };
    Ok(entries)
}

/// Quotes a CSV field if needed (RFC 4180).
fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Zip archive (in memory) with all log files plus the database log as CSV.
pub async fn logs_zip(pool: &SqlitePool) -> Result<Vec<u8>> {
    let entries = sqlx::query_as::<_, LogEntry>("SELECT timestamp, level, message FROM logs ORDER BY id")
        .fetch_all(pool)
        .await?;
    tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        use zip::write::SimpleFileOptions;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        if let Ok(dir) = std::fs::read_dir(LOG_DIR) {
            for entry in dir.flatten() {
                let path = entry.path();
                if path.is_file() {
                    zip.start_file(entry.file_name().to_string_lossy(), options)?;
                    zip.write_all(&std::fs::read(&path)?)?;
                }
            }
        }
        zip.start_file("database_logs.csv", options)?;
        writeln!(zip, "timestamp,level,message")?;
        for e in entries {
            writeln!(zip, "{},{},{}", e.timestamp, e.level, csv_field(&e.message))?;
        }
        Ok(zip.finish()?.into_inner())
    })
    .await?
}

/// Sensor readings as CSV.
pub fn readings_csv(rows: &[ReadingRow]) -> String {
    let opt = |v: Option<f64>| v.map(|x| format!("{x:.1}")).unwrap_or_default();
    let mut csv = String::from(
        "timestamp,basking_temp,cool_temp,enclosure_temp,air_temp,humidity,uv1_on,uv2_on,heat_on,overheat\n",
    );
    for r in rows {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{}\n",
            r.timestamp,
            opt(r.basking_temp),
            opt(r.cool_temp),
            opt(r.enclosure_temp),
            opt(r.air_temp),
            opt(r.humidity),
            r.uv1_on as u8,
            r.uv2_on as u8,
            r.heat_on as u8,
            r.overheat as u8
        ));
    }
    csv
}
