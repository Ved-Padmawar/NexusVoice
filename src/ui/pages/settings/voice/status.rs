//! The readout above the grid: model in use, backend and disk usage.

use gpui_kit::{div, prelude::*, px, Context, FontWeight};

use super::VoiceTab;
use crate::api::CatalogModel;
use crate::ui::components::icon;
use crate::ui::icons::AppIcon;
use crate::ui::pages::models::{format_disk_total, is_streaming};
use crate::ui::theme::{Palette, R_MD, R_SM, R_XS};
use crate::ui::vendors::{vendor_mark, Vendor};

impl VoiceTab {
    pub(super) fn render_status(
        &self,
        loaded: Option<&CatalogModel>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let p = *Palette::global(cx);
        let vendor = loaded.and_then(|m| Vendor::for_family(&m.family));

        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(10.))
            .px(px(12.))
            .py(px(8.))
            .rounded(R_MD)
            .border_1()
            .map(|el| {
                if loaded.is_some() {
                    el.border_color(p.accent_soft).bg(p.accent_soft)
                } else {
                    el.border_color(p.border_soft).bg(p.panel)
                }
            })
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .rounded(R_SM)
                    .when(loaded.is_some(), |el| el.bg(p.accent_soft))
                    .child(match vendor {
                        Some(vendor) => vendor_mark(vendor, 16., p.fg, cx).into_any_element(),
                        None => icon(AppIcon::Mic, 13., p.muted).into_any_element(),
                    }),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(13.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(p.fg)
                    .child(
                        loaded
                            .map_or("No model loaded", |m| m.display_name.as_str())
                            .to_string(),
                    ),
            )
            .when(loaded.is_some_and(is_streaming), |el| {
                el.child(
                    div()
                        .flex()
                        .flex_shrink_0()
                        .items_center()
                        .gap(px(4.))
                        .px(px(6.))
                        .py(px(2.))
                        .rounded(R_XS)
                        .bg(p.accent_soft)
                        .text_size(px(9.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.accent)
                        .child(icon(AppIcon::Radio, 9., p.accent))
                        .child("LIVE"),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(10.))
                    .text_color(p.muted)
                    .child(
                        loaded
                            .map_or("Pick a model below to start transcribing.", |m| {
                                m.detail.as_str()
                            })
                            .to_string(),
                    ),
            )
            .child(self.render_hardware(&p))
            .child(Self::render_manage_button(&p, cx))
    }

    fn render_hardware(&self, p: &Palette) -> impl IntoElement {
        let disk_bytes: u64 = self.on_disk.iter().map(|m| m.size_bytes).sum();
        let backend = self.profile.as_ref().map_or_else(
            || "Detecting…".to_string(),
            |profile| {
                let vram = if profile.vram_gb > 0. {
                    format!(" · {} GB", profile.vram_gb)
                } else {
                    String::new()
                };
                format!("{}{vram}", profile.execution_provider.to_uppercase())
            },
        );
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(12.))
            .pl(px(12.))
            .border_l_1()
            .border_color(p.border_soft)
            .text_size(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_color(p.fg_2)
                    .child(icon(AppIcon::Cpu, 11., p.accent))
                    .child(backend),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_color(p.muted)
                    .child(icon(AppIcon::HardDrive, 11., p.muted))
                    .child(format!(
                        "{} on disk · {}",
                        self.on_disk.len(),
                        format_disk_total(disk_bytes)
                    )),
            )
    }

    fn render_manage_button(p: &Palette, cx: &Context<Self>) -> impl IntoElement {
        div()
            .id("manage-models")
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(6.))
            .px(px(8.))
            .py(px(4.))
            .rounded(R_SM)
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .cursor_pointer()
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.fg_2)
            .hover(|s| s.bg(p.surface_hover).text_color(p.fg))
            .on_click(cx.listener(|this, _, window, cx| this.open_manager(window, cx)))
            .child(icon(AppIcon::Database, 11., p.fg_2))
            .child("Manage")
    }
}
