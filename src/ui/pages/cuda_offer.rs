//! CudaOffer.

use gpui_kit::{div, prelude::*, Context, Window};

pub struct CudaOffer;

impl CudaOffer {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for CudaOffer {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
