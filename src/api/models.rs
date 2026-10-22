//! Model & inference-config commands: catalog listing, downloaded-model
//! management, download start/cancel, model override, dictation language, and
//! hardware profile.

use std::sync::Arc;

use crate::events::{self, Event, ModelDownload};
use crate::inference::idle::ModelUnload;
use crate::state;

use super::error::ApiError;

// ---------------------------------------------------------------------------
// Downloaded-model management
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DownloadedModel {
    pub variant: String,
    pub display_name: String,
    pub size_bytes: u64,
    pub is_active: bool,
}

/// The full catalog, for the model picker.
#[derive(Debug, Clone)]
pub struct CatalogModel {
    pub id: String,
    pub display_name: String,
    pub family: String,
    /// Every decode path this model supports (`streaming`, `single-shot`).
    pub pipelines: Vec<String>,
    pub size_bytes: u64,
    pub multilingual: bool,
    pub description: String,
    pub detail: String,
    pub downloaded: bool,
    pub is_active: bool,
}

/// The catalog with per-model download state, so the picker renders from one
/// source of truth instead of a hardcoded frontend list.
pub fn get_model_catalog() -> Vec<CatalogModel> {
    use crate::inference::provider::{detect_backend, select_model};

    let state = state::get();

    let active = select_model(detect_backend(), state.load_model_override().as_deref());

    crate::inference::catalog::all()
        .iter()
        .map(|entry| CatalogModel {
            id: entry.id.clone(),
            display_name: entry.display_name.clone(),
            family: entry.family.clone(),
            pipelines: entry
                .pipelines
                .iter()
                .map(|p| p.as_str().to_string())
                .collect(),
            size_bytes: entry.size_bytes,
            multilingual: entry.multilingual,
            description: entry.description.clone(),
            detail: entry.detail.clone(),
            downloaded: state.models_dir.join(&entry.filename).exists(),
            is_active: entry.id == active.id,
        })
        .collect()
}

/// List all model files currently on disk.
pub fn get_downloaded_models() -> Vec<DownloadedModel> {
    use crate::inference::provider::{detect_backend, select_model};

    let state = state::get();

    let active = select_model(detect_backend(), state.load_model_override().as_deref());

    crate::inference::catalog::all()
        .iter()
        .filter_map(|entry| {
            let path = state.models_dir.join(&entry.filename);
            if !path.exists() {
                return None;
            }
            Some(DownloadedModel {
                variant: entry.id.clone(),
                display_name: entry.display_name.clone(),
                size_bytes: path.metadata().map_or(0, |m| m.len()),
                is_active: entry.id == active.id,
            })
        })
        .collect()
}

