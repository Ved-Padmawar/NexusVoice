//! The Model Manager: every model file on disk, each deletable.

use gpui_kit::component::{notification::Notification, WindowExt as _};
use gpui_kit::{div, prelude::*, px, Context, EventEmitter, FontWeight, Window};

use crate::api::{self, DownloadedModel};
use crate::ui::components::{icon, icon_button, modal};
use crate::ui::icons::AppIcon;
use crate::ui::pages::models::format_manager_size;
use crate::ui::runtime::SpawnApi as _;
use crate::ui::theme::{Palette, R_LG, R_MD, R_XS};

pub struct ManagerClosed;

pub struct ModelManager {
    models: Vec<DownloadedModel>,
    /// The variant being deleted; its button waits.
    deleting: Option<String>,
}

impl EventEmitter<ManagerClosed> for ModelManager {}

impl ModelManager {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::load(cx);
        Self {
            models: Vec::new(),
            deleting: None,
        }
    }

    fn load(cx: &mut Context<Self>) {
        cx.spawn_api(
            async { api::get_downloaded_models() },
            |this: &mut Self, models, cx| {
                this.models = models;
                cx.notify();
            },
        );
    }

    fn delete(
        &mut self,
        variant: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.deleting.is_some() {
            return;
        }
        self.deleting = Some(variant.clone());
        cx.notify();
        cx.spawn_api_in(
            window,
            async move { api::delete_model(variant).await },
            move |this: &mut Self, result, window, cx| {
                this.deleting = None;
                match result {
                    Ok(()) => {
                        window.push_notification(
                            Notification::success(format!("{name} deleted")),
                            cx,
                        );
                        Self::load(cx);
                    }
                    Err(e) => {
                        log::warn!("could not delete {name}: {e}");
                        window.push_notification(Notification::error("Failed to delete model"), cx);
                    }
                }
                cx.notify();
            },
        );
    }

    fn summary(&self) -> String {
        match self.models.len() {
            0 => "No models downloaded".to_string(),
            n => {
                let total = self.models.iter().map(|m| m.size_bytes).sum();
                let plural = if n > 1 { "s" } else { "" };
                format!("{n} model{plural} · {} on disk", format_manager_size(total))
            }
        }
    }

    fn render_row(
        &self,
        index: usize,
        model: &DownloadedModel,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let p = *Palette::global(cx);
        let busy = self.deleting.as_deref() == Some(model.variant.as_str());
        let (variant, name) = (model.variant.clone(), model.display_name.clone());
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(16.))
            .py(px(12.))
            .rounded(R_LG)
            .bg(p.surface)
            .border_1()
            .border_color(p.border_soft)
            .child(icon(AppIcon::HardDrive, 13., p.muted))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(p.fg)
                                    .child(model.display_name.clone()),
                            )
                            .when(model.is_active, |el| {
                                el.child(
                                    div()
                                        .px(px(5.))
                                        .py(px(1.))
                                        .rounded(R_XS)
                                        .border_1()
                                        .border_color(p.accent_soft)
                                        .bg(p.accent_soft)
                                        .text_size(px(9.))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(p.accent)
                                        .child("ACTIVE"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(p.muted)
                            .child(format_manager_size(model.size_bytes)),
                    ),
            )
            .child(
                div()
                    .id(("delete-model", index))
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .rounded(R_MD)
                    .text_color(p.muted)
                    .map(|el| {
                        if busy {
                            el.opacity(0.3).cursor_not_allowed()
                        } else {
                            el.cursor_pointer()
                                .hover(|s| s.bg(p.surface_hover).text_color(p.danger))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.delete(variant.clone(), name.clone(), window, cx);
                                }))
                        }
                    })
                    .child(icon(
                        AppIcon::Trash,
                        13.,
                        if busy { p.muted } else { p.fg_2 },
                    )),
            )
    }

    fn render_content(&self, cx: &Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let this = cx.entity();
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(24.))
                    .pt(px(24.))
                    .pb(px(16.))
                    .border_b_1()
                    .border_color(p.border_soft)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(
                                div()
                                    .flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .justify_center()
                                    .size(px(36.))
                                    .rounded(R_LG)
                                    .bg(p.accent_soft)
                                    .child(icon(AppIcon::Database, 15., p.accent)),
                            )
                            .child(
                                div()
                                    .child(
                                        div()
                                            .text_size(px(15.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(p.fg)
                                            .child("Model Manager"),
                                    )
                                    .child(
                                        div()
                                            .mt(px(2.))
                                            .text_size(px(11.))
                                            .text_color(p.muted)
                                            .child(self.summary()),
                                    ),
                            ),
                    )
                    .child(
                        icon_button("manager-close", AppIcon::X, 28., 14., R_MD, cx)
                            .on_click(move |_, _, cx| this.update(cx, |_, cx| cx.emit(ManagerClosed))),
                    ),
            )
            .child(div().flex().flex_col().gap(px(8.)).px(px(24.)).py(px(20.)).map(|el| {
                if self.models.is_empty() {
                    el.child(
                        div()
                            .py(px(24.))
                            .text_center()
                            .text_size(px(12.))
                            .text_color(p.muted)
                            .child("No models on disk."),
                    )
                } else {
                    el.children(self.models.iter().enumerate().map(|(i, m)| self.render_row(i, m, cx)))
                }
            }))
            .child(
                div()
                    .px(px(24.))
                    .pb(px(20.))
                    .text_size(px(10.))
                    .text_color(p.muted)
                    .child("Deleting the active model frees its memory; transcription will prompt you to pick a model again."),
            )
    }
}

impl Render for ModelManager {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let content = self.render_content(cx);
        modal(
            "model-manager",
            px(460.),
            content,
            Some(Box::new(move |_, cx| {
                this.update(cx, |_, cx| cx.emit(ManagerClosed));
            })),
            window,
            cx,
        )
    }
}
