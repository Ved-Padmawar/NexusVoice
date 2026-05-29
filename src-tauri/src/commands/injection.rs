//! Text injection command — writes text to the clipboard and pastes via Ctrl+V.

use tauri::AppHandle;

use super::error::ApiError;

#[tauri::command]
pub async fn type_text(app: AppHandle, text: String) -> Result<(), ApiError> {
    use enigo::{Enigo, Key, Keyboard, Settings};
    use tauri_plugin_clipboard_manager::ClipboardExt;

    app.clipboard()
        .write_text(text)
        .map_err(|e| ApiError::new("clipboard_error", e.to_string()))?;

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(150));
        if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
            let _ = enigo.key(Key::Control, enigo::Direction::Press);
            let _ = enigo.key(Key::Unicode('v'), enigo::Direction::Click);
            let _ = enigo.key(Key::Control, enigo::Direction::Release);
        }
    });

    Ok(())
}
