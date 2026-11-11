//! The loaded speech engine: built once, reused across recordings, unloaded
//! when the model changes or it idles.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::watch;

use crate::bridge::events::LanguageReset;
use crate::settings::Settings;
use crate::sync::lock_recovering;

use super::language::{primary_of, AUTO, DEFAULT};
use super::library::best_on_disk;
use super::provider::{detect_backend, select_model};
use super::TranscriptionEngine;

pub type SharedEngine = Arc<Mutex<TranscriptionEngine>>;

/// Generic over the engine only so tests can install one without a model.
struct Slot<E = SharedEngine> {
    engine: Option<E>,
    /// Bumped by every unload; a load begun under an older value is stale.
    generation: u64,
    /// Each recording using the engine, by the language it started in. Unloading
    /// under one would hand its finalize a different model.
    leases: Vec<Option<String>>,
    /// Only ever set while an engine is loaded.
    unload_pending: bool,
    /// The saved language.
    language: Option<String>,
}

// Derived, it would require `E: Default`, which an engine is not.
impl<E> Default for Slot<E> {
    fn default() -> Self {
        Self {
            engine: None,
            generation: 0,
            leases: Vec::new(),
            unload_pending: false,
            language: None,
        }
    }
}

/// What an unload request did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unload {
    Released,
    NothingLoaded,
    /// A recording holds the engine; it is released when the recording ends.
    Deferred,
}

/// Changes waiting for the recordings in progress to finish.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Deferred {
    pub model: bool,
    pub language: bool,
}

impl<E> Slot<E> {
    /// A recording in `language` starts using the engine.
    fn lease(&mut self, language: Option<String>) {
        self.language.clone_from(&language);
        self.leases.push(language);
    }

    /// A recording in `language` ended; applies an unload that waited on it.
    fn release(&mut self, language: Option<&str>) {
        if let Some(i) = self.leases.iter().position(|l| l.as_deref() == language) {
            self.leases.swap_remove(i);
        }
        if self.unload_pending && self.unload() == Unload::Released {
            log::info!("unloaded the engine the finished recording was using");
        }
    }

    fn deferred(&self) -> Deferred {
        Deferred {
            model: self.unload_pending,
            language: self.leases.iter().any(|l| *l != self.language),
        }
    }

    /// Unload, and void any load in flight: it may be building the old model.
    fn supersede(&mut self) -> Unload {
        self.generation += 1;
        self.unload()
    }

    /// Install `engine` unless the slot was superseded since its load began.
    fn install(&mut self, engine: E, generation: u64) -> bool {
        if self.generation != generation {
            return false;
        }
        self.engine = Some(engine);
        true
    }

    /// Deferred only while a recording holds a loaded engine.
    fn unload(&mut self) -> Unload {
        if self.engine.is_some() && !self.leases.is_empty() {
            self.unload_pending = true;
            return Unload::Deferred;
        }
        match self.clear() {
            Some(_) => Unload::Released,
            None => Unload::NothingLoaded,
        }
    }

    /// Drop the engine, and the unload that waited on it.
    fn clear(&mut self) -> Option<E> {
        self.unload_pending = false;
        self.engine.take()
    }

    /// `supersede`, but refused, with nothing queued, while a recording holds the engine.
    fn supersede_unless_leased(&mut self) -> bool {
        if !self.leases.is_empty() {
            return false;
        }
        self.supersede();
        true
    }
}

pub struct EngineCache {
    models_dir: PathBuf,
    slot: Arc<Mutex<Slot>>,
    /// Serializes loading, so two callers racing an empty slot build one engine.
    loading: tokio::sync::Mutex<()>,
    used_at: Mutex<Instant>,
    /// Copied out at load: the engine mutex is held for a whole streaming
    /// recording.
    backend: RwLock<Option<String>>,
    /// Signalled when a lease drops.
    released: Arc<tokio::sync::Notify>,
    deferred: Arc<watch::Sender<Deferred>>,
}

