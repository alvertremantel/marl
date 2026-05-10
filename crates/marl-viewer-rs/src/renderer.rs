use std::borrow::Cow;
use std::error::Error;
use std::sync::Arc;

use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::args::{CellMode, ViewerArgs};
use crate::camera::{CameraBasis, camera_basis};
use crate::io::SnapshotPayload;

// ---------------------------------------------------------------------------
// ViewerParams — must be #[repr(C)], Pod, and mirror WGSL Params exactly
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ViewerParams {
    pub(crate) grid: [u32; 4],       // grid_x, grid_y, grid_z, s_ext
    pub(crate) render: [u32; 4],     // width, height, species, steps
    pub(crate) transfer: [f32; 4],   // exposure, density_scale, cell_alpha, _unused
    pub(crate) axis_scale: [f32; 4], // grid / max_dim, 0
    pub(crate) cam_right: [f32; 4],  // right.xyz, right.w = zoom
    pub(crate) cam_up: [f32; 4],     // up.xyz, 0
    pub(crate) cam_dir: [f32; 4],    // dir.xyz, 0
    pub(crate) options: [u32; 4],    // options.x = cells_enabled
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

pub(crate) struct Renderer {
    pub(crate) window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
    params: ViewerParams,
    _field_texture: wgpu::Texture,
    _field_view: wgpu::TextureView,
    _cell_texture: Option<wgpu::Texture>,
    _cell_view: Option<wgpu::TextureView>,
}

impl Renderer {
    pub(crate) async fn new(
        window: Arc<Window>,
        payload: SnapshotPayload,
        args: ViewerArgs,
    ) -> Result<Self, Box<dyn Error>> {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|_| "no compatible GPU adapter found")?;

        let info = adapter.get_info();
        eprintln!("[viewer] using adapter: {} ({:?})", info.name, info.backend);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("MARL Viewer Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
                ..Default::default()
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|format| format.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let present_mode = surface_caps
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::Fifo)
            .unwrap_or(surface_caps.present_modes[0]);
        let alpha_mode = surface_caps.alpha_modes[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Camera basis
        let cam: CameraBasis = camera_basis(args.view_mode);

        // Normalized box dimensions
        let max_dim = payload
            .meta
            .grid_x
            .max(payload.meta.grid_y)
            .max(payload.meta.grid_z) as f32;
        if max_dim <= 0.0 {
            return Err("zero max grid dimension".into());
        }
        let axis_scale = [
            payload.meta.grid_x as f32 / max_dim,
            payload.meta.grid_y as f32 / max_dim,
            payload.meta.grid_z as f32 / max_dim,
            0.0,
        ];

        let cells_enabled = if payload.cell_mode == CellMode::Off {
            0u32
        } else {
            1u32
        };

        let params = ViewerParams {
            grid: [
                payload.meta.grid_x,
                payload.meta.grid_y,
                payload.meta.grid_z,
                payload.meta.s_ext,
            ],
            render: [config.width, config.height, payload.species, payload.steps],
            transfer: [
                payload.exposure,
                payload.density_scale,
                payload.cell_alpha,
                0.0,
            ],
            axis_scale,
            cam_right: [cam.right[0], cam.right[1], cam.right[2], cam.zoom],
            cam_up: [cam.up[0], cam.up[1], cam.up[2], 0.0],
            cam_dir: [cam.dir[0], cam.dir[1], cam.dir[2], 0.0],
            options: [cells_enabled, 0, 0, 0],
        };

        // Field texture (existing)
        let (field_texture, field_view) = create_field_texture(&device, &queue, &payload)?;

        // Cell texture (new)
        let (cell_texture, cell_view) = if cells_enabled != 0 {
            let (tex, view) = create_cell_texture(&device, &queue, &payload, &args)?;
            (Some(tex), Some(view))
        } else {
            // Bind an empty placeholder so the shader binding is always valid
            let (tex, view) = create_empty_cell_texture(&device, &queue, &payload)?;
            (Some(tex), Some(view))
        };

        // Params buffer
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MARL Viewer Params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MARL Viewer Bind Group Layout"),
            entries: &[
                // binding 0: field texture (3D, R32Float)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 1: uniform params buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 2: cell texture (3D, Rgba8Uint)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let cell_view_ref = cell_view
            .as_ref()
            .expect("cell texture view must exist after creation");

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MARL Viewer Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&field_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(cell_view_ref),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("MARL Viewer Raymarch Shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("viewer_raymarch.wgsl"))),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("MARL Viewer Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("MARL Viewer Raymarch Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        window.set_title(&format!(
            "MARL Viewer - tick {} species {} view {:?} cells {:?} ({} cells, {}x{}x{})",
            payload.tick,
            payload.species,
            args.view_mode,
            args.cell_mode,
            payload.cells.len(),
            payload.meta.grid_x,
            payload.meta.grid_y,
            payload.meta.grid_z
        ));

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group,
            params_buffer,
            params,
            _field_texture: field_texture,
            _field_view: field_view,
            _cell_texture: cell_texture,
            _cell_view: cell_view,
        })
    }

    pub(crate) fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }

        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        self.params.render[0] = size.width;
        self.params.render[1] = size.height;
        self.queue
            .write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&self.params));
    }

    pub(crate) fn render(&mut self) -> RenderResult {
        let (frame, should_reconfigure) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return RenderResult::Skip;
            }
            wgpu::CurrentSurfaceTexture::Outdated
            | wgpu::CurrentSurfaceTexture::Lost
            | wgpu::CurrentSurfaceTexture::Validation => return RenderResult::Reconfigure,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("MARL Viewer Render Encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("MARL Viewer Raymarch Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.008,
                            g: 0.01,
                            b: 0.014,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        if should_reconfigure {
            RenderResult::Reconfigure
        } else {
            RenderResult::Drawn
        }
    }
}

