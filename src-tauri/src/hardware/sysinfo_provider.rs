use super::profile::GpuDescriptor;
use super::provider::HardwareInfoProvider;

pub struct SysinfoProvider;

impl HardwareInfoProvider for SysinfoProvider {
    fn gpus(&self) -> Vec<GpuDescriptor> {
        query_gpus_dxgi()
    }
}

/// Query GPU adapters via DXGI — works on all Windows 10/11 versions.
/// Returns name, vendor ID, and dedicated video memory for each adapter.
fn query_gpus_dxgi() -> Vec<GpuDescriptor> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ERROR_NOT_FOUND};

        let factory: IDXGIFactory1 = match unsafe { CreateDXGIFactory1() } {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };

        let mut gpus = Vec::new();
        let mut i = 0u32;

        loop {
            let adapter = unsafe { factory.EnumAdapters1(i) };
            match adapter {
                Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(_) => break,
                Ok(adapter) => {
                    let desc = match unsafe { adapter.GetDesc1() } {
                        Ok(d) => d,
                        Err(_) => { i += 1; continue; }
                    };

                    // Skip software/Microsoft Basic Render Driver (Flags bit 2 = DXGI_ADAPTER_FLAG_SOFTWARE)
                    if desc.Flags & 2 != 0 {
                        i += 1;
                        continue;
                    }

                    let name = String::from_utf16_lossy(
                        &desc.Description.iter()
                            .copied()
                            .take_while(|&c| c != 0)
                            .collect::<Vec<u16>>()
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

        gpus
    }

    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}
