use super::*;

#[test]
fn only_audio_present_at_launch_is_judged_and_torn_writes_go() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("1.wav"), b"old").unwrap();
    std::fs::write(dir.path().join("2.wav.part"), b"torn").unwrap();
    let recordings = Recordings::open(dir.path().to_path_buf());
    // A save in progress: its entry may not be flagged yet.
    std::fs::write(dir.path().join("3.wav"), b"new").unwrap();

    assert_eq!(recordings.at_launch, [1]);
    assert!(!dir.path().join("2.wav.part").exists());
}
