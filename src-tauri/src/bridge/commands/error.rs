//! The error every command returns, and the mapping from each owner's errors.

use serde::Serialize;

use crate::data::DbUnavailable;
use crate::platform::hotkeys::HotkeyError;
use crate::session::RecordError;

#[derive(Debug, Serialize, specta::Type)]
pub struct ApiError {
    code: String,
    message: String,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl From<DbUnavailable> for ApiError {
    fn from(value: DbUnavailable) -> Self {
        Self::new("database_unavailable", value.0)
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(value: sqlx::Error) -> Self {
        log::warn!("database operation failed: {value}");
        let message = match value {
            sqlx::Error::RowNotFound => "That item no longer exists.",
            sqlx::Error::Io(_) => "Couldn't read your history from disk.",
            _ => "Couldn't reach your history. Try again in a moment.",
        };
        Self::new("database_error", message)
    }
}

impl From<std::io::Error> for ApiError {
    fn from(value: std::io::Error) -> Self {
        log::warn!("file operation failed: {value}");
        let message = match value.kind() {
            std::io::ErrorKind::StorageFull => "Your disk is full.",
            std::io::ErrorKind::PermissionDenied => {
                "NexusVoice doesn't have permission to do that."
            }
            std::io::ErrorKind::ResourceBusy => "That file is in use. Try again in a moment.",
            _ => "Couldn't read or write a file.",
        };
        Self::new("io_error", message)
    }
}

impl From<RecordError> for ApiError {
    fn from(value: RecordError) -> Self {
        match value {
            RecordError::AlreadyRunning => Self::new(
                "transcription_already_running",
                "A recording is already running.",
            ),
            RecordError::NotRunning => {
                Self::new("transcription_not_running", "No recording is running.")
            }
            RecordError::DictationRunning => Self::new(
                "dictation_running",
                "A dictation is already running. Finish it first.",
            ),
            RecordError::NotDictating => {
                Self::new("dictation_not_running", "No dictation is running.")
            }
            RecordError::NotRecording => {
                Self::new("dictation_not_recording", "There's no dictation to pause.")
            }
            RecordError::NotPaused => Self::new(
                "dictation_not_paused",
                "There's no paused dictation to resume.",
            ),
            RecordError::Finalizing => Self::new(
                "dictation_finalizing",
                "The dictation is already finishing.",
            ),
            RecordError::Unavailable(e) => Self::new("model_unavailable", e),
            RecordError::Capture(e) => Self::new("capture_failed", e),
            RecordError::Failed => Self::new(
                "recorder_failed",
                "Recording stopped unexpectedly. Try again.",
            ),
            RecordError::Restarting => Self::new("restarting", "NexusVoice is restarting."),
        }
    }
}

impl From<HotkeyError> for ApiError {
    fn from(value: HotkeyError) -> Self {
        match value {
            HotkeyError::Conflict { hotkey, with } => Self::new(
                "hotkey_conflict",
                format!("{hotkey} is already used for {}.", with.label()),
            ),
            HotkeyError::InUse => Self::new(
                "hotkey_already_in_use",
                "Another application is already using that shortcut.",
            ),
            HotkeyError::PermissionDenied => Self::new(
                "hotkey_permission_denied",
                "The system blocked that shortcut. Try a different combination.",
            ),
            HotkeyError::Register(e) => {
                log::warn!("hotkey registration failed: {e}");
                Self::new(
                    "hotkey_register_failed",
                    "That key combination isn't supported.",
                )
            }
            HotkeyError::Unregister(e) => {
                log::warn!("hotkey release failed: {e}");
                Self::new(
                    "hotkey_unregister_failed",
                    "Couldn't release the current shortcuts. Try again.",
                )
            }
            HotkeyError::Save(e) => {
                log::warn!("saving hotkeys failed: {e}");
                Self::new("io_error", "Couldn't save the shortcut.")
            }
        }
    }
}
