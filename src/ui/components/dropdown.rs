//! The dropdown every select uses: a trigger and a menu with optional search
//! and keyboard navigation.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::{
    anchored, canvas, deferred, div, prelude::*, px, App, Bounds, Context, ElementId, Entity,
    EventEmitter, FocusHandle, Focusable, KeyDownEvent, MouseDownEvent, Pixels, ScrollHandle,
    SharedString, Subscription, Window,
};

use super::{icon, popover_open, popover_panel};
use crate::ui::icons::AppIcon;
use crate::ui::theme::{Palette, R_LG, R_MD, R_SM};

const ITEM_H: Pixels = px(36.);
const MAX_MENU_H: Pixels = px(288.);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropdownItem {
    pub value: SharedString,
    pub label: SharedString,
    /// Draw a rule under this item, to set it apart from those below.
    pub divider_after: bool,
    /// Shown whatever the search says, e.g. an "Auto-detect" choice.
    pub pinned: bool,
}

impl DropdownItem {
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            divider_after: false,
            pinned: false,
        }
    }

    #[must_use]
    pub fn divider_after(mut self) -> Self {
        self.divider_after = true;
        self
    }

    #[must_use]
    pub fn pinned(mut self) -> Self {
        self.pinned = true;
        self
    }
}

#[derive(Debug, Clone)]
pub enum DropdownEvent {
    Changed(SharedString),
}

pub struct Dropdown {
    id: SharedString,
    items: Vec<DropdownItem>,
    selected: Option<SharedString>,
    leading_icon: Option<AppIcon>,
    placeholder: SharedString,
    /// Shown when the search matches nothing.
    empty_text: SharedString,
    disabled: bool,
    open: bool,
    highlighted: usize,
    search: Option<Entity<InputState>>,
    focus: FocusHandle,
    scroll: ScrollHandle,
    /// The trigger's last painted bounds: the menu's width, and not an outside click.
    trigger: Rc<Cell<Bounds<Pixels>>>,
    _search_changed: Option<Subscription>,
}

impl EventEmitter<DropdownEvent> for Dropdown {}

impl Focusable for Dropdown {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Dropdown {
    pub fn new(id: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            selected: None,
            leading_icon: None,
            placeholder: SharedString::default(),
            empty_text: "No matches.".into(),
            disabled: false,
            open: false,
            highlighted: 0,
            search: None,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            trigger: Rc::default(),
            _search_changed: None,
        }
    }

