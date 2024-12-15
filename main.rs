mod config;
mod sensors;
mod control;
mod web;
mod logger;

use tokio::task;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() {
    // Initialize configuration
    let config = config::load_config().expect("Failed to load config");

    // Shared state for sensors and control (thread-safe)
    let state = Arc::new(Mutex::new(control::State::new()));

    // Initialize hardware (GPIO setup)
    control::initialize_hardware(&config).expect("Failed to initialize hardware");

    // Start the web server
    let web_handle = {
        let state = state.clone();
        task::spawn(async move {
            web::start_server(state, config.clone()).await;
        })
    };

    // Sensor polling and control logic
    let sensor_handle = {
        let state = state.clone();
        task::spawn(async move {
            sensors::poll_sensors(state).await;
        })
    };

    // Logging
    let logger_handle = {
        let state = state.clone();
        task::spawn(async move {
            logger::start_logging(state).await;
        })
    };

    // Wait for tasks to finish
    let _ = tokio::join!(web_handle, sensor_handle, logger_handle);
}