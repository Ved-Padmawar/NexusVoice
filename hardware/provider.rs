use super::profile::GpuDescriptor;

pub trait HardwareInfoProvider {
  fn gpus(&self) -> Vec<GpuDescriptor>;
}
