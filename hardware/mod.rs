pub mod detector;
pub mod profile;
pub mod provider;
pub mod sysinfo_provider;

pub use detector::detect_profile;
#[allow(unused_imports)]
pub use profile::HardwareProfile;
pub use sysinfo_provider::SysinfoProvider;
