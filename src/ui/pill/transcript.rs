//! The live transcript inside the pill's card.
//!
//! The text is shaped once per partial, at the card's final width so it never
//! rewraps while the card grows, and measured then: the card's height follows
//! it. Painting only replays the shaped glyphs, bottom-aligned, which is what
//! scrolling to the end looked like in the old webview. Being its own entity,
//! a partial repaints this and nothing else.

use std::time::Duration;

use gpui_kit::{
    canvas, div, fill, font, point, px, size, Bounds, ContentMask, Context, Corners, Entity,
    IntoElement, ParentElement as _, Pixels, Render, Rgba, SharedString, Styled as _, Task,
    TextAlign, TextRun, Window, WrappedLine,
};

use super::geometry::CARD_W;
use super::theme::PillThemeDef;
use crate::events::Partial;

const FONT_SIZE: f32 = 13.5;
const LINE_HEIGHT: f32 = FONT_SIZE * 1.62;
const PAD_X: f32 = 19.0;
const PAD_BOTTOM: f32 = 8.0;
const WRAP_W: f32 = CARD_W - PAD_X * 2.0;

const CARET_W: f32 = 2.0;
const CARET_GAP: f32 = 3.0;
/// The caret sits 0.16em below the baseline, which puts its top this far
/// under the top of a line.
const CARET_TOP: f32 = 4.5;
const CARET_DIM: f32 = 0.15;
/// The caret blinks on a 1.05 s cycle, bright for the first 55% of it.
const BLINK_CYCLE: f32 = 1.05;
const BLINK_ON: f32 = 0.55;

pub struct Transcript {
    partial: Partial,
    lines: Vec<WrappedLine>,
    height: f32,
    /// Where the text ends, from the text's own top-left corner.
    end: (f32, f32),
    committed: Rgba,
    tentative: Rgba,
    accent: Rgba,
    opacity: f32,
    caret: Caret,
}

#[derive(Default)]
struct Caret {
    shown: bool,
    bright: bool,
    blink: Option<Task<()>>,
}

impl Transcript {
    pub fn new(theme: &PillThemeDef) -> Self {
        Self {
            partial: Partial::default(),
            lines: Vec::new(),
            height: 0.0,
            end: (0.0, 0.0),
            committed: theme.committed,
            tentative: theme.tentative,
            accent: theme.accent,
            opacity: 1.0,
            caret: Caret::default(),
        }
    }

    /// The text's height at the card's width, for sizing the card.
    pub fn height(&self) -> f32 {
        self.height
    }

    pub fn set_partial(&mut self, partial: Partial, window: &mut Window, cx: &mut Context<Self>) {
        if partial != self.partial {
            self.partial = partial;
            self.shape(window);
            cx.notify();
        }
    }

    pub fn set_theme(&mut self, theme: &PillThemeDef, window: &mut Window, cx: &mut Context<Self>) {
        self.committed = theme.committed;
        self.tentative = theme.tentative;
        self.accent = theme.accent;
        self.shape(window);
        cx.notify();
    }

    pub fn set_opacity(&mut self, opacity: f32, cx: &mut Context<Self>) {
        if (opacity - self.opacity).abs() > f32::EPSILON {
            self.opacity = opacity;
            cx.notify();
        }
    }

    /// Show the blinking caret, starting bright, or hide it and stop its timer.
    pub fn set_caret(&mut self, shown: bool, cx: &mut Context<Self>) {
        if shown == self.caret.shown {
            return;
        }
        self.caret.shown = shown;
        self.caret.bright = true;
        self.caret.blink = shown.then(|| {
            cx.spawn(async move |this, cx| loop {
                let bright = Duration::from_secs_f32(BLINK_CYCLE * BLINK_ON);
                let dim = Duration::from_secs_f32(BLINK_CYCLE * (1.0 - BLINK_ON));
                cx.background_executor().timer(bright).await;
                if this.update(cx, |this, cx| this.blink(false, cx)).is_err() {
                    return;
                }
                cx.background_executor().timer(dim).await;
                if this.update(cx, |this, cx| this.blink(true, cx)).is_err() {
                    return;
                }
            })
        });
        cx.notify();
    }

    fn blink(&mut self, bright: bool, cx: &mut Context<Self>) {
        self.caret.bright = bright;
        cx.notify();
    }

    fn shape(&mut self, window: &mut Window) {
        let Partial {
            committed,
            tentative,
        } = &self.partial;
        let mut text = committed.clone();
        let mut runs = vec![Self::run(committed.len(), self.committed)];
        if !tentative.is_empty() {
            text.push(' ');
            text.push_str(tentative);
            runs.push(Self::run(1, self.committed));
            runs.push(Self::run(tentative.len(), self.tentative));
        }
        runs.retain(|run| run.len > 0);

        self.lines = window
            .text_system()
            .shape_text(
                SharedString::from(text),
                px(FONT_SIZE),
                &runs,
                Some(px(WRAP_W)),
                None,
            )
            .map(|lines| lines.into_iter().collect())
            .unwrap_or_default();

        let line_height = px(LINE_HEIGHT);
        self.height = self
            .lines
            .iter()
            .map(|line| f32::from(line.size(line_height).height))
            .sum();
        self.end = self.lines.last().map_or((0.0, 0.0), |last| {
            let above = self.height - f32::from(last.size(line_height).height);
            let at = last
                .position_for_index(last.len(), line_height)
                .unwrap_or_default();
            (f32::from(at.x), above + f32::from(at.y))
        });
    }

    fn run(len: usize, color: Rgba) -> TextRun {
        TextRun {
            len,
            font: font("Inter"),
            color: color.into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut gpui_kit::App) {
        let line_height = px(LINE_HEIGHT);
        let left = bounds.left() + px(PAD_X);
        let top = bounds.bottom() - px(PAD_BOTTOM + self.height);

        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            let mut y = top;
            for line in &self.lines {
                // A failed paint is a glyph that would not rasterize; there is
                // nothing to do about it mid-frame.
                let _ = line.paint(
                    point(left, y),
                    line_height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
                y += line.size(line_height).height;
            }

            if self.caret.shown && !self.lines.is_empty() {
                let (mut x, mut y) = self.end;
                if x + CARET_GAP + CARET_W > WRAP_W {
                    // An inline caret that does not fit starts the next line.
                    x = 0.0;
                    y += LINE_HEIGHT;
                }
                let alpha = if self.caret.bright { 1.0 } else { CARET_DIM };
                let origin = point(left + px(x + CARET_GAP), top + px(y + CARET_TOP));
                let color = Rgba {
                    a: self.accent.a * alpha,
                    ..self.accent
                };
                window.paint_quad(
                    fill(Bounds::new(origin, size(px(CARET_W), px(FONT_SIZE))), color)
                        .corner_radii(Corners::all(px(CARET_W / 2.0))),
                );
            }
        });
    }
}

impl Render for Transcript {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this: Entity<Self> = cx.entity();
        div().size_full().opacity(self.opacity).child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, cx| {
                    this.update(cx, |this, cx| this.paint(bounds, window, cx));
                },
            )
            .size_full(),
        )
    }
}
