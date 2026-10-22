//! The eight app themes, defined in OKLCH exactly as the design tokens were,
//! and projected onto gpui-component's theme so kit components match.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{px, App, BoxShadow, Global, Hsla, Pixels, Rgba};

use super::settings::ThemeName;

/// Radius scale.
pub const R_XS: Pixels = px(4.);
pub const R_SM: Pixels = px(6.);
pub const R_MD: Pixels = px(8.);
pub const R_LG: Pixels = px(12.);
pub const R_XL: Pixels = px(16.);

pub const SIDEBAR_W: Pixels = px(220.);

/// Base text size and family.
pub const FONT_SIZE: Pixels = px(13.);
pub const FONT_FAMILY: &str = "Inter";

/// Windows 11's close-button red, deliberately not theme-tinted.
pub const CLOSE_RED: u32 = 0xc4_2b_1c;

/// A theme's colors. Field names follow the design tokens they replace.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub dark: bool,
    pub bg: Hsla,
    pub bg_alt: Hsla,
    pub panel: Hsla,
    pub panel_border: Hsla,
    pub surface: Hsla,
    pub surface_hover: Hsla,
    pub border: Hsla,
    pub border_soft: Hsla,
    pub fg: Hsla,
    pub fg_2: Hsla,
    pub muted: Hsla,
    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub accent_soft: Hsla,
    pub accent_fg: Hsla,
    pub success: Hsla,
    pub success_soft: Hsla,
    pub warning: Hsla,
    pub warning_soft: Hsla,
    pub danger: Hsla,
    pub danger_soft: Hsla,
    /// Shadow color for `shadow-sm/md/lg`, its alpha already the "near" layer's.
    shadow: Hsla,
    /// Shadow alphas for sm/md/lg, as `(far, near)`.
    shadow_alpha: [(f32, f32); 3],
}

impl Global for Palette {}

/// Convert an OKLCH color to GPUI's HSLA, through linear sRGB.
pub fn oklch(l: f32, c: f32, h: f32) -> Hsla {
    oklcha(l, c, h, 1.)
}

pub fn oklcha(lightness: f32, chroma: f32, hue: f32, alpha: f32) -> Hsla {
    // OKLCH → OKLab → LMS (cubed) → linear sRGB → gamma-encoded sRGB.
    let (green_red, blue_yellow) = {
        let hue = hue.to_radians();
        (chroma * hue.cos(), chroma * hue.sin())
    };
    let cone = |k_a: f32, k_b: f32| (lightness + k_a * green_red + k_b * blue_yellow).powi(3);
    let long = cone(0.396_337_78, 0.215_803_76);
    let medium = cone(-0.105_561_346, -0.063_854_17);
    let short = cone(-0.089_484_18, -1.291_485_5);
    let linear = |k_l: f32, k_m: f32, k_s: f32| k_l * long + k_m * medium + k_s * short;
    let encode = |x: f32| {
        let x = x.clamp(0., 1.);
        if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1. / 2.4) - 0.055
        }
    };
    Rgba {
        r: encode(linear(4.076_741_7, -3.307_711_6, 0.230_969_94)),
        g: encode(linear(-1.268_438, 2.609_757_4, -0.341_319_38)),
        b: encode(linear(-0.004_196_086_3, -0.703_418_6, 1.707_614_7)),
        a: alpha,
    }
    .into()
}

/// `color-mix(in srgb, a p%, b)`.
pub fn mix(a: Hsla, b: Hsla, p: f32) -> Hsla {
    let (a, b) = (Rgba::from(a), Rgba::from(b));
    let q = 1. - p;
    Rgba {
        r: a.r * p + b.r * q,
        g: a.g * p + b.g * q,
        b: a.b * p + b.b * q,
        a: a.a * p + b.a * q,
    }
    .into()
}

/// `color` at `alpha`, as `color-mix(in srgb, color p%, transparent)` gives.
pub fn alpha(color: Hsla, alpha: f32) -> Hsla {
    Hsla {
        a: color.a * alpha,
        ..color
    }
}

struct Spec {
    dark: bool,
    hue: f32,
    /// bg, bg-alt, panel, panel-border, surface, surface-hover, border, border-soft
    ground: [(f32, f32); 8],
    /// fg, fg-2, muted
    text: [(f32, f32); 3],
    /// accent (l, c, h), accent-hover (l, c), accent-fg (l, c, h)
    accent: (f32, f32, f32),
    accent_hover: (f32, f32),
    accent_soft_alpha: f32,
    accent_fg: (f32, f32, f32),
    shadow: (f32, f32, f32),
    shadow_alpha: [(f32, f32); 3],
}

