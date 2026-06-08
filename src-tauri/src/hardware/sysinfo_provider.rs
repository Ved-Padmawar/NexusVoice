//! Cross-platform hardware probe backing [`HardwareInfoProvider`].
//!
//! RAM is read via the `sysinfo` crate on every OS. GPU enumeration (name,
//! vendor id, real VRAM) is delegated to a `cfg`-selected native backend in
//! [`gpu`] — DXGI on Windows, Metal on macOS, Vulkan on Linux. There is one
//! complete real implementation per OS; no target returns an empty/zero stub.
//!
//! VRAM matters functionally: `inference::provider` selects the Whisper model
//! size from it, so a wrong/zero value silently downgrades transcription
//! quality. That is why GPU memory is queried natively rather than via a
//! lowest-common-denominator abstraction.

use super::gpu;
use super::profile::GpuDescriptor;
use super::provider::HardwareInfoProvider;

pub struct SysinfoProvider;

impl HardwareInfoProvider for SysinfoProvider {
    fn gpus(&self) -> Result<Vec<GpuDescriptor>, String> {
        gpu::query_gpus()
    }

    fn total_ram_gb(&self) -> f32 {
        query_total_ram_gb()
    }
}

/// Total physical RAM in GB (one decimal place), via `sysinfo`. Cross-OS.
fn query_total_ram_gb() -> f32 {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_memory();
    #[allow(clippy::cast_precision_loss)] // RAM value fits f32 at GB scale
    let gb = sys.total_memory() as f32 / 1_073_741_824.0;
    (gb * 10.0).round() / 10.0
}
