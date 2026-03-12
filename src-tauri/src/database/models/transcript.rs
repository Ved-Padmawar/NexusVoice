use chrono::NaiveDateTime;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Transcript {
    pub id: i64,
    pub content: String,
    pub created_at: NaiveDateTime,
}
