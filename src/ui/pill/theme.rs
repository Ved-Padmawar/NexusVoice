//! The pill's colours, one table per theme; Settings previews read it too.

use gpui_kit::{px, BoxShadow, Rgba};

use crate::transcription::controller::PillState;
use crate::ui::settings::PillTheme;

const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Rgba {
    Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a,
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    rgba(r, g, b, 1.0)
}

#[derive(Debug, Clone, Copy)]
pub struct Shadow {
    pub offset_y: f32,
    pub blur: f32,
    pub color: Rgba,
    pub inset: bool,
}

/// What a state paints behind itself.
#[derive(Debug, Clone, Copy)]
pub struct Surface {
    pub bg: Rgba,
    pub border: Rgba,
}

/// A round control: its resting colours and the ones under the pointer.
#[derive(Debug, Clone, Copy)]
pub struct ButtonColors {
    pub fg: Rgba,
    pub bg: Rgba,
    pub hover_fg: Rgba,
    pub hover_bg: Rgba,
}

#[derive(Debug, Clone, Copy)]
pub struct PillThemeDef {
    pub label: &'static str,
    pub bg: Rgba,
    pub border: Rgba,
    /// Mic icon, waveform and caret.
    pub accent: Rgba,
    pub brand: Rgba,
    pub recording_border: Rgba,
    pub dictation_border: Rgba,
    pub processing: Surface,
    pub downloading: Surface,
    pub error: Surface,
    /// Not drawn in the spinner states.
    pub shadow: &'static [Shadow],
    /// The processing spinner; downloading always uses [`DOWNLOAD`].
    pub spinner: Rgba,
    pub error_label: Rgba,
    pub control: ButtonColors,
    pub commit: ButtonColors,
    pub committed: Rgba,
    pub tentative: Rgba,
    pub hint: Rgba,
}

const DARK_CONTROL: ButtonColors = ButtonColors {
    fg: rgba(255, 255, 255, 0.82),
    bg: rgba(255, 255, 255, 0.07),
    hover_fg: rgb(255, 255, 255),
    hover_bg: rgba(255, 255, 255, 0.12),
};

const DARK_COMMIT: ButtonColors = ButtonColors {
    fg: rgb(125, 211, 167),
    bg: rgba(34, 197, 94, 0.14),
    hover_fg: rgb(183, 247, 204),
    hover_bg: rgba(34, 197, 94, 0.22),
};

const DARK_ERROR: Rgba = rgb(248, 113, 113);
const LIGHT_ERROR: Rgba = rgb(220, 38, 38);
const DARK_HINT: Rgba = rgba(255, 255, 255, 0.38);
const DARK_SHADOW: &[Shadow] = &[Shadow {
    offset_y: 1.0,
    blur: 0.0,
    color: rgba(255, 255, 255, 0.04),
    inset: true,
}];

/// The light themes style the commit button's hover with their generic
/// control rule, so it turns grey, not a lighter green. Kept as it was.
const fn light_control(ink: (u8, u8, u8)) -> ButtonColors {
    ButtonColors {
        fg: rgba(ink.0, ink.1, ink.2, 0.75),
        bg: rgba(ink.0, ink.1, ink.2, 0.07),
        hover_fg: rgba(ink.0, ink.1, ink.2, 0.95),
        hover_bg: rgba(ink.0, ink.1, ink.2, 0.12),
    }
}

const fn light_commit(ink: (u8, u8, u8)) -> ButtonColors {
    ButtonColors {
        fg: rgb(21, 128, 61),
        bg: rgba(34, 197, 94, 0.16),
        hover_fg: light_control(ink).hover_fg,
        hover_bg: light_control(ink).hover_bg,
    }
}

const CANVAS_INK: (u8, u8, u8) = (20, 20, 45);
const DAWN_INK: (u8, u8, u8) = (37, 22, 20);

const AMBER: Rgba = rgb(245, 158, 11);
/// The downloading spinner, the same in every theme.
pub const DOWNLOAD: Rgba = AMBER;
pub const DOWNLOAD_PCT: Rgba = rgba(251, 191, 36, 0.95);

