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

// ── Catalog integrity ──────────────────────────────────────────────────
// Authoring guards, like the models.json ones: `lookup` lowercases the incoming
// stem, so a table entry that is not already lowercase can never match.

#[test]
fn every_catalogued_stem_is_lowercase() {
    for (stem, name, _) in super::catalog::CATALOG {
        assert_eq!(
            *stem,
            stem.to_lowercase(),
            "{name}: stem \"{stem}\" must be lowercase or it will never match"
        );
    }
}

#[test]
fn catalogued_stems_are_unique() {
    // A duplicate stem means the later entry is dead — its name and category
    // silently never apply.
    let mut stems: Vec<&str> = super::catalog::CATALOG.iter().map(|(s, _, _)| *s).collect();
    let total = stems.len();
    stems.sort_unstable();
    stems.dedup();
    assert_eq!(
        stems.len(),
        total,
        "duplicate executable stem in the catalog"
    );
}

#[test]
fn every_catalogued_app_has_a_display_name_and_a_real_category() {
    for (stem, name, category) in super::catalog::CATALOG {
        assert!(!name.is_empty(), "{stem}: empty display name");
        assert_ne!(
            *category,
            AppCategory::Unknown,
            "{stem}: a catalogued app must have a real category"
        );
    }
}

#[test]
fn every_real_category_describes_itself_for_the_prompt() {
    // `describe` returning None is how "no app context" is signalled, so only
    // Unknown may do it — a missing arm would silently drop the destination.
    for category in [
        AppCategory::Chat,
        AppCategory::Email,
        AppCategory::Code,
        AppCategory::Notes,
        AppCategory::Browser,
        AppCategory::Terminal,
    ] {
        let described = category.describe();
        assert!(described.is_some(), "{category:?} has no description");
        assert!(
            !described.expect("checked").is_empty(),
            "{category:?} describes itself as an empty string"
        );
    }
    assert!(AppCategory::Unknown.describe().is_none());
}

#[test]
fn a_catalogued_stem_resolves_through_from_executable() {
    // Ties the table to the lookup path: every entry must be reachable.
    for (stem, name, category) in super::catalog::CATALOG {
        let target = FocusTarget::from_executable(stem);
        assert_eq!(target.name, *name, "stem {stem} resolved to the wrong name");
        assert_eq!(target.category, *category, "stem {stem} wrong category");
    }
}
