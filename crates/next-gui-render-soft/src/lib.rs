//! CPU software rasterizer for next-gui.

#![deny(missing_docs)]

use next_gui_render::{image_pixel_bounds, DrawCommand, Renderer};
use next_gui_scene::{Color, DirtyRegion, Rect, RgbaImage};

/// Soft-renderer clearing / background color when dirty regions are erased.
#[derive(Debug, Clone)]
pub struct SoftRenderer {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
    clear: Color,
}

impl SoftRenderer {
    /// Create with a clear/background color.
    pub fn new(width: u32, height: u32, clear: Color) -> Self {
        let mut r = Self {
            width: 0,
            height: 0,
            pixels: Vec::new(),
            clear,
        };
        r.resize(width, height);
        r
    }

    /// Background / clear color.
    pub fn clear_color(&self) -> Color {
        self.clear
    }

    /// Set clear color (does not itself mark dirty).
    pub fn set_clear_color(&mut self, clear: Color) {
        self.clear = clear;
    }

    fn write_rect_clipped(&mut self, rect: &Rect, color: Color, clip: &Rect) {
        let Some((x0, y0, w, h)) = image_pixel_bounds(rect, clip, self.width, self.height) else {
            return;
        };
        let px = color.to_xrgb8888();
        for y in y0..y0 + h {
            let start = (y * self.width + x0) as usize;
            self.pixels[start..start + w as usize].fill(px);
        }
    }

    fn blend_pixel(&mut self, index: usize, color: Color, alpha: u32) {
        if alpha >= 255 {
            self.pixels[index] = color.to_xrgb8888();
        } else if alpha > 0 {
            let dst = self.pixels[index];
            let inv = 255 - alpha;
            let r = (color.r as u32 * alpha + ((dst >> 16) & 255) * inv) / 255;
            let g = (color.g as u32 * alpha + ((dst >> 8) & 255) * inv) / 255;
            let b = (color.b as u32 * alpha + (dst & 255) * inv) / 255;
            self.pixels[index] = (r << 16) | (g << 8) | b;
        }
    }

    fn fill_rect_clipped(&mut self, rect: &Rect, color: Color, clip: &Rect) {
        if color.a == 255 {
            self.write_rect_clipped(rect, color, clip);
            return;
        }
        let Some((x0, y0, w, h)) = image_pixel_bounds(rect, clip, self.width, self.height) else {
            return;
        };
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                self.blend_pixel((y * self.width + x) as usize, color, color.a as u32);
            }
        }
    }

    /// Match the GPU signed-distance coverage, including fractional straight edges.
    fn fill_rounded_rect_clipped(&mut self, rect: &Rect, color: Color, radius: f32, clip: &Rect) {
        let r = radius.min(rect.width * 0.5).min(rect.height * 0.5).max(0.0);
        if r < 0.5 {
            self.fill_rect_clipped(rect, color, clip);
            return;
        }
        let Some((x0, y0, w, h)) = image_pixel_bounds(rect, clip, self.width, self.height) else {
            return;
        };
        let hw = rect.width * 0.5;
        let hh = rect.height * 0.5;
        let cx = rect.x + hw;
        let cy = rect.y + hh;
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                let qx = (x as f32 + 0.5 - cx).abs() - (hw - r).max(0.0);
                let qy = (y as f32 + 0.5 - cy).abs() - (hh - r).max(0.0);
                let dx = qx.max(0.0);
                let dy = qy.max(0.0);
                let sd = (dx * dx + dy * dy).sqrt() + qx.max(qy).min(0.0) - r;
                let coverage = (0.5 - sd).clamp(0.0, 1.0);
                let alpha = (coverage * color.a as f32).round() as u32;
                self.blend_pixel((y * self.width + x) as usize, color, alpha);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn blit_alpha_clipped(
        &mut self,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        mask: &[u8],
        color: Color,
        clip: &Rect,
    ) {
        if width == 0 || height == 0 || mask.len() < (width as usize) * (height as usize) {
            return;
        }
        let src_rect = Rect::new(x as f32, y as f32, width as f32, height as f32);
        let area = src_rect.intersect(clip);
        let Some((cx0, cy0, cw, ch)) = area.clip_to_buffer(self.width, self.height) else {
            return;
        };
        let cr = color.r as u32;
        let cg = color.g as u32;
        let cb = color.b as u32;
        let ca = color.a as u32;

        for py in cy0..cy0 + ch {
            for px in cx0..cx0 + cw {
                let mx = px as i32 - x;
                let my = py as i32 - y;
                if mx < 0 || my < 0 || mx >= width as i32 || my >= height as i32 {
                    continue;
                }
                let coverage = mask[(my as u32 * width + mx as u32) as usize] as u32;
                if coverage == 0 {
                    continue;
                }
                // coverage * color.a / 255
                let a = (coverage * ca) / 255;
                if a == 0 {
                    continue;
                }
                let dest_i = (py * self.width + px) as usize;
                let dst = self.pixels[dest_i];
                let dr = (dst >> 16) & 0xff;
                let dg = (dst >> 8) & 0xff;
                let db = dst & 0xff;
                // src over
                let inv = 255 - a;
                let or_ = (cr * a + dr * inv) / 255;
                let og = (cg * a + dg * inv) / 255;
                let ob = (cb * a + db * inv) / 255;
                self.pixels[dest_i] = (or_ << 16) | (og << 8) | ob;
            }
        }
    }

    fn blit_rgba_clipped(&mut self, rect: &Rect, clip: &Rect, image: &RgbaImage) {
        let Some((x0, y0, w, h)) = image_pixel_bounds(rect, clip, self.width, self.height) else {
            return;
        };
        for y in y0..y0 + h {
            let sy = (((y as f32 + 0.5 - rect.y) / rect.height * image.height() as f32) as u32)
                .min(image.height() - 1);
            for x in x0..x0 + w {
                let sx = (((x as f32 + 0.5 - rect.x) / rect.width * image.width() as f32) as u32)
                    .min(image.width() - 1);
                let off = (sy as usize * image.width() as usize + sx as usize) * 4;
                let rgba = &image.pixels()[off..off + 4];
                let a = rgba[3] as u32;
                let i = (y * self.width + x) as usize;
                let dst = self.pixels[i];
                let blend = |src: u8, shift: u32| {
                    (src as u32 * a + ((dst >> shift) & 255u32) * (255 - a)) / 255
                };
                self.pixels[i] =
                    (blend(rgba[0], 16) << 16) | (blend(rgba[1], 8) << 8) | blend(rgba[2], 0);
            }
        }
    }

    /// Pixel at (x,y) for tests; None if OOB.
    pub fn pixel(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.width || y >= self.height {
            None
        } else {
            Some(self.pixels[(y * self.width + x) as usize])
        }
    }
}

