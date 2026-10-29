//! The OpenAI-compatible wire shape. Every provider but Anthropic goes through
//! here, so a serde regression breaks formatting against all of them at once —
//! and only at runtime, against a live endpoint.

use super::*;

fn request() -> ChatRequest<'static> {
    ChatRequest {
        model: "qwen2.5-3b-instruct",
        messages: vec![
            ChatMessage {
                role: "system",
                content: "format this",
            },
            ChatMessage {
                role: "user",
                content: "hello world",
            },
        ],
        temperature: 0.3,
        stream: false,
        max_tokens: Some(512),
    }
}

#[test]
fn request_serializes_the_fields_the_api_expects() {
    let v: serde_json::Value = serde_json::to_value(request()).expect("serialize");

    assert_eq!(v["model"], "qwen2.5-3b-instruct");
    // f32 widens to f64 in JSON, so compare with tolerance.
    let temperature = v["temperature"].as_f64().expect("temperature is a number");
    assert!((temperature - 0.3).abs() < 1e-6, "{temperature}");
    assert_eq!(v["max_tokens"], 512);
    assert_eq!(v["messages"][0]["role"], "system");
    assert_eq!(v["messages"][0]["content"], "format this");
    assert_eq!(v["messages"][1]["role"], "user");
    assert_eq!(v["messages"][1]["content"], "hello world");
}

#[test]
fn request_always_asks_for_a_non_streamed_response() {
    // `send_chat` parses one whole JSON body. If `stream` were ever true or
    // omitted-and-defaulted-on, the response would be SSE and fail to parse.
    let v: serde_json::Value = serde_json::to_value(request()).expect("serialize");
    assert_eq!(v["stream"], false, "stream must be sent, and sent as false");
}

#[test]
fn an_absent_token_cap_is_omitted_rather_than_sent_as_null() {
    // Some OpenAI-compatible servers reject an explicit null max_tokens.
    let mut body = request();
    body.max_tokens = None;
    let v: serde_json::Value = serde_json::to_value(&body).expect("serialize");
    assert!(v.get("max_tokens").is_none(), "{v}");
}

#[test]
fn a_response_yields_the_first_choices_content() {
    let json = r#"{
        "id": "chatcmpl-1",
        "object": "chat.completion",
        "choices": [
            {"index": 0, "message": {"role": "assistant", "content": "Formatted text."},
             "finish_reason": "stop"}
        ],
        "usage": {"prompt_tokens": 10, "completion_tokens": 3}
    }"#;
    let parsed: ChatResponse = serde_json::from_str(json).expect("parse");
    assert_eq!(
        parsed.choices.into_iter().next().map(|c| c.message.content),
        Some("Formatted text.".to_string())
    );
}

#[test]
fn unknown_response_fields_are_ignored() {
    // Providers add fields freely (logprobs, reasoning, citations…); an unknown
    // one must not fail the parse and lose the user's formatted text.
    let json = r#"{
        "choices": [{"message": {"content": "kept", "reasoning": "ignored"},
                     "some_new_field": 42}],
        "another_new_field": {"nested": true}
    }"#;
    let parsed: ChatResponse = serde_json::from_str(json).expect("parse");
    assert_eq!(parsed.choices[0].message.content, "kept");
}

#[test]
fn a_response_with_no_choices_parses_but_has_nothing_to_return() {
    // `send_chat` turns this into an error rather than an empty transcript.
    let parsed: ChatResponse = serde_json::from_str(r#"{"choices": []}"#).expect("parse");
    assert!(parsed.choices.is_empty());
}
