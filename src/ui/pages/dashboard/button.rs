//! The accent button the pages share ("Set hotkey", "Add to Dictionary").

use gpui_kit::{div, prelude::*, px, App, Div, ElementId, FontWeight, Stateful, Window};

use crate::ui::theme::{Palette, R_MD};

#[derive(Clone, Copy)]
pub enum ButtonSize {
    /// 32px tall.
    Small,
    /// 36px tall.
    Default,
}

/// A filled accent button; add its label and icon as children. A disabled one
/// is dimmed and ignores clicks.
pub fn primary_button(
    id: impl Into<ElementId>,
    size: ButtonSize,
    disabled: bool,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
    cx: &App,
) -> Stateful<Div> {
    let p = Palette::global(cx);
    let (height, pad) = match size {
        ButtonSize::Small => (px(32.), px(12.)),
        ButtonSize::Default => (px(36.), px(16.)),
    };
    let (bg, hover_bg) = (p.accent, p.accent_hover);
    div()
        .id(id)
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .gap(px(if matches!(size, ButtonSize::Small) { 6. } else { 8. }))
        .h(height)
        .px(pad)
        .rounded(R_MD)
        .bg(bg)
        .text_color(p.accent_fg)
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .map(|el| {
            if disabled {
                el.opacity(0.5)
            } else {
                el.cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .on_click(move |_, window, cx| on_click(window, cx))
            }
        })
}
