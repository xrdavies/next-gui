//! wgpu GPU rasterizer for next-gui.
//!
//! Consumes the same [`DrawCommand`] stream as `next-gui-render-soft` and draws into an
//! off-screen retained texture (Rgba8Unorm).
//!
//! Present paths ([`GpuPresentPath`]):
//! - [`GpuPresentPath::ReadbackSoftbuffer`] — CPU readback → `platform.present` (softbuffer).
//! - [`GpuPresentPath::NativeSurface`] — blit off-screen → `wgpu::Surface` → present
//!   (requires a GPU-first window without softbuffer; see `docs/office-line.md`).

#![deny(missing_docs)]

use std::fmt;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use next_gui_render::{DrawCommand, Renderer};
use next_gui_scene::{Color, DirtyRegion, Rect};
use wgpu::util::DeviceExt;

const SOLID_SHADER: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(@location(0) pos: vec2<f32>, @location(1) color: vec4<f32>) -> VsOut {
    var o: VsOut;
    o.pos = vec4<f32>(pos, 0.0, 1.0);
    o.color = color;
    return o;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

const BLIT_SHADER: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@group(0) @binding(0) var mask_tex: texture_2d<f32>;
@group(0) @binding(1) var mask_samp: sampler;

@vertex
fn vs_main(
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
) -> VsOut {
    var o: VsOut;
    o.pos = vec4<f32>(pos, 0.0, 1.0);
    o.uv = uv;
    o.color = color;
    return o;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let coverage = textureSample(mask_tex, mask_samp, in.uv).r;
    return vec4<f32>(in.color.rgb, in.color.a * coverage);
}
"#;

const COPY_SHADER: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    // Fullscreen triangle
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0),
    );
    var o: VsOut;
    o.pos = vec4<f32>(pos[idx], 0.0, 1.0);
    o.uv = uv[idx];
    return o;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(src_tex, src_samp, in.uv);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SolidVertex {
    pos: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlitVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

/// Error creating a GPU renderer (no adapter, device lost, etc.).
#[derive(Debug)]
pub struct GpuInitError {
    message: String,
}

impl GpuInitError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for GpuInitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for GpuInitError {}

/// How GPU frames reach the OS window.
///
/// Office prefers [`GpuPresentPath::NativeSurface`]; query via [`GpuRenderer::present_path`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuPresentPath {
    /// Off-screen wgpu → CPU readback → `platform.present` (softbuffer).
    ReadbackSoftbuffer,
    /// Off-screen wgpu → blit → `wgpu::Surface::present` (no CPU readback).
    NativeSurface,
}

struct TargetTextures {
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    staging: Option<wgpu::Buffer>,
}

struct NativeSurfaceState {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    copy_pipeline: wgpu::RenderPipeline,
    copy_bind_layout: wgpu::BindGroupLayout,
    copy_sampler: wgpu::Sampler,
}

/// GPU backend implementing [`Renderer`] via wgpu.
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    backend_name: String,
    width: u32,
    height: u32,
    clear: Color,
    pixels: Vec<u32>,
    target: Option<TargetTextures>,
    solid_pipeline: wgpu::RenderPipeline,
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_layout: wgpu::BindGroupLayout,
    blit_sampler: wgpu::Sampler,
    native: Option<NativeSurfaceState>,
}

impl GpuRenderer {
    /// Probe whether a wgpu adapter is available (no window required).
    pub fn adapter_available() -> bool {
        Self::try_new(1, 1, Color::rgb(0, 0, 0)).is_ok()
    }

    /// Create a readback renderer (softbuffer present via [`Renderer::frame_pixels`]).
    pub fn try_new(width: u32, height: u32, clear: Color) -> Result<Self, GpuInitError> {
        Self::create(width, height, clear, None)
    }

    /// Create a native-surface renderer for a GPU-first window (no softbuffer).
    ///
    /// `window` must outlive this renderer for the surface to stay valid; pass an
    /// `Rc`/`Arc` of the winit window (implements wgpu's `WindowHandle`).
    pub fn try_new_native(
        window: impl wgpu::WindowHandle + 'static,
        width: u32,
        height: u32,
        clear: Color,
    ) -> Result<Self, GpuInitError> {
        Self::create(width, height, clear, Some(Box::new(window)))
    }

