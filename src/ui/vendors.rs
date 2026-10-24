//! The vendor logo registry, shared by the LLM provider picker and the catalog.

use gpui_kit::{div, img, prelude::*, px, svg, Hsla, ObjectFit};

use super::components::icon;
use super::icons::AppIcon;
use super::theme::{Palette, R_XS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    OpenAi,
    Anthropic,
    Ollama,
    LmStudio,
    OpenRouter,
    Nvidia,
    Ibm,
    Qwen,
    /// Publishes only a raster logo.
    Moonshine,
    /// A user-supplied endpoint: drawn as a dashed "+" tile, not a logo.
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tint {
    Brand(u32),
    Inherit,
}

impl Vendor {
    /// A formatter provider id (`ollama`, `openai`…) or `moonshine`.
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "openai" => Self::OpenAi,
            "anthropic" => Self::Anthropic,
            "ollama" => Self::Ollama,
            "lmstudio" => Self::LmStudio,
            "openrouter" => Self::OpenRouter,
            "nvidia" => Self::Nvidia,
            "qwen" => Self::Qwen,
            "moonshine" => Self::Moonshine,
            "custom" => Self::Custom,
            _ => return None,
        })
    }

    /// Which vendor trained a model family; hyphens are ignored.
    pub fn for_family(family: &str) -> Option<Self> {
        match family.replace('-', "").as_str() {
            "whisper" => Some(Self::OpenAi),
            "parakeet" | "nemotron" | "canary" => Some(Self::Nvidia),
            "granite" => Some(Self::Ibm),
            "qwen3asr" => Some(Self::Qwen),
            "moonshine" => Some(Self::Moonshine),
            _ => None,
        }
    }

    pub fn asset(self) -> Option<&'static str> {
        Some(match self {
            Self::OpenAi => "providers/openai.svg",
            Self::Anthropic => "providers/anthropic.svg",
            Self::Ollama => "providers/ollama.svg",
            Self::LmStudio => "providers/lmstudio.svg",
            Self::OpenRouter => "providers/openrouter.svg",
            Self::Nvidia => "providers/nvidia.svg",
            Self::Ibm => "providers/ibm.svg",
            Self::Qwen => "providers/qwen.svg",
            Self::Moonshine => "providers/moonshine.png",
            Self::Custom => return None,
        })
    }

    fn tint(self) -> Tint {
        match self {
            Self::OpenAi => Tint::Brand(0x10_A3_7F),
            Self::Anthropic => Tint::Brand(0xD9_77_57),
            Self::Ollama => Tint::Brand(0xFF_FF_FF),
            Self::LmStudio => Tint::Brand(0x4B_27_E5),
            Self::Nvidia => Tint::Brand(0x76_B9_00),
            Self::Ibm => Tint::Brand(0x0F_62_FE),
            // The file paints a violet gradient; a flat mask keeps its mean.
            Self::Qwen => Tint::Brand(0x61_5C_ED),
            Self::OpenRouter | Self::Moonshine | Self::Custom => Tint::Inherit,
        }
    }
}

pub fn vendor_mark(
    vendor: Vendor,
    size: f32,
    inherit: Hsla,
    cx: &gpui_kit::App,
) -> impl IntoElement {
    let tile = div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(size));
    match vendor.asset() {
        None => {
            let p = Palette::global(cx);
            tile.rounded(R_XS)
                .border_1()
                .border_dashed()
                .border_color(p.border)
                .child(icon(AppIcon::Plus, size * 0.6, p.fg_2))
        }
        Some(path) if vendor == Vendor::Moonshine => {
            tile.child(img(path).size_full().object_fit(ObjectFit::Contain))
        }
        Some(path) => {
            let color = match vendor.tint() {
                Tint::Brand(rgb) => gpui_kit::rgb(rgb).into(),
                Tint::Inherit => inherit,
            };
            tile.child(svg().path(path).size_full().text_color(color))
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/ui/vendors.rs"]
mod tests;
