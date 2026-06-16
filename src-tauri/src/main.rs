#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

// 10 MB in bytes
const LOG_MAX_SIZE: u128 = 10 * 1024 * 1024;

/// A short id identifying this app run, attached to every structured log line so
/// lines from one session can be grouped when logs from multiple runs interleave.
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
mod auth;
mod commands;
mod database;
mod hardware;
mod inference;
mod llm;
mod parakeet;
mod pipeline;
mod postprocess;
mod preprocess;
mod state;
mod transcription;

#[allow(clippy::too_many_lines)] // Tauri setup is inherently long — splitting adds no clarity
fn main() {
    // Route whisper.cpp/GGML's verbose stderr output through `log` so our level
    // filter controls it instead of it flooding stdout. Safe to call once.
    whisper_rs::install_logging_hooks();

    // Panic hook — writes panic info to log before crashing.
    // Note: only active in debug builds since release profile uses panic = "abort".
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
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets(log_targets)
                .level(log_level)
                // whisper.cpp/GGML per-token tracing is extremely verbose; mute it.
                .level_for("whisper_rs", log::LevelFilter::Warn)
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
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().map_err(std::io::Error::other)?;
            std::fs::create_dir_all(&app_data_dir)?;

            let hotkey_store_path = app_data_dir.join("hotkey");
            let dictation_hotkey_store_path = app_data_dir.join("dictation_hotkey");
            let dictation_commit_hotkey_store_path = app_data_dir.join("dictation_commit_hotkey");
            let model_override_path = app_data_dir.join("model_override");
            let active_engine_path = app_data_dir.join("active_engine");
            let beam_size_path = app_data_dir.join("beam_size");
            let format_config_path = app_data_dir.join("format_config.json");
            let models_dir = app_data_dir.join("models");
            std::fs::create_dir_all(&models_dir)?;

            // Create state immediately with NO blocking I/O.
            // DB + auth are initialized asynchronously after setup returns.
            let app_state = state::AppState::new(
                app_data_dir,
                hotkey_store_path,
                dictation_hotkey_store_path,
                dictation_commit_hotkey_store_path,
                model_override_path,
                active_engine_path,
                beam_size_path,
                format_config_path,
                models_dir,
            );
            app.manage(app_state);

            // Spawn: DB init + auth + dict cache + re-auth — fully async, never blocks main thread
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<state::AppState>();
                    let db_path = state.app_data_dir.join("nexusvoice.db");

                    // Open database (may run migrations — this is the slow part)
                    let pool = match database::connection::open_database(&db_path).await {
                        Ok(p) => p,
                        Err(e) => {
                            log::error!("database init failed: {e}");
                            let _ = app_handle.emit("auth:unauthenticated", ());
                            return;
                        }
                    };

                    // Wire up pool + auth service
                    let auth_service = auth::AuthService::new(pool.clone());
                    state.set_pool(pool.clone());
                    state.set_auth(auth_service);

                    log::info!("database ready");

                    // Preload dictionary cache
                    {
                        use database::repositories::dictionary::DictionaryRepository;
                        let entries = DictionaryRepository::new(pool.clone())
                            .list_all()
                            .await
                            .unwrap_or_default();
                        *state.dict_cache.write().await =
                            entries.into_iter().map(|e| (e.term.clone(), e)).collect();
                    }

                    // Restore the persisted session: if a user is recorded in
                    // app_session, they stay signed in across restarts.
                    match state.auth().await.current_user().await {
                        Ok(Some(user)) => {
                            state.set_auth_session(user.id).await;
                            let _ = app_handle.emit("auth:ready", user.id);
                        }
                        _ => {
                            let _ = app_handle.emit("auth:unauthenticated", ());
                        }
                    }
                });
            }

            // Spawn: hardware detection (blocking syscalls — must not run on main thread)
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    use hardware::detector::detect_profile;
                    use hardware::sysinfo_provider::SysinfoProvider;
                    use inference::provider::recommend_model_size;

                    let (hw, recommended) = tokio::task::spawn_blocking(|| {
                        let hw = detect_profile(&SysinfoProvider);
                        let recommended = recommend_model_size();
                        (hw, recommended)
                    })
                    .await
                    .unwrap_or_else(|_| {
                        (
                            hardware::profile::HardwareProfile::default(),
                            inference::provider::ModelSize::Small,
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
                    log::info!("Recommended model: {}", recommended.display_name());

                    let _ = app_handle.emit(
                        "hardware:profile",
                        serde_json::json!({
                            "gpuName": hw.gpu_type,
                            "executionProvider": hw.execution_provider,
                            "vramGb": hw.vram_gb,
                            "ramGb": hw.ram_gb,
                            "recommendedModel": recommended.display_name(),
                        }),
                    );
                });
            }

            // Spawn: hotkey restore
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    commands::restore_registered_hotkeys(&app_handle).await;
                });
            }

            // Spawn: eagerly pre-load the Whisper engine so the first transcription is instant.
            // Runs after DB init (model selection may depend on override stored on disk).
            // If the model isn't downloaded yet this is a no-op — get_or_load_engine returns Err.
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<state::AppState>();
                    match state.get_or_load_engine().await {
                        Ok(_) => log::info!("whisper engine pre-loaded and warmed up"),
                        Err(e) => log::info!("whisper engine pre-load skipped: {e}"),
                    }
                });
            }

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
                        let state = app.state::<state::AppState>();
                        state
                            .transcription_running
                            .store(false, std::sync::atomic::Ordering::SeqCst);
                        state.reset_recording_session();
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

                                // Position: centered horizontally, near bottom of primary monitor
                                if let Some(monitor) = pill.primary_monitor().ok().flatten() {
                                    let screen = monitor.size();
                                    let scale = monitor.scale_factor();
                                    let pill_w = 104.0;
                                    let pill_h = 44.0;
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
            // and on virtual-desktop switches. set_always_on_top(true) is a no-op when
            // already true — must toggle false→true to actually re-promote Z-order.
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
        .invoke_handler(tauri::generate_handler![
            commands::get_auth_state,
            commands::get_current_user,
            commands::register,
            commands::login,
            commands::logout,
            commands::start_transcription,
            commands::stop_transcription,
            commands::start_dictation,
            commands::pause_dictation,
            commands::resume_dictation,
            commands::commit_dictation,
            commands::cancel_dictation,
            commands::get_usage_stats,
            commands::get_transcripts,
            commands::search_transcripts,
            commands::export_transcripts,
            commands::get_dictionary,
            commands::save_transcript,
            commands::delete_transcript,
            commands::update_dictionary,
            commands::delete_dictionary_entry,
            commands::apply_dictionary,
            commands::show_main_window,
            commands::hide_main_window,
            commands::type_text,
            commands::register_hotkey,
            commands::unregister_hotkey,
            commands::register_dictation_hotkey,
            commands::unregister_dictation_hotkey,
            commands::register_dictation_commit_hotkey,
            commands::unregister_dictation_commit_hotkey,
            commands::get_registered_hotkeys,
            commands::get_model_info,
            commands::retry_model_download,
            commands::cancel_model_download,
            commands::set_model_override,
            commands::clear_model_override,
            commands::get_active_engine,
            commands::set_active_engine,
            commands::download_parakeet,
            commands::get_beam_size,
            commands::set_beam_size,
            commands::get_hardware_profile,
            commands::get_downloaded_models,
            commands::delete_model,
            commands::get_format_config,
            commands::set_format_config,
            commands::test_format_connection,
            commands::open_logs_folder,
            commands::log_frontend,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            use tauri::RunEvent;
            if let RunEvent::ExitRequested { .. } = event {
                let state = app.state::<state::AppState>();
                state
                    .transcription_running
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                state.reset_recording_session();
            }
        });
}
