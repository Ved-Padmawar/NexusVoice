//! Clipboard paste for Windows and macOS.

use enigo::{Enigo, Key, Keyboard, Settings};
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

#[cfg(target_os = "macos")]
const PASTE_MODIFIER: Key = Key::Meta;
#[cfg(not(target_os = "macos"))]
const PASTE_MODIFIER: Key = Key::Control;

const PASTE_DELAY_MS: u64 = 150;

/// # Errors
/// Returns an error when the transcript cannot be written to the clipboard.
/// The paste chord itself is sent from a worker thread and is not awaited.
pub async fn type_text(app: &AppHandle, text: &str) -> Result<(), String> {
    app.clipboard()
        .write_text(text.to_string())
        .map_err(|e| e.to_string())?;

    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(PASTE_DELAY_MS));
        if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
            let _ = enigo.key(PASTE_MODIFIER, enigo::Direction::Press);
            let _ = enigo.key(Key::Unicode('v'), enigo::Direction::Click);
            let _ = enigo.key(PASTE_MODIFIER, enigo::Direction::Release);
        }
    });

    Ok(())
}
