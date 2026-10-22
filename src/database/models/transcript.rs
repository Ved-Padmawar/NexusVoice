use chrono::NaiveDateTime;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Transcript {
    pub id: i64,
    pub content: String,
    pub word_count: i64,
    pub duration_seconds: Option<f64>,
    /// Display name of the app this was dictated into ("VS Code"). `None` when
    /// the foreground app could not be determined, and for pre-1.17 rows.
    pub target_app: Option<String>,
    pub created_at: NaiveDateTime,
}
