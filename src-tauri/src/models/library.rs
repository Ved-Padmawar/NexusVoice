//! Models on disk and the downloads that put them there.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio_util::sync::CancellationToken;

use crate::bridge::events::ModelDownloadCancelled;
use crate::settings::Settings;
use crate::sync::{announce, lock_recovering, Topic};

use super::cache::EngineCache;
use super::catalog::{self, ModelEntry};
use super::downloader::{download_model, CANCELLED};
use super::provider::{detect_backend, select_model};

/// How many models may transfer at once. Extras wait in `Queued` rather than
/// being refused, so the user can line several up and walk away.
pub const MAX_CONCURRENT_DOWNLOADS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DownloadStatus {
    Queued,
    Running,
    Error,
}

struct DownloadEntry {
    status: DownloadStatus,
    progress: u8,
    error: Option<String>,
    /// Awaited alongside the network read, so a cancel interrupts immediately
    /// instead of waiting for the current chunk to finish.
    cancel: CancellationToken,
    /// Make the model the active one once it is on disk.
    activate: bool,
}

/// Every download in flight or waiting, keyed by model id. Success and cancel
/// remove the entry; the file on disk is the truth for what is installed.
/// Every change a snapshot would show is announced.
pub struct Downloads {
    entries: Mutex<HashMap<String, DownloadEntry>>,
    permits: Arc<tokio::sync::Semaphore>,
}

impl Downloads {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            permits: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_DOWNLOADS)),
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, DownloadEntry>> {
        lock_recovering(&self.entries)
    }

    /// Change `id`'s entry, announcing it when `change` reports a difference.
    fn edit(&self, id: &str, change: impl FnOnce(&mut DownloadEntry) -> bool) {
        if self.lock().get_mut(id).is_some_and(change) {
            announce(Topic::Downloads);
        }
    }

    /// Registers a queued download, returning its cancel token. A model already
    /// queued or running keeps its entry (gaining `activate` if asked) and
    /// returns `None`.
    pub fn enqueue(&self, id: &str, activate: bool) -> Option<CancellationToken> {
        let mut entries = self.lock();
        if let Some(entry) = entries
            .get_mut(id)
            .filter(|e| e.status != DownloadStatus::Error)
        {
            entry.activate |= activate;
            return None;
        }
        let cancel = CancellationToken::new();
        entries.insert(
            id.to_string(),
            DownloadEntry {
                status: DownloadStatus::Queued,
                progress: 0,
                error: None,
                cancel: cancel.clone(),
                activate,
            },
        );
        announce(Topic::Downloads);
        Some(cancel)
    }

    pub fn activates(&self, id: &str) -> bool {
        self.lock().get(id).is_some_and(|e| e.activate)
    }

    pub fn set_running(&self, id: &str) {
        self.edit(id, |e| {
            e.status = DownloadStatus::Running;
            true
        });
    }

    pub fn set_progress(&self, id: &str, pct: u8) {
        self.edit(id, |e| std::mem::replace(&mut e.progress, pct) != pct);
    }

    pub fn set_error(&self, id: &str, msg: String) {
        self.edit(id, |e| {
            e.status = DownloadStatus::Error;
            e.error = Some(msg);
            true
        });
    }

    pub fn remove(&self, id: &str) {
        if self.lock().remove(id).is_some() {
            announce(Topic::Downloads);
        }
    }

    /// Trips the cancel token, if this model has an entry to cancel.
    pub fn cancel(&self, id: &str) -> bool {
        match self.lock().get(id) {
            Some(e) if e.status != DownloadStatus::Error => {
                e.cancel.cancel();
                true
            }
            _ => false,
        }
    }

    /// `(id, status, progress, error)` per entry.
    pub fn snapshot(&self) -> Vec<(String, DownloadStatus, u8, Option<String>)> {
        self.lock()
            .iter()
            .map(|(id, e)| (id.clone(), e.status, e.progress, e.error.clone()))
            .collect()
    }
}

/// The most capable model on disk other than `except`. The catalog is
/// tier-ascending, so the last match is the best one.
pub fn best_on_disk(models_dir: &Path, except: Option<&str>) -> Option<&'static ModelEntry> {
    catalog::all()
        .iter()
        .rfind(|m| Some(m.id.as_str()) != except && models_dir.join(&m.filename).exists())
}

/// Every model file added or deleted is announced. Which one is active is a
/// setting, announced by `Settings`.
pub struct ModelLibrary {
    models_dir: PathBuf,
    downloads: Arc<Downloads>,
}

