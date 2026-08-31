//! Data commands: transcripts, usage stats, and dictionary CRUD.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::database::dto::dictionary::CreateDictionaryEntry;
use crate::database::repositories::{
    dictionary::DictionaryRepository,
    transcript::{Cursor, TranscriptRepository},
};
use crate::state::AppState;

use super::dto::{DictionaryResponse, TranscriptResponse};
use super::error::ApiError;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStatsResponse {
    pub total_words: i64,
    pub speaking_time_seconds: i64,
    pub total_sessions: i64,
    pub avg_pace_wpm: i64,
}

#[tauri::command]
pub async fn get_usage_stats(state: State<'_, AppState>) -> Result<UsageStatsResponse, ApiError> {
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

const MAX_PAGE_SIZE: i64 = 200;
const DEFAULT_PAGE_SIZE: i64 = 50;

/// Pagination, cursor and filter args shared by the transcript list and search.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageQuery {
    limit: Option<i64>,
    cursor_created_at: Option<String>,
    cursor_id: Option<i64>,
    from: Option<String>,
    to: Option<String>,
    sort_asc: Option<bool>,
}

impl PageQuery {
    /// Clamped so a client can't pull the whole table into memory.
    fn limit(&self) -> i64 {
        self.limit
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE)
    }

    /// Both halves must be present; either alone is a client bug — treat as none.
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

    /// Inclusive upper bound. The picker sends a bare `YYYY-MM-DD` but
    /// `created_at` is `YYYY-MM-DD HH:MM:SS`, compared lexicographically, so a
    /// bare date would exclude the whole day it names.
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

#[tauri::command]
pub async fn get_transcripts(
    state: State<'_, AppState>,
    page: PageQuery,
) -> Result<Vec<TranscriptResponse>, ApiError> {
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

#[tauri::command]
pub async fn export_transcripts(
    state: State<'_, AppState>,
) -> Result<Vec<TranscriptResponse>, ApiError> {
    let repo = TranscriptRepository::new(state.db().await?.clone());
    let items = repo.list_all().await?;
    Ok(items.into_iter().map(TranscriptResponse::from).collect())
}

#[tauri::command]
pub async fn search_transcripts(
    state: State<'_, AppState>,
    query: String,
    page: PageQuery,
) -> Result<Vec<TranscriptResponse>, ApiError> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let fts_query = TranscriptRepository::build_fts_query(&query);

    // An all-punctuation query can normalize to an empty FTS string — that's
    // "no searchable terms", not an error.
    if fts_query.trim().is_empty() {
        return Ok(vec![]);
    }

    // Propagate real DB/FTS errors instead of masking them as empty results.
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

#[tauri::command]
pub async fn delete_transcript(state: State<'_, AppState>, id: i64) -> Result<(), ApiError> {
    let repo = TranscriptRepository::new(state.db().await?.clone());
    repo.delete_by_id(id).await?;
    Ok(())
}

#[tauri::command]
pub async fn get_dictionary(
    state: State<'_, AppState>,
) -> Result<Vec<DictionaryResponse>, ApiError> {
    let cache = state.dict_cache.read().await;
    Ok(cache
        .values()
        .cloned()
        .map(DictionaryResponse::from)
        .collect())
}

/// The upsert keys on `term`, so a rename inserts a new row; `previous_term`
/// names the old one to drop, or it lingers and keeps rewriting dictation.
#[tauri::command]
pub async fn update_dictionary(
    state: State<'_, AppState>,
    term: String,
    replacement: String,
    previous_term: Option<String>,
) -> Result<DictionaryResponse, ApiError> {
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

    // Update in-memory cache: O(1) insert/replace via HashMap
    let mut cache = state.dict_cache.write().await;
    if let Some(previous) = &renamed_from {
        cache.remove(previous);
    }
    cache.insert(entry.term.clone(), entry.clone());
    drop(cache);

    Ok(entry.into())
}

#[tauri::command]
pub async fn delete_dictionary_entry(state: State<'_, AppState>, id: i64) -> Result<(), ApiError> {
    let repo = DictionaryRepository::new(state.db().await?.clone());
    if repo.delete_by_id(id).await? {
        let mut cache = state.dict_cache.write().await;
        cache.retain(|_, e| e.id != id);
    }
    Ok(())
}