const STEEL: PillThemeDef = PillThemeDef {
    label: "Steel",
    bg: rgb(0x14, 0x18, 0x20),
    border: rgba(148, 168, 200, 0.15),
    accent: rgb(0xb8, 0xcc, 0xe0),
    brand: rgba(200, 215, 235, 0.80),
    recording_border: rgba(148, 168, 200, 0.28),
    dictation_border: rgba(148, 168, 200, 0.32),
    processing: Surface {
        bg: rgb(0x11, 0x15, 0x20),
        border: rgba(148, 168, 200, 0.5),
    },
    downloading: Surface {
        bg: rgb(0x13, 0x16, 0x1e),
        border: rgba(245, 158, 11, 0.5),
    },
    error: Surface {
        bg: rgb(0x1a, 0x12, 0x18),
        border: rgba(239, 68, 68, 0.4),
    },
    shadow: DARK_SHADOW,
    spinner: rgb(148, 168, 200),
    error_label: DARK_ERROR,
    control: DARK_CONTROL,
    commit: DARK_COMMIT,
    committed: rgba(236, 241, 248, 0.95),
    tentative: rgba(200, 215, 235, 0.45),
    hint: DARK_HINT,
};

const MIDNIGHT: PillThemeDef = PillThemeDef {
    label: "Midnight",
    bg: rgb(0x0a, 0x0d, 0x14),
    border: rgba(26, 209, 209, 0.16),
    accent: rgb(0x1a, 0xd1, 0xd1),
    brand: rgba(236, 238, 244, 0.80),
    recording_border: rgba(26, 209, 209, 0.3),
    dictation_border: rgba(26, 209, 209, 0.34),
    processing: Surface {
        bg: rgb(0x09, 0x12, 0x18),
        border: rgba(26, 209, 209, 0.5),
    },
    downloading: Surface {
        bg: rgb(0x0e, 0x18, 0x10),
        border: rgba(245, 158, 11, 0.45),
    },
    error: Surface {
        bg: rgb(0x1a, 0x12, 0x14),
        border: rgba(239, 68, 68, 0.4),
    },
    shadow: DARK_SHADOW,
    spinner: rgb(26, 209, 209),
    error_label: DARK_ERROR,
    control: DARK_CONTROL,
    commit: DARK_COMMIT,
    committed: rgba(236, 238, 244, 0.95),
    tentative: rgba(26, 209, 209, 0.50),
    hint: DARK_HINT,
};

const CANVAS: PillThemeDef = PillThemeDef {
    label: "Canvas",
    bg: rgb(0xff, 0xff, 0xff),
    border: rgba(0, 0, 0, 0.10),
    accent: rgb(0x3a, 0x5b, 0xd9),
    brand: rgba(20, 20, 45, 0.80),
    recording_border: rgba(58, 91, 217, 0.25),
    dictation_border: rgba(58, 91, 217, 0.28),
    processing: Surface {
        bg: rgb(0xf0, 0xf4, 0xff),
        border: rgba(58, 91, 217, 0.4),
    },
    downloading: Surface {
        bg: rgb(0xff, 0xfb, 0xf0),
        border: rgba(245, 158, 11, 0.5),
    },
    error: Surface {
        bg: rgb(0xff, 0xf5, 0xf5),
        border: rgba(220, 38, 38, 0.3),
    },
    shadow: &[
        Shadow {
            offset_y: 2.0,
            blur: 12.0,
            color: rgba(0, 0, 0, 0.10),
            inset: false,
        },
        Shadow {
            offset_y: 1.0,
            blur: 3.0,
            color: rgba(0, 0, 0, 0.07),
            inset: false,
        },
    ],
    spinner: rgb(58, 91, 217),
    error_label: LIGHT_ERROR,
    control: light_control(CANVAS_INK),
    commit: light_commit(CANVAS_INK),
    committed: rgba(20, 20, 45, 0.92),
    tentative: rgba(58, 91, 217, 0.55),
    hint: rgba(20, 20, 45, 0.38),
};

