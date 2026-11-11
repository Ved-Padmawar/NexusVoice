//! Optional CUDA backend: status, download and removal of the CUDA pack.

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::models::cuda::{self, CudaPack};

use super::error::ApiError;

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CudaStatus {
    /// An NVIDIA GPU on a platform the pack is built for; CUDA is hidden otherwise.
    pub supported: bool,
    pub pack: cuda::PackState,
    /// Percent of the running download; `None` when idle.
    pub download_progress: Option<u8>,
    /// The running download is unpacking and checking the pack.
    pub verifying: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn get_cuda_status(app: AppHandle) -> CudaStatus {
    // Probes the GPU driver and verifies the manifest: off the main thread.
    tauri::async_runtime::spawn_blocking(move || {
        let pack = app.state::<CudaPack>();
        CudaStatus {
            supported: cuda::supported(),
            pack: pack.state(),
            download_progress: pack.progress(),
            verifying: pack.verifying(),
        }
    })
    .await
    .expect("CUDA status probe panicked")
}

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
pub fn remove_cuda_pack(pack: State<'_, CudaPack>) -> Result<(), ApiError> {
    Ok(pack.remove()?)
}
