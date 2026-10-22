//! One transcript on the activity timeline: a dot on the rail and a card with
//! the text, when and where it was pasted, and copy / delete.

use std::time::Duration;

use gpui_kit::{
    div, prelude::*, px, Animation, AnimationExt as _, AnyElement, App, Div, ElementId, Entity,
    FontWeight, IntoElement, Stateful,
};

use gpui_kit::component::Icon;

use super::feed::Feed;
use super::format::fmt_date;
use crate::api::dto::TranscriptResponse;
use crate::ui::icons::AppIcon;
use crate::ui::theme::{Palette, R_LG, R_SM};

/// Where a row sits in the list, which decides how far the rail runs.
#[derive(Clone, Copy)]
pub struct RowPlace {
    pub first: bool,
    /// Nothing follows, not even the loading spinner.
    pub last: bool,
}

pub struct RowState {
    pub copied: bool,
    /// Just arrived: fades in rather than popping.
    pub fresh: bool,
}

pub fn transcript_row(
    item: &TranscriptResponse,
    place: RowPlace,
    state: &RowState,
    feed: &Entity<Feed>,
    cx: &App,
) -> AnyElement {
    let p = *Palette::global(cx);
    let id = item.id;
    let row = div()
        .id(ElementId::NamedInteger(
            "transcript".into(),
            id.unsigned_abs(),
        ))
        .relative()
        .flex()
        .w_full()
        .gap(px(14.))
        .pb_4()
        .when(!place.last, |el| {
            // The rail runs behind the dots, so a row only draws its own span.
            el.child(
                div()
                    .absolute()
                    .left(px(9.))
                    .top(if place.first { px(22.) } else { px(0.) })
                    .bottom_0()
                    .w(px(1.))
                    .bg(p.border_soft),
            )
        })
        .child(
            div()
                .flex_shrink_0()
                .w(px(20.))
                .flex()
                .justify_center()
                .child(div().mt(px(12.)).size(px(8.)).rounded_full().bg(p.accent)),
        )
        .child(card(item, state, feed, p));

    if state.fresh {
        row.with_animation(
            ElementId::NamedInteger("transcript-fresh".into(), id.unsigned_abs()),
            Animation::new(Duration::from_millis(180)),
            |el, t| el.opacity(t),
        )
        .into_any_element()
    } else {
        row.into_any_element()
    }
}

fn card(
    item: &TranscriptResponse,
    state: &RowState,
    feed: &Entity<Feed>,
    p: Palette,
) -> impl IntoElement {
    let id = item.id;
    let text = item.content.clone();
    let copy_feed = feed.clone();
    let delete_feed = feed.clone();

    div()
        .id(ElementId::NamedInteger("card".into(), id.unsigned_abs()))
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_2()
        .px(px(14.))
        .py_3()
        .rounded(R_LG)
        .bg(p.panel)
        .border_1()
        .border_color(p.border_soft)
        .hover(|s| s.bg(p.surface).border_color(p.border))
        .child(
            div()
                .text_size(px(13.))
                .line_height(px(20.8))
                .text_color(p.fg)
                .child(item.content.clone()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .text_size(px(10.))
                        .text_color(p.muted)
                        .child(fmt_date(&item.created_at))
                        .when_some(item.target_app.as_deref(), |el, app| {
                            el.child(format!(" · Pasted in {app}"))
                        }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(2.))
                        .child(
                            action(("copy", id), p)
                                .when(state.copied, |el| el.text_color(p.success))
                                .child(
                                    Icon::new(if state.copied {
                                        AppIcon::Check
                                    } else {
                                        AppIcon::Copy
                                    })
                                    .size(px(11.)),
                                )
                                .child(if state.copied { "Copied" } else { "Copy" })
                                .when(!state.copied, |el| el.hover(|s| s.text_color(p.accent)))
                                .on_click(move |_, _, cx| {
                                    copy_feed
                                        .update(cx, |feed, cx| feed.copy(id, text.clone(), cx));
                                }),
                        )
                        .child(
                            action(("delete", id), p)
                                .hover(|s| s.text_color(p.danger))
                                .child(Icon::new(AppIcon::Trash).size(px(11.)))
                                .child("Delete")
                                .on_click(move |_, window, cx| {
                                    delete_feed.update(cx, |feed, cx| feed.delete(id, window, cx));
                                }),
                        ),
                ),
        )
}

/// A ghost text button at the card's foot; the caller adds hover colour and content.
fn action(id: (&'static str, i64), p: Palette) -> Stateful<Div> {
    div()
        .id(ElementId::NamedInteger(id.0.into(), id.1.unsigned_abs()))
        .flex()
        .items_center()
        .gap_1()
        .px(px(6.))
        .py(px(2.))
        .rounded(R_SM)
        .text_size(px(10.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.muted)
        .cursor_pointer()
}
