//! Turn a finished recording into delivered text: decode the tail, clean it
//! up, optionally format it, apply the dictionary, save it, paste it.
//!
//! Runs as a background task the webviews have no part in, so reloading or
//! closing a window cannot drop a dictation.

use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::commands::dto::TranscriptResponse;
use crate::database::dto::transcript::CreateTranscript;
use crate::database::repositories::transcript::TranscriptRepository;
use crate::database::Database;
use crate::dictionary::Dictionary;
use crate::events::{
    TranscriptSaved, TranscriptionComplete, TranscriptionError, TranscriptionWarning,
};
use crate::focus::FocusTarget;
use crate::inference::cache::{EngineCache, EngineLease, SharedEngine};
use crate::inference::transcript::strip_leading_dashes;
use crate::postprocess::remove_fillers;
use crate::settings::Settings;
use crate::transcribe::StreamingSession;

/// Everything finalize needs, taken from the recorder when capture stops.
pub struct Recording {
    pub engine: SharedEngine,
    /// Holds the model until the decode is done.
    pub lease: EngineLease,
    /// Growing-window state; `None` decodes the whole buffer in one pass.
    pub session: Option<StreamingSession>,
    /// Transcript from a streaming-native model, already complete.
    pub streamed: Option<String>,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub duration_seconds: Option<f64>,
    /// App that had focus when recording started: shapes the formatter's
    /// output and labels the transcript.
    pub focus: Option<FocusTarget>,
}

pub fn spawn(app: AppHandle, recording: Recording) {
    tauri::async_runtime::spawn(async move {
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

        let decoded = decode(&app, engine, session, streamed, samples, sample_rate).await;
        // A long dictation is use too: the idle clock starts when it ends.
        app.state::<EngineCache>().touch();
        drop(lease);

        let (raw, language) = match decoded {
            Ok(decoded) => decoded,
            Err(message) => {
                let _ = TranscriptionError(message.to_string()).emit(&app);
                return;
            }
        };
        // Whisper emits "- " at the start of short utterances.
        let text = remove_fillers(strip_leading_dashes(&raw), language.as_deref());
        if text.is_empty() {
            let _ = TranscriptionComplete { paste_error: None }.emit(&app);
            return;
        }

        let text = format(&app, text, focus.as_ref()).await;
        let pool = app.state::<Database>().pool().await;
        let text = match &pool {
            Ok(pool) => app.state::<Dictionary>().correct(pool, &text).await,
            Err(_) => text,
        };

        save(&app, pool.ok(), &text, duration_seconds, focus).await;
        let paste_error = crate::injection::type_text(&app, &text).await.err();
        if let Some(e) = &paste_error {
            log::warn!("paste failed: {e}");
        }
        let _ = TranscriptionComplete { paste_error }.emit(&app);
    });
}

/// The transcript and the language it came out in. Engine failures discard
/// the engine, so the next recording loads a fresh one.
async fn decode(
    app: &AppHandle,
    engine: SharedEngine,
    session: Option<StreamingSession>,
    streamed: Option<String>,
    samples: Vec<f32>,
    sample_rate: u32,
) -> Result<(String, Option<String>), &'static str> {
    const ENGINE_FAILED: &str =
        "Transcription engine encountered an error and was reset. Please try again.";
    const AUDIO_FAILED: &str = "Couldn't process the recorded audio. Please try again.";

    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // A streaming-native model already decoded everything as it was fed;
        // only the growing-window path has a tail left to decode.
        let text = if let Some(text) = streamed {
            text
        } else {
            let decoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::transcribe::finalize(session, &samples, sample_rate, &engine)
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
            })?
        };
        let language = engine
            .lock()
            .ok()
            .and_then(|guard| guard.output_language().map(str::to_string));
        Ok((text, language))
    })
    .await
    .unwrap_or_else(|e| {
        log::error!("finalize task died: {e}");
        Err(ENGINE_FAILED)
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
    match crate::llm::client::format_transcript(&config, &text, category).await {
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
) {
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
        }
        Err(e) => {
            log::error!("could not save the transcript: {e}");
            let _ = TranscriptionWarning(
                "This dictation couldn't be saved to your history.".to_string(),
            )
            .emit(app);
        }
    }
}
