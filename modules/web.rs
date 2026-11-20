//! Web API (Axum) and static file serving for the local web interface.

use crate::modules::state::AppState;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json, Router,
};
use axum::http::header;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

// ===== Errors =====

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    Conflict(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, m),
            ApiError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::Internal(format!("{e:#}"))
    }
}

pub type ApiResult<T> = Result<Json<T>, ApiError>;

// ===== Extractors with JSON errors =====
// axum's own extractors answer malformed input with plain text; these wrappers turn every
// rejection into the same `{"error": "..."}` body as all other API errors (HTTP 400).

pub struct ApiJson<T>(pub T);

impl<S, T> axum::extract::FromRequest<S> for ApiJson<T>
where
    Json<T>: axum::extract::FromRequest<S, Rejection = axum::extract::rejection::JsonRejection>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, ApiError> {
        Json::<T>::from_request(req, state)
            .await
            .map(|Json(v)| ApiJson(v))
            .map_err(|e| ApiError::BadRequest(e.body_text()))
    }
}

pub struct ApiPath<T>(pub T);

impl<S, T> axum::extract::FromRequestParts<S> for ApiPath<T>
where
    T: serde::de::DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut axum::http::request::Parts, state: &S) -> Result<Self, ApiError> {
        axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Path(v)| ApiPath(v))
            .map_err(|e| ApiError::BadRequest(e.body_text()))
    }
}

pub struct ApiQuery<T>(pub T);

impl<S, T> axum::extract::FromRequestParts<S> for ApiQuery<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut axum::http::request::Parts, state: &S) -> Result<Self, ApiError> {
        axum::extract::Query::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Query(v)| ApiQuery(v))
            .map_err(|e| ApiError::BadRequest(e.body_text()))
    }
}

async fn api_not_found(uri: axum::http::Uri) -> ApiError {
    ApiError::NotFound(format!("unknown API route {}", uri.path()))
}

/// Builds the router: `/api/...` routes plus the static web interface as fallback.
pub fn router(state: AppState) -> Router {
    let static_files = ServeDir::new(&state.config.web.static_dir).append_index_html_on_directories(true);
    Router::new()
        .merge(handlers::monitoring::routes())
        .merge(handlers::schedule::routes())
        .merge(handlers::led::routes())
        .merge(handlers::system::routes())
        .route("/api/{*rest}", axum::routing::any(api_not_found))
        .fallback_service(static_files)
        // Browsers must revalidate (cheap 304 via Last-Modified), otherwise they keep old
        // HTML/JS/CSS after an update and API values may come from the cache
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static("no-cache"),
        ))
        .with_state(state)
}

mod handlers {
    use super::*;

    pub mod monitoring {
        use super::*;
        use crate::modules::gpio::Relay;
        use crate::modules::lightControl::{HeatBlock, RestrikeStatus};
        use crate::modules::logs;
        use crate::modules::storage;
        use axum::{extract::State, http::header, routing::get};
        use chrono::{Local, NaiveDate};
        use serde::{Deserialize, Serialize};
        use std::time::Instant;

