//! Saved dictation audio, and the jobs that transcribe it: re-transcribing a
//! history entry and importing an audio file.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::data::repositories::transcript::TranscriptRepository;
use crate::data::Database;
use crate::jobs::{JobSnapshot, Jobs, Previous};
use crate::models::catalog::{self, ModelEntry};
use crate::models::library::ModelLibrary;
use crate::recordings::Recordings;
use crate::settings::{SavedAudio, Settings};

use super::error::ApiError;

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SavedAudioView {
    pub enabled: bool,
    pub keep: u32,
    /// Disk the saved audio takes.
    pub bytes: u64,
}

#[tauri::command]
#[specta::specta]
pub fn get_saved_audio(
    settings: State<'_, Settings>,
    recordings: State<'_, Recordings>,
) -> SavedAudioView {
    let SavedAudio { enabled, keep } = settings.saved_audio();
    SavedAudioView {
        enabled,
        keep,
        bytes: recordings.size(),
    }
}

/// Save the policy and apply it now: audio past the new limit, or all of it
/// when switched off, is deleted before this returns. An error means the
/// policy was not saved.
#[tauri::command]
#[specta::specta]
pub async fn set_saved_audio(
    app: AppHandle,
    settings: State<'_, Settings>,
    enabled: bool,
    keep: u32,
) -> Result<(), ApiError> {
    if !SavedAudio::KEEP_RANGE.contains(&keep) {
        return Err(ApiError::new(
            "invalid_input",
            "Keep between 1 and 100 entries.",
        ));
    }
    settings.set_saved_audio(SavedAudio { enabled, keep })?;
    // Pruning is its own outcome, like after a save: the next one tries again.
    if let Err(e) = crate::recordings::apply_limit(&app).await {
        log::warn!("could not apply the saved audio limit: {e}");
    }
    Ok(())
}

/// Delete every unpinned entry's audio. The entries and their text stay.
/// Refused while files wait to be transcribed: they still need theirs.
#[tauri::command]
#[specta::specta]
pub async fn clear_saved_audio(
    app: AppHandle,
    db: State<'_, Database>,
) -> Result<ClearedAudio, ApiError> {
    let repo = TranscriptRepository::new(db.pool().await?);
    if !repo.pending().await?.is_empty() {
        return Err(ApiError::new(
            "imports_pending",
            "Files are still being transcribed. Clear the audio once they finish.",
        ));
    }
    let deleted = crate::recordings::trim(&app, None).await.map_err(|e| {
        log::warn!("could not clear the saved audio: {e}");
        ApiError::new("io_error", "Couldn't delete the saved audio.")
    })?;
    Ok(ClearedAudio {
        deleted,
        kept: repo.ids_with_audio().await?.len(),
    })
}

/// What clearing the saved audio did; what is left is pinned.
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ClearedAudio {
    pub deleted: usize,
    pub kept: usize,
}

/// Pin an entry to keep its audio past the limit; unpinning applies the limit again.
#[tauri::command]
#[specta::specta]
pub async fn set_transcript_pinned(
    app: AppHandle,
    db: State<'_, Database>,
    id: i64,
    pinned: bool,
) -> Result<(), ApiError> {
    TranscriptRepository::new(db.pool().await?)
        .set_pinned(id, pinned)
        .await?;
    // The unpin is saved either way; pruning is its own outcome.
    if !pinned {
        if let Err(e) = crate::recordings::apply_limit(&app).await {
            log::warn!("could not apply the saved audio limit: {e}");
        }
    }
    Ok(())
}

/// Put back what a re-transcription replaced, which is how one is undone.
/// `revision` is the entry's after that re-transcription: once it moved on, a
/// later change happened and this undo no longer applies.
#[tauri::command]
#[specta::specta]
pub async fn restore_transcript(
    db: State<'_, Database>,
    id: i64,
    previous: Previous,
    revision: i64,
) -> Result<(), ApiError> {
    if previous.text.trim().is_empty() {
        return Err(ApiError::new(
            "invalid_input",
            "A transcript can't be empty.",
        ));
    }
    if !TranscriptRepository::new(db.pool().await?)
        .restore_text(id, revision, &previous.text, previous.model.as_deref())
        .await?
    {
        return Err(ApiError::new(
            "stale",
            "This entry changed since, so there's nothing to undo.",
        ));
    }
    Ok(())
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioFileInfo {
    pub name: String,
    pub size_bytes: u64,
    /// `None` when the file doesn't say how long it plays.
    pub duration_seconds: Option<f64>,
}

/// What the import dialog shows about a picked or dropped file.
#[tauri::command]
#[specta::specta]
pub async fn inspect_audio_file(path: String) -> Result<AudioFileInfo, ApiError> {
    let path = PathBuf::from(path);
    tauri::async_runtime::spawn_blocking(move || {
        let size_bytes = std::fs::metadata(&path)
            .ok()
            .filter(std::fs::Metadata::is_file)
            .ok_or_else(|| ApiError::new("invalid_input", "Couldn't open that file."))?
            .len();
        let duration_seconds = crate::audio::file::duration_seconds(&path)
            .map_err(|message| ApiError::new("invalid_input", message))?;
        Ok(AudioFileInfo {
            name: path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
            size_bytes,
            duration_seconds,
        })
    })
    .await
    .expect("audio inspection panicked")
}

/// The running transcription job, then the queued ones in order.
#[tauri::command]
#[specta::specta]
pub fn get_jobs(jobs: State<'_, Jobs>) -> Vec<JobSnapshot> {
    jobs.list()
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_job(app: AppHandle, transcript_id: i64) -> Result<(), ApiError> {
    crate::jobs::cancel(&app, transcript_id).await;
    Ok(())
}

fn installed_model(library: &ModelLibrary, id: &str) -> Result<&'static ModelEntry, ApiError> {
    catalog::find(id)
        .filter(|entry| library.is_downloaded(entry))
        .ok_or_else(|| ApiError::new("model_missing", "That model isn't downloaded."))
}

/// Queue an entry's saved audio to be transcribed again with `model`.
#[tauri::command]
#[specta::specta]
pub async fn retranscribe_transcript(
    app: AppHandle,
    db: State<'_, Database>,
    library: State<'_, ModelLibrary>,
    id: i64,
    model: String,
) -> Result<(), ApiError> {
    let model = installed_model(&library, &model)?;
    let entry = TranscriptRepository::new(db.pool().await?)
        .get_by_id(id)
        .await?
        .filter(|t| t.has_audio)
        .ok_or_else(|| ApiError::new("audio_missing", "This entry's audio is no longer saved."))?;
    if !crate::jobs::retranscribe(&app, &entry, model) {
        return Err(ApiError::new(
            "job_queued",
            "This entry is already being transcribed.",
        ));
    }
    Ok(())
}

/// Add an audio file to History and queue its transcription. Nothing is
/// pasted.
#[tauri::command]
#[specta::specta]
pub async fn import_audio_file(
    app: AppHandle,
    library: State<'_, ModelLibrary>,
    path: String,
    model: String,
    language: Option<String>,
) -> Result<(), ApiError> {
    let model = installed_model(&library, &model)?;
    crate::jobs::import(&app, &PathBuf::from(path), model, language)
        .await
        .map_err(|message| ApiError::new("invalid_input", message))
}
