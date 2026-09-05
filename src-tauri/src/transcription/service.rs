//! Transcription orchestration extracted from the command layer.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar};

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

/// Live transcript sent to the pill. Both decode paths produce the same split,
/// so the pill never needs to know which one ran.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PartialTranscript {
    committed: String,
    tentative: String,
}

/// Push one live-transcript frame to the pill, skipping empty ones.
fn emit_partial(app: &AppHandle, (committed, tentative): (String, String)) {
    if committed.is_empty() && tentative.is_empty() {
        return;
    }
    let _ = app.emit(
        "transcription-partial",
        PartialTranscript {
            committed,
            tentative,
        },
    );
}

/// Poll cadence of the stream worker. Decode frequency is governed by the
/// pipeline's minimum-new-audio gate, not this; polling is just the check.
const STREAM_POLL: std::time::Duration = std::time::Duration::from_millis(200);

/// Arm the microphone and spawn the streaming transcription workers. The mic is
/// held open across recordings, so arming costs an atomic store, not a device open.
pub fn start_capture(app: &AppHandle, state: &AppState) {
    *lock_recovering(&state.stream_done.0) = false;

    // Arm first — it clears the previous error, which the open below may set.
    state.mic.arm();
    state.mic.warm_up(state.load_input_device());

    spawn_engine_load(app);
    spawn_waveform_emitter(app, state);
    spawn_stream_worker(app, state);
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
fn spawn_stream_worker(app: &AppHandle, state: &AppState) {
    spawn_local_agreement_worker(app, state);
    spawn_native_stream_worker(app, state);
}

/// Signals on drop, so the worker's early returns all unblock finalize.
struct StreamDone(Arc<(std::sync::Mutex<bool>, Condvar)>);

impl Drop for StreamDone {
    fn drop(&mut self) {
        *lock_recovering(&self.0 .0) = true;
        self.0 .1.notify_all();
    }
}

/// Feed captured audio into a streaming model's own session, stashing the
/// finished transcript for finalize to collect.
fn spawn_native_stream_worker(app: &AppHandle, state: &AppState) {
    let running = Arc::clone(&state.transcription_running);
    let audio_buffer = Arc::clone(&state.audio_buffer);
    let native_rate = Arc::clone(&state.native_sample_rate);
    let engine_cache = Arc::clone(&state.engine);
    let streamed = Arc::clone(&state.streamed_text);
    let stream_done = Arc::clone(&state.stream_done);
    let app = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let _done = StreamDone(stream_done);

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
            if !prepared.is_empty() && stream.feed(&prepared) {
                emit_partial(&app, stream.partial());
            }
        }

        let _ = app.emit("transcription-partial-end", ());
        *lock_recovering(&streamed) = stream.finalize();
    });
}

/// Feed the growing buffer to the `StreamingSession` so finalize only has the
/// tail left to decode. Needs a cached engine; without one finalize decodes
/// everything in one pass.
fn spawn_local_agreement_worker(app: &AppHandle, state: &AppState) {
    let running = Arc::clone(&state.transcription_running);
    let audio_buffer = Arc::clone(&state.audio_buffer);
    let native_rate = Arc::clone(&state.native_sample_rate);
    let session_slot = Arc::clone(&state.stream_session);
    let engine_cache = Arc::clone(&state.engine);
    let app = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        // Logged once — the loop polls continuously.
        let mut route_logged = false;
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

            if !route_logged {
                log::info!("decoding via {}", Route::LocalAgreement.as_str());
                route_logged = true;
            }

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

            emit_partial(&app, session.partial());
        }

        let _ = app.emit("transcription-partial-end", ());
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
    /// App that had focus when recording started: shapes the formatter's output
    /// and labels the transcript in the UI.
    pub focus: Option<crate::focus::FocusTarget>,
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
        focus,
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
            let category = focus.as_ref().map(|f| f.category);
            match crate::llm::client::format_transcript(&cfg, &raw_text, category).await {
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
                target_app: focus.map(|f| f.name),
            })
            .await
        {
            let _ = app.emit("transcript:new", TranscriptResponse::from(saved));
        }
    });
}
