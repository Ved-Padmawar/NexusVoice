//! The "Recent activity" list: a virtualized, keyset-paged feed of transcripts
//! or of search results, kept current as new ones are saved.

use std::time::Duration;

use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::WindowExt as _;
use gpui_kit::{
    div, list, prelude::*, px, AnyElement, App, ClipboardItem, Context, Entity, EventEmitter,
    FontWeight, ListAlignment, ListOffset, ListState, SharedString, Subscription, Task, Window,
};

use super::query::{Cursor, Loaded, TranscriptFilters};
use super::row::{transcript_row, RowPlace, RowState};
use crate::api::{self, dto::TranscriptResponse, ApiError, PageQuery};
use crate::ui::components::{icon, skeleton, spinner, LoadState, SectionState};
use crate::ui::icons::AppIcon;
use crate::ui::runtime::SpawnApi as _;
use crate::ui::store::{Store, StoreEvent};
use crate::ui::theme::{Palette, R_LG};

/// Rows rendered beyond the viewport, so scrolling doesn't pop them in.
const OVERDRAW: f32 = 600.;
const COPIED_FOR: Duration = Duration::from_secs(2);
const FADE_IN_FOR: Duration = Duration::from_millis(400);

pub enum FeedEvent {
    /// A row was deleted, so the usage totals changed.
    Deleted,
}

pub struct Feed {
    filters: TranscriptFilters,
    /// The trimmed search text; empty shows the plain feed.
    term: String,
    loaded: Option<Loaded>,
    status: LoadState,
    /// Bumped by every first-page request, so a slow older answer is dropped.
    request: u64,
    first_page_pending: bool,
    fetching_more: bool,
    /// Stops the end of the list retrying a failing page on every frame.
    more_failed: bool,
    list: ListState,
    copied: Option<(i64, Task<()>)>,
    fresh: Option<(i64, Task<()>)>,
    _store: Subscription,
}

impl EventEmitter<FeedEvent> for Feed {}

