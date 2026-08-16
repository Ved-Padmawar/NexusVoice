/// Which whisper-rs backend to use for inference (auto-detected, not user-configurable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Cuda,
    Vulkan,
    Cpu,
}

impl Backend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cuda => "cuda",
            Self::Vulkan => "vulkan",
            Self::Cpu => "cpu",
        }
    }

    pub const fn has_gpu(self) -> bool {
        matches!(self, Self::Cuda | Self::Vulkan)
    }
}

/// Which model size to load.
/// Auto-selected based on hardware; user can override.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSize {
    /// ggml-large-v3 — maximum accuracy, requires GPU ≥10 GB VRAM
    LargeFull,
    /// ggml-large-v3-turbo — best accuracy, requires GPU ≥6 GB VRAM or ≥16 GB RAM
    Large,
    /// ggml-medium.en — good accuracy, GPU ≥3 GB VRAM or ≥8 GB RAM
    Medium,
    /// ggml-small.en — standard accuracy, moderate hardware
    Small,
    /// ggml-base.en — basic accuracy, low-end hardware
    Base,
    /// ggml-tiny.en — lowest accuracy, fastest inference
    Tiny,
}

impl ModelSize {
    pub const fn filename(self) -> &'static str {
        match self {
            // LargeFull stays F16 as the multilingual anchor; turbo uses q8_0
            // (near-lossless). .en tiers use q5 (medium has no q5_1 on HF).
            Self::LargeFull => "ggml-large-v3.bin",
            Self::Large => "ggml-large-v3-turbo-q8_0.bin",
            Self::Medium => "ggml-medium.en-q5_0.bin",
            Self::Small => "ggml-small.en-q5_1.bin",
            Self::Base => "ggml-base.en-q5_1.bin",
            Self::Tiny => "ggml-tiny.en-q5_1.bin",
        }
    }

    /// DTW alignment-heads preset for word-level timestamps, matched to this model.
    pub const fn dtw_preset(self) -> whisper_rs::DtwModelPreset {
        use whisper_rs::DtwModelPreset;
        match self {
            Self::LargeFull => DtwModelPreset::LargeV3,
            Self::Large => DtwModelPreset::LargeV3Turbo,
            Self::Medium => DtwModelPreset::MediumEn,
            Self::Small => DtwModelPreset::SmallEn,
            Self::Base => DtwModelPreset::BaseEn,
            Self::Tiny => DtwModelPreset::TinyEn,
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::LargeFull => "Whisper Large v3",
            Self::Large => "Whisper Large v3 Turbo",
            Self::Medium => "Whisper Medium",
            Self::Small => "Whisper Small",
            Self::Base => "Whisper Base",
            Self::Tiny => "Whisper Tiny",
        }
    }

    pub const fn url(self) -> &'static str {
        match self {
            Self::LargeFull => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin"
            }
            Self::Large => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q8_0.bin"
            }
            Self::Medium => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.en-q5_0.bin"
            }
            Self::Small => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en-q5_1.bin"
            }
            Self::Base => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en-q5_1.bin"
            }
            Self::Tiny => {
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en-q5_1.bin"
            }
        }
    }
}

/// Detect backend from hardware.
pub fn detect_backend() -> Backend {
    let profile = crate::hardware::cached_profile();
    match profile.execution_provider.as_str() {
        "cuda" => Backend::Cuda,
        "vulkan" => Backend::Vulkan,
        _ => Backend::Cpu,
    }
}

/// Select recommended model size based on hardware profile.
/// Thresholds are tuned for the quantized catalog plus compute headroom —
/// GPU: ≥4 GB → Large, ≥2 GB → Medium, else Small. CPU: ≥12/≥6 GB RAM.
pub fn recommend_model_size() -> ModelSize {
    let profile = crate::hardware::cached_profile();
    select_model_size_from_profile(
        profile.execution_provider.as_str(),
        profile.vram_gb,
        profile.ram_gb,
    )
}

pub fn select_model_size_from_profile(
    execution_provider: &str,
    vram_gb: f32,
    ram_gb: f32,
) -> ModelSize {
    match execution_provider {
        "cuda" => {
            if vram_gb >= 4.0 {
                ModelSize::Large
            } else if vram_gb >= 2.0 {
                ModelSize::Medium
            } else {
                ModelSize::Small
            }
        }
        "vulkan" => {
            if vram_gb >= 1.0 {
                // Discrete GPU with valid VRAM reading
                if vram_gb >= 4.0 {
                    ModelSize::Large
                } else if vram_gb >= 2.0 {
                    ModelSize::Medium
                } else {
                    ModelSize::Small
                }
            } else {
                // iGPU — DXGI reports near-zero VRAM; fall back to RAM thresholds
                if ram_gb >= 12.0 {
                    ModelSize::Large
                } else {
                    ModelSize::Medium
                }
            }
        }
        _ => {
            // CPU path
            if ram_gb >= 12.0 {
                ModelSize::Large
            } else if ram_gb >= 6.0 {
                ModelSize::Medium
            } else {
                ModelSize::Small
            }
        }
    }
}

/// Resolve final model size: apply user override if set, else recommend from hardware.
/// `override_size` accepts "large-full" | "large" | "medium" | "small" | "base" | "tiny".
pub fn select_model_size(backend: Backend, override_size: Option<&str>) -> ModelSize {
    match override_size {
        Some("large-full") => ModelSize::LargeFull,
        Some("large") => ModelSize::Large,
        Some("medium") => ModelSize::Medium,
        Some("small") => ModelSize::Small,
        Some("base") => ModelSize::Base,
        Some("tiny") => ModelSize::Tiny,
        _ => {
            let profile = crate::hardware::cached_profile();
            select_model_size_from_profile(backend.as_str(), profile.vram_gb, profile.ram_gb)
        }
    }
}
