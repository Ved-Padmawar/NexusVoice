//! The error every API call returns, plus DB error mapping.

use std::fmt;

/// A stable machine-readable `code` for callers that branch on the failure,
/// and a `message` fit to show the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    code: &'static str,
    message: String,
}

impl ApiError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

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
