use super::profile::GpuDescriptor;

pub trait HardwareInfoProvider {
    /// Enumerate GPU adapters.
    ///
    /// `Ok(vec)` is an authoritative list — an empty vec means "no GPU present"
    /// (a valid CPU-only machine). `Err` means the probe itself failed (driver/
    /// API init error); the caller logs it and continues on CPU rather than
    /// silently treating a failure as "no GPU".
    fn gpus(&self) -> Result<Vec<GpuDescriptor>, String>;
    fn total_ram_gb(&self) -> f32;
}
