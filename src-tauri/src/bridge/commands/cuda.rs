//! Optional CUDA backend: status, download and removal of the CUDA pack.

use tauri::{AppHandle, Manager, State};

use crate::models::cuda::{self, CudaPack};

use super::error::ApiError;

#[tauri::command]
#[specta::specta]
pub async fn start_cuda_download(app: AppHandle) -> Result<(), ApiError> {
    let supported = tauri::async_runtime::spawn_blocking(cuda::supported)
        .await
        .expect("CUDA support probe panicked");
    if !supported {
        return Err(ApiError::new("unsupported", "CUDA needs an NVIDIA GPU"));
    }
    CudaPack::start_download(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn cancel_cuda_download(pack: State<'_, CudaPack>) {
    pack.cancel();
}

/// Remove the pack; one in use is deleted at the next start.
#[tauri::command]
#[specta::specta]
pub async fn remove_cuda_pack(app: AppHandle) -> Result<(), ApiError> {
    tauri::async_runtime::spawn_blocking(move || app.state::<CudaPack>().remove())
        .await
        .expect("CUDA pack removal panicked")?;
    Ok(())
}
