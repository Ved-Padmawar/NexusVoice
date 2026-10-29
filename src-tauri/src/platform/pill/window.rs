//! Creating the pill window and keeping it above other windows.

use tauri::AppHandle;
#[cfg(windows)]
use tauri::Manager;

use super::geometry;

/// Built after setup returns: two `WebView2`s initializing at once hang a first
/// launch on Windows, and the main window's is alive by then.
pub fn spawn_create(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let config = app.config();
        let Some(win_config) = config.app.windows.iter().find(|w| w.label == "pill") else {
            return;
        };
        match tauri::WebviewWindowBuilder::from_config(&app, win_config)
            .and_then(tauri::WebviewWindowBuilder::build)
        {
            Ok(pill) => place(&app, &pill),
            Err(e) => log::error!("failed to create pill window: {e}"),
        }
    });
}

fn place(app: &AppHandle, pill: &tauri::WebviewWindow) {
    // Windows never resizes the pill; see clickthrough.
    #[cfg(windows)]
    let (pill_w, pill_h) = {
        super::clickthrough::spawn(app.clone());
        geometry::card_window()
    };
    #[cfg(not(windows))]
    let (pill_w, pill_h) = {
        let _ = app;
        geometry::capsule_window()
    };
    let _ = pill.set_size(tauri::LogicalSize::new(pill_w, pill_h));
    // Centered horizontally, near the bottom of the primary monitor.
    if let Some(monitor) = pill.primary_monitor().ok().flatten() {
        let screen = monitor.size();
        let scale = monitor.scale_factor();
        let margin = 72.0;
        let logical_w = f64::from(screen.width) / scale;
        let logical_h = f64::from(screen.height) / scale;
        #[allow(clippy::cast_possible_truncation)] // pixel coords fit i32
        let x = ((logical_w - pill_w) / 2.0) as i32;
        #[allow(clippy::cast_possible_truncation)]
        let y = (logical_h - pill_h - margin) as i32;
        let _ = pill.set_position(tauri::LogicalPosition::new(x, y));
    }
    let _ = pill.set_skip_taskbar(true);
    let _ = pill.show();
}

/// Windows demotes always-on-top windows when another window gains focus.
/// macOS and Linux keep the flag sticky.
#[cfg(windows)]
pub fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if let tauri::WindowEvent::Focused(true) = event {
        if window.label() != "pill" {
            if let Some(pill) = window.app_handle().get_webview_window("pill") {
                promote(&pill);
                let _ = pill.show();
            }
        }
    }
}

/// Windows also demotes it for fullscreen apps, Alt+Tab cycling and
/// virtual-desktop switches, none of which focus one of our windows.
#[cfg(windows)]
pub fn spawn_keep_on_top(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut iv = tokio::time::interval(std::time::Duration::from_secs(2));
        iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            iv.tick().await;
            if let Some(pill) = app.get_webview_window("pill") {
                if !pill.is_visible().unwrap_or(true) {
                    let _ = pill.show();
                }
                promote(&pill);
            }
        }
    });
}

/// Tauri must toggle the flag to re-promote a window already marked always-on-top.
#[cfg(windows)]
fn promote(pill: &tauri::WebviewWindow) {
    let _ = pill.set_always_on_top(false);
    let _ = pill.set_always_on_top(true);
}
