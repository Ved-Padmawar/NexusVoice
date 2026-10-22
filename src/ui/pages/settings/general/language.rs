//! The dictation language. Support is per model, so the picker follows the
//! active model and hides itself for an English-only one.

use gpui_kit::component::{notification::Notification, WindowExt as _};
use gpui_kit::{div, prelude::*, Context, Entity, SharedString, Subscription, Window};

use super::widgets::section_heading;
use crate::api::{self, LanguageSettings};
use crate::ui::components::{Dropdown, DropdownEvent, DropdownItem};
use crate::ui::icons::AppIcon;
use crate::ui::runtime::SpawnApi as _;
use crate::ui::store::Store;

/// Matches `inference::language::AUTO`.
const AUTO: &str = "auto";

pub struct LanguageSection {
    dropdown: Entity<Dropdown>,
    supported: bool,
    /// The language the backend has, which a failed change goes back to.
    selected: SharedString,
    /// The model the options were fetched for.
    model_id: Option<String>,
    /// Newest fetch; an older answer arriving late is dropped.
    request: u64,
    _subscriptions: Vec<Subscription>,
}

impl LanguageSection {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dropdown = cx.new(|cx| {
            let mut dropdown = Dropdown::new("language", cx)
                .leading_icon(AppIcon::Languages)
                .placeholder("English")
                .searchable("Search languages", window, cx);
            dropdown.set_disabled(true, cx);
            dropdown
        });
        let subscriptions = vec![
            cx.subscribe_in(&dropdown, window, Self::on_dropdown),
            cx.observe_in(&Store::entity(cx), window, Self::on_store),
        ];
        let mut section = Self {
            dropdown,
            supported: false,
            selected: "en".into(),
            model_id: Self::active_model(cx),
            request: 0,
            _subscriptions: subscriptions,
        };
        section.fetch(window, cx);
        section
    }

    /// Whether the active model handles any language but English.
    pub fn supported(&self) -> bool {
        self.supported
    }

    fn active_model(cx: &Context<Self>) -> Option<String> {
        Store::entity(cx)
            .read(cx)
            .active_model()
            .map(|m| m.id.clone())
    }

    fn on_store(&mut self, _: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) {
        let model_id = Self::active_model(cx);
        if model_id != self.model_id {
            self.model_id = model_id;
            self.fetch(window, cx);
        }
    }

    fn fetch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request += 1;
        let request = self.request;
        self.dropdown.update(cx, |d, cx| d.set_disabled(true, cx));
        cx.spawn_api_in(
            window,
            async { api::get_language_options() },
            move |this, settings, _, cx| {
                if request == this.request {
                    this.show(&settings, cx);
                }
            },
        );
    }

    fn show(&mut self, settings: &LanguageSettings, cx: &mut Context<Self>) {
        self.supported = settings.supported;
        if let Some(active) = settings.options.iter().find(|o| o.is_selected) {
            self.selected = active.code.clone().into();
        }
        let items = settings
            .options
            .iter()
            .map(|o| {
                let item = DropdownItem::new(o.code.clone(), o.name.clone());
                if o.code == AUTO {
                    item.divider_after()
                } else {
                    item
                }
            })
            .collect();
        let selected = self.selected.clone();
        self.dropdown.update(cx, |dropdown, cx| {
            dropdown.set_items(items, cx);
            dropdown.set_selected(Some(selected), cx);
            dropdown.set_disabled(false, cx);
        });
        cx.notify();
    }

    fn on_dropdown(
        &mut self,
        _: &Entity<Dropdown>,
        event: &DropdownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let DropdownEvent::Changed(code) = event;
        let previous = std::mem::replace(&mut self.selected, code.clone());
        let code = code.to_string();
        cx.spawn_api_in(
            window,
            async move { api::set_language(Some(code)).await },
            move |this, result, window, cx| {
                let Err(e) = result else { return };
                this.selected = previous.clone();
                this.dropdown
                    .update(cx, |d, cx| d.set_selected(Some(previous), cx));
                window.push_notification(Notification::error(e.message().to_string()), cx);
            },
        );
    }
}

impl Render for LanguageSection {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().when(self.supported, |el| {
            el.flex()
                .flex_col()
                .gap_4()
                .child(section_heading(
                    "Language",
                    "The language you dictate in. Auto-detect can mix languages mid-sentence.",
                    cx,
                ))
                .child(self.dropdown.clone())
        })
    }
}
