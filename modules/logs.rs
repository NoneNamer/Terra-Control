use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc, Local, NaiveDateTime};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use zip::{ZipWriter, write::FileOptions};
use crate::modules::models::LogEntry;

// Function to get log entries from the database
pub async fn get_log_entries(
    db_pool: &SqlitePool,
    filter: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<LogEntry>, Box<dyn Error>> {
    let limit = limit.unwrap_or(50);
    
    let entries = match filter.as_deref() {
        Some("info") => {
            sqlx::query_as!(
                LogEntry,
                r#"
                SELECT 
                    timestamp as "timestamp: DateTime<Utc>",
                    level,
                    message
                FROM logs
                WHERE level = 'INFO'
                ORDER BY timestamp DESC
                LIMIT ?
                "#,
                limit
            )
            .fetch_all(db_pool)
            .await?
        },
        Some("warning") => {
            sqlx::query_as!(
                LogEntry,
                r#"
                SELECT 
                    timestamp as "timestamp: DateTime<Utc>",
                    level,
                    message
                FROM logs
                WHERE level = 'WARNING'
                ORDER BY timestamp DESC
                LIMIT ?
                "#,
                limit
            )
            .fetch_all(db_pool)
            .await?
        },
        Some("error") => {
            sqlx::query_as!(
                LogEntry,
                r#"
                SELECT 
                    timestamp as "timestamp: DateTime<Utc>",
                    level,
                    message
                FROM logs
                WHERE level = 'ERROR'
                ORDER BY timestamp DESC
                LIMIT ?
                "#,
                limit
            )
            .fetch_all(db_pool)
            .await?
        },
        _ => {
            sqlx::query_as!(
                LogEntry,
                r#"
                SELECT 
                    timestamp as "timestamp: DateTime<Utc>",
                    level,
                    message
                FROM logs
                ORDER BY timestamp DESC
                LIMIT ?
                "#,
                limit
            )
            .fetch_all(db_pool)
            .await?
        }
    };
    
    Ok(entries)
}

// Function to create a zip file with all log files
pub async fn create_logs_zip() -> Result<PathBuf, Box<dyn Error>> {
    let logs_dir = Path::new("logs");
    let temp_dir = Path::new("temp");
    
    // Create temp directory if it doesn't exist
    if !temp_dir.exists() {
        fs::create_dir_all(temp_dir)?;
    }
    
    let zip_path = temp_dir.join("terrarium_logs.zip");
    let file = File::create(&zip_path)?;
    
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);
    
    // If logs directory exists, add all files to the zip
    if logs_dir.exists() {
        for entry in fs::read_dir(logs_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                let file_name = path.file_name().unwrap().to_string_lossy();
                zip.start_file(file_name, options)?;
                
                let mut file = File::open(&path)?;
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer)?;
                
                zip.write_all(&buffer)?;
            }
        }
    }
    
    // Add database log entries as a CSV file
    let db_pool = sqlx::SqlitePool::connect("sqlite:data.db").await?;
    let log_entries = get_log_entries(&db_pool, None, None).await?;
    
    zip.start_file("database_logs.csv", options)?;
    zip.write_all(b"Timestamp,Level,Message\n")?;
    
    for entry in log_entries {
        let line = format!(
            "{},{},{}\n",
            entry.timestamp.to_rfc3339(),
            entry.level,
            entry.message.replace(',', ";") // Escape commas in the message
        );
        zip.write_all(line.as_bytes())?;
    }
    
    zip.finish()?;
    
    Ok(zip_path)
}

// Function to get sensor data as CSV
pub async fn get_sensor_data_csv(
    db_pool: &SqlitePool,
    start_date: &str,
    end_date: &str,
) -> Result<String, Box<dyn Error>> {
    let readings = sqlx::query!(
        r#"
        SELECT 
            timestamp,
            temperature,
            humidity,
            uv_index
        FROM history
        WHERE date(timestamp) BETWEEN date(?) AND date(?)
        ORDER BY timestamp
        "#,
        start_date,
        end_date
    )
    .fetch_all(db_pool)
    .await?;
    
    let mut csv = String::from("Timestamp,Temperature,Humidity,UV Index\n");
    
    for reading in readings {
        csv.push_str(&format!(
            "{},{},{},{}\n",
            reading.timestamp,
            reading.temperature.unwrap_or(0.0),
            reading.humidity.unwrap_or(0.0),
            reading.uv_index.unwrap_or(0.0)
        ));
    }
    
    Ok(csv)
}

// Function to log a message to the database
pub async fn log_to_db(
    db_pool: &SqlitePool,
    level: &str,
    message: &str,
) -> Result<(), Box<dyn Error>> {
    let timestamp = Utc::now();
    
    sqlx::query!(
        r#"
        INSERT INTO logs (timestamp, level, message)
        VALUES (?, ?, ?)
        "#,
        timestamp,
        level,
        message
    )
    .execute(db_pool)
    .await?;
    
    Ok(())
}

// Function to log a message to both file and database
pub async fn log(
    db_pool: &SqlitePool,
    level: &str,
    message: &str,
) -> Result<(), Box<dyn Error>> {
    // Log to database
    log_to_db(db_pool, level, message).await?;
    
    // Log to file
    let now = Local::now();
    let date_str = now.format("%Y-%m-%d").to_string();
    let time_str = now.format("%H:%M:%S").to_string();
    
    let logs_dir = Path::new("logs");
    if !logs_dir.exists() {
        fs::create_dir_all(logs_dir)?;
    }
    
    let log_file_path = logs_dir.join(format!("{}.log", date_str));
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path)?;
    
    writeln!(file, "[{}] [{}] {}", time_str, level, message)?;
    
    Ok(())
} 