//! App-wide state the main window shares between pages: startup, the model
//! catalog and downloads, the CUDA pack and the pending update.
//!
//! One entity, held in a global. Pages observe it and re-render on change;
//! backend events land here through [`handle`]. Page-local data (the feed, the
//! dictionary) stays with the page that shows it, which subscribes to
//! [`StoreEvent`] for the changes it must reflect.

use std::collections::BTreeMap;
use std::time::Duration;

use gpui_kit::{App, AppContext as _, Context, Entity, EventEmitter, Global, SharedString};

use crate::api::{self, dto::TranscriptResponse, CatalogModel, CudaStatus, ModelInfoResponse};
use crate::events::{CudaDownload, Event, ModelDownload};
use crate::inference::cuda::PackState;
use crate::platform::updater;

use super::runtime::SpawnApi as _;
use super::settings::UiSettings;

/// Lets the dashboard show before the CUDA offer opens.
const CUDA_OFFER_DELAY: Duration = Duration::from_millis(1200);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Startup {
    Starting,
    Ready,
    Failed(SharedString),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStatus {
    Queued,
    Running,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub status: DownloadStatus,
    pub progress: u8,
    pub error: Option<SharedString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    Available,
    Downloading,
    /// Installed; a restart finishes it.
    Ready,
    UpToDate,
    Failed,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateState {
    pub status: UpdateStatus,
    pub version: Option<SharedString>,
    pub progress: u8,
    pub error: Option<SharedString>,
    /// Hidden from the sidebar for this session; About still shows it.
    pub dismissed: bool,
    pending: Option<updater::Update>,
}

/// A message for the user, shown as a toast by the main window.
#[derive(Debug, Clone)]
pub enum Notice {
    Info(SharedString),
    Success(SharedString),
    Error(SharedString),
    /// CUDA finished installing; offers a restart.
    CudaReady,
}

#[derive(Debug, Clone)]
pub enum StoreEvent {
    TranscriptSaved(TranscriptResponse),
    Notice(Notice),
}

pub struct Store {
    pub startup: Startup,
    /// The model catalog, ascending by capability. Empty until loaded.
    pub catalog: Vec<CatalogModel>,
    pub model_info: Option<ModelInfoResponse>,
    /// Every download queued, running or failed, by model id.
    pub downloads: BTreeMap<String, Download>,
    /// `None` until first fetched.
    pub cuda: Option<CudaStatus>,
    pub cuda_error: Option<SharedString>,
    pub cuda_offer_open: bool,
    pub update: UpdateState,
}

impl EventEmitter<StoreEvent> for Store {}

struct StoreHandle(Entity<Store>);

impl Global for StoreHandle {}

impl Store {
    pub fn entity(cx: &App) -> Entity<Self> {
        cx.global::<StoreHandle>().0.clone()
    }

    fn notify_user(&self, notice: Notice, cx: &mut Context<Self>) {
        cx.emit(StoreEvent::Notice(notice));
    }

    fn startup(&mut self, cx: &mut Context<Self>) {
        cx.spawn_api(api::wait_until_ready(), |store, result, cx| {
            store.startup = match result {
                Ok(()) => Startup::Ready,
                Err(e) => {
                    log::error!("startup failed: {e}");
                    Startup::Failed(e.message().to_string().into())
                }
            };
            cx.notify();
        });
        self.refresh_models(cx);
        self.refresh_downloads(cx);
        self.refresh_cuda(cx);

        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            let _ = this.update(cx, |store, cx| store.check_for_update(cx));
        })
        .detach();
    }

    // ── Models ──────────────────────────────────────────────────────────────

    pub fn refresh_models(&mut self, cx: &mut Context<Self>) {
        cx.spawn_api(
            async { (api::get_model_catalog(), api::get_model_info()) },
            |store, (catalog, info), cx| {
                store.catalog = catalog;
                store.model_info = Some(info);
                cx.notify();
            },
        );
    }

    pub fn refresh_downloads(&mut self, cx: &mut Context<Self>) {
        cx.spawn_api(
            async { api::get_active_downloads() },
            |store, active, cx| {
                store.downloads = active
                    .into_iter()
                    .map(|d| {
                        let status = match d.status.as_str() {
                            "running" => DownloadStatus::Running,
                            "error" => DownloadStatus::Failed,
                            _ => DownloadStatus::Queued,
                        };
                        let download = Download {
                            status,
                            progress: d.progress,
                            error: d.error.map(Into::into),
                        };
                        (d.id, download)
                    })
                    .collect();
                cx.notify();
            },
        );
    }

    pub fn start_download(&mut self, id: &str, cx: &mut Context<Self>) {
        self.downloads.insert(
            id.to_string(),
            Download {
                status: DownloadStatus::Queued,
                progress: 0,
                error: None,
            },
        );
        cx.notify();
        let model = id.to_string();
        cx.spawn_api(async move { api::start_model_download(model) }, {
            let id = id.to_string();
            move |store, result, cx| {
                if let Err(e) = result {
                    store.downloads.remove(&id);
                    store.notify_user(Notice::Error(e.message().to_string().into()), cx);
                    cx.notify();
                }
            }
        });
    }

    pub fn cancel_download(&mut self, id: &str, _cx: &mut Context<Self>) {
        api::cancel_model_download(id);
    }

    /// The catalog entry the backend will load, if the catalog is in.
    pub fn active_model(&self) -> Option<&CatalogModel> {
        self.catalog.iter().find(|m| m.is_active)
    }

    fn on_download(&mut self, id: String, status: ModelDownload, cx: &mut Context<Self>) {
        let entry = || Download {
            status: DownloadStatus::Queued,
            progress: 0,
            error: None,
        };
        match status {
            ModelDownload::Queued => {
                self.downloads.insert(id, entry());
            }
            ModelDownload::Running => {
                self.downloads.entry(id).or_insert_with(entry).status = DownloadStatus::Running;
            }
            ModelDownload::Progress(pct) => {
                let download = self.downloads.entry(id).or_insert_with(entry);
                download.status = DownloadStatus::Running;
                download.progress = pct;
            }
            ModelDownload::Complete => {
                self.downloads.remove(&id);
                self.refresh_models(cx);
            }
            ModelDownload::Cancelled => {
                self.downloads.remove(&id);
            }
            ModelDownload::Failed(error) => {
                let download = self.downloads.entry(id).or_insert_with(entry);
                download.status = DownloadStatus::Failed;
                download.error = Some(if error.is_empty() {
                    "Download failed".into()
                } else {
                    error.into()
                });
            }
        }
        cx.notify();
    }

    // ── CUDA ────────────────────────────────────────────────────────────────

    pub fn refresh_cuda(&mut self, cx: &mut Context<Self>) {
        cx.spawn_api(async { api::get_cuda_status() }, |store, cuda, cx| {
            // CUDA added from About counts too: removing it later must not
            // re-offer it.
            if cuda.pack != PackState::Absent {
                UiSettings::update(cx, |s| s.cuda_offer_seen = true);
            }
            store.cuda = Some(cuda);
            cx.notify();
            store.offer_cuda(cx);
        });
    }

    pub fn download_cuda(&mut self, cx: &mut Context<Self>) {
        let Some(cuda) = self.cuda.as_mut() else {
            return;
        };
        cuda.download_progress = Some(0);
        self.cuda_error = None;
        cx.notify();
        cx.spawn_api(async { api::start_cuda_download() }, |store, result, cx| {
            if let Err(e) = result {
                store.cuda_error = Some(e.message().to_string().into());
                store.refresh_cuda(cx);
            }
        });
    }

    pub fn cancel_cuda_download(&mut self, _cx: &mut Context<Self>) {
        api::cancel_cuda_download();
    }

    pub fn remove_cuda(&mut self, cx: &mut Context<Self>) {
        self.cuda_error = api::remove_cuda_pack()
            .err()
            .map(|e| e.message().to_string().into());
        self.refresh_cuda(cx);
    }

    /// Offer CUDA once: a model chosen, an NVIDIA GPU, and no pack yet.
    /// Re-checked when the delay ends, since the picker or a download may
    /// have moved on.
    pub fn offer_cuda(&mut self, cx: &mut Context<Self>) {
        fn due(store: &Store, cx: &App) -> bool {
            let settings = UiSettings::global(cx);
            offer_due(
                settings.model_chosen,
                settings.cuda_offer_seen,
                store.cuda_offer_open,
                store.cuda,
            )
        }
        if !due(self, cx) {
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(CUDA_OFFER_DELAY).await;
            let _ = this.update(cx, |store, cx| {
                if due(store, cx) {
                    store.cuda_offer_open = true;
                    store.cuda_error = None;
                    UiSettings::update(cx, |s| s.cuda_offer_seen = true);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn close_cuda_offer(&mut self, cx: &mut Context<Self>) {
        self.cuda_offer_open = false;
        cx.notify();
    }

    fn on_cuda(&mut self, status: CudaDownload, cx: &mut Context<Self>) {
        match status {
            CudaDownload::Progress(pct) => {
                if let Some(cuda) = self.cuda.as_mut() {
                    cuda.download_progress = Some(pct);
                    cx.notify();
                }
            }
            CudaDownload::Complete => {
                self.refresh_cuda(cx);
                // The open dialog shows its own Restart.
                if !self.cuda_offer_open {
                    self.notify_user(Notice::CudaReady, cx);
                }
            }
            CudaDownload::Cancelled => self.refresh_cuda(cx),
            CudaDownload::Failed(error) => {
                self.cuda_error = Some(if error.is_empty() {
                    "Download failed".into()
                } else {
                    error.into()
                });
                self.refresh_cuda(cx);
            }
        }
    }

    // ── Updates ─────────────────────────────────────────────────────────────

    pub fn check_for_update(&mut self, cx: &mut Context<Self>) {
        self.update.status = UpdateStatus::Checking;
        self.update.error = None;
        cx.notify();
        cx.spawn_api(updater::check(), |store, result, cx| {
            let update = &mut store.update;
            match result {
                Ok(Some(found)) => {
                    update.status = UpdateStatus::Available;
                    update.version = Some(found.version.clone().into());
                    update.pending = Some(found);
                }
                Ok(None) => {
                    update.status = UpdateStatus::UpToDate;
                    update.version = None;
                }
                Err(e) => {
                    update.status = UpdateStatus::Failed;
                    update.error = Some(e.into());
                }
            }
            cx.notify();
        });
    }

    pub fn install_update(&mut self, cx: &mut Context<Self>) {
        if self.update.status == UpdateStatus::Downloading {
            return;
        }
        let Some(pending) = self.update.pending.clone() else {
            return;
        };
        self.update.status = UpdateStatus::Downloading;
        self.update.progress = 0;
        self.update.error = None;
        cx.notify();

        let (progress_tx, progress_rx) = async_channel::unbounded();
        cx.spawn(async move |this, cx| {
            while let Ok(pct) = progress_rx.recv().await {
                let _ = this.update(cx, |store, cx| {
                    store.update.progress = pct;
                    cx.notify();
                });
            }
        })
        .detach();
        cx.spawn_api(
            async move {
                updater::install(&pending, |pct| {
                    let _ = progress_tx.try_send(pct);
                })
                .await
            },
            |store, result, cx| {
                match result {
                    Ok(()) => {
                        store.update.status = UpdateStatus::Ready;
                        store.update.progress = 100;
                        // The Windows installer replaces and restarts the app.
                        if cfg!(windows) {
                            cx.quit();
                        }
                    }
                    Err(e) => {
                        store.update.status = UpdateStatus::Failed;
                        store.update.error = Some(e.into());
                    }
                }
                cx.notify();
            },
        );
    }

    pub fn restart(&mut self, cx: &mut Context<Self>) {
        match updater::relaunch() {
            Ok(()) => cx.quit(),
            Err(e) => {
                self.update.status = UpdateStatus::Failed;
                self.update.error = Some(format!("Restart failed: {e}").into());
                cx.notify();
            }
        }
    }

    pub fn dismiss_update(&mut self, cx: &mut Context<Self>) {
        self.update.dismissed = true;
        cx.notify();
    }
}

/// Whether the CUDA offer should open: a model chosen, an NVIDIA GPU, no pack,
/// no download already under way, and the offer neither open nor used up.
fn offer_due(model_chosen: bool, seen: bool, open: bool, cuda: Option<CudaStatus>) -> bool {
    model_chosen
        && !seen
        && !open
        && cuda.is_some_and(|c| {
            c.supported && c.pack == PackState::Absent && c.download_progress.is_none()
        })
}

pub fn init(cx: &mut App) {
    let store = cx.new(|cx| {
        let mut store = Store {
            startup: Startup::Starting,
            catalog: Vec::new(),
            model_info: None,
            downloads: BTreeMap::new(),
            cuda: None,
            cuda_error: None,
            cuda_offer_open: false,
            update: UpdateState::default(),
        };
        store.startup(cx);
        store
    });
    cx.set_global(StoreHandle(store));
}

pub fn handle(event: Event, cx: &mut App) {
    Store::entity(cx).update(cx, |store, cx| match event {
        Event::ModelDownload { id, status } => store.on_download(id, status, cx),
        Event::ModelSwitched => store.refresh_models(cx),
        // The active model was deleted: nothing is selected until another is.
        Event::ModelEvicted => {
            if let Some(info) = store.model_info.as_mut() {
                info.downloaded = false;
            }
            store.refresh_models(cx);
        }
        Event::LanguageReset => store.notify_user(
            Notice::Info(
                "This model does not support your dictation language. Switched to English.".into(),
            ),
            cx,
        ),
        Event::Cuda(status) => store.on_cuda(status, cx),
        Event::TranscriptSaved(transcript) => cx.emit(StoreEvent::TranscriptSaved(transcript)),
        _ => {}
    });
}

#[cfg(test)]
#[path = "../../tests/unit/ui/store.rs"]
mod tests;
