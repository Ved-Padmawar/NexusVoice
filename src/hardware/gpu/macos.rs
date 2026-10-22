//! macOS GPU enumeration via Metal.
//!
//! Apple Silicon uses unified memory, so there is no dedicated VRAM figure;
//! `recommendedMaxWorkingSetSize` is the GPU's usable working-set budget and is
//! the correct analogue for model-size selection. The Apple vendor id routes
//! `map_execution_provider` to the Metal backend.

use crate::hardware::profile::GpuDescriptor;

/// Apple PCI vendor id — matches `APPLE_VENDOR_ID` in `detector.rs` so the
/// execution provider resolves to `metal`.
const APPLE_VENDOR_ID: u32 = 0x106B;

pub fn query_gpus() -> Result<Vec<GpuDescriptor>, String> {
    // Metal enumeration does not fail; an empty list simply means no Metal
    // device (treated as CPU by the caller).
    Ok(metal::Device::all()
        .into_iter()
        .map(|device| {
            let working_set = device.recommended_max_working_set_size();
            GpuDescriptor {
                name: device.name().to_string(),
                vendor_id: Some(APPLE_VENDOR_ID),
                vram_bytes: working_set,
            }
        })
        .collect())
}
