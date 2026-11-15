//! `settings.json`: what survives a restart, and the one-time import of the
//! per-setting files earlier versions wrote.

use super::*;

fn load(dir: &Path) -> Settings {
    Settings::load(dir, Secrets::mock())
}

#[test]
fn a_fresh_install_starts_from_defaults_and_writes_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let settings = load(dir.path());

    assert_eq!(settings.hotkeys(), HotkeyBindings::default());
    assert_eq!(settings.model(), None);
    assert_eq!(settings.pill(), PillSettings::default());
    assert!(dir.path().join(FILE_NAME).exists());
}

#[test]
fn a_file_from_before_saved_audio_and_paste_modes_keeps_its_choices() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(FILE_NAME),
        r#"{ "version": 1, "model": "whisper-small", "keepMicOpen": true }"#,
    )
    .unwrap();
    let settings = load(dir.path());

    assert_eq!(settings.model().as_deref(), Some("whisper-small"));
    assert!(settings.keep_mic_open());
    assert_eq!(settings.saved_audio(), SavedAudio::default());
    assert_eq!(settings.paste_mode(), PasteMode::Auto);
}

#[test]
fn changes_survive_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    {
        let settings = load(dir.path());
        settings
            .select_model(catalog::find("whisper-small"))
            .unwrap();
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
fn the_microphone_is_released_when_idle_until_asked_otherwise() {
    let dir = tempfile::tempdir().unwrap();
    {
        let settings = load(dir.path());
        assert!(!settings.keep_mic_open());
        settings.set_keep_mic_open(true).unwrap();
    }
    assert!(load(dir.path()).keep_mic_open());
}

#[test]
fn turning_the_microphone_hold_back_off_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    {
        let settings = load(dir.path());
        settings.set_keep_mic_open(true).unwrap();
        settings.set_keep_mic_open(false).unwrap();
    }
    assert!(!load(dir.path()).keep_mic_open());
}

#[test]
fn a_pill_change_keeps_the_other_pill_choices() {
    // Two quick edits of different preferences must not overwrite each other.
    let dir = tempfile::tempdir().unwrap();
    let settings = load(dir.path());

    settings
        .set_pill(PillSetting::Theme(PillTheme::Aurora))
        .unwrap();
    settings
        .set_pill(PillSetting::LiveTranscript(true))
        .unwrap();

    assert_eq!(
        load(dir.path()).pill(),
        PillSettings {
            theme: PillTheme::Aurora,
            live_transcript: true,
            ..PillSettings::default()
        }
    );
}

#[test]
fn turning_formatting_off_keeps_the_endpoint_saved_just_before() {
    // The toggle can land before the screen has read the new endpoint back.
    let dir = tempfile::tempdir().unwrap();
    let settings = load(dir.path());
    let draft = |provider: &str, model: &str| formatter::FormatterDraft {
        provider: provider.into(),
        profile: formatter::ProfileUpdate {
            base_url: "http://localhost:11434/v1".into(),
            model: model.into(),
            api_key: None,
        },
    };

    settings
        .save_formatter_endpoint(&draft("lmstudio", "qwen-old"))
        .unwrap();
    settings
        .save_formatter_endpoint(&draft("ollama", "qwen-new"))
        .unwrap();
    settings.set_formatter_enabled(false).unwrap();

    let saved = load(dir.path()).read(|f| f.formatter.clone());
    assert!(!saved.enabled);
    assert_eq!(saved.provider, "ollama");
    assert_eq!(saved.endpoints["ollama"].model, "qwen-new");
    assert_eq!(saved.endpoints["lmstudio"].model, "qwen-old");
}

#[test]
fn a_saved_audio_change_keeps_the_other_choice() {
    let dir = tempfile::tempdir().unwrap();
    let settings = load(dir.path());

    settings
        .set_saved_audio(SavedAudioSetting::Keep(25))
        .unwrap();
    settings
        .set_saved_audio(SavedAudioSetting::Enabled(false))
        .unwrap();

    assert_eq!(
        load(dir.path()).saved_audio(),
        SavedAudio {
            enabled: false,
            keep: 25
        }
    );
}

#[test]
fn a_change_takes_a_revision_only_once_it_is_saved() {
    let dir = tempfile::tempdir().unwrap();
    let settings = load(dir.path());

    let saved = settings.set_language(Some("fr".into())).unwrap();

    // The folder is gone, so the next write fails and memory keeps "fr".
    dir.close().unwrap();
    assert!(settings.set_language(Some("de".into())).is_err());
    assert_eq!(settings.saved.committed(), saved);
    assert_eq!(settings.language().as_deref(), Some("fr"));
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
    let formatter = settings.read(|f| f.formatter.clone());
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

#[test]
fn a_failed_formatter_save_preserves_a_replaced_or_removed_key() {
    let dir = tempfile::tempdir().unwrap();
    let settings = load(dir.path());
    let mut draft = FormatterDraft {
        provider: "openai".into(),
        profile: formatter::ProfileUpdate {
            base_url: "https://example.com/v1".into(),
            model: "original-model".into(),
            api_key: Some("original-key".into()),
        },
    };
    let saved = settings.save_formatter_endpoint(&draft).unwrap();
    dir.close().unwrap();
    draft.profile.model = "different-model".into();

    for key in ["new-key", ""] {
        draft.profile.api_key = Some(key.into());
        assert!(settings.save_formatter_endpoint(&draft).is_err());
        assert_eq!(settings.saved.committed(), saved);
        let config = settings.format_config();
        assert_eq!(config.active().api_key, "original-key");
        assert_eq!(config.active().model, "original-model");
    }
}

#[test]
fn only_an_explicit_model_change_resets_an_incompatible_language() {
    let dir = tempfile::tempdir().unwrap();
    let settings = load(dir.path());
    settings
        .select_model(catalog::find("whisper-small"))
        .unwrap();
    settings
        .set_supported_language(Some("ja".into()))
        .unwrap()
        .unwrap();
    let english_only = catalog::find("parakeet-tdt-0.6b-v2").unwrap();
    let (_, reset) = settings.select_model(Some(english_only)).unwrap();
    assert!(reset);
    assert_eq!(settings.language().as_deref(), Some(language::DEFAULT));
    assert!(settings
        .set_supported_language(Some("ja".into()))
        .unwrap()
        .is_none());
    assert_eq!(
        load(dir.path()).language().as_deref(),
        Some(language::DEFAULT)
    );
}