impl ModelLibrary {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            models_dir,
            downloads: Arc::new(Downloads::new()),
        }
    }

    /// The model recordings use: the saved choice, else the hardware pick.
    pub fn active(settings: &Settings) -> &'static ModelEntry {
        select_model(detect_backend(), settings.model().as_deref())
    }

    pub fn dir(&self) -> &Path {
        &self.models_dir
    }

    pub fn path(&self, entry: &ModelEntry) -> PathBuf {
        self.models_dir.join(&entry.filename)
    }

    pub fn is_downloaded(&self, entry: &ModelEntry) -> bool {
        self.path(entry).exists()
    }

    pub fn best_on_disk(&self, except: Option<&str>) -> Option<&'static ModelEntry> {
        best_on_disk(&self.models_dir, except)
    }

    pub fn downloads(&self) -> Vec<(String, DownloadStatus, u8, Option<String>)> {
        self.downloads.snapshot()
    }

    pub fn cancel_download(&self, id: &str) {
        self.downloads.cancel(id);
    }

    /// Make `entry` the active model and warm it in the background, so the
    /// first recording after a switch isn't stalled by the load.
    pub fn select(app: &AppHandle, entry: &ModelEntry) -> std::io::Result<()> {
        app.state::<Settings>().set_model(Some(entry.id.clone()))?;
        app.state::<EngineCache>().unload();
        EngineCache::warm(app);
        Ok(())
    }

    /// Publish a model now on disk. Switches to it first when its download
    /// asked to, so the download ends with the model already active.
    fn complete(app: &AppHandle, downloads: &Downloads, entry: &ModelEntry) {
        announce(Topic::Models);
        let id = entry.id.as_str();
        if downloads.activates(id) {
            if let Err(e) = Self::select(app, entry) {
                downloads.set_error(id, format!("Downloaded, but could not switch to it: {e}"));
                return;
            }
        }
        downloads.remove(id);
    }

    /// Delete a downloaded model. Deleting the active one switches to the best
    /// model left on disk, or to none. Fails with `ResourceBusy` while a
    /// recording is using the active model.
    pub fn delete(&self, app: &AppHandle, entry: &ModelEntry) -> std::io::Result<()> {
        let settings = app.state::<Settings>();
        let active = Self::active(&settings).id == entry.id;
        // A loaded model's file is memory-mapped, and Windows cannot delete a
        // mapped file: release the engine first.
        if active && !app.state::<EngineCache>().unload_unless_recording() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::ResourceBusy,
                "a recording is using the model",
            ));
        }
        std::fs::remove_file(self.path(entry))?;
        announce(Topic::Models);
        if !active {
            return Ok(());
        }
        match self.best_on_disk(Some(&entry.id)) {
            Some(next) => Self::select(app, next),
            None => settings.set_model(None),
        }
    }

    /// Fetch a model in the background. Beyond `MAX_CONCURRENT_DOWNLOADS` it
    /// waits in `Queued`. With `activate`, it becomes the active model only
    /// once on disk, so the choice never points at a missing file.
    pub fn start_download(&self, app: &AppHandle, entry: &'static ModelEntry, activate: bool) {
        // Already in flight: it keeps its entry and its progress.
        let Some(cancel) = self.downloads.enqueue(&entry.id, activate) else {
            return;
        };
        // Already on disk: the entry just queued ends at once.
        if self.is_downloaded(entry) {
            Self::complete(app, &self.downloads, entry);
            return;
        }

        let downloads = Arc::clone(&self.downloads);
        let models_dir = self.models_dir.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let id = entry.id.clone();
            let cancelled = |downloads: &Downloads| {
                downloads.remove(&id);
                let _ = ModelDownloadCancelled { id: id.clone() }.emit(&app);
            };
            let permit = tokio::select! {
                p = Arc::clone(&downloads.permits).acquire_owned() => p,
                () = cancel.cancelled() => return cancelled(&downloads),
            };
            let Ok(_permit) = permit else { return };

            downloads.set_running(&id);
            match download_model(&models_dir, entry, &downloads, cancel).await {
                Ok(()) => Self::complete(&app, &downloads, entry),
                Err(e) if e == CANCELLED => cancelled(&downloads),
                Err(error) => downloads.set_error(&id, error),
            }
        });
    }
}

#[cfg(test)]
#[path = "../../tests/unit/models/library.rs"]
mod tests;
