//! The user's dictionary: saved in SQLite, mirrored in memory so the finalize
//! step corrects a transcript without a query.

use std::collections::HashMap;

use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::data::dto::dictionary::CreateDictionaryEntry;
use crate::data::repositories::dictionary::DictionaryRepository;
use crate::data::rows::dictionary::DictionaryEntry;
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

#[derive(Default)]
pub struct Dictionary {
    /// Keyed by term, which the matcher reports its hits by.
    entries: RwLock<HashMap<String, DictionaryEntry>>,
}

impl Dictionary {
    pub async fn load(&self, pool: &SqlitePool) -> Result<(), sqlx::Error> {
        let entries = DictionaryRepository::new(pool.clone()).list_all().await?;
        *self.entries.write().await = entries.into_iter().map(|e| (e.term.clone(), e)).collect();
        Ok(())
    }

    /// Sorted by term, so rows hold still across edits.
    pub async fn entries(&self) -> Vec<DictionaryEntry> {
        let mut entries: Vec<_> = self.entries.read().await.values().cloned().collect();
        entries.sort_unstable_by(|a, b| a.term.cmp(&b.term));
        entries
    }

    /// Add an entry, or with `id` edit that one in place: a rename keeps its
    /// id and use count. The dictionary stays exactly as it was on any error.
    pub async fn save(
        &self,
        pool: &SqlitePool,
        id: Option<i64>,
        term: &str,
        replacement: &str,
    ) -> Result<DictionaryEntry, DictionaryError> {
        let (term, replacement) = validate(term, replacement)?;
        // Held across the write, so two saves cannot both pass the duplicate check.
        let mut entries = self.entries.write().await;
        let key = match_key(&term);
        if entries
            .values()
            .any(|e| Some(e.id) != id && match_key(&e.term) == key)
        {
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
        entries.retain(|_, e| e.id != entry.id);
        entries.insert(entry.term.clone(), entry.clone());
        Ok(entry)
    }

    pub async fn delete(&self, pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
        if DictionaryRepository::new(pool.clone())
            .delete_by_id(id)
            .await?
        {
            self.entries.write().await.retain(|_, e| e.id != id);
        }
        Ok(())
    }

    /// Apply every correction to `text`, counting a hit for each term used.
    pub async fn correct(&self, pool: &SqlitePool, text: &str) -> String {
        let entries: Vec<_> = self.entries.read().await.values().cloned().collect();
        let (corrected, matched) = DictionaryCorrectionEngine::new(entries).apply_to_text(text);
        if matched.is_empty() {
            return corrected;
        }

        if let Err(e) = DictionaryRepository::new(pool.clone())
            .increment_hits_batch(&matched)
            .await
        {
            log::warn!("could not count dictionary hits: {e}");
        }
        let mut entries = self.entries.write().await;
        for term in &matched {
            if let Some(entry) = entries.get_mut(term) {
                entry.hits += 1;
            }
        }
        corrected
    }
}

#[cfg(test)]
#[path = "../../tests/unit/text/dictionary.rs"]
mod tests;
