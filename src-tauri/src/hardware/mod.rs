pub mod detector;
pub mod profile;
pub mod provider;
pub mod sysinfo_provider;

pub use detector::detect_profile;
pub use profile::HardwareProfile;
pub use sysinfo_provider::SysinfoProvider;

use std::sync::OnceLock;

static CACHED_PROFILE: OnceLock<HardwareProfile> = OnceLock::new();

/// Returns the hardware profile, computing it once and caching for the process lifetime.
/// Safe because hardware changes require a reboot (which resets the cache).
pub fn cached_profile() -> &'static HardwareProfile {
    CACHED_PROFILE.get_or_init(|| detect_profile(&SysinfoProvider))
}