    #[must_use]
    pub fn leading_icon(mut self, icon: AppIcon) -> Self {
        self.leading_icon = Some(icon);
        self
    }

    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Add a search field filtering by label and value.
    #[must_use]
    pub fn searchable(
        self,
        placeholder: impl Into<SharedString>,
        empty_text: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let search_changed = cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.highlighted = 0;
                cx.notify();
            }
        });
        Self {
            search: Some(search),
            empty_text: empty_text.into(),
            _search_changed: Some(search_changed),
            ..self
        }
    }

    pub fn set_items(&mut self, items: Vec<DropdownItem>, cx: &mut Context<Self>) {
        self.items = items;
        cx.notify();
    }

    pub fn set_selected(&mut self, value: Option<SharedString>, cx: &mut Context<Self>) {
        self.selected = value;
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        if disabled {
            self.open = false;
        }
        cx.notify();
    }

    fn query(&self, cx: &App) -> String {
        self.search
            .as_ref()
            .map(|s| s.read(cx).value().trim().to_lowercase())
            .unwrap_or_default()
    }

    fn visible(&self, cx: &App) -> Vec<usize> {
        let query = self.query(cx);
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                query.is_empty()
                    || item.pinned
                    || item.label.to_lowercase().contains(&query)
                    || item.value.to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.open == open {
            return;
        }
        self.open = open;
        if open {
            let visible = self.visible(cx);
            self.highlighted = visible
                .iter()
                .position(|&i| Some(&self.items[i].value) == self.selected.as_ref())
                .unwrap_or(0);
            self.scroll.scroll_to_item(self.highlighted);
            match &self.search {
                Some(search) => search.update(cx, |s, cx| s.focus(window, cx)),
                None => window.focus(&self.focus, cx),
            }
        } else {
            if let Some(search) = &self.search {
                search.update(cx, |s, cx| s.set_value("", window, cx));
            }
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    fn pick(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.items[index].value.clone();
        self.set_open(false, window, cx);
        if self.selected.as_ref() != Some(&value) {
            self.selected = Some(value.clone());
            cx.emit(DropdownEvent::Changed(value));
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        if !self.open {
            if matches!(key, "enter" | "space" | "down") && !self.disabled {
                self.set_open(true, window, cx);
                cx.stop_propagation();
            }
            return;
        }
        let visible = self.visible(cx);
        match key {
            "escape" => self.set_open(false, window, cx),
            "down" if !visible.is_empty() => {
                self.highlighted = (self.highlighted + 1).min(visible.len() - 1);
                self.scroll.scroll_to_item(self.highlighted);
            }
            "up" => {
                self.highlighted = self.highlighted.saturating_sub(1);
                self.scroll.scroll_to_item(self.highlighted);
            }
            "enter" => {
                if let Some(&index) = visible.get(self.highlighted) {
                    self.pick(index, window, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn render_row(
        &self,
        row: usize,
        index: usize,
        last: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = *Palette::global(cx);
        let item = &self.items[index];
        let active = self.selected.as_ref() == Some(&item.value);
        div()
            .id(ElementId::NamedInteger("item".into(), index as u64))
            .flex()
            .flex_shrink_0()
            .items_center()
            .h(ITEM_H)
            .px(px(14.))
            .text_size(px(12.))
            .text_color(p.fg)
            .cursor_pointer()
            .when(row == self.highlighted, |el| el.bg(p.surface))
            .when(last, |el| el.rounded_b(R_LG))
            .when(item.divider_after, |el| {
                el.border_b_1().border_color(p.border_soft)
            })
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                if this.highlighted != row {
                    this.highlighted = row;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, window, cx| this.pick(index, window, cx)))
            .child(
                div()
                    .flex_1()
                    .truncate()
                    .when(active, |el| {
                        el.font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .text_color(p.accent)
                    })
                    .child(item.label.clone()),
            )
            .when(active, |el| {
                el.child(div().ml(px(8.)).child(icon(AppIcon::Check, 13., p.accent)))
            })
    }

    fn render_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let visible = self.visible(cx);
        // Sized to the unfiltered list, so typing doesn't resize the panel.
        let search_h = if self.search.is_some() {
            px(49.)
        } else {
            px(0.)
        };
        #[allow(clippy::cast_precision_loss)] // a menu holds tens of items, not 2^23
        let height = (ITEM_H * self.items.len() as f32 + search_h + px(2.)).min(MAX_MENU_H);
        let last = visible.len().saturating_sub(1);

        let rows = visible
            .iter()
            .enumerate()
            .map(|(row, &index)| self.render_row(row, index, row == last, cx))
            .collect::<Vec<_>>();

        let panel = popover_panel("dropdown-menu", p.shadow_lg(), cx)
            .flex()
            .flex_col()
            .h(height)
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                if !this.trigger.get().contains(&event.position) {
                    this.set_open(false, window, cx);
                }
            }))
            .when_some(self.search.clone(), |el, search| {
                el.child(
                    div()
                        .flex_shrink_0()
                        .p(px(8.))
                        .border_b_1()
                        .border_color(p.border_soft)
                        .child(
                            Input::new(&search)
                                .h(px(32.))
                                .rounded(R_SM)
                                .text_size(px(12.))
                                .prefix(icon(AppIcon::Search, 13., p.fg_2)),
                        ),
                )
            })
            .child(
                div()
                    .id("dropdown-list")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .when(visible.is_empty(), |el| {
                        el.child(
                            div()
                                .px(px(14.))
                                .py(px(12.))
                                .text_size(px(12.))
                                .text_color(p.muted)
                                .child(self.empty_text.clone()),
                        )
                    })
                    .children(rows),
            );
        popover_open(panel, "dropdown-open", px(4.))
    }
}

impl Render for Dropdown {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let label = self
            .selected
            .as_ref()
            .and_then(|v| self.items.iter().find(|i| &i.value == v))
            .map_or_else(|| self.placeholder.clone(), |i| i.label.clone());
        let open = self.open;
        let disabled = self.disabled;

        div()
            .id(ElementId::Name(self.id.clone()))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .relative()
            .w_full()
            .child(
                div()
                    .id("dropdown-trigger")
                    .relative()
                    .flex()
                    .items_center()
                    .w_full()
                    .h(px(36.))
                    .pl(if self.leading_icon.is_some() {
                        px(32.)
                    } else {
                        px(12.)
                    })
                    .pr(px(32.))
                    .rounded(R_MD)
                    .bg(p.surface)
                    .border_1()
                    .border_color(if open { p.accent } else { p.border_soft })
                    .text_size(px(12.))
                    .text_color(p.fg)
                    .when(disabled, |el| el.opacity(0.5))
                    .when(!disabled, |el| {
                        el.cursor_pointer()
                            .when(!open, |el| el.hover(|s| s.border_color(p.border)))
                            .on_click(cx.listener(|this, _, window, cx| {
                                let open = !this.open;
                                this.set_open(open, window, cx);
                            }))
                    })
                    .when_some(self.leading_icon, |el, leading| {
                        el.child(
                            div()
                                .absolute()
                                .left(px(10.))
                                .top_0()
                                .bottom_0()
                                .flex()
                                .items_center()
                                .child(icon(leading, 14., p.accent)),
                        )
                    })
                    .child(div().flex_1().truncate().child(label))
                    .child({
                        let trigger = self.trigger.clone();
                        canvas(move |bounds, _, _| trigger.set(bounds), |_, (), _, _| {})
                            .absolute()
                            .size_full()
                    })
                    .child(
                        div()
                            .absolute()
                            .right(px(10.))
                            .top_0()
                            .bottom_0()
                            .flex()
                            .items_center()
                            .child(
                                icon(AppIcon::ChevronDown, 14., p.fg_2).when(open, |i| {
                                    i.rotate(gpui_kit::radians(std::f32::consts::PI))
                                }),
                            ),
                    ),
            )
            .when(open, |el| {
                let width = self.trigger.get().size.width;
                el.child(
                    deferred(
                        anchored()
                            .snap_to_window_with_margin(px(8.))
                            .child(div().w(width).child(self.render_menu(cx))),
                    )
                    .with_priority(1),
                )
            })
    }
}
