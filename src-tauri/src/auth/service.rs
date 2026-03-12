use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::database::dto::refresh_token::CreateRefreshToken;
use crate::database::dto::user::CreateUser;
use crate::database::models::user::User;
use crate::database::repositories::token::TokenRepository;
use crate::database::repositories::user::UserRepository;

use super::errors::AuthError;
use super::session::SessionState;
use super::tokens::{
    generate_access_token, generate_refresh_token, hash_token, refresh_token_expires_at,
    validate_access_token, TokenPair, ACCESS_TOKEN_DAYS,
};

fn jwt_secret() -> Vec<u8> {
    std::env::var("NEXUSVOICE_JWT_SECRET")
        .unwrap_or_else(|_| "nexusvoice-dev-secret-change-in-production".to_string())
        .into_bytes()
}

#[derive(Clone)]
pub struct AuthService {
    pool: SqlitePool,
    users: UserRepository,
    tokens: TokenRepository,
    session: SessionState,
}

impl AuthService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            users: UserRepository::new(pool.clone()),
            tokens: TokenRepository::new(pool.clone()),
            session: SessionState::new(),
            pool,
        }
    }

    #[cfg(test)]
    pub fn with_dependencies(
        users: UserRepository,
        tokens: TokenRepository,
        session: SessionState,
    ) -> Self {
        Self {
            pool: users.pool().clone(),
            users,
            tokens,
            session,
        }
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

    pub async fn login_with_tokens(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(User, TokenPair), AuthError> {
        let user = self.login(email, password).await?;
        let pair = self.issue_token_pair(user.id).await?;
        Ok((user, pair))
    }

    pub async fn register_with_tokens(
        &self,
        email: &str,
        password: &str,
    ) -> Result<(User, TokenPair), AuthError> {
        if self.users.get_by_email(email).await?.is_some() {
            return Err(AuthError::EmailTaken);
        }
        let password_hash = hash_password(password)?;
        let secret = jwt_secret();
        let raw_refresh = generate_refresh_token();
        let token_hash = hash_token(&raw_refresh);
        let expires_at = refresh_token_expires_at();

        let mut tx = self.pool.begin().await?;
        let user = insert_user_tx(
            &mut tx,
            CreateUser {
                email: email.to_string(),
                password_hash,
            },
        )
        .await?;
        let access_token = generate_access_token(user.id, &secret)?;
        insert_token_tx(
            &mut tx,
            CreateRefreshToken {
                user_id: user.id,
                token_hash,
                expires_at,
            },
        )
        .await?;
        tx.commit().await?;

        self.session.set_user(user.clone()).await;
        Ok((
            user,
            TokenPair {
                access_token,
                refresh_token: raw_refresh,
                expires_in_seconds: ACCESS_TOKEN_DAYS * 86_400,
            },
        ))
    }

    pub async fn refresh_tokens(&self, raw_refresh_token: &str) -> Result<TokenPair, AuthError> {
        let hash = hash_token(raw_refresh_token);
        let record = self
            .tokens
            .find_valid(&hash)
            .await?
            .ok_or(AuthError::TokenRevoked)?;

        let secret = jwt_secret();
        let raw_refresh = generate_refresh_token();
        let new_hash = hash_token(&raw_refresh);
        let expires_at = refresh_token_expires_at();

        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE refresh_tokens SET revoked = 1 WHERE token_hash = ? AND revoked = 0")
            .bind(&hash)
            .execute(&mut *tx)
            .await?;
        insert_token_tx(
            &mut tx,
            CreateRefreshToken {
                user_id: record.user_id,
                token_hash: new_hash,
                expires_at,
            },
        )
        .await?;
        tx.commit().await?;

        let access_token = generate_access_token(record.user_id, &secret)?;
        Ok(TokenPair {
            access_token,
            refresh_token: raw_refresh,
            expires_in_seconds: ACCESS_TOKEN_DAYS * 86_400,
        })
    }

    pub async fn revoke_token(&self, raw_refresh_token: &str) -> Result<(), AuthError> {
        let hash = hash_token(raw_refresh_token);
        self.tokens.revoke(&hash).await?;
        self.session.clear_user().await;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn revoke_all_tokens(&self, user_id: i64) -> Result<(), AuthError> {
        self.tokens.revoke_all_for_user(user_id).await?;
        self.session.clear_user().await;
        Ok(())
    }

    pub fn validate_token(&self, access_token: &str) -> Result<i64, AuthError> {
        let claims = validate_access_token(access_token, &jwt_secret())?;
        claims
            .sub
            .parse::<i64>()
            .map_err(|_| AuthError::TokenInvalid)
    }

    async fn issue_token_pair(&self, user_id: i64) -> Result<TokenPair, AuthError> {
        let secret = jwt_secret();
        let access_token = generate_access_token(user_id, &secret)?;
        let raw_refresh = generate_refresh_token();
        let hash = hash_token(&raw_refresh);
        self.tokens
            .create(CreateRefreshToken {
                user_id,
                token_hash: hash,
                expires_at: refresh_token_expires_at(),
            })
            .await?;
        Ok(TokenPair {
            access_token,
            refresh_token: raw_refresh,
            expires_in_seconds: ACCESS_TOKEN_DAYS * 86_400,
        })
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

async fn insert_user_tx(
    tx: &mut Transaction<'_, Sqlite>,
    input: CreateUser,
) -> Result<User, AuthError> {
    sqlx::query_as::<_, User>(
        "INSERT INTO users (email, password_hash) VALUES (?, ?)
         RETURNING id, email, password_hash, created_at",
    )
    .bind(input.email)
    .bind(input.password_hash)
    .fetch_one(&mut **tx)
    .await
    .map_err(AuthError::Database)
}

async fn insert_token_tx(
    tx: &mut Transaction<'_, Sqlite>,
    input: CreateRefreshToken,
) -> Result<(), AuthError> {
    sqlx::query("INSERT INTO refresh_tokens (user_id, token_hash, expires_at) VALUES (?, ?, ?)")
        .bind(input.user_id)
        .bind(input.token_hash)
        .bind(input.expires_at)
        .execute(&mut **tx)
        .await
        .map(|_| ())
        .map_err(AuthError::Database)
}

fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut rand_core::OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| AuthError::PasswordHash)?
        .to_string()
        .pipe(Ok)
}

fn verify_password(password: &str, hash: &str) -> Result<(), AuthError> {
    let parsed_hash = PasswordHash::new(hash).map_err(|_| AuthError::InvalidCredentials)?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| AuthError::InvalidCredentials)
}

trait Pipe: Sized {
    fn pipe<F: FnOnce(Self) -> O, O>(self, f: F) -> O {
        f(self)
    }
}
impl<T> Pipe for T {}
