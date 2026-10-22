//! One model in the grid: what it is, how heavy, and the one action that
//! fits its state — download, cancel, retry, switch to it, or "in use".

use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{div, prelude::*, px, AnyElement, Context, Div, FontWeight, Stateful};

use super::VoiceTab;
use crate::api::CatalogModel;
use crate::ui::components::icon;
use crate::ui::icons::AppIcon;
use crate::ui::pages::models::{
    format_model_size, icon_button, is_streaming, progress_bar, weight_tier, TIER_LABELS,
};
use crate::ui::store::{Download, DownloadStatus, Store};
use crate::ui::theme::{alpha, Palette, R_LG, R_MD, R_XS};
use crate::ui::vendors::{vendor_mark, Vendor};

#[derive(Debug, Clone, Copy)]
pub(super) struct CardState {
    pub loaded: bool,
    pub installed: bool,
    pub recommended: bool,
}

/// Light to heavy: the four bars of the weight meter, by height in px.
const TIER_HEIGHTS: [f32; 4] = [4., 6., 8., 10.];

impl VoiceTab {
    pub(super) fn render_card(
        &self,
        index: usize,
        model: &CatalogModel,
        state: CardState,
        download: Option<&Download>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = *Palette::global(cx);
        let CardState { loaded, installed, recommended } = state;
        let vendor = Vendor::for_family(&model.family);

        div()
            .id(("model-card", index))
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(12.))
            .rounded(R_LG)
            .border_1()
            .map(|el| {
                if loaded {
                    el.border_color(p.accent).bg(p.accent_soft)
                } else {
                    el.border_color(p.border_soft)
                        .bg(p.panel)
                        .hover(|s| s.border_color(p.border).bg(p.surface_hover))
                }
            })
            .child(
                div()
                    .flex()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .size(px(32.))
                            .rounded(R_MD)
                            .border_1()
                            .map(|el| {
                                if loaded {
                                    el.border_color(p.accent).bg(p.accent_soft)
                                } else if installed {
                                    el.border_color(p.accent_soft).bg(p.accent_soft)
                                } else {
                                    el.border_color(p.border).bg(p.surface)
                                }
                            })
                            .child(match vendor {
                                Some(vendor) => vendor_mark(vendor, 18., p.fg, cx).into_any_element(),
                                None => icon(AppIcon::Cpu, 15., p.muted).into_any_element(),
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_start()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_size(px(12.5))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(if loaded { p.accent } else { p.fg })
                                            .child(model.display_name.clone()),
                                    )
                                    .when(recommended && !loaded, |el| {
                                        el.child(
                                            div()
                                                .flex_shrink_0()
                                                .px(px(6.))
                                                .py(px(1.))
                                                .rounded(R_XS)
                                                .border_1()
                                                .border_color(p.accent_soft)
                                                .bg(p.accent_soft)
                                                .text_size(px(9.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(p.accent)
                                                .child("BEST FIT"),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .mt(px(2.))
                                    .truncate()
                                    .text_size(px(11.))
                                    .text_color(p.muted)
                                    .child(model.description.clone()),
                            ),
                    ),
            )
            .child(self.render_card_facts(index, model, loaded, cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(28.))
                    .child(self.render_card_action(index, model, state, download, cx)),
            )
    }

    fn render_card_facts(
        &self,
        index: usize,
        model: &CatalogModel,
        loaded: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let p = *Palette::global(cx);
        let tier = weight_tier(model.size_bytes);
        let tone = if loaded { p.accent } else { p.fg_2 };
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .id(("weight", index))
                    .flex()
                    .items_end()
                    .gap(px(2.))
                    .tooltip(move |window, cx| {
                        Tooltip::new(format!("{} download", TIER_LABELS[tier])).build(window, cx)
                    })
                    .children(TIER_HEIGHTS.iter().enumerate().map(|(i, height)| {
                        div()
                            .w(px(3.))
                            .h(px(*height))
                            .rounded(px(1.))
                            .bg(if i <= tier { tone } else { p.border })
                    })),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(tone)
                    .child(format_model_size(model.size_bytes))
            )
            .child(
                div()
                    .ml_auto()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .when(is_streaming(model), |el| {
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
                                .child("STREAMING"),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap(px(4.))
                            .text_size(px(9.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.muted)
                            .child(icon(AppIcon::Globe, 9., p.muted))
                            .child(if model.multilingual { "MULTI" } else { "EN" }),
                    ),
            )
    }

    /// Fixed height, so a card does not resize as its state changes.
    fn render_card_action(
        &self,
        index: usize,
        model: &CatalogModel,
        state: CardState,
        download: Option<&Download>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = *Palette::global(cx);
        let id = model.id.clone();

        if let Some(download) = download.filter(|d| d.status != DownloadStatus::Failed) {
            let cancel = icon_button(("cancel-download", index), AppIcon::X, 20., 13., R_XS, cx)
                .on_click(move |_, _, cx| {
                    Store::entity(cx).update(cx, |store, cx| store.cancel_download(&id, cx));
                });
            let progress = if download.status == DownloadStatus::Queued {
                div()
                    .flex_1()
                    .text_size(px(11.))
                    .text_color(p.muted)
                    .child("Queued")
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .gap(px(8.))
                    .child(progress_bar(download.progress, 6., p.surface, p.accent))
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.accent)
                            .child(format!("{}%", download.progress)),
                    )
                    .into_any_element()
            };
            return div()
                .flex()
                .flex_1()
                .items_center()
                .gap(px(8.))
                .child(progress)
                .child(cancel)
                .into_any_element();
        }

        if state.loaded {
            return edge_button("in-use", p.accent, p.accent_soft)
                .text_color(p.accent)
                .child(icon(AppIcon::Check, 12., p.accent))
                .child("In use")
                .into_any_element();
        }

        let failed = download.is_some_and(|d| d.status == DownloadStatus::Failed);
        let (tone, edge) = if failed { (p.danger, p.danger) } else { (p.fg_2, p.border) };
        let installed = state.installed;
        let label = if installed {
            "Use this model"
        } else if failed {
            "Retry download"
        } else {
            "Download"
        };
        let mut el = edge_button(("card-action", index), edge, p.surface)
            .text_color(tone)
            .font_weight(FontWeight::MEDIUM)
            .when(!self.saving, |el| {
                el.cursor_pointer()
                    .hover(|s| {
                        s.bg(p.accent_soft)
                            .text_color(p.accent)
                            .border_color(alpha(p.accent, 0.45))
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if installed {
                            this.use_model(&id, window, cx);
                        } else {
                            Store::entity(cx).update(cx, |store, cx| store.start_download(&id, cx));
                        }
                    }))
            })
            .when(self.saving, |el| el.opacity(0.5).cursor_not_allowed())
            .when(!installed, |el| el.child(icon(AppIcon::Download, 12., tone)))
            .child(label);
        if let Some(error) = download.and_then(|d| d.error.clone()) {
            el = el.tooltip(move |window, cx| Tooltip::new(error.clone()).build(window, cx));
        }
        el.into_any_element()
    }
}

/// The full-width, 28px-high button the card's action row uses.
fn edge_button(
    id: impl Into<gpui_kit::ElementId>,
    edge: gpui_kit::Hsla,
    bg: gpui_kit::Hsla,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .h(px(28.))
        .w_full()
        .rounded(R_MD)
        .border_1()
        .border_color(edge)
        .bg(bg)
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
}
