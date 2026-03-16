use chrono::NaiveDateTime;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Transcript {
    pub id: i64,
    pub content: String,
    pub word_count: i64,
    pub duration_seconds: Option<f64>,
    pub created_at: NaiveDateTime,
}
