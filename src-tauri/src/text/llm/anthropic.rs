//! Native Anthropic Messages API transport — separate from `client.rs` because
//! the wire shape differs (`system` is top-level, auth is `x-api-key`).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::completion::Completion;
use super::config::FormatConfig;
use super::error;

const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";

#[derive(Serialize)]
struct Request<'a> {
    model: &'a str,
    system: &'a str,
    messages: Vec<Message<'a>>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    content: Vec<Block>,
    stop_reason: Option<String>,
}

impl Response {
    /// The reply stopped at a limit rather than finishing.
    fn truncated(&self) -> bool {
        matches!(
            self.stop_reason.as_deref(),
            Some("max_tokens" | "model_context_window_exceeded")
        )
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Block {
    Text {
        text: String,
    },
    #[serde(other)]
    Other,
}

/// Send one Anthropic Messages API request and return the concatenated text
/// blocks. Shared by `format_transcript` and `test_connection` in `client.rs`.
pub async fn send(
    cfg: &FormatConfig,
    system: &str,
    user: &str,
    max_tokens: u32,
    temperature: f32,
    timeout: Duration,
) -> Result<Completion, String> {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    let client = CLIENT.get_or_init(reqwest::Client::new);
    let active = cfg.active();

    let body = Request {
        model: active.model.trim(),
        system,
        messages: vec![Message {
            role: "user",
            content: user,
        }],
        max_tokens,
        temperature,
    };

    let mut req = client
        .post(ENDPOINT)
        .header("anthropic-version", API_VERSION)
        .json(&body)
        .timeout(timeout);

    let key = active.api_key.trim();
    if !key.is_empty() {
        req = req.header("x-api-key", key);
    }

    let resp = req.send().await.map_err(|e| error::request_failed(&e))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(error::status_failed(status));
    }

    let parsed: Response = resp
        .json()
        .await
        .map_err(|_| error::UNEXPECTED_REPLY.to_string())?;

    Ok(Completion {
        truncated: parsed.truncated(),
        text: concat_text_blocks(parsed),
    })
}

/// Pure, so it's testable without a transport.
fn concat_text_blocks(parsed: Response) -> String {
    let mut text = String::new();
    for block in parsed.content {
        if let Block::Text { text: t } = block {
            text.push_str(&t);
        }
    }
    text
}

#[cfg(test)]
#[path = "../../../tests/unit/text/llm/anthropic.rs"]
mod tests;
