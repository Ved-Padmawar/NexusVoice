//! A segmented tab list: a surface track with the active segment raised.

use gpui_kit::{
    div, prelude::*, px, App, ElementId, FontWeight, IntoElement, SharedString, Window,
};

use super::icon;
use crate::ui::icons::AppIcon;
use crate::ui::theme::{Palette, R_LG, R_MD};

pub struct Segment<T> {
    pub value: T,
    pub label: SharedString,
    pub icon: Option<AppIcon>,
}

/// One segment per value; clicking one calls `on_select` with its value.
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
        .gap(px(2.))
        .p(px(3.))
        .rounded(R_LG)
        .bg(p.surface)
        .border_1()
        .border_color(p.border_soft)
        .children(segments.into_iter().enumerate().map(|(i, segment)| {
            let on = segment.value == active;
            let value = segment.value;
            let on_select = on_select.clone();
            div()
                .id(ElementId::NamedInteger("segment".into(), i as u64))
                .flex()
                .items_center()
                .gap(px(5.))
                .px(px(14.))
                .py(px(5.))
                .rounded(R_MD)
                .text_size(px(12.))
                .cursor_pointer()
                .map(|el| {
                    if on {
                        el.bg(p.panel)
                            .text_color(p.fg)
                            .font_weight(FontWeight::SEMIBOLD)
                            .shadow(p.shadow_sm())
                    } else {
                        el.text_color(p.muted)
                            .font_weight(FontWeight::MEDIUM)
                            .hover(|s| s.text_color(p.fg))
                    }
                })
                .on_click(move |_, window, cx| on_select(value, window, cx))
                .when_some(segment.icon, |el, i| {
                    el.child(icon(i, 12., if on { p.fg } else { p.muted }))
                })
                .child(segment.label)
        }))
}
