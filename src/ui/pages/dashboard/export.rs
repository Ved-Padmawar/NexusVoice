//! The "Export" button: pick a format and where to save.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use chrono::Local;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::WindowExt as _;
use gpui_kit::{div, prelude::*, px, Context, ElementId, Window};

use super::format::{count_noun, export_content, ExportFormat};
use super::toolbar::{popover_below, toolbar_button};
use crate::api::{self, dto::TranscriptResponse};
use crate::ui::icons::AppIcon;
use crate::ui::runtime::SpawnApi as _;
use crate::ui::theme::Palette;

#[derive(Default)]
pub struct ExportMenu {
    open: bool,
    exporting: bool,
    /// Lets the outside-click dismissal leave the button's own click to toggle.
    trigger_hovered: Rc<Cell<bool>>,
}

enum Saved {
    Cancelled,
    To,
    Failed(String),
}

impl ExportMenu {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self::default()
    }

    fn export(&mut self, format: ExportFormat, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        self.exporting = true;
        cx.notify();
        cx.spawn_api_in(
            window,
            async { api::export_transcripts().await },
            move |this, result, window, cx| match result {
                Ok(items) => this.save(format, &items, window, cx),
                Err(e) => this.finish(Saved::Failed(e.to_string()), "", window, cx),
            },
        );
    }

    fn save(
        &mut self,
        format: ExportFormat,
        items: &[TranscriptResponse],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content = match export_content(format, items, &Local) {
            Ok(content) => content,
            Err(e) => return self.finish(Saved::Failed(e.to_string()), "", window, cx),
        };
        let name = format.file_name(Local::now().date_naive());
        let dir = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_default();
        let picked = cx.prompt_for_new_path(&dir, Some(&name));
        let summary = format!(
            "Exported {} as {}",
            count_noun(items.len(), "transcript"),
            format.short_name()
        );
        cx.spawn_in(window, async move |this, cx| {
            let outcome = match picked.await {
                Ok(Ok(Some(path))) => write_file(cx, path, content).await,
                Ok(Ok(None)) | Err(_) => Saved::Cancelled,
                Ok(Err(e)) => Saved::Failed(e.to_string()),
            };
            let _ = this.update_in(cx, |this, window, cx| {
                this.finish(outcome, &summary, window, cx);
            });
        })
        .detach();
    }

    fn finish(
        &mut self,
        outcome: Saved,
        summary: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.exporting = false;
        cx.notify();
        match outcome {
            Saved::Cancelled => {}
            Saved::To => window.push_notification(Notification::success(summary.to_string()), cx),
            Saved::Failed(message) => window.push_notification(Notification::error(message), cx),
        }
    }
}

async fn write_file(cx: &gpui_kit::AsyncWindowContext, path: PathBuf, content: String) -> Saved {
    let written = cx
        .background_executor()
        .spawn(async move { std::fs::write(path, content) })
        .await;
    match written {
        Ok(()) => Saved::To,
        Err(e) => Saved::Failed(format!("Export failed: {e}")),
    }
}

impl Render for ExportMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let this = cx.entity();
        let exporting = self.exporting;
        let hovered = self.trigger_hovered.clone();
        let trigger_hovered = self.trigger_hovered.clone();

        let items = ExportFormat::ALL.map(|format| {
            div()
                .id(ElementId::Name(format.extension().into()))
                .px(px(12.))
                .py(px(7.))
                .text_size(px(12.))
                .text_color(p.fg_2)
                .cursor_pointer()
                .hover(|s| s.bg(p.surface_hover).text_color(p.fg))
                .on_click(cx.listener(move |this, _, window, cx| this.export(format, window, cx)))
                .child(format.menu_label())
        });

        div()
            .relative()
            .child(
                toolbar_button("export", AppIcon::Download, "Export", false, cx)
                    .when(exporting, |el| el.opacity(0.5))
                    .when(!exporting, |el| {
                        el.on_click(cx.listener(|this, _, _, cx| {
                            this.open = !this.open;
                            cx.notify();
                        }))
                    })
                    .on_hover(move |hover, _, _| hovered.set(*hover))
                    .when(!self.open, |el| {
                        el.tooltip(|window, cx| {
                            Tooltip::new("Export transcripts").build(window, cx)
                        })
                    }),
            )
            .when(self.open, |el| {
                el.child(popover_below(
                    "export-menu",
                    div().flex().flex_col().min_w(px(148.)).children(items),
                    px(4.),
                    false,
                    move |_, cx| {
                        if trigger_hovered.get() {
                            return;
                        }
                        this.update(cx, |this, cx| {
                            this.open = false;
                            cx.notify();
                        });
                    },
                    cx,
                ))
            })
    }
}