/// Delete a downloaded model file by catalog id. Any model may be deleted,
/// including the active one — transcription then surfaces a "no model" error
/// until another is downloaded.
pub async fn delete_model(variant: String) -> Result<(), ApiError> {
    use crate::inference::catalog;
    use crate::inference::provider::{canonical_override, detect_backend, select_model};

    let state = state::get();

    let entry = canonical_override(&variant)
        .and_then(catalog::find)
        .ok_or_else(|| ApiError::new("invalid_variant", "unknown model id"))?;

    let path = state.models_dir.join(&entry.filename);
    if !path.exists() {
        return Err(ApiError::new("not_found", "model file not found"));
    }

    std::fs::remove_file(&path).map_err(|e| ApiError::new("io_error", e.to_string()))?;

    // Nothing more to do unless we deleted the active model.
    let active = select_model(detect_backend(), state.load_model_override().as_deref());
    if entry.id != active.id {
        return Ok(());
    }

    state.evict_engine().await;

    // Prefer another downloaded model over "no model": switch to the most
    // capable file left on disk (the catalog is tier-ascending, so the last
    // match wins); only go to "no model" when nothing is left.
    let fallback = catalog::all()
        .iter()
        .rfind(|m| m.id != entry.id && state.models_dir.join(&m.filename).exists());

    if let Some(next) = fallback {
        let _ = state.save_model_override(&next.id);
        warm_engine_in_background();
        events::emit(Event::ModelSwitched);
    } else {
        state.delete_model_override();
        events::emit(Event::ModelEvicted);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Model-size override
// ---------------------------------------------------------------------------

/// Set the active model by catalog id. Evicts the cached engine and eagerly
/// warms the newly selected model in the background, so the first transcription
/// after a switch isn't stalled by load.
pub async fn set_model_override(variant: String) -> Result<(), ApiError> {
    let state = state::get();
    let id = crate::inference::provider::canonical_override(&variant)
        .ok_or_else(|| ApiError::new("invalid_variant", "unknown model id"))?;
    state
        .save_model_override(id)
        .map_err(|e| ApiError::new("io_error", e.to_string()))?;
    state.evict_engine().await;
    warm_engine_in_background();
    Ok(())
}

/// Rebuild + warm the active engine off the command path. A missing model is
/// not an error here — transcription surfaces that later.
fn warm_engine_in_background() {
    tokio::spawn(async move {
        let state = state::get();
        let before = state.load_language();
        if let Err(e) = state.get_or_load_engine().await {
            log::debug!("skipping eager warmup: {e}");
            return;
        }
        // The load resets a language the new model can't speak.
        if before.is_some() && state.load_language() != before {
            events::emit(Event::LanguageReset);
        }
    });
}

// ---------------------------------------------------------------------------
// Dictation language
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LanguageOption {
    /// ISO code, or `auto` for the detect-per-decode sentinel.
    pub code: String,
    pub name: String,
    pub is_selected: bool,
}

#[derive(Debug, Clone)]
pub struct LanguageSettings {
    /// Whether the active model handles anything but English.
    pub supported: bool,
    /// Empty when `supported` is false.
    pub options: Vec<LanguageOption>,
}

/// The picker's list with the active choice marked, `auto` first. Codes come
/// from the loaded model itself; an English-only model reports
/// `supported: false` and no options.
pub fn get_language_options() -> LanguageSettings {
    use crate::inference::language::{self, AUTO};
    use crate::inference::provider::{detect_backend, select_model};

    let state = state::get();

    let model = select_model(detect_backend(), state.load_model_override().as_deref());
    if !model.multilingual {
        return LanguageSettings {
            supported: false,
            options: Vec::new(),
        };
    }

    // From the catalog, so the list is complete whether or not a model has loaded.
    let advertised = &model.languages;

    let saved = state.load_language();
    let active = language::resolve(saved.as_deref());

    // Region shown only where a language has several locales.
    let mut named: Vec<(String, String)> = advertised
        .iter()
        .map(|code| {
            let base = language::primary_of(code);
            let regioned = advertised
                .iter()
                .filter(|c| language::primary_of(c) == base)
                .count()
                > 1;
            (code.clone(), language::display_name(code, regioned))
        })
        .collect();
    named.sort_by(|a, b| {
        a.1.to_lowercase()
            .cmp(&b.1.to_lowercase())
            .then_with(|| a.0.cmp(&b.0))
    });

    let options = std::iter::once(LanguageOption {
        code: AUTO.to_string(),
        name: "Auto-detect".to_string(),
        is_selected: active.is_none(),
    })
    .chain(named.into_iter().map(|(code, name)| LanguageOption {
        is_selected: active == Some(code.as_str()),
        code,
        name,
    }))
    .collect();

    LanguageSettings {
        supported: true,
        options,
    }
}

/// Set the dictation language. The loaded engine is repointed rather than
/// evicted — language is a per-run option, so a reload would be pure cost.
pub async fn set_language(code: Option<String>) -> Result<(), ApiError> {
    use crate::inference::language;
    use crate::inference::provider::{detect_backend, select_model};

    let state = state::get();

    let model = select_model(detect_backend(), state.load_model_override().as_deref());
    let saved = match code.as_deref() {
        None => None,
        Some(c) if c == language::AUTO => Some(c),
        Some(c) if model.languages.iter().any(|l| l == c) => Some(c),
        Some(_) => return Err(ApiError::new("invalid_language", "unsupported language")),
    };

    state
        .save_language(saved)
        .map_err(|e| ApiError::new("io_error", e.to_string()))?;

    // A streaming recording holds the engine until it ends: apply off the async
    // runtime, reading the saved choice once locked so the latest change wins.
    if let Some(engine) = state.engine.lock().await.clone() {
        tokio::task::spawn_blocking(move || {
            if let Ok(mut guard) = engine.lock() {
                let saved = state::get().load_language();
                guard.set_language(language::resolve(saved.as_deref()));
            }
        });
    }

    Ok(())
}

pub fn get_model_unload() -> ModelUnload {
    state::get().load_model_unload()
}

/// Takes effect at the idle watcher's next check.
pub fn set_model_unload(policy: ModelUnload) -> Result<(), ApiError> {
    state::get()
        .save_model_unload(policy)
        .map_err(|e| ApiError::new("io_error", e.to_string()))
}

// ---------------------------------------------------------------------------
// Hardware profile
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HardwareProfileResponse {
    pub gpu_name: String,
    /// Backend in use: the loaded model's, else the one it is expected to get.
    pub execution_provider: String,
    pub vram_gb: f32,
    pub ram_gb: f32,
    pub recommended_model: String,
}

pub fn get_hardware_profile() -> HardwareProfileResponse {
    use crate::inference::{cuda, provider::recommend_model};

    let state = state::get();

    let hw = crate::hardware::cached_profile();
    let recommended = recommend_model();
    // Before a model loads, NVIDIA runs on Vulkan unless a CUDA device registered.
    let execution_provider =
        state
            .engine_backend()
            .unwrap_or_else(|| match hw.execution_provider.as_str() {
                "cuda" if !cuda::registered() => "vulkan".to_string(),
                other => other.to_string(),
            });

    HardwareProfileResponse {
        gpu_name: hw.gpu_type.clone(),
        execution_provider,
        vram_gb: hw.vram_gb,
        ram_gb: hw.ram_gb,
        recommended_model: recommended.display_name.clone(),
    }
}

// ---------------------------------------------------------------------------
// Model download status / control
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ActiveDownload {
    pub id: String,
    pub status: String,
    pub progress: u8,
    pub error: Option<String>,
}

/// Whether the model the engine will load is on disk.
pub fn active_model_downloaded() -> bool {
    use crate::inference::provider::{detect_backend, select_model};

    let state = state::get();
    let entry = select_model(detect_backend(), state.load_model_override().as_deref());
    state.models_dir.join(&entry.filename).exists()
}

/// Fetch a model by id. Never changes the active model, so a failed or
/// cancelled download can't leave the override pointing at a missing file.
/// Beyond `MAX_CONCURRENT_DOWNLOADS` it waits in `Queued`.
pub fn start_model_download(id: String) -> Result<(), ApiError> {
    let state = state::get();
    let entry = crate::inference::catalog::all()
        .iter()
        .find(|m| m.id == id)
        .ok_or_else(|| ApiError::new("unknown_model", "no such model in the catalog"))?
        .clone();

    // Already on disk: only the complete event clears the caller's optimistic
    // `queued`, so emit one rather than returning quietly.
    if state.models_dir.join(&entry.filename).exists() {
        emit_download(id, ModelDownload::Complete);
        return Ok(());
    }

    // Already in flight: silent no-op — re-emitting `start` would rewind the
    // caller's progress bar to 0.
    let Some(cancel) = state.downloads.enqueue(&id) else {
        return Ok(());
    };

    let downloads = Arc::clone(&state.downloads);
    let models_dir = state.models_dir.clone();
    let permits = state.downloads.permits();
    emit_download(id.clone(), ModelDownload::Queued);

    tokio::spawn(async move {
        use crate::inference::downloader::{download_model, CANCELLED};

        let permit = tokio::select! {
            p = permits.acquire_owned() => p,
            () = cancel.cancelled() => {
                downloads.remove(&id);
                emit_download(id, ModelDownload::Cancelled);
                return;
            }
        };
        let Ok(_permit) = permit else { return };

        downloads.set_running(&id);
        emit_download(id.clone(), ModelDownload::Running);

        match download_model(&models_dir, &entry, &downloads, cancel).await {
            Ok(()) => {
                downloads.remove(&id);
                emit_download(id, ModelDownload::Complete);
            }
            Err(e) if e == CANCELLED => {
                downloads.remove(&id);
                emit_download(id, ModelDownload::Cancelled);
            }
            Err(e) => {
                downloads.set_error(&id, e.clone());
                emit_download(id, ModelDownload::Failed(e));
            }
        }
    });

    Ok(())
}

/// Cancel a queued or running download. A cancel pressed in the last moments of
/// a transfer arrives after the file is published; the download then stands and
/// the caller learns that from `model-download-complete`.
pub fn cancel_model_download(id: &str) {
    state::get().downloads.cancel(id);
}

fn emit_download(id: String, status: ModelDownload) {
    events::emit(Event::ModelDownload { id, status });
}

/// Every download queued, running, or holding an error, so the UI can rehydrate
/// after a reload without waiting for the next event.
pub fn get_active_downloads() -> Vec<ActiveDownload> {
    state::get()
        .downloads
        .snapshot()
        .into_iter()
        .map(|(id, status, progress, error)| ActiveDownload {
            id,
            status: status.to_string(),
            progress,
            error,
        })
        .collect()
}
