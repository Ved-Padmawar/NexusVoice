pub mod connection;
pub mod dto;
pub mod export;
pub mod repositories;
pub mod rows;

use sqlx::SqlitePool;
use tokio::sync::SetOnce;

use crate::mirror::{self, Committed, Patch, Slice, Store};

/// Writes to History's rows since launch. History is too big to send, so a
/// window reads the rows it shows again at each revision.
#[derive(Clone, PartialEq)]
struct RowWrites(u64);

impl Slice for RowWrites {
    fn patch(&self) -> Patch {
        Patch::History(self.0)
    }
}

/// Writes since launch that change History's totals or daily words: a row
/// added, removed, rewritten or finished. A pin or an audio flag is not one.
#[derive(Clone, PartialEq)]
struct TotalWrites(u64);

impl Slice for TotalWrites {
    fn patch(&self) -> Patch {
        Patch::HistoryTotals(self.0)
    }
}

/// What a write to the transcripts table changed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum HistoryWrite {
    /// Nothing: the statement matched no row.
    None,
    Rows,
    RowsAndTotals,
}

impl HistoryWrite {
    /// `self` when a statement changed `rows_affected` rows, else `None`.
    pub(super) fn if_any(self, rows_affected: u64) -> Self {
        if rows_affected > 0 {
            self
        } else {
            Self::None
        }
    }
}

pub(super) struct History {
    rows: Store<RowWrites>,
    totals: Store<TotalWrites>,
}

impl History {
    fn new() -> Self {
        mirror::atomic(|| Self {
            rows: Store::new(RowWrites(0)),
            totals: Store::new(TotalWrites(0)),
        })
    }

    /// Publish a write after its commit, so a read at its revision sees it.
    pub(super) fn wrote(&self, write: HistoryWrite) -> Committed {
        mirror::atomic(|| match write {
            HistoryWrite::None => self.rows.committed(),
            HistoryWrite::Rows => self.rows.set(|writes| writes.0 += 1),
            HistoryWrite::RowsAndTotals => {
                self.totals.set(|writes| writes.0 += 1);
                self.rows.set(|writes| writes.0 += 1)
            }
        })
    }
}

/// History's stores. Each test thread gets its own, like the mirror's hub.
pub(super) fn history() -> &'static History {
    #[cfg(not(test))]
    {
        static HISTORY: std::sync::LazyLock<History> = std::sync::LazyLock::new(History::new);
        &HISTORY
    }
    #[cfg(test)]
    {
        thread_local! {
            static HISTORY: &'static History = Box::leak(Box::new(History::new()));
        }
        HISTORY.with(|history| *history)
    }
}

/// Database init failed. Commands surface this instead of waiting forever.
pub struct DbUnavailable(pub String);

/// The pool, opened in the background at startup. Callers wait for it.
pub struct Database {
    pool: SetOnce<Result<SqlitePool, String>>,
}

impl Default for Database {
    fn default() -> Self {
        history();
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
