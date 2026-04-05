
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
            Self::Large => "ggml-large-v3-turbo.bin",
            Self::Medium => "ggml-medium.en.bin",
            Self::Small => "ggml-small.en.bin",
            Self::Base => "ggml-base.en.bin",
            Self::Tiny => "ggml-tiny.en.bin",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Large => "Whisper Large v3 Turbo",
            Self::Medium => "Whisper Medium",
            Self::Small => "Whisper Small",
            Self::Base => "Whisper Base",
            Self::Tiny => "Whisper Tiny",
        }
    }

    pub const fn url(self) -> &'static str {
        match self {
            Self::Large => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
            Self::Medium => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.en.bin",
            Self::Small => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin",
            Self::Base => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
            Self::Tiny => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
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
///
/// GPU path (VRAM reported correctly — discrete GPU):
///   ≥6 GB → Large, ≥3 GB → Medium, <3 GB → Small
///
/// iGPU fallback (Vulkan but VRAM <1 GB — DXGI reports shared memory incorrectly):
///   Use RAM thresholds: ≥16 GB → Large, else → Medium
///
/// CPU path:
///   ≥16 GB → Large, ≥8 GB → Medium, <8 GB → Small
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
            if vram_gb >= 6.0 {
                ModelSize::Large
            } else if vram_gb >= 3.0 {
                ModelSize::Medium
            } else {
                ModelSize::Small
            }
        }
        "vulkan" => {
            if vram_gb >= 1.0 {
                // Discrete GPU with valid VRAM reading
                if vram_gb >= 6.0 {
                    ModelSize::Large
                } else if vram_gb >= 3.0 {
                    ModelSize::Medium
                } else {
                    ModelSize::Small
                }
            } else {
                // iGPU — DXGI reports near-zero VRAM; fall back to RAM thresholds
                if ram_gb >= 16.0 {
                    ModelSize::Large
                } else {
                    ModelSize::Medium
                }
            }
        }
        _ => {
            // CPU path
            if ram_gb >= 16.0 {
                ModelSize::Large
            } else if ram_gb >= 8.0 {
                ModelSize::Medium
            } else {
                ModelSize::Small
            }
        }
    }
}

/// Resolve final model size: apply user override if set, else recommend from hardware.
/// `override_size` accepts "large" | "medium" | "small" | "base" | "tiny".
pub fn select_model_size(backend: Backend, override_size: Option<&str>) -> ModelSize {
    match override_size {
        Some("large") => ModelSize::Large,
        Some("medium") => ModelSize::Medium,
        Some("small") => ModelSize::Small,
        Some("base") => ModelSize::Base,
        Some("tiny") => ModelSize::Tiny,
        _ => {
            let profile = crate::hardware::cached_profile();
            select_model_size_from_profile(
                backend.as_str(),
                profile.vram_gb,
                profile.ram_gb,
            )
        }
    }
}
