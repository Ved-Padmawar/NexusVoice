//! Window & shell commands: forward webview logs, open the logs folder.

use tauri::{AppHandle, Manager};

use super::error::ApiError;

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

/// Bottom-centre the pill is pinned to. Re-read whenever it is back at capsule
/// size so dragging still moves it — re-deriving on every call drifts.
static PILL_ANCHOR: std::sync::Mutex<Option<(f64, f64)>> = std::sync::Mutex::new(None);

/// Resize the pill window to one of its two shapes, keeping its bottom-centre
/// fixed so the capsule stays put while the card grows above it.
#[tauri::command]
pub fn resize_pill(app: AppHandle, expanded: bool) -> Result<(), ApiError> {
    use crate::pill_geometry::{capsule_window, card_window};

    let Some(pill) = app.get_webview_window("pill") else {
        return Ok(());
    };
    let (width, height) = if expanded {
        card_window()
    } else {
        capsule_window()
    };
    let scale = pill
        .scale_factor()
        .map_err(|e| ApiError::new("window_error", e.to_string()))?;
    let pos = pill
        .outer_position()
        .map_err(|e| ApiError::new("window_error", e.to_string()))?
        .to_logical::<f64>(scale);
    let size = pill
        .outer_size()
        .map_err(|e| ApiError::new("window_error", e.to_string()))?
        .to_logical::<f64>(scale);

    let mut anchor = PILL_ANCHOR
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (ax, ay) = match *anchor {
        Some(a) if expanded => a,
        _ => {
            let a = (pos.x + size.width / 2.0, pos.y + size.height);
            *anchor = Some(a);
            a
        }
    };

    pill.set_size(tauri::LogicalSize::new(width, height))
        .map_err(|e| ApiError::new("window_error", e.to_string()))?;
    pill.set_position(tauri::LogicalPosition::new(ax - width / 2.0, ay - height))
        .map_err(|e| ApiError::new("window_error", e.to_string()))?;
    Ok(())
}
