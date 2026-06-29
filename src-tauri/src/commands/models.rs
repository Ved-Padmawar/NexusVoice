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
        ("tiny", ModelSize::Tiny),
        ("base", ModelSize::Base),
        ("small", ModelSize::Small),
        ("medium", ModelSize::Medium),
        ("large", ModelSize::Large),
        ("large-full", ModelSize::LargeFull),
    ];

    let active_engine = state.load_active_engine();

    let mut models: Vec<DownloadedModel> = all
        .iter()
        .filter_map(|(variant, size)| {
            let path = state.models_dir.join(size.filename());
            if !path.exists() {
                return None;
            }
            let size_bytes = path.metadata().map_or(0, |m| m.len());
            Some(DownloadedModel {
                variant: variant.to_string(),
                display_name: size.display_name().to_string(),
                size_bytes,
                is_active: active_engine == crate::state::Engine::Whisper && *size == active_size,
            })
        })
        .collect();

    if parakeet_downloaded(&state) {
        models.push(DownloadedModel {
            variant: "parakeet".to_string(),
            display_name: "Parakeet v3".to_string(),
            size_bytes: parakeet_size_bytes(&state),
            is_active: active_engine == crate::state::Engine::Parakeet,
        });
    }

    models
}

/// Total on-disk size of the Parakeet model files.
fn parakeet_size_bytes(state: &AppState) -> u64 {
    let dir = state.models_dir.join(crate::parakeet::MODEL_DIR_NAME);
    ["encoder-model.int8.onnx", "decoder_joint-model.int8.onnx", "nemo128.onnx", "vocab.txt"]
        .iter()
        .filter_map(|f| dir.join(f).metadata().ok().map(|m| m.len()))
        .sum()
}

/// Delete a downloaded model by variant ("parakeet" | "tiny" | … | "large-full").
/// Any model may be deleted, including the active one — a recording started with
/// no model present fails gracefully ("model not ready"), prompting re-download.
#[tauri::command]
pub async fn delete_model(state: State<'_, AppState>, variant: String) -> Result<(), ApiError> {
    use crate::inference::provider::ModelSize;

    if variant == "parakeet" {
        let dir = state.models_dir.join(crate::parakeet::MODEL_DIR_NAME);
        if !dir.exists() {
            return Err(ApiError::new("not_found", "model file not found"));
        }
        std::fs::remove_dir_all(&dir).map_err(|e| ApiError::new("io_error", e.to_string()))?;
        *state.engine.lock().await = None;
        return Ok(());
    }

    let size = match variant.as_str() {
        "tiny" => ModelSize::Tiny,
        "base" => ModelSize::Base,
        "small" => ModelSize::Small,
        "medium" => ModelSize::Medium,
        "large" => ModelSize::Large,
        "large-full" => ModelSize::LargeFull,
        _ => {
            return Err(ApiError::new(
                "invalid_variant",
                "variant must be tiny, base, small, medium, large, or large-full",
            ))
        }
    };

    let path = state.models_dir.join(size.filename());
    if !path.exists() {
        return Err(ApiError::new("not_found", "model file not found"));
    }

    std::fs::remove_file(&path).map_err(|e| ApiError::new("io_error", e.to_string()))?;

    // Evict the cached engine in case the deleted model was the loaded one.
    *state.engine.lock().await = None;

    Ok(())
}

// ---------------------------------------------------------------------------
// Active engine (Whisper / Parakeet)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_active_engine(state: State<'_, AppState>) -> String {
    state.load_active_engine().as_str().to_string()
}

/// Switch the active engine. Refuses to activate an engine whose model is not
/// yet on disk, so a recording can't start against a missing model. Evicts the
/// cached engine so the next recording loads the chosen one.
#[tauri::command]
pub async fn set_active_engine(state: State<'_, AppState>, engine: String) -> Result<(), ApiError> {
    let parsed = crate::state::Engine::from_str(&engine)
        .ok_or_else(|| ApiError::new("invalid_engine", "engine must be 'whisper' or 'parakeet'"))?;
    if parsed == crate::state::Engine::Parakeet && !parakeet_downloaded(&state) {
        return Err(ApiError::new(
            "model_not_downloaded",
            "Parakeet model is not downloaded yet",
        ));
    }
    state
        .save_active_engine(parsed)
        .map_err(|e| ApiError::new("io_error", e.to_string()))?;
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
    if !matches!(
        variant.as_str(),
        "tiny" | "base" | "small" | "medium" | "large" | "large-full"
    ) {
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

    let dl = &state.model_download;
    let status = dl.status.load(Ordering::SeqCst);
    let progress = *dl.progress.lock().expect("progress lock poisoned");
    let error = dl.error.lock().expect("error lock poisoned").clone();

    let (downloaded, model_name) = match state.load_active_engine() {
        crate::state::Engine::Parakeet => (parakeet_downloaded(&state), "Parakeet v3".to_string()),
        crate::state::Engine::Whisper => {
            let override_size = state.load_model_override();
            let model_size = select_model_size(detect_backend(), override_size.as_deref());
            (
                state.models_dir.join(model_size.filename()).exists(),
                model_size.display_name().to_string(),
            )
        }
    };

    Ok(ModelInfoResponse {
        // On-disk presence is authoritative and per-engine; the global download
        // status flag is not (a prior Whisper completion must not mark a missing
        // Parakeet as ready).
        downloaded,
        downloading: status == 1,
        download_progress: progress,
        download_error: error,
        model_name,
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

    // Ensure the *active engine's* model exists — not always a Whisper tier.
    // With Parakeet active we must check the Parakeet set, otherwise startup
    // would re-download a Whisper model the user never selected.
    if state.load_active_engine() == crate::state::Engine::Parakeet {
        if parakeet_downloaded(&state) {
            dl.set_complete();
            return Ok(true);
        }
        let dl_state = Arc::clone(&state.model_download);
        let models_dir = state.models_dir.clone();
        dl_state.set_downloading();
        let _ = app.emit("model-download-start", ());
        tauri::async_runtime::spawn_blocking(move || {
            match crate::inference::downloader::download_parakeet_model(&models_dir, &app, &dl_state)
            {
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
        return Ok(true);
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
        match crate::inference::downloader::download_whisper_model(
            &models_dir,
            model_size,
            &app,
            &dl_state,
        ) {
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

/// Whether the Parakeet model is fully present on disk.
fn parakeet_downloaded(state: &AppState) -> bool {
    let dir = state.models_dir.join(crate::parakeet::MODEL_DIR_NAME);
    ["encoder-model.int8.onnx", "decoder_joint-model.int8.onnx", "nemo128.onnx", "vocab.txt"]
        .iter()
        .all(|f| dir.join(f).exists())
}

/// Download (or resume) the Parakeet ONNX model set.
#[tauri::command]
pub async fn download_parakeet(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, ApiError> {
    let dl = &state.model_download;
    if dl.status.load(Ordering::SeqCst) == 1 {
        return Err(ApiError::new(
            "already_downloading",
            "model download already in progress",
        ));
    }

    if parakeet_downloaded(&state) {
        dl.set_complete();
        return Ok(true);
    }

    let dl_state = Arc::clone(&state.model_download);
    let models_dir = state.models_dir.clone();
    dl_state.set_downloading();
    let _ = app.emit("model-download-start", ());

    tauri::async_runtime::spawn_blocking(move || {
        match crate::inference::downloader::download_parakeet_model(&models_dir, &app, &dl_state) {
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