const DAWN: PillThemeDef = PillThemeDef {
    label: "Dawn",
    bg: rgb(0xff, 0xf6, 0xf4),
    border: rgba(120, 60, 40, 0.14),
    accent: rgb(0xe4, 0x38, 0x00),
    brand: rgba(37, 22, 20, 0.80),
    recording_border: rgba(228, 56, 0, 0.28),
    dictation_border: rgba(228, 56, 0, 0.32),
    processing: Surface {
        bg: rgb(0xff, 0xf0, 0xea),
        border: rgba(228, 56, 0, 0.42),
    },
    downloading: Surface {
        bg: rgb(0xff, 0xfa, 0xf0),
        border: rgba(245, 158, 11, 0.5),
    },
    error: Surface {
        bg: rgb(0xff, 0xf5, 0xf5),
        border: rgba(220, 38, 38, 0.3),
    },
    shadow: &[
        Shadow {
            offset_y: 2.0,
            blur: 12.0,
            color: rgba(90, 40, 20, 0.12),
            inset: false,
        },
        Shadow {
            offset_y: 1.0,
            blur: 3.0,
            color: rgba(90, 40, 20, 0.08),
            inset: false,
        },
    ],
    spinner: rgb(228, 56, 0),
    error_label: LIGHT_ERROR,
    control: light_control(DAWN_INK),
    commit: light_commit(DAWN_INK),
    committed: rgba(37, 22, 20, 0.92),
    tentative: rgba(228, 56, 0, 0.55),
    hint: rgba(37, 22, 20, 0.38),
};

/// In the order of [`PillTheme::ALL`].
pub const PILL_THEMES: [PillThemeDef; 4] = [STEEL, MIDNIGHT, CANVAS, DAWN];

pub const fn def(theme: PillTheme) -> &'static PillThemeDef {
    match theme {
        PillTheme::Steel => &PILL_THEMES[0],
        PillTheme::Midnight => &PILL_THEMES[1],
        PillTheme::Canvas => &PILL_THEMES[2],
        PillTheme::Dawn => &PILL_THEMES[3],
    }
}

/// What the pill's box looks like: the part that eases between states.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    pub bg: Rgba,
    pub border: Rgba,
    /// How much of the theme's shadow shows, 0 to 1.
    pub shadow: f32,
}

impl PillThemeDef {
    pub fn look(&self, state: &PillState) -> Look {
        let (bg, border, shadow) = match state {
            PillState::Idle => (self.bg, self.border, 1.0),
            PillState::Recording => (self.bg, self.recording_border, 1.0),
            PillState::Dictation { .. } => (self.bg, self.dictation_border, 1.0),
            PillState::Processing => (self.processing.bg, self.processing.border, 0.0),
            PillState::Downloading(_) => (self.downloading.bg, self.downloading.border, 0.0),
            PillState::Error(_) => (self.error.bg, self.error.border, 1.0),
        };
        Look { bg, border, shadow }
    }
}

/// Blend with alpha-weighted channels, so fading to transparent avoids black.
pub fn mix(from: Rgba, to: Rgba, t: f32) -> Rgba {
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    let alpha = lerp(from.a, to.a);
    if alpha <= f32::EPSILON {
        return Rgba { a: 0.0, ..to };
    }
    let channel = |a: f32, b: f32| lerp(a * from.a, b * to.a) / alpha;
    Rgba {
        r: channel(from.r, to.r),
        g: channel(from.g, to.g),
        b: channel(from.b, to.b),
        a: alpha,
    }
}

impl Look {
    pub fn mix(self, to: Self, t: f32) -> Self {
        Self {
            bg: mix(self.bg, to.bg, t),
            border: mix(self.border, to.border, t),
            shadow: self.shadow + (to.shadow - self.shadow) * t,
        }
    }
}

pub fn faded(color: Rgba, factor: f32) -> Rgba {
    Rgba {
        a: color.a * factor,
        ..color
    }
}

pub fn shadows(theme: &PillThemeDef, amount: f32) -> Vec<BoxShadow> {
    if amount <= 0.001 {
        return Vec::new();
    }
    theme
        .shadow
        .iter()
        .map(|shadow| {
            let built = BoxShadow::new(
                px(0.0),
                px(shadow.offset_y),
                faded(shadow.color, amount).into(),
            )
            .blur_radius(px(shadow.blur));
            if shadow.inset {
                built.inset()
            } else {
                built
            }
        })
        .collect()
}

/// The notice bubble above the pill, the same in every theme.
pub const NOTICE_BG: Rgba = rgba(15, 14, 28, 0.97);
pub const NOTICE_TEXT: Rgba = rgb(251, 191, 36);
