use std::borrow::Cow;
use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

const DEFAULT_OUTPUT_DIR: &str = "output/run_128x128x64";

#[cfg(not(target_endian = "little"))]
compile_error!("the MARL viewer expects little-endian f32 field dumps");

#[derive(Debug, Clone)]
struct ViewerArgs {
    output_dir: PathBuf,
    tick: u64,
    species: u32,
    exposure: f32,
    density_scale: f32,
    steps: u32,
}

impl ViewerArgs {
    fn parse() -> Result<Self, String> {
        let mut output_dir: Option<PathBuf> = None;
        let mut tick = 0;
        let mut species = 1;
        let mut exposure: f32 = 18.0;
        let mut density_scale: f32 = 2.0;
        let mut steps = 160;

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => return Err(usage()),
                "--dir" => {
                    output_dir = Some(PathBuf::from(next_value(&mut args, "--dir")?));
                }
                "--tick" => {
                    tick = parse_value(&mut args, "--tick")?;
                }
                "--species" => {
                    species = parse_value(&mut args, "--species")?;
                }
                "--exposure" => {
                    exposure = parse_value(&mut args, "--exposure")?;
                }
                "--scale" => {
                    density_scale = parse_value(&mut args, "--scale")?;
                }
                "--steps" => {
                    steps = parse_value(&mut args, "--steps")?;
                }
                _ if arg.starts_with('-') => {
                    return Err(format!("unknown argument: {arg}\n\n{}", usage()));
                }
                _ => {
                    if output_dir.is_some() {
                        return Err(format!("unexpected extra path: {arg}\n\n{}", usage()));
                    }
                    output_dir = Some(PathBuf::from(arg));
                }
            }
        }

        if steps == 0 {
            return Err("--steps must be greater than zero".to_string());
        }
        if !exposure.is_finite() || exposure <= 0.0 {
            return Err("--exposure must be a positive finite number".to_string());
        }
        if !density_scale.is_finite() || density_scale <= 0.0 {
            return Err("--scale must be a positive finite number".to_string());
        }

        Ok(Self {
            output_dir: output_dir.unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT_DIR)),
            tick,
            species,
            exposure,
            density_scale,
            steps,
        })
    }
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value for {flag}\n\n{}", usage()))
}

fn parse_value<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T, String> {
    let raw = next_value(args, flag)?;
    raw.parse()
        .map_err(|_| format!("invalid value for {flag}: {raw}"))
}

fn usage() -> String {
    format!(
        "Usage: cargo run --release --features viewer --bin marl-viewer -- [output-dir] [options]\n\n\
         Options:\n\
           --dir <path>       Output directory containing run_meta.json\n\
           --tick <n>         Field tick to load (default: 0)\n\
           --species <n>      Species index to render (default: 1)\n\
           --exposure <f>     Raymarch opacity multiplier (default: 18.0)\n\
           --scale <f>        Concentration-to-density scale (default: 2.0)\n\
           --steps <n>        Raymarch samples through z (default: 160)\n\n\
         If no output directory is supplied, `{DEFAULT_OUTPUT_DIR}` is used."
    )
}

#[derive(Debug, Deserialize)]
struct RunMeta {
    grid_x: u32,
    grid_y: u32,
    grid_z: u32,
    s_ext: u32,
    endianness: String,
    field_dtype: String,
    field_layout: String,
    field_byte_len: u64,
}

struct FieldPayload {
    meta: RunMeta,
    bytes: Vec<u8>,
    tick: u64,
    species: u32,
    exposure: f32,
    density_scale: f32,
    steps: u32,
}

fn load_field(args: &ViewerArgs) -> Result<FieldPayload, Box<dyn Error>> {
    let meta_path = args.output_dir.join("run_meta.json");
    let meta_bytes =
        fs::read(&meta_path).map_err(|e| format!("failed to read {}: {e}", meta_path.display()))?;
    let meta: RunMeta = serde_json::from_slice(&meta_bytes)
        .map_err(|e| format!("failed to parse {}: {e}", meta_path.display()))?;

    validate_meta(&meta)?;
    if args.species >= meta.s_ext {
        return Err(format!(
            "species {} is out of range for {} external species",
            args.species, meta.s_ext
        )
        .into());
    }

    let field_path = args
        .output_dir
        .join(format!("tick_{}.field.bin", args.tick));
    let bytes = fs::read(&field_path)
        .map_err(|e| format!("failed to read {}: {e}", field_path.display()))?;
    if bytes.len() as u64 != meta.field_byte_len {
        return Err(format!(
            "{} has {} bytes, expected {} from run_meta.json",
            field_path.display(),
            bytes.len(),
            meta.field_byte_len
        )
        .into());
    }

    Ok(FieldPayload {
        meta,
        bytes,
        tick: args.tick,
        species: args.species,
        exposure: args.exposure,
        density_scale: args.density_scale,
        steps: args.steps,
    })
}

