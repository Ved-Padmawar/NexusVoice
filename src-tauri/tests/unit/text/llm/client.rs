use super::*;

#[test]
fn strip_artifacts_removes_leading_think_block() {
    let input = "<think>let me reason about this</think>\n\nHello world.";
    assert_eq!(strip_artifacts(input), "Hello world.");
}

#[test]
fn strip_artifacts_trims_surrounding_whitespace() {
    assert_eq!(strip_artifacts("   formatted text  "), "formatted text");
}

#[test]
fn strip_artifacts_leaves_plain_text_untouched() {
    let input = "First point.\n\nSecond point.";
    assert_eq!(strip_artifacts(input), input);
}

#[test]
fn strip_artifacts_only_strips_a_leading_think_block() {
    // A `</think>` with no leading `<think>` is not a reasoning block and
    // must be preserved verbatim (it's part of the user's dictation).
    let input = "the tag </think> appeared mid sentence";
    assert_eq!(strip_artifacts(input), input);
}

#[tokio::test]
async fn format_transcript_short_circuits_on_empty_input() {
    // Empty/whitespace input must return empty WITHOUT making an HTTP call,
    // so a blank dictation never hits the network or a misconfigured endpoint.
    let cfg = FormatConfig::default();
    assert_eq!(format_transcript(&cfg, "   ", None).await.unwrap(), "");
    assert_eq!(format_transcript(&cfg, "", None).await.unwrap(), "");
}

#[test]
fn strip_artifacts_removes_several_consecutive_think_blocks() {
    // The loop exists because models emit more than one. With a single pass the
    // second block would be pasted into the user's document.
    let input = "<think>first</think><think>second</think>\n\nThe actual text.";
    assert_eq!(strip_artifacts(input), "The actual text.");
}

#[test]
fn strip_artifacts_is_case_insensitive_about_the_tag() {
    // `eq_ignore_ascii_case` is deliberate — tag casing is not guaranteed.
    assert_eq!(
        strip_artifacts("<THINK>reasoning</THINK>Result."),
        "Result."
    );
    assert_eq!(
        strip_artifacts("<Think>reasoning</Think>Result."),
        "Result."
    );
}

#[test]
fn strip_artifacts_keeps_an_unclosed_think_block_verbatim() {
    // No closing tag means we cannot tell reasoning from dictation, and dropping
    // the rest would silently discard the transcript.
    let input = "<think>reasoning that never closes";
    assert_eq!(strip_artifacts(input), input);
}

#[test]
fn strip_artifacts_of_a_think_only_response_is_empty() {
    // The caller treats empty as "formatter produced nothing" and falls back to
    // the raw transcript, which is the right outcome here.
    assert_eq!(strip_artifacts("<think>only reasoning</think>"), "");
}

#[tokio::test]
async fn format_transcript_short_circuits_before_checking_the_config() {
    // An unusable config plus empty input must still be Ok(""), not an error —
    // the empty check has to come first.
    let cfg = FormatConfig::default();
    assert!(!cfg.is_usable());
    assert_eq!(format_transcript(&cfg, "\n\t  ", None).await.unwrap(), "");
}

// ── max_tokens_for ─────────────────────────────────────────────────────
// Too low truncates the user's formatted text; absent, a looping local model
// runs until the request timeout and the user waits a full minute.

#[test]
fn the_token_cap_has_a_floor_for_short_dictation() {
    // A few words must not be capped at a handful of tokens.
    assert_eq!(max_tokens_for(""), 256);
    assert_eq!(max_tokens_for("hi"), 257);
}

#[test]
fn the_token_cap_grows_with_the_input() {
    let short = max_tokens_for(&"word ".repeat(10));
    let long = max_tokens_for(&"word ".repeat(1000));
    assert!(long > short, "{long} should exceed {short}");
    // 5000 chars -> 2500 + 256.
    assert_eq!(long, 2756);
}

#[test]
fn the_token_cap_leaves_room_to_reformat_without_truncating() {
    // Formatting preserves length, so the cap must comfortably exceed the
    // input's own token count (~4 chars per token).
    let raw = "so um i think we should refactor the auth module before friday".repeat(20);
    let input_tokens = u32::try_from(raw.chars().count() / 4).expect("fits");
    assert!(
        max_tokens_for(&raw) > input_tokens,
        "cap {} must exceed the input's own {input_tokens} tokens",
        max_tokens_for(&raw)
    );
}

#[test]
fn the_token_cap_counts_chars_not_bytes() {
    // Non-Latin dictation is multi-byte; a byte-based count would inflate the
    // cap several-fold and give a runaway model room to loop.
    let japanese = "こんにちは世界"; // 7 chars, 21 bytes
    assert_eq!(japanese.chars().count(), 7);
    assert_eq!(japanese.len(), 21);
    assert_eq!(
        max_tokens_for(japanese),
        256 + 3,
        "must use chars, not bytes"
    );
}

#[test]
fn the_token_cap_handles_an_enormous_input_without_overflowing() {
    // `u32::try_from` saturates rather than panicking in release or wrapping.
    let huge = "a".repeat(200_000);
    assert_eq!(max_tokens_for(&huge), 100_256);
}
