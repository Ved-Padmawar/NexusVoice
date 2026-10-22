//! The dictionary: trigger words and what they are replaced with, added from
//! a quick form and edited in place in the table.

mod form;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::WindowExt as _;
use gpui_kit::{
    div, list, prelude::*, px, AnyElement, App, Context, ElementId, Entity, FontWeight,
    ListAlignment, ListState, Subscription, Window,
};

use super::dashboard::{primary_button, ButtonSize};
use crate::api::{self, dto::DictionaryResponse};
use crate::ui::components::{icon, page_header, skeleton, text_input, LoadState, SectionState};
use crate::ui::icons::AppIcon;
use crate::ui::runtime::SpawnApi as _;
use crate::ui::theme::{alpha, Palette, R_LG, R_MD, R_SM, R_XL};
use form::entry_from_fields;

/// What Tailwind's `font-mono` resolved to on each platform; the correction
/// chip is the one place the app shows code-like text.
const MONO_FONT: &str = if cfg!(target_os = "windows") {
    "Consolas"
} else if cfg!(target_os = "macos") {
    "Menlo"
} else {
    "DejaVu Sans Mono"
};

/// Rows rendered beyond the viewport, so scrolling doesn't pop them in.
const OVERDRAW: f32 = 400.;

/// The row being edited in place.
struct Edit {
    id: i64,
    /// The term as stored, so a rename can drop the old entry.
    previous_term: String,
    term: Entity<InputState>,
    replacement: Entity<InputState>,
    saving: bool,
    _enter: Subscription,
}

pub struct Dictionary {
    entries: Vec<DictionaryResponse>,
    state: LoadState,
    term: Entity<InputState>,
    replacement: Entity<InputState>,
    saving: bool,
    edit: Option<Edit>,
    list: ListState,
    _enter: Subscription,
}

fn input(
    window: &mut Window,
    cx: &mut Context<Dictionary>,
    placeholder: &'static str,
) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
}

fn pressed_enter(event: &InputEvent) -> bool {
    matches!(event, InputEvent::PressEnter { .. })
}

