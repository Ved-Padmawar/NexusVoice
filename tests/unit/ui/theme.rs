//! Theme colors are defined in OKLCH and converted here; a conversion slip
//! shifts every color in the app at once.

use gpui_kit::Rgba;

use super::{mix, oklch, Palette};
use crate::ui::settings::ThemeName;

fn rgb(color: gpui_kit::Hsla) -> (u8, u8, u8) {
    let c = Rgba::from(color);
    let byte = |v: f32| (v * 255.).round() as u8;
    (byte(c.r), byte(c.g), byte(c.b))
}

#[test]
fn oklch_converts_to_the_srgb_a_browser_paints() {
    // Reference values from CSS Color 4: these are the sRGB primaries and white.
    assert_eq!(rgb(oklch(1., 0., 0.)), (255, 255, 255));
    assert_eq!(rgb(oklch(0.627_955, 0.257_683, 29.234)), (255, 0, 0));
    assert_eq!(rgb(oklch(0.452_014, 0.313_214, 264.052)), (0, 0, 255));
}

#[test]
fn mix_weights_the_first_color_like_css_color_mix() {
    let black = oklch(0., 0., 0.);
    let white = oklch(1., 0., 0.);
    assert_eq!(rgb(mix(white, black, 0.25)), (64, 64, 64));
    assert_eq!(rgb(mix(white, black, 1.)), (255, 255, 255));
}

#[test]
fn every_theme_keeps_readable_text_on_its_background() {
    // A swapped or mistyped token shows up as text the same shade as its ground.
    for name in ThemeName::ALL {
        let p = Palette::of(name);
        let contrast = (p.fg.l - p.bg.l).abs();
        assert!(contrast > 0.5, "{name:?}: fg/bg lightness gap {contrast}");
        assert_eq!(p.dark, p.bg.l < p.fg.l, "{name:?}: dark flag disagrees with its colors");
    }
}

#[test]
fn midnight_alone_moves_the_semantic_colors_off_the_shared_hues() {
    let abyss = Palette::of(ThemeName::Abyss);
    let midnight = Palette::of(ThemeName::Midnight);
    let canvas = Palette::of(ThemeName::Canvas);
    assert_eq!(abyss.danger, canvas.danger);
    assert_ne!(abyss.danger, midnight.danger);
    assert_ne!(abyss.success, midnight.success);
}
