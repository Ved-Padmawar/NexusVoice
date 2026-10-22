//! The main window: title bar, sidebar, the active page, the startup screen,
//! first-run modals and toasts.
//!
//! Closing the window removes it; the app lives on in the tray and the pill.
//! Pages are created on first visit and kept, so switching back is instant and
//! keeps scroll position and inputs.

mod sidebar;
mod title_bar;

use std::time::Duration;

use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt as _;
use gpui_kit::{
    div, prelude::*, px, size, App, AppContext as _, Bounds, Context, Entity, Global, Subscription,
    WindowBounds, WindowDecorations, WindowHandle, WindowOptions,
};

use super::components::spinner;
use super::pages::{CudaOffer, Dashboard, Dictionary, ModelPicker, Settings};
use super::settings::{Route, UiSettings};
use super::store::{Notice, Startup, Store, StoreEvent};
use super::theme::Palette;
use crate::platform::{self, paths};
use crate::state::HotkeyKind;

/// The system-wide hotkeys, owned by the UI thread (see `platform::hotkeys`).
/// `None` where the platform offers none, e.g. Wayland.
pub struct Hotkeys(Option<platform::hotkeys::Hotkeys>);

impl Global for Hotkeys {}

impl Hotkeys {
    pub fn bindings(cx: &App) -> Option<&crate::api::HotkeyBindings> {
        cx.global::<Self>().0.as_ref().map(platform::hotkeys::Hotkeys::bindings)
    }

    /// Bind or clear one hotkey.
    ///
    /// # Errors
    /// The hotkey is invalid, taken, or global hotkeys are unavailable.
    pub fn set(
        kind: HotkeyKind,
        hotkey: Option<String>,
        cx: &mut App,
    ) -> Result<(), crate::api::ApiError> {
        match cx.global_mut::<Self>().0.as_mut() {
            Some(hotkeys) => hotkeys.set(kind, hotkey),
            None => Err(crate::api::ApiError::new(
                "hotkeys_unavailable",
                "global hotkeys are not available in this session",
            )),
        }
    }
}

pub fn init_hotkeys(cx: &mut App) {
    let hotkeys = platform::hotkeys::Hotkeys::new()
        .inspect_err(|e| log::warn!("global hotkeys unavailable: {e}"))
        .ok();
    cx.set_global(Hotkeys(hotkeys));
}

struct MainWindowHandle(Option<WindowHandle<gpui_kit::base::Root>>);

impl Global for MainWindowHandle {}

/// Bring the main window forward, opening it if it was closed.
pub fn open_main_window(cx: &mut App) {
    let existing = cx
        .try_global::<MainWindowHandle>()
        .and_then(|handle| handle.0);
    if let Some(handle) = existing {
        if handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
        {
            return;
        }
    }

    let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(860.), px(560.))),
        window_decorations: Some(WindowDecorations::Client),
        window_background: gpui_kit::WindowBackgroundAppearance::Opaque,
        app_id: Some(paths::IDENTIFIER.to_string()),
        ..gpui_kit::component::TitleBar::window_options()
    };
    match gpui_kit::open_window(options, cx, |window, cx| {
        window.set_window_title(paths::PRODUCT_NAME);
        cx.new(|cx| MainWindow::new(window, cx))
    }) {
        Ok((handle, _)) => {
            cx.set_global(MainWindowHandle(handle.downcast()));
        }
        Err(e) => log::error!("could not open the main window: {e}"),
    }
}

pub struct MainWindow {
    dashboard: Option<Entity<Dashboard>>,
    dictionary: Option<Entity<Dictionary>>,
    settings: Option<Entity<Settings>>,
    model_picker: Option<Entity<ModelPicker>>,
    cuda_offer: Option<Entity<CudaOffer>>,
    /// The spinner waits a beat, so a fast database open doesn't flash it.
    show_startup_spinner: bool,
    _subscriptions: Vec<Subscription>,
}