impl Dictionary {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let term = input(window, cx, "e.g. teh, gonna");
        let replacement = input(window, cx, "e.g. the, going to");
        let enter = cx.subscribe_in(
            &replacement,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if pressed_enter(event) {
                    this.add(window, cx);
                }
            },
        );
        let dictionary = Self {
            entries: Vec::new(),
            state: LoadState::Loading,
            term,
            replacement,
            saving: false,
            edit: None,
            list: ListState::new(0, ListAlignment::Top, px(OVERDRAW)),
            _enter: enter,
        };
        Self::refresh(cx);
        dictionary
    }

    fn refresh(cx: &mut Context<Self>) {
        cx.spawn_api(async { api::get_dictionary().await }, |this, result, cx| {
            match result {
                Ok(entries) => {
                    this.entries = entries;
                    this.state = LoadState::Loaded;
                    this.list.reset(this.entries.len());
                }
                Err(e) => this.state = LoadState::Failed(e.to_string().into()),
            }
            cx.notify();
        });
    }

    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let fields = entry_from_fields(
            &self.term.read(cx).value(),
            &self.replacement.read(cx).value(),
        );
        let Some((term, replacement)) = fields.filter(|_| !self.saving) else {
            return;
        };
        self.saving = true;
        cx.notify();
        cx.spawn_api_in(
            window,
            async move { api::update_dictionary(term, replacement, None).await },
            |this, result, window, cx| {
                this.saving = false;
                match result {
                    Ok(_) => {
                        for field in [&this.term, &this.replacement] {
                            field.update(cx, |field, cx| field.set_value("", window, cx));
                        }
                        window.push_notification(Notification::success("Entry saved"), cx);
                        Self::refresh(cx);
                    }
                    // The inputs keep their text for another try.
                    Err(e) => window.push_notification(Notification::error(e.to_string()), cx),
                }
                cx.notify();
            },
        );
    }

    fn start_edit(
        &mut self,
        entry: &DictionaryResponse,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let term = cx.new(|cx| InputState::new(window, cx).default_value(entry.term.clone()));
        let replacement =
            cx.new(|cx| InputState::new(window, cx).default_value(entry.replacement.clone()));
        let enter = cx.subscribe_in(
            &replacement,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if pressed_enter(event) {
                    this.commit_edit(window, cx);
                }
            },
        );
        let previous = self.edit.replace(Edit {
            id: entry.id,
            previous_term: entry.term.clone(),
            term,
            replacement,
            saving: false,
            _enter: enter,
        });
        self.remeasure_rows(previous.map(|e| e.id).into_iter().chain([entry.id]));
        cx.notify();
    }

    fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        if let Some(edit) = self.edit.take() {
            self.remeasure_rows([edit.id]);
        }
        cx.notify();
    }

    /// An edited row is taller than a plain one, so the list measures it again.
    fn remeasure_rows(&self, ids: impl IntoIterator<Item = i64>) {
        for id in ids {
            if let Some(ix) = self.entries.iter().position(|e| e.id == id) {
                self.list.splice(ix..ix + 1, 1);
            }
        }
    }

    fn commit_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edit) = self.edit.as_mut().filter(|e| !e.saving) else {
            return;
        };
        let Some((term, replacement)) = entry_from_fields(
            &edit.term.read(cx).value(),
            &edit.replacement.read(cx).value(),
        ) else {
            return;
        };
        edit.saving = true;
        let previous = Some(edit.previous_term.clone());
        cx.notify();
        cx.spawn_api_in(
            window,
            async move { api::update_dictionary(term, replacement, previous).await },
            |this, result, window, cx| {
                if let Some(edit) = this.edit.as_mut() {
                    edit.saving = false;
                }
                match result {
                    Ok(_) => {
                        this.cancel_edit(cx);
                        Self::refresh(cx);
                    }
                    // The edit stays open for another try.
                    Err(e) => window.push_notification(Notification::error(e.to_string()), cx),
                }
                cx.notify();
            },
        );
    }

    fn delete(id: i64, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_api_in(
            window,
            async move { api::delete_dictionary_entry(id).await },
            |_, result, window, cx| match result {
                Ok(()) => Self::refresh(cx),
                Err(e) => window.push_notification(Notification::error(e.to_string()), cx),
            },
        );
    }

    fn render_quick_add(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let can_add = !self.saving
            && entry_from_fields(
                &self.term.read(cx).value(),
                &self.replacement.read(cx).value(),
            )
            .is_some();
        let field = |label: &'static str, state: &Entity<InputState>| {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(6.))
                .child(
                    div()
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.muted)
                        .child(label),
                )
                .child(text_input(state, cx).disabled(self.saving))
        };
        div()
            .flex_shrink_0()
            .px(px(18.))
            .py_3()
            .rounded(R_XL)
            .bg(p.panel)
            .border_1()
            .border_color(p.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .mb(px(10.))
                    .child(icon(AppIcon::Plus, 12., p.accent))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.fg_2)
                            .child("Quick addition"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap(px(10.))
                    .child(field("Trigger word", &self.term))
                    .child(field("Corrected text", &self.replacement))
                    .child(
                        primary_button(
                            "add-entry",
                            ButtonSize::Default,
                            !can_add,
                            {
                                let this = cx.entity();
                                move |window, cx| this.update(cx, |this, cx| this.add(window, cx))
                            },
                            cx,
                        )
                        .child(if self.saving {
                            "Saving…"
                        } else {
                            "Add to Dictionary"
                        }),
                    ),
            )
    }

    fn render_table(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = *Palette::global(cx);
        let content = match self.state {
            LoadState::Loaded if self.entries.is_empty() => empty_state(cx),
            LoadState::Loaded => self.entries_table(cx),
            _ => div().into_any_element(),
        };
        let retry = cx.entity();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(10.))
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.fg_2)
                    .child("Vocabulary engine"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .rounded(R_XL)
                    .border_1()
                    .border_color(p.border)
                    .bg(p.bg)
                    .overflow_hidden()
                    .child(SectionState::new(
                        self.state.clone(),
                        false,
                        table_skeleton(cx),
                        content,
                        move |_, cx| retry.update(cx, |_, cx| Self::refresh(cx)),
                    )),
            )
    }

    fn entries_table(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = *Palette::global(cx);
        let this = cx.entity();
        let rows = list(self.list.clone(), move |ix, _, cx| {
            this.read(cx).render_row(ix, &this, cx)
        })
        .size_full();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .bg(p.panel)
                    .border_b_1()
                    .border_color(p.border)
                    .child(cell(Column::Term, header_label("Input trigger", p.muted)))
                    .child(cell(
                        Column::Replacement,
                        header_label("Output correction", p.muted),
                    ))
                    .child(cell(Column::Hits, header_label("Hits", p.muted)))
                    .child(cell(Column::Actions, header_label("Actions", p.muted))),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(rows)
                    .vertical_scrollbar(&self.list),
            )
            .into_any_element()
    }

    fn render_row(&self, ix: usize, this: &Entity<Self>, cx: &App) -> AnyElement {
        let Some(entry) = self.entries.get(ix) else {
            return div().into_any_element();
        };
        let p = *Palette::global(cx);
        let editing = self.edit.as_ref().filter(|e| e.id == entry.id);
        let id = ElementId::NamedInteger("entry".into(), entry.id.unsigned_abs());
        let row = div()
            .id(id)
            .flex()
            .w_full()
            .items_center()
            .when(ix + 1 < self.entries.len(), |el| {
                el.border_b_1().border_color(p.border_soft)
            })
            .hover(|s| s.bg(p.surface));
        match editing {
            Some(edit) => row.children(edit_cells(entry, edit, this, &p, cx)),
            None => row.children(display_cells(entry, this, &p)),
        }
        .into_any_element()
    }
}

