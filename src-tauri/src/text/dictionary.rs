//! The user's dictionary: saved in SQLite, mirrored in memory so the finalize
//! step corrects a transcript without a query.

use std::collections::HashMap;

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::bridge::commands::dto::DictionaryResponse;
use crate::data::dto::dictionary::CreateDictionaryEntry;
use crate::data::repositories::dictionary::DictionaryRepository;
use crate::data::rows::dictionary::DictionaryEntry;
use crate::mirror::{Committed, Patch, Slice, Store};
use crate::text::dictionary_engine::{match_key, MAX_SPAN};
use crate::text::DictionaryCorrectionEngine;

/// Why an entry was not saved. The messages are for the user.
#[derive(Debug)]
pub enum DictionaryError {
    Invalid(String),
    /// Another entry already matches the same spoken words.
    Duplicate(String),
    Database(sqlx::Error),
}

impl From<sqlx::Error> for DictionaryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

/// The trimmed term and replacement, or what is wrong with them.
fn validate(term: &str, replacement: &str) -> Result<(String, String), DictionaryError> {
    let (term, replacement) = (term.trim(), replacement.trim());
    if term.is_empty() {
        return Err(DictionaryError::Invalid(
            "Enter the word or phrase to correct.".to_string(),
        ));
    }
    if replacement.is_empty() {
        return Err(DictionaryError::Invalid(
            "Enter what it should be replaced with.".to_string(),
        ));
    }
    if term.split_whitespace().count() > MAX_SPAN {
        return Err(DictionaryError::Invalid(format!(
            "A term can be at most {MAX_SPAN} words long."
        )));
    }
    Ok((term.to_string(), replacement.to_string()))
}

/// Keyed by term, which the matcher reports its hits by.
#[derive(Clone, Default, PartialEq)]
struct Entries(HashMap<String, DictionaryEntry>);

impl Slice for Entries {
    /// Sorted by term, so rows hold still across edits.
    fn patch(&self, rev: u64) -> Patch {
        let mut value: Vec<DictionaryResponse> = self.0.values().cloned().map(Into::into).collect();
        value.sort_unstable_by(|a, b| a.term.cmp(&b.term));
        Patch::Dictionary { rev, value }
    }
}

/// The entries change in memory only once the database holds the change.
pub struct Dictionary {
    entries: Store<Entries>,
    /// Held across each write and its database round trip, so two saves cannot
    /// both pass the duplicate check, and a hit is never counted twice.
    writing: Mutex<()>,
}

impl Dictionary {
    pub fn new() -> Self {
        Self {
            entries: Store::new(Entries::default()),
            writing: Mutex::new(()),
        }
    }

    pub async fn load(&self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        let _writing = self.writing.lock().await;
        let loaded = DictionaryRepository::new(pool.clone()).list_all().await?;
        let loaded = loaded.into_iter().map(|e| (e.term.clone(), e)).collect();
        self.entries.set(|entries| entries.0 = loaded);
        Ok(())
    }

    /// Add an entry, or with `id` edit that one in place: a rename keeps its
    /// id and use count. The dictionary stays exactly as it was on any error.
    pub async fn save(
        &self,
        pool: &SqlitePool,
        id: Option<i64>,
        term: &str,
        replacement: &str,
    ) -> Result<Committed, DictionaryError> {
        let (term, replacement) = validate(term, replacement)?;
        let _writing = self.writing.lock().await;
        let key = match_key(&term);
        if self.entries.read(|entries| {
            entries
                .0
                .values()
                .any(|e| Some(e.id) != id && match_key(&e.term) == key)
        }) {
            return Err(DictionaryError::Duplicate(format!(
                "“{term}” is already in your dictionary."
            )));
        }

        let repo = DictionaryRepository::new(pool.clone());
        let input = CreateDictionaryEntry { term, replacement };
        let entry = match id {
            Some(id) => repo.update(id, input).await?,
            None => repo.create(input).await?,
        };
        Ok(self.entries.set(|entries| {
            entries.0.retain(|_, e| e.id != entry.id);
            entries.0.insert(entry.term.clone(), entry);
        }))
    }

    pub async fn delete(&self, pool: &SqlitePool, id: i64) -> Result<Committed, sqlx::Error> {
        let _writing = self.writing.lock().await;
        DictionaryRepository::new(pool.clone())
            .delete_by_id(id)
            .await?;
        Ok(self.entries.set(|entries| entries.0.retain(|_, e| e.id != id)))
    }

    /// Apply every correction to `text`, counting a hit for each term used.
    pub async fn correct(&self, pool: &SqlitePool, text: &str) -> String {
        let entries = self
            .entries
            .read(|entries| entries.0.values().cloned().collect());
        let (corrected, matched) = DictionaryCorrectionEngine::new(entries).apply_to_text(text);
        if matched.is_empty() {
            return corrected;
        }

        // Like `save`: an edit reading the row back meanwhile would already
        // hold this hit, and count it again here.
        let _writing = self.writing.lock().await;
        // Counted in memory only once saved, so a restart never takes hits back.
        if let Err(e) = DictionaryRepository::new(pool.clone())
            .increment_hits_batch(&matched)
            .await
        {
            log::warn!("could not count dictionary hits: {e}");
            return corrected;
        }
        self.entries.set(|entries| {
            for term in &matched {
                if let Some(entry) = entries.0.get_mut(term) {
                    entry.hits += 1;
                }
            }
        });
        corrected
    }
}

#[cfg(test)]
#[path = "../../tests/unit/text/dictionary.rs"]
mod tests;
