//! The dashboard: usage totals and the transcript feed.

mod export;
mod feed;
mod filter;
mod format;
mod query;
mod row;
mod toolbar;

use std::time::Duration;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{div, prelude::*, px, Context, Entity, FontWeight, Subscription, Task, Window};

use crate::api::{self, UsageStatsResponse};
use crate::ui::components::{
    button, icon, page_header, skeleton, text_input, ButtonSize, LoadState, SectionState,
};
use crate::ui::icons::AppIcon;
use crate::ui::runtime::SpawnApi as _;
use crate::ui::settings::{Route, SettingsTab, UiSettings};
use crate::ui::shell::Hotkeys;
use crate::ui::store::{Store, StoreEvent};
use crate::ui::theme::{alpha, Palette, R_LG, R_MD, R_XL};
use export::ExportMenu;
use feed::{Feed, FeedEvent};
use filter::{FilterEvent, FilterPopover};
use format::{fmt_count, fmt_time};
use query::TranscriptFilters;

const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);

pub struct Dashboard {
    stats: Option<UsageStatsResponse>,
    stats_state: LoadState,
    filters: TranscriptFilters,
    search: Entity<InputState>,
    search_debounce: Option<Task<()>>,
    feed: Entity<Feed>,
    export: Entity<ExportMenu>,
    filter: Entity<FilterPopover>,
    _subscriptions: Vec<Subscription>,
}

impl Dashboard {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search transcripts…"));
        let feed = cx.new(|cx| Feed::new(window, cx));
        let export = cx.new(|cx| ExportMenu::new(window, cx));
        let filter = cx.new(|cx| FilterPopover::new(window, cx));

