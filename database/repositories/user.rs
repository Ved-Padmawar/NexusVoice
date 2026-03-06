use sqlx::SqlitePool;

use crate::database::dto::user::CreateUser;
use crate::database::models::user::User;

#[derive(Clone)]
pub struct UserRepository {
    pool: SqlitePool,
}

impl UserRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    #[cfg(test)]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn create(&self, input: CreateUser) -> Result<User, sqlx::Error> {
        sqlx::query_as::<_, User>(
      "INSERT INTO users (email, password_hash) VALUES (?, ?) RETURNING id, email, password_hash, created_at",
    )
    .bind(input.email)
    .bind(input.password_hash)
    .fetch_one(&self.pool)
    .await
    }

    #[allow(dead_code)]
    pub async fn get_by_id(&self, id: i64) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as::<_, User>(
            "SELECT id, email, password_hash, created_at FROM users WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn get_by_email(&self, email: &str) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as::<_, User>(
            "SELECT id, email, password_hash, created_at FROM users WHERE email = ?",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
    }
}
