use super::*;

fn model(family: &str, size_bytes: u64) -> CatalogModel {
    CatalogModel {
        id: format!("{family}-{size_bytes}"),
        display_name: format!("{family} {size_bytes}"),
        family: family.to_string(),
        pipelines: vec!["single-shot".to_string()],
        size_bytes,
        multilingual: false,
        description: "Balanced performance".to_string(),
        detail: String::new(),
        downloaded: false,
        is_active: false,
    }
}

fn families(sorted: &[&CatalogModel]) -> Vec<String> {
    sorted.iter().map(|m| m.family.clone()).collect()
}

#[test]
fn families_follow_the_curated_order_not_the_alphabet() {
    let catalog = [
        model("moonshine", 1),
        model("whisper", 1),
        model("nemotron", 1),
        model("canary", 1),
    ];
    assert_eq!(
        families(&sort_for_display(&catalog)),
        ["whisper", "nemotron", "canary", "moonshine"]
    );
}

#[test]
fn smallest_model_comes_first_within_a_family() {
    let catalog = [
        model("whisper", 900),
        model("whisper", 100),
        model("whisper", 500),
    ];
    let sizes: Vec<u64> = sort_for_display(&catalog)
        .iter()
        .map(|m| m.size_bytes)
        .collect();
    assert_eq!(sizes, [100, 500, 900]);
}

#[test]
fn family_grouping_outranks_size() {
    let catalog = [model("moonshine", 1), model("whisper", 9_000_000_000)];
    assert_eq!(
        families(&sort_for_display(&catalog)),
        ["whisper", "moonshine"]
    );
}

#[test]
fn hyphenated_known_family_is_not_treated_as_unlisted() {
    // `qwen3-asr` sits between nemotron and canary in the curated order.
    let catalog = [
        model("canary", 1),
        model("zeta", 1),
        model("qwen3-asr", 1),
        model("nemotron", 1),
    ];
    assert_eq!(
        families(&sort_for_display(&catalog)),
        ["nemotron", "qwen3-asr", "canary", "zeta"]
    );
}

#[test]
fn unlisted_families_trail_and_are_alphabetical_whatever_the_input_order() {
    // They share a rank: without the tiebreak the grid would reshuffle with
    // whatever order the backend happened to send.
    let catalog = [
        model("zeta", 1),
        model("moonshine", 1),
        model("alpha", 1),
        model("mid", 1),
    ];
    assert_eq!(
        families(&sort_for_display(&catalog)),
        ["moonshine", "alpha", "mid", "zeta"]
    );
}

#[test]
fn model_names_resolve_case_insensitively_to_a_later_entry() {
    // The fallback is the first entry, so a match on it proves nothing.
    let mut second = model("parakeet", 2);
    second.display_name = "Parakeet Unified EN 0.6B".to_string();
    let catalog = [model("whisper", 1), second];
    assert_eq!(
        model_name_to_id("PARAKEET unified en 0.6b", &catalog),
        Some("parakeet-2")
    );
}

#[test]
fn an_unknown_model_name_falls_back_to_the_first_entry() {
    let catalog = [model("whisper", 1), model("parakeet", 2)];
    assert_eq!(model_name_to_id("Unknown", &catalog), Some("whisper-1"));
    assert_eq!(model_name_to_id("Unknown", &[]), None);
}

#[test]
fn catalog_sizes_round_to_the_unit_shown() {
    assert_eq!(format_model_size(582_669_056), "583 MB");
    assert_eq!(format_model_size(1_668_741_440), "1.7 GB");
    // Rounds up into the next unit's boundary, as the label reads.
    assert_eq!(format_model_size(999_999_999), "1000 MB");
    assert_eq!(format_model_size(1_000_000_000), "1.0 GB");
    assert_eq!(format_model_size(1_049_999_999), "1.0 GB");
    assert_eq!(format_model_size(1_050_000_000), "1.1 GB");
}

#[test]
fn model_manager_sizes_use_binary_units() {
    assert_eq!(format_manager_size(0), "0 B");
    assert_eq!(format_manager_size(1 << 30), "1.00 GB");
    assert_eq!(format_manager_size(1_717_986_918), "1.60 GB");
    assert_eq!(format_manager_size(582_669_056), "556 MB");
}

#[test]
fn about_sizes_step_from_gb_through_mb_to_kb() {
    assert_eq!(format_about_size(3 * (1 << 30) + (1 << 29)), "3.5 GB");
    assert_eq!(format_about_size(5 << 20), "5 MB");
    assert_eq!(format_about_size(2048), "2 KB");
    // Never "0 KB" for a file that exists.
    assert_eq!(format_about_size(10), "1 KB");
}

#[test]
fn disk_total_is_decimal_gb_with_two_places() {
    assert_eq!(format_disk_total(0), "0.00 GB");
    assert_eq!(format_disk_total(2_345_000_000), "2.35 GB");
}

#[test]
fn weight_tiers_change_at_their_thresholds() {
    assert_eq!(weight_tier(179_999_999), 0);
    assert_eq!(weight_tier(180_000_000), 1);
    assert_eq!(weight_tier(599_999_999), 1);
    assert_eq!(weight_tier(600_000_000), 2);
    assert_eq!(weight_tier(999_999_999), 2);
    assert_eq!(weight_tier(1_000_000_000), 3);
}

#[test]
fn filters_narrow_by_disk_streaming_and_language() {
    let mut live = model("nemotron", 1);
    live.pipelines = vec!["streaming".to_string(), "single-shot".to_string()];
    let mut multi = model("whisper", 1);
    multi.multilingual = true;
    let plain = model("moonshine", 1);

    assert!(ModelFilter::All.admits(&plain, false));
    assert!(ModelFilter::OnDisk.admits(&plain, true));
    assert!(!ModelFilter::OnDisk.admits(&plain, false));
    assert!(ModelFilter::Streaming.admits(&live, false));
    assert!(!ModelFilter::Streaming.admits(&plain, false));
    assert!(ModelFilter::Multilingual.admits(&multi, false));
    assert!(!ModelFilter::Multilingual.admits(&live, false));
}

#[test]
fn search_matches_name_or_description_ignoring_case_and_padding() {
    let mut m = model("whisper", 1);
    m.display_name = "Whisper Medium".to_string();
    m.description = "Great for most machines".to_string();
    assert!(matches_query(&m, ""));
    assert!(matches_query(&m, "   "));
    assert!(matches_query(&m, "  MEDIUM "));
    assert!(matches_query(&m, "most MACHINES"));
    assert!(!matches_query(&m, "parakeet"));
}
