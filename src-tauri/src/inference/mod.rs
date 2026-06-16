pub mod downloader;
pub mod engine;
pub mod provider;

pub use engine::WhisperEngine;

pub trait TranscriptionEngine: Send + Sync {
    fn transcribe(&self, samples_16k: &[f32], prompt: &str, beam_size: i32)
        -> Result<String, String>;

    /// Whisper supports the mid-recording chunk pipeline; Parakeet is one-shot.
    fn supports_streaming(&self) -> bool;
}
