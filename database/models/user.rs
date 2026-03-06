use chrono::NaiveDateTime;
use serde::Serialize;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: i64,
    pub email: String,
    #[serde(skip)]
    pub password_hash: String,
    #[serde(skip)]
    pub created_at: NaiveDateTime,
}
