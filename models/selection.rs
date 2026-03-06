#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSize {
    Tiny,
    Base,
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone)]
pub struct ModelSelection {
    pub size: ModelSize,
    pub reason: String,
}

use crate::hardware::HardwareProfile;

pub fn select_model(profile: &HardwareProfile, override_size: Option<ModelSize>) -> ModelSelection {
    if let Some(size) = override_size {
        return ModelSelection {
            size,
            reason: "override".to_string(),
        };
    }

    if profile.execution_provider == "cpu" {
        return ModelSelection {
            size: ModelSize::Tiny,
            reason: "cpu".to_string(),
        };
    }

    let size = if profile.vram_gb >= 8.0 {
        ModelSize::Large
    } else if profile.vram_gb >= 6.0 {
        ModelSize::Medium
    } else if profile.vram_gb >= 4.0 {
        ModelSize::Small
    } else if profile.vram_gb >= 2.0 {
        ModelSize::Base
    } else {
        ModelSize::Tiny
    };

    ModelSelection {
        size,
        reason: "auto".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(vram: f32, provider: &str) -> HardwareProfile {
        HardwareProfile {
            gpu_type: "Test".to_string(),
            vram_gb: vram,
            execution_provider: provider.to_string(),
        }
    }

    #[test]
    fn cpu_forces_tiny() {
        let selection = select_model(&profile(12.0, "cpu"), None);
        assert_eq!(selection.size, ModelSize::Tiny);
    }

    #[test]
    fn nvidia_8gb_large() {
        let selection = select_model(&profile(8.0, "cuda"), None);
        assert_eq!(selection.size, ModelSize::Large);
    }

    #[test]
    fn nvidia_6gb_medium() {
        let selection = select_model(&profile(6.0, "cuda"), None);
        assert_eq!(selection.size, ModelSize::Medium);
    }

    #[test]
    fn nvidia_4gb_small() {
        let selection = select_model(&profile(4.0, "cuda"), None);
        assert_eq!(selection.size, ModelSize::Small);
    }

    #[test]
    fn nvidia_low_vram_tiny() {
        let selection = select_model(&profile(3.5, "cuda"), None);
        assert_eq!(selection.size, ModelSize::Tiny);
    }

    #[test]
    fn override_wins() {
        let selection = select_model(&profile(12.0, "cuda"), Some(ModelSize::Small));
        assert_eq!(selection.size, ModelSize::Small);
        assert_eq!(selection.reason, "override");
    }
}
