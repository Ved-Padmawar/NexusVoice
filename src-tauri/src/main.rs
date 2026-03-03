#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
  menu::{Menu, MenuItem},
  tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
  Manager,
};

mod database;
mod auth;
mod hardware;
mod models;
mod inference;
mod audio;
mod postprocess;
mod commands;
mod state;

fn main() {
  tauri::Builder::default()
    .plugin(tauri_plugin_global_shortcut::Builder::new().build())
    .plugin(tauri_plugin_clipboard_manager::init())
    .setup(|app| {
      // Database setup
      let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
      std::fs::create_dir_all(&app_data_dir)?;

      let db_path = app_data_dir.join("nexusvoice.db");
      let db_url = format!(
        "sqlite://{}",
        db_path.to_string_lossy().replace('\\', "/")
      );

      let pool = tauri::async_runtime::block_on(database::connection::create_pool(&db_url))
        .map_err(|err| {
          std::io::Error::new(std::io::ErrorKind::Other, format!("database init failed: {err}"))
        })?;
      tauri::async_runtime::block_on(database::connection::init_database(&pool))
        .map_err(|err| {
          std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("database migrations failed: {err}"),
          )
        })?;

      let auth = auth::AuthService::new(pool.clone());
      app.manage(state::AppState::new(pool, auth));

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

      // Note: No default hotkey is registered. Users must set it in Settings.

      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      commands::register,
      commands::login,
      commands::get_hardware_profile,
      commands::start_transcription,
      commands::stop_transcription,
      commands::get_model_info,
      commands::get_transcripts,
      commands::get_dictionary,
      commands::save_transcript,
      commands::update_dictionary,
      commands::show_main_window,
      commands::hide_main_window,
      commands::type_text,
      commands::register_hotkey,
      commands::get_registered_hotkeys,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
