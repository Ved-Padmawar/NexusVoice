//! Every event the backend emits. Each is a type registered in
//! `command_bindings()`, so `bindings.ts` carries its name and payload and the
//! frontend never spells either by hand.
//!
//! Saved data reaches the windows one way: its owner announces `Changed`, and
//! every query that reads it reads again. The other events are streams (the
//! recording, the live transcript, the waveform) and notices of something that
//! happened, for a toast or a one-off reaction. Neither carries state a window
//! caches.

use serde::Serialize;
use specta::Type;
use tauri_specta::Event;

use crate::audio::waveform::BANDS;
use crate::sync::Topic;

/// An owner's data changed; read it again.
#[derive(Debug, Clone, Copy, Serialize, Type, Event)]
pub struct Changed(pub Topic);

/// Every transition of the recording, each with its revision, so a window
/// drops one older than the snapshot it holds.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct RecordingStateChanged(pub crate::session::recorder::RecordingSnapshot);

/// Live transcript while recording. Both decode paths produce the same split,
/// so the pill never needs to know which one ran.
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionPartial {
    pub committed: String,
    pub tentative: String,
}

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct TranscriptionPartialEnd;

/// A recording finished. Saving and pasting are reported separately: a failed
/// paste leaves the transcript in history, a failed save still pastes.
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionComplete {
    /// Why the transcript could not be pasted; `None` when it was, or when
    /// there was nothing to paste.
    pub paste_error: Option<String>,
    /// The transcript is in history. A failed paste may only point there
    /// when this is set.
    pub saved: bool,
}

/// A recording produced no transcript. The message is for the user.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct TranscriptionError(pub String);

/// Something the user should know about while the recording carries on.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct TranscriptionWarning(pub String);

/// A transcription job ended, by finishing, being cancelled or failing.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct JobFinished(pub crate::jobs::JobFinish);

/// Capture levels for the pill waveform, one per band, 0–1.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct PillWaveform(pub [f32; BANDS]);

/// A model download was cancelled, from any window.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct ModelDownloadCancelled {
    pub id: String,
}

/// The new model cannot speak the saved dictation language, so it was reset.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct LanguageReset;

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct CudaDownloadComplete;

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct CudaDownloadError(pub String);
