use crate::bridge::{commands, events};

/// The command and event registry, and the single source of the generated TS
/// bindings. Adding one here is what puts it in `src/shared/bindings.ts`.
pub fn command_bindings() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .error_handling(tauri_specta::ErrorHandlingMode::Throw)
        .commands(tauri_specta::collect_commands![
            commands::subscribe,
            commands::list_input_devices,
            commands::set_input_device,
            commands::set_keep_mic_open,
            commands::pause_dictation,
            commands::resume_dictation,
            commands::commit_dictation,
            commands::get_usage_stats,
            commands::get_daily_words,
            commands::get_transcripts,
            commands::search_transcripts,
            commands::delete_transcript,
            commands::update_dictionary,
            commands::delete_dictionary_entry,
            commands::save_export,
            commands::reveal_in_folder,
            commands::get_injection_status,
            commands::set_paste_mode,
            commands::set_hotkey,
            commands::suspend_hotkeys,
            commands::resume_hotkeys,
            commands::set_pill_setting,
            commands::start_model_download,
            commands::cancel_model_download,
            commands::set_model_override,
            commands::get_language_options_for,
            commands::set_language,
            commands::set_model_unload,
            commands::get_hardware_profile,
            commands::get_model_catalog,
            commands::rescan_models,
            commands::delete_model,
            commands::start_cuda_download,
            commands::cancel_cuda_download,
            commands::remove_cuda_pack,
            commands::set_format_enabled,
            commands::save_format_endpoint,
            commands::test_format_connection,
            commands::open_logs_folder,
            commands::resize_pill,
            commands::set_pill_hitbox,
            commands::log_frontend,
            commands::wait_for_app_ready,
            commands::prepare_restart,
            commands::cancel_restart,
            commands::relaunch_app,
            commands::set_saved_audio,
            commands::clear_saved_audio,
            commands::set_transcript_pinned,
            commands::restore_transcript,
            commands::inspect_audio_file,
            commands::cancel_job,
            commands::retranscribe_transcript,
            commands::import_audio_file,
        ])
        .events(tauri_specta::collect_events![
            events::TranscriptionPartial,
            events::TranscriptionPartialEnd,
            events::TranscriptionComplete,
            events::TranscriptionError,
            events::TranscriptionWarning,
            events::PillWaveform,
            events::ModelDownloadCancelled,
            events::LanguageReset,
            events::CudaDownloadComplete,
            events::CudaDownloadError,
            events::JobFinished,
        ])
}

/// Write `src/shared/bindings.ts`; debug launches and the test suite both regenerate it.
#[cfg(any(debug_assertions, test))]
pub fn export_bindings(builder: &tauri_specta::Builder<tauri::Wry>) {
    builder
        .export(
            specta_typescript::Typescript::default()
                .bigint(specta_typescript::BigIntExportBehavior::Number),
            "../src/shared/bindings.ts",
        )
        .expect("failed to export typescript bindings");
}

#[cfg(test)]
#[path = "../../tests/unit/app/bindings.rs"]
mod tests;
