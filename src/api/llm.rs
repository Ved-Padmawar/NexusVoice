//! The formatter LLM: config and an endpoint test.

use crate::llm::config::FormatConfig;
use crate::state;

use super::error::ApiError;

pub fn get_format_config() -> FormatConfig {
    state::get().load_format_config()
}

pub fn set_format_config(config: &FormatConfig) -> Result<(), ApiError> {
    state::get()
        .save_format_config(config)
        .map_err(|e| ApiError::new("io_error", e.to_string()))
}

/// Send one trivial request with a candidate config; nothing is saved.
pub async fn test_format_connection(config: &FormatConfig) -> Result<(), ApiError> {
    let active = config.active();
    if config.provider != "anthropic" && active.base_url.trim().is_empty() {
        return Err(ApiError::new("invalid_input", "base URL is required"));
    }
    if active.model.trim().is_empty() {
        return Err(ApiError::new("invalid_input", "model name is required"));
    }
    crate::llm::client::test_connection(config)
        .await
        .map_err(|e| ApiError::new("connection_failed", e))
}
