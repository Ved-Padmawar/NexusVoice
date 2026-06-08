//! Shared API error type returned by every command, plus DB/auth error mapping.

use serde::Serialize;

use crate::auth::AuthError;

#[derive(Debug, Serialize)]
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

impl From<sqlx::Error> for ApiError {
    fn from(value: sqlx::Error) -> Self {
        Self::new("database_error", map_db_error(&value))
    }
}

impl From<AuthError> for ApiError {
    fn from(value: AuthError) -> Self {
        match value {
            AuthError::EmailTaken => Self::new("email_taken", "email already registered"),
            AuthError::InvalidCredentials => {
                Self::new("invalid_credentials", "invalid credentials")
            }
            AuthError::PasswordHash => Self::new("password_hash_failed", "password hashing failed"),
            AuthError::Database(err) => Self::new("database_error", map_db_error(&err)),
        }
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
