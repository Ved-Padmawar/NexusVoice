//! Choosing which input device records.

use std::time::Duration;

use gpui_kit::component::{notification::Notification, WindowExt as _};
use gpui_kit::{div, prelude::*, px, Context, Entity, SharedString, Subscription, Window};

use super::widgets::{section_heading, spinning_icon};
use crate::api::{self, InputDeviceInfo};
use crate::ui::components::{icon, Dropdown, DropdownEvent, DropdownItem};
use crate::ui::icons::AppIcon;
use crate::ui::runtime::SpawnApi as _;
use crate::ui::theme::{Palette, R_MD};

/// The "Default" option's value: no saved preference, follow the OS.
const DEFAULT_VALUE: &str = "__default__";

/// Long enough that the refresh icon visibly turns even when the device list
/// comes back from a cache.
const MIN_REFRESH: Duration = Duration::from_millis(500);

pub struct MicrophoneSection {
    dropdown: Entity<Dropdown>,
    /// The device the backend has, which a failed change goes back to.
    selected: SharedString,
    loading: bool,
    refreshing: bool,
    _dropdown_changed: Subscription,
}

impl MicrophoneSection {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dropdown = cx.new(|cx| {
            let mut dropdown = Dropdown::new("microphone", cx)
                .leading_icon(AppIcon::Mic)
                .placeholder("Default");
            dropdown.set_disabled(true, cx);
            dropdown
        });
        let subscription = cx.subscribe_in(&dropdown, window, Self::on_dropdown);
        let mut section = Self {
            dropdown,
            selected: DEFAULT_VALUE.into(),
            loading: true,
            refreshing: false,
            _dropdown_changed: subscription,
        };
        section.fetch(false, window, cx);
        section
    }

    fn on_dropdown(
        &mut self,
        _: &Entity<Dropdown>,
        event: &DropdownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let DropdownEvent::Changed(value) = event;
        let previous = std::mem::replace(&mut self.selected, value.clone());
        let name = (value != DEFAULT_VALUE).then(|| value.to_string());
        cx.spawn_api_in(
            window,
            async move { api::set_input_device(name.as_deref()) },
            move |this, result, window, cx| {
                let Err(e) = result else { return };
                this.selected = previous.clone();
                this.dropdown
                    .update(cx, |d, cx| d.set_selected(Some(previous), cx));
                window.push_notification(Notification::error(e.message().to_string()), cx);
            },
        );
    }

    /// List the devices. A refresh keeps the controls usable and only spins
    /// the icon.
    fn fetch(&mut self, refresh: bool, window: &mut Window, cx: &mut Context<Self>) {
        if refresh {
            self.refreshing = true;
            cx.notify();
        }
        cx.spawn_api_in(
            window,
            async move {
                let started = std::time::Instant::now();
                let devices = api::list_input_devices();
                if refresh {
                    tokio::time::sleep(MIN_REFRESH.saturating_sub(started.elapsed())).await;
                }
                devices
            },
            |this, devices, _, cx| {
                this.loading = false;
                this.refreshing = false;
                this.show(&devices, cx);
                cx.notify();
            },
        );
    }

    fn show(&mut self, devices: &[InputDeviceInfo], cx: &mut Context<Self>) {
        self.selected = selected_value(devices);
        let items = options(devices)
            .into_iter()
            .map(|(value, label)| DropdownItem::new(value, label))
            .collect();
        let selected = self.selected.clone();
        self.dropdown.update(cx, |dropdown, cx| {
            dropdown.set_items(items, cx);
            dropdown.set_selected(Some(selected), cx);
            dropdown.set_disabled(false, cx);
        });
    }

    fn refresh_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let color = if self.refreshing { p.fg } else { p.fg_2 };
        div()
            .id("refresh-devices")
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .size(px(36.))
            .rounded(R_MD)
            .bg(p.surface)
            .border_1()
            .border_color(p.border_soft)
            .text_color(p.fg_2)
            .when(self.loading, |el| el.opacity(0.5))
            .when(!self.loading, |el| {
                el.cursor_pointer()
                    .hover(|s| s.border_color(p.border).text_color(p.fg))
                    .on_click(cx.listener(|this, _, window, cx| {
                        if !this.refreshing {
                            this.fetch(true, window, cx);
                        }
                    }))
            })
            .child(if self.refreshing {
                spinning_icon(
                    "refresh-devices-spin",
                    AppIcon::RefreshCw,
                    13.,
                    color,
                    Duration::from_millis(700),
                )
                .into_any_element()
            } else {
                icon(AppIcon::RefreshCw, 13., color).into_any_element()
            })
    }
}

/// The "Default" option for the OS default device, then every other named
/// device once. The default device is only listed as that sentinel, so it never
/// appears twice.
fn options(devices: &[InputDeviceInfo]) -> Vec<(SharedString, SharedString)> {
    let default_label = devices.iter().find(|d| d.is_default).map_or_else(
        || "Default".to_string(),
        |d| format!("Default — {}", d.name),
    );
    let mut options = vec![(DEFAULT_VALUE.into(), default_label.into())];
    for device in devices {
        let listed = options.iter().any(|(value, _)| *value == device.name);
        if !device.is_default && !device.name.starts_with("Default") && !listed {
            options.push((device.name.clone().into(), device.name.clone().into()));
        }
    }
    options
}

fn selected_value(devices: &[InputDeviceInfo]) -> SharedString {
    devices
        .iter()
        .find(|d| d.is_selected && !d.is_default)
        .map_or_else(|| DEFAULT_VALUE.into(), |d| d.name.clone().into())
}

impl Render for MicrophoneSection {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(section_heading(
                "Microphone",
                "Choose which input device records your voice.",
                cx,
            ))
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().min_w_0().child(self.dropdown.clone()))
                    .child(self.refresh_button(cx)),
            )
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/ui/settings/microphone.rs"]
mod tests;
