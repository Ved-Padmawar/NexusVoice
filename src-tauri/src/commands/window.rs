//! Window & shell commands: forward webview logs, open the logs folder.

use tauri::{AppHandle, Manager, State};

use crate::state::AppState;

use super::error::ApiError;

/// Resolves once the database is open. A command, not an event — an event fired
/// before the webview attaches its listener would be missed, wedging startup.
#[tauri::command]
#[specta::specta]
pub async fn wait_for_app_ready(state: State<'_, AppState>) -> Result<(), ApiError> {
    state.db().await?;
    Ok(())
}

/// Forward a log record from the webview into the unified backend log file,
/// so frontend errors land in the same structured log as backend events.
/// `level` is one of: error, warn, info, debug, trace (defaults to info).
#[tauri::command]
#[specta::specta]
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
#[specta::specta]
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
#[cfg(not(windows))]
static PILL_ANCHOR: std::sync::Mutex<Option<(f64, f64)>> = std::sync::Mutex::new(None);

#[cfg(windows)]
static PILL_REGION_EXPANDED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Keep `WebView2`'s viewport stable. Windows clips both painting and hit testing
/// to this region, so the transparent area above an idle pill does not eat clicks.
#[cfg(windows)]
pub(crate) fn set_pill_window_region(
    pill: &tauri::WebviewWindow,
    expanded: bool,
) -> Result<(), ApiError> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{CreateRectRgn, DeleteObject, SetWindowRgn, HGDIOBJ};

    // SetWindowRgn repaints the nonclient area, so clear any frame bits Tauri
    // restored since startup before they can be drawn.
    remove_pill_native_frame(pill)?;

    let hwnd = HWND(
        pill.hwnd()
            .map_err(|e| ApiError::new("window_error", e.to_string()))?
            .0,
    );
    let region = if expanded {
        None
    } else {
        let scale = pill
            .scale_factor()
            .map_err(|e| ApiError::new("window_error", e.to_string()))?;
        let size = pill
            .outer_size()
            .map_err(|e| ApiError::new("window_error", e.to_string()))?;
        let (w, h) = crate::pill_geometry::capsule_window();
        let capsule = tauri::LogicalSize::new(w, h).to_physical::<u32>(scale);
        let width = capsule.width.min(size.width).cast_signed();
        let height = capsule.height.min(size.height).cast_signed();
        let left = (size.width.cast_signed() - width) / 2;
        let top = size.height.cast_signed() - height;
        // SAFETY: integer coordinates only; ownership is handled below.
        let region = unsafe { CreateRectRgn(left, top, left + width, top + height) };
        if region.0.is_null() {
            return Err(ApiError::new(
                "window_error",
                std::io::Error::last_os_error().to_string(),
            ));
        }
        Some(region)
    };
    // SAFETY: hwnd belongs to a live Tauri window. On success Windows takes
    // ownership of the region; on failure we must release it ourselves.
    if unsafe { SetWindowRgn(hwnd, region, true) } == 0 {
        let error = std::io::Error::last_os_error();
        if let Some(region) = region {
            let _ = unsafe { DeleteObject(HGDIOBJ(region.0)) };
        }
        return Err(ApiError::new("window_error", error.to_string()));
    }
    PILL_REGION_EXPANDED.store(expanded, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

#[cfg(windows)]
pub(crate) fn refresh_pill_window_region(pill: &tauri::WebviewWindow) -> Result<(), ApiError> {
    set_pill_window_region(
        pill,
        PILL_REGION_EXPANDED.load(std::sync::atomic::Ordering::Relaxed),
    )
}

/// Tauri hides decorations through `WM_NCCALCSIZE` but retains `WS_CAPTION`.
/// A region change can still paint that native frame for one frame. Strip the
/// unused chrome before showing this overlay; keep normal main-window styles.
#[cfg(windows)]
pub(crate) fn remove_pill_native_frame(pill: &tauri::WebviewWindow) -> Result<(), ApiError> {
    use windows::Win32::Foundation::{GetLastError, SetLastError, ERROR_SUCCESS, HWND};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE,
        SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_CAPTION,
        WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_EX_STATICEDGE, WS_EX_WINDOWEDGE, WS_MAXIMIZEBOX,
        WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
    };

    let hwnd = HWND(
        pill.hwnd()
            .map_err(|e| ApiError::new("window_error", e.to_string()))?
            .0,
    );
    let masks = [
        (
            GWL_STYLE,
            (WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX).0,
        ),
        (
            GWL_EXSTYLE,
            (WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE | WS_EX_DLGMODALFRAME | WS_EX_STATICEDGE).0,
        ),
    ];
    // SAFETY: the live, still-hidden pill window belongs to this process. Only
    // decoration bits are removed; transparency, activation and Z-order stay.
    unsafe {
        for (index, mask) in masks {
            let current = GetWindowLongPtrW(hwnd, index);
            SetLastError(ERROR_SUCCESS);
            if SetWindowLongPtrW(hwnd, index, current & !(isize::try_from(mask).unwrap_or(0))) == 0
                && GetLastError() != ERROR_SUCCESS
            {
                return Err(ApiError::new(
                    "window_error",
                    std::io::Error::last_os_error().to_string(),
                ));
            }
        }
        SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        )
    }
    .map_err(|e| ApiError::new("window_error", e.to_string()))
}

/// Reassert Z-order without rewriting window styles or recalculating its frame.
/// Toggling Tauri's always-on-top flag also forces `SWP_FRAMECHANGED`, which can
/// expose the native caption through a transparent, undecorated `WebView2` window.
#[cfg(windows)]
pub(crate) fn raise_pill_without_activation(pill: &tauri::WebviewWindow) -> Result<(), ApiError> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };

    let hwnd = HWND(
        pill.hwnd()
            .map_err(|e| ApiError::new("window_error", e.to_string()))?
            .0,
    );
    // SAFETY: a live pill HWND; no size/position or focus change is requested.
    unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    }
    .map_err(|e| ApiError::new("window_error", e.to_string()))
}

/// Resize the pill window to one of its two shapes, keeping its bottom-centre
/// fixed so the capsule stays put while the card grows above it.
#[tauri::command]
#[specta::specta]
pub fn resize_pill(app: AppHandle, expanded: bool) -> Result<(), ApiError> {
    #[cfg(windows)]
    {
        let Some(pill) = app.get_webview_window("pill") else {
            return Ok(());
        };
        set_pill_window_region(&pill, expanded)
    }
    #[cfg(not(windows))]
    {
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
}
