//! wgpu GPU rasterizer for next-gui.
//!
//! Consumes the same [`DrawCommand`] stream as `next-gui-render-soft`, draws into an
//! off-screen texture, then readbacks XRGB8888 for [`next_gui_platform`] present.
//!
//! Native wgpu `Surface` present is the Office-line goal but **not wired yet** — see
//! [`GpuPresentPath`] and `docs/office-line.md`. Softbuffer ownership on the OS window
//! blocks attaching a wgpu surface without a GPU-first platform path.

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
/// Office prefers [`GpuPresentPath::NativeSurface`]; the running backend reports what
/// it actually does today via [`GpuRenderer::present_path`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuPresentPath {
    /// Off-screen wgpu → CPU readback → `platform.present` (softbuffer). **Current.**
    ReadbackSoftbuffer,
    /// Direct `wgpu::Surface` present on the OS window. **Not implemented** until
    /// `platform` offers a GPU-first window without softbuffer contention.
    NativeSurface,
}

/// GPU backend implementing [`Renderer`] via wgpu off-screen + CPU readback.
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
}

struct TargetTextures {
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    staging: wgpu::Buffer,
}

impl GpuRenderer {
    /// Try to create a GPU renderer. Falls back path: caller should use soft on `Err`.
    pub fn try_new(width: u32, height: u32, clear: Color) -> Result<Self, GpuInitError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            backend_options: Default::default(),
            ..Default::default()
        });

        let adapter = {
            let mut found = None;
            for force_fallback in [false, true] {
                found = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
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
        };
        gpu.resize(width, height);
        Ok(gpu)
    }

    /// Adapter backend label (e.g. `Vulkan`, `Gl`).
    pub fn backend_name(&self) -> &str {
        &self.backend_name
    }

    /// How this renderer presents frames (always readback until Surface lands).
    pub fn present_path(&self) -> GpuPresentPath {
        GpuPresentPath::ReadbackSoftbuffer
    }

    /// Clear / background color.
    pub fn clear_color(&self) -> Color {
        self.clear
    }

    /// Update clear color.
    pub fn set_clear_color(&mut self, clear: Color) {
        self.clear = clear;
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());

        let unpadded_bytes_per_row = self.width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
        let staging_size = padded_bytes_per_row as u64 * self.height as u64;
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu-staging"),
            size: staging_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

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
        if self.width == 0 || self.height == 0 {
            return;
        }

        let unpadded = self.width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = (unpadded + align - 1) / align * align;

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
                buffer: &target.staging,
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

        let slice = target.staging.slice(..);
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
        target.staging.unmap();
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

        // Keep mask textures alive for the submit.
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

                // Clear dirty patch.
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

        self.readback_to_pixels();
    }

    fn frame_pixels(&self) -> Option<(&[u32], u32, u32)> {
        Some((&self.pixels, self.width, self.height))
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
                // Allow some variance; expect red-ish in low bits of XRGB.
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