        pub fn routes() -> Router<AppState> {
            Router::new()
                .route("/api/values", get(current_values))
                .route("/api/graph/today", get(graph_today))
                .route("/api/graph/yesterday", get(graph_yesterday))
                .route("/api/graph/date/{date}", get(graph_date))
                .route("/api/data/download", get(download_data))
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct CurrentValues {
            /// Time of the measurement (None before the first one)
            timestamp: Option<String>,
            basking_temp: Option<f32>,
            cool_zone_temp: Option<f32>,
            enclosure_temp: Option<f32>,
            air_temp: Option<f32>,
            humidity: Option<f32>,
            uv1_on: bool,
            uv2_on: bool,
            heat_on: bool,
            led_on: bool,
            overheat: bool,
            heat_block: Option<HeatBlock>,
            /// Remaining UV re-strike pause in seconds
            restrike: RestrikeStatus,
        }

        async fn current_values(State(state): State<AppState>) -> Json<CurrentValues> {
            let readings = state.readings.read().await.clone();
            let (overheat, heat_block, restrike) = {
                let light = state.light.lock().await;
                let now = Instant::now();
                (light.is_overheat(now), light.heat_block(now), light.restrike_status(now))
            };
            let led_on = !state.led.lock().await.output().is_off();
            let hw = state.hw.lock();
            Json(CurrentValues {
                timestamp: readings.as_ref().map(|r| r.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()),
                basking_temp: readings.as_ref().and_then(|r| r.basking_temp),
                cool_zone_temp: readings.as_ref().and_then(|r| r.cool_temp),
                enclosure_temp: readings.as_ref().and_then(|r| r.enclosure_temp),
                air_temp: readings.as_ref().and_then(|r| r.air_temp),
                humidity: readings.as_ref().and_then(|r| r.humidity),
                uv1_on: hw.relay(Relay::Uv1),
                uv2_on: hw.relay(Relay::Uv2),
                heat_on: hw.relay(Relay::Heat),
                led_on,
                overheat,
                heat_block,
                restrike,
            })
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        pub struct GraphPoint {
            time: String,
            /// Basking temperature (name kept for the existing chart code)
            temperature: Option<f64>,
            cool_zone_temp: Option<f64>,
            air_temp: Option<f64>,
            humidity: Option<f64>,
        }

        async fn graph_for(state: &AppState, day: NaiveDate) -> ApiResult<Vec<GraphPoint>> {
            let rows = storage::get_readings(&state.pool, day, day).await?;
            Ok(Json(
                rows.into_iter()
                    .map(|r| GraphPoint {
                        // "YYYY-MM-DD HH:MM:SS" -> "HH:MM"
                        time: r.timestamp.get(11..16).unwrap_or_default().to_string(),
                        temperature: r.basking_temp,
                        cool_zone_temp: r.cool_temp,
                        air_temp: r.air_temp,
                        humidity: r.humidity,
                    })
                    .collect(),
            ))
        }

        async fn graph_today(State(state): State<AppState>) -> ApiResult<Vec<GraphPoint>> {
            graph_for(&state, Local::now().date_naive()).await
        }

        async fn graph_yesterday(State(state): State<AppState>) -> ApiResult<Vec<GraphPoint>> {
            let yesterday = Local::now().date_naive().pred_opt().expect("valid date");
            graph_for(&state, yesterday).await
        }

        fn parse_date(s: &str) -> Result<NaiveDate, ApiError> {
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| ApiError::BadRequest(format!("invalid date '{s}', expected YYYY-MM-DD")))
        }

        /// Readings of one day (YYYY-MM-DD).
        async fn graph_date(State(state): State<AppState>, ApiPath(date): ApiPath<String>) -> ApiResult<Vec<GraphPoint>> {
            graph_for(&state, parse_date(&date)?).await
        }

        #[derive(Deserialize)]
        pub struct DateRange {
            start: String,
            end: String,
        }

        async fn download_data(
            State(state): State<AppState>,
            ApiQuery(range): ApiQuery<DateRange>,
        ) -> Result<Response, ApiError> {
            let (start, end) = (parse_date(&range.start)?, parse_date(&range.end)?);
            if end < start {
                return Err(ApiError::BadRequest("end date is before start date".into()));
            }
            if (end - start).num_days() > 366 {
                return Err(ApiError::BadRequest("at most one year per download".into()));
            }
            let rows = storage::get_readings(&state.pool, start, end).await?;
            let filename = format!("terrarium_data_{start}_to_{end}.csv");
            Ok((
                [
                    (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
                    (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"")),
                ],
                logs::readings_csv(&rows),
            )
                .into_response())
        }
    }