impl Renderer for SoftRenderer {
    fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        let n = (width as usize).saturating_mul(height as usize);
        let clear_px = self.clear.to_xrgb8888();
        self.pixels.resize(n, clear_px);
        for p in &mut self.pixels {
            *p = clear_px;
        }
    }

    fn render(&mut self, commands: &[DrawCommand], dirty: &DirtyRegion) {
        if dirty.is_empty() || self.width == 0 || self.height == 0 {
            return;
        }
        for d in &dirty.rects {
            // Clear dirty patch to background first.
            self.write_rect_clipped(d, self.clear, d);
            for cmd in commands {
                match cmd {
                    DrawCommand::BlitRgba { rect, clip, image } => {
                        self.blit_rgba_clipped(rect, &clip.intersect(d), image);
                    }
                    DrawCommand::FillRect { rect, color } => {
                        if rect.intersects(d) {
                            self.fill_rect_clipped(rect, *color, d);
                        }
                    }
                    DrawCommand::FillRoundedRect {
                        rect,
                        clip,
                        color,
                        radius,
                    } => {
                        if rect.intersects(d) {
                            self.fill_rounded_rect_clipped(
                                rect,
                                *color,
                                *radius,
                                &clip.intersect(d),
                            );
                        }
                    }
                    DrawCommand::BlitAlpha {
                        x,
                        y,
                        width,
                        height,
                        pixels,
                        color,
                    } => {
                        let bounds = Rect::new(*x as f32, *y as f32, *width as f32, *height as f32);
                        if bounds.intersects(d) {
                            self.blit_alpha_clipped(*x, *y, *width, *height, pixels, *color, d);
                        }
                    }
                }
            }
        }
    }

    fn frame_pixels(&self) -> Option<(&[u32], u32, u32)> {
        Some((&self.pixels, self.width, self.height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use next_gui_render::{encode_scene, DrawCommand};
    use next_gui_scene::Scene;
    use std::sync::Arc;

    #[test]
    fn fractional_primitives_sample_centers_and_blend_edges() {
        let mut dirty = DirtyRegion::new();
        dirty.add(Rect::new(0.0, 0.0, 16.0, 16.0));
        let mut soft = SoftRenderer::new(16, 16, Color::rgb(100, 0, 0));
        soft.render(
            &[DrawCommand::FillRect {
                rect: Rect::new(1.75, 1.75, 3.0, 3.0),
                color: Color {
                    r: 0,
                    g: 0,
                    b: 255,
                    a: 128,
                },
            }],
            &dirty,
        );
        assert_eq!(soft.pixel(1, 2), Some(0x640000));
        assert_eq!(soft.pixel(2, 2), Some(0x310080));
        assert_eq!(soft.pixel(4, 4), Some(0x310080));
        assert_eq!(soft.pixel(5, 4), Some(0x640000));
        let mut soft = SoftRenderer::new(16, 16, Color::rgb(0, 0, 0));
        soft.render(
            &[DrawCommand::FillRoundedRect {
                rect: Rect::new(2.25, 2.25, 10.5, 10.5),
                clip: Rect::new(0.0, 0.0, 16.0, 16.0),
                color: Color::rgb(255, 0, 0),
                radius: 2.0,
            }],
            &dirty,
        );
        assert_eq!(soft.pixel(7, 1), Some(0));
        assert_eq!(soft.pixel(7, 2), Some(0xbf0000));
        assert_eq!(soft.pixel(7, 3), Some(0xff0000));
        assert_eq!(soft.pixel(2, 2), Some(0x060000));
    }

    #[test]
    fn dirty_fill_writes_pixels() {
        let mut soft = SoftRenderer::new(64, 64, Color::rgb(0, 0, 0));
        let cmds = [DrawCommand::FillRect {
            rect: Rect::new(10.0, 10.0, 5.0, 5.0),
            color: Color::rgb(255, 0, 0),
        }];
        let mut dirty = DirtyRegion::new();
        dirty.add(Rect::new(10.0, 10.0, 5.0, 5.0));
        soft.render(&cmds, &dirty);
        assert_eq!(
            soft.pixel(12, 12),
            Some(Color::rgb(255, 0, 0).to_xrgb8888())
        );
        assert_eq!(soft.pixel(0, 0), Some(Color::rgb(0, 0, 0).to_xrgb8888()));
    }

    #[test]
    fn blit_alpha_tints() {
        let mut soft = SoftRenderer::new(16, 16, Color::rgb(0, 0, 0));
        let mask: Arc<[u8]> = Arc::from([255u8]);
        let cmds = [DrawCommand::BlitAlpha {
            x: 3,
            y: 4,
            width: 1,
            height: 1,
            pixels: mask,
            color: Color::rgb(0, 0, 255),
        }];
        let mut dirty = DirtyRegion::new();
        dirty.add(Rect::new(0.0, 0.0, 16.0, 16.0));
        soft.render(&cmds, &dirty);
        assert_eq!(soft.pixel(3, 4), Some(Color::rgb(0, 0, 255).to_xrgb8888()));
        assert_eq!(soft.pixel(0, 0), Some(Color::rgb(0, 0, 0).to_xrgb8888()));
    }

    #[test]
    fn rounded_rect_skips_far_corner() {
        let mut soft = SoftRenderer::new(32, 32, Color::rgb(0, 0, 0));
        let cmds = [DrawCommand::FillRoundedRect {
            rect: Rect::new(4.0, 4.0, 20.0, 20.0),
            clip: Rect::new(4.0, 4.0, 20.0, 20.0),
            color: Color::rgb(255, 0, 0),
            radius: 8.0,
        }];
        let mut dirty = DirtyRegion::new();
        dirty.add(Rect::new(0.0, 0.0, 32.0, 32.0));
        soft.render(&cmds, &dirty);
        // Center of rounded rect is filled.
        assert_eq!(
            soft.pixel(14, 14),
            Some(Color::rgb(255, 0, 0).to_xrgb8888())
        );
        // Far outside the corner circle stays clear.
        assert_eq!(soft.pixel(4, 4), Some(Color::rgb(0, 0, 0).to_xrgb8888()));
    }

    #[test]
    fn rounded_scene_clips_keep_original_geometry() {
        for radius in [10.0, 0.25] {
            let mut scene = Scene::new();
            let parent = scene
                .insert(scene.root(), Rect::empty(), Color::rgb(0, 0, 0))
                .unwrap();
            let node = scene
                .insert(
                    parent,
                    Rect::new(0.0, 0.0, 100.0, 50.0),
                    Color::rgb(255, 255, 255),
                )
                .unwrap();
            scene.set_corner_radius(node, radius);
            scene.mark_full_dirty(110.0, 60.0);
            let dirty = scene.take_dirty();
            let mut full = SoftRenderer::new(110, 60, Color::rgb(0, 0, 0));
            full.render(&encode_scene(&scene, &dirty), &dirty);
            for clip in [
                Rect::new(50.0, 0.0, 50.0, 50.0),
                Rect::new(50.6, 1.6, 47.8, 46.8),
            ] {
                scene.set_clip(parent, Some(Rect::new(30.0, 0.0, 70.0, 50.0)));
                scene.set_clip(node, Some(clip));
                let mut clipped = SoftRenderer::new(110, 60, Color::rgb(0, 0, 0));
                clipped.render(&encode_scene(&scene, &dirty), &dirty);
                for y in 0..60 {
                    for x in 0..110 {
                        let expected = if clip.contains(x as f32 + 0.5, y as f32 + 0.5) {
                            full.pixel(x, y)
                        } else {
                            Some(0)
                        };
                        assert_eq!(
                            clipped.pixel(x, y),
                            expected,
                            "({x},{y}), radius={radius}, clip={clip:?}"
                        );
                    }
                }
                let before = clipped.frame_pixels().unwrap().0.to_vec();
                let mut patch = DirtyRegion::new();
                patch.add(Rect::new(48.0, 0.0, 18.0, 50.0));
                clipped.render(&encode_scene(&scene, &patch), &patch);
                assert_eq!(clipped.frame_pixels().unwrap().0, before);
            }
        }
    }
}
