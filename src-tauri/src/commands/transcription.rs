//! Transcription commands: thin endpoints over `crate::transcription`.

use std::sync::{atomic::Ordering, Arc};

use tauri::{AppHandle, Emitter, State};

use crate::state::{AppState, RecordingMode, SessionPhase};
use crate::transcription::{self, service::FinalizeContext};

use super::error::ApiError;

#[tauri::command]
pub async fn start_transcription(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, ApiError> {
    if state.current_user_id().await.is_none() {
        return Err(ApiError::new(
            "unauthenticated",
            "must be logged in to transcribe",
        ));
    }

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

    transcription::start_capture(&app, &state);

    // Don't report "started" until the mic is actually delivering audio —
    // otherwise the first ~100–300ms of speech is lost to cpal warm-up.
    wait_for_capture_ready(&state).await;

    Ok(true)
}

#[tauri::command]
pub async fn stop_transcription(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, ApiError> {
    if state.recording_mode() == RecordingMode::Dictation {
        return Err(ApiError::new(
            "dictation_running",
            "dictation is running; use commit_dictation or cancel_dictation",
        ));
    }

    finalize_current_recording(app, &state).await
}

#[tauri::command]
pub async fn start_dictation(app: AppHandle, state: State<'_, AppState>) -> Result<bool, ApiError> {
    if state.current_user_id().await.is_none() {
        return Err(ApiError::new(
            "unauthenticated",
            "must be logged in to transcribe",
        ));
    }

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

    transcription::start_capture(&app, &state);

    // Don't report "started" until the mic is actually delivering audio —
    // otherwise the first ~100–300ms of speech is lost to cpal warm-up.
    wait_for_capture_ready(&state).await;

    Ok(true)
}

#[tauri::command]
pub async fn pause_dictation(state: State<'_, AppState>) -> Result<bool, ApiError> {
    ensure_dictation_active(&state)?;

    if !state.transition_session_phase(SessionPhase::Recording, SessionPhase::Paused) {
        return Err(ApiError::new(
            "dictation_not_recording",
            "dictation is not recording",
        ));
    }

    state.capture_paused.store(true, Ordering::SeqCst);
    Ok(true)
}

#[tauri::command]
pub async fn resume_dictation(state: State<'_, AppState>) -> Result<bool, ApiError> {
    ensure_dictation_active(&state)?;

    if !state.transition_session_phase(SessionPhase::Paused, SessionPhase::Recording) {
        return Err(ApiError::new(
            "dictation_not_paused",
            "dictation is not paused",
        ));
    }

    state.capture_paused.store(false, Ordering::SeqCst);
    Ok(true)
}

#[tauri::command]
pub async fn commit_dictation(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, ApiError> {
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

#[tauri::command]
pub async fn cancel_dictation(state: State<'_, AppState>) -> Result<bool, ApiError> {
    ensure_dictation_active(&state)?;

    state.capture_paused.store(false, Ordering::SeqCst);
    let _ = state.try_stop_transcription();
    wait_for_capture_done(&state).await;
    clear_audio_buffer(&state);
    state.reset_recording_session();

    Ok(true)
}

fn clear_audio_buffer(state: &AppState) {
    let mut buf = state
        .audio_buffer
        .lock()
        .expect("audio_buffer lock poisoned");
    buf.clear();
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

/// Block until the capture callback has delivered its first sample (or the
/// timeout elapses). Lets `start_transcription` report "started" only once the
/// mic is actually producing audio, so leading speech isn't clipped during the
/// cpal stream warm-up. The timeout guards against a device that never delivers.
async fn wait_for_capture_ready(state: &AppState) {
    const READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);
    let capture_ready = Arc::clone(&state.capture_ready);
    tauri::async_runtime::spawn_blocking(move || {
        let (lock, cvar) = &*capture_ready;
        let _ = cvar.wait_timeout_while(
            lock.lock().expect("capture_ready lock poisoned"),
            READY_TIMEOUT,
            |ready| !*ready,
        );
    })
    .await
    .ok();
}

async fn wait_for_capture_done(state: &AppState) {
    let capture_done = Arc::clone(&state.capture_done);
    tauri::async_runtime::spawn_blocking(move || {
        let (lock, cvar) = &*capture_done;
        let _guard = cvar
            .wait_while(lock.lock().expect("capture_done lock poisoned"), |done| {
                !*done
            })
            .expect("capture_done condvar poisoned");
    })
    .await
    .ok();
}

async fn finalize_current_recording(app: AppHandle, state: &AppState) -> Result<bool, ApiError> {
    const MIN_DURATION_SECS: f64 = 0.5;

    if !state.try_stop_transcription() {
        return Err(ApiError::new(
            "transcription_not_running",
            "transcription not running",
        ));
    }

    wait_for_capture_done(state).await;

    let (samples, captured_rate) = {
        let mut buf = state
            .audio_buffer
            .lock()
            .expect("audio_buffer lock poisoned");
        let rate = *state
            .native_sample_rate
            .lock()
            .expect("native_sample_rate lock poisoned");
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
        return Ok(false);
    }

    let engine = match state.get_or_load_engine().await {
        Ok(e) => e,
        Err(e) => {
            log::error!("engine load failed: {e}");
            state.reset_recording_session();
            let _ = app.emit("transcription-error", format!("model not ready: {e}"));
            return Ok(false);
        }
    };

    let cfg = state.load_format_config();
    let format = cfg.is_usable().then_some(cfg);

    transcription::spawn_finalize(
        app,
        FinalizeContext {
            pool: state.db().await.clone(),
            dict_cache: Arc::clone(&state.dict_cache),
            engine,
            engine_cache: Arc::clone(&state.engine),
            samples,
            captured_rate,
            duration_seconds,
            format,
        },
    );

    state.reset_recording_session();

    Ok(true)
}
