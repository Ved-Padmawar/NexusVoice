use crate::hardware::{detect_profile, SysinfoProvider};

/// Which whisper-rs backend to use for inference (auto-detected, not user-configurable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Cuda,
    Vulkan,
    Cpu,
}

impl Backend {
    pub fn as_str(self) -> &'static str {
        match self {
            Backend::Cuda => "cuda",
            Backend::Vulkan => "vulkan",
            Backend::Cpu => "cpu",
        }
    }

    pub fn has_gpu(self) -> bool {
        matches!(self, Backend::Cuda | Backend::Vulkan)
    }
}

/// Which model size to load.
/// Auto-selected based on hardware; user can override.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSize {
    /// ggml-large-v3-turbo — best accuracy, requires GPU (~1.5 GB)
    Large,
    /// ggml-medium.en — good accuracy, runs well on CPU (~750 MB)
    Medium,
}

impl ModelSize {
    pub fn filename(self) -> &'static str {
        match self {
            ModelSize::Large => "ggml-large-v3-turbo.bin",
            ModelSize::Medium => "ggml-medium.en.bin",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            ModelSize::Large => "large-v3-turbo",
            ModelSize::Medium => "medium.en",
        }
    }

    pub fn url(self) -> &'static str {
        match self {
            ModelSize::Large => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
            ModelSize::Medium => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.en.bin",
        }
    }
}

/// Detect backend from hardware. Called once on engine load.
pub fn detect_backend() -> Backend {
    let profile = detect_profile(&SysinfoProvider);
    match profile.execution_provider.as_str() {
        "cuda" => Backend::Cuda,
        "vulkan" => Backend::Vulkan,
        _ => Backend::Cpu,
    }
}

/// Select model size: GPU → Large, CPU → Medium.
/// `override_size` ("large" | "medium") lets the user override.
pub fn select_model_size(backend: Backend, override_size: Option<&str>) -> ModelSize {
    match override_size {
        Some("large") => ModelSize::Large,
        Some("medium") => ModelSize::Medium,
        _ => {
            if backend.has_gpu() {
                ModelSize::Large
            } else {
                ModelSize::Medium
            }
        }
    }
}
