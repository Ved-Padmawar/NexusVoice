//! Speech models: the catalog, downloads, the loaded engine, the CUDA pack,
//! and the audio preprocessing and decode paths that run on them.

pub mod cache;
pub mod catalog;
pub mod cuda;
pub mod decode;
pub mod downloader;
pub mod engine;
pub mod idle;
pub mod language;
pub mod library;
pub mod preprocess;
pub mod provider;
pub mod transcript;

pub use engine::{Pass, TranscriptionEngine};
pub use transcript::TimedSegment;
