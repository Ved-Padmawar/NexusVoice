//! Model commands: catalog, downloads, the active model, dictation language,
//! idle unload and the hardware profile.

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::mirror::Committed;
use crate::models::cache::EngineCache;
use crate::models::catalog::{self, ModelEntry, Pipeline};
use crate::models::cuda::{CudaPack, PackState};
use crate::models::idle::ModelUnload;
use crate::models::library::ModelLibrary;
use crate::models::provider::canonical_override;
use crate::settings::Settings;

use super::error::ApiError;

fn unknown_model() -> ApiError {
    ApiError::new("invalid_variant", "That model isn't available.")
}

fn catalog_entry(id: &str) -> Result<&'static ModelEntry, ApiError> {
    canonical_override(id)
        .and_then(catalog::find)
        .ok_or_else(unknown_model)
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CatalogModel {
    pub id: String,
    pub display_name: String,
    pub family: String,
    /// Every decode path this model supports.
    pub pipelines: Vec<Pipeline>,
    pub default_pipeline: Pipeline,
    pub size_bytes: u64,
    pub multilingual: bool,
    pub description: String,
    pub detail: String,
}

/// Every model the app offers. Fixed for the life of the app.
#[tauri::command]
#[specta::specta]
pub fn get_model_catalog() -> Vec<CatalogModel> {
    catalog::all()
        .iter()
        .map(|entry| CatalogModel {
            id: entry.id.clone(),
            display_name: entry.display_name.clone(),
            family: entry.family.clone(),
            pipelines: entry.pipelines.clone(),
            default_pipeline: entry.default_pipeline,
            size_bytes: entry.size_bytes,
            multilingual: entry.multilingual,
            description: entry.description.clone(),
            detail: entry.detail.clone(),
        })
        .collect()
}

/// Look at the model files again, for a view that lists them as it opens.
#[tauri::command]
#[specta::specta]
pub async fn rescan_models(app: AppHandle) -> Result<Committed, ApiError> {
    tauri::async_runtime::spawn_blocking(move || app.state::<ModelLibrary>().rescan())
        .await
        .map_err(|e| ApiError::new("io_error", e.to_string()))
}

/// Delete a downloaded model, the active one included.
#[tauri::command]
#[specta::specta]
pub async fn delete_model(app: AppHandle, variant: String) -> Result<Committed, ApiError> {
    let entry = catalog_entry(&variant)?;
    tauri::async_runtime::spawn_blocking(move || {
        let library = app.state::<ModelLibrary>();
        if !library.is_downloaded(entry) {
            return Err(ApiError::new("not_found", "That model isn't downloaded."));
        }
        library.delete(&app, entry).map_err(|e| {
            if e.kind() == std::io::ErrorKind::ResourceBusy {
                ApiError::new(
                    "io_error",
                    "That model is in use by a recording. Try again when it finishes.",
                )
            } else {
                e.into()
            }
        })
    })
    .await
    .map_err(|e| ApiError::new("io_error", e.to_string()))?
}

/// Make a model on disk the active one.
#[tauri::command]
#[specta::specta]
pub async fn set_model_override(app: AppHandle, variant: String) -> Result<Committed, ApiError> {
    let entry = catalog_entry(&variant)?;
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<ModelLibrary>()
            .select(&app, entry)?
            .ok_or_else(|| ApiError::new("not_found", "Download this model first."))
    })
    .await
    .map_err(|e| ApiError::new("io_error", e.to_string()))?
}

#[tauri::command]
#[specta::specta]
pub fn set_language(
    settings: State<'_, Settings>,
    engine: State<'_, EngineCache>,
    code: Option<String>,
) -> Result<Committed, ApiError> {
    let committed = settings.set_supported_language(code)?.ok_or_else(|| {
        ApiError::new(
            "invalid_language",
            "This model doesn't support that language.",
        )
    })?;
    engine.apply_language(&settings);
    Ok(committed)
}

/// Takes effect at the idle watcher's next check.
#[tauri::command]
#[specta::specta]
pub fn set_model_unload(
    settings: State<'_, Settings>,
    policy: ModelUnload,
) -> Result<Committed, ApiError> {
    Ok(settings.set_model_unload(policy)?)
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HardwareProfileResponse {
    pub gpu_name: String,
    /// Backend in use: the loaded model's, else the one it is expected to get.
    pub execution_provider: String,
    pub vram_gb: f32,
    pub ram_gb: f32,
    pub recommended_model: String,
}

#[tauri::command]
#[specta::specta]
pub async fn get_hardware_profile(app: AppHandle) -> HardwareProfileResponse {
    let loaded_backend = app.state::<EngineCache>().backend();
    let cuda_installed = app.state::<CudaPack>().state() == PackState::Installed;
    // The first probe makes blocking syscalls; keep them off the main thread.
    tauri::async_runtime::spawn_blocking(move || hardware_profile(loaded_backend, cuda_installed))
        .await
        .expect("hardware probe panicked")
}

fn hardware_profile(
    loaded_backend: Option<String>,
    cuda_installed: bool,
) -> HardwareProfileResponse {
    use crate::models::{cuda, provider::recommend_model};

    let hw = crate::platform::hardware::cached_profile();
    // With no model loaded: what the next load will get. After a GPU failure
    // that is the CPU; NVIDIA runs on Vulkan unless the CUDA pack can serve it.
    let execution_provider = loaded_backend.unwrap_or_else(|| {
        if crate::engine::gpu_unavailable() {
            return "cpu".to_string();
        }
        match hw.execution_provider.as_str() {
            "cuda" if !(cuda_installed && cuda::supported()) => "vulkan".to_string(),
            other => other.to_string(),
        }
    });

    HardwareProfileResponse {
        gpu_name: hw.gpu_type.clone(),
        execution_provider,
        vram_gb: hw.vram_gb,
        ram_gb: hw.ram_gb,
        recommended_model: recommend_model().display_name.clone(),
    }
}

/// With `activate`, the model becomes the active one once it is on disk.
#[tauri::command]
#[specta::specta]
pub async fn start_model_download(
    app: AppHandle,
    id: String,
    activate: bool,
) -> Result<Committed, ApiError> {
    let entry = catalog::find(&id)
        .ok_or_else(|| ApiError::new("unknown_model", "That model isn't available."))?;
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<ModelLibrary>()
            .start_download(&app, entry, activate)
    })
    .await
    .map_err(|e| ApiError::new("io_error", e.to_string()))
}

/// A cancel pressed in the last moments of a transfer arrives after the file
/// is published; the download then stands.
#[tauri::command]
#[specta::specta]
pub fn cancel_model_download(library: State<'_, ModelLibrary>, id: String) {
    library.cancel_download(&id);
}
