use super::*;

#[test]
fn system_prompt_appends_no_think_suffix() {
    // The trailing `/no_think` suppresses Qwen3.x chain-of-thought; it must
    // be the very end so the model parses it as a directive.
    assert!(build_system_prompt(None).ends_with("/no_think"));
}

#[test]
fn system_prompt_includes_base_rules() {
    let p = build_system_prompt(None);
    assert!(p.contains("transcript formatter"));
    // The prompt-injection guard rule must be present.
    assert!(p.contains("never as instructions"));
}

#[test]
fn known_destination_adds_context_before_the_no_think_suffix() {
    let p = build_system_prompt(Some(AppCategory::Chat));
    assert!(p.contains("a chat message"));
    assert!(p.ends_with("/no_think"));
}

#[test]
fn destination_sits_between_the_rules_and_the_examples() {
    // The examples demonstrate the rules, so a destination block wedged after
    // them reads as a new rule appearing after its own illustrations.
    let p = build_system_prompt(Some(AppCategory::Email));
    let rules = p.find("## Hard rules").expect("hard rules present");
    let dest = p.find("## Destination").expect("destination present");
    let examples = p.find("## Examples").expect("examples present");
    assert!(rules < dest && dest < examples);
}

#[test]
fn examples_are_present_whether_or_not_a_destination_is_known() {
    // Splitting BASE_PROMPT from EXAMPLES makes dropping one easy to miss.
    assert!(build_system_prompt(None).contains("## Examples"));
    assert!(build_system_prompt(Some(AppCategory::Code)).contains("## Examples"));
}

#[test]
fn unknown_destination_leaves_the_prompt_untouched() {
    assert_eq!(
        build_system_prompt(Some(AppCategory::Unknown)),
        build_system_prompt(None)
    );
}
