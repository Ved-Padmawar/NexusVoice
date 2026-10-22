//! Settings: four tabs under one header. A tab is built the first time it is
//! opened and kept, so it keeps its scroll position and what was typed in it.

mod about;
mod appearance;
mod general;
mod voice;

pub use about::AboutTab;
pub use appearance::AppearanceTab;
pub use general::GeneralTab;
pub use voice::VoiceTab;

use gpui_kit::{
    div, prelude::*, px, AnyView, Context, Entity, FontWeight, Subscription, Window,
};

use crate::api;
use crate::ui::components::{icon, page_header, segmented, Segment};
use crate::ui::icons::AppIcon;
use crate::ui::settings::{SettingsTab, UiSettings};
use crate::ui::theme::{Palette, R_MD};
use gpui_kit::component::{notification::Notification, WindowExt as _};

pub struct Settings {
    general: Option<Entity<GeneralTab>>,
    voice: Option<Entity<VoiceTab>>,
    appearance: Option<Entity<AppearanceTab>>,
    about: Option<Entity<AboutTab>>,
    _settings_changed: Subscription,
}

impl Settings {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            general: None,
            voice: None,
            appearance: None,
            about: None,
            _settings_changed: cx.observe_global::<UiSettings>(|_, cx| cx.notify()),
        }
    }

    fn tab_view(&mut self, tab: SettingsTab, window: &mut Window, cx: &mut Context<Self>) -> AnyView {
        match tab {
            SettingsTab::General => self
                .general
                .get_or_insert_with(|| cx.new(|cx| GeneralTab::new(window, cx)))
                .clone()
                .into(),
            SettingsTab::Voice => self
                .voice
                .get_or_insert_with(|| cx.new(|cx| VoiceTab::new(window, cx)))
                .clone()
                .into(),
            SettingsTab::Appearance => self
                .appearance
                .get_or_insert_with(|| cx.new(|cx| AppearanceTab::new(window, cx)))
                .clone()
                .into(),
            SettingsTab::About => self
                .about
                .get_or_insert_with(|| cx.new(|cx| AboutTab::new(window, cx)))
                .clone()
                .into(),
        }
    }
}

fn subtitle(tab: SettingsTab) -> &'static str {
    match tab {
        SettingsTab::General => "Input device, dictation language, formatting and hotkeys",
        SettingsTab::Voice => "Transcription model",
        SettingsTab::Appearance => "Themes for the window and the recording pill",
        SettingsTab::About => "Version, updates and system information",
    }
}

fn tab_label(tab: SettingsTab) -> (&'static str, AppIcon) {
    match tab {
        SettingsTab::General => ("General", AppIcon::SlidersHorizontal),
        SettingsTab::Voice => ("Voice", AppIcon::Mic),
        SettingsTab::Appearance => ("Appearance", AppIcon::Palette),
        SettingsTab::About => ("About", AppIcon::Info),
    }
}

/// How far apart a tab's blocks sit.
fn tab_gap(tab: SettingsTab) -> f32 {
    match tab {
        SettingsTab::General => 20.,
        SettingsTab::Voice | SettingsTab::About => 12.,
        SettingsTab::Appearance => 24.,
    }
}

fn open_logs(window: &mut Window, cx: &mut gpui_kit::App) {
    if let Err(e) = api::open_logs_folder() {
        window.push_notification(Notification::error(e.message().to_string()), cx);
    }
}

fn logs_button(p: Palette) -> impl IntoElement {
    div()
        .id("open-logs")
        .flex()
        .items_center()
        .gap(px(5.))
        .h(px(36.))
        .px(px(10.))
        .mt(px(3.))
        .self_start()
        .rounded(R_MD)
        .bg(p.surface)
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.fg_2)
        .cursor_pointer()
        .hover(|s| s.text_color(p.fg))
        .on_click(|_, window, cx| open_logs(window, cx))
        .child(icon(AppIcon::FolderOpen, 12., p.fg_2))
        .child("Logs")
}

impl Render for Settings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let tab = UiSettings::global(cx).settings_tab;
        let view = self.tab_view(tab, window, cx);

        let segments = SettingsTab::ALL
            .into_iter()
            .map(|tab| {
                let (label, icon) = tab_label(tab);
                Segment { value: tab, label: label.into(), icon: Some(icon) }
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .px_8()
            .pt(px(28.))
            .pb_4()
            .child(page_header(AppIcon::Settings, "Settings", subtitle(tab)))
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .mb_3()
                    .child(segmented(
                        "settings-tabs",
                        segments,
                        tab,
                        |tab, _, cx| UiSettings::update(cx, |s| s.settings_tab = tab),
                        cx,
                    ))
                    .when(tab == SettingsTab::About, |el| el.child(logs_button(p))),
            )
            .child(
                div()
                    .id(("settings-tab", tab as u64))
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pr_1()
                    .gap(px(tab_gap(tab)))
                    .child(view),
            )
    }
}