impl Palette {
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn of(name: ThemeName) -> Self {
        let spec = spec(name);
        let ground = spec.ground.map(|(l, c)| oklch(l, c, spec.hue));
        let text = spec.text.map(|(l, c)| oklch(l, c, spec.hue));
        let (al, ac, ah) = spec.accent;
        let accent = oklch(al, ac, ah);
        let mut palette = Self {
            dark: spec.dark,
            bg: ground[0],
            bg_alt: ground[1],
            panel: ground[2],
            panel_border: ground[3],
            surface: ground[4],
            surface_hover: ground[5],
            border: ground[6],
            border_soft: ground[7],
            fg: text[0],
            fg_2: text[1],
            muted: text[2],
            accent,
            accent_hover: oklch(spec.accent_hover.0, spec.accent_hover.1, ah),
            accent_soft: oklcha(al, ac, ah, spec.accent_soft_alpha),
            accent_fg: oklch(spec.accent_fg.0, spec.accent_fg.1, spec.accent_fg.2),
            success: oklch(0.70, 0.17, 145.),
            success_soft: oklcha(0.70, 0.17, 145., 0.12),
            warning: oklch(0.78, 0.17, 70.),
            warning_soft: oklcha(0.78, 0.17, 70., 0.12),
            danger: oklch(0.63, 0.22, 22.),
            danger_soft: oklcha(0.63, 0.22, 22., 0.14),
            shadow: oklch(spec.shadow.0, spec.shadow.1, spec.shadow.2),
            shadow_alpha: spec.shadow_alpha,
        };
        // The one multi-hue theme: semantics on their own hues rather than the
        // shared ones, far enough from its cyan accent to read as signals.
        if name == ThemeName::Midnight {
            palette.success = oklch(0.78, 0.17, 145.);
            palette.success_soft = oklcha(0.78, 0.17, 145., 0.14);
            palette.warning = oklch(0.80, 0.15, 75.);
            palette.warning_soft = oklcha(0.80, 0.15, 75., 0.14);
            palette.danger = oklch(0.66, 0.21, 12.);
            palette.danger_soft = oklcha(0.66, 0.21, 12., 0.16);
        }
        palette
    }

    fn shadow(&self, level: usize, layers: [(f32, f32, f32); 2]) -> Vec<BoxShadow> {
        let (far, near) = self.shadow_alpha[level];
        [(layers[0], far), (layers[1], near)]
            .into_iter()
            .map(|((y, blur, _), a)| BoxShadow {
                color: alpha(self.shadow, a),
                offset: gpui_kit::point(px(0.), px(y)),
                blur_radius: px(blur),
                spread_radius: px(0.),
                inset: false,
            })
            .collect()
    }

    /// `0 1px 3px, 0 1px 2px`
    pub fn shadow_sm(&self) -> Vec<BoxShadow> {
        self.shadow(0, [(1., 3., 0.), (1., 2., 0.)])
    }

    /// `0 4px 12px, 0 2px 4px`
    pub fn shadow_md(&self) -> Vec<BoxShadow> {
        self.shadow(1, [(4., 12., 0.), (2., 4., 0.)])
    }

    /// `0 12px 28px, 0 4px 8px`
    pub fn shadow_lg(&self) -> Vec<BoxShadow> {
        self.shadow(2, [(12., 28., 0.), (4., 8., 0.)])
    }
}

