use axum::{
    extract::{Json, State, Query, Path},
    routing::{get, post},
    Router,
    response::{IntoResponse, Response},
    http::{StatusCode, header},
    body::Body,
};
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, SqlitePoolOptions};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::modules::config::{WebConfig, Config};
use crate::modules::models::Schedule;
use crate::modules::gpio::{RelayController, RelayType, RGBWW};
use crate::modules::lightControl::LightController;
use crate::modules::ledStrip::LEDController;
use crate::modules::getData::{CurrentReadings, get_current_readings, get_overheat_status};
use crate::modules::logs;
use crate::modules::cam::{CameraService, CameraError};
use chrono::{DateTime, Utc, NaiveDateTime, NaiveDate, NaiveTime};
use std::fs::File;
use std::io::Read;
use std::path::Path;

// ===== Utility Types =====

/// Custom error type for API responses
#[derive(Debug)]
pub enum ApiError {
    InternalError(String),
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
        };
        
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

/// Type alias for API responses
pub type ApiResult<T> = Result<Json<T>, ApiError>;

/// Success response builder
pub fn success<T>(data: T) -> ApiResult<T> {
    Ok(Json(data))
}

/// Database error mapper
pub fn map_db_error<E: std::fmt::Display>(err: E) -> ApiError {
    ApiError::InternalError(format!("Database error: {}", err))
}

// Shared application state
#[derive(Clone)]
pub struct AppState {
    db_pool: Arc<SqlitePool>,
    light_controller: Arc<Mutex<LightController>>,
    relay_controller: Arc<Mutex<RelayController>>,
    led_controller: Arc<Mutex<LEDController>>,
    current_readings: Arc<Mutex<CurrentReadings>>,
    config: Arc<Config>,
    camera_service: Arc<CameraService>,
}

// Helper methods for AppState
impl AppState {
    /// Access the database pool
    pub fn db(&self) -> &SqlitePool {
        &self.db_pool
    }
    
    /// Execute a function with the light controller
    pub async fn with_light_controller<F, R>(&self, f: F) -> R 
    where
        F: FnOnce(&mut LightController) -> R,
    {
        let mut controller = self.light_controller.lock().await;
        f(&mut controller)
    }
    
    /// Execute a function with the relay controller
    pub async fn with_relay_controller<F, R>(&self, f: F) -> R 
    where
        F: FnOnce(&mut RelayController) -> R,
    {
        let mut controller = self.relay_controller.lock().await;
        f(&mut controller)
    }
    
    /// Execute a function with the LED controller
    pub async fn with_led_controller<F, R>(&self, f: F) -> R 
    where
        F: FnOnce(&mut LEDController) -> R,
    {
        let mut controller = self.led_controller.lock().await;
        f(&mut controller)
    }
    
    /// Execute a function with the current readings
    pub async fn with_current_readings<F, R>(&self, f: F) -> R 
    where
        F: FnOnce(&CurrentReadings) -> R,
    {
        let readings = self.current_readings.lock().await;
        f(&readings)
    }
    
    /// Get a reference to the config
    pub fn config(&self) -> &Config {
        &self.config
    }
    
    /// Execute a database query and map the error to an ApiError
    pub async fn query<T, E, F>(&self, query_fn: F) -> Result<T, ApiError>
    where
        F: FnOnce(&SqlitePool) -> futures::future::BoxFuture<'_, Result<T, E>>,
        E: std::fmt::Display,
    {
        query_fn(&self.db_pool)
            .await
            .map_err(map_db_error)
    }
    
    /// Execute a function with the camera service
    pub async fn with_camera<F, R, E>(&self, f: F) -> Result<R, E> 
    where
        F: FnOnce(&CameraService) -> Result<R, E>,
    {
        f(&self.camera_service)
    }
}

// ===== Module Organization =====

mod handlers {
    pub mod schedule;
    pub mod led;
    pub mod monitoring;
    pub mod system;
    pub mod camera;
}

