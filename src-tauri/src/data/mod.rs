pub mod connection;
pub mod dto;
pub mod repositories;
pub mod rows;

use std::sync::{Arc, LazyLock};

use sqlx::SqlitePool;
use tokio::sync::SetOnce;

use crate::mirror::{Patch, Slice, Store};

/// Writes to the transcripts table since launch. History is too big to send,
/// so a window reads it again at each revision.
#[derive(Clone, PartialEq)]
pub(super) struct Writes(pub(super) u64);

impl Slice for Writes {
    fn patch(&self, rev: u64) -> Patch {
        Patch::History { rev, value: self.0 }
    }
}

static HISTORY: LazyLock<Arc<Store<Writes>>> = LazyLock::new(|| Arc::new(Store::new(Writes(0))));

/// Database init failed. Commands surface this instead of waiting forever.
pub struct DbUnavailable(pub String);

/// The pool, opened in the background at startup. Callers wait for it.
pub struct Database {
    pool: SetOnce<Result<SqlitePool, String>>,
}

impl Default for Database {
    fn default() -> Self {
        LazyLock::force(&HISTORY);
        Self {
            pool: SetOnce::new(),
        }
    }
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
#[path = "../../tests/unit/data/mod.rs"]
mod tests;