// ---------------------------------------------------------------------------
// RenderResult
// ---------------------------------------------------------------------------

pub(crate) enum RenderResult {
    Drawn,
    Skip,
    Reconfigure,
}

// ---------------------------------------------------------------------------
// Field texture creation (preserved from Phase 1)
// ---------------------------------------------------------------------------

pub(crate) fn create_field_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    payload: &SnapshotPayload,
) -> Result<(wgpu::Texture, wgpu::TextureView), Box<dyn Error>> {
    let texture_width = payload
        .meta
        .grid_x
        .checked_mul(payload.meta.s_ext)
        .ok_or("field texture width overflows u32")?;
    let max_3d = device.limits().max_texture_dimension_3d;
    if texture_width > max_3d || payload.meta.grid_y > max_3d || payload.meta.grid_z > max_3d {
        return Err(format!(
            "field texture dimensions {}x{}x{} exceed this adapter's 3D texture limit of {}",
            texture_width, payload.meta.grid_y, payload.meta.grid_z, max_3d
        )
        .into());
    }
    let size = wgpu::Extent3d {
        width: texture_width,
        height: payload.meta.grid_y,
        depth_or_array_layers: payload.meta.grid_z,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("MARL Viewer Field Texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::R32Float,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &payload.field_bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(texture_width * 4),
            rows_per_image: Some(payload.meta.grid_y),
        },
        size,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("MARL Viewer Field Texture View"),
        dimension: Some(wgpu::TextureViewDimension::D3),
        ..Default::default()
    });
    Ok((texture, view))
}

// ---------------------------------------------------------------------------
// Cell texture creation
// ---------------------------------------------------------------------------

/// Build a 3D `Rgba8Uint` texture sized `(grid_x, grid_y, grid_z)`.
///
/// Each occupied voxel stores RGBA bytes encoding the cell marker color and
/// alpha. Unoccupied voxels are zeroed.
fn create_cell_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    payload: &SnapshotPayload,
    args: &ViewerArgs,
) -> Result<(wgpu::Texture, wgpu::TextureView), Box<dyn Error>> {
    let gx = payload.meta.grid_x as usize;
    let gy = payload.meta.grid_y as usize;
    let gz = payload.meta.grid_z as usize;

    let max_3d = device.limits().max_texture_dimension_3d;
    if gx as u32 > max_3d || gy as u32 > max_3d || gz as u32 > max_3d {
        return Err(format!(
            "cell texture dimensions {}x{}x{} exceed adapter 3D limit {}",
            gx, gy, gz, max_3d
        )
        .into());
    }

    let size = wgpu::Extent3d {
        width: gx as u32,
        height: gy as u32,
        depth_or_array_layers: gz as u32,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("MARL Viewer Cell Texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::Rgba8Uint,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let alpha_byte = (args.cell_alpha * 255.0).round() as u8;
    let data = build_cell_texture_data(gx, gy, gz, &payload.cells, payload.cell_mode, alpha_byte);

    let bytes_per_row = (gx as u32)
        .checked_mul(4)
        .ok_or("cell texture bytes_per_row overflow")?;

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(bytes_per_row),
            rows_per_image: Some(gy as u32),
        },
        size,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("MARL Viewer Cell Texture View"),
        dimension: Some(wgpu::TextureViewDimension::D3),
        ..Default::default()
    });

    Ok((texture, view))
}

