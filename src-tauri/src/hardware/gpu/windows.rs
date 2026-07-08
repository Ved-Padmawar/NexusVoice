//! Windows GPU enumeration via DXGI. Works on all Windows 10/11 versions and
//! reports name, vendor id, and dedicated VRAM for every hardware adapter.

use crate::hardware::profile::GpuDescriptor;

pub fn query_gpus() -> Result<Vec<GpuDescriptor>, String> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ERROR_NOT_FOUND};

    // Factory creation failing is a probe failure (not "no GPU") — surface it.
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }
        .map_err(|e| format!("DXGI factory creation failed: {e}"))?;

    let mut gpus = Vec::new();
    let mut i = 0u32;

    loop {
        let adapter = unsafe { factory.EnumAdapters1(i) };
        match adapter {
            Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(_) => break,
            Ok(adapter) => {
                let Ok(desc) = (unsafe { adapter.GetDesc1() }) else {
                    i += 1;
                    continue;
                };

                // Skip software/Microsoft Basic Render Driver (Flags bit 2 = DXGI_ADAPTER_FLAG_SOFTWARE)
                if desc.Flags & 2 != 0 {
                    i += 1;
                    continue;
                }

                let name = String::from_utf16_lossy(
                    &desc
                        .Description
                        .iter()
                        .copied()
                        .take_while(|&c| c != 0)
                        .collect::<Vec<u16>>(),
                );

                let name_lower = name.to_lowercase();
                if name_lower.contains("microsoft basic") || name_lower.contains("basic render") {
                    i += 1;
                    continue;
                }

                gpus.push(GpuDescriptor {
                    name,
                    vendor_id: Some(desc.VendorId),
                    vram_bytes: desc.DedicatedVideoMemory as u64,
                });
            }
        }
        i += 1;
    }

    Ok(gpus)
}
