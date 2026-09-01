//! Model & inference-config commands: catalog listing, downloaded-model
//! management, download start/cancel, model override, dictation language, and
//! hardware profile.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

use super::error::ApiError;

// ---------------------------------------------------------------------------
// Downloaded-model management
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedModel {
    pub variant: String,
    pub display_name: String,
    pub size_bytes: u64,
    pub is_active: bool,
}

/// The full catalog, for the model picker.
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CatalogModel {
    pub id: String,
    pub display_name: String,
    pub family: String,
    /// Every decode path this model supports (`streaming`, `single-shot`).
    pub pipelines: Vec<String>,
    pub default_pipeline: String,
    pub size_bytes: u64,
    pub multilingual: bool,
    pub description: String,
    pub detail: String,
    pub downloaded: bool,
    pub is_active: bool,
}

/// The catalog with per-model download state, so the picker renders from one
/// source of truth instead of a hardcoded frontend list.
#[tauri::command]
#[specta::specta]
pub fn get_model_catalog(state: State<'_, AppState>) -> Vec<CatalogModel> {
    use crate::inference::provider::{detect_backend, select_model};

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
            default_pipeline: entry.default_pipeline.as_str().to_string(),
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
#[tauri::command]
#[specta::specta]
pub fn get_downloaded_models(state: State<'_, AppState>) -> Vec<DownloadedModel> {
    use crate::inference::provider::{detect_backend, select_model};

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
#[tauri::command]
#[specta::specta]
pub async fn delete_model(
    app: AppHandle,
    state: State<'_, AppState>,
    variant: String,
) -> Result<(), ApiError> {
    use crate::inference::catalog;
    use crate::inference::provider::{canonical_override, detect_backend, select_model};

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

    *state.engine.lock().await = None;

    // Prefer another downloaded model over "no model": switch to the most
    // capable file left on disk (the catalog is tier-ascending, so the last
    // match wins); only go to "no model" when nothing is left.
    let fallback = catalog::all()
        .iter()
        .rfind(|m| m.id != entry.id && state.models_dir.join(&m.filename).exists());

    if let Some(next) = fallback {
        let _ = state.save_model_override(&next.id);
        warm_engine_in_background(&app);
        let _ = app.emit("model-switched", ());
    } else {
        state.delete_model_override();
        let _ = app.emit("model-evicted", ());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Model-size override
// ---------------------------------------------------------------------------

/// Set the active model by catalog id. Evicts the cached engine and eagerly
/// warms the newly selected model in the background, so the first transcription
/// after a switch isn't stalled by load.
#[tauri::command]
#[specta::specta]
pub async fn set_model_override(
    app: AppHandle,
    state: State<'_, AppState>,
    variant: String,
) -> Result<(), ApiError> {
    let id = crate::inference::provider::canonical_override(&variant)
        .ok_or_else(|| ApiError::new("invalid_variant", "unknown model id"))?;
    state
        .save_model_override(id)
        .map_err(|e| ApiError::new("io_error", e.to_string()))?;
    *state.engine.lock().await = None;
    warm_engine_in_background(&app);
    Ok(())
}

/// Rebuild + warm the active engine off the command path. A missing model is
/// not an error here — transcription surfaces that later.
fn warm_engine_in_background(app: &AppHandle) {
    use tauri::Manager;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let before = state.load_language();
        if let Err(e) = state.get_or_load_engine().await {
            log::debug!("skipping eager warmup: {e}");
            return;
        }
        // The load resets a language the new model can't speak.
        if before.is_some() && state.load_language() != before {
            let _ = app.emit("language-reset", before);
        }
    });
}

// ---------------------------------------------------------------------------
// Dictation language
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LanguageOption {
    /// ISO code, or `auto` for the detect-per-decode sentinel.
    pub code: String,
    pub name: String,
    pub is_selected: bool,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LanguageSettings {
    /// Whether the active model handles anything but English.
    pub supported: bool,
    /// Empty when `supported` is false.
    pub options: Vec<LanguageOption>,
}

/// The picker's list with the active choice marked, `auto` first. Codes come
/// from the loaded model itself; an English-only model reports
/// `supported: false` and no options.
#[tauri::command]
#[specta::specta]
pub async fn get_language_options(
    state: State<'_, AppState>,
) -> Result<LanguageSettings, ApiError> {
    use crate::inference::language::{self, AUTO};
    use crate::inference::provider::{detect_backend, select_model};

    let model = select_model(detect_backend(), state.load_model_override().as_deref());
    if !model.multilingual {
        return Ok(LanguageSettings {
            supported: false,
            options: Vec::new(),
        });
    }

    // Before the engine loads, or for a model advertising none, the table
    // stands in — the engine drops anything the model won't take.
    let advertised: Vec<String> = state
        .engine
        .lock()
        .await
        .as_ref()
        .and_then(|e| e.lock().ok().map(|g| g.languages().to_vec()))
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| {
            language::LANGUAGES
                .iter()
                .map(|l| l.code.to_string())
                .collect()
        });

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

    Ok(LanguageSettings {
        supported: true,
        options,
    })
}

/// Set the dictation language. The loaded engine is repointed rather than
/// evicted — language is a per-run option, so a reload would be pure cost.
#[tauri::command]
#[specta::specta]
pub async fn set_language(
    state: State<'_, AppState>,
    code: Option<String>,
) -> Result<(), ApiError> {
    use crate::inference::language;

    let engine = state.engine.lock().await;

    // The loaded model is the authority; with none loaded the table stands in
    // and the engine re-checks at load.
    let advertised: Option<Vec<String>> = engine
        .as_ref()
        .and_then(|e| e.lock().ok().map(|g| g.languages().to_vec()))
        .filter(|l| !l.is_empty());

    let saved = match code.as_deref() {
        None => None,
        Some(c) if c == language::AUTO => Some(c),
        Some(c) => {
            let known = advertised.map_or_else(
                || language::is_supported(c),
                |list| list.iter().any(|l| l == c),
            );
            if !known {
                return Err(ApiError::new("invalid_language", "unsupported language"));
            }
            Some(c)
        }
    };

    state
        .save_language(saved)
        .map_err(|e| ApiError::new("io_error", e.to_string()))?;

    if let Some(engine) = engine.as_ref() {
        if let Ok(mut guard) = engine.lock() {
            guard.set_language(language::resolve(saved));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Hardware profile
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HardwareProfileResponse {
    pub gpu_name: String,
    pub execution_provider: String,
    pub vram_gb: f32,
    pub ram_gb: f32,
    pub recommended_model: String,
}

#[tauri::command]
#[specta::specta]
pub async fn get_hardware_profile() -> Result<HardwareProfileResponse, ApiError> {
    use crate::inference::provider::recommend_model;

    let hw = crate::hardware::cached_profile();
    let recommended = recommend_model();

    Ok(HardwareProfileResponse {
        gpu_name: hw.gpu_type.clone(),
        execution_provider: hw.execution_provider.clone(),
        vram_gb: hw.vram_gb,
        ram_gb: hw.ram_gb,
        recommended_model: recommended.display_name.clone(),
    })
}

// ---------------------------------------------------------------------------
// Model download status / control
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize, specta::Type)]
struct DownloadEvent {
    id: String,
}

#[derive(Clone, Serialize, specta::Type)]
struct DownloadErrorEvent {
    id: String,
    error: String,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActiveDownload {
    pub id: String,
    pub status: String,
    pub progress: u8,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfoResponse {
    pub downloaded: bool,
    pub downloading: bool,
    pub model_name: String,
}

#[tauri::command]
#[specta::specta]
pub async fn get_model_info(state: State<'_, AppState>) -> Result<ModelInfoResponse, ApiError> {
    use crate::inference::provider::{detect_backend, select_model};

    let entry = select_model(detect_backend(), state.load_model_override().as_deref());

    Ok(ModelInfoResponse {
        downloaded: state.models_dir.join(&entry.filename).exists(),
        downloading: state.downloads.is_pending(&entry.id),
        model_name: entry.display_name.clone(),
    })
}

/// Fetch a model by id. Never changes the active model, so a failed or
/// cancelled download can't leave the override pointing at a missing file.
/// Beyond `MAX_CONCURRENT_DOWNLOADS` it waits in `Queued`.
#[tauri::command]
#[specta::specta]
pub async fn start_model_download(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), ApiError> {
    let entry = crate::inference::catalog::all()
        .iter()
        .find(|m| m.id == id)
        .ok_or_else(|| ApiError::new("unknown_model", "no such model in the catalog"))?
        .clone();

    // Already on disk: only the complete event clears the caller's optimistic
    // `queued`, so emit one rather than returning quietly.
    if state.models_dir.join(&entry.filename).exists() {
        let _ = app.emit("model-download-complete", DownloadEvent { id });
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
    let _ = app.emit("model-download-start", DownloadEvent { id: id.clone() });

    tauri::async_runtime::spawn(async move {
        use crate::inference::downloader::{download_model, CANCELLED};

        let permit = tokio::select! {
            p = permits.acquire_owned() => p,
            () = cancel.cancelled() => {
                downloads.remove(&id);
                let _ = app.emit("model-download-cancelled", DownloadEvent { id });
                return;
            }
        };
        let Ok(_permit) = permit else { return };

        downloads.set_running(&id);
        let _ = app.emit("model-download-running", DownloadEvent { id: id.clone() });

        match download_model(&models_dir, &entry, &app, &downloads, cancel).await {
            Ok(()) => {
                downloads.remove(&id);
                let _ = app.emit("model-download-complete", DownloadEvent { id });
            }
            Err(e) if e == CANCELLED => {
                downloads.remove(&id);
                let _ = app.emit("model-download-cancelled", DownloadEvent { id });
            }
            Err(e) => {
                downloads.set_error(&id, e.clone());
                let _ = app.emit("model-download-error", DownloadErrorEvent { id, error: e });
            }
        }
    });

    Ok(())
}

/// Cancel a queued or running download. A cancel pressed in the last moments of
/// a transfer arrives after the file is published; the download then stands and
/// the caller learns that from `model-download-complete`.
#[tauri::command]
#[specta::specta]
pub fn cancel_model_download(id: String, state: State<'_, AppState>) {
    state.downloads.cancel(&id);
}

/// Every download queued, running, or holding an error, so the UI can rehydrate
/// after a reload without waiting for the next event.
#[tauri::command]
#[specta::specta]
pub fn get_active_downloads(state: State<'_, AppState>) -> Vec<ActiveDownload> {
    state
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
