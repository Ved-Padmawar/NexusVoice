//! The model catalog: what is running, how to unload it, and a card per model
//! to download, cancel or switch to. Filters narrow the grid rather than
//! folding it.

mod card;
mod status;
mod unload;

use std::collections::HashSet;

use gpui_kit::component::input::InputEvent;
use gpui_kit::component::input::InputState;
use gpui_kit::component::{notification::Notification, WindowExt as _};
use gpui_kit::{
    div, prelude::*, px, AppContext as _, Context, Entity, Subscription, Window,
};

use crate::api::{self, DownloadedModel, HardwareProfileResponse};
use crate::ui::components::{icon, text_input};
use crate::ui::icons::AppIcon;
use crate::ui::pages::model_manager::{ManagerClosed, ModelManager};
use crate::ui::pages::models::{
    matches_query, model_name_to_id, pill, sort_for_display, ModelFilter,
};
use crate::ui::runtime::SpawnApi as _;
use crate::ui::store::Store;
use crate::ui::theme::Palette;
use unload::ModelUnloadSection;

struct OpenManager {
    view: Entity<ModelManager>,
    _closed: Subscription,
}

pub struct VoiceTab {
    profile: Option<HardwareProfileResponse>,
    on_disk: Vec<DownloadedModel>,
    /// A model switch is in flight; the cards wait for it.
    saving: bool,
    /// The model just chosen, shown as in use until the catalog catches up.
    chosen: Option<String>,
    filter: ModelFilter,
    query: Entity<InputState>,
    query_text: String,
    unload: Entity<ModelUnloadSection>,
    manager: Option<OpenManager>,
    /// The disk is re-read when this changes: a finished download adds a file.
    seen_downloads: usize,
    _subscriptions: Vec<Subscription>,
}

impl VoiceTab {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::entity(cx);
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Find a model…"));
        let subscriptions = vec![
            cx.observe(&store, Self::on_store_changed),
            cx.subscribe(&query, |this, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.query_text = state.read(cx).value().to_string();
                    cx.notify();
                }
            }),
        ];
        store.update(cx, |store, cx| {
            store.refresh_models(cx);
            store.refresh_downloads(cx);
        });

        let mut tab = Self {
            profile: None,
            on_disk: Vec::new(),
            saving: false,
            chosen: None,
            filter: ModelFilter::All,
            query,
            query_text: String::new(),
            unload: cx.new(|cx| ModelUnloadSection::new(window, cx)),
            manager: None,
            seen_downloads: store.read(cx).downloads.len(),
            _subscriptions: subscriptions,
        };
        tab.refresh_on_disk(cx);
        cx.spawn_api(
            async { api::get_hardware_profile() },
            |this: &mut Self, profile, cx| {
                this.profile = Some(profile);
                cx.notify();
            },
        );
        tab
    }

    fn refresh_on_disk(&mut self, cx: &mut Context<Self>) {
        cx.spawn_api(async { api::get_downloaded_models() }, |this: &mut Self, models, cx| {
            this.on_disk = models;
            cx.notify();
        });
    }

    fn on_store_changed(&mut self, store: Entity<Store>, cx: &mut Context<Self>) {
        let store = store.read(cx);
        let downloads = store.downloads.len();
        let caught_up = self
            .chosen
            .as_deref()
            .zip(store.active_model())
            .is_some_and(|(chosen, active)| chosen == active.id);
        if caught_up {
            self.chosen = None;
        }
        if downloads != self.seen_downloads {
            self.seen_downloads = downloads;
            self.refresh_on_disk(cx);
        }
        cx.notify();
    }

    /// Only offered for a model on disk, so the override can't point at nothing.
    fn use_model(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.chosen = Some(id.to_string());
        self.saving = true;
        cx.notify();
        let variant = id.to_string();
        cx.spawn_api_in(
            window,
            async move { api::set_model_override(variant).await },
            |this: &mut Self, result, window, cx| {
                this.saving = false;
                match result {
                    Ok(()) => {
                        Store::entity(cx).update(cx, |store, cx| store.refresh_models(cx));
                        window.push_notification(Notification::success("Model updated"), cx);
                    }
                    Err(e) => {
                        this.chosen = None;
                        window.push_notification(Notification::error(e.message().to_string()), cx);
                    }
                }
                cx.notify();
            },
        );
    }

    fn open_manager(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.new(|cx| ModelManager::new(window, cx));
        let closed = cx.subscribe(&view, |this, _, _: &ManagerClosed, cx| {
            this.manager = None;
            this.refresh_on_disk(cx);
            Store::entity(cx).update(cx, |store, cx| store.refresh_models(cx));
            cx.notify();
        });
        self.manager = Some(OpenManager {
            view,
            _closed: closed,
        });
    }

    fn set_filter(&mut self, filter: ModelFilter, cx: &mut Context<Self>) {
        if self.filter != filter {
            self.filter = filter;
            cx.notify();
        }
    }
}

