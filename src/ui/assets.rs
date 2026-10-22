//! Bundled assets: Phosphor icons and the Inter font, embedded in the binary.

use std::borrow::Cow;

use gpui_kit::{App, AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
#[include = "fonts/*.ttf"]
#[include = "logo.png"]
#[include = "providers/*"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path).map(|file| file.data))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter(|name| name.starts_with(path))
            .map(SharedString::from)
            .collect())
    }
}

/// Register Inter, so text renders the same on every machine: it is absent on
/// most Linux installs, where the fallback's wider metrics overflow layouts.
pub fn load_fonts(cx: &App) {
    let fonts = Assets::iter()
        .filter(|name| name.starts_with("fonts/"))
        .filter_map(|name| Assets::get(&name).map(|file| file.data))
        .collect();
    if let Err(e) = cx.text_system().add_fonts(fonts) {
        log::warn!("could not load the bundled fonts: {e}");
    }
}
