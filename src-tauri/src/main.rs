#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

// 10 MB in bytes
const LOG_MAX_SIZE: u128 = 10 * 1024 * 1024;

/// A short id identifying this app run, attached to every structured log line so
/// lines from one session can be grouped when logs from multiple runs interleave.
/// Models live in the release data dir even in dev, so a dev build reuses
/// downloads. Everything else stays per-identifier. Dev's dir is the release
/// one plus a `.dev` suffix.
fn shared_models_dir(app_data_dir: &std::path::Path) -> std::path::PathBuf {
    let shared_root = app_data_dir
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".dev"))
        .map_or_else(
            || app_data_dir.to_path_buf(),
            |release| app_data_dir.with_file_name(release),
        );
    shared_root.join("models")
}

fn session_id() -> &'static str {
    use std::sync::OnceLock;
    static SESSION_ID: OnceLock<String> = OnceLock::new();
    SESSION_ID.get_or_init(|| {
        // Derive 8 hex chars from the startup timestamp — no extra deps needed.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        #[allow(clippy::cast_possible_truncation)] // low 32 bits are all we want for an id
        let low = (nanos & 0xffff_ffff) as u64;
        format!("{low:08x}")
    })
}

mod audio;
mod commands;
mod database;
mod dictionary;
mod events;
mod focus;
mod hardware;
mod hotkeys;
mod inference;
mod injection;
mod llm;
mod pill_geometry;
mod postprocess;
mod preprocess;
mod remote;
mod settings;
mod sync;
mod transcribe;
mod transcription;

use database::Database;
use dictionary::Dictionary;
use hotkeys::Hotkeys;
use inference::cache::EngineCache;
use inference::cuda::CudaPack;
use inference::library::ModelLibrary;
use settings::{Secrets, Settings};
use transcription::Recorder;

/// The command and event registry, and the single source of the generated TS
/// bindings. Adding one here is what puts it in `src/bindings.ts`.
fn command_bindings() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .error_handling(tauri_specta::ErrorHandlingMode::Throw)
        .commands(tauri_specta::collect_commands![
            commands::start_transcription,
            commands::stop_transcription,
            commands::list_input_devices,
            commands::set_input_device,
            commands::start_dictation,
            commands::pause_dictation,
            commands::resume_dictation,
            commands::commit_dictation,
            commands::get_usage_stats,
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
            commands::log_frontend,
            commands::wait_for_app_ready,
        ])
        .events(tauri_specta::collect_events![
            events::HotkeyPressed,
            events::HotkeyReleased,
            events::DictationHotkeyPressed,
            events::DictationCommitHotkeyPressed,
            events::RecordingCancelled,
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
            events::CudaDownloadComplete,
            events::CudaDownloadCancelled,
            events::CudaDownloadError,
        ])
}

/// Write `src/bindings.ts`; debug launches and the test suite both regenerate it.
#[cfg(any(debug_assertions, test))]
fn export_bindings(builder: &tauri_specta::Builder<tauri::Wry>) {
    builder
        .export(
            specta_typescript::Typescript::default()
                .bigint(specta_typescript::BigIntExportBehavior::Number),
            "../src/bindings.ts",
        )
        .expect("failed to export typescript bindings");
}