use handlers::schedule::*;
use handlers::led::*;
use handlers::monitoring::*;
use handlers::system::*;
use handlers::camera::*;

/// Main function to create the Axum router with all routes
pub async fn create_router(
    db_pool: &SqlitePool,
    light_controller: Arc<Mutex<LightController>>,
    relay_controller: Arc<Mutex<RelayController>>,
    led_controller: Arc<Mutex<LEDController>>,
    current_readings: Arc<Mutex<CurrentReadings>>,
    config: Arc<Config>,
    camera_service: Arc<CameraService>,
) -> Router {
    let state = AppState {
        db_pool: Arc::new(db_pool.clone()),
        light_controller,
        relay_controller,
        led_controller,
        current_readings,
        config,
        camera_service,
    };

    Router::new()
        .merge(schedule_routes())
        .merge(led_routes())
        .merge(monitoring_routes())
        .merge(system_routes())
        .merge(camera_routes())
        .fallback(handle_not_found)
        .with_state(state)
}

// ===== Fallback Handler =====

/// Handler for routes that don't exist
async fn handle_not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "Not found",
            "message": "The requested resource does not exist"
        }))
    )
}

// ===== Route Definitions =====

/// Schedule management routes
fn schedule_routes() -> Router {
    Router::new()
        .route("/api/schedule", get(get_schedule).post(update_schedule))
}

/// LED control routes
fn led_routes() -> Router {
    Router::new()
        .route("/api/led/power", post(set_led_power))
        .route("/api/led/color", post(set_led_color))
        .route("/api/led/status", get(get_led_status))
        .route("/api/led/natural", post(set_natural_light_settings))
        .route("/api/led/presets", 
            get(get_natural_light_presets)
            .post(set_natural_light_presets))
}

/// Monitoring and data visualization routes
fn monitoring_routes() -> Router {
    Router::new()
        .route("/api/values", get(get_current_values))
        .route("/api/graph/today", get(get_graph_data_today))
        .route("/api/graph/yesterday", get(get_graph_data_yesterday))
        .route("/api/data/download", get(download_sensor_data))
}

/// System management routes
fn system_routes() -> Router {
    Router::new()
        .route("/api/system/status", get(get_system_status))
        .route("/api/logs", get(get_logs))
        .route("/api/logs/download", get(download_logs))
}

/// Camera streaming routes
fn camera_routes() -> Router {
    Router::new()
        .route("/api/camera/status", get(get_camera_status))
        .route("/api/camera/snapshot", get(get_camera_snapshot))
        .route("/api/camera/stream", get(get_camera_stream_url))
}

// ===== Handler Modules =====

// Schedule handlers module
pub mod handlers {
    use super::*;
    
    pub mod schedule {
        use super::*;
        
        /// Handler: Fetch schedule as JSON
        pub async fn get_schedule(State(state): State<AppState>) -> ApiResult<Vec<Schedule>> {
            let db_pool = &state.db_pool;

            sqlx::query_as!(
                Schedule,
                r#"
                SELECT week_number, uv1_start, uv1_end, uv2_start, uv2_end, heat_start, heat_end,
                       led_r AS red, led_g AS green, led_b AS blue, led_cw, led_ww
                FROM schedule
                ORDER BY week_number
                "#
            )
            .fetch_all(db_pool)
            .await
            .map_err(map_db_error)
            .map(Json)
        }

        /// Handler: Update schedule via JSON
        pub async fn update_schedule(
            Json(payload): Json<Vec<Schedule>>,
            State(state): State<AppState>,
        ) -> ApiResult<&'static str> {
            let db_pool = &state.db_pool;

