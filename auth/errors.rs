use std::fmt;

#[derive(Debug)]
pub enum AuthError {
  EmailTaken,
  InvalidCredentials,
  PasswordHash,
  Database(sqlx::Error),
}

impl fmt::Display for AuthError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::EmailTaken => write!(f, "email already registered"),
      Self::InvalidCredentials => write!(f, "invalid credentials"),
      Self::PasswordHash => write!(f, "password hashing failed"),
      Self::Database(_) => write!(f, "database error"),
    }
  }
}

impl std::error::Error for AuthError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Self::Database(err) => Some(err),
      _ => None,
    }
  }
}

impl From<sqlx::Error> for AuthError {
  fn from(value: sqlx::Error) -> Self {
    Self::Database(value)
  }
}
