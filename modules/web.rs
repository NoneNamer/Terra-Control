use axum::{
    extract::{Json, State},
    response::Html,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, SqlitePoolOptions};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    db_pool: Arc<SqlitePool>,
}

// struct for weekly settings
#[derive(Debug, Serialize, Deserialize)]
struct WeeklySettings {
    week: u32,
    uv1_start: String, // hh:mm format
    uv1_end: String,
    uv2_start: String,
    uv2_end: String,
    heat_start: String,
    heat_end: String,
    red: u8,
    green: u8,
    blue: u8,
    led_cw: u8, // Cool white
    led_ww: u8, // Warm white
}

// Create the Axum router with the necessary routes
pub async fn create_router(config: &Config) -> Router {
    let db_pool = SqlitePoolOptions::new()
        .connect("sqlite://schedule.db")
        .await
        .expect("Failed to connect to SQLite");

    let state = AppState {
        db_pool: Arc::new(db_pool),
    };

    Router::new()
        .route("/schedule", get(get_schedule).post(update_schedule))
        .with_state(state)
}

// Handler: Render the schedule form dynamically
async fn get_schedule(State(state): State<AppState>) -> Html<String> {
    let db_pool = &state.db_pool;

    // Fetch all weekly settings from the database
    let settings = sqlx::query!(
        r#"
        SELECT week_number, uv1_start, uv1_end, uv2_start, uv2_end, heat_start, heat_end,
               led_r, led_g, led_b, led_cw, led_ww
        FROM schedule
        ORDER BY week_number
        "#
    )
    .fetch_all(db_pool)
    .await
    .unwrap(); // Use better error handling in production!

    // Build the HTML form dynamically
    let mut form_html = String::new();
    for setting in settings {
        form_html.push_str(&format!(
            r#"
            <div class="week-row">
                <div class="week-settings">
                    <h3>KW {week_number}</h3>
                    <div class="input-group">
                        <label for="uv1Start{week_number}">UV1 Start Time:</label>
                        <input type="time" id="uv1Start{week_number}" name="uv1Start{week_number}" value="{uv1_start}">
                    </div>
                    <div class="input-group">
                        <label for="uv1End{week_number}">UV1 End Time:</label>
                        <input type="time" id="uv1End{week_number}" name="uv1End{week_number}" value="{uv1_end}">
                    </div>
                    <div class="input-group">
                        <label for="uv2Start{week_number}">UV2 Start Time:</label>
                        <input type="time" id="uv2Start{week_number}" name="uv2Start{week_number}" value="{uv2_start}">
                    </div>
                    <div class="input-group">
                        <label for="uv2End{week_number}">UV2 End Time:</label>
                        <input type="time" id="uv2End{week_number}" name="uv2End{week_number}" value="{uv2_end}">
                    </div>
                    <div class="input-group">
                        <label for="heatStart{week_number}">Heat Start Time:</label>
                        <input type="time" id="heatStart{week_number}" name="heatStart{week_number}" value="{heat_start}">
                    </div>
                    <div class="input-group">
                        <label for="heatEnd{week_number}">Heat End Time:</label>
                        <input type="time" id="heatEnd{week_number}" name="heatEnd{week_number}" value="{heat_end}">
                    </div>
                    <div class="input-group">
                        <label for="red{week_number}">Red:</label>
                        <input type="number" id="red{week_number}" name="red{week_number}" min="0" max="255" value="{led_r}">
                    </div>
                    <div class="input-group">
                        <label for="green{week_number}">Green:</label>
                        <input type="number" id="green{week_number}" name="green{week_number}" min="0" max="255" value="{led_g}">
                    </div>
                    <div class="input-group">
                        <label for="blue{week_number}">Blue:</label>
                        <input type="number" id="blue{week_number}" name="blue{week_number}" min="0" max="255" value="{led_b}">
                    </div>
                    <div class="input-group">
                        <label for="cw{week_number}">Cool White:</label>
                        <input type="number" id="cw{week_number}" name="cw{week_number}" min="0" max="255" value="{led_cw}">
                    </div>
                    <div class="input-group">
                        <label for="ww{week_number}">Warm White:</label>
                        <input type="number" id="ww{week_number}" name="ww{week_number}" min="0" max="255" value="{led_ww}">
                    </div>
                </div>
            </div>
            "#,
            week_number = setting.week_number,
            uv1_start = setting.uv1_start,
            uv1_end = setting.uv1_end,
            uv2_start = setting.uv2_start,
            uv2_end = setting.uv2_end,
            heat_start = setting.heat_start,
            heat_end = setting.heat_end,
            led_r = setting.led_r,
            led_g = setting.led_g,
            led_b = setting.led_b,
            led_cw = setting.led_cw,
            led_ww = setting.led_ww
        ));
    }

    // Wrap the generated form in a full HTML structure
    Html(format!(
        r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Weekly Lighting Schedule</title>
            <link rel="stylesheet" href="styles.css">
        </head>
        <body>
            <div class="container">
                <h1>Terrarium Controller</h1>
                <hr>
                <form id="weeklySettingsForm" method="POST" action="/schedule">
                    {}
                    <input type="submit" value="Submit">
                </form>
            </div>
        </body>
        </html>
        "#,
        form_html
    ))
}

// Handler: Process form submission and update the database
async fn update_schedule(
    Json(payload): Json<Vec<WeeklySettings>>,
    State(state): State<AppState>,
) -> Result<(), String> {
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

    Ok(())
}
