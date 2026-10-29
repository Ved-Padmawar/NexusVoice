//! Turn a finished recording into delivered text: decode the tail, clean it
//! up, optionally format it, apply the dictionary, save it, paste it.
//!
//! Runs as a background task the webviews have no part in, so reloading or
//! closing a window cannot drop a dictation.

use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::bridge::commands::dto::TranscriptResponse;
use crate::bridge::events::{
    TranscriptSaved, TranscriptionComplete, TranscriptionError, TranscriptionWarning,
};
use crate::data::dto::transcript::CreateTranscript;
use crate::data::repositories::transcript::TranscriptRepository;
use crate::data::Database;
use crate::models::cache::{EngineCache, EngineLease, SharedEngine};
use crate::models::decode::{FinalText, Streamed, StreamingSession};
use crate::models::transcript::strip_leading_dashes;
use crate::platform::focus::FocusTarget;
use crate::settings::Settings;
use crate::text::dictionary::Dictionary;
use crate::text::remove_fillers;

/// Everything finalize needs, taken from the recorder when capture stops.
pub struct Recording {
    pub engine: SharedEngine,
    /// Holds the model until the decode is done.
    pub lease: EngineLease,
    /// Growing-window state; `None` decodes the whole buffer in one pass.
    pub session: Option<StreamingSession>,
    /// Transcript from a streaming-native model.
    pub streamed: Option<Streamed>,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub duration_seconds: Option<f64>,
    /// App that had focus when recording started: shapes the formatter's
    /// output and labels the transcript.
    pub focus: Option<FocusTarget>,
}

pub async fn run(app: &AppHandle, recording: Recording) {
    let Recording {
        engine,
        lease,
        session,
        streamed,
        samples,
        sample_rate,
        duration_seconds,
        focus,
    } = recording;

    let decoded = decode(app, engine, session, streamed, samples, sample_rate).await;
    // A long dictation is use too: the idle clock starts when it ends.
    app.state::<EngineCache>().touch();
    drop(lease);

    let (raw, language, complete) = match decoded {
        Ok(decoded) => decoded,
        Err(message) => {
            let _ = TranscriptionError(message.to_string()).emit(app);
            return;
        }
    };
    if !complete {
        let _ = TranscriptionWarning(
            "Part of your dictation could not be transcribed, so the end may be missing."
                .to_string(),
        )
        .emit(app);
    }
    // Whisper emits "- " at the start of short utterances.
    let text = remove_fillers(strip_leading_dashes(&raw), language.as_deref());
    if text.is_empty() {
        let _ = TranscriptionComplete {
            paste_error: None,
            saved: false,
        }
        .emit(app);
        return;
    }

    let text = format(app, text, focus.as_ref()).await;
    let pool = app.state::<Database>().pool().await;
    let text = match &pool {
        Ok(pool) => app.state::<Dictionary>().correct(pool, &text).await,
        Err(_) => text,
    };

    let saved = save(app, pool.ok(), &text, duration_seconds, focus).await;
    let paste_error = crate::platform::injection::type_text(app, &text)
        .await
        .err();
    if let Some(e) = &paste_error {
        log::warn!("paste failed: {e}");
    }
    let _ = TranscriptionComplete { paste_error, saved }.emit(app);
}

const ENGINE_FAILED: &str =
    "Transcription engine encountered an error and was reset. Please try again.";
const AUDIO_FAILED: &str = "Couldn't process the recorded audio. Please try again.";

/// Engine failures discard the engine, so the next recording loads a fresh one.
async fn decode(
    app: &AppHandle,
    engine: SharedEngine,
    session: Option<StreamingSession>,
    streamed: Option<Streamed>,
    samples: Vec<f32>,
    sample_rate: u32,
) -> Result<(String, Option<String>, bool), &'static str> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let run_buffer = |session| decode_buffer(&app, &engine, session, &samples, sample_rate);
        let FinalText { text, complete } = match streamed {
            Some(Streamed {
                text,
                truncated: false,
            }) => FinalText {
                text,
                complete: true,
            },
            // Cut short: decode the buffer, keeping the stream's text only if that fails.
            Some(Streamed { text, .. }) => match run_buffer(None) {
                Ok(retry) if retry.complete => retry,
                _ => FinalText {
                    text,
                    complete: false,
                },
            },
            None => run_buffer(session)?,
        };
        let language = engine
            .lock()
            .ok()
            .and_then(|guard| guard.output_language().map(str::to_string));
        Ok((text, language, complete))
    })
    .await
    .unwrap_or_else(|e| {
        log::error!("finalize task died: {e}");
        Err(ENGINE_FAILED)
    })
}

fn decode_buffer(
    app: &AppHandle,
    engine: &SharedEngine,
    session: Option<StreamingSession>,
    samples: &[f32],
    sample_rate: u32,
) -> Result<FinalText, &'static str> {
    let decoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::models::decode::finalize(session, samples, sample_rate, engine)
    }));
    if decoded.is_err() || engine.is_poisoned() {
        log::error!("engine failed during finalize; discarding it");
        app.state::<EngineCache>().discard();
    }
    let Ok(decoded) = decoded else {
        return Err(ENGINE_FAILED);
    };
    decoded.map_err(|e| {
        log::error!("finalize failed: {e}");
        AUDIO_FAILED
    })
}

/// Optional LLM formatting. Any failure falls back to the raw transcript, so a
/// misconfigured or unreachable formatter never drops the user's dictation.
async fn format(app: &AppHandle, text: String, focus: Option<&FocusTarget>) -> String {
    let config = app.state::<Settings>().format_config();
    if !config.is_usable() {
        return text;
    }
    let category = focus.map(|f| f.category);
    match crate::text::llm::client::format_transcript(&config, &text, category).await {
        Ok(formatted) if !formatted.trim().is_empty() => formatted,
        Ok(_) => {
            log::warn!("formatter returned empty output; using the raw transcript");
            text
        }
        Err(e) => {
            log::warn!("formatter failed ({e}); using the raw transcript");
            text
        }
    }
}

/// A failed save is reported but does not stop the paste.
async fn save(
    app: &AppHandle,
    pool: Option<sqlx::SqlitePool>,
    text: &str,
    duration_seconds: Option<f64>,
    focus: Option<FocusTarget>,
) -> bool {
    #[allow(clippy::cast_possible_wrap)] // word count never exceeds i64::MAX
    let word_count = text.split_whitespace().count() as i64;
    let saved = match pool {
        Some(pool) => TranscriptRepository::new(pool)
            .create(CreateTranscript {
                content: text.to_string(),
                word_count,
                duration_seconds,
                target_app: focus.map(|f| f.name),
            })
            .await
            .map_err(|e| e.to_string()),
        None => Err("database unavailable".to_string()),
    };
    match saved {
        Ok(saved) => {
            let _ = TranscriptSaved(TranscriptResponse::from(saved)).emit(app);
            true
        }
        Err(e) => {
            log::error!("could not save the transcript: {e}");
            let _ = TranscriptionWarning(
                "This dictation couldn't be saved to your history.".to_string(),
            )
            .emit(app);
            false
        }
    }
}
