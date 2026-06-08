//! Local-profile authentication: username/password with an
//! Argon2 hash, and a single persisted "current user" row. No JWT, no refresh
//! tokens, no expiry — this is a local-first single-machine app, so a signed-in
//! profile simply stays signed in until explicit logout.

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sqlx::SqlitePool;

use crate::database::dto::user::CreateUser;
use crate::database::models::user::User;
use crate::database::repositories::session::SessionRepository;
use crate::database::repositories::user::UserRepository;

use super::errors::AuthError;

#[derive(Clone)]
pub struct AuthService {
    users: UserRepository,
    session: SessionRepository,
}

impl AuthService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            users: UserRepository::new(pool.clone()),
            session: SessionRepository::new(pool),
        }
    }

    #[cfg(test)]
    pub fn with_dependencies(users: UserRepository, session: SessionRepository) -> Self {
        Self { users, session }
    }

    /// Create a profile and sign it in (persisting the session).
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
        self.session.set(user.id).await?;
        Ok(user)
    }

    /// Verify credentials and sign in (persisting the session).
    pub async fn login(&self, email: &str, password: &str) -> Result<User, AuthError> {
        let Some(user) = self.users.get_by_email(email).await? else {
            return Err(AuthError::InvalidCredentials);
        };
        verify_password(password, &user.password_hash)?;
        self.session.set(user.id).await?;
        Ok(user)
    }

    /// Clear the persisted session (so a restart does not auto-sign-in).
    pub async fn logout(&self) -> Result<(), AuthError> {
        self.session.clear().await?;
        Ok(())
    }

    /// The persisted signed-in user, if any. Used at startup to restore login.
    pub async fn current_user(&self) -> Result<Option<User>, AuthError> {
        let Some(user_id) = self.session.get().await? else {
            return Ok(None);
        };
        Ok(self.users.get_by_id(user_id).await?)
    }
}

fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut rand_core::OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| AuthError::PasswordHash)?
        .to_string())
}

fn verify_password(password: &str, hash: &str) -> Result<(), AuthError> {
    let parsed_hash = PasswordHash::new(hash).map_err(|_| AuthError::InvalidCredentials)?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| AuthError::InvalidCredentials)
}
