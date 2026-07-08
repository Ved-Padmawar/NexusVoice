//! NVIDIA speech-model catalog used by both model management and inference.

const REPO: &str = "https://huggingface.co/mudler/parakeet-cpp-gguf/resolve/main";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Tiny,
    Small,
    Medium,
    Turbo,
    Large,
}

pub const ALL_MODELS: [Model; 5] = [
    Model::Tiny,
    Model::Small,
    Model::Medium,
    Model::Turbo,
    Model::Large,
];

impl Model {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Tiny => "parakeet-tdt-ctc-110m",
            Self::Small => "parakeet-realtime-eou-120m",
            Self::Medium => "parakeet-tdt-0.6b-v3",
            Self::Turbo => "nemotron-3.5-asr-0.6b",
            Self::Large => "parakeet-tdt-1.1b",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Tiny => "Tiny — Parakeet TDT-CTC 110M",
            Self::Small => "Small — Parakeet Realtime EOU 120M",
            Self::Medium => "Medium — Parakeet TDT 0.6B v3",
            Self::Turbo => "Turbo — Nemotron 3.5 ASR 0.6B",
            Self::Large => "Large — Parakeet TDT 1.1B",
        }
    }

    pub const fn filename(self) -> &'static str {
        match self {
            Self::Tiny => "tdt_ctc-110m-q5_k.gguf",
            Self::Small => "realtime_eou_120m-v1-q5_k.gguf",
            Self::Medium => "tdt-0.6b-v3-q5_k.gguf",
            Self::Turbo => "nemotron-3.5-asr-streaming-0.6b-q5_k.gguf",
            Self::Large => "tdt-1.1b-q5_k.gguf",
        }
    }

    pub const fn size_bytes(self) -> u64 {
        match self {
            Self::Tiny => 143_290_496,
            Self::Small => 141_151_648,
            Self::Medium => 741_867_360,
            Self::Turbo => 784_801_888,
            Self::Large => 1_207_914_592,
        }
    }

    pub fn url(self) -> String {
        format!("{REPO}/{}", self.filename())
    }

    pub fn from_id(id: &str) -> Option<Self> {
        ALL_MODELS.into_iter().find(|model| model.id() == id)
    }
}

pub fn recommended_model() -> Model {
    let profile = crate::hardware::cached_profile();
    if profile.vram_gb >= 8.0 || (profile.vram_gb < 1.0 && profile.ram_gb >= 24.0) {
        Model::Turbo
    } else if profile.vram_gb >= 4.0 || profile.ram_gb >= 12.0 {
        Model::Medium
    } else {
        Model::Tiny
    }
}

pub fn selected_model(override_id: Option<&str>) -> Model {
    override_id
        .and_then(Model::from_id)
        .unwrap_or_else(recommended_model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_filenames_are_unique() {
        for (index, model) in ALL_MODELS.iter().enumerate() {
            assert!(Model::from_id(model.id()).is_some());
            assert!(ALL_MODELS[index + 1..]
                .iter()
                .all(|other| { other.id() != model.id() && other.filename() != model.filename() }));
        }
    }
}