            for setting in payload {
                sqlx::query!(
                    r#"
                    INSERT INTO schedule (week_number, uv1_start, uv1_end, uv2_start, uv2_end, heat_start, heat_end, led_r, led_g, led_b, led_cw, led_ww)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(week_number) DO UPDATE SET
                        uv1_start = excluded.uv1_start,
                        uv1_end = excluded.uv1_end,
                        uv2_start = excluded.uv2_start,
                        uv2_end = excluded.uv2_end,
                        heat_start = excluded.heat_start,
                        heat_end = excluded.heat_end,
                        led_r = excluded.led_r,
                        led_g = excluded.led_g,
                        led_b = excluded.led_b,
                        led_cw = excluded.led_cw,
                        led_ww = excluded.led_ww
                    "#,
                    setting.week,
                    setting.uv1_start,
                    setting.uv1_end,
                    setting.uv2_start,
                    setting.uv2_end,
                    setting.heat_start,
                    setting.heat_end,
                    setting.red,
                    setting.green,
                    setting.blue,
                    setting.led_cw,
                    setting.led_ww,
                )
                .execute(db_pool)
                .await
                .map_err(map_db_error)?;
            }

            success("Schedule updated successfully")
        }
    }

    // LED handlers module
    pub mod led {
        use super::*;
        
        #[derive(Deserialize)]
        pub struct LEDPowerRequest {
            pub power: bool,
        }

        /// Set LED power state
        pub async fn set_led_power(
            State(state): State<AppState>,
            Json(payload): Json<LEDPowerRequest>,
        ) -> ApiResult<&'static str> {
            let result = if payload.power {
                state.with_led_controller(|controller| {
                    controller.power_on()
                }).await
            } else {
                state.with_led_controller(|controller| {
                    controller.power_off()
                }).await
            };
            
            result.map_err(|e| ApiError::InternalError(e.to_string()))?;
            
            success("LED power state updated")
        }

        #[derive(Deserialize)]
        pub struct LEDColorRequest {
            pub r: u8,
            pub g: u8,
            pub b: u8,
            pub ww: u8,
            pub cw: u8,
        }

        /// Set LED color
        pub async fn set_led_color(
            State(state): State<AppState>,
            Json(payload): Json<LEDColorRequest>,
        ) -> ApiResult<&'static str> {
            let mut led_controller = state.led_controller.lock().await;
            
            led_controller.set_rgbww(
                payload.r, 
                payload.g, 
                payload.b, 
                payload.ww, 
                payload.cw
            ).await.map_err(|e| ApiError::InternalError(e.to_string()))?;
            
            // Update the database with the new settings
            let db_pool = &state.db_pool;
            sqlx::query!(
                r#"
                INSERT OR REPLACE INTO led_settings (id, r, g, b, ww, cw, enabled)
                VALUES (1, ?, ?, ?, ?, ?, true)
                "#,
                payload.r as i32,
                payload.g as i32,
                payload.b as i32,
                payload.ww as i32,
                payload.cw as i32,
            )
            .execute(db_pool)
            .await
            .map_err(map_db_error)?;
            
            success("LED color updated")
        }

        #[derive(Deserialize)]
        pub struct NaturalLightRequest {
            pub override_settings: bool,
            pub season_weight: f32,
        }

        /// Set natural light settings
        pub async fn set_natural_light_settings(
            State(state): State<AppState>,
            Json(payload): Json<NaturalLightRequest>,
        ) -> Result<Json<&'static str>, String> {
            let mut led_controller = state.led_controller.lock().await;
            
            led_controller.set_natural_light_mode(
                payload.override_settings,
                payload.season_weight
            ).await.map_err(|e| e.to_string())?;
            
            Ok(Json("Natural light settings updated"))
        }

        #[derive(Serialize)]
        pub struct LEDStatus {
            pub power: bool,
            pub r: u8,
            pub g: u8,
            pub b: u8,
            pub ww: u8,
            pub cw: u8,
            pub use_natural: bool,
            pub season_weight: f32,
        }

        /// Get LED status
        pub async fn get_led_status(
            State(state): State<AppState>,
        ) -> Result<Json<LEDStatus>, String> {
            let led_controller = state.led_controller.lock().await;
            
            let status = LEDStatus {
                power: led_controller.is_on(),
                r: led_controller.get_red(),
                g: led_controller.get_green(),
                b: led_controller.get_blue(),
                ww: led_controller.get_warm_white(),
                cw: led_controller.get_cool_white(),
                use_natural: led_controller.is_natural_mode(),
                season_weight: led_controller.get_season_weight(),
            };
            
            Ok(Json(status))
        }

        #[derive(Deserialize, Serialize)]
        pub struct NaturalLightPresetsRequest {
            pub morning_r: u8,
            pub morning_g: u8,
            pub morning_b: u8,
            pub morning_ww: u8,
            pub morning_cw: u8,
            pub noon_r: u8,
            pub noon_g: u8,
            pub noon_b: u8,
            pub noon_ww: u8,
            pub noon_cw: u8,
            pub evening_r: u8,
            pub evening_g: u8,
            pub evening_b: u8,
            pub evening_ww: u8,
            pub evening_cw: u8
        }

        /// Set natural light presets
        pub async fn set_natural_light_presets(
            State(state): State<AppState>,
            Json(payload): Json<NaturalLightPresetsRequest>,
        ) -> Result<Json<&'static str>, String> {
            let mut led_controller = state.led_controller.lock().await;
            
            led_controller.set_natural_light_presets(
                (payload.morning_r, payload.morning_g, payload.morning_b, payload.morning_ww, payload.morning_cw),
                (payload.noon_r, payload.noon_g, payload.noon_b, payload.noon_ww, payload.noon_cw),
                (payload.evening_r, payload.evening_g, payload.evening_b, payload.evening_ww, payload.evening_cw),
            ).await.map_err(|e| e.to_string())?;
            
            Ok(Json("Natural light presets updated"))
        }

        /// Get natural light presets
        pub async fn get_natural_light_presets(
            State(state): State<AppState>,
        ) -> Result<Json<NaturalLightPresetsRequest>, String> {
            let led_controller = state.led_controller.lock().await;
            
            let (morning, noon, evening) = led_controller.get_natural_light_presets();
            
            let presets = NaturalLightPresetsRequest {
                morning_r: morning.0,
                morning_g: morning.1,
                morning_b: morning.2,
                morning_ww: morning.3,
                morning_cw: morning.4,
                noon_r: noon.0,
                noon_g: noon.1,
                noon_b: noon.2,
                noon_ww: noon.3,
                noon_cw: noon.4,
                evening_r: evening.0,
                evening_g: evening.1,
                evening_b: evening.2,
                evening_ww: evening.3,
                evening_cw: evening.4,
            };
            
            Ok(Json(presets))
        }
    }

    // Monitoring handlers module
    pub mod monitoring {
        use super::*;
        
        #[derive(Serialize)]
        pub struct CurrentValuesResponse {
            pub timestamp: String,
            pub baskingTemp: f32,
            pub controlTemp: f32,
            pub coolZoneTemp: f32,
            pub humidity: f32,
            pub uv1: f32,
            pub uv2: f32,
            pub uv1_on: bool,
            pub uv2_on: bool,
            pub heat_on: bool,
            pub led_on: bool,
            pub overheat: bool,
        }

        /// Get current sensor values
        pub async fn get_current_values(
            State(state): State<AppState>,
        ) -> Json<CurrentValuesResponse> {
            let current_readings = state.current_readings.lock().await;
            let light_controller = state.light_controller.lock().await;
            let led_controller = state.led_controller.lock().await;
            
            let (overheat, _) = get_overheat_status(&state.db_pool).await;
            
            let response = CurrentValuesResponse {
                timestamp: Utc::now().to_rfc3339(),
                baskingTemp: current_readings.basking_temp,
                controlTemp: current_readings.control_temp,
                coolZoneTemp: current_readings.cool_zone_temp,
                humidity: current_readings.humidity,
                uv1: current_readings.uv1_intensity,
                uv2: current_readings.uv2_intensity,
                uv1_on: light_controller.is_uv1_on(),
                uv2_on: light_controller.is_uv2_on(),
                heat_on: light_controller.is_heat_on(),
                led_on: led_controller.is_on(),
                overheat,
            };
            
            Json(response)
        }

        #[derive(Serialize)]
        pub struct GraphDataPoint {
            pub time: String,
            pub temperature: f32,
            pub controlTemp: f32,
            pub coolZoneTemp: f32,
            pub humidity: f32,
        }

        /// Get today's graph data
        pub async fn get_graph_data_today(
            State(state): State<AppState>,
        ) -> Json<Vec<GraphDataPoint>> {
            let today = chrono::Local::now().date_naive();
            Json(get_graph_data_for_date(&state.db_pool, today).await)
        }

        /// Get yesterday's graph data
        pub async fn get_graph_data_yesterday(
            State(state): State<AppState>,
        ) -> Json<Vec<GraphDataPoint>> {
            let yesterday = chrono::Local::now().date_naive() - chrono::Duration::days(1);
            Json(get_graph_data_for_date(&state.db_pool, yesterday).await)
        }

        /// Helper function to get graph data for a specific date
        pub async fn get_graph_data_for_date(pool: &SqlitePool, date: NaiveDate) -> Vec<GraphDataPoint> {
            let start_of_day = date.and_hms_opt(0, 0, 0).unwrap();
            let end_of_day = date.and_hms_opt(23, 59, 59).unwrap();
            
            let result = sqlx::query!(
                r#"
                SELECT timestamp, basking_temp, control_temp, cool_zone_temp, humidity
                FROM readings
                WHERE timestamp BETWEEN ? AND ?
                ORDER BY timestamp
                "#,
                start_of_day.to_string(),
                end_of_day.to_string()
            )
            .fetch_all(pool)
            .await;
            
            match result {
                Ok(rows) => {
                    rows.into_iter().map(|row| {
                        let dt = NaiveDateTime::parse_from_str(&row.timestamp, "%Y-%m-%d %H:%M:%S")
                            .unwrap_or_else(|_| NaiveDateTime::from_timestamp_opt(0, 0).unwrap());
                            
                        GraphDataPoint {
                            time: dt.format("%H:%M").to_string(),
                            temperature: row.basking_temp,
                            controlTemp: row.control_temp,
                            coolZoneTemp: row.cool_zone_temp,
                            humidity: row.humidity,
                        }
                    }).collect()
                },
                Err(_) => Vec::new(),
            }
        }

        #[derive(Deserialize)]
        pub struct SensorDataQueryParams {
            pub start: String,
            pub end: String,
        }

        /// Download sensor data as CSV
        pub async fn download_sensor_data(
            State(state): State<AppState>,
            Query(params): Query<SensorDataQueryParams>,
        ) -> Result<impl IntoResponse, (StatusCode, String)> {
            // ... existing implementation ...
        
            // Placeholder for the actual implementation
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/csv")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"sensor_data_{}.csv\"", params.start)
                )
                .body(Body::from(String::new()))
                .unwrap())
        }
    }

    // System handlers module
    pub mod system {
        use super::*;
        
        #[derive(Serialize)]
        pub struct SystemStatusResponse {
            pub version: String,
            pub uptime_seconds: u64,
            pub overheat_detected: bool,
            pub last_overheat: Option<String>,
            pub cooldown_remaining: Option<u64>,
            pub data_collection_interval: u64,
            pub free_disk_space_mb: u64,
        }

        /// Get system status
        pub async fn get_system_status(
            State(state): State<AppState>,
        ) -> Json<SystemStatusResponse> {
            // ... existing implementation ...
            
            // Placeholder for the actual implementation
            Json(SystemStatusResponse {
                version: env!("CARGO_PKG_VERSION").to_string(),
                uptime_seconds: 0,
                overheat_detected: false,
                last_overheat: None,
                cooldown_remaining: None,
                data_collection_interval: 60,
                free_disk_space_mb: 0,
            })
        }

        #[derive(Deserialize)]
        pub struct LogQueryParams {
            pub filter: Option<String>,
            pub limit: Option<i64>,
        }

        /// Get system logs
        pub async fn get_logs(
            State(state): State<AppState>,
            Query(params): Query<LogQueryParams>,
        ) -> Result<Json<Vec<logs::LogEntry>>, (StatusCode, String)> {
            // ... existing implementation ...
            
            // Placeholder for the actual implementation
            Ok(Json(Vec::new()))
        }

        /// Download logs as file
        pub async fn download_logs(
            State(state): State<AppState>,
        ) -> Result<impl IntoResponse, (StatusCode, String)> {
            // ... existing implementation ...
            
            // Placeholder for the actual implementation
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/plain")
                .header(
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"system_logs.txt\""
                )
                .body(Body::from(String::new()))
                .unwrap())
        }
    }

    // Camera handlers module
    pub mod camera {
        use super::*;
        
        #[derive(Serialize)]
        pub struct CameraStreamResponse {
            pub stream_url: String,
        }
        
        #[derive(Serialize)]
        pub struct CameraStatusResponse {
            pub camera_available: bool,
            pub camera_initialized: bool,
            pub stream_url: Option<String>,
        }
        
        /// Get camera status
        pub async fn get_camera_status(
            State(state): State<AppState>,
        ) -> ApiResult<CameraStatusResponse> {
            // Check camera availability using the static method
            let camera_available = CameraService::is_camera_available();
            
            // Use the helper method to check if camera is initialized
            let camera_initialized = state.with_camera(|camera| {
                camera.is_initialized()
            }).await;
            
            // Build the stream URL only if camera is available and initialized
            let stream_url = if camera_available && camera_initialized {
                Some(format!("http://{}:{}/stream", 
                    state.config().web.address, 
                    state.config().web.camera_port.unwrap_or(3030)))
            } else {
                None
            };
            
            success(CameraStatusResponse {
                camera_available,
                camera_initialized,
                stream_url,
            })
        }
        
        /// Get camera stream URL
        pub async fn get_camera_stream_url(
            State(state): State<AppState>,
        ) -> ApiResult<CameraStreamResponse> {
            // Check if camera is available
            if !CameraService::is_camera_available() {
                return Err(ApiError::NotFound("Camera is not available".to_string()));
            }
            
            // Use the helper method to check if camera is initialized
            let camera_initialized = state.with_camera(|camera| {
                camera.is_initialized()
            }).await;
            
            if !camera_initialized {
                return Err(ApiError::InternalError("Camera is not initialized".to_string()));
            }
            
            // Get the configured camera stream URL from config
            let stream_url = format!("http://{}:{}/stream", 
                state.config().web.address, 
                state.config().web.camera_port.unwrap_or(3030));
                
            success(CameraStreamResponse {
                stream_url,
            })
        }
        
        /// Get a snapshot from the camera
        pub async fn get_camera_snapshot(
            State(state): State<AppState>,
        ) -> Result<impl IntoResponse, ApiError> {
            // Check if camera is available
            if !CameraService::is_camera_available() {
                return Err(ApiError::NotFound("Camera is not available".to_string()));
            }
            
            // Use the helper method to check if camera is initialized
            let camera_initialized = state.with_camera(|camera| {
                camera.is_initialized()
            }).await;
            
            if !camera_initialized {
                return Err(ApiError::InternalError("Camera is not initialized".to_string()));
            }
            
            // Use the helper method to take a snapshot
            let jpeg_data = state.with_camera(|camera| {
                camera.take_snapshot()
            }).await
                .map_err(|e| ApiError::InternalError(format!("Failed to take camera snapshot: {}", e)))?;
            
            // Return the image data with correct MIME type
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/jpeg")
                .body(Body::from(jpeg_data))
                .map_err(|e| ApiError::InternalError(format!("Failed to create response: {}", e)))?)
        }
    }
}