        let subscriptions = vec![
            cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.debounce_search(cx);
                }
            }),
            cx.subscribe(&filter, |this, _, FilterEvent::Applied(filters), cx| {
                this.filters = *filters;
                this.apply_query(cx);
            }),
            cx.subscribe(&feed, |_, _, FeedEvent::Deleted, cx| {
                Self::refresh_stats(cx);
            }),
            cx.subscribe(&Store::entity(cx), |_, _, event: &StoreEvent, cx| {
                if matches!(event, StoreEvent::TranscriptSaved(_)) {
                    Self::refresh_stats(cx);
                }
            }),
        ];

        let dashboard = Self {
            stats: None,
            stats_state: LoadState::Loading,
            filters: TranscriptFilters::default(),
            search,
            search_debounce: None,
            feed,
            export,
            filter,
            _subscriptions: subscriptions,
        };
        Self::refresh_stats(cx);
        dashboard
    }

    fn refresh_stats(cx: &mut Context<Self>) {
        cx.spawn_api(
            async { api::get_usage_stats().await },
            |this, result, cx| {
                match result {
                    Ok(stats) => {
                        this.stats = Some(stats);
                        this.stats_state = LoadState::Loaded;
                    }
                    Err(e) => this.stats_state = LoadState::Failed(e.to_string().into()),
                }
                cx.notify();
            },
        );
    }

    fn debounce_search(&mut self, cx: &mut Context<Self>) {
        self.search_debounce = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SEARCH_DEBOUNCE).await;
            let _ = this.update(cx, Self::apply_query);
        }));
    }

    fn apply_query(&mut self, cx: &mut Context<Self>) {
        let term = self.search.read(cx).value().to_string();
        let filters = self.filters;
        self.feed
            .update(cx, |feed, cx| feed.set_query(&term, filters, cx));
    }

    fn render_notice(cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(10.))
            .px(px(14.))
            .py(px(10.))
            .rounded(R_LG)
            .bg(p.warning_soft)
            .border_1()
            .border_color(alpha(p.warning, 0.25))
            .text_size(px(12.))
            .line_height(px(16.8))
            .text_color(p.fg_2)
            .child(icon(AppIcon::AlertCircle, 14., p.warning))
            .child(
                div()
                    .flex_1()
                    .child("No hotkey set — NexusVoice won't record until you configure one."),
            )
            .child(
                button("set-hotkey", "Set hotkey")
                    .size(ButtonSize::Sm)
                    .icon(AppIcon::Settings2)
                    .icon_size(12.)
                    .on_click(|_, cx| {
                        UiSettings::update(cx, |s| {
                            s.route = Route::Settings;
                            s.settings_tab = SettingsTab::General;
                        });
                    }),
            )
    }

    fn render_stats(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let stats = self.stats.unwrap_or_default();
        let tiles = [
            (AppIcon::Hash, "Total Words", fmt_count(stats.total_words)),
            (
                AppIcon::Timer,
                "Speaking Time",
                fmt_time(stats.speaking_time_seconds),
            ),
            (AppIcon::Mic, "Sessions", fmt_count(stats.total_sessions)),
            (
                AppIcon::Activity,
                "Avg Pace",
                stats.avg_pace_wpm.to_string(),
            ),
        ];
        let has_data = self.stats.is_some();
        let retry = cx.entity();
        SectionState::new(
            self.stats_state.clone(),
            has_data,
            stats_skeleton(cx),
            div()
                .flex()
                .gap(px(10.))
                .children(tiles.map(|(glyph, label, value)| {
                    div()
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .items_center()
                        .gap(px(14.))
                        .px(px(18.))
                        .py_4()
                        .rounded(R_XL)
                        .bg(p.panel)
                        .border_1()
                        .border_color(p.border)
                        .child(
                            div()
                                .flex()
                                .flex_shrink_0()
                                .items_center()
                                .justify_center()
                                .size(px(36.))
                                .rounded(R_MD)
                                .bg(p.accent_soft)
                                .child(icon(glyph, 15., p.accent)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(3.))
                                .min_w_0()
                                .child(
                                    div()
                                        .text_size(px(20.))
                                        .font_weight(FontWeight::BOLD)
                                        .line_height(px(20.))
                                        .text_color(p.fg)
                                        .child(value),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(p.muted)
                                        .child(label),
                                ),
                        )
                })),
            move |_, cx| retry.update(cx, |_, cx| Self::refresh_stats(cx)),
        )
    }

    fn render_activity_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let badge = self.feed.read(cx).badge_count();
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .mb_4()
            .pr_3()
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.fg_2)
                    .child("Recent activity"),
            )
            .when_some(badge, |el, count| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .min_w(px(20.))
                        .h(px(20.))
                        .px(px(6.))
                        .rounded_full()
                        .bg(p.accent_soft)
                        .text_color(p.accent)
                        .text_size(px(10.))
                        .font_weight(FontWeight::BOLD)
                        .child(count.to_string()),
                )
            })
            .child(
                div()
                    .ml_auto()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(self.export.clone())
                    .child(self.filter.clone())
                    .child(
                        div().w(px(180.)).child(
                            text_input(&self.search, cx)
                                .h(px(28.))
                                .text_size(px(12.))
                                .prefix(icon(AppIcon::Search, 12., p.muted)),
                        ),
                    ),
            )
    }
}

fn stats_skeleton(cx: &gpui_kit::App) -> impl IntoElement {
    let p = *Palette::global(cx);
    div().flex().gap(px(10.)).children((0..4).map(|i| {
        div()
            .flex()
            .flex_1()
            .items_center()
            .gap(px(14.))
            .px(px(18.))
            .py_4()
            .rounded(R_XL)
            .bg(p.panel)
            .border_1()
            .border_color(p.border)
            .child(skeleton(
                format!("stat-skeleton-icon-{i}"),
                |d| d.size(px(36.)).rounded(R_MD).flex_shrink_0(),
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(skeleton(
                        format!("stat-skeleton-value-{i}"),
                        |d| d.w(px(56.)).h(px(16.)),
                        cx,
                    ))
                    .child(skeleton(
                        format!("stat-skeleton-label-{i}"),
                        |d| d.w(px(64.)).h(px(10.)),
                        cx,
                    )),
            )
    }))
}

impl Render for Dashboard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let missing_hotkey = Hotkeys::bindings(cx).is_some_and(|b| b.push_to_talk.is_none());
        div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .px_8()
            .pt_7()
            .pb_4()
            .child(page_header(
                AppIcon::LayoutDashboard,
                "Dashboard",
                "Your voice, transcribed instantly.",
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap_5()
                    .mt(px(6.))
                    .when(missing_hotkey, |el| el.child(Self::render_notice(cx)))
                    .child(self.render_stats(cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h_0()
                            .overflow_hidden()
                            .child(self.render_activity_header(cx))
                            .child(self.feed.clone()),
                    ),
            )
    }
}
