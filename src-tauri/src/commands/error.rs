//! Shared API error type returned by every command, plus DB error mapping.

use serde::Serialize;

#[derive(Debug, Serialize, specta::Type)]
pub struct ApiError {
    code: String,
    message: String,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl From<crate::state::DbUnavailable> for ApiError {
    fn from(value: crate::state::DbUnavailable) -> Self {
        Self::new("database_unavailable", value.0)
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(value: sqlx::Error) -> Self {
        Self::new("database_error", map_db_error(&value))
    }
}

fn map_db_error(error: &sqlx::Error) -> String {
    match error {
        sqlx::Error::RowNotFound => "record not found".to_string(),
        sqlx::Error::PoolClosed => "database unavailable".to_string(),
        sqlx::Error::Io(_) => "database io error".to_string(),
        _ => "database error".to_string(),
    }
}
