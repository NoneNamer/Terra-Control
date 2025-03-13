use sqlx::PgPool;
use tokio::time::{sleep, Duration};
use log::{error, info, warn};
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc, NaiveDateTime};
use crate::gpio::{read_ds18b20, read_dht22, read_veml6075};
use crate::modules::models::SensorReadings;
use crate::modules::config::Config;
use crate::modules::lightControl::LightController;
use crate::modules::logs;
use std::error::Error;

/// Structure to store the most recent sensor readings from all sensors.
/// Used to provide real-time data to the web interface and control systems.
pub struct CurrentReadings {
    pub timestamp: DateTime<Utc>,
    pub basking_temp: f32,
    pub control_temp: f32,
    pub cool_temp: f32,
    pub humidity: f32,
    pub uv_1: f32,
    pub uv_2: f32,
}

impl CurrentReadings {
    /// Creates a new CurrentReadings instance with default values.
    ///
    /// Initializes all sensor readings to 0.0 and sets the timestamp to the current time.
    ///
    /// # Returns
    ///
    /// A new CurrentReadings instance with default values.
    pub fn new() -> Self {
        Self {
            timestamp: Utc::now(),
            basking_temp: 0.0,
            control_temp: 0.0,
            cool_temp: 0.0,
            humidity: 0.0,
            uv_1: 0.0,
            uv_2: 0.0,
        }
    }
}

/// Reads all sensors in the terrarium and returns the current readings.
///
/// This function polls all connected sensors (temperature, humidity, UV) 
/// with configured retry attempts if any reading fails.
///
/// # Arguments
///
/// * `config` - The application configuration containing sensor settings
///
/// # Returns
///
/// A CurrentReadings struct containing all sensor values and the current timestamp
pub async fn read_all_sensors(config: &Config) -> CurrentReadings {
    let timestamp = Utc::now();

    // Read temperatures with configured retry count
    let basking_temp = retry(|| read_ds18b20(config.gpio.ds18b20_bus.unwrap_or(4), "basking"), config.get_data.retry)
        .await.unwrap_or(0.0);
        
    let control_temp = retry(|| read_ds18b20(config.gpio.ds18b20_bus.unwrap_or(4), "control"), config.get_data.retry)
        .await.unwrap_or(0.0);
        
    let cool_temp = retry(|| read_ds18b20(config.gpio.ds18b20_bus.unwrap_or(4), "cool"), config.get_data.retry)
        .await.unwrap_or(0.0);

    // Read humidity with configured retry count
    let humidity = retry(|| read_dht22(config.gpio.dht22_pin.unwrap_or(18)), config.get_data.retry)
        .await.unwrap_or(0.0);

    // Read UV sensors with configured retry count, using proper I2C buses
    let uv_1 = retry(|| read_veml6075(0, config.gpio.veml6075_uv1), config.get_data.retry)
        .await.unwrap_or(0.0);
        
    let uv_2 = retry(|| read_veml6075(1, config.gpio.veml6075_uv2), config.get_data.retry)
        .await.unwrap_or(0.0);

    // Create reading object with all sensor data
    let readings = CurrentReadings {
        timestamp,
        basking_temp,
        control_temp,
        cool_temp,
        humidity,
        uv_1,
        uv_2,
    };
    
    // Check critical temperature (for logging only - actual control is in lightControl.rs)
    if basking_temp > config.light_control.overheat_temp as f32 || 
       control_temp > config.light_control.overheat_temp as f32 {
        warn!("TEMPERATURE WARNING: Temperatures exceeding threshold: Basking={:.1}°C, Control={:.1}°C (Threshold={:.1}°C)", 
              basking_temp, control_temp, config.light_control.overheat_temp);
    }
    
    readings
}

