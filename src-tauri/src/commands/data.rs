//! Data commands: transcripts, usage stats and the dictionary.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::database::repositories::transcript::{Cursor, TranscriptRepository};
use crate::database::Database;
use crate::dictionary::Dictionary;

use super::dto::{DictionaryResponse, TranscriptResponse};
use super::error::ApiError;

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UsageStatsResponse {
    pub total_words: i64,
    pub speaking_time_seconds: i64,
    pub total_sessions: i64,
    pub avg_pace_wpm: i64,
}

#[tauri::command]
#[specta::specta]
pub async fn get_usage_stats(db: State<'_, Database>) -> Result<UsageStatsResponse, ApiError> {
    let repo = TranscriptRepository::new(db.pool().await?);
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

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DailyWordsResponse {
    /// Local calendar day, `YYYY-MM-DD`.
    pub day: String,
    pub words: i64,
}

/// A year, the most the dashboard could ever draw.
const MAX_DAYS: u32 = 366;
/// UTC−12 to UTC+14, the range real time zones span.
const UTC_OFFSET_RANGE: std::ops::RangeInclusive<i32> = -12 * 60..=14 * 60;

/// Words per day for the dashboard's waveform, in the caller's time zone.
#[tauri::command]
#[specta::specta]
pub async fn get_daily_words(
    db: State<'_, Database>,
    days: u32,
    utc_offset_minutes: i32,
) -> Result<Vec<DailyWordsResponse>, ApiError> {
    if !UTC_OFFSET_RANGE.contains(&utc_offset_minutes) {
        return Err(ApiError::new("invalid_input", "UTC offset is out of range"));
    }
    let rows = TranscriptRepository::new(db.pool().await?)
        .daily_words(
            i64::from(days.clamp(1, MAX_DAYS)),
            i64::from(utc_offset_minutes),
        )
        .await?;
    Ok(rows
        .into_iter()
        .map(|(day, words)| DailyWordsResponse { day, words })
        .collect())
}

const MAX_PAGE_SIZE: i64 = 200;
const DEFAULT_PAGE_SIZE: i64 = 50;

/// Pagination, cursor and filter args shared by the transcript list and search.
#[derive(Debug, Default, Deserialize, specta::Type)]
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
#[specta::specta]
pub async fn get_transcripts(
    db: State<'_, Database>,
    page: PageQuery,
) -> Result<Vec<TranscriptResponse>, ApiError> {
    let repo = TranscriptRepository::new(db.pool().await?);
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
#[specta::specta]
pub async fn search_transcripts(
    db: State<'_, Database>,
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
    let repo = TranscriptRepository::new(db.pool().await?);
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
#[specta::specta]
pub async fn delete_transcript(db: State<'_, Database>, id: i64) -> Result<(), ApiError> {
    TranscriptRepository::new(db.pool().await?)
        .delete_by_id(id)
        .await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_dictionary(
    dictionary: State<'_, Dictionary>,
) -> Result<Vec<DictionaryResponse>, ApiError> {
    let entries = dictionary.entries().await;
    Ok(entries.into_iter().map(DictionaryResponse::from).collect())
}

/// `previous_term` names the entry being renamed, if any.
#[tauri::command]
#[specta::specta]
pub async fn update_dictionary(
    db: State<'_, Database>,
    dictionary: State<'_, Dictionary>,
    term: String,
    replacement: String,
    previous_term: Option<String>,
) -> Result<DictionaryResponse, ApiError> {
    let entry = dictionary
        .upsert(&db.pool().await?, term, replacement, previous_term)
        .await?;
    Ok(entry.into())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_dictionary_entry(
    db: State<'_, Database>,
    dictionary: State<'_, Dictionary>,
    id: i64,
) -> Result<(), ApiError> {
    Ok(dictionary.delete(&db.pool().await?, id).await?)
}

#[cfg(test)]
#[path = "../../tests/unit/commands/data.rs"]
mod tests;
