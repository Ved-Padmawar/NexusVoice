//! Native GPU enumeration, one real backend per OS.
//!
//! Each backend returns `Ok` with the discrete/integrated adapters (name, PCI
//! vendor id, device-local VRAM) — an empty vec meaning "no GPU present" — or
//! `Err` if the probe API itself failed. Software/CPU adapters are excluded so
//! they never win model-size selection. The `compile_error!` guard makes an
//! unsupported target fail to build rather than silently degrade.

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
pub use windows::query_gpus;
#[cfg(target_os = "macos")]
pub use macos::query_gpus;
#[cfg(target_os = "linux")]
pub use linux::query_gpus;

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
compile_error!("NexusVoice supports Windows, macOS, and Linux only");

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn query_gpus() -> Result<Vec<super::profile::GpuDescriptor>, String> {
    unreachable!("guarded by compile_error! above")
}
