//! First run: pick a speech model, then download it. Shown over the window
//! until `UiSettings::model_chosen` is set; choosing a model that is already
//! on disk skips the download.

use gpui_kit::component::{notification::Notification, WindowExt as _};
use gpui_kit::{
    div, prelude::*, px, Context, Entity, FontWeight, Subscription, Window,
};

use crate::api::{self, CatalogModel, HardwareProfileResponse};
use crate::ui::components::{icon, modal};
use crate::ui::icons::AppIcon;
use crate::ui::pages::models::{
    button, format_model_size, is_streaming, model_name_to_id, progress_bar, sort_for_display,
};
use crate::ui::runtime::SpawnApi as _;
use crate::ui::settings::UiSettings;
use crate::ui::store::{DownloadStatus, Store};
use crate::ui::theme::{alpha, Palette, R_LG, R_MD, R_XS};

pub struct ModelPicker {
    profile: Option<HardwareProfileResponse>,
    profile_requested: bool,
    selected: Option<String>,
    /// The override is being saved.
    confirming: bool,
    /// The download is under way; the footer shows its progress.
    confirmed: bool,
    _store: Subscription,
}

impl ModelPicker {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::entity(cx);
        let subscription = cx.observe(&store, Self::on_store_changed);
        let mut picker = Self {
            profile: None,
            profile_requested: false,
            selected: None,
            confirming: false,
            confirmed: false,
            _store: subscription,
        };
        picker.on_store_changed(store, cx);
        picker
    }

    fn on_store_changed(&mut self, store: Entity<Store>, cx: &mut Context<Self>) {
        let store = store.read(cx);
        // A returning user whose choice was reset but whose model is on disk.
        let idle = !self.confirming && !self.confirmed;
        if idle && store.model_info.as_ref().is_some_and(|info| info.downloaded) {
            return Self::finish(cx);
        }
        if !self.profile_requested && !store.catalog.is_empty() {
            self.profile_requested = true;
            self.request_profile(cx);
        }
        if self.confirmed {
            self.follow_download(store.downloads.get(self.selected.as_deref().unwrap_or_default()), cx);
        }
        cx.notify();
    }

    fn request_profile(&self, cx: &mut Context<Self>) {
        cx.spawn_api(
            async { api::get_hardware_profile() },
            |this: &mut Self, profile, cx| {
                let catalog = &Store::entity(cx).read(cx).catalog;
                // A model picked by hand before the profile arrived stays.
                if this.selected.is_none() {
                    this.selected =
                        model_name_to_id(&profile.recommended_model, catalog).map(str::to_string);
                }
                this.profile = Some(profile);
                cx.notify();
            },
        );
    }

    /// The download entry is removed when it finishes, which is the close
    /// signal. An entry gone without the file is a failed start: back to the
    /// choice, so the user can try again.
    fn follow_download(&mut self, download: Option<&crate::ui::store::Download>, cx: &mut Context<Self>) {
        if download.is_some() {
            return;
        }
        if api::get_model_info().downloaded {
            Self::finish(cx);
        } else {
            self.confirmed = false;
            self.confirming = false;
        }
    }

    fn finish(cx: &mut Context<Self>) {
        UiSettings::update(cx, |s| s.model_chosen = true);
        Store::entity(cx).update(cx, |store, cx| store.offer_cuda(cx));
    }

    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else { return };
        self.confirming = true;
        cx.notify();
        let variant = id.clone();
        cx.spawn_api_in(
            window,
            async move {
                api::set_model_override(variant).await?;
                Ok::<_, api::ApiError>(api::get_model_info())
            },
            move |this: &mut Self, result, window, cx| match result {
                Ok(info) => {
                    Store::entity(cx).update(cx, |store, cx| store.refresh_models(cx));
                    // Already on disk: skip the download step.
                    if info.downloaded {
                        return Self::finish(cx);
                    }
                    Store::entity(cx).update(cx, |store, cx| store.start_download(&id, cx));
                    this.confirmed = true;
                    cx.notify();
                }
                Err(e) => {
                    this.confirming = false;
                    window.push_notification(Notification::error(e.message().to_string()), cx);
                    cx.notify();
                }
            },
        );
    }

    fn retry_download(&self, cx: &mut Context<Self>) {
        if let Some(id) = self.selected.clone() {
            Store::entity(cx).update(cx, |store, cx| store.start_download(&id, cx));
        }
    }

    fn render_header(&self, cx: &Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        div()
            .flex_shrink_0()
            .px(px(24.))
            .pt(px(20.))
            .pb(px(16.))
            .border_b_1()
            .border_color(p.border_soft)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .mb(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .size(px(32.))
                            .rounded(R_MD)
                            .bg(p.accent)
                            .child(icon(AppIcon::Zap, 14., p.accent_fg)),
                    )
                    .child(
                        div()
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(p.fg)
                                    .child("Choose your AI model"),
                            )
                            .child(
                                div()
                                    .mt(px(1.))
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .child("Select once — you can change this later in Settings."),
                            ),
                    ),
            )
            .children(self.profile.as_ref().map(|profile| {
                let vram = if profile.vram_gb > 0. {
                    format!(" · {} GB VRAM", profile.vram_gb)
                } else {
                    String::new()
                };
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .w_auto()
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(R_MD)
                    .bg(p.surface)
                    .border_1()
                    .border_color(p.border_soft)
                    .child(icon(AppIcon::Cpu, 10., p.muted))
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(p.fg_2)
                            .child(format!(
                                "{} · {}{vram}",
                                profile.gpu_name,
                                profile.execution_provider.to_uppercase()
                            )),
                    )
            }))
    }

    fn render_option(
        &self,
        index: usize,
        model: &CatalogModel,
        recommended: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = *Palette::global(cx);
        let active = self.selected.as_deref() == Some(model.id.as_str());
        let id = model.id.clone();
        let detail = model.detail.clone();
        div()
            .id(("picker-model", index))
            .flex()
            .items_start()
            .gap(px(10.))
            .px(px(12.))
            .py(px(8.))
            .rounded(R_LG)
            .border(px(1.5))
            .map(|el| {
                if active {
                    el.border_color(p.accent).bg(p.accent_soft)
                } else {
                    el.border_color(p.border)
                        .bg(p.surface)
                        .hover(|s| s.bg(p.surface_hover))
                }
            })
            .map(|el| {
                if self.confirmed {
                    el.cursor_not_allowed()
                } else {
                    el.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = Some(id.clone());
                        cx.notify();
                    }))
                }
            })
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(detail.clone()).build(window, cx)
            })
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .size(px(16.))
                    .mt(px(2.))
                    .rounded_full()
                    .border(px(1.5))
                    .map(|el| {
                        if active {
                            el.border_color(p.accent).bg(p.accent)
                        } else {
                            el.border_color(p.border)
                        }
                    })
                    .when(active, |el| el.child(icon(AppIcon::Check, 9., p.accent_fg))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if active { p.accent } else { p.fg })
                                    .child(model.display_name.clone()),
                            )
                            .when(recommended, |el| {
                                el.child(
                                    div()
                                        .flex_shrink_0()
                                        .px(px(4.))
                                        .py(px(1.))
                                        .rounded(R_XS)
                                        .border_1()
                                        .border_color(alpha(p.accent, 0.3))
                                        .bg(p.accent_soft)
                                        .text_size(px(8.5))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(p.accent)
                                        .child("BEST"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(10.5))
                            .text_color(p.muted)
                            .child(model.description.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .mt(px(4.))
                            .text_size(px(10.))
                            .text_color(p.muted)
                            .child(fact(AppIcon::HardDrive, format_model_size(model.size_bytes), p.muted))
                            .child(fact(
                                AppIcon::Globe,
                                if model.multilingual { "Multilingual" } else { "English" }.to_string(),
                                p.muted,
                            ))
                            .when(is_streaming(model), |el| {
                                el.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(4.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.accent)
                                        .child(icon(AppIcon::Radio, 10., p.accent))
                                        .child("Live"),
                                )
                            }),
                    ),
            )
    }

    fn render_footer(&self, store: &Store, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let this = cx.entity();
        let footer = div()
            .flex_shrink_0()
            .px(px(24.))
            .pb(px(20.))
            .pt(px(16.))
            .border_t_1()
            .border_color(p.border_soft);
        if !self.confirmed {
            return footer.child(self.render_confirm_row(store, cx));
        }
        let download = self.selected.as_ref().and_then(|id| store.downloads.get(id));
        if let Some(failed) = download.filter(|d| d.status == DownloadStatus::Failed) {
            return footer.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(11.))
                            .text_color(p.danger)
                            .child(failed.error.clone().unwrap_or_default()),
                    )
                    .child(
                        button("picker-retry", "Retry download")
                            .icon(AppIcon::RefreshCw)
                            .min_w(px(176.))
                            .on_click(move |_, cx| this.update(cx, |this, cx| this.retry_download(cx))),
                    ),
            );
        }
        let percent = download.map_or(100, |d| d.progress);
        footer.child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .mb(px(4.))
                        .text_size(px(11.))
                        .child(div().text_color(p.fg_2).child(if download.is_some() {
                            "Downloading model…"
                        } else {
                            "Download complete — loading…"
                        }))
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.accent)
                                .child(format!("{percent}%")),
                        ),
                )
                .child(div().flex().child(progress_bar(percent, 3., p.border, p.accent))),
        )
    }

    fn render_confirm_row(&self, store: &Store, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let this = cx.entity();
        // An empty catalog after the load finished: nothing to pick from.
        let load_failed = store.catalog.is_empty() && store.model_info.is_some();
        let selected = self
            .selected
            .as_ref()
            .and_then(|id| store.catalog.iter().find(|m| &m.id == id));
        let message = if load_failed {
            "Could not load the model catalog.".to_string()
        } else if let Some(model) = selected {
            format!("{} · {} download", model.display_name, format_model_size(model.size_bytes))
        } else {
            "Select a model to continue".to_string()
        };
        let action = if load_failed {
            button("picker-reload", "Try again")
                .icon(AppIcon::RefreshCw)
                .min_w(px(176.))
                .on_click(|_, cx| Store::entity(cx).update(cx, |store, cx| store.refresh_models(cx)))
        } else {
            button("picker-confirm", if self.confirming { "Starting download…" } else { "Download & continue" })
                .icon(AppIcon::Download)
                .min_w(px(176.))
                .busy(self.confirming)
                .disabled(self.selected.is_none())
                .on_click(move |window, cx| this.update(cx, |this, cx| this.confirm(window, cx)))
        };
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(11.))
                    .text_color(if load_failed { p.danger } else { p.muted })
                    .child(message),
            )
            .child(action)
    }
}

fn fact(name: AppIcon, text: String, color: gpui_kit::Hsla) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .child(icon(name, 10., color))
        .child(text)
}

impl Render for ModelPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = Store::entity(cx).read(cx);
        let recommended = self
            .profile
            .as_ref()
            .and_then(|profile| model_name_to_id(&profile.recommended_model, &store.catalog));
        let options: Vec<_> = sort_for_display(&store.catalog)
            .into_iter()
            .enumerate()
            .map(|(i, model)| self.render_option(i, model, recommended == Some(model.id.as_str()), cx))
            .collect();
        let width = px(660.).min(window.viewport_size().width * 0.92);

        let content = div()
            .flex()
            .flex_col()
            .min_h_0()
            .child(self.render_header(cx))
            .child(
                div()
                    .id("picker-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(24.))
                    .py(px(16.))
                    .child(div().grid().grid_cols(2).gap(px(8.)).children(options)),
            )
            .child(self.render_footer(store, cx));
        modal("model-picker", width, content, None, cx)
    }
}
