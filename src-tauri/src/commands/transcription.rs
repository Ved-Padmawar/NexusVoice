//! Recording commands and microphone selection.

use tauri::{AppHandle, State};

use crate::settings::Settings;
use crate::transcription::{Mode, Recorder};

use super::error::ApiError;

#[tauri::command]
#[specta::specta]
pub async fn start_transcription(
    app: AppHandle,
    recorder: State<'_, Recorder>,
) -> Result<(), ApiError> {
    Ok(recorder.start(&app, Mode::PushToTalk).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn stop_transcription(
    app: AppHandle,
    recorder: State<'_, Recorder>,
) -> Result<(), ApiError> {
    Ok(recorder.stop(&app).await?)
}

#[tauri::command]
#[specta::specta]
pub async fn start_dictation(
    app: AppHandle,
    recorder: State<'_, Recorder>,
) -> Result<(), ApiError> {
    Ok(recorder.start(&app, Mode::Dictation).await?)
}

#[tauri::command]
#[specta::specta]
pub fn pause_dictation(recorder: State<'_, Recorder>) -> Result<(), ApiError> {
    Ok(recorder.pause()?)
}

#[tauri::command]
#[specta::specta]
pub fn resume_dictation(recorder: State<'_, Recorder>) -> Result<(), ApiError> {
    Ok(recorder.resume()?)
}

#[tauri::command]
#[specta::specta]
pub async fn commit_dictation(
    app: AppHandle,
    recorder: State<'_, Recorder>,
) -> Result<(), ApiError> {
    Ok(recorder.commit(&app).await?)
}

/// A selectable microphone. `is_default` marks the OS default; `is_selected`
/// marks the saved preference.
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InputDeviceInfo {
    pub name: String,
    pub is_default: bool,
    pub is_selected: bool,
}

/// Usable input devices, excluding loopback and virtual endpoints. The UI
/// lists "Default" first; these are the named alternatives.
#[tauri::command]
#[specta::specta]
pub fn list_input_devices(settings: State<'_, Settings>) -> Vec<InputDeviceInfo> {
    let selected = settings.input_device();
    crate::audio::list_input_devices()
        .into_iter()
        .map(|d| InputDeviceInfo {
            is_selected: selected.as_deref() == Some(d.name.as_str()),
            is_default: d.is_default,
            name: d.name,
        })
        .collect()
}

/// Choose an input device by name; `None` follows the OS default. Takes
/// effect on the next recording.
#[tauri::command]
#[specta::specta]
pub fn set_input_device(
    settings: State<'_, Settings>,
    recorder: State<'_, Recorder>,
    name: Option<String>,
) -> Result<(), ApiError> {
    let name = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    settings.set_input_device(name)?;
    recorder.close_mic_if_idle();
    Ok(())
}
