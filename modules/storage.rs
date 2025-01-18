use sqlx::SqlitePool;

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