    fn create(
        width: u32,
        height: u32,
        clear: Color,
        window: Option<Box<dyn wgpu::WindowHandle>>,
    ) -> Result<Self, GpuInitError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            backend_options: Default::default(),
        });

        let surface = if let Some(window) = window {
            Some(
                instance
                    .create_surface(window)
                    .map_err(|e| GpuInitError::new(format!("create_surface: {e}")))?,
            )
        } else {
            None
        };

        let adapter = {
            let mut found = None;
            for force_fallback in [false, true] {
                found = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: surface.as_ref(),
                    force_fallback_adapter: force_fallback,
                }));
                if found.is_some() {
                    break;
                }
            }
            found.ok_or_else(|| GpuInitError::new("no wgpu adapter (GPU unavailable)"))?
        };

        let info = adapter.get_info();
        let backend_name = format!("{:?}", info.backend);

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("next-gui-gpu"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|e| GpuInitError::new(format!("request_device: {e}")))?;

        let solid_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("solid"),
            source: wgpu::ShaderSource::Wgsl(SOLID_SHADER.into()),
        });
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
        });

        let solid_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("solid-layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let blit_bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit-bind"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let blit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit-layout"),
            bind_group_layouts: &[&blit_bind_layout],
            push_constant_ranges: &[],
        });

        let target_format = wgpu::TextureFormat::Rgba8Unorm;

        let solid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("solid-pipeline"),
            layout: Some(&solid_layout),
            vertex: wgpu::VertexState {
                module: &solid_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SolidVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &solid_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit-pipeline"),
            layout: Some(&blit_layout),
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<BlitVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2,
                        2 => Float32x4
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blit-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let native = if let Some(surface) = surface {
            let caps = surface.get_capabilities(&adapter);
            if caps.formats.is_empty() {
                return Err(GpuInitError::new("surface has no supported formats"));
            }
            // Prefer non-sRGB Unorm so surface presentation matches the offscreen
            // Rgba8Unorm pipeline and softbuffer readback (F04). sRGB swapchains apply
            // an extra encode and wash out Office colors.
            let format = caps
                .formats
                .iter()
                .copied()
                .find(|f| {
                    matches!(
                        f,
                        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                    )
                })
                .or_else(|| {
                    caps.formats.iter().copied().find(|f| {
                        matches!(
                            f,
                            wgpu::TextureFormat::Bgra8UnormSrgb
                                | wgpu::TextureFormat::Rgba8UnormSrgb
                        )
                    })
                })
                .unwrap_or(caps.formats[0]);

            let alpha_mode = caps
                .alpha_modes
                .iter()
                .copied()
                .find(|m| *m == wgpu::CompositeAlphaMode::Opaque)
                .unwrap_or(caps.alpha_modes[0]);

            let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
                wgpu::PresentMode::Fifo
            } else {
                caps.present_modes[0]
            };

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: width.max(1),
                height: height.max(1),
                present_mode,
                desired_maximum_frame_latency: 2,
                alpha_mode,
                view_formats: vec![],
            };
            surface.configure(&device, &config);

            let copy_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("copy"),
                source: wgpu::ShaderSource::Wgsl(COPY_SHADER.into()),
            });
            let copy_bind_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("copy-bind"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });
            let copy_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("copy-layout"),
                bind_group_layouts: &[&copy_bind_layout],
                push_constant_ranges: &[],
            });
            let copy_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("copy-pipeline"),
                layout: Some(&copy_layout),
                vertex: wgpu::VertexState {
                    module: &copy_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &copy_shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });
            let copy_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("copy-sampler"),
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });

            Some(NativeSurfaceState {
                surface,
                config,
                copy_pipeline,
                copy_bind_layout,
                copy_sampler,
            })
        } else {
            None
        };

        let mut gpu = Self {
            device,
            queue,
            backend_name,
            width: 0,
            height: 0,
            clear,
            pixels: Vec::new(),
            target: None,
            solid_pipeline,
            blit_pipeline,
            blit_bind_layout,
            blit_sampler,
            native,
        };
        gpu.resize(width, height);
        Ok(gpu)
    }

    /// Adapter backend label (e.g. `Vulkan`, `Gl`).
    pub fn backend_name(&self) -> &str {
        &self.backend_name
    }

    /// How this renderer presents frames.
    pub fn present_path(&self) -> GpuPresentPath {
        if self.native.is_some() {
            GpuPresentPath::NativeSurface
        } else {
            GpuPresentPath::ReadbackSoftbuffer
        }
    }

    /// Clear / background color.
    pub fn clear_color(&self) -> Color {
        self.clear
    }

    /// Update clear color.
    pub fn set_clear_color(&mut self, clear: Color) {
        self.clear = clear;
    }

    /// Upload CPU XRGB8888 pixels and present via the native surface (Soft-on-GPU window).
    ///
    /// No-op / error if this renderer is readback-only.
    pub fn present_xrgb_frame(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u32],
    ) -> Result<(), GpuInitError> {
        if self.native.is_none() {
            return Err(GpuInitError::new(
                "present_xrgb_frame requires NativeSurface path",
            ));
        }
        if width == 0 || height == 0 {
            return Ok(());
        }
        if pixels.len() < (width as usize) * (height as usize) {
            return Err(GpuInitError::new("pixel buffer too small"));
        }
        if width != self.width || height != self.height {
            self.resize(width, height);
        }
        self.ensure_target();
        let Some(target) = self.target.as_ref() else {
            return Ok(());
        };

        // Convert XRGB → RGBA bytes for write_texture.
        let mut rgba = vec![0u8; (width as usize) * (height as usize) * 4];
        for (i, px) in pixels[..(width as usize) * (height as usize)]
            .iter()
            .enumerate()
        {
            let r = ((*px >> 16) & 0xff) as u8;
            let g = ((*px >> 8) & 0xff) as u8;
            let b = (*px & 0xff) as u8;
            let o = i * 4;
            rgba[o] = r;
            rgba[o + 1] = g;
            rgba[o + 2] = b;
            rgba[o + 3] = 255;
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &target.color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.blit_offscreen_to_surface()
            .map_err(GpuInitError::new)?;
        Ok(())
    }

    fn ensure_target(&mut self) {
        if self.width == 0 || self.height == 0 {
            self.target = None;
            return;
        }
        let need_new = match &self.target {
            None => true,
            Some(t) => t.color.size().width != self.width || t.color.size().height != self.height,
        };
        if !need_new {
            return;
        }

        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::TEXTURE_BINDING;

        let color = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gpu-color"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage,
            view_formats: &[],
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());

        let staging = if self.native.is_none() {
            let unpadded_bytes_per_row = self.width * 4;
            let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
            let staging_size = padded_bytes_per_row as u64 * self.height as u64;
            Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu-staging"),
                size: staging_size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }))
        } else {
            None
        };

        self.target = Some(TargetTextures {
            color,
            color_view,
            staging,
        });
    }

    fn to_ndc(x: f32, y: f32, width: f32, height: f32) -> [f32; 2] {
        let nx = (x / width) * 2.0 - 1.0;
        let ny = 1.0 - (y / height) * 2.0;
        [nx, ny]
    }

    fn color_f32(c: Color) -> [f32; 4] {
        [
            c.r as f32 / 255.0,
            c.g as f32 / 255.0,
            c.b as f32 / 255.0,
            c.a as f32 / 255.0,
        ]
    }

    fn solid_quad(rect: &Rect, color: Color, fw: f32, fh: f32) -> [SolidVertex; 6] {
        let c = Self::color_f32(color);
        let x0 = rect.x;
        let y0 = rect.y;
        let x1 = rect.x + rect.width;
        let y1 = rect.y + rect.height;
        let p00 = Self::to_ndc(x0, y0, fw, fh);
        let p10 = Self::to_ndc(x1, y0, fw, fh);
        let p01 = Self::to_ndc(x0, y1, fw, fh);
        let p11 = Self::to_ndc(x1, y1, fw, fh);
        [
            SolidVertex { pos: p00, color: c },
            SolidVertex { pos: p10, color: c },
            SolidVertex { pos: p01, color: c },
            SolidVertex { pos: p10, color: c },
            SolidVertex { pos: p11, color: c },
            SolidVertex { pos: p01, color: c },
        ]
    }

    fn blit_quad(
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        color: Color,
        fw: f32,
        fh: f32,
    ) -> [BlitVertex; 6] {
        let c = Self::color_f32(color);
        let x0 = x as f32;
        let y0 = y as f32;
        let x1 = x0 + width as f32;
        let y1 = y0 + height as f32;
        let p00 = Self::to_ndc(x0, y0, fw, fh);
        let p10 = Self::to_ndc(x1, y0, fw, fh);
        let p01 = Self::to_ndc(x0, y1, fw, fh);
        let p11 = Self::to_ndc(x1, y1, fw, fh);
        [
            BlitVertex {
                pos: p00,
                uv: [0.0, 0.0],
                color: c,
            },
            BlitVertex {
                pos: p10,
                uv: [1.0, 0.0],
                color: c,
            },
            BlitVertex {
                pos: p01,
                uv: [0.0, 1.0],
                color: c,
            },
            BlitVertex {
                pos: p10,
                uv: [1.0, 0.0],
                color: c,
            },
            BlitVertex {
                pos: p11,
                uv: [1.0, 1.0],
                color: c,
            },
            BlitVertex {
                pos: p01,
                uv: [0.0, 1.0],
                color: c,
            },
        ]
    }

    fn scissor_from_rect(rect: &Rect, buf_w: u32, buf_h: u32) -> Option<(u32, u32, u32, u32)> {
        let (x, y, w, h) = rect.clip_to_buffer(buf_w, buf_h)?;
        if w == 0 || h == 0 {
            None
        } else {
            Some((x, y, w, h))
        }
    }

    fn readback_to_pixels(&mut self) {
        let Some(target) = self.target.as_ref() else {
            return;
        };
        let Some(staging) = target.staging.as_ref() else {
            return;
        };
        if self.width == 0 || self.height == 0 {
            return;
        }

        let unpadded = self.width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target.color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        if let Err(e) = rx.recv().unwrap_or(Err(wgpu::BufferAsyncError)) {
            eprintln!("next-gui-render-gpu: map_async failed: {e:?}");
            return;
        }

        let data = slice.get_mapped_range();
        let n = (self.width as usize) * (self.height as usize);
        if self.pixels.len() != n {
            self.pixels.resize(n, 0);
        }
        for y in 0..self.height as usize {
            let src_row = y * padded as usize;
            let dst_row = y * self.width as usize;
            for x in 0..self.width as usize {
                let i = src_row + x * 4;
                let r = data[i] as u32;
                let g = data[i + 1] as u32;
                let b = data[i + 2] as u32;
                self.pixels[dst_row + x] = (r << 16) | (g << 8) | b;
            }
        }
        drop(data);
        staging.unmap();
    }

    fn blit_offscreen_to_surface(&mut self) -> Result<(), String> {
        let Some(native) = self.native.as_ref() else {
            return Ok(());
        };
        let Some(target) = self.target.as_ref() else {
            return Ok(());
        };
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }

        let frame = match native.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                // Reconfigure and retry once.
                native.surface.configure(&self.device, &native.config);
                native
                    .surface
                    .get_current_texture()
                    .map_err(|e| format!("get_current_texture retry: {e}"))?
            }
            Err(e) => return Err(format!("get_current_texture: {e}")),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("copy-bg"),
            layout: &native.copy_bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&target.color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&native.copy_sampler),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("surface-blit"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("surface-copy"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&native.copy_pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }

    fn upload_mask(
        &self,
        width: u32,
        height: u32,
        pixels: &Arc<[u8]>,
    ) -> (wgpu::Texture, wgpu::BindGroup) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("alpha-mask"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width.max(1)),
                rows_per_image: Some(height.max(1)),
            },
            wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit-bg"),
            layout: &self.blit_bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
                },
            ],
        });
        (texture, bind)
    }

    fn configure_surface_size(&mut self, width: u32, height: u32) {
        if let Some(native) = self.native.as_mut() {
            let w = width.max(1);
            let h = height.max(1);
            if native.config.width != w || native.config.height != h {
                native.config.width = w;
                native.config.height = h;
                native.surface.configure(&self.device, &native.config);
            }
        }
    }
}

