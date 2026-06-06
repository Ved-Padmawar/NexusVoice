//! OpenAI-compatible chat client for the formatting stage.
//!
//! One async `reqwest` call to `{base_url}/chat/completions`. Works against
//! Ollama, `OpenAI`, `OpenRouter`, and any compatible endpoint. The API key is
//! sent as a bearer token only when present.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::config::FormatConfig;
use super::prompt::build_system_prompt;

/// Upper bound on a formatting request. Local models can be slow; cloud is fast.
/// Kept generous so a slow local model doesn't spuriously fail, but bounded so a
/// hung endpoint can't block the transcript paste forever.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    /// Low temperature → deterministic, faithful formatting.
    temperature: f32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

/// Format `raw` via the configured endpoint. Returns the cleaned text.
///
/// Errors are returned (not panicked) so the caller can fall back to the raw
/// transcript — a formatter failure must never drop the user's dictation.
pub async fn format_transcript(cfg: &FormatConfig, raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(String::new());
    }

    let system = build_system_prompt();
    let body = ChatRequest {
        model: cfg.model.trim(),
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
        max_tokens: None,
    };

    let text = send_chat(cfg, &body, REQUEST_TIMEOUT).await?;
    Ok(strip_artifacts(&text))
}

/// Validate a config with a minimal reachability ping — a one-word prompt with a
/// tiny token cap and short timeout. Confirms the endpoint is reachable and the
/// model responds, without paying for a full formatting request. Used by the
/// modal's "Test" button.
pub async fn test_connection(cfg: &FormatConfig) -> Result<(), String> {
    const TEST_TIMEOUT: Duration = Duration::from_secs(15);
    let body = ChatRequest {
        model: cfg.model.trim(),
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
    send_chat(cfg, &body, TEST_TIMEOUT).await.map(|_| ())
}

/// Send a chat-completions request and return the first choice's content.
/// Shared by `format_transcript` and `test_connection`.
async fn send_chat(
    cfg: &FormatConfig,
    body: &ChatRequest<'_>,
    timeout: Duration,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("http client init: {e}"))?;

    let mut req = client.post(cfg.endpoint()).json(body);
    let key = cfg.api_key.trim();
    if !key.is_empty() {
        req = req.bearer_auth(key);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let detail = resp.text().await.unwrap_or_default();
        let snippet: String = detail.chars().take(200).collect();
        return Err(format!("HTTP {status}: {snippet}"));
    }

    let parsed: ChatResponse = resp
        .json()
        .await
        .map_err(|e| format!("invalid response body: {e}"))?;

    parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| "response contained no choices".to_string())
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
