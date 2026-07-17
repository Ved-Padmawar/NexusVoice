use super::profile::{GpuDescriptor, HardwareProfile};
use super::provider::HardwareInfoProvider;

const NVIDIA_VENDOR_ID: u32 = 0x10DE;
const AMD_VENDOR_ID: u32 = 0x1002;
const INTEL_VENDOR_ID: u32 = 0x8086;
const APPLE_VENDOR_ID: u32 = 0x106B;

pub fn detect_profile<P: HardwareInfoProvider>(provider: &P) -> HardwareProfile {
    // A probe *failure* is logged and treated as CPU so the app still runs — but
    // it is no longer silently indistinguishable from a genuinely GPU-less
    // machine (`Ok(empty)`), which is the legitimate CPU case.
    let gpus = match provider.gpus() {
        Ok(gpus) => gpus,
        Err(e) => {
            log::warn!("GPU probe failed ({e}) — falling back to CPU execution");
            Vec::new()
        }
    };
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
    #[allow(clippy::cast_precision_loss)] // VRAM values fit f32 mantissa at GB scale
    let gb = bytes as f32 / 1_073_741_824.0;
    (gb * 10.0).round() / 10.0
}

fn map_execution_provider(gpu: &GpuDescriptor) -> String {
    if let Some(vendor_id) = gpu.vendor_id {
        return match vendor_id {
            NVIDIA_VENDOR_ID => "cuda".to_string(),
            AMD_VENDOR_ID | INTEL_VENDOR_ID => "vulkan".to_string(),
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
#[allow(clippy::float_cmp)] // test asserts compare exact-representable values passed through unchanged
#[path = "../../tests/unit/hardware/detector.rs"]
mod tests;
