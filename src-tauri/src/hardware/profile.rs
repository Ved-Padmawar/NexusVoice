#[derive(Debug, Clone)]
pub struct HardwareProfile {
    #[allow(dead_code)]
    pub gpu_type: String,
    #[allow(dead_code)]
    pub vram_gb: f32,
    pub execution_provider: String,
}

#[derive(Debug, Clone)]
pub struct GpuDescriptor {
    pub name: String,
    pub vendor_id: Option<u32>,
    pub vram_bytes: u64,
}
