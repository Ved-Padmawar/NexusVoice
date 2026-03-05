use sysinfo::System;

use super::profile::GpuDescriptor;
use super::provider::HardwareInfoProvider;

pub struct SysinfoProvider {
    system: System,
}

impl SysinfoProvider {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        Self { system }
    }
}

impl Default for SysinfoProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareInfoProvider for SysinfoProvider {
    fn gpus(&self) -> Vec<GpuDescriptor> {
        let _ = &self.system;
        Vec::new()
    }
}