#[allow(clippy::too_many_lines)] // one table of eight themes reads best whole
fn spec(name: ThemeName) -> Spec {
    match name {
        // Blue-grey dark, periwinkle accent (default).
        ThemeName::Abyss => Spec {
            dark: true,
            hue: 250.,
            ground: [
                (0.20, 0.02),
                (0.18, 0.02),
                (0.24, 0.02),
                (0.30, 0.02),
                (0.27, 0.02),
                (0.32, 0.02),
                (0.35, 0.02),
                (0.28, 0.02),
            ],
            text: [(0.92, 0.01), (0.85, 0.01), (0.56, 0.01)],
            accent: (0.75, 0.12, 250.),
            accent_hover: (0.80, 0.12),
            accent_soft_alpha: 0.15,
            accent_fg: (0.98, 0.005, 0.),
            shadow: (0., 0., 0.),
            shadow_alpha: [(0.30, 0.20), (0.40, 0.25), (0.60, 0.35)],
        },
        // Near-neutral indigo, cyan accent.
        ThemeName::Midnight => Spec {
            dark: true,
            hue: 265.,
            ground: [
                (0.15, 0.015),
                (0.12, 0.018),
                (0.19, 0.014),
                (0.27, 0.016),
                (0.23, 0.013),
                (0.28, 0.015),
                (0.32, 0.018),
                (0.25, 0.014),
            ],
            text: [(0.95, 0.008), (0.86, 0.010), (0.60, 0.020)],
            accent: (0.78, 0.13, 195.),
            accent_hover: (0.84, 0.12),
            accent_soft_alpha: 0.16,
            accent_fg: (0.16, 0.02, 220.),
            shadow: (0., 0., 0.),
            shadow_alpha: [(0.45, 0.30), (0.55, 0.35), (0.70, 0.45)],
        },
        // Slate blue-grey, pale icy accent; matches the Steel pill.
        ThemeName::Steel => Spec {
            dark: true,
            hue: 264.,
            ground: [
                (0.21, 0.017),
                (0.18, 0.017),
                (0.23, 0.021),
                (0.28, 0.024),
                (0.26, 0.023),
                (0.31, 0.026),
                (0.30, 0.026),
                (0.26, 0.022),
            ],
            text: [(0.92, 0.014), (0.82, 0.020), (0.55, 0.040)],
            accent: (0.84, 0.035, 248.),
            accent_hover: (0.90, 0.030),
            accent_soft_alpha: 0.15,
            accent_fg: (0.18, 0.020, 260.),
            shadow: (0., 0., 0.),
            shadow_alpha: [(0.35, 0.22), (0.50, 0.30), (0.70, 0.40)],
        },
        // Dark forest green, mint accent.
        ThemeName::Pine => Spec {
            dark: true,
            hue: 150.,
            ground: [
                (0.22, 0.02),
                (0.19, 0.02),
                (0.27, 0.02),
                (0.34, 0.02),
                (0.31, 0.02),
                (0.36, 0.02),
                (0.39, 0.02),
                (0.31, 0.02),
            ],
            text: [(0.92, 0.01), (0.84, 0.01), (0.56, 0.02)],
            accent: (0.72, 0.14, 150.),
            accent_hover: (0.78, 0.14),
            accent_soft_alpha: 0.15,
            accent_fg: (0.98, 0.005, 0.),
            shadow: (0., 0., 0.),
            shadow_alpha: [(0.30, 0.20), (0.45, 0.28), (0.60, 0.35)],
        },
        // Off-white light, cobalt accent.
        ThemeName::Canvas => Spec {
            dark: false,
            hue: 250.,
            ground: [
                (0.97, 0.005),
                (0.94, 0.005),
                (1.00, 0.000),
                (0.91, 0.005),
                (0.95, 0.005),
                (0.92, 0.008),
                (0.85, 0.008),
                (0.92, 0.005),
            ],
            text: [(0.25, 0.01), (0.35, 0.01), (0.55, 0.01)],
            accent: (0.48, 0.16, 255.),
            accent_hover: (0.42, 0.16),
            accent_soft_alpha: 0.16,
            accent_fg: (0.98, 0.01, 250.),
            shadow: (0.25, 0.01, 250.),
            shadow_alpha: [(0.04, 0.03), (0.08, 0.05), (0.12, 0.07)],
        },
        // Warm white light, orange accent.
        ThemeName::Dawn => Spec {
            dark: false,
            hue: 32.,
            ground: [
                (0.98, 0.010),
                (0.96, 0.014),
                (0.93, 0.020),
                (0.87, 0.024),
                (0.96, 0.010),
                (0.92, 0.018),
                (0.84, 0.024),
                (0.89, 0.016),
            ],
            text: [(0.22, 0.024), (0.38, 0.020), (0.56, 0.016)],
            accent: (0.60, 0.22, 40.),
            accent_hover: (0.54, 0.24),
            accent_soft_alpha: 0.16,
            accent_fg: (1.00, 0.00, 0.),
            shadow: (0.22, 0.02, 30.),
            shadow_alpha: [(0.07, 0.04), (0.12, 0.07), (0.18, 0.10)],
        },
        // Cool teal light, cyan-teal accent.
        ThemeName::Breeze => Spec {
            dark: false,
            hue: 200.,
            ground: [
                (0.97, 0.016),
                (0.94, 0.018),
                (0.91, 0.024),
                (0.85, 0.022),
                (0.95, 0.016),
                (0.91, 0.020),
                (0.82, 0.020),
                (0.88, 0.018),
            ],
            text: [(0.20, 0.016), (0.34, 0.014), (0.52, 0.016)],
            accent: (0.50, 0.14, 200.),
            accent_hover: (0.44, 0.16),
            accent_soft_alpha: 0.16,
            accent_fg: (0.99, 0.01, 200.),
            shadow: (0.20, 0.01, 200.),
            shadow_alpha: [(0.06, 0.04), (0.10, 0.06), (0.14, 0.08)],
        },
        // Soft rose light, pink accent.
        ThemeName::Blossom => Spec {
            dark: false,
            hue: 355.,
            ground: [
                (0.97, 0.014),
                (0.94, 0.016),
                (0.91, 0.022),
                (0.85, 0.020),
                (0.95, 0.014),
                (0.91, 0.018),
                (0.83, 0.018),
                (0.89, 0.016),
            ],
            text: [(0.22, 0.016), (0.36, 0.014), (0.54, 0.016)],
            accent: (0.50, 0.18, 355.),
            accent_hover: (0.44, 0.20),
            accent_soft_alpha: 0.16,
            accent_fg: (0.99, 0.01, 355.),
            shadow: (0.22, 0.01, 355.),
            shadow_alpha: [(0.06, 0.04), (0.09, 0.05), (0.14, 0.08)],
        },
    }
}

