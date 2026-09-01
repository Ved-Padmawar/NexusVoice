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
