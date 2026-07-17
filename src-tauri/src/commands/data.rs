//! Data commands: transcripts, usage stats, and dictionary CRUD.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::database::dto::{dictionary::CreateDictionaryEntry, transcript::CreateTranscript};
use crate::database::repositories::{
    dictionary::DictionaryRepository,
    transcript::{Cursor, TranscriptRepository},
};
use crate::postprocess::DictionaryCorrectionEngine;
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
    let repo = TranscriptRepository::new(state.db().await.clone());
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
}

#[tauri::command]
pub async fn get_transcripts(
    state: State<'_, AppState>,
    page: PageQuery,
) -> Result<Vec<TranscriptResponse>, ApiError> {
    let repo = TranscriptRepository::new(state.db().await.clone());
    let items = repo
        .list_keyset(
            page.limit(),
            page.cursor(),
            page.from.as_deref(),
            page.to.as_deref(),
            page.sort_desc(),
        )
        .await?;
    Ok(items.into_iter().map(TranscriptResponse::from).collect())
}

#[tauri::command]
pub async fn export_transcripts(
    state: State<'_, AppState>,
) -> Result<Vec<TranscriptResponse>, ApiError> {
    let repo = TranscriptRepository::new(state.db().await.clone());
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

    // Load user vocabulary from word_frequency table for fuzzy matching.
    // Best-effort: fuzzy matching is an enhancement, so an empty vocab on failure
    // just skips it — but log so the failure isn't fully silent.
    let vocab: Vec<String> =
        sqlx::query_scalar("SELECT word FROM word_frequency ORDER BY count DESC LIMIT 2000")
            .fetch_all(state.db().await)
            .await
            .unwrap_or_else(|e| {
                log::warn!("vocab load for fuzzy search failed: {e}");
                Vec::new()
            });

    let fts_query = TranscriptRepository::build_fts_query(&query, &vocab);

    // An all-punctuation query can normalize to an empty FTS string — that's
    // "no searchable terms", not an error.
    if fts_query.trim().is_empty() {
        return Ok(vec![]);
    }

    // Propagate real DB/FTS errors instead of masking them as empty results.
    let repo = TranscriptRepository::new(state.db().await.clone());
    let items = repo
        .search(
            &fts_query,
            page.limit(),
            page.cursor(),
            page.from.as_deref(),
            page.to.as_deref(),
            page.sort_desc(),
        )
        .await?;
    Ok(items.into_iter().map(TranscriptResponse::from).collect())
}

#[tauri::command]
pub async fn save_transcript(
    state: State<'_, AppState>,
    content: String,
) -> Result<TranscriptResponse, ApiError> {
    if content.trim().is_empty() {
        return Err(ApiError::new(
            "invalid_input",
            "transcript content cannot be empty",
        ));
    }
    let repo = TranscriptRepository::new(state.db().await.clone());
    #[allow(clippy::cast_possible_wrap)]
    let word_count = content.split_whitespace().count() as i64;
    let transcript = repo
        .create(CreateTranscript {
            content,
            word_count,
            duration_seconds: None,
        })
        .await?;
    Ok(transcript.into())
}

#[tauri::command]
pub async fn delete_transcript(state: State<'_, AppState>, id: i64) -> Result<bool, ApiError> {
    let repo = TranscriptRepository::new(state.db().await.clone());
    Ok(repo.delete_by_id(id).await?)
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

#[tauri::command]
pub async fn update_dictionary(
    state: State<'_, AppState>,
    term: String,
    replacement: String,
) -> Result<DictionaryResponse, ApiError> {
    let repo = DictionaryRepository::new(state.db().await.clone());
    let entry = repo
        .upsert(CreateDictionaryEntry {
            term: term.clone(),
            replacement,
        })
        .await?;

    // Update in-memory cache: O(1) insert/replace via HashMap
    state
        .dict_cache
        .write()
        .await
        .insert(entry.term.clone(), entry.clone());

    Ok(entry.into())
}

#[tauri::command]
pub async fn delete_dictionary_entry(
    state: State<'_, AppState>,
    id: i64,
) -> Result<bool, ApiError> {
    let repo = DictionaryRepository::new(state.db().await.clone());
    let deleted = repo.delete_by_id(id).await?;
    if deleted {
        let mut cache = state.dict_cache.write().await;
        cache.retain(|_, e| e.id != id);
    }
    Ok(deleted)
}

#[tauri::command]
pub async fn apply_dictionary(
    state: State<'_, AppState>,
    text: String,
) -> Result<String, ApiError> {
    let entries: Vec<_> = state.dict_cache.read().await.values().cloned().collect();
    let engine = DictionaryCorrectionEngine::new(entries);
    Ok(engine.apply_to_text(&text).0)
}