impl Feed {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = cx.subscribe(&Store::entity(cx), |this, _, event: &StoreEvent, cx| {
            if let StoreEvent::TranscriptSaved(transcript) = event {
                this.on_transcript_saved(transcript.clone(), cx);
            }
        });
        let mut feed = Self {
            filters: TranscriptFilters::default(),
            term: String::new(),
            loaded: None,
            status: LoadState::Loading,
            request: 0,
            first_page_pending: false,
            fetching_more: false,
            more_failed: false,
            list: ListState::new(0, ListAlignment::Top, px(OVERDRAW)),
            copied: None,
            fresh: None,
            _store: store,
        };
        feed.fetch_first_page(true, cx);
        feed
    }

    /// The count badge: rows of the plain feed loaded so far.
    pub fn badge_count(&self) -> Option<usize> {
        let loaded = self.loaded.as_ref()?;
        (self.term.is_empty() && !loaded.items.is_empty()).then_some(loaded.items.len())
    }

    /// Change what is listed; unchanged input does nothing. The old rows stay
    /// up until the new first page arrives.
    pub fn set_query(&mut self, term: &str, filters: TranscriptFilters, cx: &mut Context<Self>) {
        let term = term.trim();
        if self.term == term && self.filters == filters {
            return;
        }
        self.term = term.to_string();
        self.filters = filters;
        self.fetch_first_page(true, cx);
    }

    fn is_default_view(&self) -> bool {
        self.term.is_empty() && !self.filters.is_active()
    }

    fn row_count(&self) -> usize {
        self.loaded
            .as_ref()
            .map_or(0, |l| l.items.len() + usize::from(l.has_more()))
    }

    fn fetch_first_page(&mut self, to_top: bool, cx: &mut Context<Self>) {
        self.request += 1;
        let request = self.request;
        self.first_page_pending = true;
        self.fetching_more = false;
        self.more_failed = false;
        if self.loaded.is_none() {
            self.status = LoadState::Loading;
            cx.notify();
        }
        let call = page_call(self.term.clone(), self.filters.page_query(None));
        cx.spawn_api(call, move |this, result, cx| {
            if this.request == request {
                this.on_first_page(result, to_top, cx);
            }
        });
    }

    fn on_first_page(
        &mut self,
        result: Result<Vec<TranscriptResponse>, ApiError>,
        to_top: bool,
        cx: &mut Context<Self>,
    ) {
        self.first_page_pending = false;
        match result {
            Ok(rows) => {
                let loaded = Loaded::from_first_page(rows);
                self.status = LoadState::Loaded;
                self.list.reset(loaded.items.len() + usize::from(loaded.has_more()));
                if to_top {
                    self.list.scroll_to(ListOffset::default());
                }
                self.loaded = Some(loaded);
            }
            Err(e) => self.status = LoadState::Failed(e.to_string().into()),
        }
        cx.notify();
    }

    /// Fetch the page after the last loaded row, once the end of the list is in view.
    fn load_more(&mut self, window: &Window, cx: &mut Context<Self>) {
        let ready = self.status == LoadState::Loaded && !self.fetching_more && !self.more_failed;
        let Some(cursor) = self.loaded.as_ref().filter(|_| ready).and_then(|l| l.next_cursor()) else {
            return;
        };
        self.fetching_more = true;
        let request = self.request;
        let call = page_call(self.term.clone(), self.filters.page_query(Some(cursor)));
        cx.spawn_api_in(window, call, move |this, result, window, cx| {
            if this.request == request {
                this.on_next_page(result, window, cx);
            }
        });
    }

    fn on_next_page(
        &mut self,
        result: Result<Vec<TranscriptResponse>, ApiError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.fetching_more = false;
        match (result, self.loaded.as_mut()) {
            (Ok(rows), Some(loaded)) => {
                let (before, spinner_slot) = (loaded.items.len(), usize::from(loaded.has_more()));
                loaded.append_page(rows);
                let after = loaded.items.len() + usize::from(loaded.has_more());
                self.list.splice(before..before + spinner_slot, after - before);
            }
            (Err(e), _) => {
                self.more_failed = true;
                window.push_notification(Notification::error(e.to_string()), cx);
            }
            (Ok(_), None) => {}
        }
        cx.notify();
    }

    fn retry_more(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.more_failed = false;
        self.load_more(window, cx);
        cx.notify();
    }

    fn on_transcript_saved(&mut self, transcript: TranscriptResponse, cx: &mut Context<Self>) {
        let can_prepend = self.is_default_view() && !self.first_page_pending;
        let Some(loaded) = self.loaded.as_mut().filter(|_| can_prepend) else {
            // Other filters and searches are the backend's to evaluate.
            self.fetch_first_page(false, cx);
            return;
        };
        let id = transcript.id;
        if !loaded.prepend(transcript) {
            return;
        }
        let at_top = self.list.logical_scroll_top().item_ix == 0;
        self.list.splice(0..0, 1);
        if at_top {
            self.list.scroll_to(ListOffset::default());
        }
        self.fresh = Some((id, expire(cx, FADE_IN_FOR, |this| this.fresh = None)));
        cx.notify();
    }

    pub(super) fn copy(&mut self, id: i64, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.copied = Some((id, expire(cx, COPIED_FOR, |this| this.copied = None)));
        cx.notify();
    }

    pub(super) fn delete(&mut self, id: i64, window: &Window, cx: &mut Context<Self>) {
        cx.spawn_api_in(
            window,
            async move { api::delete_transcript(id).await },
            move |this, result, window, cx| match result {
                Ok(()) => this.on_deleted(id, cx),
                Err(e) => window.push_notification(Notification::error(e.to_string()), cx),
            },
        );
    }

    /// Drop the row in place rather than refetch every page.
    fn on_deleted(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Some(index) = self.loaded.as_mut().and_then(|l| l.remove(id)) {
            self.list.splice(index..index + 1, 0);
        }
        // A fetch that started before the delete may still bring the row back.
        if self.first_page_pending {
            self.fetch_first_page(false, cx);
        }
        cx.emit(FeedEvent::Deleted);
        cx.notify();
    }

    fn render_item(&self, ix: usize, this: &Entity<Self>, window: &mut Window, cx: &App) -> AnyElement {
        let Some(loaded) = &self.loaded else {
            return div().into_any_element();
        };
        let Some(item) = loaded.items.get(ix) else {
            return self.end_of_list(this, window, cx);
        };
        let place = RowPlace { first: ix == 0, last: ix + 1 == loaded.items.len() && !loaded.has_more() };
        let state = RowState {
            copied: self.copied.as_ref().is_some_and(|(id, _)| *id == item.id),
            fresh: self.fresh.as_ref().is_some_and(|(id, _)| *id == item.id),
        };
        transcript_row(item, place, &state, this, cx)
    }

    /// The row after the last: a spinner while the next page loads, which also
    /// starts the load as soon as it scrolls near, like the old scroll sentinel.
    fn end_of_list(&self, this: &Entity<Self>, window: &mut Window, cx: &App) -> AnyElement {
        let p = *Palette::global(cx);
        let body = if self.more_failed {
            let this = this.clone();
            div()
                .id("feed-retry-more")
                .text_size(px(12.))
                .text_color(p.muted)
                .cursor_pointer()
                .hover(|s| s.text_color(p.accent))
                .on_click(move |_, window, cx| {
                    this.update(cx, |feed, cx| feed.retry_more(window, cx));
                })
                .child("Couldn’t load more. Try again")
                .into_any_element()
        } else {
            let this = this.clone();
            window.defer(cx, move |window, cx| {
                this.update(cx, |feed, cx| feed.load_more(window, cx));
            });
            spinner(
                "feed-more-spinner",
                16.,
                2.,
                p.border,
                p.accent,
                Duration::from_millis(650),
            )
            .into_any_element()
        };
        div().flex().justify_center().py_4().child(body).into_any_element()
    }
}

/// Runs `then` on the feed after `after`; dropping the task cancels it.
fn expire(cx: &mut Context<Feed>, after: Duration, then: fn(&mut Feed)) -> Task<()> {
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(after).await;
        let _ = this.update(cx, |feed, cx| {
            then(feed);
            cx.notify();
        });
    })
}

async fn page_call(term: String, page: PageQuery) -> Result<Vec<TranscriptResponse>, ApiError> {
    if term.is_empty() {
        api::get_transcripts(page).await
    } else {
        api::search_transcripts(term, page).await
    }
}

impl Render for Feed {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_data = self.loaded.is_some() && !matches!(self.status, LoadState::Failed(_));
        let content = match &self.loaded {
            Some(loaded) if loaded.items.is_empty() => empty_state(!self.term.is_empty(), cx),
            Some(_) => self.timeline(cx),
            None => div().into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(SectionState::new(
                self.status.clone(),
                has_data,
                feed_skeleton(cx),
                content,
                cx.listener_for_retry(window),
            ))
    }
}
