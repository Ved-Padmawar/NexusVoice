use chrono::NaiveDateTime;

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct DictionaryEntry {
    pub id: i64,
    pub term: String,
    pub replacement: String,
    pub hits: i64,
    pub created_at: NaiveDateTime,
}
