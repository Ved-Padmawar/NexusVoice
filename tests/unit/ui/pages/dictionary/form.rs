use super::*;

#[test]
fn fields_are_trimmed_before_saving() {
    assert_eq!(
        entry_from_fields("  teh ", "\tthe\n"),
        Some(("teh".to_string(), "the".to_string()))
    );
}

#[test]
fn a_blank_field_blocks_the_save() {
    assert_eq!(entry_from_fields("", "the"), None);
    assert_eq!(entry_from_fields("teh", ""), None);
    assert_eq!(entry_from_fields("   ", "the"), None);
    assert_eq!(entry_from_fields("teh", " \t "), None);
}

#[test]
fn inner_spacing_is_kept() {
    assert_eq!(
        entry_from_fields("gonna", "going  to"),
        Some(("gonna".to_string(), "going  to".to_string()))
    );
}
