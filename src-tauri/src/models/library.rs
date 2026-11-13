//! Models on disk and the downloads that put them there.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio_util::sync::CancellationToken;

use crate::bridge::events::{
    ModelDownloadCancelled, ModelDownloadComplete, ModelDownloadError, ModelDownloadRunning,
    ModelDownloadStart, ModelEvicted, ModelSwitched,
};
use crate::settings::Settings;
use crate::sync::lock_recovering;

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
        Some(cancel)
    }

    pub fn activates(&self, id: &str) -> bool {
        self.lock().get(id).is_some_and(|e| e.activate)
    }

    pub fn set_running(&self, id: &str) {
        if let Some(e) = self.lock().get_mut(id) {
            e.status = DownloadStatus::Running;
        }
    }

    pub fn set_progress(&self, id: &str, pct: u8) {
        if let Some(e) = self.lock().get_mut(id) {
            e.progress = pct;
        }
    }

    pub fn set_error(&self, id: &str, msg: String) {
        if let Some(e) = self.lock().get_mut(id) {
            e.status = DownloadStatus::Error;
            e.error = Some(msg);
        }
    }

    pub fn remove(&self, id: &str) {
        self.lock().remove(id);
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
    /// asked to, so the complete event already finds it active.
    fn complete(app: &AppHandle, downloads: &Downloads, entry: &ModelEntry) {
        let id = entry.id.clone();
        if downloads.activates(&id) {
            if let Err(e) = Self::select(app, entry) {
                let error = format!("Downloaded, but could not switch to it: {e}");
                downloads.set_error(&id, error.clone());
                let _ = ModelDownloadError { id, error }.emit(app);
                return;
            }
            let _ = ModelSwitched.emit(app);
        }
        downloads.remove(&id);
        let _ = ModelDownloadComplete { id }.emit(app);
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
        if !active {
            return Ok(());
        }
        if let Some(next) = self.best_on_disk(Some(&entry.id)) {
            Self::select(app, next)?;
            let _ = ModelSwitched.emit(app);
        } else {
            settings.set_model(None)?;
            let _ = ModelEvicted.emit(app);
        }
        Ok(())
    }

    /// Fetch a model in the background. Beyond `MAX_CONCURRENT_DOWNLOADS` it
    /// waits in `Queued`. With `activate`, it becomes the active model only
    /// once on disk, so the choice never points at a missing file.
    pub fn start_download(&self, app: &AppHandle, entry: &'static ModelEntry, activate: bool) {
        let id = entry.id.clone();
        // Already in flight: re-emitting `start` would rewind the caller's
        // progress bar to 0.
        let Some(cancel) = self.downloads.enqueue(&id, activate) else {
            return;
        };
        // Already on disk: only the complete event clears the caller's
        // optimistic `queued`, so emit one rather than returning quietly.
        if self.is_downloaded(entry) {
            Self::complete(app, &self.downloads, entry);
            return;
        }

        let downloads = Arc::clone(&self.downloads);
        let models_dir = self.models_dir.clone();
        let app = app.clone();
        let _ = ModelDownloadStart { id: id.clone() }.emit(&app);

        tauri::async_runtime::spawn(async move {
            let permit = tokio::select! {
                p = Arc::clone(&downloads.permits).acquire_owned() => p,
                () = cancel.cancelled() => {
                    downloads.remove(&id);
                    let _ = ModelDownloadCancelled { id }.emit(&app);
                    return;
                }
            };
            let Ok(_permit) = permit else { return };

            downloads.set_running(&id);
            let _ = ModelDownloadRunning { id: id.clone() }.emit(&app);

            match download_model(&models_dir, entry, &app, &downloads, cancel).await {
                Ok(()) => Self::complete(&app, &downloads, entry),
                Err(e) if e == CANCELLED => {
                    downloads.remove(&id);
                    let _ = ModelDownloadCancelled { id }.emit(&app);
                }
                Err(error) => {
                    downloads.set_error(&id, error.clone());
                    let _ = ModelDownloadError { id, error }.emit(&app);
                }
            }
        });
    }
}

#[cfg(test)]
#[path = "../../tests/unit/models/library.rs"]
mod tests;
