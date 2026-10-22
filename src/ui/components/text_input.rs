//! Text fields, styled as the app's inputs: 34px, surface fill, a 1px edge
//! that turns accent on focus.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::{prelude::*, px, App, Entity};

use crate::ui::theme::{Palette, R_MD};

pub fn text_input(state: &Entity<InputState>, cx: &App) -> Input {
    let p = Palette::global(cx);
    Input::new(state)
        .h(px(34.))
        .px(px(10.))
        .rounded(R_MD)
        .bg(p.surface)
        .border_color(p.border)
        .text_size(px(13.))
        .text_color(p.fg)
}
