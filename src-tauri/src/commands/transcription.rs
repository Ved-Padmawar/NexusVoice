//! Transcription commands: thin endpoints over `crate::transcription`.

use std::sync::{atomic::Ordering, Arc};

use tauri::{AppHandle, Emitter, State};

use crate::state::{lock_recovering, AppState, RecordingMode, SessionPhase};
use crate::transcription::{self, service::FinalizeContext};

use super::error::ApiError;

#[tauri::command]
#[specta::specta]
pub async fn start_transcription(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), ApiError> {
    if !state.try_start_transcription() {
        return Err(ApiError::new(
            "transcription_already_running",
            "transcription already running",
        ));
    }

    state.set_recording_mode(RecordingMode::PushToTalk);
    state.set_session_phase(SessionPhase::Idle);
    state.capture_paused.store(false, Ordering::SeqCst);
    clear_audio_buffer(&state);
    state.begin_stream_session();

    transcription::start_capture(&app, &state);

    await_capture_started(&app, &state).await
}

#[tauri::command]
#[specta::specta]
pub async fn stop_transcription(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), ApiError> {
    if state.recording_mode() == RecordingMode::Dictation {
        return Err(ApiError::new(
            "dictation_running",
            "dictation is running; use commit_dictation",
        ));
    }

    finalize_current_recording(app, &state).await
}

#[tauri::command]
#[specta::specta]
pub async fn start_dictation(app: AppHandle, state: State<'_, AppState>) -> Result<(), ApiError> {
    if !state.try_start_transcription() {
        return Err(ApiError::new(
            "transcription_already_running",
            "transcription already running",
        ));
    }

    state.set_recording_mode(RecordingMode::Dictation);
    state.set_session_phase(SessionPhase::Recording);
    state.capture_paused.store(false, Ordering::SeqCst);
    clear_audio_buffer(&state);
    state.begin_stream_session();

    transcription::start_capture(&app, &state);

    await_capture_started(&app, &state).await
}

