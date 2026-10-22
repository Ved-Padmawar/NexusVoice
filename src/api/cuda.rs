//! Optional CUDA backend: status, download and removal of the CUDA pack.

use std::sync::Arc;

use crate::events::{self, CudaDownload, Event};
use crate::inference::{cuda, downloader::CANCELLED};
use crate::state;

use super::error::ApiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CudaStatus {
    /// An NVIDIA GPU on a platform the pack is built for; CUDA is hidden otherwise.
    pub supported: bool,
    pub pack: cuda::PackState,
    /// Percent of the running download; `None` when idle.
    pub download_progress: Option<u8>,
}

pub fn get_cuda_status() -> CudaStatus {
    let state = state::get();
    CudaStatus {
        supported: cuda::supported(),
        pack: cuda::pack_state(&state.app_data_dir),
        download_progress: state.cuda_download.progress(),
    }
}

/// Fetch the CUDA pack in the background, reported through `cuda-download-*` events.
pub fn start_cuda_download() -> Result<(), ApiError> {
    let state = state::get();
    if !cuda::supported() {
        return Err(ApiError::new("unsupported", "CUDA needs an NVIDIA GPU"));
    }
    let download = Arc::clone(&state.cuda_download);
    let app_data_dir = state.app_data_dir.clone();

    tokio::spawn(async move {
        let status = match download.run(&app_data_dir).await {
            Ok(()) => CudaDownload::Complete,
            Err(e) if e == CANCELLED => CudaDownload::Cancelled,
            Err(e) => {
                log::warn!("CUDA pack download failed: {e}");
                CudaDownload::Failed(e)
            }
        };
        events::emit(Event::Cuda(status));
    });
    Ok(())
}

pub fn cancel_cuda_download() {
    state::get().cuda_download.cancel();
}

/// Remove the pack; one in use is deleted at the next start.
pub fn remove_cuda_pack() -> Result<(), ApiError> {
    let state = state::get();
    state.cuda_download.cancel();
    cuda::remove(&state.app_data_dir).map_err(|e| ApiError::new("io_error", e.to_string()))
}
