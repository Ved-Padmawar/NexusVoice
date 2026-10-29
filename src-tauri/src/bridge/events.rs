//! Every event the backend emits. Each is a type registered in
//! `command_bindings()`, so `bindings.ts` carries its name and payload and the
//! frontend never spells either by hand.

use serde::Serialize;
use specta::Type;
use tauri_specta::Event;

use crate::audio::waveform::BANDS;
use crate::bridge::commands::dto::TranscriptResponse;
use crate::settings::PillSettingsSnapshot;

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

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct TranscriptSaved(pub TranscriptResponse);

/// Capture levels for the pill waveform, one per band, 0–1.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct PillWaveform(pub [f32; BANDS]);

/// The pill's preferences changed. Carries the revision so a window can drop
/// an update older than the snapshot it already holds.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct PillSettingsChanged(pub PillSettingsSnapshot);

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct ModelDownloadStart {
    pub id: String,
}

/// A queued download got a transfer slot.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct ModelDownloadRunning {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct ModelDownloadProgress {
    pub id: String,
    pub pct: u8,
}

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct ModelDownloadComplete {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct ModelDownloadError {
    pub id: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct ModelDownloadCancelled {
    pub id: String,
}

/// The active model was deleted and no other is on disk.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct ModelEvicted;

/// The active model was deleted and another on disk took its place.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct ModelSwitched;

/// The new model cannot speak the saved dictation language, so it was reset.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct LanguageReset;

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct CudaDownloadProgress(pub u8);

/// Every file is downloaded; the pack is being unpacked and checked.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct CudaDownloadVerifying;

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct CudaDownloadComplete;

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct CudaDownloadCancelled;

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct CudaDownloadError(pub String);
