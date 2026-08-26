//! Transcription decode paths, chosen by what the loaded model can do:
//!
//! - [`local_agreement`] — single-shot models. Re-decodes a growing window and
//!   confirms text with LocalAgreement-2, trimming at segment boundaries.
//! - [`streaming`] — models that own their session state. Audio is fed in as it
//!   arrives and the model reports what it has committed.

pub mod local_agreement;
pub mod streaming;

pub use local_agreement::{finalize, StreamingSession};
pub use streaming::StreamSession;

use crate::inference::TranscriptionEngine;

/// Which decode path a recording will use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Growing-window re-decode with LocalAgreement-2 confirmation.
    LocalAgreement,
    /// The model's own incremental session.
    Streaming,
}

impl Route {
    /// Choose the path for a loaded engine.
    pub fn for_engine(engine: &TranscriptionEngine) -> Self {
        if engine.supports_streaming() {
            Self::Streaming
        } else {
            Self::LocalAgreement
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LocalAgreement => "local-agreement",
            Self::Streaming => "streaming",
        }
    }
}
