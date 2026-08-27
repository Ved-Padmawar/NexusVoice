//! Transcription orchestration extracted from the command layer.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::commands::dto::TranscriptResponse;
use crate::database::dto::transcript::CreateTranscript;
use crate::database::repositories::{
    dictionary::DictionaryRepository, transcript::TranscriptRepository,
};
use crate::inference::TranscriptionEngine;
use crate::llm::FormatConfig;
use crate::postprocess::DictionaryCorrectionEngine;
use crate::state::{lock_recovering, AppState, DictCache};
use crate::transcribe::{Route, StreamSession};

/// Poll cadence of the stream worker. Decode frequency is governed by the
/// pipeline's minimum-new-audio gate, not this; polling is just the check.
const STREAM_POLL: std::time::Duration = std::time::Duration::from_millis(200);

/// Spawn the microphone capture thread and the streaming transcription worker.
///
/// Capture runs on a dedicated OS thread (cpal stream); the worker incrementally
/// transcribes the growing recording so finalize only has the tail left to do.
pub fn start_capture(app: &AppHandle, state: &AppState) {
    let running = Arc::clone(&state.transcription_running);
    let paused = Arc::clone(&state.capture_paused);
    let audio_buffer = Arc::clone(&state.audio_buffer);
    let native_rate = Arc::clone(&state.native_sample_rate);
    let waveform = Arc::clone(&state.waveform);
    let capture_done = Arc::clone(&state.capture_done);
    let capture_ready = Arc::clone(&state.capture_ready);
    let recording_mode = Arc::clone(&state.recording_mode);
    let session_phase = Arc::clone(&state.session_phase);
    let preferred_device = state.load_input_device();
    let app_handle = app.clone();

    // Reset the done + ready flags before starting a new capture session.
    *lock_recovering(&capture_done.0) = false;
    *lock_recovering(&capture_ready.0) = false;

    spawn_engine_load(app);
    spawn_waveform_emitter(app, state);
    spawn_stream_worker(state);

    std::thread::spawn(move || {
        if let Err(e) = crate::audio::capture_microphone(
            Arc::clone(&running),
            Arc::clone(&paused),
            audio_buffer,
            native_rate,
            waveform,
            Arc::clone(&capture_done),
            Arc::clone(&capture_ready),
            preferred_device,
        ) {
            log::error!("microphone capture error: {e}");
            running.store(false, Ordering::SeqCst);
            paused.store(false, Ordering::SeqCst);
            recording_mode.store(
                crate::state::RecordingMode::PushToTalk as u8,
                Ordering::SeqCst,
            );
            session_phase.store(crate::state::SessionPhase::Idle as u8, Ordering::SeqCst);
            // Signal done + ready even on error so neither stop_transcription
            // nor start_transcription's ready wait blocks forever.
            *lock_recovering(&capture_done.0) = true;
            capture_done.1.notify_one();
            *lock_recovering(&capture_ready.0) = true;
            capture_ready.1.notify_one();
            let _ = app_handle.emit(
                "transcription-error",
                crate::audio::error::friendly_capture_error(&e),
            );
        }
    });
}

/// Ensure the engine is loading as soon as recording starts — startup warmup is
/// fire-and-forget, so the cache can still be empty when the hotkey lands.
/// Spawned, not awaited, so the load races the recording instead of delaying capture.
fn spawn_engine_load(app: &AppHandle) {
    use tauri::Manager;

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = app.state::<AppState>().get_or_load_engine().await {
            log::debug!("engine not available for streaming: {e}");
        }
    });
}

/// Spawn the worker that decodes while recording. The engine may still be
/// loading, so both start and each exits if the other owns this recording.
fn spawn_stream_worker(state: &AppState) {
    spawn_local_agreement_worker(state);
    spawn_native_stream_worker(state);
}

