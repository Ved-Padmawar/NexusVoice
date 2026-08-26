//! Model & inference-config commands: catalog listing, downloaded-model
//! management, download retry/cancel, model override, and hardware profile.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

use super::error::ApiError;

// ---------------------------------------------------------------------------
// Downloaded-model management
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedModel {
    pub variant: String,
    pub display_name: String,
    pub size_bytes: u64,
    pub is_active: bool,
}

/// The full catalog, for the model picker.
#[derive(Debug, Serialize)]
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

/// Clear the model size override, reverting to auto-selection based on hardware.
#[tauri::command]
pub async fn clear_model_override(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), ApiError> {
    state.delete_model_override();
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
        if let Err(e) = state.get_or_load_engine().await {
            log::debug!("skipping eager warmup: {e}");
        }
    });
}

// ---------------------------------------------------------------------------
// Hardware profile
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareProfileResponse {
    pub gpu_name: String,
    pub execution_provider: String,
    pub vram_gb: f32,
    pub ram_gb: f32,
    pub recommended_model: String,
}

#[tauri::command]
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfoResponse {
    pub downloaded: bool,
    pub downloading: bool,
    pub download_progress: u8,
    pub download_error: Option<String>,
    pub model_name: String,
}

#[tauri::command]
pub async fn get_model_info(state: State<'_, AppState>) -> Result<ModelInfoResponse, ApiError> {
    use crate::inference::provider::{detect_backend, select_model};

    let entry = select_model(detect_backend(), state.load_model_override().as_deref());
    let model_path = state.models_dir.join(&entry.filename);

    let dl = &state.model_download;
    let status = dl.status.load(Ordering::SeqCst);
    let downloading = status == 1;
    // File on disk is the truth for the selected model; download status/progress
    // can be stale from a previous model, so only trust progress while downloading.
    let on_disk = model_path.exists();
    let progress = if downloading {
        *dl.progress.lock().expect("progress lock poisoned")
    } else if on_disk {
        100
    } else {
        0
    };
    let error = dl.error.lock().expect("error lock poisoned").clone();

    Ok(ModelInfoResponse {
        downloaded: on_disk,
        downloading,
        download_progress: progress,
        download_error: error,
        model_name: entry.display_name.clone(),
    })
}

#[tauri::command]
pub async fn retry_model_download(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, ApiError> {
    use crate::inference::provider::{detect_backend, select_model};

    let dl = &state.model_download;
    let status = dl.status.load(Ordering::SeqCst);

    if status == 1 {
        return Err(ApiError::new(
            "already_downloading",
            "model download already in progress",
        ));
    }

    let entry = select_model(detect_backend(), state.load_model_override().as_deref());
    let model_path = state.models_dir.join(&entry.filename);

    if model_path.exists() {
        dl.set_complete();
        return Ok(true);
    }

    let dl_state = Arc::clone(&state.model_download);
    let models_dir = state.models_dir.clone();
    dl_state.set_downloading();
    let _ = app.emit("model-download-start", ());

    tauri::async_runtime::spawn(async move {
        use crate::inference::downloader::{download_model, CANCELLED};

        match download_model(&models_dir, entry, &app, &dl_state).await {
            Ok(()) => {
                dl_state.set_complete();
                // Drop any fallback engine cached while this was downloading.
                {
                    use tauri::Manager;
                    *app.state::<AppState>().engine.lock().await = None;
                }
                let _ = app.emit("model-download-complete", ());
                warm_engine_in_background(&app);
            }
            Err(e) if e == CANCELLED => {
                dl_state.set_cancelled();
                let _ = app.emit("model-download-cancelled", ());
            }
            Err(e) => {
                dl_state.set_error(e.clone());
                let _ = app.emit("model-download-error", e);
            }
        }
    });

    Ok(true)
}

/// Cancel an in-progress model download. Trips the cancellation token the
/// transfer loop is awaiting, so it stops without finishing the current chunk.
#[tauri::command]
pub fn cancel_model_download(state: State<'_, AppState>) {
    let dl = &state.model_download;
    if dl.status.load(Ordering::SeqCst) == 1 {
        dl.cancel();
    }
}