/// Register compute backends before any model loads, the CUDA pack first so ggml prefers it.
fn init_compute_backends(app_data_dir: &std::path::Path) {
    inference::cuda::activate(app_data_dir);
    match transcribe_cpp::init_backends_default() {
        Ok(()) => {
            let devices = transcribe_cpp::devices();
            log::info!(
                "transcribe.cpp: {} compute device(s) [{}]",
                devices.len(),
                devices
                    .iter()
                    .map(|d| format!("{} ({})", d.name, d.kind))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        Err(e) => log::warn!("transcribe.cpp backend init failed: {e}"),
    }
}

#[allow(clippy::too_many_lines)] // Tauri setup is inherently long — splitting adds no clarity
fn main() {
    // Route transcribe.cpp/GGML's verbose stderr output through `log` so our
    // level filter controls it instead of it flooding stdout. Safe to call once.
    transcribe_cpp::init_logging();

    // Panic hook — writes panic info to log before crashing. Debug only; release
    // unwinds so `catch_unwind` on the recording path can recover instead.
    #[cfg(debug_assertions)]
    {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            log::error!("PANIC: {info}");
            default_hook(info);
        }));
    }

    // Build log targets conditionally
    #[cfg(debug_assertions)]
    let log_targets = vec![
        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),
        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
            file_name: Some("nexusvoice".into()),
        }),
    ];
    #[cfg(not(debug_assertions))]
    let log_targets = vec![tauri_plugin_log::Target::new(
        tauri_plugin_log::TargetKind::LogDir {
            file_name: Some("nexusvoice".into()),
        },
    )];

    #[cfg(debug_assertions)]
    let log_level = log::LevelFilter::Info;
    #[cfg(not(debug_assertions))]
    let log_level = log::LevelFilter::Info;

    let specta_builder = command_bindings();

    // Debug only: regenerate the TS bindings. Release builds ship the committed file.
    #[cfg(debug_assertions)]
    export_bindings(&specta_builder);

    tauri::Builder::default()
        .on_window_event(|window, event| {
            // Windows-only: it demotes always-on-top windows when another app gains
            // focus, so when a non-pill window is focused (e.g. Alt+Tab to main) we
            // re-promote the pill's Z-order. macOS/Linux keep always-on-top sticky,
            // so the toggle is unnecessary churn there.
            #[cfg(windows)]
            if let tauri::WindowEvent::Focused(true) = event {
                if window.label() != "pill" {
                    if let Some(pill) = window.app_handle().get_webview_window("pill") {
                        let _ = pill.set_always_on_top(false);
                        let _ = pill.set_always_on_top(true);
                        let _ = pill.show();
                    }
                }
            }
            // Silence unused-variable warnings on non-Windows where the body is gone.
            #[cfg(not(windows))]
            {
                let _ = (window, event);
            }
        })
        // A second launch is either a dictation flag (the Wayland path, where
        // the desktop owns the shortcut) or the user reopening the window.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if remote::handle_args(app, &args) {
                return;
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets(log_targets)
                .level(log_level)
                // transcribe.cpp/GGML per-token tracing is extremely verbose; mute it.
                .level_for("transcribe_cpp", log::LevelFilter::Warn)
                .max_file_size(LOG_MAX_SIZE)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                // Structured (JSON-line) output: one object per record with a stable
                // schema (ts/level/target/session/msg) so logs are queryable.
                .format(|out, message, record| {
                    let line = serde_json::json!({
                        "ts": chrono::Utc::now().to_rfc3339(),
                        "level": record.level().as_str(),
                        "target": record.target(),
                        "session": session_id(),
                        "msg": message.to_string(),
                    });
                    out.finish(format_args!("{line}"));
                })
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            specta_builder.mount_events(app);

            let app_data_dir = app.path().app_data_dir().map_err(std::io::Error::other)?;
            std::fs::create_dir_all(&app_data_dir)?;
            init_compute_backends(&app_data_dir);

            let models_dir = shared_models_dir(&app_data_dir);
            std::fs::create_dir_all(&models_dir)?;
            // Nothing can be mid-download at startup, so any .part file is
            // debris from a crash or a quit during a download.
            inference::downloader::clean_stale_parts(&models_dir);

            // No blocking I/O beyond the settings file: the database opens in
            // the background after setup returns.
            let secrets = Secrets::new(app.config().identifier.clone());
            app.manage(Settings::load(&app_data_dir, secrets));
            app.manage(Database::default());
            app.manage(Dictionary::default());
            app.manage(EngineCache::new(models_dir.clone()));
            app.manage(ModelLibrary::new(models_dir));
            app.manage(CudaPack::new(app_data_dir.clone()));
            app.manage(Recorder::new());
            app.manage(Hotkeys::default());

            {
                let app_handle = app.handle().clone();
                let db_path = app_data_dir.join("nexusvoice.db");
                tauri::async_runtime::spawn(async move {
                    // May run migrations; this is the slow part.
                    let result = database::connection::open_database(&db_path).await;
                    // Publish either outcome, so waiting commands are released.
                    app_handle.state::<Database>().set(result.clone());
                    let pool = match result {
                        Ok(pool) => pool,
                        Err(e) => {
                            log::error!("database init failed: {e}");
                            return;
                        }
                    };
                    log::info!("database ready");
                    if let Err(e) = app_handle.state::<Dictionary>().load(&pool).await {
                        log::error!("could not load the dictionary: {e}");
                    }
                });
            }

            // Spawn: hardware detection (blocking syscalls — must not run on main thread)
            {
                tauri::async_runtime::spawn(async move {
                    use inference::provider::recommend_model;

                    // The cached profile, so recommend_model() reuses this probe.
                    let (hw, recommended) = tokio::task::spawn_blocking(|| {
                        let hw = hardware::cached_profile().clone();
                        let recommended = recommend_model();
                        (hw, recommended)
                    })
                    .await
                    .unwrap_or_else(|_| {
                        // Detection failed; fall back to the catalog's entry level.
                        (
                            hardware::profile::HardwareProfile::default(),
                            &inference::catalog::all()[0],
                        )
                    });

                    log::info!("NexusVoice v{} starting", env!("CARGO_PKG_VERSION"));
                    log::info!("OS: {}", std::env::consts::OS);
                    log::info!("RAM: {:.1} GB", hw.ram_gb);
                    log::info!(
                        "GPU: {} ({}, {:.1} GB VRAM)",
                        hw.gpu_type,
                        hw.execution_provider,
                        hw.vram_gb
                    );
                    log::info!("Recommended model: {}", recommended.display_name);
                });
            }

            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    app_handle
                        .state::<Hotkeys>()
                        .restore(&app_handle, &app_handle.state::<Settings>())
                        .await;
                });
            }

            // Global shortcuts cannot be registered under Wayland, so dictation
            // is also reachable by signal for users who bind it themselves.
            #[cfg(target_os = "linux")]
            remote::listen_for_signals(app.handle().clone());

            // Preload so the first transcription is instant. Without a model on
            // disk this is a no-op, and recording start tries again.
            EngineCache::warm(app.handle());
            inference::idle::spawn_watcher(app.handle().clone());

            // System tray
            let show_item = MenuItem::with_id(app, "show", "Show Dashboard", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            // Use the app icon for the tray, but load a high-resolution (256px)
            // render of it explicitly. `default_window_icon()` resolves to a small
            // layer that Windows then upscales, which looks low-res in the tray;
            // handing it a crisp 256px source lets Windows downscale cleanly.
            // Embedded so the path resolves identically in dev and bundled builds.
            let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;

            let _tray = TrayIconBuilder::new()
                .icon(tray_icon)
                .tooltip("NexusVoice")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.state::<Recorder>().shutdown();
                        let app_handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                            app_handle.exit(0);
                        });
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Create pill window deferred — avoids two simultaneous WebView2 initializations
            // on first launch which causes "not responding" hang on Windows.
            // main window's WebView2 is already alive at this point in setup().
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // Find pill window config and build it manually
                    let config = app_handle.config();
                    if let Some(win_config) = config.app.windows.iter().find(|w| w.label == "pill")
                    {
                        match tauri::WebviewWindowBuilder::from_config(&app_handle, win_config)
                            .and_then(tauri::WebviewWindowBuilder::build)
                        {
                            Ok(pill) => {
                                let (pill_w, pill_h) = crate::pill_geometry::capsule_window();
                                let _ = pill.set_size(tauri::LogicalSize::new(pill_w, pill_h));
                                // Position: centered horizontally, near bottom of primary monitor
                                if let Some(monitor) = pill.primary_monitor().ok().flatten() {
                                    let screen = monitor.size();
                                    let scale = monitor.scale_factor();
                                    let margin = 72.0;
                                    let logical_w = f64::from(screen.width) / scale;
                                    let logical_h = f64::from(screen.height) / scale;
                                    #[allow(clippy::cast_possible_truncation)]
                                    // pixel coords fit i32
                                    let x = ((logical_w - pill_w) / 2.0) as i32;
                                    #[allow(clippy::cast_possible_truncation)]
                                    let y = (logical_h - pill_h - margin) as i32;
                                    let _ = pill.set_position(tauri::LogicalPosition::new(x, y));
                                }
                                let _ = pill.set_skip_taskbar(true);
                                let _ = pill.show();
                            }
                            Err(e) => log::error!("failed to create pill window: {e}"),
                        }
                    }
                });
            }

            // Re-assert pill Z-order periodically. Windows demotes always-on-top windows
            // when fullscreen apps appear, after Alt+Tab cycling between other apps,
            // and on virtual-desktop switches. Tauri must toggle the flag to
            // re-promote a window that is already marked always-on-top.
            // macOS/Linux keep always-on-top sticky, so this loop is Windows-only.
            #[cfg(windows)]
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let mut iv = tokio::time::interval(std::time::Duration::from_secs(2));
                    iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    loop {
                        iv.tick().await;
                        if let Some(pill) = app_handle.get_webview_window("pill") {
                            if !pill.is_visible().unwrap_or(true) {
                                let _ = pill.show();
                            }
                            let _ = pill.set_always_on_top(false);
                            let _ = pill.set_always_on_top(true);
                        }
                    }
                });
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                app.state::<Recorder>().shutdown();
            }
        });
}

#[cfg(test)]
#[path = "../tests/unit/bindings.rs"]
mod bindings_tests;