#[tauri::command]
#[specta::specta]
pub async fn pause_dictation(state: State<'_, AppState>) -> Result<(), ApiError> {
    ensure_dictation_active(&state)?;

    if !state.transition_session_phase(SessionPhase::Recording, SessionPhase::Paused) {
        return Err(ApiError::new(
            "dictation_not_recording",
            "dictation is not recording",
        ));
    }

    state.capture_paused.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn resume_dictation(state: State<'_, AppState>) -> Result<(), ApiError> {
    ensure_dictation_active(&state)?;

    if !state.transition_session_phase(SessionPhase::Paused, SessionPhase::Recording) {
        return Err(ApiError::new(
            "dictation_not_paused",
            "dictation is not paused",
        ));
    }

    state.capture_paused.store(false, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn commit_dictation(app: AppHandle, state: State<'_, AppState>) -> Result<(), ApiError> {
    ensure_dictation_active(&state)?;

    match state.session_phase() {
        SessionPhase::Recording | SessionPhase::Paused => {
            state.set_session_phase(SessionPhase::Finalizing);
            state.capture_paused.store(false, Ordering::SeqCst);
            let result = finalize_current_recording(app, &state).await;
            state.reset_recording_session();
            result
        }
        SessionPhase::Finalizing => Err(ApiError::new(
            "dictation_finalizing",
            "dictation is already finalizing",
        )),
        SessionPhase::Idle => Err(ApiError::new(
            "dictation_not_running",
            "dictation is not running",
        )),
    }
}

fn clear_audio_buffer(state: &AppState) {
    lock_recovering(&state.audio_buffer).clear();
}

fn ensure_dictation_active(state: &AppState) -> Result<(), ApiError> {
    if !state.transcription_running.load(Ordering::SeqCst)
        || state.recording_mode() != RecordingMode::Dictation
    {
        return Err(ApiError::new(
            "dictation_not_running",
            "dictation is not running",
        ));
    }
    Ok(())
}

/// Report "started" only once the mic is producing audio. A warm stream
/// delivers on the next callback; a cold open takes longer, and the timeout
/// guards a device that never delivers. A failed open aborts the recording.
async fn await_capture_started(app: &AppHandle, state: &AppState) -> Result<(), ApiError> {
    const READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
    let ready = state.mic.ready_signal();
    tauri::async_runtime::spawn_blocking(move || {
        let (lock, cvar) = &*ready;
        let _ = cvar.wait_timeout_while(lock_recovering(lock), READY_TIMEOUT, |ready| !*ready);
    })
    .await
    .ok();

    let Some(e) = state.mic.take_error() else {
        return Ok(());
    };
    state.try_stop_transcription();
    state.reset_recording_session();
    let _ = app.emit(
        "transcription-error",
        crate::audio::error::friendly_capture_error(&e),
    );
    Err(ApiError::new("capture_failed", e))
}

/// Stop capturing after a post-roll, leaving the device open for next time.
async fn stop_capture(state: &AppState) {
    let mic = Arc::clone(&state.mic);
    tauri::async_runtime::spawn_blocking(move || mic.disarm_after_post_roll())
        .await
        .ok();
}

/// The streaming worker writes `streamed_text` only after `running` goes
/// false, so reading it straight after capture stops loses the tail.
async fn wait_for_stream_done(state: &AppState) {
    const STREAM_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
    let stream_done = Arc::clone(&state.stream_done);
    tauri::async_runtime::spawn_blocking(move || {
        let (lock, cvar) = &*stream_done;
        let (_guard, timeout) = cvar
            .wait_timeout_while(lock_recovering(lock), STREAM_TIMEOUT, |done| !*done)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timeout.timed_out() {
            log::warn!("stream-done signal missed — finalizing without the streamed tail");
        }
    })
    .await
    .ok();
}

async fn finalize_current_recording(app: AppHandle, state: &AppState) -> Result<(), ApiError> {
    const MIN_DURATION_SECS: f64 = 0.5;

    if !state.try_stop_transcription() {
        return Err(ApiError::new(
            "transcription_not_running",
            "transcription not running",
        ));
    }

    stop_capture(state).await;
    wait_for_stream_done(state).await;

    // Session first, audio buffer second — the lock order the stream worker
    // uses. Taking the session waits out any in-flight stream decode.
    let session = state.take_stream_session();
    let streamed = state.take_streamed_text();

    let (samples, captured_rate) = {
        let mut buf = lock_recovering(&state.audio_buffer);
        let rate = *lock_recovering(&state.native_sample_rate);
        (std::mem::take(&mut *buf), rate)
    };

    #[allow(clippy::cast_precision_loss)]
    let duration_seconds: Option<f64> = if captured_rate > 0 && !samples.is_empty() {
        Some(samples.len() as f64 / f64::from(captured_rate))
    } else {
        None
    };

    #[allow(clippy::cast_precision_loss)]
    let too_short = samples.is_empty()
        || (captured_rate > 0
            && (samples.len() as f64 / f64::from(captured_rate)) < MIN_DURATION_SECS);
    if too_short {
        state.reset_recording_session();
        let _ = app.emit("transcription-complete", "");
        return Ok(());
    }

    let engine = match state.get_or_load_engine().await {
        Ok(e) => e,
        Err(e) => {
            log::error!("engine load failed: {e}");
            state.reset_recording_session();
            let _ = app.emit("transcription-error", format!("model not ready: {e}"));
            return Ok(());
        }
    };

    let cfg = state.load_format_config();
    let format = cfg.is_usable().then_some(cfg);

    transcription::spawn_finalize(
        app,
        FinalizeContext {
            pool: state.db().await?.clone(),
            dict_cache: Arc::clone(&state.dict_cache),
            engine,
            engine_cache: Arc::clone(&state.engine),
            session,
            streamed,
            samples,
            captured_rate,
            duration_seconds,
            format,
            focus: state.take_focus_target(),
        },
    );

    state.reset_recording_session();

    Ok(())
}

// ---------------------------------------------------------------------------
// Input device selection
// ---------------------------------------------------------------------------

/// A selectable microphone. `is_default` marks the OS default; `is_selected`
/// marks the user's saved preference (or the default when none is saved).
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InputDeviceInfo {
    pub name: String,
    pub is_default: bool,
    pub is_selected: bool,
}

/// List usable input devices, excluding loopback/virtual endpoints. The first
/// entry the UI shows is "Default"; these are the named alternatives.
#[tauri::command]
#[specta::specta]
pub fn list_input_devices(state: State<'_, AppState>) -> Vec<InputDeviceInfo> {
    let selected = state.load_input_device();
    crate::audio::list_input_devices()
        .into_iter()
        .map(|d| InputDeviceInfo {
            is_selected: selected.as_deref() == Some(d.name.as_str()),
            is_default: d.is_default,
            name: d.name,
        })
        .collect()
}

/// Choose an input device by name. Pass `None` (or omit) to follow the OS
/// default. Takes effect on the next recording — the current one is unaffected.
#[tauri::command]
#[specta::specta]
pub fn set_input_device(state: State<'_, AppState>, name: Option<String>) -> Result<(), ApiError> {
    match name {
        Some(name) if !name.trim().is_empty() => {
            state
                .save_input_device(name.trim())
                .map_err(|e| ApiError::new("io_error", e.to_string()))?;
            state.mic.close();
            Ok(())
        }
        _ => {
            state.delete_input_device();
            state.mic.close();
            Ok(())
        }
    }
}
