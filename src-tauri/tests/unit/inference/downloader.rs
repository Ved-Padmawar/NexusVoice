use super::*;

#[test]
fn pct_of_reports_zero_when_total_is_unknown() {
    // Server sent no Content-Length; reporting 0 beats dividing by zero.
    assert_eq!(pct_of(1234, 0), 0);
}

#[test]
fn pct_of_scales_and_saturates() {
    assert_eq!(pct_of(0, 100), 0);
    assert_eq!(pct_of(50, 100), 50);
    assert_eq!(pct_of(100, 100), 100);
    // A resumed transfer can overshoot a stale total; never exceed 100.
    assert_eq!(pct_of(150, 100), 100);
}

#[test]
fn clean_stale_parts_removes_only_part_files() {
    let dir = std::env::temp_dir().join("nv_clean_parts");
    std::fs::create_dir_all(&dir).unwrap();
    let part = dir.join("ggml-tiny.en-q5_1.part");
    let model = dir.join("ggml-tiny.en-q5_1.bin");
    std::fs::write(&part, b"partial").unwrap();
    std::fs::write(&model, b"complete").unwrap();

    clean_stale_parts(&dir);

    assert!(!part.exists(), "stale .part should be swept");
    assert!(model.exists(), "finished model must survive");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn clean_stale_parts_tolerates_a_missing_directory() {
    // Startup runs this before the models dir is guaranteed to exist.
    clean_stale_parts(&std::env::temp_dir().join("nv_no_such_dir"));
}
