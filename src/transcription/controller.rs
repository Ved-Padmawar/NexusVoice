//! The recording flow: hotkeys and pill buttons in, pill state out.
//!
//! One task owns the state and handles one input at a time, so a fast
//! press-and-release can never stop a recording that is still starting, and no
//! two triggers (a hotkey, a pill click, a Wayland signal) can race each other.
//! The UI only draws what [`Event::Pill`] says.

use std::sync::OnceLock;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::api::{self, ApiError};
use crate::events::{self, Event, ModelDownload};
use crate::state;

/// How long an error stays on the pill before it returns to idle.
const ERROR_LINGER: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PillState {
    Idle,
    /// Push-to-talk, held down.
    Recording,
    Dictation {
        paused: bool,
    },
    /// Finalizing the transcript and typing it.
    Processing,
    /// A model is downloading; the percentage of the current file.
    Downloading(u8),
    Error(String),
}

impl PillState {
    pub fn is_capturing(&self) -> bool {
        matches!(self, Self::Recording | Self::Dictation { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    PushToTalkPressed,
    PushToTalkReleased,
    /// Start dictation, or pause/resume a running one.
    DictationToggle,
    DictationCommit,
    /// Throw the current recording away without transcribing it.
    Cancel,
    /// The finished transcript, or an empty string when nothing was said.
    Finished(String),
    Failed(String),
    Download(ModelDownload),
    /// Clears the error with this generation if it is still showing.
    ClearError(u64),
}

static INPUTS: OnceLock<mpsc::UnboundedSender<Input>> = OnceLock::new();

/// Start the controller. Must run inside the tokio runtime, once.
pub fn spawn() {
    let (tx, rx) = mpsc::unbounded_channel();
    if INPUTS.set(tx).is_err() {
        return;
    }
    tokio::spawn(Controller::default().run(rx));
}

pub fn send(input: Input) {
    if let Some(tx) = INPUTS.get() {
        let _ = tx.send(input);
    }
}

/// Feed the controller the backend events that change what the pill shows.
pub(crate) fn observe(event: &Event) {
    if let Event::ModelDownload { status, .. } = event {
        send(Input::Download(status.clone()));
    }
}

#[derive(Default)]
struct Controller {
    pill: Option<PillState>,
    error_generation: u64,
}

impl Controller {
    async fn run(mut self, mut inputs: mpsc::UnboundedReceiver<Input>) {
        self.set(PillState::Idle);
        while let Some(input) = inputs.recv().await {
            self.handle(input).await;
        }
    }

    fn pill(&self) -> &PillState {
        self.pill.as_ref().unwrap_or(&PillState::Idle)
    }

    fn set(&mut self, next: PillState) {
        if self.pill.as_ref() != Some(&next) {
            self.pill = Some(next.clone());
            events::emit(Event::Pill(next));
        }
    }

    fn fail(&mut self, message: String) {
        self.error_generation += 1;
        let generation = self.error_generation;
        self.set(PillState::Error(message));
        tokio::spawn(async move {
            tokio::time::sleep(ERROR_LINGER).await;
            send(Input::ClearError(generation));
        });
    }

    #[allow(clippy::too_many_lines)] // one match over every input reads best whole
    async fn handle(&mut self, input: Input) {
        match input {
            Input::PushToTalkPressed => {
                if self.pill().is_capturing() || *self.pill() == PillState::Processing {
                    return;
                }
                if !self.model_ready() {
                    return;
                }
                self.set(PillState::Recording);
                if let Err(e) = api::start_transcription().await {
                    self.fail(start_error(&e));
                }
            }
            Input::PushToTalkReleased => {
                if *self.pill() != PillState::Recording {
                    return;
                }
                self.set(PillState::Processing);
                if let Err(e) = api::stop_transcription().await {
                    self.fail(e.message().to_string());
                }
            }
            Input::DictationToggle => match *self.pill() {
                PillState::Recording | PillState::Processing => {}
                PillState::Dictation { paused: false } => match api::pause_dictation() {
                    Ok(()) => self.set(PillState::Dictation { paused: true }),
                    Err(e) => self.fail(e.message().to_string()),
                },
                PillState::Dictation { paused: true } => match api::resume_dictation() {
                    Ok(()) => self.set(PillState::Dictation { paused: false }),
                    Err(e) => self.fail(e.message().to_string()),
                },
                PillState::Idle | PillState::Downloading(_) | PillState::Error(_) => {
                    if !self.model_ready() {
                        return;
                    }
                    self.set(PillState::Dictation { paused: false });
                    if let Err(e) = api::start_dictation().await {
                        self.fail(start_error(&e));
                    }
                }
            },
            Input::DictationCommit => {
                if !matches!(self.pill(), PillState::Dictation { .. }) {
                    return;
                }
                self.set(PillState::Processing);
                if let Err(e) = api::commit_dictation().await {
                    self.fail(e.message().to_string());
                }
            }
            Input::Cancel => {
                if self.pill().is_capturing() {
                    api::cancel_recording();
                    self.set(PillState::Idle);
                }
            }
            Input::Finished(text) => {
                if !text.is_empty() {
                    // A failed paste leaves the transcript on the clipboard and in
                    // the history, so the user can still paste it themselves.
                    if let Err(e) = crate::injection::type_text(&text).await {
                        log::warn!("text injection failed: {e}");
                    }
                }
                self.set(PillState::Idle);
            }
            Input::Failed(message) => self.fail(message),
            Input::Download(status) => self.on_download(status),
            Input::ClearError(generation) => {
                if generation == self.error_generation && matches!(self.pill(), PillState::Error(_))
                {
                    self.set(PillState::Idle);
                }
            }
        }
    }

    /// The pill mirrors a download only while it has nothing better to show.
    fn on_download(&mut self, status: ModelDownload) {
        let showing = matches!(self.pill(), PillState::Downloading(_));
        match status {
            ModelDownload::Queued | ModelDownload::Running => {
                if *self.pill() == PillState::Idle {
                    self.set(PillState::Downloading(0));
                }
            }
            ModelDownload::Progress(pct) => {
                if showing || *self.pill() == PillState::Idle {
                    self.set(PillState::Downloading(pct));
                }
            }
            ModelDownload::Complete | ModelDownload::Cancelled | ModelDownload::Failed(_) => {
                if showing {
                    self.set(PillState::Idle);
                }
            }
        }
    }

    /// Recording needs any model on disk: the engine falls back to one when the
    /// selected model is still downloading. Tells the user why when it can't.
    fn model_ready(&self) -> bool {
        if state::get().has_downloaded_model() {
            return true;
        }
        let notice = if matches!(self.pill(), PillState::Downloading(_)) {
            "Model downloading… please wait"
        } else {
            "No model installed — download one in Settings"
        };
        events::emit(Event::PillNotice(notice.to_string()));
        false
    }
}

/// Starting fails mostly on the microphone, which deserves a plain message.
fn start_error(error: &ApiError) -> String {
    let raw = error.message();
    let lower = raw.to_lowercase();
    if lower.contains("no input device") || lower.contains("no microphone") {
        "No microphone found".to_string()
    } else if lower.contains("permission") || lower.contains("access denied") {
        "Mic access denied".to_string()
    } else {
        raw.to_string()
    }
}

#[cfg(test)]
#[path = "../../tests/unit/transcription/controller.rs"]
mod tests;
