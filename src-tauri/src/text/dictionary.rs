//! The user's dictionary: saved in SQLite, mirrored in memory so the finalize
//! step corrects a transcript without a query.

use std::collections::HashMap;

use sqlx::SqlitePool;
use tokio::sync::RwLock;

use crate::data::dto::dictionary::CreateDictionaryEntry;
use crate::data::repositories::dictionary::DictionaryRepository;
use crate::data::rows::dictionary::DictionaryEntry;
use crate::text::DictionaryCorrectionEngine;

#[derive(Default)]
pub struct Dictionary {
    /// Keyed by term, the column the upsert dedupes on.
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

    /// The upsert keys on `term`, so a rename inserts a new row;
    /// `previous_term` names the old one to drop, or it lingers and keeps
    /// rewriting dictation.
    pub async fn upsert(
        &self,
        pool: &SqlitePool,
        term: String,
        replacement: String,
        previous_term: Option<String>,
    ) -> Result<DictionaryEntry, sqlx::Error> {
        let repo = DictionaryRepository::new(pool.clone());
        let entry = repo
            .upsert(CreateDictionaryEntry { term, replacement })
            .await?;

        let renamed_from = previous_term.filter(|previous| previous != &entry.term);
        if let Some(previous) = &renamed_from {
            repo.delete_by_term(previous).await?;
        }

        let mut entries = self.entries.write().await;
        if let Some(previous) = &renamed_from {
            entries.remove(previous);
        }
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
