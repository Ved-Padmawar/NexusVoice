use argon2::{password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString}, Argon2};
use sqlx::SqlitePool;

use crate::database::dto::user::CreateUser;
use crate::database::models::user::User;
use crate::database::repositories::user::UserRepository;

use super::errors::AuthError;
use super::session::SessionState;

#[derive(Clone)]
pub struct AuthService {
  users: UserRepository,
  session: SessionState,
}

impl AuthService {
  pub fn new(pool: SqlitePool) -> Self {
    Self {
      users: UserRepository::new(pool),
      session: SessionState::new(),
    }
  }

  #[cfg(test)]
  pub fn with_dependencies(users: UserRepository, session: SessionState) -> Self {
    Self { users, session }
  }

  pub async fn register(&self, email: &str, password: &str) -> Result<User, AuthError> {
    if self.users.get_by_email(email).await?.is_some() {
      return Err(AuthError::EmailTaken);
    }

    let password_hash = hash_password(password)?;

    let user = self
      .users
      .create(CreateUser {
        email: email.to_string(),
        password_hash,
      })
      .await?;

    self.session.set_user(user.clone()).await;
    Ok(user)
  }

  pub async fn login(&self, email: &str, password: &str) -> Result<User, AuthError> {
    let Some(user) = self.users.get_by_email(email).await? else {
      return Err(AuthError::InvalidCredentials);
    };

    verify_password(password, &user.password_hash)?;

    self.session.set_user(user.clone()).await;
    Ok(user)
  }

  #[cfg(test)]
  pub async fn logout(&self) {
    self.session.clear_user().await;
  }

  #[cfg(test)]
  pub async fn current_user(&self) -> Option<User> {
    self.session.get_user().await
  }
}

fn hash_password(password: &str) -> Result<String, AuthError> {
  let salt = SaltString::generate(&mut rand_core::OsRng);
  let argon2 = Argon2::default();
  let hash = argon2
    .hash_password(password.as_bytes(), &salt)
    .map_err(|_| AuthError::PasswordHash)?
    .to_string();
  Ok(hash)
}

fn verify_password(password: &str, hash: &str) -> Result<(), AuthError> {
  let parsed_hash = PasswordHash::new(hash).map_err(|_| AuthError::InvalidCredentials)?;
  Argon2::default()
    .verify_password(password.as_bytes(), &parsed_hash)
    .map_err(|_| AuthError::InvalidCredentials)
}