fn validate_meta(meta: &RunMeta) -> Result<(), Box<dyn Error>> {
    if meta.endianness != "little" {
        return Err(format!("unsupported field endianness: {}", meta.endianness).into());
    }
    if meta.field_dtype != "f32" {
        return Err(format!("unsupported field dtype: {}", meta.field_dtype).into());
    }
    if meta.field_layout != "z_y_x_species" {
        return Err(format!("unsupported field layout: {}", meta.field_layout).into());
    }
    if meta.grid_x == 0 || meta.grid_y == 0 || meta.grid_z == 0 || meta.s_ext == 0 {
        return Err("run_meta.json contains a zero grid dimension or species count".into());
    }

    let expected = u64::from(meta.grid_x)
        .checked_mul(u64::from(meta.grid_y))
        .and_then(|n| n.checked_mul(u64::from(meta.grid_z)))
        .and_then(|n| n.checked_mul(u64::from(meta.s_ext)))
        .and_then(|n| n.checked_mul(4))
        .ok_or("field byte length overflows u64")?;
    if meta.field_byte_len != expected {
        return Err(format!(
            "run_meta.json field_byte_len is {}, expected {expected}",
            meta.field_byte_len
        )
        .into());
    }

    Ok(())
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ViewerParams {
    grid: [u32; 4],
    render: [u32; 4],
    transfer: [f32; 4],
}

struct Renderer {
    window: Arc<Window>,
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
}

impl Renderer {
    async fn new(window: Arc<Window>, payload: FieldPayload) -> Result<Self, Box<dyn Error>> {
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

        let params = ViewerParams {
            grid: [
                payload.meta.grid_x,
                payload.meta.grid_y,
                payload.meta.grid_z,
                payload.meta.s_ext,
            ],
            render: [config.width, config.height, payload.species, payload.steps],
            transfer: [payload.exposure, payload.density_scale, 0.0, 0.0],
        };

        let (field_texture, field_view) = create_field_texture(&device, &queue, &payload)?;
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MARL Viewer Params"),
            contents: bytemuck::bytes_of(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MARL Viewer Bind Group Layout"),
            entries: &[
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
            ],
        });
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
            "MARL Viewer - tick {} species {} ({}x{}x{})",
            payload.tick,
            payload.species,
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
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
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

    fn render(&mut self) -> RenderResult {
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

enum RenderResult {
    Drawn,
    Skip,
    Reconfigure,
}

fn create_field_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    payload: &FieldPayload,
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
        &payload.bytes,
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

struct ViewerApp {
    payload: Option<FieldPayload>,
    renderer: Option<Renderer>,
}

impl ApplicationHandler for ViewerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            return;
        }
        let Some(payload) = self.payload.take() else {
            event_loop.exit();
            return;
        };

        let attrs = Window::default_attributes()
            .with_title("MARL Viewer")
            .with_inner_size(PhysicalSize::new(1280, 720));
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(e) => {
                eprintln!("failed to create viewer window: {e}");
                event_loop.exit();
                return;
            }
        };

        match pollster::block_on(Renderer::new(window, payload)) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(e) => {
                eprintln!("failed to initialize viewer renderer: {e}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        if window_id != renderer.window.id() {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => renderer.resize(size),
            WindowEvent::ScaleFactorChanged { .. } => renderer.resize(renderer.window.inner_size()),
            WindowEvent::RedrawRequested => match renderer.render() {
                RenderResult::Drawn | RenderResult::Skip => {}
                RenderResult::Reconfigure => renderer.resize(renderer.window.inner_size()),
            },
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(renderer) = self.renderer.as_ref() {
            renderer.window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = match ViewerArgs::parse() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(if e.starts_with("Usage:") { 0 } else { 2 });
        }
    };
    let payload = load_field(&args)?;
    eprintln!(
        "[viewer] loaded tick {} species {} from {} ({} bytes)",
        payload.tick,
        payload.species,
        args.output_dir.display(),
        payload.bytes.len()
    );

    let event_loop = EventLoop::new()?;
    let mut app = ViewerApp {
        payload: Some(payload),
        renderer: None,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}