impl MainWindow {
    fn new(window: &mut gpui_kit::Window, cx: &mut Context<Self>) -> Self {
        let store = Store::entity(cx);
        let subscriptions = vec![
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.observe_global::<UiSettings>(|_, cx| cx.notify()),
            cx.subscribe_in(&store, window, |_, _, event: &StoreEvent, window, cx| {
                if let StoreEvent::Notice(notice) = event {
                    window.push_notification(toast(notice), cx);
                }
            }),
        ];
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(250)).await;
            let _ = this.update(cx, |this, cx| {
                this.show_startup_spinner = true;
                cx.notify();
            });
        })
        .detach();

        Self {
            dashboard: None,
            dictionary: None,
            settings: None,
            model_picker: None,
            cuda_offer: None,
            show_startup_spinner: false,
            _subscriptions: subscriptions,
        }
    }

    fn page(&mut self, route: Route, window: &mut gpui_kit::Window, cx: &mut Context<Self>) -> gpui_kit::AnyView {
        match route {
            Route::Dashboard => self
                .dashboard
                .get_or_insert_with(|| cx.new(|cx| Dashboard::new(window, cx)))
                .clone()
                .into(),
            Route::Dictionary => self
                .dictionary
                .get_or_insert_with(|| cx.new(|cx| Dictionary::new(window, cx)))
                .clone()
                .into(),
            Route::Settings => self
                .settings
                .get_or_insert_with(|| cx.new(|cx| Settings::new(window, cx)))
                .clone()
                .into(),
        }
    }

    fn render_startup(&self, cx: &App) -> impl IntoElement {
        let p = Palette::global(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.))
            .relative()
            .bg(p.bg)
            .child(title_bar::TitleBar)
            .when(self.show_startup_spinner, |el| {
                el.child(spinner(
                    "startup-spinner",
                    28.,
                    2.,
                    p.border,
                    p.accent,
                    Duration::from_millis(650),
                ))
                .child(div().text_size(px(12.)).text_color(p.muted).child("Loading…"))
            })
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut gpui_kit::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let store = Store::entity(cx).read(cx);
        if store.startup == Startup::Starting {
            return self.render_startup(cx).into_any_element();
        }
        let cuda_offer_open = store.cuda_offer_open;
        let settings = UiSettings::global(cx);
        let (route, model_chosen) = (settings.route, settings.model_chosen);

        if model_chosen {
            self.model_picker = None;
        } else if self.model_picker.is_none() {
            self.model_picker = Some(cx.new(|cx| ModelPicker::new(window, cx)));
        }
        if !cuda_offer_open {
            self.cuda_offer = None;
        } else if self.cuda_offer.is_none() {
            self.cuda_offer = Some(cx.new(|cx| CudaOffer::new(window, cx)));
        }
        let page = self.page(route, window, cx);

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(p.bg)
            .font_family(super::theme::FONT_FAMILY)
            .text_size(super::theme::FONT_SIZE)
            .text_color(p.fg)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(sidebar::Sidebar { route })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .overflow_hidden()
                            .child(page),
                    ),
            )
            .child(title_bar::TitleBar)
            .children(self.cuda_offer.clone())
            .children(self.model_picker.clone())
            .into_any_element()
    }
}

fn toast(notice: &Notice) -> Notification {
    match notice {
        Notice::Info(message) => Notification::info(message.clone()),
        Notice::Success(message) => Notification::success(message.clone()),
        Notice::Error(message) => Notification::error(message.clone()),
        Notice::CudaReady => Notification::success("CUDA is ready. Restart NexusVoice to use it.")
            .action(|_, _, _| {
                gpui_kit::component::button::Button::new("restart")
                    .label("Restart")
                    .on_click(|_, _, cx| {
                        Store::entity(cx).update(cx, |store, cx| store.restart(cx));
                    })
            }),
    }
}
