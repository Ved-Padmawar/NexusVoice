//! Linux GPU enumeration via Vulkan (`ash`).
//!
//! Vulkan is the common denominator across NVIDIA/AMD/Intel on Linux and is
//! already part of the app's graphics stack. For each physical device we read
//! its name, PCI vendor id, and the largest `DEVICE_LOCAL` memory heap as VRAM.
//! CPU/software devices (no device-local heap) are skipped so they never win
//! model-size selection.

use crate::hardware::profile::GpuDescriptor;

use ash::vk;

pub fn query_gpus() -> Result<Vec<GpuDescriptor>, String> {
    // SAFETY: all calls below follow Vulkan's documented lifetime rules — the
    // Entry/Instance outlive every handle derived from them, and we only read
    // POD device/memory properties.
    //
    // A failure to load Vulkan or create an instance is a *probe failure*
    // (`Err`) — distinct from a successful enumeration that finds no usable GPU
    // (`Ok(vec![])`). The caller logs the former and continues on CPU rather
    // than conflating the two.
    unsafe { query_gpus_inner() }
}

unsafe fn query_gpus_inner() -> Result<Vec<GpuDescriptor>, String> {
    let entry = ash::Entry::load().map_err(|e| format!("Vulkan loader unavailable: {e}"))?;

    let app_info = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_0);
    let create_info = vk::InstanceCreateInfo::default().application_info(&app_info);
    let instance = entry
        .create_instance(&create_info, None)
        .map_err(|e| format!("Vulkan instance creation failed: {e}"))?;

    let result = (|| {
        let devices = instance
            .enumerate_physical_devices()
            .map_err(|e| format!("Vulkan device enumeration failed: {e}"))?;
        let mut gpus = Vec::new();

        for device in devices {
            let props = instance.get_physical_device_properties(device);
            let mem = instance.get_physical_device_memory_properties(device);

            // Largest DEVICE_LOCAL heap = dedicated VRAM (unified on integrated).
            let vram_bytes = mem.memory_heaps[..mem.memory_heap_count as usize]
                .iter()
                .filter(|h| h.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL))
                .map(|h| h.size)
                .max()
                .unwrap_or(0);

            if vram_bytes == 0 {
                continue; // no device-local memory → not a usable GPU
            }

            let name = c_str_to_string(&props.device_name);

            gpus.push(GpuDescriptor {
                name,
                vendor_id: Some(props.vendor_id),
                vram_bytes,
            });
        }

        Ok(gpus)
    })();

    instance.destroy_instance(None);
    result
}

/// Convert a fixed-size NUL-terminated C char array (Vulkan device name) to a
/// `String`, stopping at the first NUL.
fn c_str_to_string(buf: &[std::os::raw::c_char]) -> String {
    let bytes: Vec<u8> = buf
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}
