//! Saved audio: one WAV per History entry, beside the database.
//! Writing it never delays or fails a dictation, and the database records which
//! entries have a file, so a player is only offered for one that exists.

pub mod wav;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::bridge::events::{EntryAudio, SavedAudioChanged};
use crate::data::repositories::transcript::TranscriptRepository;
use crate::data::Database;
use crate::models::preprocess::To16k;
use crate::settings::Settings;

pub struct Recordings {
    dir: PathBuf,
    /// Ids with a file when the app opened the folder, before any save could
    /// start: [`reconcile`] judges only these.
    at_launch: Vec<i64>,
}

impl Recordings {
    pub fn open(dir: PathBuf) -> Self {
        let at_launch = scan(&dir);
        Self { dir, at_launch }
    }

    pub fn path(&self, id: i64) -> PathBuf {
        self.dir.join(format!("{id}.wav"))
    }

    /// 16 kHz mono samples of an entry's saved audio.
    pub fn read(&self, id: i64) -> Result<Vec<f32>, String> {
        let bytes = std::fs::read(self.path(id)).map_err(|e| format!("no saved audio: {e}"))?;
        wav::decode(&bytes)
    }

    /// Encode and write through a temp file, so a crash never leaves a torn one.
    fn write(&self, id: i64, samples_16k: &[f32]) -> Result<(), String> {
        let bytes = wav::encode(samples_16k)?;
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let path = self.path(id);
        let tmp = path.with_extension("wav.part");
        std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    /// Delete entry `id`'s audio. A file already gone counts as deleted.
    pub fn remove(&self, id: i64) -> Result<(), String> {
        match std::fs::remove_file(self.path(id)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                Err(format!("could not delete the audio of entry {id}: {e}"))
            }
            _ => Ok(()),
        }
    }

    /// Bytes the saved audio takes.
    pub fn size(&self) -> u64 {
        std::fs::read_dir(&self.dir).map_or(0, |entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
    }
}

/// Ids of the WAVs in `dir`; temp files of interrupted writes are deleted.
fn scan(dir: &Path) -> Vec<i64> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".wav.part") {
            if let Err(e) = std::fs::remove_file(entry.path()) {
                log::warn!("could not delete the leftover {name}: {e}");
            }
        } else if let Some(id) = name.strip_suffix(".wav").and_then(|id| id.parse().ok()) {
            ids.push(id);
        }
    }
    ids
}

/// Keep a dictation's audio, at the microphone's own rate, in the background,
/// when saving audio is on.
pub fn keep_native(app: &AppHandle, id: i64, samples: Vec<f32>, native_rate: u32) {
    if !app.state::<Settings>().saved_audio().enabled {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let converted = tauri::async_runtime::spawn_blocking(move || {
            let mut converter = To16k::new(native_rate)?;
            let mut at_16k = converter.push(&samples)?;
            at_16k.extend(converter.finish()?);
            Ok::<_, String>(at_16k)
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r);
        let result = match converted {
            Ok(samples) => store(&app, id, samples).await,
            Err(e) => Err(e),
        };
        if let Err(e) = result {
            log::warn!("could not keep the audio of entry {id}: {e}");
        }
    });
}

/// Write entry `id`'s audio (16 kHz mono). Audio past the limit is deleted only
/// after the new file is down; a failure there doesn't fail this save.
pub async fn store(app: &AppHandle, id: i64, samples_16k: Vec<f32>) -> Result<(), String> {
    let written = {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            app.state::<Recordings>().write(id, &samples_16k)
        })
        .await
        .map_err(|e| e.to_string())?
    };
    written?;
    let flagged = async {
        let repo =
            TranscriptRepository::new(app.state::<Database>().pool().await.map_err(|e| e.0)?);
        repo.set_has_audio(id, true)
            .await
            .map_err(|e| e.to_string())
    };
    // Unflagged, the file would belong to no entry: the entry was deleted
    // while the audio encoded, or the flag couldn't be written.
    match flagged.await {
        Ok(true) => {}
        Ok(false) => return app.state::<Recordings>().remove(id),
        Err(e) => {
            if let Err(e) = app.state::<Recordings>().remove(id) {
                log::warn!("{e}");
            }
            return Err(e);
        }
    }
    let audio_path = app.state::<Recordings>().path(id);
    let _ = SavedAudioChanged(vec![EntryAudio {
        id,
        audio_path: Some(audio_path.to_string_lossy().into_owned()),
    }])
    .emit(app);
    // Pruning older audio is its own outcome: this file is saved either way.
    if let Err(e) = apply_limit(app).await {
        log::warn!("could not apply the saved audio limit: {e}");
    }
    Ok(())
}

