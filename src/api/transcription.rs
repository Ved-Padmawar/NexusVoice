//! Recording lifecycle, serialized by the [`controller`](crate::transcription::controller).

use std::sync::{atomic::Ordering, Arc};

use crate::events::{self, Event};
use crate::state::{self, lock_recovering, AppState, RecordingMode, SessionPhase};
use crate::transcription::controller::{self, Input};
use crate::transcription::{self, service::FinalizeContext};

use super::error::ApiError;

pub async fn start_transcription() -> Result<(), ApiError> {
    let state = state::get();
    if !state.try_start_transcription() {
        return Err(ApiError::new(
            "transcription_already_running",
            "transcription already running",
        ));
    }

    state.set_recording_mode(RecordingMode::PushToTalk);
    state.set_session_phase(SessionPhase::Idle);
    state.capture_paused.store(false, Ordering::SeqCst);
    clear_audio_buffer(state);
    state.begin_stream_session();

    transcription::start_capture(state);

    await_capture_started(state).await
}

pub async fn stop_transcription() -> Result<(), ApiError> {
    let state = state::get();
    if state.recording_mode() == RecordingMode::Dictation {
        return Err(ApiError::new(
            "dictation_running",
            "dictation is running; use commit_dictation",
        ));
    }

    finalize_current_recording(state).await
}

pub async fn start_dictation() -> Result<(), ApiError> {
    let state = state::get();
    if !state.try_start_transcription() {
        return Err(ApiError::new(
            "transcription_already_running",
            "transcription already running",
        ));
    }

    state.set_recording_mode(RecordingMode::Dictation);
    state.set_session_phase(SessionPhase::Recording);
    state.capture_paused.store(false, Ordering::SeqCst);
    clear_audio_buffer(state);
    state.begin_stream_session();

    transcription::start_capture(state);

    await_capture_started(state).await
}

pub fn pause_dictation() -> Result<(), ApiError> {
    let state = state::get();
    ensure_dictation_active(state)?;

    if !state.transition_session_phase(SessionPhase::Recording, SessionPhase::Paused) {
        return Err(ApiError::new(
            "dictation_not_recording",
            "dictation is not recording",
        ));
    }

    state.capture_paused.store(true, Ordering::SeqCst);
    Ok(())
}

pub fn resume_dictation() -> Result<(), ApiError> {
    let state = state::get();
    ensure_dictation_active(state)?;

    if !state.transition_session_phase(SessionPhase::Paused, SessionPhase::Recording) {
        return Err(ApiError::new(
            "dictation_not_paused",
            "dictation is not paused",
        ));
    }

    state.capture_paused.store(false, Ordering::SeqCst);
    Ok(())
}

