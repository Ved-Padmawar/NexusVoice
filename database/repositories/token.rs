use sqlx::SqlitePool;

use crate::database::dto::refresh_token::CreateRefreshToken;
use crate::database::models::refresh_token::RefreshToken;

#[derive(Clone)]
pub struct TokenRepository {
    pool: SqlitePool,
}

impl TokenRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: CreateRefreshToken) -> Result<RefreshToken, sqlx::Error> {
        sqlx::query_as::<_, RefreshToken>(
            "INSERT INTO refresh_tokens (user_id, token_hash, expires_at)
             VALUES (?, ?, ?)
             RETURNING id, user_id, token_hash, expires_at, revoked, created_at",
        )
        .bind(input.user_id)
        .bind(input.token_hash)
        .bind(input.expires_at)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_valid(&self, token_hash: &str) -> Result<Option<RefreshToken>, sqlx::Error> {
        sqlx::query_as::<_, RefreshToken>(
            "SELECT id, user_id, token_hash, expires_at, revoked, created_at
             FROM refresh_tokens
             WHERE token_hash = ? AND revoked = 0 AND expires_at > CURRENT_TIMESTAMP",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn revoke(&self, token_hash: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE refresh_tokens SET revoked = 1 WHERE token_hash = ? AND revoked = 0",
        )
        .bind(token_hash)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    #[allow(dead_code)]
    pub async fn revoke_all_for_user(&self, user_id: i64) -> Result<u64, sqlx::Error> {
        let result =
            sqlx::query("UPDATE refresh_tokens SET revoked = 1 WHERE user_id = ? AND revoked = 0")
                .bind(user_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }
}
