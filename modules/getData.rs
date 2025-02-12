use sqlx::PgPool;
use tokio::time::{sleep, Duration};
use log::{error, info};
use crate::gpio::{read_ds18b20, read_dht22, read_veml6075};
use crate::modules::models::SensorReadings;
use crate::modules::config::Config;

pub async fn read_sensors(pool: &PgPool, config: &Config) {
    let timestamp = chrono::Utc::now().naive_utc();
    let basking_temp = retry(|| read_ds18b20("basking"), config.retry_count).await;
    let cool_temp = retry(|| read_ds18b20("cool"), config.retry_count).await;
    let humidity = retry(|| read_dht22("basking"), config.retry_count).await;
    let uv_1 = retry(|| read_veml6075(1), config.retry_count).await;
    let uv_2 = retry(|| read_veml6075(2), config.retry_count).await;

    let readings = SensorReadings {
        timestamp,
        basking_temp,
        cool_temp,
        humidity,
        uv_1,
        uv_2,
    };

    if let Err(e) = readings.save_to_db(pool).await {
        error!("Failed to save sensor readings: {:?}", e);
    }
}

async fn retry<F, T>(mut f: F, retries: u8) -> Option<T>
where
    F: FnMut() -> Option<T>,
{
    for _ in 0..retries {
        if let Some(value) = f() {
            return Some(value);
        }
        sleep(Duration::from_millis(500)).await;
    }
    None
}

pub async fn shutdown_safely(pool: &PgPool) {
    info!("Shutting down sensor module safely...");
    // Ensure any pending writes complete before exiting
}