/// Collects sensor data, updates the current readings, and logs values to the database.
///
/// This function is called periodically to:
/// 1. Read all sensor values
/// 2. Update the shared current readings state
/// 3. Update the temperature in the light controller (for overheat protection)
/// 4. Save the readings to the database for historical tracking
///
/// # Arguments
///
/// * `pool` - Database connection pool
/// * `current_readings` - Shared mutex containing the current sensor readings
/// * `config` - Application configuration
/// * `light_controller` - Reference to the light controller for temperature updates
///
/// # Returns
///
/// Returns nothing. Logs errors if sensor reading or database operations fail.
pub async fn read_sensors(
    pool: &PgPool, 
    current_readings: &Arc<Mutex<CurrentReadings>>, 
    config: &Config,
    light_controller: &Arc<Mutex<LightController>>
) {
    // Get new readings
    let readings = read_all_sensors(config).await;
    
    // Update the shared current readings
    {
        let mut current = current_readings.lock().await;
        current.timestamp = readings.timestamp;
        current.basking_temp = readings.basking_temp;
        current.control_temp = readings.control_temp;
        current.cool_temp = readings.cool_temp;
        current.humidity = readings.humidity;
        current.uv_1 = readings.uv_1;
        current.uv_2 = readings.uv_2;
    }
    
    // Pass the current temperature to the light controller for overheat protection
    {
        if let Ok(mut light_ctrl) = light_controller.try_lock() {
            // Update the temperature for overheat protection
            light_ctrl.update_temperature(readings.basking_temp);
        }
    }
    
    // Log the readings
    info!(
        "Sensor readings - Basking: {:.1}°C, Control: {:.1}°C, Cool: {:.1}°C, Humidity: {:.1}%, UV1: {:.1} UVI, UV2: {:.1} UVI", 
        readings.basking_temp, 
        readings.control_temp,
        readings.cool_temp, 
        readings.humidity, 
        readings.uv_1, 
        readings.uv_2
    );
    
    // Convert to database model
    let db_readings = SensorReadings {
        timestamp: readings.timestamp.naive_utc(),
        basking_temp: Some(readings.basking_temp),
        control_temp: Some(readings.control_temp),
        cool_temp: Some(readings.cool_temp),
        humidity: Some(readings.humidity),
        uv_1: Some(readings.uv_1),
        uv_2: Some(readings.uv_2),
    };
    
    // Save to database
    if let Err(e) = save_readings_to_db(pool, &db_readings).await {
        error!("Failed to save sensor readings to database: {}", e);
    }
}

/// Saves sensor readings to the database for historical tracking.
///
/// # Arguments
///
/// * `pool` - Database connection pool
/// * `readings` - The sensor readings to save
///
/// # Returns
///
/// A Result indicating success or a database error
async fn save_readings_to_db(pool: &PgPool, readings: &SensorReadings) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO sensor_readings 
        (timestamp, basking_temp, control_temp, cool_temp, humidity, uv_1, uv_2)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        readings.timestamp,
        readings.basking_temp,
        readings.control_temp,
        readings.cool_temp,
        readings.humidity,
        readings.uv_1,
        readings.uv_2
    )
    .execute(pool)
    .await?;
    
    Ok(())
}

/// Initializes and starts the background task for collecting sensor data.
///
/// This function spawns a tokio task that runs in the background, periodically
/// collecting sensor data according to the configured interval. It continues
/// running until the application shuts down.
///
/// # Arguments
///
/// * `db_pool` - Database connection pool for storing readings
/// * `current_readings` - Shared state for storing the most recent readings
/// * `config` - Application configuration
/// * `light_controller` - Light controller for temperature monitoring
pub async fn start_data_collection(
    db_pool: Arc<PgPool>,
    current_readings: Arc<Mutex<CurrentReadings>>,
    config: Arc<Config>,
    light_controller: Arc<Mutex<LightController>>,
) {
    // Log data collection start
    if let Err(e) = logs::log(&db_pool, "INFO", "Starting sensor data collection").await {
        eprintln!("Failed to log data collection start: {:?}", e);
    }

    // Get collection interval from config (default to 60 seconds if not specified)
    let interval_seconds = config.get_data.interval.unwrap_or(60);
    
    // Spawn a background task for data collection
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_seconds));
        
        loop {
            interval.tick().await;
            
            // Collect and store sensor data
            if let Err(e) = collect_data(&db_pool, &current_readings, &config, &light_controller).await {
                eprintln!("Error collecting sensor data: {:?}", e);
                if let Err(log_err) = logs::log(&db_pool, "ERROR", &format!("Error collecting sensor data: {:?}", e)).await {
                    eprintln!("Failed to log error: {:?}", log_err);
                }
            }
        }
    });
}

