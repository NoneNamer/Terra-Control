mod modules;

use modules::config::Config;
use modules::web;
use modules::gpio::RelayController;
use modules::lightControl;
use modules::ledStrip::{LEDController, update_leds};
use modules::storage;
use modules::getData::{self, CurrentReadings};
use modules::logs;
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task;
use axum::{
    extract::State,
    response::{sse::{Sse, Event}, IntoResponse},
    routing::get,
    Router,
    http::StatusCode,
};
use std::time::Duration;

/// Main entry point
///
/// This function initializes all the necessary components:
/// - Loads the configuration from config.toml
/// - Initializes the database connection
/// - Sets up the relay controller for device control
/// - Initializes the light and LED controllers
/// - Starts background tasks for:
///   - Sensor data collection
///   - Light control based on schedule
///   - LED control based on schedule
///   - Web server for the control interface
///
/// # Errors
///
/// Returns an error if any of the initialization steps fail or if any of the
/// background tasks encounter an unrecoverable error.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load the configuration from the config.toml file
    let config = Arc::new(Config::load()?);
    println!("Configuration loaded successfully: {:?}", config);

    // Initialize database connection
    let db_pool = Arc::new(storage::initialize_db().await?);
    
    // Log system startup
    logs::log(&db_pool, "INFO", "Terrarium Controller system starting up").await?;
    
    // Initialize the relay controller
    let relay_controller = Arc::new(Mutex::new(
        RelayController::new().expect("Failed to initialize relay controller")
    ));
    
    // Create a light controller
    let light_controller = Arc::new(Mutex::new(
        lightControl::LightController::new(config.light_control.clone())
            .expect("Failed to initialize light controller")
    ));
    
    // Create an LED controller that uses the relay controller
    let led_controller = Arc::new(Mutex::new(
        LEDController::new(Arc::clone(&relay_controller))
    ));
    
    // Initialize the LED controller
    {
        let mut led_ctrl = led_controller.lock().await;
        if let Err(e) = led_ctrl.initialize().await {
            eprintln!("Warning: Failed to initialize LED controller: {:?}", e);
            logs::log(&db_pool, "WARNING", &format!("Failed to initialize LED controller: {:?}", e)).await?;
        }
    }
    
    // Create a shared state for current sensor readings
    let current_readings = Arc::new(Mutex::new(CurrentReadings::new()));

    // Initialize and start the sensor data collection task
    getData::start_data_collection(
        Arc::clone(&db_pool),
        Arc::clone(&current_readings),
        Arc::clone(&config),
        Arc::clone(&light_controller)
    ).await;

    // Initialize the light control task
    let light_control_handle = task::spawn({
        let config = Arc::clone(&config);
        let light_controller = Arc::clone(&light_controller);
        let db_pool = Arc::clone(&db_pool);
        
        async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                
                // Update light control based on schedule
                if let Err(e) = lightControl::update_lights(&db_pool, &light_controller, &config).await {
                    eprintln!("Error updating lights: {:?}", e);
                    if let Err(log_err) = logs::log(&db_pool, "ERROR", &format!("Error updating lights: {:?}", e)).await {
                        eprintln!("Failed to log error: {:?}", log_err);
                    }
                }
            }
        }
    });
    
    // Initialize the LED control task
    let led_control_handle = task::spawn({
        let config = Arc::clone(&config);
        let led_controller = Arc::clone(&led_controller);
        let db_pool = Arc::clone(&db_pool);
        
        async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                // Update LED control based on schedule or settings
                if let Err(e) = update_leds(&db_pool, &led_controller, &config).await {
                    eprintln!("Error updating LEDs: {:?}", e);
                    if let Err(log_err) = logs::log(&db_pool, "ERROR", &format!("Error updating LEDs: {:?}", e)).await {
                        eprintln!("Failed to log error: {:?}", log_err);
                    }
                }
            }
        }
    });

    // Log web server startup
    logs::log(&db_pool, "INFO", "Starting web server").await?;

    // Initialize the web server
    let web_handle = task::spawn({
        let db_pool = Arc::clone(&db_pool);
        let light_controller = Arc::clone(&light_controller);
        let relay_controller = Arc::clone(&relay_controller);
        let led_controller = Arc::clone(&led_controller);
        let current_readings = Arc::clone(&current_readings);
        let config = Arc::clone(&config);
        
        async move {
            let router = web::create_router(
                &db_pool, 
                light_controller, 
                relay_controller, 
                led_controller,
                current_readings,
                config
            ).await;
            
            let addr: SocketAddr = format!("{}:{}", config.web.address, config.web.port)
                .parse()
                .expect("Invalid address");
                
            println!("Starting web server at {}", addr);
            axum::Server::bind(&addr)
                .serve(router.into_make_service())
                .await
                .expect("Failed to start server");
        }
    });

    // Wait for all tasks to finish (they shouldn't unless there's an error)
    tokio::try_join!(light_control_handle, led_control_handle, web_handle)?;

    // Log system shutdown
    logs::log(&db_pool, "INFO", "Terrarium Controller shutting down").await?;

    // Perform safe shutdown
    getData::shutdown_safely(&db_pool).await;

    Ok(())
}