/// Make `name` the active theme, for our own views and the kit's components.
pub fn apply(name: ThemeName, cx: &mut App) {
    let p = Palette::of(name);
    cx.set_global(p);

    let mode = if p.dark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    Theme::change(mode, None, cx);
    Theme::update(cx, |theme| {
        theme.font_family = FONT_FAMILY.into();
        theme.font_size = FONT_SIZE;
        theme.radius = R_MD;
        theme.radius_lg = R_LG;
        theme.shadow = true;
        theme.notification.placement = gpui_kit::Anchor::BottomRight;
        theme.notification.width = px(356.);

        let c = &mut theme.colors;
        c.background = p.bg;
        c.foreground = p.fg;
        c.border = p.border;
        c.input = p.border;
        c.ring = p.accent;
        c.caret = p.accent;
        c.selection = p.accent_soft;
        c.accent = p.surface_hover;
        c.accent_foreground = p.fg;
        c.muted = p.surface;
        c.muted_foreground = p.muted;
        c.popover = p.panel;
        c.popover_foreground = p.fg;
        c.primary = p.accent;
        c.primary_hover = p.accent_hover;
        c.primary_active = p.accent_hover;
        c.primary_foreground = p.accent_fg;
        c.secondary = p.surface;
        c.secondary_hover = p.surface_hover;
        c.secondary_active = p.surface_hover;
        c.secondary_foreground = p.fg;
        c.danger = p.danger;
        c.danger_hover = p.danger;
        c.danger_active = p.danger;
        c.danger_foreground = gpui_kit::white();
        c.success = p.success;
        c.success_hover = p.success;
        c.success_active = p.success;
        c.warning = p.warning;
        c.warning_hover = p.warning;
        c.warning_active = p.warning;
        c.link = p.accent;
        c.link_hover = p.accent_hover;
        c.link_active = p.accent_hover;
        c.list = p.panel;
        c.list_hover = p.surface_hover;
        c.list_active = p.accent_soft;
        c.list_active_border = p.accent;
        c.switch = p.border;
        c.switch_thumb = gpui_kit::white();
        c.slider_bar = p.accent;
        c.slider_thumb = gpui_kit::white();
        c.progress_bar = p.accent;
        c.scrollbar = gpui_kit::transparent_black();
        c.scrollbar_thumb = p.border;
        c.scrollbar_thumb_hover = p.muted;
        c.skeleton = p.surface;
        c.overlay = alpha(gpui_kit::black(), 0.5);
        c.window_border = p.border;
        c.sidebar = p.panel;
        c.sidebar_border = p.border;
        c.sidebar_foreground = p.fg;
        c.sidebar_accent = p.surface;
        c.sidebar_accent_foreground = p.fg;
        c.sidebar_primary = p.accent;
        c.sidebar_primary_foreground = p.accent_fg;
        c.title_bar = p.bg;
        c.title_bar_border = p.border;
        c.tab_bar = p.surface;
        c.tab = gpui_kit::transparent_black();
        c.tab_active = p.panel;
        c.tab_foreground = p.muted;
        c.tab_active_foreground = p.fg;
    });
}

#[cfg(test)]
#[path = "../../tests/unit/ui/theme.rs"]
mod tests;
