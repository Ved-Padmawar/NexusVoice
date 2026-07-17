//! Transcription orchestration service.
//!
//! Owns the streaming recording lifecycle: starting microphone capture, the
//! mid-recording chunk poller, and finalizing the transcript on stop
//! (preprocess → whisper → dictionary post-process → persist → emit).
//!
//! The command layer (`commands::transcription`) only validates the running
//! state and delegates here, so all the heavy logic lives in one place and is
//! ready for a future LLM-formatting stage to slot in after `finalize`.

pub mod service;

pub use service::{spawn_finalize, start_capture};
