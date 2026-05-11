use std::fmt;

/// Errors returned by the GPU simulation path.
#[derive(Debug, Clone)]
pub enum GpuError {
    NoAdapter,
    DeviceRequest(String),
    BufferMap(String),
    InvalidInput(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAdapter => write!(f, "no compatible GPU adapter found"),
            Self::DeviceRequest(e) => write!(f, "failed to request GPU device: {e}"),
            Self::BufferMap(e) => write!(f, "failed to map GPU buffer: {e}"),
            Self::InvalidInput(e) => write!(f, "invalid GPU diffuser input: {e}"),
        }
    }
}

impl std::error::Error for GpuError {}

pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GpuContext {
    pub fn new() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .map_err(|_| GpuError::NoAdapter)?;

        let info = adapter.get_info();
        eprintln!("[gpu] using adapter: {} ({:?})", info.name, info.backend);

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("MARL GPU Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
            ..Default::default()
        }))
        .map_err(|e| GpuError::DeviceRequest(e.to_string()))?;

        Ok(Self { device, queue })
    }
}
