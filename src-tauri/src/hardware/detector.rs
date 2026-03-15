use super::profile::{GpuDescriptor, HardwareProfile};
use super::provider::HardwareInfoProvider;

const NVIDIA_VENDOR_ID: u32 = 0x10DE;
const AMD_VENDOR_ID: u32 = 0x1002;
const INTEL_VENDOR_ID: u32 = 0x8086;
const APPLE_VENDOR_ID: u32 = 0x106B;

pub fn detect_profile<P: HardwareInfoProvider>(provider: &P) -> HardwareProfile {
    let gpus = provider.gpus();
    let ram_gb = provider.total_ram_gb();

    if gpus.is_empty() {
        return HardwareProfile {
            gpu_type: "cpu".to_string(),
            vram_gb: 0.0,
            ram_gb,
            execution_provider: "cpu".to_string(),
        };
    }

    let best_gpu = gpus.into_iter().max_by_key(|gpu| gpu.vram_bytes).unwrap();
    let vram_gb = bytes_to_gb(best_gpu.vram_bytes);
    let execution_provider = map_execution_provider(&best_gpu);

    HardwareProfile {
        gpu_type: best_gpu.name,
        vram_gb,
        ram_gb,
        execution_provider,
    }
}

fn bytes_to_gb(bytes: u64) -> f32 {
    let gb = bytes as f32 / 1_073_741_824.0;
    (gb * 10.0).round() / 10.0
}

fn map_execution_provider(gpu: &GpuDescriptor) -> String {
    if let Some(vendor_id) = gpu.vendor_id {
        return match vendor_id {
            NVIDIA_VENDOR_ID => "cuda".to_string(),
            AMD_VENDOR_ID => "vulkan".to_string(),
            INTEL_VENDOR_ID => "vulkan".to_string(),
            APPLE_VENDOR_ID => "metal".to_string(),
            _ => "cpu".to_string(),
        };
    }

    let name = gpu.name.to_lowercase();
    if name.contains("nvidia") {
        "cuda".to_string()
    } else if name.contains("amd") || name.contains("radeon") || name.contains("intel") {
        "vulkan".to_string()
    } else if name.contains("apple") || name.contains("metal") {
        "metal".to_string()
    } else {
        "cpu".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockProvider {
        gpus: Vec<GpuDescriptor>,
        ram_gb: f32,
    }

    impl HardwareInfoProvider for MockProvider {
        fn gpus(&self) -> Vec<GpuDescriptor> {
            self.gpus.clone()
        }
        fn total_ram_gb(&self) -> f32 {
            self.ram_gb
        }
    }

    #[test]
    fn no_gpu_defaults_to_cpu() {
        let provider = MockProvider { gpus: vec![], ram_gb: 16.0 };
        let profile = detect_profile(&provider);
        assert_eq!(profile.gpu_type, "cpu");
        assert_eq!(profile.execution_provider, "cpu");
        assert_eq!(profile.vram_gb, 0.0);
        assert_eq!(profile.ram_gb, 16.0);
    }

    #[test]
    fn selects_highest_vram_gpu() {
        let provider = MockProvider {
            gpus: vec![
                GpuDescriptor {
                    name: "Low".to_string(),
                    vendor_id: Some(NVIDIA_VENDOR_ID),
                    vram_bytes: 2 * 1_073_741_824,
                },
                GpuDescriptor {
                    name: "High".to_string(),
                    vendor_id: Some(NVIDIA_VENDOR_ID),
                    vram_bytes: 8 * 1_073_741_824,
                },
            ],
            ram_gb: 32.0,
        };
        let profile = detect_profile(&provider);
        assert_eq!(profile.gpu_type, "High");
        assert_eq!(profile.execution_provider, "cuda");
        assert_eq!(profile.vram_gb, 8.0);
    }

    #[test]
    fn intel_maps_to_directml() {
        let provider = MockProvider {
            gpus: vec![GpuDescriptor {
                name: "Intel".to_string(),
                vendor_id: Some(INTEL_VENDOR_ID),
                vram_bytes: 1_073_741_824,
            }],
            ram_gb: 16.0,
        };
        let profile = detect_profile(&provider);
        assert_eq!(profile.execution_provider, "vulkan");
    }
}
