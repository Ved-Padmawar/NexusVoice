//! The activity toolbar's buttons and the popover each opens below it.

use gpui_kit::{anchored, deferred, div, prelude::*, px, Anchor, App, IntoElement, Pixels, Window};

use crate::ui::components::{popover_open, popover_panel};
use crate::ui::icons::AppIcon;
use crate::ui::theme::{alpha, Palette, R_MD};
use gpui_kit::component::Icon;

/// The 28px toolbar button; `highlighted` marks an applied filter.
pub fn toolbar_button(
    id: &'static str,
    glyph: AppIcon,
    label: &'static str,
    highlighted: bool,
    cx: &App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let p = Palette::global(cx);
    let (fg, hover_fg, border, bg) = if highlighted {
        (p.accent, p.accent, alpha(p.accent, 0.6), p.accent_soft)
    } else {
        (p.fg_2, p.fg, p.border, p.panel)
    };
    div()
        .id(id)
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap(px(5.))
        .h(px(28.))
        .px(px(10.))
        .rounded(R_MD)
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_size(px(11.))
        .font_weight(gpui_kit::FontWeight::MEDIUM)
        .text_color(fg)
        .cursor_pointer()
        .hover(move |s| s.text_color(hover_fg).border_color(alpha(p.accent, 0.6)))
        .child(Icon::new(glyph).size(px(11.)))
        .child(label)
}

/// `content` right-aligned below a `relative` trigger, `gap` away; `on_dismiss`
/// runs on an outside click.
pub fn popover_below(
    id: &'static str,
    content: impl IntoElement,
    gap: Pixels,
    shadow_lg: bool,
    on_dismiss: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    let p = Palette::global(cx);
    let shadow = if shadow_lg {
        p.shadow_lg()
    } else {
        p.shadow_md()
    };
    let panel = popover_open(
        popover_panel(id, shadow, cx)
            .on_mouse_down_out(move |_, window, cx| on_dismiss(window, cx))
            .child(content),
        id,
        gap,
    );

    div()
        .absolute()
        .right_0()
        .bottom_0()
        .size_0()
        .child(deferred(anchored().anchor(Anchor::TopRight).child(panel)).with_priority(1))
}
