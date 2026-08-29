//! Formatter LLM dispatcher: routes to the Anthropic Messages API or the
//! OpenAI-compatible chat-completions transport based on `cfg.provider`, and
//! owns the formatting-specific request shaping (system prompt, token cap,
//! reasoning-tag stripping) shared by both.

use std::time::Duration;

use super::anthropic;
use super::config::FormatConfig;
use super::openai::{self, ChatMessage, ChatRequest};
use super::prompt::build_system_prompt;

/// Upper bound on a formatting request. Local models can be slow; cloud is fast.
/// Kept generous so a slow local model doesn't spuriously fail, but bounded so a
/// hung endpoint can't block the transcript paste forever.
const REQUEST_TIMEOUT: Duration = Duration::from_mins(1);

/// Format `raw` via the configured endpoint. Returns the cleaned text.
///
/// Errors are returned (not panicked) so the caller can fall back to the raw
/// transcript — a formatter failure must never drop the user's dictation.
pub async fn format_transcript(cfg: &FormatConfig, raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(String::new());
    }

    // Cap generation relative to input size: formatting roughly preserves
    // length, so ~2× the input tokens (≈ len/4 chars each) plus headroom is
    // plenty. Without a cap, a looping local model generates until the 60 s
    // timeout and the user waits the full minute before the raw fallback.
    let max_tokens = u32::try_from(raw.len() / 2 + 256).unwrap_or(u32::MAX);

    let system = build_system_prompt();

    let active = cfg.active();
    let text = if cfg.provider == "anthropic" {
        anthropic::send(cfg, &system, raw, max_tokens, 0.3, REQUEST_TIMEOUT).await?
    } else {
        let body = ChatRequest {
            model: active.model.trim(),
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: &system,
                },
                ChatMessage {
                    role: "user",
                    content: raw,
                },
            ],
            temperature: 0.3,
            stream: false,
            max_tokens: Some(max_tokens),
        };
        openai::send_chat(cfg, &body, REQUEST_TIMEOUT).await?
    };

    Ok(strip_artifacts(&text))
}

/// Validate a config with a minimal reachability ping — a one-word prompt with a
/// tiny token cap and short timeout. Confirms the endpoint is reachable and the
/// model responds, without paying for a full formatting request. Used by the
/// modal's "Test" button.
pub async fn test_connection(cfg: &FormatConfig) -> Result<(), String> {
    const TEST_TIMEOUT: Duration = Duration::from_secs(15);

    if cfg.provider == "anthropic" {
        return anthropic::send(cfg, "Reply with one word.", "hi", 1, 0.0, TEST_TIMEOUT)
            .await
            .map(|_| ());
    }

    let active = cfg.active();
    let body = ChatRequest {
        model: active.model.trim(),
        messages: vec![ChatMessage {
            role: "user",
            content: "hi",
        }],
        temperature: 0.0,
        stream: false,
        max_tokens: Some(1),
    };
    // A non-empty response isn't required (max_tokens=1 may yield little); a
    // successful HTTP round-trip with a parseable body means we're connected.
    openai::send_chat(cfg, &body, TEST_TIMEOUT)
        .await
        .map(|_| ())
}

/// Strip reasoning tags some models emit (e.g. Qwen `<think>…</think>`) and any
/// stray chat markers, then trim.
fn strip_artifacts(text: &str) -> String {
    let mut s = text.trim();
    if let Some(rest) = s.strip_prefix("<think>") {
        if let Some(end) = rest.find("</think>") {
            s = rest[end + "</think>".len()..].trim_start();
        }
    }
    s.trim().to_string()
}

#[cfg(test)]
#[path = "../../tests/unit/llm/client.rs"]
mod tests;