impl Render for VoiceTab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let store = Store::entity(cx).read(cx);
        let ordered = sort_for_display(&store.catalog);
        let on_disk: HashSet<&str> = self.on_disk.iter().map(|m| m.variant.as_str()).collect();
        let recommended = self
            .profile
            .as_ref()
            .and_then(|profile| model_name_to_id(&profile.recommended_model, &store.catalog));
        // The active id is set even with nothing downloaded, so require the file too.
        let loaded = self
            .chosen
            .as_deref()
            .or_else(|| store.active_model().map(|m| m.id.as_str()))
            .filter(|id| on_disk.contains(id));

        let visible: Vec<_> = ordered
            .iter()
            .filter(|m| {
                self.filter.admits(m, on_disk.contains(m.id.as_str()))
                    && matches_query(m, &self.query_text)
            })
            .collect();
        let count = |filter: ModelFilter| {
            ordered
                .iter()
                .filter(|m| filter.admits(m, on_disk.contains(m.id.as_str())))
                .count()
        };
        let loaded_model = loaded.and_then(|id| store.catalog.iter().find(|m| m.id == id));

        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(self.render_status(loaded_model, cx))
            .child(self.unload.clone())
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap(px(8.))
                    .child(div().flex().items_center().gap(px(4.)).children(
                        ModelFilter::ALL.into_iter().enumerate().map(|(i, filter)| {
                            pill(("filter", i), filter.label(), self.filter == filter, false, cx)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.set_filter(filter, cx);
                                }))
                                .child(count(filter).to_string())
                        }),
                    ))
                    .child(
                        div()
                            .relative()
                            .flex_shrink_0()
                            .w(px(168.))
                            .child(
                                div()
                                    .absolute()
                                    .left(px(10.))
                                    .top_0()
                                    .bottom_0()
                                    .flex()
                                    .items_center()
                                    .child(icon(AppIcon::Search, 12., p.muted)),
                            )
                            .child(
                                text_input(&self.query, cx)
                                    .h(px(28.))
                                    .pl(px(28.))
                                    .pr(px(8.))
                                    .text_size(px(11.)),
                            ),
                    )
                    .child(
                        div()
                            .ml_auto()
                            .min_w_0()
                            .truncate()
                            .text_right()
                            .text_size(px(10.))
                            .text_color(p.muted)
                            .child("Bars show download weight — light to heavy."),
                    ),
            )
            .child(div().grid().grid_cols(3).gap(px(10.)).children(
                visible.iter().enumerate().map(|(i, model)| {
                    let state = card::CardState {
                        loaded: loaded == Some(model.id.as_str()),
                        installed: on_disk.contains(model.id.as_str()),
                        recommended: recommended == Some(model.id.as_str()),
                    };
                    self.render_card(i, model, state, store.downloads.get(&model.id), cx)
                }),
            ))
            .when(visible.is_empty(), |el| {
                el.child(
                    div()
                        .py(px(32.))
                        .text_center()
                        .text_size(px(12.))
                        .text_color(p.muted)
                        .child("No models match that filter."),
                )
            })
            .children(self.manager.as_ref().map(|open| open.view.clone()))
    }
}
