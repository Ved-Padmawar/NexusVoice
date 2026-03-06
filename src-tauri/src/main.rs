#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

mod auth;
mod commands;
mod database;
mod hardware;
mod inference;
mod models;
mod postprocess;
mod state;

fn main() {
    tauri::Builder::default()
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
            let models_dir = app_data_dir.join("models");
            std::fs::create_dir_all(&models_dir)?;
            let app_state = state::AppState::new(pool, auth, token_store_path, hotkey_store_path, models_dir);
            app.manage(app_state);

            // Silent re-auth on startup
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = app_handle.state::<state::AppState>();
                if let Some(raw_token) = state.load_refresh_token() {
                    match state.auth.refresh_tokens(&raw_token).await {
                        Ok(pair) => {
                            // Parse user_id from the new access token
                            if let Ok(user_id) = state.auth.validate_token(&pair.access_token) {
                                state.set_auth_session(user_id, pair.access_token).await;
                                // Persist the new rotated refresh token
                                let _ = state.save_refresh_token(&pair.refresh_token);
                                let _ = app_handle.emit("auth:ready", user_id);
                                return;
                            }
                        }
                        Err(_) => {
                            // Token invalid/expired — clear it
                            state.delete_refresh_token();
                        }
                    }
                }
                let _ = app_handle.emit("auth:unauthenticated", ());
            });

            // System tray setup
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
                        // Stop any active mic capture before exiting —
                        // app.exit() calls std::process::exit() which does NOT
                        // trigger RunEvent::Exit, so we must clean up here.
                        let state = app.state::<state::AppState>();
                        state.transcription_running.store(
                            false,
                            std::sync::atomic::Ordering::SeqCst,
                        );
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

            // Restore persisted model override
            {
                let state = app.state::<state::AppState>();
                if let Some(size_str) = state.load_model_override() {
                    let parsed = match size_str.as_str() {
                        "tiny"   => Some(models::ModelSize::Tiny),
                        "base"   => Some(models::ModelSize::Base),
                        "small"  => Some(models::ModelSize::Small),
                        "medium" => Some(models::ModelSize::Medium),
                        "large"  => Some(models::ModelSize::Large),
                        _ => None,
                    };
                    if let Some(size) = parsed {
                        let mut guard = tauri::async_runtime::block_on(state.model_override.lock());
                        *guard = Some(size);
                    }
                }
            }

            // Position pill window: centered horizontally, near bottom of primary monitor
            if let Some(pill) = app.get_webview_window("pill") {
                if let Some(monitor) = pill.primary_monitor().ok().flatten() {
                    let screen = monitor.size();
                    let scale = monitor.scale_factor();
                    // Physical pill size → logical pixels
                    let pill_w = 176.0;
                    let pill_h = 44.0;
                    let margin = 24.0;
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
            commands::store_refresh_token,
            commands::clear_stored_token,
            commands::register,
            commands::login,
            commands::login_with_tokens,
            commands::register_with_tokens,
            commands::refresh_token,
            commands::logout_token,
            commands::get_hardware_tier,
            commands::get_hardware_profile,
            commands::set_model_override,
            commands::clear_model_override,
            commands::start_transcription,
            commands::stop_transcription,
            commands::get_model_info,
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
            commands::get_word_suggestions,
            commands::accept_word_suggestion,
            commands::dismiss_word_suggestion,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            use tauri::RunEvent;
            match event {
                RunEvent::Exit => {
                    // Ensure any active mic capture thread is stopped before the
                    // process exits. Tauri does NOT call Drop on managed state,
                    // so we must signal manually.
                    let state = app.state::<state::AppState>();
                    state.transcription_running.store(false, std::sync::atomic::Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                RunEvent::ExitRequested { .. } => {
                    // Also stop recording when the last window is closed
                    let state = app.state::<state::AppState>();
                    state.transcription_running.store(false, std::sync::atomic::Ordering::SeqCst);
                }
                _ => {}
            }
        });
}
