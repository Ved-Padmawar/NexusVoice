//! Turn a finished recording into delivered text: decode the tail, clean it
//! up, optionally format it, apply the dictionary, save it, paste it.
//!
//! Runs as a background task the webviews have no part in, so reloading or
//! closing a window cannot drop a dictation.

use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::bridge::events::{TranscriptionComplete, TranscriptionError, TranscriptionWarning};
use crate::data::dto::transcript::CreateTranscript;
use crate::data::repositories::transcript::TranscriptRepository;
use crate::data::Database;
use crate::models::cache::{EngineCache, EngineLease, SharedEngine};
use crate::models::decode::{contains_speech, FinalText, Streamed, StreamingSession};
use crate::models::transcript::strip_leading_dashes;
use crate::models::TranscriptionEngine;
use crate::platform::focus::{foreground_app, FocusTarget};
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
    /// App that had focus when recording started: shapes the formatter's output.
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

    let language = lease.language().map(str::to_string);
    let model = engine.entry.id.clone();
    // Kept apart from the decode, which consumes the samples.
    let audio = app
        .state::<Settings>()
        .saved_audio()
        .enabled
        .then(|| samples.clone());
    let decoded = decode(engine, session, streamed, samples, sample_rate, language).await;
    // A long dictation is use too: the idle clock starts when it ends.
    app.state::<EngineCache>().touch();
    drop(lease);

    let Decoded {
        text,
        complete,
        heard_speech,
    } = match decoded {
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
    if text.is_empty() {
        if heard_speech {
            log::info!("speech was heard but nothing could be transcribed");
            let _ =
                TranscriptionWarning("Couldn't transcribe that. Try again.".to_string()).emit(app);
        } else {
            log::info!("no speech detected; nothing to paste");
        }
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

    // Where the text lands is whatever has focus now, not where recording began.
    let landed = foreground_app().or(focus);
    let saved_id = save(app, pool.ok(), &text, duration_seconds, landed, model).await;
    let saved = saved_id.is_some();
    let paste_error = crate::platform::injection::type_text(app, &text)
        .await
        .err();
    if let Some(e) = &paste_error {
        log::warn!("paste failed: {e}");
    }
    let _ = TranscriptionComplete { paste_error, saved }.emit(app);
    if let (Some(id), Some(samples)) = (saved_id, audio) {
        crate::recordings::keep_native(app, id, samples, sample_rate);
    }
}

const ENGINE_FAILED: &str = "Transcription failed unexpectedly. Please try again.";
const AUDIO_FAILED: &str = "Couldn't process the recorded audio. Please try again.";

/// A finished decode, cleaned up and ready to format.
struct Decoded {
    text: String,
    /// `false` when the end of the recording may be missing.
    complete: bool,
    /// Voice detection found speech in the recording. Only worked out for an
    /// empty transcript, the one case that needs it.
    heard_speech: bool,
}

/// The engine recovers from its own crashes and hangs, so a failure here
/// only costs this recording.
async fn decode(
    engine: SharedEngine,
    session: Option<StreamingSession>,
    streamed: Option<Streamed>,
    samples: Vec<f32>,
    sample_rate: u32,
    language: Option<String>,
) -> Result<Decoded, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let language = language.as_deref();
        let run_buffer = |session| decode_buffer(&engine, session, &samples, sample_rate, language);
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
        let output = engine.output_language(language);
        // Whisper emits "- " at the start of short utterances.
        let text = remove_fillers(strip_leading_dashes(&text), output.as_deref());
        let heard_speech = text.is_empty() && contains_speech(&samples, sample_rate);
        Ok(Decoded {
            text,
            complete,
            heard_speech,
        })
    })
    .await
    .unwrap_or_else(|e| {
        log::error!("finalize task died: {e}");
        Err(ENGINE_FAILED)
    })
}

fn decode_buffer(
    engine: &TranscriptionEngine,
    session: Option<StreamingSession>,
    samples: &[f32],
    sample_rate: u32,
    language: Option<&str>,
) -> Result<FinalText, &'static str> {
    crate::models::decode::finalize(session, samples, sample_rate, engine, language).map_err(|e| {
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
        Ok(formatted) => formatted,
        Err(e) => {
            log::warn!("formatter failed ({e}); using the raw transcript");
            let _ = TranscriptionWarning("Pasted without formatting.".to_string()).emit(app);
            text
        }
    }
}

/// A failed save is reported but does not stop the paste. Returns the new
/// entry's id.
async fn save(
    app: &AppHandle,
    pool: Option<sqlx::SqlitePool>,
    text: &str,
    duration_seconds: Option<f64>,
    landed: Option<FocusTarget>,
    model: String,
) -> Option<i64> {
    #[allow(clippy::cast_possible_wrap)] // word count never exceeds i64::MAX
    let word_count = text.split_whitespace().count() as i64;
    let saved = match pool {
        Some(pool) => TranscriptRepository::new(pool)
            .create(CreateTranscript {
                content: text.to_string(),
                word_count,
                duration_seconds,
                target_app: landed.map(|f| f.name),
                source_name: None,
                model: Some(model),
                pending: false,
                language: None,
            })
            .await
            .map_err(|e| e.to_string()),
        None => Err("database unavailable".to_string()),
    };
    match saved {
        Ok(saved) => Some(saved.id),
        Err(e) => {
            log::error!("could not save the transcript: {e}");
            let _ = TranscriptionWarning(
                "This dictation couldn't be saved to your history.".to_string(),
            )
            .emit(app);
            None
        }
    }
}
