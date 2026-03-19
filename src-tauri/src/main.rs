#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

// 10 MB in bytes
const LOG_MAX_SIZE: u128 = 10 * 1024 * 1024;

mod audio;
mod auth;
mod commands;
mod database;
mod hardware;
mod inference;
mod postprocess;
mod preprocess;
mod state;

fn main() {
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
        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir { file_name: Some("nexusvoice".into()) }),
    ];
    #[cfg(not(debug_assertions))]
    let log_targets = vec![
        tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir { file_name: Some("nexusvoice".into()) }),
    ];

    #[cfg(debug_assertions)]
    let log_level = log::LevelFilter::Debug;
    #[cfg(not(debug_assertions))]
    let log_level = log::LevelFilter::Info;

    tauri::Builder::default()
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
                .max_file_size(LOG_MAX_SIZE)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().map_err(std::io::Error::other)?;
            std::fs::create_dir_all(&app_data_dir)?;

            let token_store_path = app_data_dir.join("refresh_token");
            let hotkey_store_path = app_data_dir.join("hotkey");
            let model_override_path = app_data_dir.join("model_override");
            let models_dir = app_data_dir.join("models");
            std::fs::create_dir_all(&models_dir)?;

            // Create state immediately with NO blocking I/O.
            // DB + auth are initialized asynchronously after setup returns.
            let app_state = state::AppState::new(
                app_data_dir,
                token_store_path,
                hotkey_store_path,
                model_override_path,
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
                    let jwt_secret_path = state.app_data_dir.join("jwt_secret");
                    let jwt_secret = match auth::load_or_create_jwt_secret(&jwt_secret_path) {
                        Ok(s) => s,
                        Err(e) => {
                            log::error!("jwt secret init failed: {e}");
                            let _ = app_handle.emit("auth:unauthenticated", ());
                            return;
                        }
                    };
                    let auth_service = auth::AuthService::new(pool.clone(), jwt_secret);
                    state.set_pool(pool.clone()).await;
                    state.set_auth(auth_service).await;

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

                    // Silent re-auth
                    if let Some(raw_token) = state.load_refresh_token() {
                        match state.auth().await.refresh_tokens(&raw_token).await {
                            Ok(pair) => {
                                if let Ok(user_id) = state.auth().await.validate_token(&pair.access_token) {
                                    state.set_auth_session(user_id, pair.access_token).await;
                                    let _ = state.save_refresh_token(&pair.refresh_token);
                                    let _ = app_handle.emit("auth:ready", user_id);
                                    return;
                                }
                            }
                            Err(_) => {
                                state.delete_refresh_token();
                            }
                        }
                    }
                    let _ = app_handle.emit("auth:unauthenticated", ());
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
                    }).await.unwrap_or_else(|_| (Default::default(), inference::provider::ModelSize::Small));

                    log::info!("NexusVoice v{} starting", env!("CARGO_PKG_VERSION"));
                    log::info!("OS: {}", std::env::consts::OS);
                    log::info!("RAM: {:.1} GB", hw.ram_gb);
                    log::info!("GPU: {} ({}, {:.1} GB VRAM)", hw.gpu_type, hw.execution_provider, hw.vram_gb);
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
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<state::AppState>();
                    if let Some(hotkey) = state.load_hotkey() {
                        let inner_handle = app_handle.clone();
                        let _ = app_handle.global_shortcut().on_shortcut(
                            hotkey.as_str(),
                            move |_app, _shortcut, event| {
                                use tauri_plugin_global_shortcut::ShortcutState;
                                if event.state == ShortcutState::Pressed {
                                    let _ = inner_handle.emit("hotkey-pressed", ());
                                } else {
                                    let _ = inner_handle.emit("hotkey-released", ());
                                }
                            },
                        );
                        *state.current_hotkey.lock().await = Some(hotkey);
                    }
                });
            }

            // System tray
            let show_item = MenuItem::with_id(app, "show", "Show Dashboard", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
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
                        std::thread::sleep(std::time::Duration::from_millis(200));
                        app.exit(0);
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
                    if let Some(win_config) = config.app.windows.iter().find(|w| w.label == "pill") {
                        match tauri::WebviewWindowBuilder::from_config(&app_handle, win_config)
                            .and_then(|b| b.build())
                        {
                            Ok(pill) => {
                                // Position: centered horizontally, near bottom of primary monitor
                                if let Some(monitor) = pill.primary_monitor().ok().flatten() {
                                    let screen = monitor.size();
                                    let scale = monitor.scale_factor();
                                    let pill_w = 120.0;
                                    let pill_h = 44.0;
                                    let margin = 72.0;
                                    let logical_w = screen.width as f64 / scale;
                                    let logical_h = screen.height as f64 / scale;
                                    let x = ((logical_w - pill_w) / 2.0) as i32;
                                    let y = (logical_h - pill_h - margin) as i32;
                                    let _ = pill.set_position(tauri::LogicalPosition::new(x, y));
                                }
                                let _ = pill.show();
                            }
                            Err(e) => log::error!("failed to create pill window: {e}"),
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_auth_state,
            commands::get_current_user,
            commands::store_refresh_token,
            commands::clear_stored_token,
            commands::register,
            commands::login,
            commands::login_with_tokens,
            commands::register_with_tokens,
            commands::refresh_token,
            commands::logout_token,
            commands::start_transcription,
            commands::stop_transcription,
            commands::get_usage_stats,
            commands::get_transcripts,
            commands::search_transcripts,
            commands::export_transcripts,
            commands::get_dictionary,
            commands::save_transcript,
            commands::update_dictionary,
            commands::delete_dictionary_entry,
            commands::apply_dictionary,
            commands::show_main_window,
            commands::hide_main_window,
            commands::type_text,
            commands::register_hotkey,
            commands::unregister_hotkey,
            commands::get_registered_hotkeys,
            commands::get_model_info,
            commands::retry_model_download,
            commands::set_model_override,
            commands::clear_model_override,
            commands::get_hardware_profile,
            commands::open_logs_folder,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            use tauri::RunEvent;
            match event {
                RunEvent::Exit => {
                    let state = app.state::<state::AppState>();
                    state
                        .transcription_running
                        .store(false, std::sync::atomic::Ordering::SeqCst);
                    let app_handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app_handle.state::<state::AppState>();
                        *state.engine.lock().await = None;
                        state.db().await.close().await;
                    });
                    std::thread::sleep(std::time::Duration::from_millis(300));
                }
                RunEvent::ExitRequested { .. } => {
                    let state = app.state::<state::AppState>();
                    state
                        .transcription_running
                        .store(false, std::sync::atomic::Ordering::SeqCst);
                }
                _ => {}
            }
        });
}
