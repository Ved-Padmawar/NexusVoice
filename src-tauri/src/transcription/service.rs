//! Capture and one-shot transcription orchestration.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::commands::dto::TranscriptResponse;
use crate::database::dto::transcript::CreateTranscript;
use crate::database::repositories::{
    dictionary::DictionaryRepository, transcript::TranscriptRepository,
};
use crate::llm::FormatConfig;
use crate::postprocess::DictionaryCorrectionEngine;
use crate::state::{AppState, DictCache, EngineCache, SharedEngine};

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
    let app_handle = app.clone();

    *capture_done.0.lock().expect("capture_done lock poisoned") = false;
    *capture_ready.0.lock().expect("capture_ready lock poisoned") = false;
    spawn_waveform_emitter(app, state);

    std::thread::spawn(move || {
        if let Err(error) = crate::audio::capture_microphone(
            Arc::clone(&running),
            Arc::clone(&paused),
            audio_buffer,
            native_rate,
            waveform,
            Arc::clone(&capture_done),
            Arc::clone(&capture_ready),
        ) {
            log::error!("microphone capture error: {error}");
            running.store(false, Ordering::SeqCst);
            paused.store(false, Ordering::SeqCst);
            recording_mode.store(
                crate::state::RecordingMode::PushToTalk as u8,
                Ordering::SeqCst,
            );
            session_phase.store(crate::state::SessionPhase::Idle as u8, Ordering::SeqCst);
            *capture_done.0.lock().expect("capture_done lock poisoned") = true;
            capture_done.1.notify_one();
            *capture_ready.0.lock().expect("capture_ready lock poisoned") = true;
            capture_ready.1.notify_one();
            let _ = app_handle.emit("transcription-error", error);
        }
    });
}

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

pub struct FinalizeContext {
    pub pool: sqlx::SqlitePool,
    pub dict_cache: DictCache,
    pub engine: SharedEngine,
    pub engine_cache: EngineCache,
    pub samples: Vec<f32>,
    pub captured_rate: u32,
    pub duration_seconds: Option<f64>,
    pub format: Option<FormatConfig>,
}

pub fn spawn_finalize(app: AppHandle, ctx: FinalizeContext) {
    let FinalizeContext {
        pool,
        dict_cache,
        engine,
        engine_cache,
        samples,
        captured_rate,
        duration_seconds,
        format,
    } = ctx;

    tauri::async_runtime::spawn(async move {
        let raw_text = tauri::async_runtime::spawn_blocking(move || {
            let audio = crate::preprocess::preprocess(&samples, captured_rate);
            if audio.is_empty() {
                return Ok(String::new());
            }
            let mut guard = engine
                .lock()
                .map_err(|_| "parakeet.cpp engine lock poisoned".to_string())?;
            guard.transcribe(&audio)
        })
        .await
        .map_err(|e| format!("transcription task failed: {e}"))
        .and_then(|result| result);

        let raw_text = match raw_text {
            Ok(text) => text,
            Err(error) => {
                *engine_cache.lock().await = None;
                log::error!("parakeet.cpp transcription failed: {error}");
                let _ = app.emit(
                    "transcription-error",
                    "Transcription failed and the model was reset. Please try again.",
                );
                return;
            }
        };
        if raw_text.is_empty() {
            let _ = app.emit("transcription-complete", "");
            return;
        }

        let formatted = if let Some(config) = format {
            match crate::llm::client::format_transcript(&config, &raw_text).await {
                Ok(text) if !text.trim().is_empty() => text,
                Ok(_) => raw_text.clone(),
                Err(error) => {
                    log::warn!("formatter failed ({error}); using raw transcript");
                    raw_text.clone()
                }
            }
        } else {
            raw_text
        };

        let entries: Vec<_> = dict_cache.read().await.values().cloned().collect();
        let (text, matched_terms) =
            DictionaryCorrectionEngine::new(entries).apply_to_text(&formatted);
        if !matched_terms.is_empty() {
            let repository = DictionaryRepository::new(pool.clone());
            let _ = repository.increment_hits_batch(&matched_terms).await;
            let mut cache = dict_cache.write().await;
            for term in &matched_terms {
                if let Some(entry) = cache.get_mut(term) {
                    entry.hits += 1;
                }
            }
        }

        let _ = app.emit("transcription-complete", text.clone());
        let repository = TranscriptRepository::new(pool);
        #[allow(clippy::cast_possible_wrap)]
        let word_count = text.split_whitespace().count() as i64;
        if let Ok(saved) = repository
            .create(CreateTranscript {
                content: text,
                word_count,
                duration_seconds,
            })
            .await
        {
            let _ = app.emit("transcript:new", TranscriptResponse::from(saved));
        }
    });
}