pub async fn commit_dictation() -> Result<(), ApiError> {
    let state = state::get();
    ensure_dictation_active(state)?;

    match state.session_phase() {
        SessionPhase::Recording | SessionPhase::Paused => {
            state.set_session_phase(SessionPhase::Finalizing);
            state.capture_paused.store(false, Ordering::SeqCst);
            let result = finalize_current_recording(state).await;
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

/// Finalize leaves the buffer without capacity; reserve ~30 s so the audio
/// callback doesn't reallocate under the lock.
fn clear_audio_buffer(state: &AppState) {
    const RESERVE_SECS: usize = 30;
    let rate = *lock_recovering(&state.native_sample_rate) as usize;
    let mut buf = lock_recovering(&state.audio_buffer);
    buf.clear();
    buf.reserve(rate * RESERVE_SECS);
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

/// Report "started" only once the mic delivers audio; a failed or silent
/// device aborts the recording.
async fn await_capture_started(state: &AppState) -> Result<(), ApiError> {
    const READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
    let ready = state.mic.ready_signal();
    let timed_out = tokio::task::spawn_blocking(move || {
        let (lock, cvar) = &*ready;
        let (_guard, wait) = cvar
            .wait_timeout_while(lock_recovering(lock), READY_TIMEOUT, |ready| !*ready)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        wait.timed_out()
    })
    .await
    .unwrap_or(false);

    let Some(e) = state
        .mic
        .take_error()
        .or_else(|| timed_out.then(|| "microphone is not responding".to_string()))
    else {
        return Ok(());
    };
    log::warn!("capture failed to start: {e}");
    state.mic.disarm();
    state.try_stop_transcription();
    state.reset_recording_session();
    Err(ApiError::new(
        "capture_failed",
        crate::audio::error::friendly_capture_error(&e),
    ))
}

/// Discard the recording: nothing is decoded, typed or saved.
pub fn cancel_recording() {
    let state = state::get();
    if !state.try_stop_transcription() {
        return;
    }
    state.mic.disarm();
    drop(state.take_stream_session());
    drop(state.take_streamed_text());
    lock_recovering(&state.audio_buffer).clear();
    let _ = state.mic.take_error();
    state.reset_recording_session();
    log::info!("recording cancelled");
}

async fn stop_capture(state: &AppState) {
    let mic = Arc::clone(&state.mic);
    tokio::task::spawn_blocking(move || mic.disarm_after_post_roll())
        .await
        .ok();
}

/// `streamed_text` is written only after `running` drops, so wait for the tail.
async fn wait_for_stream_done(state: &AppState) {
    const STREAM_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
    let stream_done = Arc::clone(&state.stream_done);
    tokio::task::spawn_blocking(move || {
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

async fn finalize_current_recording(state: &'static AppState) -> Result<(), ApiError> {
    // The post-roll alone is 0.2 s, so this only drops a tap.
    const MIN_DURATION_SECS: f64 = 0.3;

    // Before the flag drops, or the idle watcher could unload the engine mid-stop.
    state.touch_engine();
    if !state.try_stop_transcription() {
        return Err(ApiError::new(
            "transcription_not_running",
            "transcription not running",
        ));
    }

    stop_capture(state).await;
    wait_for_stream_done(state).await;

    // Session, then audio buffer: the stream worker's lock order.
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

    // Set when the mic died mid-recording; what it captured first still counts.
    let mic_error = state.mic.take_error();

    #[allow(clippy::cast_precision_loss)]
    let too_short = samples.is_empty()
        || (captured_rate > 0
            && (samples.len() as f64 / f64::from(captured_rate)) < MIN_DURATION_SECS);
    if too_short {
        state.reset_recording_session();
        controller::send(match mic_error {
            Some(e) => Input::Failed(crate::audio::error::friendly_capture_error(&e)),
            None => Input::Finished(String::new()),
        });
        return Ok(());
    }
    if let Some(e) = mic_error {
        log::warn!("microphone failed mid-recording, finalizing what was captured: {e}");
        events::emit(Event::PillNotice(
            "Your microphone stopped mid-recording, so the transcript may be cut off.".to_string(),
        ));
    }

    let engine = match state.get_or_load_engine().await {
        Ok(e) => e,
        Err(e) => {
            log::error!("engine load failed: {e}");
            state.reset_recording_session();
            controller::send(Input::Failed(format!("model not ready: {e}")));
            return Ok(());
        }
    };

    let cfg = state.load_format_config();
    let format = cfg.is_usable().then_some(cfg);

    transcription::spawn_finalize(
        state,
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

/// `is_selected` is the saved preference, or the default when none is saved.
#[derive(Debug, Clone)]
pub struct InputDeviceInfo {
    pub name: String,
    pub is_default: bool,
    pub is_selected: bool,
}

/// Usable input devices, excluding loopback/virtual endpoints.
pub fn list_input_devices() -> Vec<InputDeviceInfo> {
    let state = state::get();
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

/// `None` follows the OS default. Applies from the next recording.
pub fn set_input_device(name: Option<&str>) -> Result<(), ApiError> {
    let state = state::get();
    match name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => state
            .save_input_device(name)
            .map_err(|e| ApiError::new("io_error", e.to_string()))?,
        None => state.delete_input_device(),
    }
    // A recording keeps its stream; the next one reopens on the new device.
    if !state.transcription_running.load(Ordering::SeqCst) {
        state.mic.close();
    }
    Ok(())
}
