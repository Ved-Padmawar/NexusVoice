//! Smart formatting: sends each transcript to the user's LLM to tidy
//! punctuation and lists. Off by default; turning it on without an endpoint
//! opens the provider modal.

mod modal;
mod presets;

use gpui_kit::component::{notification::Notification, Icon, WindowExt as _};
use gpui_kit::{
    div, prelude::*, px, Context, Div, Entity, FontWeight, Stateful, Subscription, Window,
};

use self::modal::{ProviderModal, ProviderModalEvent};
use self::presets::{is_configured, label};
use super::widgets::{over_window, switch};
use crate::api;
use crate::llm::config::FormatConfig;
use crate::ui::components::icon;
use crate::ui::icons::AppIcon;
use crate::ui::runtime::SpawnApi as _;
use crate::ui::theme::{Palette, R_LG, R_MD};
use crate::ui::vendors::{vendor_mark, Vendor};

pub struct FormattingToggle {
    config: FormatConfig,
    /// The open modal, with the subscription that hears it close or save.
    modal: Option<(Entity<ProviderModal>, Subscription)>,
    /// Set by the first flip, so the switch does not slide when the page opens.
    flipped: bool,
}

impl FormattingToggle {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.spawn_api(async { api::get_format_config() }, |this, config, cx| {
            this.config = config;
            cx.notify();
        });
        Self {
            config: FormatConfig::default(),
            modal: None,
            flipped: false,
        }
    }

    fn configured(&self) -> bool {
        let profile = self.config.active();
        is_configured(&self.config.provider, &profile.base_url, &profile.model)
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flipped = true;
        let enabled = !self.config.enabled;
        self.config.enabled = enabled;
        if enabled && !self.configured() {
            // On while the endpoint is set up; closing unsaved puts it back.
            self.open_modal(window, cx);
        } else {
            self.persist(window, cx);
        }
        cx.notify();
    }

    fn persist(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let config = self.config.clone();
        cx.spawn_api_in(
            window,
            async move { api::set_format_config(&config) },
            |_, result, window, cx| {
                if result.is_err() {
                    window.push_notification(
                        Notification::error("Failed to save formatting settings"),
                        cx,
                    );
                }
            },
        );
    }

    fn open_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let initial = self.config.clone();
        let modal = cx.new(|cx| ProviderModal::new(initial, window, cx));
        let events = cx.subscribe(&modal, |this, _, event, cx| {
            match event {
                ProviderModalEvent::Saved(config) => this.config = config.clone(),
                ProviderModalEvent::Closed => {
                    if !this.configured() {
                        this.config.enabled = false;
                    }
                }
            }
            this.modal = None;
            cx.notify();
        });
        self.modal = Some((modal, events));
    }

    fn summary(&self, p: &Palette, cx: &gpui_kit::App) -> impl IntoElement {
        let line = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(p.fg_2);
        if !self.configured() {
            return line.child("No endpoint configured");
        }
        let provider = &self.config.provider;
        let model = self.config.active().model;
        line.children(Vendor::from_id(provider).map(|vendor| vendor_mark(vendor, 14., p.fg_2, cx)))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .child(format!("{} · {model}", label(provider))),
            )
    }
}

fn configure_button(p: &Palette) -> Stateful<Div> {
    div()
        .id("configure-formatting")
        .flex()
        .items_center()
        .gap(px(6.))
        .px(px(10.))
        .py(px(6.))
        .rounded(R_MD)
        .bg(p.surface)
        .border_1()
        .border_color(p.border)
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.fg_2)
        .cursor_pointer()
        .hover(|s| s.bg(p.surface_hover).text_color(p.fg))
        .child(Icon::new(AppIcon::Settings2).size(px(12.)))
        .child("Configure")
}

impl Render for FormattingToggle {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let this = cx.entity();
        div()
            .overflow_hidden()
            .rounded(R_LG)
            .bg(p.panel)
            .border_1()
            .border_color(p.border_soft)
            .child(
                div()
                    .px_4()
                    .py(px(10.))
                    .border_b_1()
                    .border_color(p.border_soft)
                    .text_size(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.muted)
                    .child("SMART FORMATTING"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .p_4()
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap_3()
                            .child(
                                div()
                                    .mt(px(1.))
                                    .flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .justify_center()
                                    .size_8()
                                    .rounded(R_MD)
                                    .bg(p.accent_soft)
                                    .child(icon(AppIcon::Sparkles, 14., p.accent)),
                            )
                            .child(
                                div()
                                    .child(self.summary(&p, cx))
                                    .child(
                                        div()
                                            .mt(px(3.))
                                            .max_w(px(380.))
                                            .text_size(px(11.))
                                            .text_color(p.muted)
                                            .child(
                                                "Sends each transcript to your chosen LLM (local Ollama, \
                                                 OpenAI, Anthropic, OpenRouter…) to clean up punctuation \
                                                 and turn spoken lists into real lists before pasting.",
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap_2()
                            .child(
                                configure_button(&p)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_modal(window, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(switch(
                                "formatting-switch",
                                self.config.enabled,
                                self.flipped,
                                move |window, cx| {
                                    this.update(cx, |this, cx| this.toggle(window, cx));
                                },
                                cx,
                            )),
                    ),
            )
            .children(self.modal.as_ref().map(|(modal, _)| over_window(modal.clone(), window)))
    }
}
