//! Optional CUDA backend: status, download and removal of the CUDA pack.

use serde::Serialize;
use tauri::{AppHandle, State};

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
pub fn get_cuda_status(pack: State<'_, CudaPack>) -> CudaStatus {
    CudaStatus {
        supported: cuda::supported(),
        pack: pack.state(),
        download_progress: pack.progress(),
        verifying: pack.verifying(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn start_cuda_download(app: AppHandle) -> Result<(), ApiError> {
    if !cuda::supported() {
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
