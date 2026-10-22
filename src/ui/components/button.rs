//! The button every page uses, and the icon-only button.

use std::time::Duration;

use gpui_kit::{
    div, prelude::*, px, App, ClickEvent, Div, ElementId, FontWeight, Pixels, SharedString,
    Stateful, StyleRefinement, Window,
};

use super::{icon, spinner};
use crate::ui::icons::AppIcon;
use crate::ui::theme::{alpha, Palette, R_MD};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Outline,
    /// An outline that turns red on hover: for removing things.
    OutlineDanger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSize {
    Default,
    Sm,
}

type ClickHandler = Box<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    kind: ButtonKind,
    size: ButtonSize,
    icon: Option<AppIcon>,
    icon_size: Option<f32>,
    disabled: bool,
    /// Working: shows a spinner in place of the icon and ignores clicks.
    busy: bool,
    on_click: Option<ClickHandler>,
    style: StyleRefinement,
}

pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button {
        id: id.into(),
        label: label.into(),
        kind: ButtonKind::Primary,
        size: ButtonSize::Default,
        icon: None,
        icon_size: None,
        disabled: false,
        busy: false,
        on_click: None,
        style: StyleRefinement::default(),
    }
}

impl Button {
    #[must_use]
    pub fn kind(mut self, kind: ButtonKind) -> Self {
        self.kind = kind;
        self
    }

    #[must_use]
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    #[must_use]
    pub fn icon(mut self, icon: AppIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    #[must_use]
    pub fn icon_size(mut self, size: f32) -> Self {
        self.icon_size = Some(size);
        self
    }

    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    #[must_use]
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    #[must_use]
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = Palette::global(cx);
        let (height, pad, gap, icon_px) = match (self.size, self.icon.is_some()) {
            (ButtonSize::Default, false) => (36., 16., 8., 14.),
            (ButtonSize::Default, true) => (36., 12., 8., 14.),
            (ButtonSize::Sm, false) => (32., 12., 6., 11.),
            (ButtonSize::Sm, true) => (32., 10., 6., 11.),
        };
        let icon_px = self.icon_size.unwrap_or(icon_px);
        let enabled = !self.disabled && !self.busy;
        let text = match self.kind {
            ButtonKind::Primary => p.accent_fg,
            ButtonKind::Outline | ButtonKind::OutlineDanger => p.fg,
        };
        let mut el = div()
            .id(self.id)
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .gap(px(gap))
            .h(px(height))
            .px(px(pad))
            .rounded(R_MD)
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .whitespace_nowrap()
            .text_color(text)
            .map(|el| match self.kind {
                ButtonKind::Primary => el.bg(p.accent),
                ButtonKind::Outline | ButtonKind::OutlineDanger => {
                    el.bg(p.bg).border_1().border_color(p.border)
                }
            })
            .when(enabled, |el| match self.kind {
                ButtonKind::Primary => el.hover(|s| s.bg(p.accent_hover)),
                ButtonKind::Outline => el.hover(|s| s.bg(p.surface_hover)),
                ButtonKind::OutlineDanger => el.hover(|s| {
                    s.bg(p.danger_soft)
                        .border_color(p.danger)
                        .text_color(p.danger)
                }),
            })
            .map(|el| match (self.busy, self.icon) {
                (true, _) => el.child(spinner(
                    "button-spinner",
                    icon_px,
                    2.,
                    alpha(text, 0.3),
                    text,
                    Duration::from_millis(800),
                )),
                (false, Some(name)) => el.child(icon(name, icon_px, text)),
                (false, None) => el,
            })
            .child(self.label);
        if !enabled {
            el = el.opacity(0.5).cursor_not_allowed();
        } else if let Some(handler) = self.on_click {
            el = el
                .cursor_pointer()
                .on_click(move |_: &ClickEvent, window, cx| handler(window, cx));
        }
        el.style().refine(&self.style);
        el
    }
}

/// A square icon-only button that reddens on hover.
pub fn icon_button(
    id: impl Into<ElementId>,
    name: AppIcon,
    side: f32,
    glyph: f32,
    radius: Pixels,
    cx: &App,
) -> Stateful<Div> {
    let p = Palette::global(cx);
    div()
        .id(id.into())
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(side))
        .rounded(radius)
        .cursor_pointer()
        .text_color(p.muted)
        .hover(|s| s.text_color(p.danger).bg(alpha(p.danger, 0.12)))
        .child(icon(name, glyph, p.muted))
}
