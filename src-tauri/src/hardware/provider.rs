use super::profile::GpuDescriptor;

pub trait HardwareInfoProvider {
    fn gpus(&self) -> Vec<GpuDescriptor>;
    fn total_ram_gb(&self) -> f32;
}
