use axum::{
    extract::{Json, State},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, SqlitePoolOptions};
use std::sync::Arc;
use crate::modules::config::WebConfig;
use crate::modules::models::Schedule
use crate::modules::models::Override

#[derive(Clone)]
struct AppState {
    db_pool: Arc<SqlitePool>,
}

// Create the Axum router with the necessary routes
pub async fn create_router() -> Router {
    let db_pool = SqlitePoolOptions::new()
        .connect("sqlite://schedule.db")
        .await
        .expect("Failed to connect to SQLite");

    let state = AppState {
        db_pool: Arc::new(db_pool),
    };

    Router::new()
        .route("/api/override", get(get_override).post(update_override))
        .route("/api/schedule", get(get_schedule).post(update_schedule))
        .route("/api/values", get(get_current_values))
        .route("/api/graph/today", get(get_graph_data_today))
        .route("/api/graph/yesterday", get(get_graph_data_yesterday))
        .with_state(state)
}

// Handler: Fetch schedule as JSON
async fn get_schedule(State(state): State<AppState>) -> Json<Vec<Schedule>> {
    let db_pool = &state.db_pool;

    let settings = sqlx::query_as!(
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
    .unwrap(); // Use better error handling in production!

    Json(settings)
}

// Handler: Update schedule via JSON
async fn update_schedule(
    Json(payload): Json<Vec<Schedule>>,
    State(state): State<AppState>,
) -> Result<Json<&'static str>, String> {
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
        .map_err(|e| format!("Database error: {}", e))?;
    }

    Ok(Json("Schedule updated successfully"))
}
