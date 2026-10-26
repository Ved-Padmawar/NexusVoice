//! The error every command returns, and the mapping from each owner's errors.

use serde::Serialize;

use crate::database::DbUnavailable;
use crate::hotkeys::HotkeyError;
use crate::transcription::RecordError;

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
        Self::new("io_error", value.to_string())
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
            RecordError::Capture(e) => Self::new("capture_failed", e),
        }
    }
}

impl From<HotkeyError> for ApiError {
    fn from(value: HotkeyError) -> Self {
        match value {
            HotkeyError::Conflict => Self::new("hotkey_conflict", "hotkeys must be different"),
            HotkeyError::InUse => Self::new(
                "hotkey_already_in_use",
                "this hotkey is already in use by another application",
            ),
            HotkeyError::PermissionDenied => Self::new(
                "hotkey_permission_denied",
                "OS denied hotkey registration - try a different combination",
            ),
            HotkeyError::Register(e) => Self::new("hotkey_register_failed", e),
            HotkeyError::Unregister(e) => Self::new("hotkey_unregister_failed", e),
            HotkeyError::Save(e) => Self::new("io_error", e),
        }
    }
}
