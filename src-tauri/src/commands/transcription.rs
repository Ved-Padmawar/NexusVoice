//! Transcription commands — thin endpoints over `crate::transcription`.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;
use crate::transcription::{self, service::FinalizeContext};

use super::error::ApiError;

#[tauri::command]
pub async fn start_transcription(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, ApiError> {
    if state.current_user_id().await.is_none() {
        return Err(ApiError::new("unauthenticated", "must be logged in to transcribe"));
    }

    if !state.try_start_transcription() {
        return Err(ApiError::new(
            "transcription_already_running",
            "transcription already running",
        ));
    }

    {
        let mut buf = state.audio_buffer.lock().expect("audio_buffer lock poisoned");
        buf.clear();
    }

    // Install a fresh pipeline for this recording session.
    *state.pipeline.lock().await = Some(crate::pipeline::StreamingPipeline::new());

    let pool = state.db().await.clone();
    transcription::start_capture(&app, &state, pool);

    Ok(true)
}

#[tauri::command]
pub async fn stop_transcription(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, ApiError> {
    const MIN_DURATION_SECS: f64 = 0.5;

    if !state.try_stop_transcription() {
        return Err(ApiError::new(
            "transcription_not_running",
            "transcription not running",
        ));
    }

    // Wait for the capture thread to fully stop and drop the cpal stream.
    // Uses a condvar instead of a fixed sleep — returns as soon as the thread signals done.
    let capture_done = Arc::clone(&state.capture_done);
    tauri::async_runtime::spawn_blocking(move || {
        let (lock, cvar) = &*capture_done;
        let _guard = cvar
            .wait_while(lock.lock().expect("capture_done lock poisoned"), |done| !*done)
            .expect("capture_done condvar poisoned");
    })
    .await
    .ok();

    let (samples, captured_rate) = {
        let mut buf = state.audio_buffer.lock().expect("audio_buffer lock poisoned");
        let rate = *state.native_sample_rate.lock().expect("native_sample_rate lock poisoned");
        (std::mem::take(&mut *buf), rate)
    };

    // Compute real recording duration from raw samples before any processing.
    #[allow(clippy::cast_precision_loss)] // sample counts fit f64 mantissa at typical lengths
    let duration_seconds: Option<f64> = if captured_rate > 0 && !samples.is_empty() {
        Some(samples.len() as f64 / f64::from(captured_rate))
    } else {
        None
    };

    // Reject empty or sub-0.5s recordings.
    #[allow(clippy::cast_precision_loss)]
    let too_short = samples.is_empty()
        || (captured_rate > 0 && (samples.len() as f64 / f64::from(captured_rate)) < MIN_DURATION_SECS);
    if too_short {
        *state.pipeline.lock().await = None;
        let _ = app.emit("transcription-complete", "");
        return Ok(false);
    }

    let engine = match state.get_or_load_engine().await {
        Ok(e) => e,
        Err(e) => {
            log::error!("engine load failed: {e}");
            *state.pipeline.lock().await = None;
            let _ = app.emit("transcription-error", format!("model not ready: {e}"));
            return Ok(false);
        }
    };

    // Take the pipeline out of state — finalize consumes it.
    let pipeline = state.pipeline.lock().await.take();

    // Pass the formatter config only if it's enabled and usable; otherwise the
    // finalize stage skips formatting and uses the raw transcript.
    let cfg = state.load_format_config();
    let format = cfg.is_usable().then_some(cfg);

    transcription::spawn_finalize(
        app,
        FinalizeContext {
            pool: state.db().await.clone(),
            dict_cache: Arc::clone(&state.dict_cache),
            engine,
            engine_cache: Arc::clone(&state.engine),
            pipeline,
            samples,
            captured_rate,
            beam_size: state.load_beam_size(),
            duration_seconds,
            format,
        },
    );

    Ok(true)
}