#[derive(Clone, Copy)]
enum Column {
    Term,
    Replacement,
    Hits,
    Actions,
}

/// A table cell with the column's width and alignment.
fn cell(column: Column, content: impl IntoElement) -> impl IntoElement {
    let base = div().flex().items_center().px_4().py(px(14.)).min_w_0();
    match column {
        Column::Term | Column::Replacement => base.w(gpui_kit::relative(0.3)).child(content),
        Column::Hits => base
            .flex_1()
            .min_w(px(100.))
            .justify_center()
            .child(content),
        Column::Actions => base
            .w(px(120.))
            .flex_shrink_0()
            .justify_end()
            .child(content),
    }
}

fn header_label(text: &'static str, color: gpui_kit::Hsla) -> impl IntoElement {
    div()
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .whitespace_nowrap()
        .child(text)
}

fn hits(entry: &DictionaryResponse, p: &Palette) -> impl IntoElement {
    div()
        .text_size(px(11.))
        .font_weight(FontWeight::BOLD)
        .text_color(p.muted)
        .child(entry.hits.to_string())
}

fn display_cells(
    entry: &DictionaryResponse,
    this: &Entity<Dictionary>,
    p: &Palette,
) -> Vec<AnyElement> {
    let (edit_entry, edit_this) = (entry.clone(), this.clone());
    let (id, delete_this) = (entry.id, this.clone());
    vec![
        cell(
            Column::Term,
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(icon(AppIcon::Mic, 13., p.muted))
                .child(
                    div()
                        .truncate()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.fg)
                        .child(entry.term.clone()),
                ),
        )
        .into_any_element(),
        cell(
            Column::Replacement,
            div()
                .truncate()
                .px_2()
                .py(px(2.))
                .rounded(R_SM)
                .bg(p.accent_soft)
                .border_1()
                .border_color(alpha(p.accent, 0.2))
                .font_family(MONO_FONT)
                .text_size(px(12.))
                .text_color(p.accent)
                .child(entry.replacement.clone()),
        )
        .into_any_element(),
        cell(Column::Hits, hits(entry, p)).into_any_element(),
        cell(
            Column::Actions,
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap(px(2.))
                .child(icon_button(
                    "edit",
                    entry.id,
                    AppIcon::Pencil,
                    p.accent_soft,
                    p.accent,
                    p,
                    move |window, cx| {
                        let entry = edit_entry.clone();
                        edit_this.update(cx, |this, cx| this.start_edit(&entry, window, cx));
                    },
                ))
                .child(icon_button(
                    "delete",
                    entry.id,
                    AppIcon::Trash,
                    alpha(p.danger, 0.12),
                    p.danger,
                    p,
                    move |window, cx| {
                        delete_this.update(cx, |_, cx| Dictionary::delete(id, window, cx));
                    },
                )),
        )
        .into_any_element(),
    ]
}

