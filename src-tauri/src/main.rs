#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

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
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            // Database setup
            let app_data_dir = app.path().app_data_dir().map_err(std::io::Error::other)?;
            std::fs::create_dir_all(&app_data_dir)?;

            let db_path = app_data_dir.join("nexusvoice.db");
            let db_url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));

            let pool =
                tauri::async_runtime::block_on(database::connection::create_pool(&db_url))
                    .map_err(|err| std::io::Error::other(format!("database init failed: {err}")))?;
            tauri::async_runtime::block_on(database::connection::init_database(&pool)).map_err(
                |err| std::io::Error::other(format!("database migrations failed: {err}")),
            )?;

            let auth = auth::AuthService::new(pool.clone());
            let token_store_path = app_data_dir.join("refresh_token");
            let hotkey_store_path = app_data_dir.join("hotkey");
            let model_override_path = app_data_dir.join("model_override");
            let models_dir = app_data_dir.join("models");
            std::fs::create_dir_all(&models_dir)?;

            let app_state =
                state::AppState::new(pool, auth, token_store_path, hotkey_store_path, model_override_path, models_dir);
            app.manage(app_state);


            // Emit hardware profile event
            {
                use hardware::detector::detect_profile;
                use hardware::sysinfo_provider::SysinfoProvider;
                use inference::provider::{detect_backend, select_model_size};

                let hw = detect_profile(&SysinfoProvider);
                let backend = detect_backend();
                let model_size = select_model_size(backend, None);
                let _ = app.emit(
                    "hardware:profile",
                    serde_json::json!({
                        "gpuName": hw.gpu_type,
                        "executionProvider": hw.execution_provider,
                        "vramGb": hw.vram_gb,
                        "recommendedModel": model_size.display_name(),
                    }),
                );
            }

            // Silent re-auth on startup
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = app_handle.state::<state::AppState>();
                if let Some(raw_token) = state.load_refresh_token() {
                    match state.auth.refresh_tokens(&raw_token).await {
                        Ok(pair) => {
                            if let Ok(user_id) = state.auth.validate_token(&pair.access_token) {
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

            // Position pill window: centered horizontally, near bottom of primary monitor
            if let Some(pill) = app.get_webview_window("pill") {
                if let Some(monitor) = pill.primary_monitor().ok().flatten() {
                    let screen = monitor.size();
                    let scale = monitor.scale_factor();
                    let pill_w = 120.0;
                    let pill_h = 44.0;
                    let margin = 72.0; // 40px taskbar + 32px breathing room
                    let logical_w = screen.width as f64 / scale;
                    let logical_h = screen.height as f64 / scale;
                    let x = ((logical_w - pill_w) / 2.0) as i32;
                    let y = (logical_h - pill_h - margin) as i32;
                    let _ = pill.set_position(tauri::LogicalPosition::new(x, y));
                }
            }

            // Re-register persisted hotkey on startup
            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                let state = app.state::<state::AppState>();
                if let Some(hotkey) = state.load_hotkey() {
                    let app_handle = app.handle().clone();
                    let _ = app.global_shortcut().on_shortcut(
                        hotkey.as_str(),
                        move |_app, _shortcut, event| {
                            use tauri_plugin_global_shortcut::ShortcutState;
                            if event.state == ShortcutState::Pressed {
                                let _ = app_handle.emit("hotkey-pressed", ());
                            } else {
                                let _ = app_handle.emit("hotkey-released", ());
                            }
                        },
                    );
                    let mut current = tauri::async_runtime::block_on(state.current_hotkey.lock());
                    *current = Some(hotkey);
                }
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
                    std::thread::sleep(std::time::Duration::from_millis(200));
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
