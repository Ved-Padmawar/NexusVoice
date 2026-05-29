//! Model & inference-config commands: downloaded-model management, download
//! retry/cancel, model-size override, beam-size preset, and hardware profile.

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

/// List all model files currently on disk.
#[tauri::command]
pub fn get_downloaded_models(state: State<'_, AppState>) -> Vec<DownloadedModel> {
    use crate::inference::provider::{detect_backend, select_model_size, ModelSize};

    let active_override = state.load_model_override();
    let active_backend = detect_backend();
    let active_size = select_model_size(active_backend, active_override.as_deref());

    let all: &[(&str, ModelSize)] = &[
        ("tiny",       ModelSize::Tiny),
        ("base",       ModelSize::Base),
        ("small",      ModelSize::Small),
        ("medium",     ModelSize::Medium),
        ("large",      ModelSize::Large),
        ("large-full", ModelSize::LargeFull),
    ];

    all.iter().filter_map(|(variant, size)| {
        let path = state.models_dir.join(size.filename());
        if !path.exists() { return None; }
        let size_bytes = path.metadata().map(|m| m.len()).unwrap_or(0);
        Some(DownloadedModel {
            variant: variant.to_string(),
            display_name: size.display_name().to_string(),
            size_bytes,
            is_active: *size == active_size,
        })
    }).collect()
}

/// Delete a downloaded model file by variant ("tiny" | "base" | "small" | "medium" | "large").
/// Refuses to delete the currently active model.
#[tauri::command]
pub async fn delete_model(
    state: State<'_, AppState>,
    variant: String,
) -> Result<(), ApiError> {
    use crate::inference::provider::{detect_backend, select_model_size, ModelSize};

    let size = match variant.as_str() {
        "tiny"       => ModelSize::Tiny,
        "base"       => ModelSize::Base,
        "small"      => ModelSize::Small,
        "medium"     => ModelSize::Medium,
        "large"      => ModelSize::Large,
        "large-full" => ModelSize::LargeFull,
        _ => return Err(ApiError::new("invalid_variant", "variant must be tiny, base, small, medium, large, or large-full")),
    };

    let active_override = state.load_model_override();
    let active_backend = detect_backend();
    let active_size = select_model_size(active_backend, active_override.as_deref());

    if size == active_size {
        return Err(ApiError::new("active_model", "cannot delete the currently active model"));
    }

    let path = state.models_dir.join(size.filename());
    if !path.exists() {
        return Err(ApiError::new("not_found", "model file not found"));
    }

    std::fs::remove_file(&path)
        .map_err(|e| ApiError::new("io_error", e.to_string()))?;

    // If deleted model was cached in engine, evict it
    *state.engine.lock().await = None;

    Ok(())
}

// ---------------------------------------------------------------------------
// Model-size override
// ---------------------------------------------------------------------------

/// Set model size override ("tiny" | "base" | "small" | "medium" | "large" | "large-full").
/// Clears the cached engine so the next transcription reloads with the chosen model.
#[tauri::command]
pub async fn set_model_override(
    state: State<'_, AppState>,
    variant: String,
) -> Result<(), ApiError> {
    if !matches!(variant.as_str(), "tiny" | "base" | "small" | "medium" | "large" | "large-full") {
        return Err(ApiError::new(
            "invalid_variant",
            "variant must be 'tiny', 'base', 'small', 'medium', 'large', or 'large-full'",
        ));
    }
    state
        .save_model_override(&variant)
        .map_err(|e| ApiError::new("io_error", e.to_string()))?;
    *state.engine.lock().await = None;
    Ok(())
}

/// Clear the model size override, reverting to auto-selection based on hardware.
#[tauri::command]
pub async fn clear_model_override(state: State<'_, AppState>) -> Result<(), ApiError> {
    state.delete_model_override();
    *state.engine.lock().await = None;
    Ok(())
}

// ---------------------------------------------------------------------------
// Beam size
// ---------------------------------------------------------------------------

/// Get the current beam size. Returns 2 (Fast), 5 (Balanced), or 8 (Accurate).
#[tauri::command]
pub fn get_beam_size(state: State<'_, AppState>) -> i32 {
    state.load_beam_size()
}

/// Set beam size to 2, 5, or 8. Does not evict the engine — `beam_size` is applied
/// per transcription call, so it takes effect immediately on the next recording.
#[tauri::command]
pub fn set_beam_size(state: State<'_, AppState>, beam_size: i32) -> Result<(), ApiError> {
    if beam_size != 2 && beam_size != 5 && beam_size != 8 {
        return Err(ApiError::new(
            "invalid_beam_size",
            "beam_size must be 2 (Fast), 5 (Balanced), or 8 (Accurate)",
        ));
    }
    state
        .save_beam_size(beam_size)
        .map_err(|e| ApiError::new("io_error", e.to_string()))
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
    use crate::inference::provider::recommend_model_size;

    let hw = crate::hardware::cached_profile();
    let recommended = recommend_model_size();

    Ok(HardwareProfileResponse {
        gpu_name: hw.gpu_type.clone(),
        execution_provider: hw.execution_provider.clone(),
        vram_gb: hw.vram_gb,
        ram_gb: hw.ram_gb,
        recommended_model: recommended.display_name().to_string(),
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
    use crate::inference::provider::{detect_backend, select_model_size};

    let override_size = state.load_model_override();
    let backend = detect_backend();
    let model_size = select_model_size(backend, override_size.as_deref());
    let model_path = state.models_dir.join(model_size.filename());

    let dl = &state.model_download;
    let status = dl.status.load(Ordering::SeqCst);
    let progress = *dl.progress.lock().expect("progress lock poisoned");
    let error = dl.error.lock().expect("error lock poisoned").clone();

    Ok(ModelInfoResponse {
        downloaded: model_path.exists() || status == 2,
        downloading: status == 1,
        download_progress: progress,
        download_error: error,
        model_name: model_size.display_name().to_string(),
    })
}

#[tauri::command]
pub async fn retry_model_download(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, ApiError> {
    use crate::inference::provider::{detect_backend, select_model_size};

    let dl = &state.model_download;
    let status = dl.status.load(Ordering::SeqCst);

    if status == 1 {
        return Err(ApiError::new(
            "already_downloading",
            "model download already in progress",
        ));
    }

    let override_size = state.load_model_override();
    let backend = detect_backend();
    let model_size = select_model_size(backend, override_size.as_deref());
    let model_path = state.models_dir.join(model_size.filename());

    if model_path.exists() {
        dl.set_complete();
        return Ok(true);
    }

    let dl_state = Arc::clone(&state.model_download);
    let models_dir = state.models_dir.clone();
    dl_state.set_downloading();
    let _ = app.emit("model-download-start", ());

    tauri::async_runtime::spawn_blocking(move || {
        match crate::inference::downloader::download_whisper_model(&models_dir, model_size, &app, &dl_state) {
            Ok(()) => {
                dl_state.set_complete();
                let _ = app.emit("model-download-complete", ());
            }
            Err(e) if e == "download_cancelled" => {
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

/// Cancel an in-progress model download. The download loop checks this flag
/// each chunk and exits cleanly, deleting the partial .tmp file.
#[tauri::command]
pub fn cancel_model_download(state: State<'_, AppState>) {
    let dl = &state.model_download;
    let status = dl.status.load(Ordering::SeqCst);
    if status == 1 {
        dl.cancelled.store(true, Ordering::SeqCst);
    }
}
