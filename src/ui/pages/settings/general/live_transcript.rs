//! Expand the pill into a live transcript card as you speak.

use gpui_kit::{div, prelude::*, px, Context, FontWeight, Subscription, Window};

use crate::ui::components::{icon, switch};
use crate::ui::icons::AppIcon;
use crate::ui::settings::UiSettings;
use crate::ui::theme::{Palette, R_LG, R_MD};

pub struct LiveTranscriptToggle {
    /// Set by the first flip, so the switch doesn't slide in.
    flipped: bool,
    _settings_changed: Subscription,
}

impl LiveTranscriptToggle {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            flipped: false,
            _settings_changed: cx.observe_global::<UiSettings>(|_, cx| cx.notify()),
        }
    }
}

impl Render for LiveTranscriptToggle {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let on = UiSettings::global(cx).live_transcript;
        let this = cx.entity();
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .px(px(16.))
            .py(px(12.))
            .rounded(R_LG)
            .bg(p.panel)
            .border_1()
            .border_color(p.border_soft)
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .size(px(32.))
                            .rounded(R_MD)
                            .bg(p.accent_soft)
                            .child(icon(AppIcon::Captions, 14., p.accent)),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(p.fg_2)
                                    .child("Live transcript in the pill"),
                            )
                            .child(
                                div()
                                    .mt(px(3.))
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .child(
                                        "The pill expands into a card and fills in as you speak. \
                                         A large model on modest hardware may lag behind your voice.",
                                    ),
                            ),
                    ),
            )
            .child(switch(
                "live-transcript-switch",
                on,
                self.flipped,
                move |_, cx| {
                    this.update(cx, |this, _| this.flipped = true);
                    UiSettings::update(cx, |s| s.live_transcript = !s.live_transcript);
                },
                cx,
            ))
    }
}
