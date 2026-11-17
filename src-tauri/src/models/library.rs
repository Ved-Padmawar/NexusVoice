//! Models on disk and the downloads that put them there.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio_util::sync::CancellationToken;

use crate::bridge::events::{LanguageReset, ModelDownloadCancelled};
use crate::mirror::{self, Committed, Patch, Slice, Store};
use crate::settings::Settings;
use crate::sync::lock_recovering;

use super::cache::EngineCache;
use super::catalog::{self, ModelEntry};
use super::downloader::{download_model, CANCELLED};

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
    cancel: Arc<CancellationToken>,
    /// Make the model the active one once it is on disk.
    activate: bool,
}

/// Two entries are the same download only with the same token.
impl PartialEq for DownloadEntry {
    fn eq(&self, other: &Self) -> bool {
        self.status == other.status
            && self.progress == other.progress
            && self.error == other.error
            && self.activate == other.activate
            && Arc::ptr_eq(&self.cancel, &other.cancel)
    }
}

#[derive(Clone, Default, PartialEq)]
struct Entries(HashMap<String, DownloadEntry>);

impl Slice for Entries {
    fn patch(&self) -> Patch {
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
        Patch::Downloads(value)
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
                    cancel: Arc::new(cancel.clone()),
                    activate,
                },
            );
            queued = Some(cancel);
        });
        queued
    }

    pub fn activates(&self, id: &str) -> bool {
        self.entries
            .read(|e| e.0.get(id).is_some_and(|e| e.activate))
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
    fn patch(&self) -> Patch {
        Patch::Models(self.0.clone())
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
    changing: Mutex<()>,
}

impl ModelLibrary {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            on_disk: Store::new(OnDisk::scan(&models_dir)),
            models_dir,
            downloads: Arc::new(Downloads::new()),
            changing: Mutex::default(),
        }
    }

    /// Look at the model files again. Dev and release share them and neither
    /// hears the other, so a view that lists them asks for this as it opens.
    pub fn rescan(&self) -> Committed {
        self.on_disk
            .set(|on_disk| *on_disk = OnDisk::scan(&self.models_dir))
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
    pub fn select(
        &self,
        app: &AppHandle,
        entry: &ModelEntry,
    ) -> std::io::Result<Option<Committed>> {
        let _changing = lock_recovering(&self.changing);
        if !self.is_downloaded(entry) {
            return Ok(None);
        }
        let (committed, reset) = app.state::<Settings>().select_model(Some(entry))?;
        Self::load_selected(app, reset);
        Ok(Some(committed))
    }

    /// Swap the engine for the model just selected. `reset`: the selection
    /// also reset the language, which the new model doesn't offer.
    fn load_selected(app: &AppHandle, reset: bool) {
        let cache = app.state::<EngineCache>();
        cache.unload();
        if reset {
            cache.apply_language(&app.state::<Settings>());
            let _ = LanguageReset.emit(app);
        }
        EngineCache::warm(app);
    }

    /// Publish a model now on disk in one commit with the end of its download
    /// and the switch it asked for, so no window sees one without the others.
    fn complete(app: &AppHandle, downloads: &Downloads, entry: &ModelEntry) {
        let library = app.state::<Self>();
        let settings = app.state::<Settings>();
        let _changing = lock_recovering(&library.changing);
        let id = entry.id.as_str();
        let activate = downloads.activates(id);
        let completed = mirror::atomic(|| {
            library.rescan();
            if !library.is_downloaded(entry) {
                return Err("The downloaded file is no longer available.".to_string());
            }
            let reset = if activate {
                let (_, reset) = settings
                    .select_model(Some(entry))
                    .map_err(|e| format!("Downloaded, but could not switch to it: {e}"))?;
                Some(reset)
            } else {
                None
            };
            downloads.remove(id);
            Ok(reset)
        });
        match completed {
            Ok(Some(reset)) => Self::load_selected(app, reset),
            Ok(None) if settings.active_model().id == id => {
                // A temporary fallback may be loaded, or still loading.
                app.state::<EngineCache>().unload();
                EngineCache::warm(app);
            }
            Ok(None) => {}
            Err(message) => downloads.set_error(id, message),
        }
    }

    /// Delete a downloaded model. Deleting the active one switches to the best
    /// model left on disk, or to none, in the same commit as the model list.
    /// Fails with `ResourceBusy` while a recording is using the active model.
    pub fn delete(&self, app: &AppHandle, entry: &ModelEntry) -> std::io::Result<Committed> {
        let _changing = lock_recovering(&self.changing);
        let settings = app.state::<Settings>();
        let active = settings.active_model().id == entry.id;
        // A loaded model's file is memory-mapped, and Windows cannot delete a
        // mapped file: release the engine first.
        if active && !app.state::<EngineCache>().unload_unless_recording() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::ResourceBusy,
                "a recording is using the model",
            ));
        }
        std::fs::remove_file(self.path(entry))?;
        let (committed, reset) = mirror::atomic(|| {
            let committed = self.rescan();
            if !active {
                return Ok((committed, None));
            }
            let next = self.best_on_disk(Some(&entry.id));
            let (_, reset) = settings.select_model(next)?;
            Ok::<_, std::io::Error>((committed, next.map(|_| reset)))
        })?;
        if let Some(reset) = reset {
            Self::load_selected(app, reset);
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
                Ok(()) => {
                    let completed_downloads = Arc::clone(&downloads);
                    if let Err(e) = tauri::async_runtime::spawn_blocking(move || {
                        Self::complete(&app, &completed_downloads, entry);
                    })
                    .await
                    {
                        log::error!("could not publish the downloaded model: {e}");
                        downloads.set_error(
                            &id,
                            "Downloaded, but could not refresh the model list.".to_string(),
                        );
                    }
                }
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
