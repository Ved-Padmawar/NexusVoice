use super::*;

#[test]
fn known_executable_resolves_to_display_name_and_category() {
    let target = FocusTarget::from_executable("Code");
    assert_eq!(target.name, "VS Code");
    assert_eq!(target.category, AppCategory::Code);
}

#[test]
fn lookup_is_case_insensitive() {
    // Win32 reports whatever casing is on disk, which varies by installer.
    assert_eq!(
        FocusTarget::from_executable("SLACK"),
        FocusTarget::from_executable("slack")
    );
}

#[test]
fn unknown_executable_keeps_its_name_and_has_no_category() {
    let target = FocusTarget::from_executable("someapp");
    assert_eq!(target.name, "someapp");
    assert_eq!(target.category, AppCategory::Unknown);
}

#[test]
fn unknown_category_contributes_no_prompt_context() {
    // A missing app must leave the prompt exactly as it was, not describe a guess.
    assert!(AppCategory::Unknown.describe().is_none());
    assert!(AppCategory::Chat.describe().is_some());
}