/// Holds the engine for one recording, which keeps the language it started
/// with. Dropping it applies an unload requested meanwhile.
pub struct EngineLease {
    slot: Arc<Mutex<Slot>>,
    released: Arc<tokio::sync::Notify>,
    deferred: Arc<watch::Sender<Deferred>>,
    language: Option<String>,
}

impl EngineLease {
    /// The saved language when the recording started.
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }
}

impl Drop for EngineLease {
    fn drop(&mut self) {
        let language = self.language.as_deref();
        edit(&self.slot, &self.deferred, |slot| slot.release(language));
        self.released.notify_waiters();
    }
}

/// Change the slot, and announce it when that changed what recordings defer.
fn edit<R>(
    slot: &Mutex<Slot>,
    deferred: &watch::Sender<Deferred>,
    change: impl FnOnce(&mut Slot) -> R,
) -> R {
    let mut slot = lock_recovering(slot);
    let result = change(&mut slot);
    let now = slot.deferred();
    deferred.send_if_modified(|seen| std::mem::replace(seen, now) != now);
    result
}

impl EngineCache {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            models_dir,
            slot: Arc::default(),
            loading: tokio::sync::Mutex::new(()),
            used_at: Mutex::new(Instant::now()),
            backend: RwLock::new(None),
            released: Arc::new(tokio::sync::Notify::new()),
            deferred: Arc::new(watch::Sender::new(Deferred::default())),
        }
    }

    /// Hold the engine for a recording in the saved language.
    pub fn lease(&self, settings: &Settings) -> EngineLease {
        // Under the slot lock, like `apply_language`, so a save can't land in between.
        let language = self.edit(|slot| {
            let language = settings.language();
            slot.lease(language.clone());
            language
        });
        EngineLease {
            slot: Arc::clone(&self.slot),
            released: Arc::clone(&self.released),
            deferred: Arc::clone(&self.deferred),
            language,
        }
    }

    /// What the recordings in progress keep from before a change.
    pub fn deferred(&self) -> Deferred {
        lock_recovering(&self.slot).deferred()
    }

    /// Wakes whenever `deferred` changes.
    pub fn watch_deferred(&self) -> watch::Receiver<Deferred> {
        self.deferred.subscribe()
    }

    fn edit<R>(&self, change: impl FnOnce(&mut Slot) -> R) -> R {
        edit(&self.slot, &self.deferred, change)
    }

    /// Returns once no unload waits on a recording. Until then the slot still
    /// holds the superseded engine, and a load would just return it.
    async fn superseded_released(&self) {
        loop {
            let released = self.released.notified();
            tokio::pin!(released);
            // Registered before the check, so a lease dropping in between still wakes us.
            released.as_mut().enable();
            if !lock_recovering(&self.slot).unload_pending {
                return;
            }
            released.await;
        }
    }

    /// The engine if one is loaded; never starts a load.
    pub fn loaded(&self) -> Option<SharedEngine> {
        lock_recovering(&self.slot).engine.clone()
    }

    /// Get the engine, loading the selected model if none is. Fails when no
    /// model is on disk.
    ///
    /// Load, GPU init and warmup take seconds, so they run on a blocking
    /// thread with no lock held; blocking the runtime would stall every
    /// command.
    /// A model switch mid-load drops the stale engine and loads the new pick.
    pub async fn get_or_load(&self, settings: &Settings) -> Result<SharedEngine, String> {
        self.touch();
        if let Some(engine) = self.loaded() {
            return Ok(engine);
        }
        let _loading = self.loading.lock().await;
        loop {
            let generation = match self.loaded_or_generation() {
                Ok(engine) => return Ok(engine),
                Err(generation) => generation,
            };
            let engine = self.build(settings).await?;
            let backend = engine.backend();
            let languages = engine.languages().to_vec();
            let engine = Arc::new(Mutex::new(engine));
            if !self.edit(|slot| slot.install(Arc::clone(&engine), generation)) {
                log::info!("model changed while loading; loading the new selection");
                continue;
            }
            reset_unsupported_language(settings, &languages, settings.language().as_deref());
            *self
                .backend
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(backend);
            return Ok(engine);
        }
    }

    /// The loaded engine, or the generation a load starting now runs under.
    fn loaded_or_generation(&self) -> Result<SharedEngine, u64> {
        let slot = lock_recovering(&self.slot);
        slot.engine.clone().ok_or(slot.generation)
    }

    /// Build the selected model's engine off the runtime.
    async fn build(&self, settings: &Settings) -> Result<TranscriptionEngine, String> {
        let mut model = settings.model();
        let entry = select_model(detect_backend(), model.as_deref());
        // The selected model may still be downloading. Load whatever is present
        // so recording keeps working, but never rewrite the choice; that would
        // silently cancel the user's pick mid-download.
        if !self.models_dir.join(&entry.filename).exists() {
            let fallback = best_on_disk(&self.models_dir, None)
                .ok_or_else(|| "model not downloaded yet".to_string())?;
            log::warn!(
                "selected model not on disk; loading {} for now",
                fallback.id
            );
            model = Some(fallback.id.clone());
        }

        let models_dir = self.models_dir.clone();
        tauri::async_runtime::spawn_blocking(move || {
            TranscriptionEngine::new(&models_dir, model.as_deref())
        })
        .await
        .map_err(|e| {
            log::error!("engine load task failed: {e}");
            "Loading the model failed. Please try again.".to_string()
        })?
    }

    /// Load the selected model in the background. A missing model is not an
    /// error here; recording surfaces that. A mid-recording switch warms once
    /// that recording lets go.
    pub fn warm(app: &AppHandle) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let cache = app.state::<Self>();
            cache.superseded_released().await;
            let settings = app.state::<Settings>();
            let before = settings.language();
            if let Err(e) = cache.get_or_load(&settings).await {
                log::info!("engine warmup skipped: {e}");
                return;
            }
            // The load resets a language the new model can't speak.
            if before.is_some() && settings.language() != before {
                let _ = LanguageReset.emit(&app);
            }
        });
    }

    /// Note a newly saved language; recordings in progress keep theirs.
    pub fn apply_language(&self, settings: &Settings) {
        // Read under the slot lock, so concurrent saves apply in order.
        self.edit(|slot| slot.language = settings.language());
    }

    /// Unload now, or once the recordings holding the engine finish. A load in
    /// flight is superseded.
    pub fn unload(&self) -> Unload {
        self.edit(Slot::supersede)
    }

    /// Unload now, or refuse while a recording holds the engine; never queues.
    pub fn unload_unless_recording(&self) -> bool {
        self.edit(Slot::supersede_unless_leased)
    }

    /// Drop an engine that panicked, even mid-recording: it cannot be trusted.
    pub fn discard(&self) {
        self.edit(|slot| {
            slot.clear();
        });
    }

    pub fn in_use(&self) -> bool {
        !lock_recovering(&self.slot).leases.is_empty()
    }

    /// Defer an idle unload: the engine is in use now.
    pub fn touch(&self) {
        *lock_recovering(&self.used_at) = Instant::now();
    }

    pub fn idle_for(&self) -> Duration {
        lock_recovering(&self.used_at).elapsed()
    }

    /// Backend the last loaded model bound to; `None` before the first load.
    pub fn backend(&self) -> Option<String> {
        self.backend
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// Otherwise every decode silently falls back to English and the picker still
/// shows a language the model cannot speak.
fn reset_unsupported_language(settings: &Settings, languages: &[String], saved: Option<&str>) {
    let Some(code) = saved.filter(|c| *c != AUTO && *c != DEFAULT) else {
        return;
    };
    if languages.is_empty() || languages.iter().any(|l| l == code || primary_of(l) == code) {
        return;
    }
    if let Err(e) = settings.set_language(Some(DEFAULT.to_string())) {
        log::warn!("could not reset unsupported language {code}: {e}");
    }
}

#[cfg(test)]
#[path = "../../tests/unit/models/cache.rs"]
mod tests;
