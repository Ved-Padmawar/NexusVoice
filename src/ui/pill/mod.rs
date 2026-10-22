//! The recording pill: a small always-on-top overlay that mirrors the
//! controller's state, and expands into a card for the live transcript.
//!
//! [`theme`] and [`waveform`] are shared with Settings, which previews the
//! pill themes and waveform styles with them.

mod geometry;
mod motion;
mod native;
pub mod theme;
mod transcript;
mod view;
pub mod waveform;

use gpui_kit::{
    point, px, size, App, AppContext as _, Bounds, Global, WindowBackgroundAppearance,
    WindowBounds, WindowDecorations, WindowHandle, WindowKind, WindowOptions,
};

use crate::events::Event;
use geometry::{capsule_window, BOTTOM_MARGIN};
use view::PillView;

/// The pill's window, kept so events can reach its view.
struct PillWindow(WindowHandle<PillView>);

impl Global for PillWindow {}

/// Open the pill at the bottom-centre of the primary display.
pub fn open(cx: &mut App) {
    let (width, height) = capsule_window();
    let window_size = size(px(width), px(height));
    let origin = cx
        .primary_display()
        .map_or(point(px(0.0), px(0.0)), |display| {
            let screen = display.bounds();
            point(
                screen.origin.x + (screen.size.width - window_size.width) / 2.0,
                screen.origin.y + screen.size.height - window_size.height - px(BOTTOM_MARGIN),
            )
        });

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, window_size))),
        titlebar: None,
        // Dictation types into whatever the user is in; the pill never takes focus.
        focus: false,
        show: true,
        kind: WindowKind::PopUp,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        window_decorations: Some(WindowDecorations::Client),
        // An unfocused window is otherwise held to 30 fps, which would make
        // the waveform and the card's growth visibly choppy.
        inactive_frame_interval: None,
        app_id: Some("nexusvoice-pill".into()),
        ..WindowOptions::default()
    };
    match cx.open_window(options, |window, cx| cx.new(|cx| PillView::new(window, cx))) {
        Ok(handle) => cx.set_global(PillWindow(handle)),
        Err(e) => log::error!("could not open the pill window: {e}"),
    }
}

/// Route a pill event to the view.
pub fn handle(event: Event, cx: &mut App) {
    let Some(PillWindow(handle)) = cx.try_global::<PillWindow>() else {
        return;
    };
    let handle = *handle;
    let _ = handle.update(cx, |pill, window, cx| pill.handle(event, window, cx));
}
