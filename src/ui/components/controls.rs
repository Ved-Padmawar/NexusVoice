//! Small shared controls: the toggle chip, the progress bar and status tones.

use gpui_kit::{
    div, prelude::*, px, relative, App, Div, ElementId, FontWeight, Hsla, SharedString, Stateful,
};

use crate::ui::theme::{alpha, Palette};

/// A rounded toggle chip. The caller adds the click handler (never when disabled).
pub fn pill(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on: bool,
    disabled: bool,
    cx: &App,
) -> Stateful<Div> {
    let p = Palette::global(cx);
    div()
        .id(id.into())
        .flex()
        .items_center()
        .gap(px(6.))
        .px(px(10.))
        .py(px(4.))
        .rounded_full()
        .border_1()
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .map(|el| {
            if on {
                el.border_color(alpha(p.accent, 0.55))
                    .bg(p.accent_soft)
                    .text_color(p.accent)
            } else {
                el.border_color(p.border)
                    .text_color(p.fg_2)
                    .hover(|s| s.border_color(p.muted).text_color(p.fg))
            }
        })
        .map(|el| {
            if disabled {
                el.opacity(0.5)
            } else {
                el.cursor_pointer()
            }
        })
        .child(label.into())
}

pub fn progress_bar(percent: u8, height: f32, track: Hsla, fill: Hsla) -> Div {
    div()
        .h(px(height))
        .flex_1()
        .overflow_hidden()
        .rounded_full()
        .bg(track)
        .child(
            div()
                .h_full()
                .w(relative(f32::from(percent.min(100)) / 100.))
                .rounded_full()
                .bg(fill),
        )
}

/// The colour a status tile or dialog badge speaks in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Accent,
    Success,
    Danger,
}

impl Tone {
    pub fn badge(self, p: &Palette) -> (Hsla, Hsla) {
        match self {
            Self::Accent => (p.accent_soft, p.accent),
            Self::Success => (p.success_soft, p.success),
            Self::Danger => (p.danger_soft, p.danger),
        }
    }

    pub fn title(self, p: &Palette) -> Hsla {
        match self {
            Self::Accent => p.fg,
            Self::Success => p.success,
            Self::Danger => p.danger,
        }
    }
}
