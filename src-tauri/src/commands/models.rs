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

    all.iter()
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
                is_active: *size == active_size,
            })
        })
        .collect()
}

/// Delete a downloaded model file by variant ("tiny" | "base" | "small" | "medium" | "large").
/// Any model may be deleted, including the active one — transcription then surfaces
/// a "no model" error until another is downloaded.
#[tauri::command]
pub async fn delete_model(
    app: AppHandle,
    state: State<'_, AppState>,
    variant: String,
) -> Result<(), ApiError> {
    use crate::inference::provider::{detect_backend, select_model_size, ModelSize};

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

    // Nothing more to do unless we deleted the active model.
    let active_size = select_model_size(detect_backend(), state.load_model_override().as_deref());
    if size != active_size {
        return Ok(());
    }

    *state.engine.lock().await = None;

    // Prefer another downloaded model over "no model": switch to the largest
    // remaining file on disk; only go to "no model" when nothing is left.
    let remaining: &[(&str, ModelSize)] = &[
        ("large-full", ModelSize::LargeFull),
        ("large", ModelSize::Large),
        ("medium", ModelSize::Medium),
        ("small", ModelSize::Small),
        ("base", ModelSize::Base),
        ("tiny", ModelSize::Tiny),
    ];
    let fallback = remaining
        .iter()
        .find(|(_, s)| *s != size && state.models_dir.join(s.filename()).exists());

    if let Some((variant, _)) = fallback {
        let _ = state.save_model_override(variant);
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

/// Set model size override ("tiny" | "base" | "small" | "medium" | "large" | "large-full").
/// Evicts the cached engine and eagerly warms the newly selected model in the
/// background, so the first transcription after a switch isn't stalled by load.
#[tauri::command]
pub async fn set_model_override(
    app: AppHandle,
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

    tauri::async_runtime::spawn(async move {
        use crate::inference::downloader::{download_whisper_model, CANCELLED};

        match download_whisper_model(&models_dir, model_size, &app, &dl_state).await {
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
