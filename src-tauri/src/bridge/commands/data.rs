//! Data commands: transcripts, usage stats, the dictionary and exports.

use chrono::{DateTime, Local, NaiveDate, NaiveTime, TimeDelta, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use tauri::{AppHandle, Manager, State};

use crate::data::repositories::transcript::{Cursor, TranscriptRepository};
use crate::data::Database;
use crate::text::dictionary::{Dictionary, DictionaryError};

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
        return Err(ApiError::new(
            "invalid_input",
            "Couldn't work out your time zone.",
        ));
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

    fn created_at_range(&self) -> Result<CreatedAtRange, ApiError> {
        self.created_at_range_in(&Local)
    }

    /// `from` and `to` are local `YYYY-MM-DD` days but `created_at` is UTC, so
    /// each day widens to the UTC instants it spans in `tz`.
    fn created_at_range_in<Tz: TimeZone>(&self, tz: &Tz) -> Result<CreatedAtRange, ApiError> {
        let start = self
            .from
            .as_deref()
            .map(|day| day_start_utc(parse_day(day)?, tz))
            .transpose()?;
        let end = self
            .to
            .as_deref()
            .map(|day| {
                let next = parse_day(day)?.succ_opt().ok_or_else(bad_date_filter)?;
                Ok::<_, ApiError>(day_start_utc(next, tz)? - TimeDelta::seconds(1))
            })
            .transpose()?;
        Ok(CreatedAtRange {
            start: start.map(sqlite_datetime),
            end: end.map(sqlite_datetime),
        })
    }
}

/// Inclusive bounds in `created_at`'s text form, so they compare
/// lexicographically against the column.
struct CreatedAtRange {
    start: Option<String>,
    end: Option<String>,
}

/// The UI picks dates from a calendar, so a bad one is never the user's typing.
fn bad_date_filter() -> ApiError {
    ApiError::new(
        "invalid_input",
        "Couldn't apply that date filter. Try picking the dates again.",
    )
}

fn parse_day(day: &str) -> Result<NaiveDate, ApiError> {
    NaiveDate::parse_from_str(day, "%Y-%m-%d").map_err(|_| bad_date_filter())
}

/// First instant of `day` in `tz`. Where a DST change skips midnight, the day
/// starts when the clocks resume, an hour later.
fn day_start_utc<Tz: TimeZone>(day: NaiveDate, tz: &Tz) -> Result<DateTime<Utc>, ApiError> {
    let midnight = day.and_time(NaiveTime::MIN);
    tz.from_local_datetime(&midnight)
        .earliest()
        .or_else(|| {
            tz.from_local_datetime(&(midnight + TimeDelta::hours(1)))
                .earliest()
        })
        .map(|start| start.with_timezone(&Utc))
        .ok_or_else(bad_date_filter)
}

/// `SQLite`'s `CURRENT_TIMESTAMP` form, which `created_at` holds.
fn sqlite_datetime(at: DateTime<Utc>) -> String {
    at.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[tauri::command]
#[specta::specta]
pub async fn get_transcripts(
    db: State<'_, Database>,
    page: PageQuery,
) -> Result<Vec<TranscriptResponse>, ApiError> {
    let range = page.created_at_range()?;
    let repo = TranscriptRepository::new(db.pool().await?);
    let items = repo
        .list_keyset(
            page.limit(),
            page.cursor(),
            range.start.as_deref(),
            range.end.as_deref(),
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

    let range = page.created_at_range()?;
    // Propagate real DB/FTS errors instead of masking them as empty results.
    let repo = TranscriptRepository::new(db.pool().await?);
    let items = repo
        .search(
            &fts_query,
            page.limit(),
            page.cursor(),
            range.start.as_deref(),
            range.end.as_deref(),
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

/// Add an entry, or with `id` edit that one (a rename keeps its use count).
#[tauri::command]
#[specta::specta]
pub async fn update_dictionary(
    db: State<'_, Database>,
    dictionary: State<'_, Dictionary>,
    term: String,
    replacement: String,
    id: Option<i64>,
) -> Result<DictionaryResponse, ApiError> {
    let entry = dictionary
        .save(&db.pool().await?, id, &term, &replacement)
        .await
        .map_err(|e| match e {
            DictionaryError::Invalid(message) => ApiError::new("invalid_input", message),
            DictionaryError::Duplicate(message) => ApiError::new("dictionary_duplicate", message),
            DictionaryError::Database(e) => e.into(),
        })?;
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

/// Write an export into the user's Downloads folder and return where it went.
/// An existing file is never overwritten: the name gets " (2)", " (3)", ….
#[tauri::command]
#[specta::specta]
pub async fn save_export(
    app: AppHandle,
    file_name: String,
    content: String,
) -> Result<String, ApiError> {
    if !is_bare_file_name(&file_name) {
        return Err(ApiError::new(
            "invalid_input",
            "Couldn't save the export to Downloads.",
        ));
    }
    let downloads = app.path().download_dir().map_err(|e| {
        log::warn!("downloads folder not found: {e}");
        ApiError::new("io_error", "Couldn't find your Downloads folder.")
    })?;
    // Off the main thread: a large export or a slow folder would freeze every window.
    let path = tauri::async_runtime::spawn_blocking(move || {
        write_without_overwriting(&downloads, &file_name, &content)
    })
    .await
    .expect("export write panicked")?;
    Ok(path.to_string_lossy().into_owned())
}

/// Open the file manager with `path` selected.
#[tauri::command]
#[specta::specta]
pub fn reveal_in_folder(path: String) -> Result<(), ApiError> {
    opener::reveal(path).map_err(|e| {
        log::warn!("reveal in folder failed: {e}");
        ApiError::new("open_error", "Couldn't open the folder.")
    })
}

/// A single file name: no directories, and none of the characters Windows
/// refuses in one. Backslash is listed because Unix would accept it as a
/// plain character.
fn is_bare_file_name(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    ) && !name.contains(['\\', '<', '>', ':', '"', '|', '?', '*'])
}

/// Create `dir/file_name`, or the first free "name (n).ext", and fill it.
/// Creation is exclusive, so a file that appears meanwhile is not clobbered.
fn write_without_overwriting(
    dir: &Path,
    file_name: &str,
    content: &str,
) -> std::io::Result<PathBuf> {
    let name = Path::new(file_name);
    let stem = name.file_stem().unwrap_or_default().to_string_lossy();
    let extension = name
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    for attempt in 1.. {
        let candidate = if attempt == 1 {
            dir.join(file_name)
        } else {
            dir.join(format!("{stem} ({attempt}){extension}"))
        };
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                file.write_all(content.as_bytes())?;
                return Ok(candidate);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    unreachable!("the attempt counter is unbounded")
}

#[cfg(test)]
#[path = "../../../tests/unit/bridge/commands/data.rs"]
mod tests;