fn edit_cells(
    entry: &DictionaryResponse,
    edit: &Edit,
    this: &Entity<Dictionary>,
    p: &Palette,
    cx: &App,
) -> Vec<AnyElement> {
    let field = |state: &Entity<InputState>| -> Input {
        text_input(state, cx)
            .h(px(30.))
            .px(px(8.))
            .text_size(px(12.))
            .disabled(edit.saving)
    };
    let (commit_this, cancel_this) = (this.clone(), this.clone());
    vec![
        cell(Column::Term, div().w_full().child(field(&edit.term))).into_any_element(),
        cell(
            Column::Replacement,
            div().w_full().child(field(&edit.replacement)),
        )
        .into_any_element(),
        cell(Column::Hits, hits(entry, p)).into_any_element(),
        cell(
            Column::Actions,
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap(px(2.))
                .child(
                    icon_button(
                        "commit",
                        entry.id,
                        AppIcon::Check,
                        alpha(p.success, 0.12),
                        p.success,
                        p,
                        move |window, cx| {
                            commit_this.update(cx, |this, cx| this.commit_edit(window, cx));
                        },
                    )
                    .when(edit.saving, |el| el.opacity(0.5)),
                )
                .child(icon_button(
                    "cancel",
                    entry.id,
                    AppIcon::X,
                    alpha(p.danger, 0.12),
                    p.danger,
                    p,
                    move |_, cx| {
                        cancel_this.update(cx, Dictionary::cancel_edit);
                    },
                )),
        )
        .into_any_element(),
    ]
}

/// A 28px square icon button that tints on hover.
fn icon_button(
    kind: &'static str,
    id: i64,
    glyph: AppIcon,
    tint_bg: gpui_kit::Hsla,
    tint: gpui_kit::Hsla,
    p: &Palette,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    div()
        .id(ElementId::NamedInteger(kind.into(), id.unsigned_abs()))
        .flex()
        .items_center()
        .justify_center()
        .size(px(28.))
        .rounded(R_MD)
        .text_color(p.muted)
        .cursor_pointer()
        .hover(move |s| s.bg(tint_bg).text_color(tint))
        .on_click(move |_, window, cx| on_click(window, cx))
        .child(gpui_kit::component::Icon::new(glyph).size(px(14.)))
}

fn empty_state(cx: &App) -> AnyElement {
    let p = *Palette::global(cx);
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap_2()
        .py_8()
        .px_4()
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(36.))
                .rounded(R_LG)
                .bg(p.surface)
                .border_1()
                .border_color(p.border_soft)
                .opacity(0.8)
                .child(icon(AppIcon::BookOpen, 16., p.muted)),
        )
        .child(
            div()
                .max_w(px(240.))
                .text_size(px(12.))
                .text_color(p.muted)
                .text_center()
                .child("No entries yet. Add your first correction above."),
        )
        .into_any_element()
}

fn table_skeleton(cx: &App) -> impl IntoElement {
    let p = *Palette::global(cx);
    div().flex().flex_col().children((0..5).map(|i| {
        div()
            .flex()
            .items_center()
            .gap_4()
            .px_4()
            .py(px(14.))
            .when(i < 4, |el| el.border_b_1().border_color(p.border_soft))
            .child(skeleton(
                format!("dict-skeleton-a-{i}"),
                |d| d.w(gpui_kit::relative(0.28)).h(px(14.)),
                cx,
            ))
            .child(skeleton(
                format!("dict-skeleton-b-{i}"),
                |d| d.w(gpui_kit::relative(0.28)).h(px(14.)),
                cx,
            ))
            .child(div().ml_auto().child(skeleton(
                format!("dict-skeleton-c-{i}"),
                |d| d.w(px(32.)).h(px(14.)),
                cx,
            )))
            .child(skeleton(
                format!("dict-skeleton-d-{i}"),
                |d| d.w(px(48.)).h(px(14.)),
                cx,
            ))
    }))
}

impl Render for Dictionary {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .px_8()
            .pt_7()
            .pb_4()
            .child(page_header(
                AppIcon::BookOpen,
                "Dictionary",
                "Custom phonetics and word replacements.",
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap_5()
                    .mt(px(6.))
                    .child(self.render_quick_add(cx))
                    .child(self.render_table(cx)),
            )
    }
}
