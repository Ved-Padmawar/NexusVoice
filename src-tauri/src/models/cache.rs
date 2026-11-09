//! The loaded speech engine: built once, reused across recordings, unloaded
//! when the model changes or it idles.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::bridge::events::LanguageReset;
use crate::settings::Settings;
use crate::sync::lock_recovering;

use super::language::{self, primary_of, AUTO, DEFAULT};
use super::library::best_on_disk;
use super::provider::{detect_backend, select_model};
use super::TranscriptionEngine;

pub type SharedEngine = Arc<Mutex<TranscriptionEngine>>;

/// Generic over the engine only so tests can install one without a model.
struct Slot<E = SharedEngine> {
    engine: Option<E>,
    /// Bumped by every unload; a load begun under an older value is stale.
    generation: u64,
    /// Recordings still using the engine. Unloading under one would hand its
    /// finalize a different model than the one that decoded its partials.
    leases: usize,
    unload_pending: bool,
}

// Derived, it would require `E: Default`, which an engine is not.
impl<E> Default for Slot<E> {
    fn default() -> Self {
        Self {
            engine: None,
            generation: 0,
            leases: 0,
            unload_pending: false,
        }
    }
}

impl<E> Slot<E> {
    /// Unload, and void any load in flight: it may be building the old model.
    fn supersede(&mut self) -> bool {
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

    fn unload(&mut self) -> bool {
        if self.leases > 0 {
            self.unload_pending = true;
            return false;
        }
        self.unload_pending = false;
        self.engine.take().is_some()
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
}

/// Holds the engine for one recording. Dropping it applies an unload that
/// was requested meanwhile.
pub struct EngineLease {
    slot: Arc<Mutex<Slot>>,
    released: Arc<tokio::sync::Notify>,
}

impl Drop for EngineLease {
    fn drop(&mut self) {
        {
            let mut slot = lock_recovering(&self.slot);
            slot.leases -= 1;
            if slot.unload_pending && slot.unload() {
                log::info!("unloaded the engine the finished recording was using");
            }
        }
        self.released.notify_waiters();
    }
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
        }
    }

    pub fn lease(&self) -> EngineLease {
        lock_recovering(&self.slot).leases += 1;
        EngineLease {
            slot: Arc::clone(&self.slot),
            released: Arc::clone(&self.released),
        }
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
            let saved = settings.language();
            let engine = self.build(settings, saved.as_deref()).await?;
            let backend = engine.backend();
            let languages = engine.languages().to_vec();
            let engine = Arc::new(Mutex::new(engine));
            if !lock_recovering(&self.slot).install(Arc::clone(&engine), generation) {
                log::info!("model changed while loading; loading the new selection");
                continue;
            }
            reset_unsupported_language(settings, &languages, saved.as_deref());
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
    async fn build(
        &self,
        settings: &Settings,
        saved_language: Option<&str>,
    ) -> Result<TranscriptionEngine, String> {
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

        let hint = language::resolve(saved_language).map(str::to_string);
        let models_dir = self.models_dir.clone();
        tauri::async_runtime::spawn_blocking(move || {
            TranscriptionEngine::new(&models_dir, model.as_deref(), hint.as_deref())
        })
        .await
        .map_err(|e| format!("engine load task failed: {e}"))?
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

    /// Point the loaded engine at the saved language. Language is a per-run
    /// option, so the engine is repointed rather than reloaded.
    pub fn apply_language(app: &AppHandle) {
        let Some(engine) = app.state::<Self>().loaded() else {
            return;
        };
        // A streaming recording holds the engine until it ends: wait off the
        // runtime, and read the saved choice once locked so the latest wins.
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Ok(mut engine) = engine.lock() {
                let saved = app.state::<Settings>().language();
                engine.set_language(language::resolve(saved.as_deref()));
            }
        });
    }

    /// Unload now, or once the recordings holding the engine finish. A load in
    /// flight is superseded.
    pub fn unload(&self) -> bool {
        lock_recovering(&self.slot).supersede()
    }

    /// Unload for a model change. True when a recording keeps the old engine.
    pub fn switch_model(&self) -> bool {
        let mut slot = lock_recovering(&self.slot);
        slot.supersede();
        // With nothing loaded yet, the recording's own load picks up the new model.
        slot.unload_pending && slot.engine.is_some()
    }

    /// Drop an engine that panicked, even mid-recording: it cannot be trusted.
    pub fn discard(&self) {
        lock_recovering(&self.slot).engine = None;
    }

    pub fn in_use(&self) -> bool {
        lock_recovering(&self.slot).leases > 0
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
