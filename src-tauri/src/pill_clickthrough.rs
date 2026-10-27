//! Windows keeps the pill window at card size: resizing a visible `WebView2`
//! window flashes a blank or corner-pinned frame. So the window ignores the
//! mouse everywhere but the pill's box, which the webview reports.

use std::sync::Mutex;

use serde::Deserialize;

use crate::sync::lock_recovering;

/// The pill's box in CSS pixels, relative to the window's client area.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, specta::Type)]
pub struct PillHitbox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

static HITBOX: Mutex<Option<PillHitbox>> = Mutex::new(None);

pub fn set_hitbox(hitbox: PillHitbox) {
    *lock_recovering(&HITBOX) = Some(hitbox);
}

/// `origin` and `cursor` are physical pixels.
#[cfg_attr(not(windows), allow(dead_code))]
fn contains(hitbox: PillHitbox, origin: (f64, f64), scale: f64, cursor: (f64, f64)) -> bool {
    let left = origin.0 + hitbox.x * scale;
    let top = origin.1 + hitbox.y * scale;
    (left..left + hitbox.width * scale).contains(&cursor.0)
        && (top..top + hitbox.height * scale).contains(&cursor.1)
}

/// Until the webview reports a box, the whole window lets clicks through.
#[cfg(windows)]
pub fn spawn(app: tauri::AppHandle) {
    use tauri::Manager;

    tauri::async_runtime::spawn(async move {
        let mut iv = tokio::time::interval(std::time::Duration::from_millis(16));
        iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut ignoring = None;
        loop {
            iv.tick().await;
            let Some(pill) = app.get_webview_window("pill") else {
                continue;
            };
            let hitbox = *lock_recovering(&HITBOX);
            let over = hitbox.is_some_and(|hitbox| {
                let (Ok(cursor), Ok(origin), Ok(scale)) = (
                    app.cursor_position(),
                    pill.inner_position(),
                    pill.scale_factor(),
                ) else {
                    return false;
                };
                contains(
                    hitbox,
                    (f64::from(origin.x), f64::from(origin.y)),
                    scale,
                    (cursor.x, cursor.y),
                )
            });
            if ignoring != Some(!over) && pill.set_ignore_cursor_events(!over).is_ok() {
                ignoring = Some(!over);
            }
        }
    });
}

#[cfg(test)]
#[path = "../tests/unit/pill_clickthrough.rs"]
mod tests;
