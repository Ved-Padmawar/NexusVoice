//! The GPUI front end: the main window and the recording pill.
//!
//! Views never poll. Backend events arrive on one channel, drained here on the
//! UI thread and routed to the entity that cares: pill events to the pill,
//! data events to the [`store`]. Only the views reading what changed repaint.

mod assets;
pub mod components;
mod icons;
mod pages;
mod pill;
pub mod runtime;
pub mod settings;
mod shell;
pub mod store;
pub mod theme;
pub mod vendors;

use gpui_kit::{App, AsyncApp, QuitMode};

use crate::events::{self, Event};
use crate::platform;
use settings::UiSettings;

pub fn run() {
    gpui_kit::application()
        .with_assets(assets::Assets)
        // The pill window outlives the main one, but quitting is the tray's call.
        .with_quit_mode(QuitMode::Explicit)
        .run(|cx| {
            gpui_kit::init(cx);
            assets::load_fonts(cx);

            let settings = UiSettings::load();
            theme::apply(settings.theme, cx);
            cx.set_global(settings);
            cx.observe_global::<UiSettings>(|cx| {
                let wanted = UiSettings::global(cx).theme;
                if theme::Palette::global(cx) != &theme::Palette::of(wanted) {
                    theme::apply(wanted, cx);
                }
            })
            .detach();

            store::init(cx);
            platform::tray::install();
            shell::init_hotkeys(cx);
            pill::open(cx);
            shell::open_main_window(cx);

            cx.spawn(pump_events).detach();
        });
}

/// Route every backend event to its view, in order, on the UI thread.
async fn pump_events(cx: &mut AsyncApp) {
    let receiver = events::receiver();
    while let Ok(event) = receiver.recv().await {
        cx.update(|cx| dispatch(event, cx));
    }
}

fn dispatch(event: Event, cx: &mut App) {
    match event {
        Event::Pill(_)
        | Event::PillNotice(_)
        | Event::Waveform(_)
        | Event::Partial(_)
        | Event::PartialEnd => pill::handle(event, cx),
        Event::ShowMainWindow => shell::open_main_window(cx),
        Event::Quit => cx.quit(),
        Event::ModelDownload { .. }
        | Event::ModelEvicted
        | Event::ModelSwitched
        | Event::LanguageReset
        | Event::Cuda(_)
        | Event::TranscriptSaved(_) => store::handle(event, cx),
    }
}
