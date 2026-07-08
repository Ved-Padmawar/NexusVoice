//! Transcription orchestration service.
//!
//! Owns the recording lifecycle: starting microphone capture, then finalizing
//! the captured audio in one shot on stop
//! (preprocess → parakeet.cpp → dictionary post-process → persist → emit).
//!
//! The command layer (`commands::transcription`) only validates the running
//! state and delegates here, so all the heavy logic lives in one place and is
//! ready for a future LLM-formatting stage to slot in after `finalize`.

pub mod service;

pub use service::{spawn_finalize, start_capture};