    pub mod schedule {
        use super::*;
        use crate::modules::lightControl;
        use crate::modules::models::ScheduleRow;
        use crate::modules::storage;
        use axum::{extract::State, routing::{get, post}};
        use chrono::{Datelike, Local};
        use serde::{Deserialize, Serialize};

        pub fn routes() -> Router<AppState> {
            Router::new()
                .route("/api/schedule", get(get_all))
                .route("/api/schedule/current", get(get_current))
                .route("/api/schedule/copy", post(copy_weeks))
                .route("/api/schedule/reset", post(reset_weeks))
                .route("/api/schedule/{week}", get(get_week).put(put_week).delete(delete_week))
        }

        fn check_week(week: i64) -> Result<(), ApiError> {
            if (1..=53).contains(&week) {
                Ok(())
            } else {
                Err(ApiError::BadRequest(format!("week must be 1-53, got {week}")))
            }
        }

        fn check_weeks(weeks: &[i64]) -> Result<(), ApiError> {
            if weeks.is_empty() {
                return Err(ApiError::BadRequest("no weeks given".into()));
            }
            weeks.iter().try_for_each(|w| check_week(*w))
        }

        /// Validates a row and returns it in normalised form ("HH:MM", empty -> NULL).
        fn normalise(week: i64, row: &ScheduleRow) -> Result<ScheduleRow, ApiError> {
            let mut row = row.clone();
            row.week = week;
            let schedule = row.to_week_schedule().map_err(ApiError::BadRequest)?;
            Ok(ScheduleRow::from_week_schedule(week, &schedule))
        }

        /// Applies schedule changes to the lamps and LEDs right away.
        async fn apply_now(state: &AppState) -> Result<(), ApiError> {
            lightControl::update_lights(state).await?;
            crate::modules::ledStrip::update_leds(state).await?;
            Ok(())
        }

        #[derive(Serialize)]
        pub struct ScheduleOverview {
            current_week: u32,
            /// Schedule used for weeks without their own row ([schedule_default] in config.toml)
            default: ScheduleRow,
            /// Weeks 1-53, each either from the database or the default
            weeks: Vec<WeekResponse>,
        }

        async fn get_all(State(state): State<AppState>) -> ApiResult<ScheduleOverview> {
            let rows = storage::get_schedule(&state.pool).await?;
            let default = state.config.default_schedule();
            let weeks = (1..=53)
                .map(|week| match rows.iter().find(|r| r.week == week) {
                    Some(row) => WeekResponse { source: "database", schedule: row.clone() },
                    None => WeekResponse { source: "default", schedule: ScheduleRow::from_week_schedule(week, &default) },
                })
                .collect();
            Ok(Json(ScheduleOverview {
                current_week: Local::now().iso_week().week(),
                default: ScheduleRow::from_week_schedule(0, &default),
                weeks,
            }))
        }

        #[derive(Serialize)]
        pub struct WeekResponse {
            /// "database" or "default"
            source: &'static str,
            #[serde(flatten)]
            schedule: ScheduleRow,
        }

        async fn week_response(state: &AppState, week: i64) -> Result<WeekResponse, ApiError> {
            check_week(week)?;
            Ok(match storage::get_schedule_week(&state.pool, week).await? {
                Some(row) => WeekResponse { source: "database", schedule: row },
                None => WeekResponse {
                    source: "default",
                    schedule: ScheduleRow::from_week_schedule(week, &state.config.default_schedule()),
                },
            })
        }

        async fn get_week(State(state): State<AppState>, ApiPath(week): ApiPath<i64>) -> ApiResult<WeekResponse> {
            Ok(Json(week_response(&state, week).await?))
        }

        async fn get_current(State(state): State<AppState>) -> ApiResult<WeekResponse> {
            let week = Local::now().iso_week().week() as i64;
            Ok(Json(week_response(&state, week).await?))
        }

