//! Persisted login session — a single-row `app_session` table holding the id of
//! the currently signed-in user. This is the whole of `NexusVoice`'s "stay logged
//! in across restarts" mechanism (no tokens). Login sets it, logout clears it,
//! startup reads it.

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct SessionRepository {
    pool: SqlitePool,
}

impl SessionRepository {
    pub const fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Persist the signed-in user id. Overwrites any existing row so only one
    /// profile is "current" at a time.
    pub async fn set(&self, user_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO app_session (id, user_id) VALUES (1, ?)
             ON CONFLICT(id) DO UPDATE SET user_id = excluded.user_id",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map(|_| ())
    }

    /// Clear the persisted session (explicit logout).
    pub async fn clear(&self) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM app_session WHERE id = 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
    }

    /// Read the persisted user id, if any. `None` when no one is signed in.
    pub async fn get(&self) -> Result<Option<i64>, sqlx::Error> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT user_id FROM app_session WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(id,)| id))
    }
}
