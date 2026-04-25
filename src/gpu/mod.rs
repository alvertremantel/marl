//! GPU-accelerated compute path for MARL.

pub mod context;
pub mod field_diffusion;

pub use context::GpuError;
pub use field_diffusion::GpuFieldDiffuser;