/// Create a minimal all-zeros cell texture for when cells are disabled.
fn create_empty_cell_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    payload: &SnapshotPayload,
) -> Result<(wgpu::Texture, wgpu::TextureView), Box<dyn Error>> {
    let gx = payload.meta.grid_x;
    let gy = payload.meta.grid_y;
    let gz = payload.meta.grid_z;

    let size = wgpu::Extent3d {
        width: gx,
        height: gy,
        depth_or_array_layers: gz,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("MARL Viewer Empty Cell Texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::Rgba8Uint,
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let data = vec![0u8; (gx * gy * gz) as usize * 4];
    let bytes_per_row = gx
        .checked_mul(4)
        .ok_or("empty cell texture bytes_per_row overflow")?;
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(bytes_per_row),
            rows_per_image: Some(gy),
        },
        size,
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("MARL Viewer Empty Cell Texture View"),
        dimension: Some(wgpu::TextureViewDimension::D3),
        ..Default::default()
    });
    Ok((texture, view))
}

// ---------------------------------------------------------------------------
// Cell texture data builder (pure, testable)
// ---------------------------------------------------------------------------

use crate::io::LoadedCell;

pub(crate) fn build_cell_texture_data(
    gx: usize,
    gy: usize,
    gz: usize,
    cells: &[LoadedCell],
    mode: CellMode,
    alpha: u8,
) -> Vec<u8> {
    let voxel_count = gx
        .checked_mul(gy)
        .and_then(|v| v.checked_mul(gz))
        .unwrap_or(0);
    let mut data = vec![0u8; voxel_count * 4];

    match mode {
        CellMode::Off => {
            // already all zeros
        }
        CellMode::Starter => {
            for cell in cells {
                let x = cell.pos[0] as usize;
                let y = cell.pos[1] as usize;
                let z = cell.pos[2] as usize;
                if x >= gx || y >= gy || z >= gz {
                    continue;
                }
                let idx = ((z * gy + y) * gx + x) * 4;
                // If already populated (duplicate), keep first; log warning
                if data[idx + 3] != 0 {
                    // duplicate: keep the first, skip this one
                    continue;
                }
                let (r, g, b) = starter_color(cell.starter_type);
                data[idx] = r;
                data[idx + 1] = g;
                data[idx + 2] = b;
                data[idx + 3] = alpha;
            }
        }
        CellMode::Energy => {
            let max_energy = cells
                .iter()
                .map(|c| c.energy)
                .fold(0.0f32, |a, b| a.max(b))
                .max(1.0); // safe fallback
            for cell in cells {
                let x = cell.pos[0] as usize;
                let y = cell.pos[1] as usize;
                let z = cell.pos[2] as usize;
                if x >= gx || y >= gy || z >= gz {
                    continue;
                }
                let idx = ((z * gy + y) * gx + x) * 4;
                // For energy mode, keep the higher-energy cell on duplicate
                if data[idx + 3] != 0 {
                    continue; // first wins for simplicity
                }
                let t = (cell.energy / max_energy).clamp(0.0, 1.0);
                let (r, g, b) = energy_color(t);
                data[idx] = r;
                data[idx + 1] = g;
                data[idx + 2] = b;
                data[idx + 3] = alpha;
            }
        }
    }

    data
}

fn starter_color(starter_type: u8) -> (u8, u8, u8) {
    match starter_type {
        0 => (230, 60, 55),  // phototroph: red
        1 => (70, 220, 90),  // chemolithotroph: green
        2 => (75, 130, 255), // anaerobe: blue
        _ => (230, 75, 230), // unknown: magenta
    }
}

