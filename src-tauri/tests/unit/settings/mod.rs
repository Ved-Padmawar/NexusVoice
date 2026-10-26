//! `settings.json`: what survives a restart, and the one-time import of the
//! per-setting files earlier versions wrote.

use super::*;

fn load(dir: &Path) -> Settings {
    Settings::load(dir, Secrets::new("nexusvoice-tests"))
}

#[test]
fn a_fresh_install_starts_from_defaults_and_writes_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let settings = load(dir.path());

    assert_eq!(settings.hotkeys(), HotkeyBindings::default());
    assert_eq!(settings.model(), None);
    assert_eq!(settings.pill().settings, PillSettings::default());
    assert!(dir.path().join(FILE_NAME).exists());
}

#[test]
fn changes_survive_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    {
        let settings = load(dir.path());
        settings.set_model(Some("whisper-small".into())).unwrap();
        settings.set_language(Some("fr".into())).unwrap();
        settings
            .set_model_unload(ModelUnload::After5Minutes)
            .unwrap();
    }
    let settings = load(dir.path());
    assert_eq!(settings.model().as_deref(), Some("whisper-small"));
    assert_eq!(settings.language().as_deref(), Some("fr"));
    assert_eq!(settings.model_unload(), ModelUnload::After5Minutes);
}

#[test]
fn every_pill_change_moves_the_revision_forward() {
    // A window drops an event older than its snapshot, so revisions must grow.
    let dir = tempfile::tempdir().unwrap();
    let settings = load(dir.path());
    let before = settings.pill().revision;

    let first = settings
        .set_pill(PillSettings {
            theme: PillTheme::Aurora,
            ..PillSettings::default()
        })
        .unwrap();
    let second = settings
        .set_pill(PillSettings {
            live_transcript: true,
            ..first.settings
        })
        .unwrap();

    assert!(first.revision > before);
    assert!(second.revision > first.revision);
    assert_eq!(settings.pill().revision, second.revision);
    assert_eq!(settings.pill().settings.theme, PillTheme::Aurora);
}

#[test]
fn the_old_per_setting_files_are_imported_once_then_removed() {
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, text: &str| std::fs::write(dir.path().join(name), text).unwrap();
    write(
        "hotkeys.json",
        r#"{"pushToTalk":"Ctrl+Space","dictationCommit":"Ctrl+Enter"}"#,
    );
    write("model_override", "parakeet\n");
    write("model_unload", r#""after10Minutes""#);
    write("language", "de");
    write("input_device", "USB Mic");
    write(
        "format_config.json",
        r#"{"enabled":true,"provider":"ollama","profiles":{"ollama":{"baseUrl":"http://localhost:11434/v1","model":"qwen","apiKey":""}}}"#,
    );

    let settings = load(dir.path());

    assert_eq!(
        settings.hotkeys().push_to_talk.as_deref(),
        Some("Ctrl+Space")
    );
    assert_eq!(settings.hotkeys().dictation, None);
    assert_eq!(
        settings.hotkeys().dictation_commit.as_deref(),
        Some("Ctrl+Enter")
    );
    assert_eq!(settings.model().as_deref(), Some("parakeet"));
    assert_eq!(settings.model_unload(), ModelUnload::After10Minutes);
    assert_eq!(settings.language().as_deref(), Some("de"));
    assert_eq!(settings.input_device().as_deref(), Some("USB Mic"));
    let formatter = lock_recovering(&settings.file).formatter.clone();
    assert!(formatter.enabled);
    assert_eq!(formatter.endpoints["ollama"].model, "qwen");

    for name in [
        "hotkeys.json",
        "model_override",
        "model_unload",
        "language",
        "input_device",
        "format_config.json",
    ] {
        assert!(!dir.path().join(name).exists(), "{name} should be gone");
    }
}

#[test]
fn an_unreadable_file_falls_back_to_defaults() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(FILE_NAME), "{ not json").unwrap();
    // Still present: it must not be mistaken for a first run and re-imported.
    std::fs::write(dir.path().join("model_override"), "parakeet").unwrap();

    let settings = load(dir.path());
    assert_eq!(settings.model(), None);
}

#[test]
fn a_write_leaves_no_temp_file_behind() {
    let dir = tempfile::tempdir().unwrap();
    let settings = load(dir.path());
    settings.set_input_device(Some("Mic".into())).unwrap();

    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, [FILE_NAME]);
}