/// Feed captured audio into a streaming model's own session, stashing the
/// finished transcript for finalize to collect.
fn spawn_native_stream_worker(state: &AppState) {
    let running = Arc::clone(&state.transcription_running);
    let audio_buffer = Arc::clone(&state.audio_buffer);
    let native_rate = Arc::clone(&state.native_sample_rate);
    let engine_cache = Arc::clone(&state.engine);
    let streamed = Arc::clone(&state.streamed_text);

    tauri::async_runtime::spawn_blocking(move || {
        // Wait for the engine, which loads in parallel with capture starting.
        let engine = loop {
            if !running.load(Ordering::SeqCst) {
                return;
            }
            if let Some(engine) = engine_cache.blocking_lock().clone() {
                break engine;
            }
            std::thread::sleep(STREAM_POLL);
        };

        let Ok(mut guard) = engine.lock() else {
            return;
        };
        if Route::for_engine(&guard) != Route::Streaming {
            return;
        }

        let options = guard.stream_run_options();
        let Ok(mut stream) = StreamSession::begin(guard.session_mut(), &options) else {
            return;
        };
        log::info!("decoding via {}", Route::Streaming.as_str());

        // Feed only what capture has added since the last chunk.
        let mut fed = 0usize;
        while running.load(Ordering::SeqCst) {
            std::thread::sleep(STREAM_POLL);
            let rate = *lock_recovering(&native_rate);
            let chunk = {
                let buf = lock_recovering(&audio_buffer);
                if buf.len() <= fed {
                    continue;
                }
                let chunk = buf[fed..].to_vec();
                fed = buf.len();
                chunk
            };
            let prepared = crate::preprocess::to_16k(&chunk, rate);
            if !prepared.is_empty() {
                stream.feed(&prepared);
            }
        }

        *lock_recovering(&streamed) = stream.finalize();
    });
}

/// Feed the growing buffer to the `StreamingSession` so finalize only has the
/// tail left to decode. Needs a cached engine; without one finalize decodes
/// everything in one pass.
fn spawn_local_agreement_worker(state: &AppState) {
    let running = Arc::clone(&state.transcription_running);
    let audio_buffer = Arc::clone(&state.audio_buffer);
    let native_rate = Arc::clone(&state.native_sample_rate);
    let session_slot = Arc::clone(&state.stream_session);
    let engine_cache = Arc::clone(&state.engine);

    tauri::async_runtime::spawn_blocking(move || {
        while running.load(Ordering::SeqCst) {
            std::thread::sleep(STREAM_POLL);
            let Some(engine) = engine_cache.blocking_lock().clone() else {
                continue;
            };

            // Models that own their session state are driven by the streaming
            // path instead; this worker has nothing to do for them.
            if engine
                .lock()
                .is_ok_and(|e| Route::for_engine(&e) == Route::Streaming)
            {
                break;
            }

            // Lock order everywhere is session → audio buffer. Finalize takes
            // the session first too, so it waits out an in-flight decode and
            // the worker exits on the emptied slot.
            let mut slot = lock_recovering(&session_slot);
            let Some(session) = slot.as_mut() else {
                break;
            };
            let rate = *lock_recovering(&native_rate);
            // Snapshot only when a decode will run — the copy grows with the recording.
            let Some(window) = ({
                let buf = lock_recovering(&audio_buffer);
                session
                    .would_decode(buf.len(), rate)
                    .then(|| buf[session.window_start().min(buf.len())..].to_vec())
            }) else {
                continue;
            };

            let polled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                session.poll(&window, rate, &engine);
            }));
            if polled.is_err() {
                // Engine panicked mid-decode: evict it and drop the session
                // so finalize falls back to a clean single-pass decode.
                log::error!("TranscriptionEngine panicked during streaming — evicting");
                *slot = None;
                *engine_cache.blocking_lock() = None;
                break;
            }
        }
    });
}

/// Emit pill waveform levels at ~30 Hz from the spectrum meter while recording.
fn spawn_waveform_emitter(app: &AppHandle, state: &AppState) {
    let running = Arc::clone(&state.transcription_running);
    let waveform = Arc::clone(&state.waveform);
    let app = app.clone();

    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_millis(33));
        while running.load(Ordering::SeqCst) {
            tick.tick().await;
            let _ = app.emit("pill:waveform", waveform.levels());
        }
        let _ = app.emit("pill:waveform", [0.0_f32; crate::audio::waveform::BANDS]);
    });
}

/// Inputs captured from `AppState` for the finalize task. Gathered in the
/// command (where `State` is available) and handed to the spawned task.
pub struct FinalizeContext {
    pub pool: sqlx::SqlitePool,
    pub dict_cache: DictCache,
    pub engine: Arc<std::sync::Mutex<TranscriptionEngine>>,
    pub engine_cache: Arc<tokio::sync::Mutex<Option<Arc<std::sync::Mutex<TranscriptionEngine>>>>>,
    /// Streaming state accumulated while recording; `None` falls back to a
    /// single-pass decode of the whole buffer.
    pub session: Option<crate::transcribe::StreamingSession>,
    /// Transcript from a streaming-native model. When present the audio is
    /// already fully decoded and no further pass is needed.
    pub streamed: Option<String>,
    pub samples: Vec<f32>,
    pub captured_rate: u32,
    pub duration_seconds: Option<f64>,
    /// Formatter config to apply to the final transcript. `None` when formatting is
    /// disabled or unconfigured — the raw transcript is then used unchanged.
    pub format: Option<FormatConfig>,
}

