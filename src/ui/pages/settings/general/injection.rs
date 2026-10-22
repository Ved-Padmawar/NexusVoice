//! Linux typing helper status: Wayland blocks synthetic keystrokes, so the
//! working helper is shown rather than failing silently. Empty elsewhere.

use std::time::Duration;

use gpui_kit::{div, prelude::*, px, Context, FontWeight, Window};

use crate::api::{self, dto::InjectionStatus};
use crate::ui::components::{icon, spinning_icon};
use crate::ui::icons::AppIcon;
use crate::ui::runtime::SpawnApi as _;
use crate::ui::theme::{Palette, R_LG, R_MD, R_SM, R_XS};

pub struct TextInjectionSection {
    status: Option<InjectionStatus>,
    checking: bool,
}

impl TextInjectionSection {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::load(cx);
        Self {
            status: None,
            checking: false,
        }
    }

    /// Probes spawn a process per tool, so this runs on demand, not on a timer.
    fn load(cx: &mut Context<Self>) {
        cx.spawn_api(
            async { api::get_injection_status().await },
            |this, status, cx| {
                this.status = Some(status);
                this.checking = false;
                cx.notify();
            },
        );
    }

    fn recheck(&mut self, cx: &mut Context<Self>) {
        if self.checking {
            return;
        }
        self.checking = true;
        cx.notify();
        Self::load(cx);
    }

    fn header(&self, status: &InjectionStatus, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let ready = status.selected.is_some();
        let summary = match &status.selected {
            Some(tool) => format!("Typing transcripts with {tool} on {}.", status.session),
            None => format!(
                "No supported tool found on {}. Install one below, or nothing will be typed.",
                status.session
            ),
        };
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .rounded(R_SM)
                    .when(ready, |el| el.bg(p.accent_soft))
                    .child(if ready {
                        icon(AppIcon::Keyboard, 14., p.accent)
                    } else {
                        icon(AppIcon::AlertTriangle, 14., p.danger)
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(p.fg)
                            .child("Text injection"),
                    )
                    .child(
                        div()
                            .mt(px(2.))
                            .text_size(px(11.))
                            .text_color(p.muted)
                            .child(summary),
                    ),
            )
            .child(self.recheck_button(cx))
    }

    fn recheck_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let checking = self.checking;
        div()
            .id("recheck-injection")
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(6.))
            .px(px(8.))
            .py(px(4.))
            .rounded(R_SM)
            .bg(p.surface)
            .border_1()
            .border_color(p.border)
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.fg_2)
            .when(checking, |el| el.opacity(0.5))
            .when(!checking, |el| {
                el.cursor_pointer()
                    .hover(|s| s.bg(p.surface_hover).text_color(p.fg))
                    .on_click(cx.listener(|this, _, _, cx| this.recheck(cx)))
            })
            .child(if checking {
                spinning_icon(
                    "recheck-injection-spin",
                    AppIcon::RefreshCw,
                    11.,
                    p.fg_2,
                    Duration::from_secs(1),
                )
                .into_any_element()
            } else {
                icon(AppIcon::RefreshCw, 11., p.fg_2).into_any_element()
            })
            .child("Recheck")
    }
}

fn tool_row(tool: &api::dto::InjectionTool, p: &Palette) -> impl IntoElement {
    let name_color = if tool.preferred { p.accent } else { p.fg_2 };
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(10.))
        .py(px(6.))
        .rounded(R_MD)
        .border_1()
        .when(tool.preferred, |el| {
            el.border_color(p.accent_soft).bg(p.accent_soft)
        })
        .when(!tool.preferred, |el| {
            el.border_color(p.border_soft).bg(p.surface)
        })
        .child(
            div()
                .flex_shrink_0()
                .size(px(12.))
                .when(tool.available, |el| {
                    el.child(icon(AppIcon::Check, 12., p.accent))
                }),
        )
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(11.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(name_color)
                .child(tool.name.clone()),
        )
        .when(tool.preferred, |el| {
            el.child(
                div()
                    .flex_shrink_0()
                    .px(px(6.))
                    .py(px(1.))
                    .rounded(R_XS)
                    .bg(p.accent_soft)
                    .text_size(px(9.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.accent)
                    .child("IN USE"),
            )
        })
        .when(!tool.available, |el| {
            el.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_right()
                    .text_size(px(10.5))
                    .text_color(p.muted)
                    .child(tool.install_hint.clone()),
            )
        })
}

impl Render for TextInjectionSection {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let status = self.status.as_ref().filter(|s| s.configurable);
        div().children(status.map(|status| {
            div()
                .p(px(16.))
                .rounded(R_LG)
                .bg(p.panel)
                .border_1()
                .border_color(p.border_soft)
                .child(self.header(status, cx))
                .child(
                    div()
                        .mt(px(12.))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .children(status.tools.iter().map(|tool| tool_row(tool, &p))),
                )
        }))
    }
}
