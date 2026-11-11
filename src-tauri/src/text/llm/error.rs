//! Plain-sentence messages for a failed formatter request. They reach the
//! user (the connection test) and the log, so they carry the host only —
//! never a full URL, which can hold a key or a path — and none of reqwest's
//! own wording.

use reqwest::StatusCode;

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

/// What an error status means for the person who configured the formatter.
pub fn status_failed(status: StatusCode) -> String {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => "The API key was rejected.".to_string(),
        StatusCode::NOT_FOUND => "No model or endpoint found at that address.".to_string(),
        other => format!("The server answered with an error ({}).", other.as_u16()),
    }
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
