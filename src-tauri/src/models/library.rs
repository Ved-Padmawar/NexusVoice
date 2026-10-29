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

    pub fn is_pending(&self, id: &str) -> bool {
        self.lock()
            .get(id)
            .is_some_and(|e| e.status != DownloadStatus::Error)
    }

    /// Registers a queued download, returning its cancel token. A model already
    /// queued or running keeps its existing entry and returns `None`.
    pub fn enqueue(&self, id: &str) -> Option<CancellationToken> {
        let mut entries = self.lock();
        if entries
            .get(id)
            .is_some_and(|e| e.status != DownloadStatus::Error)
        {
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
            },
        );
        Some(cancel)
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

    pub fn path(&self, entry: &ModelEntry) -> PathBuf {
        self.models_dir.join(&entry.filename)
    }

    pub fn is_downloaded(&self, entry: &ModelEntry) -> bool {
        self.path(entry).exists()
    }

    pub fn best_on_disk(&self, except: Option<&str>) -> Option<&'static ModelEntry> {
        best_on_disk(&self.models_dir, except)
    }

    pub fn is_downloading(&self, id: &str) -> bool {
        self.downloads.is_pending(id)
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

    /// Delete a downloaded model. Deleting the active one switches to the best
    /// model left on disk, or to none.
    pub fn delete(&self, app: &AppHandle, entry: &ModelEntry) -> std::io::Result<()> {
        std::fs::remove_file(self.path(entry))?;
        let settings = app.state::<Settings>();
        if Self::active(&settings).id != entry.id {
            return Ok(());
        }
        if let Some(next) = self.best_on_disk(Some(&entry.id)) {
            Self::select(app, next)?;
            let _ = ModelSwitched.emit(app);
        } else {
            settings.set_model(None)?;
            app.state::<EngineCache>().unload();
            let _ = ModelEvicted.emit(app);
        }
        Ok(())
    }

    /// Fetch a model in the background. Never changes the active model, so a
    /// failed or cancelled download can't leave the choice pointing at a
    /// missing file. Beyond `MAX_CONCURRENT_DOWNLOADS` it waits in `Queued`.
    pub fn start_download(&self, app: &AppHandle, entry: &'static ModelEntry) {
        let id = entry.id.clone();
        // Already on disk: only the complete event clears the caller's
        // optimistic `queued`, so emit one rather than returning quietly.
        if self.is_downloaded(entry) {
            let _ = ModelDownloadComplete { id }.emit(app);
            return;
        }
        // Already in flight: re-emitting `start` would rewind the caller's
        // progress bar to 0.
        let Some(cancel) = self.downloads.enqueue(&id) else {
            return;
        };

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
                Ok(()) => {
                    downloads.remove(&id);
                    let _ = ModelDownloadComplete { id }.emit(&app);
                }
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
