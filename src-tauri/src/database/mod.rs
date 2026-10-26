pub mod connection;
pub mod dto;
pub mod models;
pub mod repositories;

use sqlx::SqlitePool;
use tokio::sync::SetOnce;

/// Database init failed. Commands surface this instead of waiting forever.
pub struct DbUnavailable(pub String);

/// The pool, opened in the background at startup. Callers wait for it.
#[derive(Default)]
pub struct Database {
    pool: SetOnce<Result<SqlitePool, String>>,
}

impl Database {
    /// Called once, by the startup task.
    pub fn set(&self, result: Result<SqlitePool, String>) {
        let _ = self.pool.set(result);
    }

    pub async fn pool(&self) -> Result<SqlitePool, DbUnavailable> {
        self.pool.wait().await.clone().map_err(DbUnavailable)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/database/mod.rs"]
mod tests;
