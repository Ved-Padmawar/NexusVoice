//! Transcripts, usage stats and the dictionary.

use crate::database::dto::dictionary::CreateDictionaryEntry;
use crate::database::repositories::{
    dictionary::DictionaryRepository,
    transcript::{Cursor, TranscriptRepository},
};
use crate::state;

use super::dto::{DictionaryResponse, TranscriptResponse};
use super::error::ApiError;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UsageStatsResponse {
    pub total_words: i64,
    pub speaking_time_seconds: i64,
    pub total_sessions: i64,
    pub avg_pace_wpm: i64,
}

pub async fn get_usage_stats() -> Result<UsageStatsResponse, ApiError> {
    let state = state::get();
    let repo = TranscriptRepository::new(state.db().await?.clone());
    let (total_sessions, total_words, total_duration_seconds) = repo.get_stats().await?;

    #[allow(clippy::cast_possible_truncation)] // durations and word counts fit i64
    let speaking_time_seconds = total_duration_seconds.round() as i64;

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let avg_pace_wpm = if total_duration_seconds > 0.0 {
        ((total_words as f64 / (total_duration_seconds / 60.0)).round()) as i64
    } else {
        0
    };

    Ok(UsageStatsResponse {
        total_words,
        speaking_time_seconds,
        total_sessions,
        avg_pace_wpm,
    })
}

pub async fn wait_until_ready() -> Result<(), ApiError> {
    state::get().db().await?;
    Ok(())
}

const MAX_PAGE_SIZE: i64 = 200;
const DEFAULT_PAGE_SIZE: i64 = 50;

#[derive(Debug, Clone, Default)]
pub struct PageQuery {
    pub limit: Option<i64>,
    pub cursor_created_at: Option<String>,
    pub cursor_id: Option<i64>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub sort_asc: Option<bool>,
}

impl PageQuery {
    /// Clamped so a client can't pull the whole table into memory.
    fn limit(&self) -> i64 {
        self.limit
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE)
    }

    /// Either half alone is a client bug; treat it as none.
    fn cursor(&self) -> Option<Cursor<'_>> {
        match (self.cursor_created_at.as_deref(), self.cursor_id) {
            (Some(created_at), Some(id)) => Some(Cursor { created_at, id }),
            _ => None,
        }
    }

    fn sort_desc(&self) -> bool {
        !self.sort_asc.unwrap_or(false)
    }

    fn start_bound(&self) -> Option<&str> {
        self.from.as_deref()
    }

    /// Inclusive upper bound: a bare `YYYY-MM-DD` is extended to the day's end, as
    /// `created_at` compares lexicographically.
    fn end_bound(&self) -> Option<String> {
        self.to.as_deref().map(|t| {
            if t.len() == 10 {
                format!("{t} 23:59:59")
            } else {
                t.to_string()
            }
        })
    }
}

pub async fn get_transcripts(page: PageQuery) -> Result<Vec<TranscriptResponse>, ApiError> {
    let state = state::get();
    let repo = TranscriptRepository::new(state.db().await?.clone());
    let to = page.end_bound();
    let items = repo
        .list_keyset(
            page.limit(),
            page.cursor(),
            page.start_bound(),
            to.as_deref(),
            page.sort_desc(),
        )
        .await?;
    Ok(items.into_iter().map(TranscriptResponse::from).collect())
}

pub async fn export_transcripts() -> Result<Vec<TranscriptResponse>, ApiError> {
    let state = state::get();
    let repo = TranscriptRepository::new(state.db().await?.clone());
    let items = repo.list_all().await?;
    Ok(items.into_iter().map(TranscriptResponse::from).collect())
}

pub async fn search_transcripts(
    query: String,
    page: PageQuery,
) -> Result<Vec<TranscriptResponse>, ApiError> {
    let state = state::get();
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let fts_query = TranscriptRepository::build_fts_query(&query);

    // An all-punctuation query has no searchable terms; not an error.
    if fts_query.trim().is_empty() {
        return Ok(vec![]);
    }

    let repo = TranscriptRepository::new(state.db().await?.clone());
    let to = page.end_bound();
    let items = repo
        .search(
            &fts_query,
            page.limit(),
            page.cursor(),
            page.start_bound(),
            to.as_deref(),
            page.sort_desc(),
        )
        .await?;
    Ok(items.into_iter().map(TranscriptResponse::from).collect())
}

pub async fn delete_transcript(id: i64) -> Result<(), ApiError> {
    let state = state::get();
    let repo = TranscriptRepository::new(state.db().await?.clone());
    repo.delete_by_id(id).await?;
    Ok(())
}

pub async fn get_dictionary() -> Result<Vec<DictionaryResponse>, ApiError> {
    let state = state::get();
    let cache = state.dict_cache.read().await;
    let mut entries: Vec<_> = cache.values().cloned().collect();
    // The cache is a HashMap: sort, or rows reshuffle after every edit.
    entries.sort_unstable_by(|a, b| a.term.cmp(&b.term));
    Ok(entries.into_iter().map(DictionaryResponse::from).collect())
}

/// The upsert keys on `term`, so a rename must drop `previous_term` or it lingers.
pub async fn update_dictionary(
    term: String,
    replacement: String,
    previous_term: Option<String>,
) -> Result<DictionaryResponse, ApiError> {
    let state = state::get();
    let repo = DictionaryRepository::new(state.db().await?.clone());
    let entry = repo
        .upsert(CreateDictionaryEntry {
            term: term.clone(),
            replacement,
        })
        .await?;

    let renamed_from = previous_term.filter(|previous| previous != &entry.term);
    if let Some(previous) = &renamed_from {
        repo.delete_by_term(previous).await?;
    }

    let mut cache = state.dict_cache.write().await;
    if let Some(previous) = &renamed_from {
        cache.remove(previous);
    }
    cache.insert(entry.term.clone(), entry.clone());
    drop(cache);

    Ok(entry.into())
}

pub async fn delete_dictionary_entry(id: i64) -> Result<(), ApiError> {
    let state = state::get();
    let repo = DictionaryRepository::new(state.db().await?.clone());
    if repo.delete_by_id(id).await? {
        let mut cache = state.dict_cache.write().await;
        cache.retain(|_, e| e.id != id);
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/api/data.rs"]
mod tests;
