//! Optional LLM formatting stage. After Whisper produces the raw transcript,
//! an `OpenAI`-compatible chat endpoint (local Ollama, `OpenAI`, `OpenRouter`, or
//! any custom endpoint) reformats it — punctuation, structure, list inference —
//! before paste. Disabled by default; configured via the formatter settings
//! modal. There is no in-process model: this is purely an HTTP call, so it adds
//! no native dependencies and cannot conflict with whisper's ggml.

pub mod client;
pub mod config;
pub mod prompt;

pub use config::FormatConfig;
