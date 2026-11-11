//! Recording commands and microphone selection.

use tauri::{AppHandle, Manager, State};

use crate::models::cache::EngineCache;
use crate::session::{
    recorder::{Action, RecordingSnapshot},
    Recorder,
};
use crate::settings::Settings;

use super::error::ApiError;

#[tauri::command]
#[specta::specta]
pub async fn pause_dictation(
    app: AppHandle,
    recorder: State<'_, Recorder>,
) -> Result<(), ApiError> {
    Ok(recorder.request(&app, Action::Pause).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn resume_dictation(
    app: AppHandle,
    recorder: State<'_, Recorder>,
) -> Result<(), ApiError> {
    Ok(recorder.request(&app, Action::Resume).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn commit_dictation(
    app: AppHandle,
    recorder: State<'_, Recorder>,
) -> Result<(), ApiError> {
    Ok(recorder.request(&app, Action::Commit).await?)
}

/// A selectable microphone. `is_default` marks the OS default; `is_selected`
/// marks the saved preference.
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InputDeviceInfo {
    /// What to save: identical mics share a name.
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub is_selected: bool,
}

/// Usable input devices, excluding loopback and virtual endpoints. The UI
/// lists "Default" first; these are the named alternatives.
#[tauri::command]
#[specta::specta]
pub async fn list_input_devices(app: AppHandle) -> Vec<InputDeviceInfo> {
    let selected = app.state::<Settings>().input_device();
    // Enumerating audio endpoints blocks on the OS: off the main thread.
    let devices = tauri::async_runtime::spawn_blocking(crate::audio::list_input_devices)
        .await
        .unwrap_or_default();
    devices
        .into_iter()
        .map(|d| InputDeviceInfo {
            is_selected: selected.as_deref() == Some(d.id.as_str()),
            is_default: d.is_default,
            id: d.id,
            name: d.name,
        })
        .collect()
}

/// Choose an input device by id; `None` follows the OS default.
#[tauri::command]
#[specta::specta]
pub fn set_input_device(
    app: AppHandle,
    settings: State<'_, Settings>,
    recorder: State<'_, Recorder>,
    id: Option<String>,
) -> Result<(), ApiError> {
    let id = id.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    settings.set_input_device(id)?;
    recorder.change_mic(&app);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_keep_mic_open(settings: State<'_, Settings>) -> bool {
    settings.keep_mic_open()
}

#[tauri::command]
#[specta::specta]
pub fn set_keep_mic_open(
    settings: State<'_, Settings>,
    recorder: State<'_, Recorder>,
    keep: bool,
) -> Result<(), ApiError> {
    settings.set_keep_mic_open(keep)?;
    recorder.set_keep_mic_open(keep);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn get_recording_state(
    recorder: State<'_, Recorder>,
    engine: State<'_, EngineCache>,
) -> RecordingSnapshot {
    recorder.snapshot(&engine)
}
