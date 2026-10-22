//! Catalog integrity: the JSON is compiled in, so these guard authoring errors.

use super::*;

#[test]
fn catalog_parses_and_is_non_empty() {
    assert!(!all().is_empty());
}

#[test]
fn ids_are_unique() {
    let mut ids: Vec<&str> = all().iter().map(|m| m.id.as_str()).collect();
    ids.sort_unstable();
    let count = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), count, "duplicate model id in models.json");
}

#[test]
fn filenames_are_unique() {
    // Two entries sharing a filename would collide on disk and in delete//list.
    let mut names: Vec<&str> = all().iter().map(|m| m.filename.as_str()).collect();
    names.sort_unstable();
    let count = names.len();
    names.dedup();
    assert_eq!(names.len(), count, "duplicate filename in models.json");
}

#[test]
fn entries_are_sorted_by_tier() {
    assert!(all().windows(2).all(|w| w[0].tier <= w[1].tier));
}

#[test]
fn every_entry_is_well_formed() {
    for m in all() {
        assert!(!m.id.is_empty(), "empty id");
        assert!(!m.display_name.is_empty(), "{}: empty display_name", m.id);
        assert!(!m.pipelines.is_empty(), "{}: no pipelines", m.id);
        assert!(
            m.pipelines.contains(&m.default_pipeline),
            "{}: default_pipeline not in pipelines",
            m.id
        );
        assert!(m.size_bytes > 0, "{}: size_bytes must be set", m.id);
        assert!(m.url.starts_with("https://"), "{}: url must be https", m.id);
        assert!(
            m.url.ends_with(&m.filename),
            "{}: url must end with filename ({})",
            m.id,
            m.filename
        );
    }
}

#[test]
fn urls_point_at_gguf_files() {
    // transcribe.cpp loads GGUF; the legacy ggml `.bin` files fail at load time.
    for m in all() {
        assert!(
            std::path::Path::new(&m.filename)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("gguf")),
            "{} is not a GGUF file ({})",
            m.id,
            m.filename
        );
    }
}

#[test]
fn find_resolves_known_and_rejects_unknown() {
    let first = &all()[0];
    assert_eq!(
        find(&first.id).map(|m| m.id.as_str()),
        Some(first.id.as_str())
    );
    assert!(find("no-such-model").is_none());
}

#[test]
fn catalog_offers_both_pipelines() {
    // The two-path design needs at least one model on each side.
    let has = |p: Pipeline| all().iter().any(|m| m.pipelines.contains(&p));
    assert!(has(Pipeline::SingleShot));
    assert!(has(Pipeline::Streaming));
}

#[test]
fn dual_pipeline_models_default_to_streaming() {
    // A model that can stream should, since that is the lower-latency path.
    for m in all().iter().filter(|m| m.pipelines.len() > 1) {
        assert_eq!(m.default_pipeline, Pipeline::Streaming, "{}", m.id);
    }
}

#[test]
fn whisper_family_is_flagged_for_run_extension_gating() {
    // Attaching the whisper run extension to another arch is rejected upstream,
    // so `is_whisper` must track the family field exactly.
    for m in all() {
        assert_eq!(m.is_whisper(), m.family == "whisper", "{}", m.id);
    }
}

#[test]
fn part_file_paths_cannot_collide() {
    // The downloader derives its resume file as `filename.with_extension("part")`.
    // Two models whose names differ only by extension would share one `.part`
    // and corrupt each other's resumed transfer.
    let mut parts: Vec<String> = all()
        .iter()
        .map(|m| {
            std::path::Path::new(&m.filename)
                .with_extension("part")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let total = parts.len();
    parts.sort_unstable();
    parts.dedup();
    assert_eq!(parts.len(), total, "two models would share a .part file");
}

#[test]
fn no_filename_contains_a_path_separator() {
    // `models_dir.join(filename)` must stay inside the models directory.
    for m in all() {
        assert!(
            !m.filename.contains('/') && !m.filename.contains('\\'),
            "{}: filename must be a bare file name ({})",
            m.id,
            m.filename
        );
        assert!(!m.filename.contains(".."), "{}: path traversal", m.id);
    }
}

#[test]
fn tiers_are_distinct_enough_to_order_recommendations() {
    // `best_at_or_below` picks the last entry at or under a ceiling, so the
    // catalog needs more than one tier or every machine gets the same model.
    let mut tiers: Vec<u32> = all().iter().map(|m| m.tier).collect();
    tiers.dedup();
    assert!(tiers.len() > 1, "catalog has only one tier");
}

#[test]
fn english_only_models_are_not_flagged_multilingual() {
    // `run_options` pins a non-multilingual model to English and passes the
    // user's language through for the rest. Flagging an English-only build as
    // multilingual sends it a code it rejects — failing every decode, not just
    // degrading quality. The filename is the ground truth here.
    for m in all() {
        let name = m.filename.to_lowercase();
        let english_only = name.contains(".en-") || name.contains("-en-");
        if english_only {
            assert!(
                !m.multilingual,
                "{} is an English-only build ({}) but is flagged multilingual",
                m.id, m.filename
            );
        }
    }
}
