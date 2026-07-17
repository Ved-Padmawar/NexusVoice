use super::*;

struct MockProvider {
    gpus: Vec<GpuDescriptor>,
    ram_gb: f32,
}

impl HardwareInfoProvider for MockProvider {
    fn gpus(&self) -> Result<Vec<GpuDescriptor>, String> {
        Ok(self.gpus.clone())
    }
    fn total_ram_gb(&self) -> f32 {
        self.ram_gb
    }
}

#[test]
fn no_gpu_defaults_to_cpu() {
    let provider = MockProvider {
        gpus: vec![],
        ram_gb: 16.0,
    };
    let profile = detect_profile(&provider);
    assert_eq!(profile.gpu_type, "cpu");
    assert_eq!(profile.execution_provider, "cpu");
    assert_eq!(profile.vram_gb, 0.0);
    assert_eq!(profile.ram_gb, 16.0);
}

#[test]
fn probe_failure_falls_back_to_cpu_without_panicking() {
    // A failed GPU probe (Err) must degrade to CPU — the app keeps running on
    // a machine where the GPU API is unavailable — and still report RAM.
    struct FailingProvider;
    impl HardwareInfoProvider for FailingProvider {
        fn gpus(&self) -> Result<Vec<GpuDescriptor>, String> {
            Err("driver init failed".to_string())
        }
        fn total_ram_gb(&self) -> f32 {
            8.0
        }
    }
    let profile = detect_profile(&FailingProvider);
    assert_eq!(profile.execution_provider, "cpu");
    assert_eq!(profile.ram_gb, 8.0);
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
