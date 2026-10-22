//! The pill theme: the pill in miniature on a backdrop, and a swatch per theme.

use gpui_kit::{div, prelude::*, px, Context, FontWeight, Subscription, Window};

use super::card::{
    capsule, choice_grid, group_label, mic, over, section_card, stage, strip, Choice,
};
use crate::ui::icons::AppIcon;
use crate::ui::pill::theme::{def, PillThemeDef};
use crate::ui::settings::{PillTheme, UiSettings};
use crate::ui::theme::Palette;

pub struct PillSection {
    _settings_changed: Subscription,
}

impl PillSection {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            _settings_changed: cx.observe_global::<UiSettings>(|_, cx| cx.notify()),
        }
    }
}

impl Render for PillSection {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let current = UiSettings::global(cx).pill_theme;
        let current_def = def(current);
        let choices = PillTheme::ALL.map(|theme| {
            let t = def(theme);
            // Translucent values shown on the pill's own ground, as they really appear.
            let lead = strip(
                [t.bg, t.border, t.brand, t.accent].map(|color| over(color, t.bg)),
                13.,
                &p,
            );
            Choice {
                id: ("pill-theme", theme as u64).into(),
                active: theme == current,
                lead,
                title: t.label,
                hint: None,
            }
            .render(&p, move |_, cx| {
                UiSettings::update(cx, |s| s.pill_theme = theme);
            })
        });
        let choices = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(group_label(AppIcon::Radio, "RECORDING PILL", &p))
            .child(choice_grid().children(choices));
        section_card(
            stage(current_def, &p, mini_pill(current_def)),
            current_def.label,
            "shown while you dictate",
            choices,
            &p,
        )
    }
}

fn mini_pill(t: &PillThemeDef) -> impl IntoElement {
    capsule(t, 22.)
        .gap(px(4.))
        .pl(px(6.))
        .pr(px(7.))
        .child(mic(t, 7.))
        .child(
            div()
                .whitespace_nowrap()
                .text_size(px(8.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.brand)
                .child("NexusVoice"),
        )
}
