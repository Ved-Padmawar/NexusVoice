use chrono::NaiveDateTime;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DictionaryEntry {
    pub id: i64,
    pub term: String,
    pub replacement: String,
    pub created_at: NaiveDateTime,
}
