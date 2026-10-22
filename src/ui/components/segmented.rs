//! A segmented tab list: a surface track with the active segment tinted.

use gpui_kit::{
    div, prelude::*, px, transparent_black, App, ElementId, FontWeight, IntoElement, SharedString,
    Window,
};

use super::icon;
use crate::ui::icons::AppIcon;
use crate::ui::theme::{alpha, Palette, R_LG, R_MD};

pub struct Segment<T> {
    pub value: T,
    pub label: SharedString,
    pub icon: Option<AppIcon>,
}

pub fn segmented<T: Copy + PartialEq + 'static>(
    id: impl Into<ElementId>,
    segments: Vec<Segment<T>>,
    active: T,
    on_select: impl Fn(T, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let p = Palette::global(cx);
    let on_select = std::rc::Rc::new(on_select);
    div()
        .id(id.into())
        .flex()
        .items_center()
        .h(px(36.))
        .p(px(3.))
        .rounded(R_LG)
        .bg(p.surface)
        .border_1()
        .border_color(p.border_soft)
        .children(segments.into_iter().enumerate().map(|(i, segment)| {
            let on = segment.value == active;
            let value = segment.value;
            let on_select = on_select.clone();
            let color = if on { p.accent } else { p.muted };
            div()
                .id(ElementId::NamedInteger("segment".into(), i as u64))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(5.))
                .h_full()
                .px(px(8.))
                .py(px(4.))
                .rounded(R_MD)
                .border_1()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(color)
                .cursor_pointer()
                .map(|el| {
                    if on {
                        el.bg(p.accent_soft)
                            .border_color(alpha(p.accent, 0.35))
                            .shadow(p.shadow_sm())
                    } else {
                        el.border_color(transparent_black())
                            .hover(|s| s.text_color(p.fg_2))
                    }
                })
                .on_click(move |_, window, cx| on_select(value, window, cx))
                .when_some(segment.icon, |el, i| el.child(icon(i, 12., color)))
                .child(segment.label)
        }))
}