impl Renderer for GpuRenderer {
    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        let n = (width as usize).saturating_mul(height as usize);
        let clear_px = self.clear.to_xrgb8888();
        self.pixels.resize(n, clear_px);
        for p in &mut self.pixels {
            *p = clear_px;
        }
        self.configure_surface_size(width, height);
        self.ensure_target();
    }

    fn render(&mut self, commands: &[DrawCommand], dirty: &DirtyRegion) {
        if dirty.is_empty() || self.width == 0 || self.height == 0 {
            return;
        }
        self.ensure_target();
        let Some(target) = self.target.as_ref() else {
            return;
        };

        let fw = self.width as f32;
        let fh = self.height as f32;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu-frame"),
            });

        let mut mask_keep: Vec<(wgpu::Texture, wgpu::BindGroup)> = Vec::new();
        let mut solid_bufs: Vec<wgpu::Buffer> = Vec::new();
        let mut blit_bufs: Vec<wgpu::Buffer> = Vec::new();

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            for d in &dirty.rects {
                let Some((sx, sy, sw, sh)) = Self::scissor_from_rect(d, self.width, self.height)
                else {
                    continue;
                };
                pass.set_scissor_rect(sx, sy, sw, sh);

                let clear_verts = Self::solid_quad(d, self.clear, fw, fh);
                let clear_buf = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("clear-vb"),
                        contents: bytemuck::cast_slice(&clear_verts),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                pass.set_pipeline(&self.solid_pipeline);
                pass.set_vertex_buffer(0, clear_buf.slice(..));
                pass.draw(0..6, 0..1);
                solid_bufs.push(clear_buf);

                for cmd in commands {
                    match cmd {
                        DrawCommand::FillRect { rect, color } => {
                            if !rect.intersects(d) {
                                continue;
                            }
                            let verts = Self::solid_quad(rect, *color, fw, fh);
                            let vb = self.device.create_buffer_init(
                                &wgpu::util::BufferInitDescriptor {
                                    label: Some("fill-vb"),
                                    contents: bytemuck::cast_slice(&verts),
                                    usage: wgpu::BufferUsages::VERTEX,
                                },
                            );
                            pass.set_pipeline(&self.solid_pipeline);
                            pass.set_vertex_buffer(0, vb.slice(..));
                            pass.draw(0..6, 0..1);
                            solid_bufs.push(vb);
                        }
                        DrawCommand::BlitAlpha {
                            x,
                            y,
                            width,
                            height,
                            pixels,
                            color,
                        } => {
                            if *width == 0 || *height == 0 {
                                continue;
                            }
                            let bounds =
                                Rect::new(*x as f32, *y as f32, *width as f32, *height as f32);
                            if !bounds.intersects(d) {
                                continue;
                            }
                            if pixels.len() < (*width as usize) * (*height as usize) {
                                continue;
                            }
                            let (tex, bind) = self.upload_mask(*width, *height, pixels);
                            let verts =
                                Self::blit_quad(*x, *y, *width, *height, *color, fw, fh);
                            let vb = self.device.create_buffer_init(
                                &wgpu::util::BufferInitDescriptor {
                                    label: Some("blit-vb"),
                                    contents: bytemuck::cast_slice(&verts),
                                    usage: wgpu::BufferUsages::VERTEX,
                                },
                            );
                            pass.set_pipeline(&self.blit_pipeline);
                            pass.set_bind_group(0, &bind, &[]);
                            pass.set_vertex_buffer(0, vb.slice(..));
                            pass.draw(0..6, 0..1);
                            blit_bufs.push(vb);
                            mask_keep.push((tex, bind));
                        }
                    }
                }
            }
        }

        self.queue.submit(Some(encoder.finish()));
        drop(mask_keep);
        drop(solid_bufs);
        drop(blit_bufs);

        if self.native.is_some() {
            if let Err(e) = self.blit_offscreen_to_surface() {
                eprintln!("next-gui-render-gpu: surface present failed: {e}");
            }
        } else {
            self.readback_to_pixels();
        }
    }

    fn frame_pixels(&self) -> Option<(&[u32], u32, u32)> {
        if self.native.is_some() {
            None
        } else {
            Some((&self.pixels, self.width, self.height))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use next_gui_render::DrawCommand;
    use std::sync::Arc;

    #[test]
    fn try_new_or_skip() {
        match GpuRenderer::try_new(32, 32, Color::rgb(0, 0, 0)) {
            Ok(gpu) => {
                assert!(!gpu.backend_name().is_empty());
                assert_eq!(gpu.present_path(), GpuPresentPath::ReadbackSoftbuffer);
                let mut dirty = DirtyRegion::new();
                dirty.add(Rect::new(0.0, 0.0, 32.0, 32.0));
                let cmds = [DrawCommand::FillRect {
                    rect: Rect::new(4.0, 4.0, 8.0, 8.0),
                    color: Color::rgb(255, 0, 0),
                }];
                let mut gpu = gpu;
                gpu.render(&cmds, &dirty);
                let (px, w, h) = gpu.frame_pixels().unwrap();
                assert_eq!((w, h), (32, 32));
                let p = px[(4 * 32 + 4) as usize];
                let r = (p >> 16) & 0xff;
                assert!(r > 200, "expected red pixel, got {p:#x}");
            }
            Err(e) => {
                eprintln!("skip GPU test: {e}");
            }
        }
    }

    #[test]
    fn blit_alpha_or_skip() {
        let Ok(mut gpu) = GpuRenderer::try_new(16, 16, Color::rgb(0, 0, 0)) else {
            return;
        };
        let mask: Arc<[u8]> = Arc::from([255u8]);
        let cmds = [DrawCommand::BlitAlpha {
            x: 2,
            y: 3,
            width: 1,
            height: 1,
            pixels: mask,
            color: Color::rgb(0, 0, 255),
        }];
        let mut dirty = DirtyRegion::new();
        dirty.add(Rect::new(0.0, 0.0, 16.0, 16.0));
        gpu.render(&cmds, &dirty);
        let (px, _, _) = gpu.frame_pixels().unwrap();
        let p = px[(3 * 16 + 2) as usize];
        let b = p & 0xff;
        assert!(b > 200, "expected blue, got {p:#x}");
    }
}
