use sqlx::SqlitePool;
use std::error::Error;
use sqlx::sqlite::SqlitePoolOptions;

// Initialize the database connection and create tables if they don't exist
pub async fn initialize_db() -> Result<SqlitePool, Box<dyn Error>> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite:data.db")
        .await?;

    // Create tables if they don't exist
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS schedule (
            week_number INTEGER PRIMARY KEY,
            uv1_start TEXT NOT NULL,
            uv1_end TEXT NOT NULL,
            uv2_start TEXT NOT NULL,
            uv2_end TEXT NOT NULL,
            heat_start TEXT NOT NULL,
            heat_end TEXT NOT NULL,
            led_r INTEGER NOT NULL,
            led_g INTEGER NOT NULL,
            led_b INTEGER NOT NULL,
            led_cw INTEGER NOT NULL,
            led_ww INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS overrides (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            date TEXT NOT NULL,
            uv1_enabled INTEGER NOT NULL,
            uv2_enabled INTEGER NOT NULL,
            heat_enabled INTEGER NOT NULL,
            led_enabled INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            temperature REAL,
            humidity REAL,
            uv_index REAL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Create LED settings table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS led_settings (
            id INTEGER PRIMARY KEY,
            r INTEGER NOT NULL,
            g INTEGER NOT NULL,
            b INTEGER NOT NULL,
            ww INTEGER NOT NULL,
            cw INTEGER NOT NULL,
            enabled INTEGER NOT NULL,
            override INTEGER NOT NULL DEFAULT 0,
            season_weight REAL NOT NULL DEFAULT 0.3
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Create logs table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            level TEXT NOT NULL,
            message TEXT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Insert default LED settings if not exists
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO led_settings (id, r, g, b, ww, cw, enabled, override, season_weight)
        VALUES (1, 150, 150, 128, 128, 128, 1, 0, 0.3)
        "#,
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

impl Schedule {
    pub async fn get_schedule(pool: &SqlitePool) -> Result<Vec<Schedule>, sqlx::Error> {
        let schedules = sqlx::query_as!(
            Schedule,
            r#"
            SELECT * FROM schedule
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(schedules)
    }
}

impl Override {
    pub async fn get_overrides(pool: &SqlitePool) -> Result<Vec<Override>, sqlx::Error> {
        let overrides = sqlx::query_as!(
            Override,
            r#"
            SELECT * FROM overrides
            "#
        )
        .fetch_all(pool)
        .await?;

        Ok(overrides)
    }
}

impl History {
    pub async fn get_history_for_month(
        pool: &SqlitePool,
        month: String,
    ) -> Result<Vec<History>, sqlx::Error> {
        let history = sqlx::query_as!(
            History,
            r#"
            SELECT * FROM history
            WHERE strftime('%Y-%m', timestamp) = ?
            ORDER BY timestamp
            "#,
            month
        )
        .fetch_all(pool)
        .await?;

        Ok(history)
    }
}
