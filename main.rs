mod modules;

use modules::config::Config;
use modules::web;
use modules::gpio::RelayController;
use modules::lightControl;
use modules::ledStrip::{LEDController, update_leds};
use modules::storage;
use modules::getData::{self, CurrentReadings};
use modules::logs;
use modules::cam::CameraService;
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
use futures::stream::{self, Stream};
use std::convert::Infallible;
use tokio_stream::StreamExt;
use std::time::Duration;

/// Main entry point
///
/// This function initializes all the necessary components:
/// - Loads the configuration from config.toml
/// - Initializes the database connection
/// - Sets up the relay controller for device control
/// - Initializes the light and LED controllers
/// - Sets up the camera service
/// - Starts background tasks for:
///   - Sensor data collection
///   - Light control based on schedule
///   - LED control based on schedule
///   - Camera streaming server
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

    // Initialize the camera service
    let camera_service = Arc::new(CameraService::new());
    if let Err(e) = camera_service.initialize().await {
        eprintln!("Warning: Failed to initialize camera: {:?}", e);
        logs::log(&db_pool, "WARNING", &format!("Failed to initialize camera: {:?}", e)).await?;
    }

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

    // Start the camera stream server (separate from main web server)
    let camera_stream_handle = task::spawn({
        let camera_service_clone = Arc::clone(&camera_service);
        let config_clone = Arc::clone(&config);
        
        async move {
            if let Err(e) = start_camera_stream_server(camera_service_clone, config_clone).await {
                eprintln!("Error running camera stream server: {:?}", e);
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
        let camera_service = Arc::clone(&camera_service);
        
        async move {
            let router = web::create_router(
                &db_pool, 
                light_controller, 
                relay_controller, 
                led_controller,
                current_readings,
                config,
                camera_service
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
    tokio::try_join!(light_control_handle, led_control_handle, camera_stream_handle, web_handle)?;

    // Log system shutdown
    logs::log(&db_pool, "INFO", "Terrarium Controller shutting down").await?;

    // Perform safe shutdown
    getData::shutdown_safely(&db_pool).await;

    Ok(())
}

/// Starts a separate HTTP server dedicated to streaming camera footage.
/// 
/// This function creates an Axum server that provides:
/// - A `/stream` endpoint that sends camera frames as Server-Sent Events (SSE)
/// - Static file serving from the `./static` directory
/// 
/// The server runs on the port specified in the configuration (default: 3030)
/// and accepts connections from any network interface.
/// 
/// # Arguments
/// 
/// * `camera_service` - A reference-counted pointer to the camera service
/// * `config` - A reference-counted pointer to the application configuration
/// 
/// # Errors
/// 
/// Returns an error if the server fails to bind to the specified address
/// or encounters any other error during operation.
async fn start_camera_stream_server(
    camera_service: Arc<CameraService>,
    config: Arc<Config>
) -> Result<(), Box<dyn Error>> {
    // Create app state
    let state = CameraStreamState { camera_service };

    // Create router
    let router = Router::new()
        .route("/stream", get(handle_camera_stream))
        .nest_service("/", tower_http::services::ServeDir::new("./static"))
        .with_state(state);

    // Get port from config, or use default 3030
    let port = config.web.camera_port.unwrap_or(3030);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    
    println!("Starting camera stream server on port {}", port);
    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await?;
        
    Ok(())
}

// State for the camera stream server
#[derive(Clone)]
struct CameraStreamState {
    camera_service: Arc<CameraService>,
}

/// Handles requests to the camera stream endpoint.
/// 
/// This function creates a Server-Sent Events (SSE) stream that sends camera frames
/// as base64-encoded JPEG images at a rate of approximately 30 frames per second.
/// 
/// # Arguments
/// 
/// * `State(state)` - The application state containing the camera service
/// 
/// # Returns
/// 
/// Returns an SSE stream that can be consumed by web clients.
async fn handle_camera_stream(
    State(state): State<CameraStreamState>,
) -> impl IntoResponse {
    // Build a SSE stream of camera frames
    let stream = stream::unfold(state.camera_service.clone(), |camera_service| async move {
        // Create a 30 FPS stream (33ms per frame)
        tokio::time::sleep(Duration::from_millis(33)).await;
        
        match camera_service.take_snapshot().await {
            Ok(jpeg_data) => {
                // Encode the JPEG data as base64
                let base64_data = base64::encode(&jpeg_data);
                // Create an SSE event with the base64 data
                let event = Event::default().data(base64_data);
                Some((event, camera_service))
            },
            Err(e) => {
                eprintln!("Error capturing frame: {:?}", e);
                // Continue the stream even if there's an error
                Some((Event::default().comment("Error capturing frame"), camera_service))
            }
        }
    });

    // Create an SSE response from the stream
    Sse::new(stream)
        .keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(Duration::from_secs(1))
                .text("keep-alive-text")
        )
}