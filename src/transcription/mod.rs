//! Transcription orchestration.
//!
//! [`controller`] owns the recording flow and what the pill shows; [`service`]
//! runs capture, streaming decode and finalize (preprocess → decode →
//! post-process → persist).

pub mod controller;
pub mod service;

pub use service::{spawn_finalize, start_capture};