/// Delete entry `id`'s audio and tell the window, whose disk usage moves.
pub fn delete(app: &AppHandle, id: i64) -> Result<(), String> {
    app.state::<Recordings>().remove(id)?;
    let _ = SavedAudioChanged(vec![EntryAudio {
        id,
        audio_path: None,
    }])
    .emit(app);
    Ok(())
}

/// Make the folder and the database agree after a launch: a file no entry
/// claims (left by a quit mid-delete) is deleted, and an entry whose file is
/// gone loses its player.
pub async fn reconcile(app: &AppHandle) -> Result<(), String> {
    let repo = TranscriptRepository::new(app.state::<Database>().pool().await.map_err(|e| e.0)?);
    let claimed: HashSet<i64> = repo
        .ids_with_audio()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .collect();
    let recordings = app.state::<Recordings>();
    let mut lost = Vec::new();
    for &id in &recordings.at_launch {
        if !claimed.contains(&id) {
            match recordings.remove(id) {
                Ok(()) => lost.push(id),
                Err(e) => log::warn!("{e}"),
            }
        }
    }
    for id in claimed {
        if !recordings.path(id).exists() {
            repo.set_has_audio(id, false)
                .await
                .map_err(|e| e.to_string())?;
            lost.push(id);
        }
    }
    // A window already open may show these players, or the bytes they took.
    if !lost.is_empty() {
        let lost = lost
            .into_iter()
            .map(|id| EntryAudio {
                id,
                audio_path: None,
            })
            .collect();
        let _ = SavedAudioChanged(lost).emit(app);
    }
    Ok(())
}

/// Delete the audio the saved limit no longer covers: all unpinned audio when
/// saving is off, else everything unpinned past the newest `keep`.
pub async fn apply_limit(app: &AppHandle) -> Result<(), String> {
    let saved = app.state::<Settings>().saved_audio();
    trim(app, saved.enabled.then_some(saved.keep)).await
}

/// Delete unpinned audio past the newest `keep`; `None` deletes it all. The
/// entries and their text stay.
pub async fn trim(app: &AppHandle, keep: Option<u32>) -> Result<(), String> {
    let repo = TranscriptRepository::new(app.state::<Database>().pool().await.map_err(|e| e.0)?);
    // The flag goes first, so a player is never offered for a file being
    // deleted. A file that won't delete gets its flag back: it still plays,
    // and the next trim tries it again.
    let released = repo
        .release_audio_beyond(keep)
        .await
        .map_err(|e| e.to_string())?;
    let recordings = app.state::<Recordings>();
    let mut lost = Vec::new();
    let mut failure = None;
    for id in released {
        match recordings.remove(id) {
            Ok(()) => lost.push(EntryAudio {
                id,
                audio_path: None,
            }),
            Err(e) => {
                repo.set_has_audio(id, true)
                    .await
                    .map_err(|e| e.to_string())?;
                failure = Some(e);
            }
        }
    }
    if !lost.is_empty() {
        let _ = SavedAudioChanged(lost).emit(app);
    }
    failure.map_or(Ok(()), Err)
}

#[cfg(test)]
#[path = "../../tests/unit/recordings/mod.rs"]
mod tests;
