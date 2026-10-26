//! Model commands: catalog, downloads, the active model, dictation language,
//! idle unload and the hardware profile.

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::inference::cache::EngineCache;
use crate::inference::catalog::{self, ModelEntry, Pipeline};
use crate::inference::idle::ModelUnload;
use crate::inference::language::{self, AUTO};
use crate::inference::library::{DownloadStatus, ModelLibrary};
use crate::inference::provider::canonical_override;
use crate::settings::Settings;

use super::error::ApiError;

fn unknown_model() -> ApiError {
    ApiError::new("invalid_variant", "unknown model id")
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
    pub downloaded: bool,
    pub is_active: bool,
}

/// The catalog with per-model download state, for the model picker.
#[tauri::command]
#[specta::specta]
pub fn get_model_catalog(
    settings: State<'_, Settings>,
    library: State<'_, ModelLibrary>,
) -> Vec<CatalogModel> {
    let active = ModelLibrary::active(&settings);
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
            downloaded: library.is_downloaded(entry),
            is_active: entry.id == active.id,
        })
        .collect()
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DownloadedModel {
    pub variant: String,
    pub display_name: String,
    pub size_bytes: u64,
    pub is_active: bool,
}

#[tauri::command]
#[specta::specta]
pub fn get_downloaded_models(
    settings: State<'_, Settings>,
    library: State<'_, ModelLibrary>,
) -> Vec<DownloadedModel> {
    let active = ModelLibrary::active(&settings);
    catalog::all()
        .iter()
        .filter_map(|entry| {
            let size_bytes = library.path(entry).metadata().ok()?.len();
            Some(DownloadedModel {
                variant: entry.id.clone(),
                display_name: entry.display_name.clone(),
                size_bytes,
                is_active: entry.id == active.id,
            })
        })
        .collect()
}

/// Delete a downloaded model, the active one included.
#[tauri::command]
#[specta::specta]
pub fn delete_model(
    app: AppHandle,
    library: State<'_, ModelLibrary>,
    variant: String,
) -> Result<(), ApiError> {
    let entry = catalog_entry(&variant)?;
    if !library.is_downloaded(entry) {
        return Err(ApiError::new("not_found", "model file not found"));
    }
    Ok(library.delete(&app, entry)?)
}

/// Make a model the active one.
#[tauri::command]
#[specta::specta]
pub fn set_model_override(app: AppHandle, variant: String) -> Result<(), ApiError> {
    Ok(ModelLibrary::select(&app, catalog_entry(&variant)?)?)
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LanguageOption {
    /// ISO code, or `auto` for the detect-per-decode sentinel.
    pub code: String,
    pub name: String,
    pub is_selected: bool,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LanguageSettings {
    /// Whether the active model handles anything but English.
    pub supported: bool,
    /// Empty when `supported` is false.
    pub options: Vec<LanguageOption>,
}

/// The picker's list with the active choice marked, `auto` first. Codes come
/// from the catalog, so the list is complete whether or not a model has loaded.
#[tauri::command]
#[specta::specta]
pub fn get_language_options(settings: State<'_, Settings>) -> LanguageSettings {
    let model = ModelLibrary::active(&settings);
    if !model.multilingual {
        return LanguageSettings {
            supported: false,
            options: Vec::new(),
        };
    }

    let advertised = &model.languages;
    let saved = settings.language();
    let active = language::resolve(saved.as_deref());

    // Region shown only where a language has several locales.
    let mut named: Vec<(String, String)> = advertised
        .iter()
        .map(|code| {
            let base = language::primary_of(code);
            let regioned = advertised
                .iter()
                .filter(|c| language::primary_of(c) == base)
                .count()
                > 1;
            (code.clone(), language::display_name(code, regioned))
        })
        .collect();
    named.sort_by(|a, b| {
        a.1.to_lowercase()
            .cmp(&b.1.to_lowercase())
            .then_with(|| a.0.cmp(&b.0))
    });

    let options = std::iter::once(LanguageOption {
        code: AUTO.to_string(),
        name: "Auto-detect".to_string(),
        is_selected: active.is_none(),
    })
    .chain(named.into_iter().map(|(code, name)| LanguageOption {
        is_selected: active == Some(code.as_str()),
        code,
        name,
    }))
    .collect();

    LanguageSettings {
        supported: true,
        options,
    }
}

#[tauri::command]
#[specta::specta]
pub fn set_language(
    app: AppHandle,
    settings: State<'_, Settings>,
    code: Option<String>,
) -> Result<(), ApiError> {
    let model = ModelLibrary::active(&settings);
    if let Some(c) = code.as_deref() {
        if c != AUTO && !model.languages.iter().any(|l| l == c) {
            return Err(ApiError::new("invalid_language", "unsupported language"));
        }
    }
    settings.set_language(code)?;
    EngineCache::apply_language(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_model_unload(settings: State<'_, Settings>) -> ModelUnload {
    settings.model_unload()
}

/// Takes effect at the idle watcher's next check.
#[tauri::command]
#[specta::specta]
pub fn set_model_unload(
    settings: State<'_, Settings>,
    policy: ModelUnload,
) -> Result<(), ApiError> {
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
pub fn get_hardware_profile(engine: State<'_, EngineCache>) -> HardwareProfileResponse {
    use crate::inference::{cuda, provider::recommend_model};

    let hw = crate::hardware::cached_profile();
    // Before a model loads, NVIDIA runs on Vulkan unless a CUDA device registered.
    let execution_provider =
        engine
            .backend()
            .unwrap_or_else(|| match hw.execution_provider.as_str() {
                "cuda" if !cuda::registered() => "vulkan".to_string(),
                other => other.to_string(),
            });

    HardwareProfileResponse {
        gpu_name: hw.gpu_type.clone(),
        execution_provider,
        vram_gb: hw.vram_gb,
        ram_gb: hw.ram_gb,
        recommended_model: recommend_model().display_name.clone(),
    }
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfoResponse {
    pub downloaded: bool,
    pub downloading: bool,
    pub model_name: String,
}

#[tauri::command]
#[specta::specta]
pub fn get_model_info(
    settings: State<'_, Settings>,
    library: State<'_, ModelLibrary>,
) -> ModelInfoResponse {
    let entry = ModelLibrary::active(&settings);
    ModelInfoResponse {
        downloaded: library.is_downloaded(entry),
        downloading: library.is_downloading(&entry.id),
        model_name: entry.display_name.clone(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn start_model_download(
    app: AppHandle,
    library: State<'_, ModelLibrary>,
    id: String,
) -> Result<(), ApiError> {
    let entry = catalog::find(&id)
        .ok_or_else(|| ApiError::new("unknown_model", "no such model in the catalog"))?;
    library.start_download(&app, entry);
    Ok(())
}

/// A cancel pressed in the last moments of a transfer arrives after the file
/// is published; the download then stands, reported by its complete event.
#[tauri::command]
#[specta::specta]
pub fn cancel_model_download(library: State<'_, ModelLibrary>, id: String) {
    library.cancel_download(&id);
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActiveDownload {
    pub id: String,
    pub status: DownloadStatus,
    pub progress: u8,
    pub error: Option<String>,
}

/// Every download queued, running or holding an error, so the UI can
/// rehydrate after a reload without waiting for the next event.
#[tauri::command]
#[specta::specta]
pub fn get_active_downloads(library: State<'_, ModelLibrary>) -> Vec<ActiveDownload> {
    library
        .downloads()
        .into_iter()
        .map(|(id, status, progress, error)| ActiveDownload {
            id,
            status,
            progress,
            error,
        })
        .collect()
}