/// Spawn the finalize task: resolve the transcript, optionally LLM-format it,
/// apply dictionary corrections, emit results to the frontend, and persist.
#[allow(clippy::too_many_lines)] // cohesive single task — splitting adds no clarity
pub fn spawn_finalize(app: AppHandle, ctx: FinalizeContext) {
    let FinalizeContext {
        pool,
        dict_cache,
        engine,
        engine_cache,
        session,
        streamed,
        samples,
        captured_rate,
        duration_seconds,
        format,
    } = ctx;

    tauri::async_runtime::spawn(async move {
        let raw_text = tauri::async_runtime::spawn_blocking({
            let engine = Arc::clone(&engine);
            let engine_cache = Arc::clone(&engine_cache);
            move || -> Result<String, String> {
                // A streaming-native model already decoded everything as it was
                // fed; only the growing-window path has a tail left to decode.
                if let Some(text) = streamed {
                    return Ok(text);
                }
                let Ok(text) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    crate::transcribe::finalize(session, &samples, captured_rate, &engine)
                })) else {
                    log::error!("TranscriptionEngine panicked during finalize — evicting");
                    *engine_cache.blocking_lock() = None;
                    return Err("engine_poisoned".to_string());
                };
                // Also evict if Mutex was poisoned during finalize
                if engine.is_poisoned() {
                    log::error!("TranscriptionEngine mutex poisoned after finalize — evicting");
                    *engine_cache.blocking_lock() = None;
                }
                Ok(text)
            }
        })
        .await
        .map_err(|e| format!("finalize join error: {e}"))
        .and_then(|r| r);

        let raw_text = match raw_text {
            Ok(t) => t,
            Err(e) => {
                let _ = app.emit(
                    "transcription-error",
                    "Transcription engine encountered an error and was reset. Please try again.",
                );
                log::error!("finalize failed: {e}");
                return;
            }
        };

        // Strip leading dash hallucinations — Whisper emits "- " at the start of short utterances.
        let raw_text = raw_text
            .trim_start_matches(|c: char| c == '-' || c == '–' || c == '—' || c.is_whitespace())
            .to_string();

        log::debug!("final transcript: {} chars", raw_text.len());

        if raw_text.is_empty() {
            let _ = app.emit("transcription-complete", "");
            return;
        }

        // Optional LLM formatting stage — an HTTP call to the configured
        // OpenAI-compatible endpoint. Any failure falls back to the raw
        // transcript so a misconfigured or unreachable formatter never drops
        // the user's dictation.
        let formatted_text = if let Some(cfg) = format {
            match crate::llm::client::format_transcript(&cfg, &raw_text).await {
                Ok(t) if !t.trim().is_empty() => t,
                Ok(_) => {
                    log::warn!("formatter returned empty output — using raw transcript");
                    raw_text.clone()
                }
                Err(e) => {
                    log::warn!("formatter failed ({e}) — using raw transcript");
                    raw_text.clone()
                }
            }
        } else {
            raw_text.clone()
        };

        // Post-process: apply dictionary corrections.
        let dict_entries: Vec<_> = dict_cache.read().await.values().cloned().collect();
        let corrector = DictionaryCorrectionEngine::new(dict_entries);
        let (text, matched_terms) = corrector.apply_to_text(&formatted_text);

        if !matched_terms.is_empty() {
            let dict_repo = DictionaryRepository::new(pool.clone());
            let _ = dict_repo.increment_hits_batch(&matched_terms).await;
            let mut cache = dict_cache.write().await;
            for term in &matched_terms {
                if let Some(entry) = cache.get_mut(term) {
                    entry.hits += 1;
                }
            }
        }

        let _ = app.emit("transcription-complete", text.clone());

        let repo = TranscriptRepository::new(pool.clone());
        #[allow(clippy::cast_possible_wrap)] // word count never exceeds i64::MAX
        let word_count = text.split_whitespace().count() as i64;
        if let Ok(saved) = repo
            .create(CreateTranscript {
                content: text.clone(),
                word_count,
                duration_seconds,
            })
            .await
        {
            let _ = app.emit("transcript:new", TranscriptResponse::from(saved));
        }
    });
}
