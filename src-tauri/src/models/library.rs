//! Models on disk and the downloads that put them there.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio_util::sync::CancellationToken;

use crate::bridge::events::ModelDownloadCancelled;
use crate::mirror::{Committed, Patch, Slice, Store};
use crate::settings::Settings;

use super::cache::EngineCache;
use super::catalog::{self, ModelEntry};
use super::downloader::{download_model, CANCELLED};
use super::provider::{detect_backend, select_model};

/// How many models may transfer at once. Extras wait in `Queued` rather than
/// being refused, so the user can line several up and walk away.
pub const MAX_CONCURRENT_DOWNLOADS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DownloadStatus {
    Queued,
    Running,
    Error,
}

/// A model download as the windows see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Download {
    pub id: String,
    pub status: DownloadStatus,
    pub progress: u8,
    pub error: Option<String>,
}

#[derive(Clone)]
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

/// The cancel token is the entry's handle, not part of what it shows.
impl PartialEq for DownloadEntry {
    fn eq(&self, other: &Self) -> bool {
        self.status == other.status
            && self.progress == other.progress
            && self.error == other.error
            && self.activate == other.activate
    }
}

#[derive(Clone, Default, PartialEq)]
struct Entries(HashMap<String, DownloadEntry>);

impl Slice for Entries {
    fn patch(&self, rev: u64) -> Patch {
        let value = self
            .0
            .iter()
            .map(|(id, e)| Download {
                id: id.clone(),
                status: e.status,
                progress: e.progress,
                error: e.error.clone(),
            })
            .collect();
        Patch::Downloads { rev, value }
    }
}

/// Every download in flight or waiting, keyed by model id. Success and cancel
/// remove the entry; the file on disk is the truth for what is installed.
pub struct Downloads {
    entries: Store<Entries>,
    permits: Arc<tokio::sync::Semaphore>,
}

impl Downloads {
    pub fn new() -> Self {
        Self {
            entries: Store::new(Entries::default()),
            permits: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_DOWNLOADS)),
        }
    }

    fn edit(&self, id: &str, change: impl FnOnce(&mut DownloadEntry)) {
        self.entries.set(|entries| {
            if let Some(entry) = entries.0.get_mut(id) {
                change(entry);
            }
        });
    }

    /// Registers a queued download, returning its cancel token. A model already
    /// queued or running keeps its entry (gaining `activate` if asked) and
    /// returns `None`.
    pub fn enqueue(&self, id: &str, activate: bool) -> Option<CancellationToken> {
        let mut queued = None;
        self.entries.set(|entries| {
            if let Some(entry) = entries
                .0
                .get_mut(id)
                .filter(|e| e.status != DownloadStatus::Error)
            {
                entry.activate |= activate;
                return;
            }
            let cancel = CancellationToken::new();
            entries.0.insert(
                id.to_string(),
                DownloadEntry {
                    status: DownloadStatus::Queued,
                    progress: 0,
                    error: None,
                    cancel: cancel.clone(),
                    activate,
                },
            );
            queued = Some(cancel);
        });
        queued
    }

    pub fn activates(&self, id: &str) -> bool {
        self.entries.read(|e| e.0.get(id).is_some_and(|e| e.activate))
    }

    pub fn set_running(&self, id: &str) {
        self.edit(id, |e| e.status = DownloadStatus::Running);
    }

    pub fn set_progress(&self, id: &str, pct: u8) {
        self.edit(id, |e| e.progress = pct);
    }

    pub fn set_error(&self, id: &str, msg: String) {
        self.edit(id, |e| {
            e.status = DownloadStatus::Error;
            e.error = Some(msg);
        });
    }

    pub fn remove(&self, id: &str) {
        self.entries.set(|entries| {
            entries.0.remove(id);
        });
    }

    /// Trips the cancel token, if this model has an entry to cancel.
    pub fn cancel(&self, id: &str) -> bool {
        self.entries.read(|entries| match entries.0.get(id) {
            Some(e) if e.status != DownloadStatus::Error => {
                e.cancel.cancel();
                true
            }
            _ => false,
        })
    }

    pub fn committed(&self) -> Committed {
        self.entries.committed()
    }
}

