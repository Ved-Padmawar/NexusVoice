use super::profile::GpuDescriptor;
use super::provider::HardwareInfoProvider;

pub struct SysinfoProvider;

impl SysinfoProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SysinfoProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareInfoProvider for SysinfoProvider {
    fn gpus(&self) -> Vec<GpuDescriptor> {
        query_gpus_wmic()
    }
}

/// Query GPU info via `wmic` on Windows.
/// Returns GPU name and dedicated VRAM (AdapterRAM field).
/// Falls back to empty vec on any error so detection gracefully degrades to CPU.
fn query_gpus_wmic() -> Vec<GpuDescriptor> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        let output = Command::new("wmic")
            .args(["path", "win32_VideoController", "get", "Name,AdapterRAM,PNPDeviceID", "/format:csv"])
            .output();

        let output = match output {
            Ok(o) if o.status.success() => o,
            _ => return Vec::new(),
        };

        let text = String::from_utf8_lossy(&output.stdout);
        let mut gpus = Vec::new();

        for line in text.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() { continue; }

            // CSV columns: Node,AdapterRAM,Name,PNPDeviceID
            let cols: Vec<&str> = line.splitn(5, ',').collect();
            if cols.len() < 4 { continue; }

            let adapter_ram_str = cols[1].trim();
            let name = cols[2].trim().to_string();
            let pnp_id = cols[3].trim().to_lowercase();

            if name.is_empty() { continue; }

            // Skip Microsoft Basic Display / software renderers
            let name_lower = name.to_lowercase();
            if name_lower.contains("microsoft basic") || name_lower.contains("basic display") {
                continue;
            }

            let vram_bytes: u64 = adapter_ram_str.parse().unwrap_or(0);

            // Determine vendor_id from PNP device ID (e.g. PCI\VEN_10DE&DEV_...)
            let vendor_id = if pnp_id.contains("ven_10de") {
                Some(0x10DE_u32) // NVIDIA
            } else if pnp_id.contains("ven_1002") {
                Some(0x1002_u32) // AMD
            } else if pnp_id.contains("ven_8086") {
                Some(0x8086_u32) // Intel
            } else {
                // Fallback: infer from name string
                if name_lower.contains("nvidia") || name_lower.contains("geforce") || name_lower.contains("quadro") || name_lower.contains("rtx") || name_lower.contains("gtx") {
                    Some(0x10DE_u32)
                } else if name_lower.contains("amd") || name_lower.contains("radeon") || name_lower.contains("rx ") {
                    Some(0x1002_u32)
                } else if name_lower.contains("intel") {
                    Some(0x8086_u32)
                } else {
                    None
                }
            };

            gpus.push(GpuDescriptor { name, vendor_id, vram_bytes });
        }

        gpus
    }

    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}
