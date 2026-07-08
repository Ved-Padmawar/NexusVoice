//! NVIDIA model selection, download, and hardware-profile commands.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::inference::provider::{selected_model, Model, ALL_MODELS};
use crate::state::AppState;

use super::error::ApiError;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedModel {
    variant: String,
    display_name: String,
    size_bytes: u64,
    is_active: bool,
}

#[tauri::command]
pub fn get_downloaded_models(state: State<'_, AppState>) -> Vec<DownloadedModel> {
    let active = active_model(&state);
    ALL_MODELS
        .into_iter()
        .filter_map(|model| {
            let path = state.models_dir.join(model.filename());
            let size_bytes = path.metadata().ok()?.len();
            Some(DownloadedModel {
                variant: model.id().into(),
                display_name: model.display_name().into(),
                size_bytes,
                is_active: model == active,
            })
        })
        .collect()
}

#[tauri::command]
pub async fn delete_model(state: State<'_, AppState>, variant: String) -> Result<(), ApiError> {
    let model = parse_model(&variant)?;
    if model == active_model(&state) {
        return Err(ApiError::new(
            "active_model",
            "the active model cannot be deleted",
        ));
    }
    let path = state.models_dir.join(model.filename());
    if !path.is_file() {
        return Err(ApiError::new("not_found", "model file not found"));
    }
    std::fs::remove_file(path).map_err(|e| ApiError::new("io_error", e.to_string()))
}

#[tauri::command]
pub async fn set_model_override(
    state: State<'_, AppState>,
    variant: String,
) -> Result<(), ApiError> {
    parse_model(&variant)?;
    state
        .save_model_override(&variant)
        .map_err(|e| ApiError::new("io_error", e.to_string()))?;
    *state.engine.lock().await = None;
    Ok(())
}

#[tauri::command]
pub async fn clear_model_override(state: State<'_, AppState>) -> Result<(), ApiError> {
    state.delete_model_override();
    *state.engine.lock().await = None;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareProfileResponse {
    gpu_name: String,
    execution_provider: String,
    vram_gb: f32,
    ram_gb: f32,
    recommended_model: String,
}

#[tauri::command]
pub async fn get_hardware_profile() -> Result<HardwareProfileResponse, ApiError> {
    let hardware = crate::hardware::cached_profile();
    let recommended = crate::inference::provider::recommended_model();
    Ok(HardwareProfileResponse {
        gpu_name: hardware.gpu_type.clone(),
        execution_provider: hardware.execution_provider.clone(),
        vram_gb: hardware.vram_gb,
        ram_gb: hardware.ram_gb,
        recommended_model: recommended.display_name().into(),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfoResponse {
    downloaded: bool,
    downloading: bool,
    download_progress: u8,
    download_error: Option<String>,
    model_name: String,
}

#[tauri::command]
pub async fn get_model_info(state: State<'_, AppState>) -> Result<ModelInfoResponse, ApiError> {
    let model = active_model(&state);
    Ok(ModelInfoResponse {
        downloaded: state.models_dir.join(model.filename()).is_file(),
        downloading: state.model_download.status.load(Ordering::SeqCst) == 1,
        download_progress: *state
            .model_download
            .progress
            .lock()
            .expect("progress lock poisoned"),
        download_error: state
            .model_download
            .error
            .lock()
            .expect("error lock poisoned")
            .clone(),
        model_name: model.display_name().into(),
    })
}

#[tauri::command]
pub async fn retry_model_download(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, ApiError> {
    if state.model_download.status.load(Ordering::SeqCst) == 1 {
        return Err(ApiError::new(
            "already_downloading",
            "model download already in progress",
        ));
    }
    let model = active_model(&state);
    if state.models_dir.join(model.filename()).is_file() {
        state.model_download.set_complete();
        return Ok(true);
    }

    let download_state = Arc::clone(&state.model_download);
    let models_dir = state.models_dir.clone();
    download_state.set_downloading();
    let _ = app.emit("model-download-start", ());
    tauri::async_runtime::spawn_blocking(
        move || match crate::inference::downloader::download_model(
            &models_dir,
            model,
            &app,
            &download_state,
        ) {
            Ok(()) => {
                download_state.set_complete();
                let _ = app.emit("model-download-complete", ());
            }
            Err(error) if error == "download_cancelled" => {
                download_state.set_cancelled();
                let _ = app.emit("model-download-cancelled", ());
            }
            Err(error) => {
                download_state.set_error(error.clone());
                let _ = app.emit("model-download-error", error);
            }
        },
    );
    Ok(true)
}

#[tauri::command]
pub fn cancel_model_download(state: State<'_, AppState>) {
    if state.model_download.status.load(Ordering::SeqCst) == 1 {
        state.model_download.cancelled.store(true, Ordering::SeqCst);
    }
}

fn active_model(state: &AppState) -> Model {
    selected_model(state.load_model_override().as_deref())
}

fn parse_model(id: &str) -> Result<Model, ApiError> {
    Model::from_id(id)
        .ok_or_else(|| ApiError::new("invalid_variant", "unknown NVIDIA speech model variant"))
}