        async fn put_week(
            State(state): State<AppState>,
            ApiPath(week): ApiPath<i64>,
            ApiJson(row): ApiJson<ScheduleRow>,
        ) -> ApiResult<ScheduleRow> {
            check_week(week)?;
            let row = normalise(week, &row)?;
            storage::upsert_schedule_week(&state.pool, &row).await?;
            apply_now(&state).await?;
            Ok(Json(row))
        }

        async fn delete_week(State(state): State<AppState>, ApiPath(week): ApiPath<i64>) -> ApiResult<WeekResponse> {
            check_week(week)?;
            storage::delete_schedule_week(&state.pool, week).await?;
            apply_now(&state).await?;
            Ok(Json(week_response(&state, week).await?))
        }

        #[derive(Deserialize)]
        pub struct CopyRequest {
            weeks: Vec<i64>,
            schedule: ScheduleRow,
        }

        /// Stores the same schedule for several weeks (validated once, all or nothing).
        async fn copy_weeks(State(state): State<AppState>, ApiJson(req): ApiJson<CopyRequest>) -> ApiResult<usize> {
            check_weeks(&req.weeks)?;
            let rows = req.weeks.iter().map(|w| normalise(*w, &req.schedule)).collect::<Result<Vec<_>, _>>()?;
            for row in &rows {
                storage::upsert_schedule_week(&state.pool, row).await?;
            }
            apply_now(&state).await?;
            Ok(Json(rows.len()))
        }

        #[derive(Deserialize)]
        pub struct ResetRequest {
            weeks: Vec<i64>,
        }

        /// Removes the own rows of several weeks so they use the default again.
        async fn reset_weeks(State(state): State<AppState>, ApiJson(req): ApiJson<ResetRequest>) -> ApiResult<usize> {
            check_weeks(&req.weeks)?;
            let mut removed = 0;
            for week in &req.weeks {
                removed += storage::delete_schedule_week(&state.pool, *week).await? as usize;
            }
            apply_now(&state).await?;
            Ok(Json(removed))
        }
    }

    pub mod led {
        use super::*;
        use crate::modules::ledStrip::update_leds;
        use crate::modules::models::{LedMode, LedPresets, LedSettings, Rgbww};
        use crate::modules::storage;
        use axum::{extract::State, routing::{get, post}};
        use serde::{Deserialize, Serialize};

        pub fn routes() -> Router<AppState> {
            Router::new()
                .route("/api/led/status", get(status))
                .route("/api/led/power", post(set_power))
                .route("/api/led/color", post(set_color))
                .route("/api/led/natural", post(set_natural))
                .route("/api/led/brightness", post(set_brightness))
                .route("/api/led/presets", get(get_presets).post(set_presets))
        }

        #[derive(Serialize)]
        pub struct LedStatus {
            /// LEDs enabled by the user (the schedule may still keep them off)
            power: bool,
            /// Manual colour
            r: u8,
            g: u8,
            b: u8,
            ww: u8,
            cw: u8,
            use_natural: bool,
            season_weight: f32,
            brightness: u8,
            /// Colour currently written to the strips
            output: Rgbww,
        }

        async fn status(State(state): State<AppState>) -> Json<LedStatus> {
            let led = state.led.lock().await;
            let s = led.settings();
            Json(LedStatus {
                power: s.enabled,
                r: s.manual_color.r,
                g: s.manual_color.g,
                b: s.manual_color.b,
                ww: s.manual_color.ww,
                cw: s.manual_color.cw,
                use_natural: s.mode == LedMode::Natural,
                season_weight: s.season_weight,
                brightness: s.brightness,
                output: led.output(),
            })
        }

        /// Changes the settings, stores them and applies them immediately.
        async fn change(state: &AppState, f: impl FnOnce(&mut LedSettings)) -> ApiResult<&'static str> {
            let settings = {
                let mut led = state.led.lock().await;
                let mut s = led.settings();
                f(&mut s);
                led.set_settings(s);
                s
            };
            storage::save_led_settings(&state.pool, &settings).await?;
            update_leds(state).await?;
            Ok(Json("ok"))
        }

