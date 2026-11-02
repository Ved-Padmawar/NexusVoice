//! OpenAI-compatible chat-completions transport for the formatting stage.
//!
//! One async `reqwest` call to `{base_url}/chat/completions`. Works against
//! Ollama, `OpenAI`, `OpenRouter`, and any compatible endpoint. The API key is
//! sent as a bearer token only when present.

use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::completion::Completion;
use super::config::FormatConfig;

#[derive(Serialize)]
pub struct ChatRequest<'a> {
    pub model: &'a str,
    pub messages: Vec<ChatMessage<'a>>,
    /// Low temperature → deterministic, faithful formatting.
    pub temperature: f32,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Serialize)]
pub struct ChatMessage<'a> {
    pub role: &'a str,
    pub content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
    finish_reason: Option<String>,
}

impl Choice {
    /// `length` is the API's word for "stopped at `max_tokens`".
    fn truncated(&self) -> bool {
        self.finish_reason.as_deref() == Some("length")
    }
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

/// Send a chat-completions request and return the first choice.
/// Shared by `format_transcript` and `test_connection` in `client.rs`.
pub async fn send_chat(
    cfg: &FormatConfig,
    body: &ChatRequest<'_>,
    timeout: Duration,
) -> Result<Completion, String> {
    // One shared client (connection pool + TLS config) for the process;
    // the timeout differs per call so it is applied per request.
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    let client = CLIENT.get_or_init(reqwest::Client::new);

    let mut req = client.post(cfg.endpoint()).json(body).timeout(timeout);
    let active = cfg.active();
    let key = active.api_key.trim();
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
        .map(|choice| Completion {
            truncated: choice.truncated(),
            text: choice.message.content,
        })
        .ok_or_else(|| "response contained no choices".to_string())
}

#[cfg(test)]
#[path = "../../../tests/unit/text/llm/openai.rs"]
mod tests;
