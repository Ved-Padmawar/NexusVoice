//! The tray icon: left click or "Show Dashboard" opens the window, "Quit" exits.

use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::events::{self, Event};

const SHOW: &str = "show";
const QUIT: &str = "quit";

fn build() -> Result<TrayIcon, String> {
    // A 256px source downscales cleanly; the small default layer looks blurry.
    let image = image::load_from_memory(include_bytes!("../../packaging/icons/tray.png"))
        .map_err(|e| e.to_string())?
        .into_rgba8();
    let (width, height) = image.dimensions();
    let icon = Icon::from_rgba(image.into_raw(), width, height).map_err(|e| e.to_string())?;

    let menu = Menu::new();
    menu.append_items(&[
        &MenuItem::with_id(SHOW, "Show Dashboard", true, None),
        &MenuItem::with_id(QUIT, "Quit", true, None),
    ])
    .map_err(|e| e.to_string())?;

    MenuEvent::set_event_handler(Some(|event: MenuEvent| match event.id.as_ref() {
        SHOW => events::emit(Event::ShowMainWindow),
        QUIT => events::emit(Event::Quit),
        _ => {}
    }));
    TrayIconEvent::set_event_handler(Some(|event: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            events::emit(Event::ShowMainWindow);
        }
    }));

    TrayIconBuilder::new()
        .with_icon(icon)
        .with_tooltip(super::paths::PRODUCT_NAME)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .map_err(|e| e.to_string())
}

/// Linux trays are GTK widgets, so the icon gets a GTK main loop thread.
#[cfg(target_os = "linux")]
pub fn install() {
    let spawned = std::thread::Builder::new().name("tray".into()).spawn(|| {
        if let Err(e) = gtk::init() {
            log::warn!("tray unavailable, GTK failed to start: {e}");
            return;
        }
        match build() {
            Ok(_tray) => gtk::main(),
            Err(e) => log::warn!("tray unavailable: {e}"),
        }
    });
    if let Err(e) = spawned {
        log::warn!("tray thread failed to start: {e}");
    }
}

/// Built on the UI thread, which receives its messages on Windows.
#[cfg(not(target_os = "linux"))]
pub fn install() {
    thread_local! {
        static TRAY: std::cell::OnceCell<TrayIcon> = const { std::cell::OnceCell::new() };
    }
    match build() {
        Ok(tray) => TRAY.with(|slot| {
            let _ = slot.set(tray);
        }),
        Err(e) => log::warn!("tray unavailable: {e}"),
    }
}