        #[derive(Deserialize)]
        pub struct PowerRequest {
            power: bool,
        }

        async fn set_power(State(state): State<AppState>, ApiJson(req): ApiJson<PowerRequest>) -> ApiResult<&'static str> {
            change(&state, |s| s.enabled = req.power).await
        }

        async fn set_color(State(state): State<AppState>, ApiJson(color): ApiJson<Rgbww>) -> ApiResult<&'static str> {
            change(&state, |s| s.manual_color = color).await
        }

        #[derive(Deserialize)]
        pub struct NaturalRequest {
            /// true = manual colour, false = natural daily curve
            override_settings: bool,
            season_weight: f32,
        }

        async fn set_natural(State(state): State<AppState>, ApiJson(req): ApiJson<NaturalRequest>) -> ApiResult<&'static str> {
            if !(0.0..=1.0).contains(&req.season_weight) {
                return Err(ApiError::BadRequest("season_weight must be 0.0-1.0".into()));
            }
            change(&state, |s| {
                s.mode = if req.override_settings { LedMode::Manual } else { LedMode::Natural };
                s.season_weight = req.season_weight;
            })
            .await
        }

        #[derive(Deserialize)]
        pub struct BrightnessRequest {
            brightness: u8,
        }

        async fn set_brightness(
            State(state): State<AppState>,
            ApiJson(req): ApiJson<BrightnessRequest>,
        ) -> ApiResult<&'static str> {
            if req.brightness > 100 {
                return Err(ApiError::BadRequest("brightness must be 0-100".into()));
            }
            change(&state, |s| s.brightness = req.brightness).await
        }

        /// Flat preset format used by led.html (morning_r, morning_g, ...).
        #[derive(Serialize, Deserialize)]
        pub struct PresetsDto {
            morning_r: u8, morning_g: u8, morning_b: u8, morning_ww: u8, morning_cw: u8,
            noon_r: u8, noon_g: u8, noon_b: u8, noon_ww: u8, noon_cw: u8,
            evening_r: u8, evening_g: u8, evening_b: u8, evening_ww: u8, evening_cw: u8,
        }

        impl From<LedPresets> for PresetsDto {
            fn from(p: LedPresets) -> Self {
                PresetsDto {
                    morning_r: p.morning.r, morning_g: p.morning.g, morning_b: p.morning.b,
                    morning_ww: p.morning.ww, morning_cw: p.morning.cw,
                    noon_r: p.noon.r, noon_g: p.noon.g, noon_b: p.noon.b, noon_ww: p.noon.ww, noon_cw: p.noon.cw,
                    evening_r: p.evening.r, evening_g: p.evening.g, evening_b: p.evening.b,
                    evening_ww: p.evening.ww, evening_cw: p.evening.cw,
                }
            }
        }

        impl From<PresetsDto> for LedPresets {
            fn from(d: PresetsDto) -> Self {
                LedPresets {
                    morning: Rgbww { r: d.morning_r, g: d.morning_g, b: d.morning_b, ww: d.morning_ww, cw: d.morning_cw },
                    noon: Rgbww { r: d.noon_r, g: d.noon_g, b: d.noon_b, ww: d.noon_ww, cw: d.noon_cw },
                    evening: Rgbww { r: d.evening_r, g: d.evening_g, b: d.evening_b, ww: d.evening_ww, cw: d.evening_cw },
                }
            }
        }

        async fn get_presets(State(state): State<AppState>) -> Json<PresetsDto> {
            Json(state.led.lock().await.presets().into())
        }

        async fn set_presets(State(state): State<AppState>, ApiJson(dto): ApiJson<PresetsDto>) -> ApiResult<&'static str> {
            let presets: LedPresets = dto.into();
            storage::save_led_presets(&state.pool, &presets).await?;
            state.led.lock().await.set_presets(presets);
            update_leds(&state).await?;
            Ok(Json("ok"))
        }
    }

    pub mod system {
        use super::*;
        use crate::modules::lightControl::{self, HeatBlock, LampStates, RestrikeStatus};
        use crate::modules::logs::{self, Level};
        use axum::{
            extract::State,
            http::header,
            routing::{get, post},
        };
        use chrono::{Datelike, Local};
        use serde::{Deserialize, Serialize};
        use std::time::Instant;

        pub fn routes() -> Router<AppState> {
            Router::new()
                .route("/api/system/status", get(status))
                .route("/api/logs", get(get_logs))
                .route("/api/logs/download", get(download_logs))
                .route("/api/relays/{relay}", post(set_relay))
        }

        #[derive(Serialize)]
        pub struct SystemStatus {
            version: &'static str,
            uptime_seconds: u64,
            hardware: &'static str,
            setup_mode: bool,
            calendar_week: u32,
            data_collection_interval: u64,
            desired: LampStates,
            heat_block: Option<HeatBlock>,
            restrike: RestrikeStatus,
        }

        async fn status(State(state): State<AppState>) -> Json<SystemStatus> {
            let (desired, heat_block, restrike) = {
                let light = state.light.lock().await;
                let now = Instant::now();
                (light.desired(), light.heat_block(now), light.restrike_status(now))
            };
            Json(SystemStatus {
                version: env!("CARGO_PKG_VERSION"),
                uptime_seconds: state.started.elapsed().as_secs(),
                hardware: state.hw.lock().backend(),
                setup_mode: state.config.main.setup_mode,
                calendar_week: Local::now().iso_week().week(),
                data_collection_interval: state.config.sensors.interval_secs,
                desired,
                heat_block,
                restrike,
            })
        }

        #[derive(Deserialize)]
        pub struct LogQuery {
            filter: Option<String>,
            limit: Option<i64>,
        }

        async fn get_logs(State(state): State<AppState>, ApiQuery(q): ApiQuery<LogQuery>) -> ApiResult<Vec<logs::LogEntry>> {
            let filter = q.filter.as_deref().and_then(Level::parse_filter);
            Ok(Json(logs::get_log_entries(&state.pool, filter, q.limit.unwrap_or(50)).await?))
        }

        async fn download_logs(State(state): State<AppState>) -> Result<Response, ApiError> {
            let zip = logs::logs_zip(&state.pool).await?;
            let filename = format!("terrarium_logs_{}.zip", Local::now().format("%Y-%m-%d"));
            Ok((
                [
                    (header::CONTENT_TYPE, "application/zip".to_string()),
                    (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"")),
                ],
                zip,
            )
                .into_response())
        }

        #[derive(Deserialize)]
        pub struct RelayRequest {
            on: bool,
        }

        /// Manual lamp switching, only in setup mode. Overheat protection still applies to heat.
        async fn set_relay(
            State(state): State<AppState>,
            ApiPath(relay): ApiPath<String>,
            ApiJson(req): ApiJson<RelayRequest>,
        ) -> ApiResult<LampStates> {
            if !state.config.main.setup_mode {
                return Err(ApiError::Conflict("manual switching is only allowed in setup mode".into()));
            }
            let desired = {
                let mut light = state.light.lock().await;
                let mut d = light.desired();
                match relay.as_str() {
                    "uv1" => d.uv1 = req.on,
                    "uv2" => d.uv2 = req.on,
                    "heat" => d.heat = req.on,
                    other => return Err(ApiError::NotFound(format!("unknown relay '{other}' (uv1, uv2, heat)"))),
                }
                light.set_desired(d);
                d
            };
            logs::log(&state.pool, Level::Info, &format!("Manual request: {relay} {}", if req.on { "on" } else { "off" })).await;
            lightControl::apply_and_log(&state).await?;
            Ok(Json(desired))
        }
    }
}
