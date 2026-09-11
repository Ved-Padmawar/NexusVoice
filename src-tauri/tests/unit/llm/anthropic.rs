use super::*;

#[test]
fn concat_text_blocks_joins_multiple_text_blocks() {
    let parsed = Response {
        content: vec![
            Block::Text {
                text: "Hello, ".to_string(),
            },
            Block::Text {
                text: "world.".to_string(),
            },
        ],
    };
    assert_eq!(concat_text_blocks(parsed), "Hello, world.");
}

#[test]
fn concat_text_blocks_skips_non_text_blocks() {
    let parsed = Response {
        content: vec![
            Block::Other,
            Block::Text {
                text: "kept".to_string(),
            },
        ],
    };
    assert_eq!(concat_text_blocks(parsed), "kept");
}

#[test]
fn concat_text_blocks_empty_content_is_empty_string() {
    let parsed = Response { content: vec![] };
    assert_eq!(concat_text_blocks(parsed), "");
}

// ── Wire shape ─────────────────────────────────────────────────────────
// The tests above build `Response` in Rust, which proves the concatenation but
// never exercises serde. These parse the actual API shapes instead, so a tag
// name or rename_all regression fails here rather than against the live API.

#[test]
fn a_messages_api_response_parses_into_text_blocks() {
    let json = r#"{
        "id": "msg_01",
        "type": "message",
        "role": "assistant",
        "model": "claude-sonnet-5",
        "content": [{"type": "text", "text": "Formatted text."}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 10, "output_tokens": 3}
    }"#;
    let parsed: Response = serde_json::from_str(json).expect("parse");
    assert_eq!(concat_text_blocks(parsed), "Formatted text.");
}

#[test]
fn a_non_text_block_is_skipped_by_its_type_tag() {
    // The `#[serde(other)]` fallback must absorb any block kind the API adds,
    // rather than failing the parse and losing the transcript.
    let json = r#"{"content": [
        {"type": "thinking", "thinking": "reasoning we must not paste"},
        {"type": "text", "text": "kept"},
        {"type": "tool_use", "id": "t1", "name": "x", "input": {}}
    ]}"#;
    let parsed: Response = serde_json::from_str(json).expect("parse");
    assert_eq!(concat_text_blocks(parsed), "kept");
}

#[test]
fn a_response_with_no_content_field_parses_as_empty() {
    // `#[serde(default)]` on `content` is what makes this not an error.
    let parsed: Response = serde_json::from_str(r#"{"id": "msg_02"}"#).expect("parse");
    assert_eq!(concat_text_blocks(parsed), "");
}

#[test]
fn the_request_sends_system_as_a_top_level_field() {
    // This is the whole reason this module exists instead of reusing the
    // OpenAI transport: Anthropic takes `system` top-level, not as a message.
    let body = Request {
        model: "claude-sonnet-5",
        system: "you are a transcript formatter",
        messages: vec![Message {
            role: "user",
            content: "hello world",
        }],
        max_tokens: 512,
        temperature: 0.3,
    };
    let v: serde_json::Value = serde_json::to_value(&body).expect("serialize");

    assert_eq!(v["system"], "you are a transcript formatter");
    assert_eq!(v["model"], "claude-sonnet-5");
    assert_eq!(v["max_tokens"], 512);
    assert_eq!(v["messages"].as_array().expect("messages").len(), 1);
    assert_eq!(v["messages"][0]["role"], "user");
    assert_eq!(
        v["messages"][0]["content"], "hello world",
        "the system prompt must not be folded into the messages array"
    );
}

#[test]
fn the_endpoint_and_api_version_are_pinned() {
    // Anthropic has no user-supplied base URL, so these constants are the only
    // thing aiming the request. `anthropic-version` is required by the API.
    assert_eq!(ENDPOINT, "https://api.anthropic.com/v1/messages");
    assert_eq!(API_VERSION, "2023-06-01");
}
