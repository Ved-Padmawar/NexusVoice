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
fn intel_maps_to_vulkan() {
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

#[test]
fn amd_maps_to_vulkan() {
    let provider = MockProvider {
        gpus: vec![GpuDescriptor {
            name: "Radeon RX 7800".to_string(),
            vendor_id: Some(AMD_VENDOR_ID),
            vram_bytes: 16 * 1_073_741_824,
        }],
        ram_gb: 32.0,
    };
    assert_eq!(detect_profile(&provider).execution_provider, "vulkan");
}

#[test]
fn an_unrecognized_vendor_id_falls_back_to_cpu() {
    // Better to run slowly on the CPU than to load a backend the GPU cannot run.
    let provider = MockProvider {
        gpus: vec![GpuDescriptor {
            name: "Mystery Accelerator".to_string(),
            vendor_id: Some(0xDEAD),
            vram_bytes: 8 * 1_073_741_824,
        }],
        ram_gb: 16.0,
    };
    assert_eq!(detect_profile(&provider).execution_provider, "cpu");
}

#[test]
fn a_missing_vendor_id_is_resolved_from_the_device_name() {
    // Some probes report no vendor id at all; the name is the only signal left.
    let by_name = |name: &str| {
        let provider = MockProvider {
            gpus: vec![GpuDescriptor {
                name: name.to_string(),
                vendor_id: None,
                vram_bytes: 8 * 1_073_741_824,
            }],
            ram_gb: 16.0,
        };
        detect_profile(&provider).execution_provider
    };

    assert_eq!(by_name("NVIDIA GeForce RTX 4070"), "cuda");
    assert_eq!(by_name("AMD Radeon Graphics"), "vulkan");
    assert_eq!(by_name("Radeon RX 6600"), "vulkan");
    assert_eq!(by_name("Intel Arc A770"), "vulkan");
    assert_eq!(by_name("Apple M3 Pro"), "metal");
    assert_eq!(by_name("Some Unknown Display Adapter"), "cpu");
}

#[test]
fn name_matching_ignores_case() {
    // Vendor strings arrive in whatever case the driver reports.
    let provider = MockProvider {
        gpus: vec![GpuDescriptor {
            name: "nvidia geforce gtx 1660".to_string(),
            vendor_id: None,
            vram_bytes: 6 * 1_073_741_824,
        }],
        ram_gb: 16.0,
    };
    assert_eq!(detect_profile(&provider).execution_provider, "cuda");
}

#[test]
fn a_vendor_id_outranks_a_contradicting_device_name() {
    // The id is authoritative; a rebranded OEM name must not override it.
    let provider = MockProvider {
        gpus: vec![GpuDescriptor {
            name: "NVIDIA-branded AMD reference board".to_string(),
            vendor_id: Some(AMD_VENDOR_ID),
            vram_bytes: 8 * 1_073_741_824,
        }],
        ram_gb: 16.0,
    };
    assert_eq!(detect_profile(&provider).execution_provider, "vulkan");
}

#[test]
fn vram_is_reported_in_gb_to_one_decimal() {
    let provider = MockProvider {
        gpus: vec![GpuDescriptor {
            name: "Card".to_string(),
            vendor_id: Some(NVIDIA_VENDOR_ID),
            // 6.5 GiB
            vram_bytes: 6_979_321_856,
        }],
        ram_gb: 16.0,
    };
    assert_eq!(detect_profile(&provider).vram_gb, 6.5);
}
