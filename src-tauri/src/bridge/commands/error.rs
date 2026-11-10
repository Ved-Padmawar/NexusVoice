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
        let message = match value {
            sqlx::Error::RowNotFound => "record not found",
            sqlx::Error::PoolClosed => "database unavailable",
            sqlx::Error::Io(_) => "database io error",
            _ => "database error",
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
                "transcription already running",
            ),
            RecordError::NotRunning => {
                Self::new("transcription_not_running", "transcription not running")
            }
            RecordError::DictationRunning => Self::new(
                "dictation_running",
                "dictation is running; use commit_dictation",
            ),
            RecordError::NotDictating => {
                Self::new("dictation_not_running", "dictation is not running")
            }
            RecordError::NotRecording => {
                Self::new("dictation_not_recording", "dictation is not recording")
            }
            RecordError::NotPaused => Self::new("dictation_not_paused", "dictation is not paused"),
            RecordError::Finalizing => {
                Self::new("dictation_finalizing", "dictation is already finalizing")
            }
            RecordError::Unavailable(e) => Self::new("model_unavailable", e),
            RecordError::Capture(e) => Self::new("capture_failed", e),
            RecordError::Failed => {
                Self::new("recorder_failed", "the recorder hit an internal error")
            }
            RecordError::Restarting => Self::new("restarting", "NexusVoice is restarting"),
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
