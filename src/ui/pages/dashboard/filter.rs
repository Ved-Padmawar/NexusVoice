//! The "Filter" button and its popover: a date range or one day, and the sort
//! order. Nothing applies until "Apply".

use std::cell::Cell;
use std::rc::Rc;

use chrono::NaiveDate;
use gpui_kit::component::calendar::Date;
use gpui_kit::component::date_picker::{DatePicker, DatePickerState};
use gpui_kit::component::{Sizable as _, Size};
use gpui_kit::{
    div, prelude::*, px, App, AppContext as _, Context, ElementId, Entity, EventEmitter,
    FontWeight, SharedString, Window,
};

use super::query::TranscriptFilters;
use super::toolbar::{popover_below, toolbar_button};
use crate::ui::icons::AppIcon;
use crate::ui::theme::{Palette, R_SM};

const DATE_FORMAT: &str = "%Y-%m-%d";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateMode {
    Range,
    On,
}

pub enum FilterEvent {
    Applied(TranscriptFilters),
}

pub struct FilterPopover {
    applied: TranscriptFilters,
    open: bool,
    mode: DateMode,
    sort_asc: bool,
    from: Entity<DatePickerState>,
    to: Entity<DatePickerState>,
    on: Entity<DatePickerState>,
    /// Lets the outside-click dismissal leave the button's own click to toggle.
    trigger_hovered: Rc<Cell<bool>>,
}

impl EventEmitter<FilterEvent> for FilterPopover {}

fn date_picker_state(window: &mut Window, cx: &mut Context<FilterPopover>) -> Entity<DatePickerState> {
    cx.new(|cx| DatePickerState::new(window, cx).date_format(DATE_FORMAT))
}

fn picked(state: &Entity<DatePickerState>, cx: &App) -> Option<NaiveDate> {
    match state.read(cx).date() {
        Date::Single(date) => date,
        Date::Range(start, _) => start,
    }
}

fn set_picked(
    state: &Entity<DatePickerState>,
    date: Option<NaiveDate>,
    window: &mut Window,
    cx: &mut Context<FilterPopover>,
) {
    state.update(cx, |state, cx| state.set_date(Date::Single(date), window, cx));
}

impl FilterPopover {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            applied: TranscriptFilters::default(),
            open: false,
            mode: DateMode::Range,
            sort_asc: false,
            from: date_picker_state(window, cx),
            to: date_picker_state(window, cx),
            on: date_picker_state(window, cx),
            trigger_hovered: Rc::default(),
        }
    }

    /// Opening shows what is applied, discarding edits that were never applied.
    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if open {
            let applied = self.applied;
            set_picked(&self.from, applied.from, window, cx);
            set_picked(&self.to, applied.to, window, cx);
            set_picked(&self.on, None, window, cx);
            self.sort_asc = applied.sort_asc;
        }
        self.open = open;
        cx.notify();
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        let sort_asc = self.sort_asc;
        let filters = match (self.mode, picked(&self.on, cx)) {
            (DateMode::On, Some(day)) => TranscriptFilters { from: Some(day), to: Some(day), sort_asc },
            _ => TranscriptFilters {
                from: picked(&self.from, cx),
                to: picked(&self.to, cx),
                sort_asc,
            },
        };
        self.commit(filters, cx);
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.commit(TranscriptFilters::default(), cx);
    }

    fn commit(&mut self, filters: TranscriptFilters, cx: &mut Context<Self>) {
        self.applied = filters;
        self.open = false;
        cx.emit(FilterEvent::Applied(filters));
        cx.notify();
    }

    fn choice<T: Copy + PartialEq + 'static>(
        &self,
        id: &'static str,
        options: [(T, &'static str); 2],
        current: T,
        set: fn(&mut Self, T),
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = *Palette::global(cx);
        div().flex().gap_1().children(options.map(|(value, label)| {
            let on = value == current;
            div()
                .id(ElementId::NamedInteger(id.into(), u64::from(label.len() as u32)))
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .h(px(26.))
                .rounded(R_SM)
                .border_1()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .map(|el| {
                    if on {
                        el.border_color(p.accent).bg(p.accent_soft).text_color(p.accent)
                    } else {
                        el.border_color(p.border)
                            .text_color(p.fg_2)
                            .hover(|s| s.text_color(p.fg))
                    }
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    set(this, value);
                    cx.notify();
                }))
                .child(label)
        }))
    }

    fn date_field(&self, label: Option<&'static str>, state: &Entity<DatePickerState>, cx: &App) -> impl IntoElement {
        let p = Palette::global(cx);
        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap(px(5.))
            .when_some(label, |el, label| el.child(section_label(label, p.muted)))
            .child(
                DatePicker::new(state)
                    .with_size(Size::Small)
                    .cleanable(true)
                    .placeholder(SharedString::from(DATE_FORMAT.replace('%', "")))
                    .text_size(px(11.)),
            )
    }

    fn form(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let mode = self.choice(
            "date-mode",
            [(DateMode::Range, "Range"), (DateMode::On, "Specific day")],
            self.mode,
            |this, mode| this.mode = mode,
            cx,
        );
        let sort = self.choice(
            "sort-order",
            [(false, "Newest first"), (true, "Oldest first")],
            self.sort_asc,
            |this, asc| this.sort_asc = asc,
            cx,
        );
        let dates = match self.mode {
            DateMode::On => div().flex().child(self.date_field(None, &self.on, cx)),
            DateMode::Range => div()
                .flex()
                .gap_2()
                .child(self.date_field(Some("From"), &self.from, cx))
                .child(self.date_field(Some("To"), &self.to, cx)),
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .w(px(280.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(section_label("Date", p.muted))
                    .child(mode),
            )
            .child(dates)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(section_label("Sort order", p.muted))
                    .child(sort),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .pt_1()
                    .border_t_1()
                    .border_color(p.border_soft)
                    .when(self.applied.is_active(), |el| {
                        el.child(
                            div()
                                .id("filter-reset")
                                .text_size(px(11.))
                                .text_color(p.muted)
                                .cursor_pointer()
                                .hover(|s| s.text_color(p.fg))
                                .on_click(cx.listener(|this, _, _, cx| this.reset(cx)))
                                .child("Reset"),
                        )
                    })
                    .child(
                        div()
                            .id("filter-apply")
                            .ml_auto()
                            .flex()
                            .items_center()
                            .h(px(26.))
                            .px_3()
                            .rounded(R_SM)
                            .bg(p.accent)
                            .text_color(p.accent_fg)
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.9))
                            .on_click(cx.listener(|this, _, _, cx| this.apply(cx)))
                            .child("Apply"),
                    ),
            )
    }
}

fn section_label(text: &'static str, color: gpui_kit::Hsla) -> impl IntoElement {
    div()
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(text)
}

impl Render for FilterPopover {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let hovered = self.trigger_hovered.clone();
        let trigger_hovered = self.trigger_hovered.clone();
        let label = if self.applied.is_active() { "Filter ·" } else { "Filter" };

        div()
            .relative()
            .child(
                toolbar_button("filter", AppIcon::SlidersHorizontal, label, self.applied.is_active(), cx)
                    .on_hover(move |hover, _, _| hovered.set(*hover))
                    .on_click(cx.listener(|this, _, window, cx| {
                        let open = !this.open;
                        this.set_open(open, window, cx);
                    })),
            )
            .when(self.open, |el| {
                el.child(popover_below(
                    "filter-panel",
                    self.form(cx),
                    px(6.),
                    true,
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
