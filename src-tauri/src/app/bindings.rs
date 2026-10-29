use crate::bridge::{commands, events};

/// The command and event registry, and the single source of the generated TS
/// bindings. Adding one here is what puts it in `src/shared/bindings.ts`.
pub fn command_bindings() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .error_handling(tauri_specta::ErrorHandlingMode::Throw)
        .commands(tauri_specta::collect_commands![
            commands::get_recording_state,
            commands::list_input_devices,
            commands::set_input_device,
            commands::get_keep_mic_open,
            commands::set_keep_mic_open,
            commands::pause_dictation,
            commands::resume_dictation,
            commands::commit_dictation,
            commands::get_usage_stats,
            commands::get_daily_words,
            commands::get_transcripts,
            commands::search_transcripts,
            commands::get_dictionary,
            commands::delete_transcript,
            commands::update_dictionary,
            commands::delete_dictionary_entry,
            commands::get_injection_status,
            commands::get_hotkeys,
            commands::set_hotkey,
            commands::get_pill_settings,
            commands::set_pill_settings,
            commands::get_model_info,
            commands::start_model_download,
            commands::get_active_downloads,
            commands::cancel_model_download,
            commands::set_model_override,
            commands::get_language_options,
            commands::set_language,
            commands::get_model_unload,
            commands::set_model_unload,
            commands::get_hardware_profile,
            commands::get_model_catalog,
            commands::get_downloaded_models,
            commands::delete_model,
            commands::get_cuda_status,
            commands::start_cuda_download,
            commands::cancel_cuda_download,
            commands::remove_cuda_pack,
            commands::get_format_config,
            commands::set_format_config,
            commands::test_format_connection,
            commands::open_logs_folder,
            commands::resize_pill,
            commands::set_pill_hitbox,
            commands::log_frontend,
            commands::wait_for_app_ready,
        ])
        .events(tauri_specta::collect_events![
            events::RecordingStateChanged,
            events::TranscriptionPartial,
            events::TranscriptionPartialEnd,
            events::TranscriptionComplete,
            events::TranscriptionError,
            events::TranscriptionWarning,
            events::TranscriptSaved,
            events::PillWaveform,
            events::PillSettingsChanged,
            events::ModelDownloadStart,
            events::ModelDownloadRunning,
            events::ModelDownloadProgress,
            events::ModelDownloadComplete,
            events::ModelDownloadError,
            events::ModelDownloadCancelled,
            events::ModelEvicted,
            events::ModelSwitched,
            events::LanguageReset,
            events::CudaDownloadProgress,
            events::CudaDownloadVerifying,
            events::CudaDownloadComplete,
            events::CudaDownloadCancelled,
            events::CudaDownloadError,
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
