//! Optional CUDA backend: status, download and removal of the CUDA pack.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::inference::{cuda, downloader::CANCELLED};
use crate::state::AppState;

use super::error::ApiError;

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CudaStatus {
    /// An NVIDIA GPU on a platform the pack is built for; CUDA is hidden otherwise.
    pub supported: bool,
    pub pack: cuda::PackState,
    /// Percent of the running download; `None` when idle.
    pub download_progress: Option<u8>,
}

#[tauri::command]
#[specta::specta]
pub fn get_cuda_status(state: State<'_, AppState>) -> CudaStatus {
    CudaStatus {
        supported: cuda::supported(),
        pack: cuda::pack_state(&state.app_data_dir),
        download_progress: state.cuda_download.progress(),
    }
}

/// Fetch the CUDA pack in the background, reported through `cuda-download-*` events.
#[tauri::command]
#[specta::specta]
pub fn start_cuda_download(app: AppHandle, state: State<'_, AppState>) -> Result<(), ApiError> {
    if !cuda::supported() {
        return Err(ApiError::new("unsupported", "CUDA needs an NVIDIA GPU"));
    }
    let download = Arc::clone(&state.cuda_download);
    let app_data_dir = state.app_data_dir.clone();

    tauri::async_runtime::spawn(async move {
        match download.run(&app, &app_data_dir).await {
            Ok(()) => {
                let _ = app.emit("cuda-download-complete", ());
            }
            Err(e) if e == CANCELLED => {
                let _ = app.emit("cuda-download-cancelled", ());
            }
            Err(e) => {
                log::warn!("CUDA pack download failed: {e}");
                let _ = app.emit("cuda-download-error", e);
            }
        }
    });
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn cancel_cuda_download(state: State<'_, AppState>) {
    state.cuda_download.cancel();
}

/// Remove the pack; one in use is deleted at the next start.
#[tauri::command]
#[specta::specta]
pub fn remove_cuda_pack(state: State<'_, AppState>) -> Result<(), ApiError> {
    state.cuda_download.cancel();
    cuda::remove(&state.app_data_dir).map_err(|e| ApiError::new("io_error", e.to_string()))
}
