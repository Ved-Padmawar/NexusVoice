//! Plain-sentence messages for a failed formatter request. They reach the
//! user (the connection test) and the log, so they carry the host only —
//! never the request's URL, which can hold a key or a path — and none of
//! reqwest's own wording. A server's own reason for an error is passed on.

use reqwest::StatusCode;
use serde::Deserialize;

/// Why a request never got an answer.
pub fn request_failed(error: &reqwest::Error) -> String {
    let host = error.url().and_then(host_and_port);
    if error.is_builder() {
        return INVALID_URL.to_string();
    }
    if error.is_timeout() {
        return match host {
            Some(host) => format!("{host} took too long to answer."),
            None => "The server took too long to answer.".to_string(),
        };
    }
    if error.is_connect() {
        return match host {
            Some(host) => format!("Couldn't reach {host}. Is the server running?"),
            None => "Couldn't reach the server. Is it running?".to_string(),
        };
    }
    "The request to the server failed.".to_string()
}

/// What an error status means for the person who configured the formatter,
/// with the server's own reason when its body gives one.
pub async fn status_failed(response: reqwest::Response) -> String {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    describe_status(status, server_message(&body).as_deref())
}

fn describe_status(status: StatusCode, reason: Option<&str>) -> String {
    let summary = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => "The API key was rejected".to_string(),
        StatusCode::NOT_FOUND => "No model or endpoint found at that address".to_string(),
        other => format!("The server answered with an error ({})", other.as_u16()),
    };
    match reason {
        Some(reason) => format!("{summary}: {reason}"),
        None => format!("{summary}."),
    }
}

/// Every provider puts the reason under a top-level `error`: an object with a
/// `message`, or a bare string as Ollama's native API sends.
#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorField,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ErrorField {
    Text(String),
    Object { message: String },
}

/// A server can put anything in its message.
const MAX_REASON_CHARS: usize = 300;

fn server_message(body: &str) -> Option<String> {
    let ErrorBody { error } = serde_json::from_str(body).ok()?;
    let (ErrorField::Text(message) | ErrorField::Object { message }) = error;
    let message = message.trim();
    (!message.is_empty()).then(|| message.chars().take(MAX_REASON_CHARS).collect())
}

/// The server answered with something that is not a chat completion.
pub const UNEXPECTED_REPLY: &str = "The server's reply wasn't in the expected format.";

const INVALID_URL: &str = "That base URL isn't valid.";

fn host_and_port(url: &reqwest::Url) -> Option<String> {
    let host = url.host_str()?;
    Some(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    })
}

#[cfg(test)]
#[path = "../../../tests/unit/text/llm/error.rs"]
mod tests;