/// A model on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedModel {
    pub variant: String,
    pub display_name: String,
    pub size_bytes: u64,
}

#[derive(Clone, PartialEq)]
struct OnDisk(Vec<DownloadedModel>);

impl OnDisk {
    fn scan(models_dir: &Path) -> Self {
        let found = catalog::all()
            .iter()
            .filter_map(|entry| {
                let size_bytes = models_dir.join(&entry.filename).metadata().ok()?.len();
                Some(DownloadedModel {
                    variant: entry.id.clone(),
                    display_name: entry.display_name.clone(),
                    size_bytes,
                })
            })
            .collect();
        Self(found)
    }
}

impl Slice for OnDisk {
    fn patch(&self, rev: u64) -> Patch {
        Patch::Models {
            rev,
            value: self.0.clone(),
        }
    }
}

/// The most capable model on disk other than `except`. The catalog is
/// tier-ascending, so the last match is the best one.
pub fn best_on_disk(models_dir: &Path, except: Option<&str>) -> Option<&'static ModelEntry> {
    catalog::all()
        .iter()
        .rfind(|m| Some(m.id.as_str()) != except && models_dir.join(&m.filename).exists())
}

/// The model files, and the downloads that add them. Which one is active is a
/// setting, kept by `Settings`.
pub struct ModelLibrary {
    models_dir: PathBuf,
    on_disk: Store<OnDisk>,
    downloads: Arc<Downloads>,
}

impl ModelLibrary {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            on_disk: Store::new(OnDisk::scan(&models_dir)),
            models_dir,
            downloads: Arc::new(Downloads::new()),
        }
    }

    /// Look at the model files again. Dev and release share them and neither
    /// hears the other, so a view that lists them asks for this as it opens.
    pub fn rescan(&self) -> Committed {
        let found = OnDisk::scan(&self.models_dir);
        self.on_disk.set(|on_disk| *on_disk = found)
    }

    /// The model recordings use: the saved choice, else the hardware pick.
    pub fn active(settings: &Settings) -> &'static ModelEntry {
        Self::active_for(settings.model().as_deref())
    }

    /// The model a `saved` choice resolves to on this hardware.
    pub fn active_for(saved: Option<&str>) -> &'static ModelEntry {
        select_model(detect_backend(), saved)
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

    pub fn cancel_download(&self, id: &str) {
        self.downloads.cancel(id);
    }

    /// Make `entry` the active model and warm it in the background, so the
    /// first recording after a switch isn't stalled by the load.
    pub fn select(app: &AppHandle, entry: &ModelEntry) -> std::io::Result<Committed> {
        let committed = app.state::<Settings>().set_model(Some(entry.id.clone()))?;
        app.state::<EngineCache>().unload();
        EngineCache::warm(app);
        Ok(committed)
    }

    /// Publish a model now on disk. Switches to it first when its download
    /// asked to, so the download ends with the model already active.
    fn complete(app: &AppHandle, downloads: &Downloads, entry: &ModelEntry) {
        app.state::<Self>().rescan();
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
    pub fn delete(&self, app: &AppHandle, entry: &ModelEntry) -> std::io::Result<Committed> {
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
        let committed = self.rescan();
        if active {
            match self.best_on_disk(Some(&entry.id)) {
                Some(next) => Self::select(app, next)?,
                None => settings.set_model(None)?,
            };
        }
        Ok(committed)
    }

    /// Fetch a model in the background. Beyond `MAX_CONCURRENT_DOWNLOADS` it
    /// waits in `Queued`. With `activate`, it becomes the active model only
    /// once on disk, so the choice never points at a missing file. Returns the
    /// download list's revision once the download is queued or done.
    pub fn start_download(
        &self,
        app: &AppHandle,
        entry: &'static ModelEntry,
        activate: bool,
    ) -> Committed {
        // Already in flight: it keeps its entry and its progress.
        let Some(cancel) = self.downloads.enqueue(&entry.id, activate) else {
            return self.downloads.committed();
        };
        // Already on disk: the entry just queued ends at once.
        if self.is_downloaded(entry) {
            Self::complete(app, &self.downloads, entry);
            return self.downloads.committed();
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
        self.downloads.committed()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/models/library.rs"]
mod tests;
