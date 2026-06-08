//! Streaming transcription orchestration extracted from the command layer.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::commands::dto::TranscriptResponse;
use crate::database::dto::transcript::CreateTranscript;
use crate::database::repositories::{
    dictionary::DictionaryRepository, transcript::TranscriptRepository,
};
use crate::inference::WhisperEngine;
use crate::llm::FormatConfig;
use crate::postprocess::DictionaryCorrectionEngine;
use crate::state::{AppState, DictCache};

/// Build the Whisper `initial_prompt` from the user's most recent transcripts,
/// oldest-first, so the model is biased toward the user's vocabulary/style.
async fn recent_transcripts_prompt(repo: &TranscriptRepository) -> String {
    repo.list_recent(5)
        .await
        .unwrap_or_default()
        .into_iter()
        .rev()
        .map(|t| t.content)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Spawn the microphone capture thread and the mid-recording chunk poller.
///
/// Capture runs on a dedicated OS thread (cpal stream); the poller wakes every
/// 2 s and commits a pipeline chunk if enough audio has accumulated, so most of
/// the transcription work is done before the user releases the hotkey.
pub fn start_capture(app: &AppHandle, state: &AppState, pool: sqlx::SqlitePool) {
    let running = Arc::clone(&state.transcription_running);
    let paused = Arc::clone(&state.capture_paused);
    let audio_buffer = Arc::clone(&state.audio_buffer);
    let native_rate = Arc::clone(&state.native_sample_rate);
    let waveform = Arc::clone(&state.waveform);
    let capture_done = Arc::clone(&state.capture_done);
    let recording_mode = Arc::clone(&state.recording_mode);
    let session_phase = Arc::clone(&state.session_phase);
    let app_handle = app.clone();

    // Reset the done flag before starting a new capture session.
    *capture_done.0.lock().expect("capture_done lock poisoned") = false;

    spawn_waveform_emitter(app, state);

    std::thread::spawn(move || {
        if let Err(e) = crate::audio::capture_microphone(
            Arc::clone(&running),
            Arc::clone(&paused),
            audio_buffer,
            native_rate,
            waveform,
            Arc::clone(&capture_done),
        ) {
            log::error!("microphone capture error: {e}");
            running.store(false, Ordering::SeqCst);
            paused.store(false, Ordering::SeqCst);
            recording_mode.store(
                crate::state::RecordingMode::PushToTalk as u8,
                Ordering::SeqCst,
            );
            session_phase.store(crate::state::SessionPhase::Idle as u8, Ordering::SeqCst);
            // Signal done even on error so stop_transcription doesn't wait forever.
            *capture_done.0.lock().expect("capture_done lock poisoned") = true;
            capture_done.1.notify_one();
            let _ = app_handle.emit("transcription-error", e);
        }
    });

    spawn_chunk_poller(state, pool);
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

/// Background poller that fires pipeline chunks while recording. Wakes every 2 s,
/// snapshots the audio buffer, and transcribes the next chunk if the engine is
/// loaded — so finalize only needs to process the tail.
fn spawn_chunk_poller(state: &AppState, pool: sqlx::SqlitePool) {
    let running = Arc::clone(&state.transcription_running);
    let audio_buffer = Arc::clone(&state.audio_buffer);
    let native_rate_arc = Arc::clone(&state.native_sample_rate);
    let pipeline_arc = Arc::clone(&state.pipeline);
    let engine_arc = Arc::clone(&state.engine);
    let engine_cache = Arc::clone(&state.engine);

    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            if !running.load(Ordering::SeqCst) {
                break;
            }

            // Snapshot buffer + rate under lock, then release immediately.
            let (buffer_snapshot, captured_rate) = {
                let buf = audio_buffer.lock().expect("audio_buffer lock poisoned");
                let rate = *native_rate_arc.lock().expect("native_rate lock poisoned");
                (buf.clone(), rate)
            };

            // Get engine — skip this tick if not loaded yet.
            let engine = {
                let guard = engine_arc.lock().await;
                guard.as_ref().map(Arc::clone)
            };
            let Some(engine) = engine else { continue };

            // Build prompt from recent transcripts for this chunk.
            let prompt = {
                let repo = TranscriptRepository::new(pool.clone());
                recent_transcripts_prompt(&repo).await
            };

            let committed = tauri::async_runtime::spawn_blocking({
                let engine = Arc::clone(&engine);
                let engine_cache = Arc::clone(&engine_cache);
                let pipeline_arc = Arc::clone(&pipeline_arc);
                move || {
                    let mut pl_guard = pipeline_arc.blocking_lock();
                    if let Some(pl) = pl_guard.as_mut() {
                        // Mid-recording chunks use the Balanced beam size (5);
                        // the configured beam size is applied on finalize.
                        let did_commit = pl.try_commit_chunk(
                            &buffer_snapshot,
                            captured_rate,
                            &engine,
                            &prompt,
                            5,
                        );
                        if did_commit && engine.is_poisoned() {
                            log::error!(
                                "WhisperEngine mutex poisoned during streaming chunk — evicting"
                            );
                            drop(pl_guard);
                            *engine_cache.blocking_lock() = None;
                            return Err("engine_poisoned");
                        }
                        Ok(did_commit)
                    } else {
                        Ok(false)
                    }
                }
            })
            .await;

            match committed {
                Ok(Ok(true)) => log::debug!("streaming: mid-recording chunk committed"),
                Ok(Err("engine_poisoned")) => break,
                _ => {}
            }
        }
    });
}

