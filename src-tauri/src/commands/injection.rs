//! Text injection command — writes text to the clipboard and pastes via the
//! platform paste shortcut (Cmd+V on macOS, Ctrl+V elsewhere).

use tauri::AppHandle;

use super::error::ApiError;

/// The modifier held during the paste keystroke. macOS pastes with Command;
/// Windows and Linux paste with Control. Selected at compile time so there is a
/// single correct keystroke per OS.
#[cfg(target_os = "macos")]
const PASTE_MODIFIER: enigo::Key = enigo::Key::Meta;
#[cfg(not(target_os = "macos"))]
const PASTE_MODIFIER: enigo::Key = enigo::Key::Control;

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
            let _ = enigo.key(PASTE_MODIFIER, enigo::Direction::Press);
            let _ = enigo.key(Key::Unicode('v'), enigo::Direction::Click);
            let _ = enigo.key(PASTE_MODIFIER, enigo::Direction::Release);
        }
    });

    Ok(())
}