fn energy_color(t: f32) -> (u8, u8, u8) {
    // Dark purple (low energy) → yellow (high energy)
    let r = ((30.0 + t * 225.0).round() as u8).min(255);
    let g = ((10.0 + t * 245.0).round() as u8).min(255);
    let b = ((60.0 * (1.0 - t)).round() as u8).min(255);
    (r, g, b)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_params_size_multiple_of_16() {
        assert_eq!(std::mem::size_of::<ViewerParams>() % 16, 0);
    }

    #[test]
    fn empty_cell_data_all_zeros() {
        let data = build_cell_texture_data(2, 2, 2, &[], CellMode::Starter, 200);
        assert_eq!(data.len(), 2 * 2 * 2 * 4);
        assert!(data.iter().all(|&b| b == 0));
    }

    #[test]
    fn starter_voxel_at_origin() {
        let cell = LoadedCell {
            pos: [0, 0, 0],
            lineage_id: 1,
            starter_type: 0,
            energy: 1.0,
        };
        let data = build_cell_texture_data(2, 2, 2, &[cell], CellMode::Starter, 200);
        //  4 bytes per voxel = RGBA
        assert_eq!(data[0], 230); // phototroph red
        assert_eq!(data[1], 60);
        assert_eq!(data[2], 55);
        assert_eq!(data[3], 200); // alpha
        // Rest zero
        for i in 4..data.len() {
            assert_eq!(data[i], 0, "byte {i} should be zero");
        }
    }

    #[test]
    fn starter_voxel_different_positions() {
        let cells = vec![
            LoadedCell {
                pos: [0, 0, 0],
                lineage_id: 1,
                starter_type: 1,
                energy: 1.0,
            },
            LoadedCell {
                pos: [1, 0, 0],
                lineage_id: 2,
                starter_type: 2,
                energy: 1.0,
            },
        ];
        let data = build_cell_texture_data(2, 1, 1, &cells, CellMode::Starter, 255);
        // cell 0 at idx 0: green
        assert_eq!(&data[0..4], &[70, 220, 90, 255]);
        // cell 1 at idx 4: blue
        assert_eq!(&data[4..8], &[75, 130, 255, 255]);
    }

    #[test]
    fn energy_voxel_color() {
        let cell = LoadedCell {
            pos: [0, 0, 0],
            lineage_id: 1,
            starter_type: 0,
            energy: 5.0,
        };
        let cells = vec![cell];
        let data = build_cell_texture_data(2, 2, 2, &cells, CellMode::Energy, 128);
        // max_energy = 5.0, so t = 1.0 => full energy color
        assert_eq!(data[3], 128); // alpha
        // R,G,B should be near (255, 255, 0) for t=1
        assert!(data[0] > 200);
        assert!(data[1] > 200);
        assert!(data[2] < 30);
    }

    #[test]
    fn duplicate_position_first_wins_starter() {
        let cells = vec![
            LoadedCell {
                pos: [0, 0, 0],
                lineage_id: 1,
                starter_type: 0,
                energy: 1.0,
            },
            LoadedCell {
                pos: [0, 0, 0],
                lineage_id: 2,
                starter_type: 1,
                energy: 2.0,
            },
        ];
        let data = build_cell_texture_data(2, 2, 2, &cells, CellMode::Starter, 200);
        // First cell (phototroph) should win
        assert_eq!(data[0], 230);
        assert_eq!(data[1], 60);
        assert_eq!(data[2], 55);
    }

    #[test]
    fn out_of_bounds_pos_ignored() {
        let cell = LoadedCell {
            pos: [2, 0, 0], // x >= gx=2
            lineage_id: 1,
            starter_type: 0,
            energy: 1.0,
        };
        let data = build_cell_texture_data(2, 2, 2, &[cell], CellMode::Starter, 200);
        assert!(data.iter().all(|&b| b == 0));
    }

    #[test]
    fn cell_mode_off_always_empty() {
        let cell = LoadedCell {
            pos: [0, 0, 0],
            lineage_id: 1,
            starter_type: 0,
            energy: 1.0,
        };
        let data = build_cell_texture_data(2, 2, 2, &[cell], CellMode::Off, 200);
        assert!(data.iter().all(|&b| b == 0));
    }
}