/// Inputs captured from `AppState` for the finalize task. Gathered in the
/// command (where `State` is available) and handed to the spawned task.
pub struct FinalizeContext {
    pub pool: sqlx::SqlitePool,
    pub dict_cache: DictCache,
    pub engine: Arc<std::sync::Mutex<WhisperEngine>>,
    pub engine_cache: Arc<tokio::sync::Mutex<Option<Arc<std::sync::Mutex<WhisperEngine>>>>>,
    pub pipeline: Option<crate::pipeline::StreamingPipeline>,
    pub samples: Vec<f32>,
    pub captured_rate: u32,
    pub beam_size: i32,
    pub duration_seconds: Option<f64>,
    /// Formatter config to apply after stitching. `None` when formatting is
    /// disabled or unconfigured — the raw transcript is then used unchanged.
    pub format: Option<FormatConfig>,
}

/// Spawn the finalize task: stitch the transcript, optionally LLM-format it,
/// apply dictionary corrections, emit results to the frontend, and persist.
#[allow(clippy::too_many_lines)] // cohesive single task — splitting adds no clarity
pub fn spawn_finalize(app: AppHandle, ctx: FinalizeContext) {
    let FinalizeContext {
        pool,
        dict_cache,
        engine,
        engine_cache,
        pipeline,
        samples,
        captured_rate,
        beam_size,
        duration_seconds,
        format,
    } = ctx;

    tauri::async_runtime::spawn(async move {
        let prompt = {
            let repo = TranscriptRepository::new(pool.clone());
            recent_transcripts_prompt(&repo).await
        };

        // finalize() preprocesses only the tail (audio since last committed chunk)
        // and stitches with previously committed chunk texts — fast path when
        // mid-recording chunks already covered most of the speech.
        let raw_text = tauri::async_runtime::spawn_blocking({
            let engine = Arc::clone(&engine);
            let engine_cache = Arc::clone(&engine_cache);
            move || -> Result<String, String> {
                let pl = pipeline.unwrap_or_else(crate::pipeline::StreamingPipeline::new);
                let Ok(text) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    pl.finalize(&samples, captured_rate, &engine, &prompt, beam_size)
                })) else {
                    log::error!("WhisperEngine panicked during finalize — evicting");
                    *engine_cache.blocking_lock() = None;
                    return Err("engine_poisoned".to_string());
                };
                // Also evict if Mutex was poisoned during finalize
                if engine.is_poisoned() {
                    log::error!("WhisperEngine mutex poisoned after finalize — evicting");
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

        log::debug!("final stitched result: {} chars", raw_text.len());

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
