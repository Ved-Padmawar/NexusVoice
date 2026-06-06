//! Window & shell commands: show/hide the main window, open the logs folder.

use tauri::{AppHandle, Manager};

use super::error::ApiError;

#[tauri::command]
pub async fn show_main_window(app: AppHandle) -> Result<(), ApiError> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .show()
            .map_err(|e| ApiError::new("window_error", e.to_string()))?;
        window
            .set_focus()
            .map_err(|e| ApiError::new("window_error", e.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn hide_main_window(app: AppHandle) -> Result<(), ApiError> {
    if let Some(window) = app.get_webview_window("main") {
        window
            .hide()
            .map_err(|e| ApiError::new("window_error", e.to_string()))?;
    }
    Ok(())
}

/// Forward a log record from the webview into the unified backend log file,
/// so frontend errors land in the same structured log as backend events.
/// `level` is one of: error, warn, info, debug, trace (defaults to info).
#[tauri::command]
pub fn log_frontend(level: String, message: String, context: Option<String>) {
    let msg = match context {
        Some(ctx) if !ctx.is_empty() => format!("[frontend] {message} {ctx}"),
        _ => format!("[frontend] {message}"),
    };
    match level.to_ascii_lowercase().as_str() {
        "error" => log::error!(target: "frontend", "{msg}"),
        "warn" => log::warn!(target: "frontend", "{msg}"),
        "debug" => log::debug!(target: "frontend", "{msg}"),
        "trace" => log::trace!(target: "frontend", "{msg}"),
        _ => log::info!(target: "frontend", "{msg}"),
    }
}

#[tauri::command]
pub async fn open_logs_folder(app: AppHandle) -> Result<(), ApiError> {
    let logs_dir = app
        .path()
        .app_log_dir()
        .map_err(|e| ApiError::new("path_error", e.to_string()))?;
    std::fs::create_dir_all(&logs_dir).map_err(|e| ApiError::new("io_error", e.to_string()))?;
    opener::open(&logs_dir).map_err(|e| ApiError::new("open_error", e.to_string()))?;
    Ok(())
}