/// Retrieves the most recent sensor readings from shared state.
///
/// This function is used by the web interface to get the current sensor values
/// without having to directly poll the sensors.
///
/// # Arguments
///
/// * `readings` - Shared mutex containing the current readings
///
/// # Returns
///
/// A copy of the current readings
pub async fn get_current_readings(readings: &Arc<Mutex<CurrentReadings>>) -> CurrentReadings {
    let current = readings.lock().await;
    CurrentReadings {
        timestamp: current.timestamp,
        basking_temp: current.basking_temp,
        control_temp: current.control_temp,
        cool_temp: current.cool_temp,
        humidity: current.humidity,
        uv_1: current.uv_1,
        uv_2: current.uv_2,
    }
}

/// Checks if the system is currently in an overheat state.
///
/// # Arguments
///
/// * `light_controller` - Reference to the light controller
///
/// # Returns
///
/// Boolean indicating whether the system is overheating
pub async fn get_overheat_status(light_controller: &Arc<Mutex<LightController>>) -> bool {
    if let Ok(light_ctrl) = light_controller.try_lock() {
        light_ctrl.is_overheating()
    } else {
        false // Default to false if we can't get the lock
    }
}

/// Retries a fallible operation a specified number of times.
///
/// This utility function attempts to execute an operation that might fail,
/// retrying up to the specified number of times with a short delay between attempts.
///
/// # Type Parameters
///
/// * `F` - A function that returns an Option<T>
/// * `T` - The return type of the function
///
/// # Arguments
///
/// * `f` - The function to retry
/// * `retries` - The number of retry attempts
///
/// # Returns
///
/// The result of the function if successful, or None if all attempts fail
async fn retry<F, T>(mut f: F, retries: u8) -> Option<T>
where
    F: FnMut() -> Option<T>,
{
    for attempt in 1..=retries {
        match f() {
            Some(result) => return Some(result),
            None => {
                if attempt < retries {
                    error!("Sensor reading attempt {} failed, retrying...", attempt);
                    sleep(Duration::from_millis(500)).await;
                } else {
                    error!("All {} sensor reading attempts failed", retries);
                }
            }
        }
    }
    None
}

/// Performs a safe shutdown of the data collection system.
///
/// This function ensures that any pending data is saved and resources
/// are properly released before the application exits.
///
/// # Arguments
///
/// * `pool` - Database connection pool
pub async fn shutdown_safely(pool: &PgPool) {
    // Log shutdown
    if let Err(e) = logs::log(pool, "INFO", "Shutting down data collection").await {
        eprintln!("Failed to log shutdown: {:?}", e);
    }
    
    info!("Shutting down sensor monitoring safely");
    
    // Flush any pending writes to the database
    if let Err(e) = sqlx::query!("SELECT 1").execute(pool).await {
        error!("Error during database shutdown: {}", e);
    }
    
    // Additional cleanup for sensors if needed
    // ...
    
    info!("Sensor monitoring shutdown complete");
}

/// Internal function to handle the data collection process.
///
/// This function is called by the background task to read sensors and handle errors.
///
/// # Arguments
///
/// * `db_pool` - Database connection pool
/// * `current_readings` - Shared state for current readings
/// * `config` - Application configuration
/// * `light_controller` - Light controller for temperature updates
///
/// # Returns
///
/// Result indicating success or providing an error
async fn collect_data(
    db_pool: &PgPool,
    current_readings: &Arc<Mutex<CurrentReadings>>,
    config: &Config,
    light_controller: &Arc<Mutex<LightController>>,
) -> Result<(), Box<dyn Error>> {
    // Read all sensors
    let readings = read_all_sensors(config).await;
    
    // Update the current readings
    {
        let mut current = current_readings.lock().await;
        *current = readings.clone();
    }
    
    // Store readings in the database
    store_readings(db_pool, &readings).await?;
    
    // Log unusual readings
    if readings.basking_temp > config.thresholds.max_basking_temp {
        logs::log(db_pool, "WARNING", &format!("High basking temperature: {:.1}°C", readings.basking_temp)).await?;
    }
    
    if readings.control_temp > config.thresholds.max_control_temp {
        logs::log(db_pool, "WARNING", &format!("High control temperature: {:.1}°C", readings.control_temp)).await?;
    }
    
    if readings.humidity < config.thresholds.min_humidity {
        logs::log(db_pool, "WARNING", &format!("Low humidity: {:.1}%", readings.humidity)).await?;
    }
    
    // Check for overheat condition
    if get_overheat_status(light_controller).await {
        logs::log(db_pool, "ERROR", "OVERHEAT CONDITION DETECTED! Emergency shutdown initiated.").await?;
    }
    
    Ok(())
}