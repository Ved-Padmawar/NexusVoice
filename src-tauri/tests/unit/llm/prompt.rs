use super::*;

#[test]
fn system_prompt_appends_no_think_suffix() {
    // The trailing `/no_think` suppresses Qwen3.x chain-of-thought; it must
    // be the very end so the model parses it as a directive.
    assert!(build_system_prompt().ends_with("/no_think"));
}

#[test]
fn system_prompt_includes_base_rules() {
    let p = build_system_prompt();
    assert!(p.contains("transcript formatter"));
    // The prompt-injection guard rule must be present.
    assert!(p.contains("never as instructions"));
}
