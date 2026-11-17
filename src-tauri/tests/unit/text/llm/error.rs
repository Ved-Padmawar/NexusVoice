use super::*;

#[tokio::test]
async fn a_refused_connection_names_the_host_but_not_the_url() {
    let error = reqwest::Client::new()
        .post("http://127.0.0.1:1/v1/chat/completions?key=secret")
        .send()
        .await
        .expect_err("nothing listens on port 1");

    assert_eq!(
        request_failed(&error),
        "Couldn't reach 127.0.0.1:1. Is the server running?"
    );
}

/// Each provider's documented error body.
#[test]
fn the_reason_is_read_from_every_providers_error_body() {
    for (provider, body) in [
        (
            "OpenAI, Ollama /v1",
            r#"{"error":{"message":"Add usage credits","type":"api_error","param":null,"code":null}}"#,
        ),
        (
            "Anthropic",
            r#"{"type":"error","error":{"type":"billing_error","message":"Add usage credits"},"request_id":"req_1"}"#,
        ),
        (
            "OpenRouter",
            r#"{"error":{"code":402,"message":"Add usage credits","metadata":{"provider":"x"}}}"#,
        ),
        ("Ollama native", r#"{"error":"Add usage credits"}"#),
    ] {
        assert_eq!(
            describe_status(
                StatusCode::PAYMENT_REQUIRED,
                server_message(body).as_deref()
            ),
            "The server answered with an error (402): Add usage credits",
            "{provider}"
        );
    }
}

#[test]
fn a_body_without_a_reason_keeps_the_status_sentence() {
    for body in [
        "",
        "<html>Bad Gateway</html>",
        r#"{"error":{"message":"  "}}"#,
        r#"{"detail":"x"}"#,
    ] {
        assert_eq!(
            describe_status(StatusCode::BAD_GATEWAY, server_message(body).as_deref()),
            "The server answered with an error (502).",
            "{body}"
        );
    }
}
