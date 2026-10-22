//! The one channel from the backend to the UI.
//!
//! Every background task reports through [`emit`]. Events that steer the
//! recording flow are shown to the [`controller`](crate::transcription::controller)
//! first, then everything is queued for the UI, which drains the channel on its
//! own executor. Nothing here blocks: a send is a lock-free push.

use std::sync::OnceLock;

use crate::api::dto::TranscriptResponse;
use crate::audio::waveform::BANDS;
use crate::transcription::controller::{self, PillState};

/// What the pill shows while a recording runs: the settled words, then the
/// words the decoder may still revise.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Partial {
    pub committed: String,
    pub tentative: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelDownload {
    Queued,
    Running,
    Progress(u8),
    Complete,
    Cancelled,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CudaDownload {
    Progress(u8),
    Complete,
    Cancelled,
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum Event {
    ModelDownload { id: String, status: ModelDownload },
    /// The active model was deleted and nothing else is on disk.
    ModelEvicted,
    /// The active model was deleted and another downloaded one took its place.
    ModelSwitched,
    /// A model switch reset a language the new model cannot speak.
    LanguageReset,
    Cuda(CudaDownload),
    Pill(PillState),
    /// A short message above the pill. The recording, if any, carries on.
    PillNotice(String),
    Waveform([f32; BANDS]),
    Partial(Partial),
    PartialEnd,
    TranscriptSaved(TranscriptResponse),
    /// Bring the main window forward: the tray, or a second launch.
    ShowMainWindow,
    Quit,
}

static SENDER: OnceLock<async_channel::Sender<Event>> = OnceLock::new();
static RECEIVER: OnceLock<async_channel::Receiver<Event>> = OnceLock::new();

fn channel() -> &'static async_channel::Sender<Event> {
    SENDER.get_or_init(|| {
        let (tx, rx) = async_channel::unbounded();
        let _ = RECEIVER.set(rx);
        tx
    })
}

/// Hand `event` to the controller, then queue it for the UI.
pub fn emit(event: Event) {
    controller::observe(&event);
    // Unbounded and never closed while the app runs, so this cannot fail.
    let _ = channel().try_send(event);
}

/// The UI's end of the channel. Events emitted before the UI starts are kept.
pub fn receiver() -> async_channel::Receiver<Event> {
    channel();
    RECEIVER.get().expect("receiver initialised with the sender").clone()
}
