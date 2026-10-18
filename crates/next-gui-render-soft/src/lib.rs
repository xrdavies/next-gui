//! CPU software rasterizer for next-gui.

#![deny(missing_docs)]

use next_gui_render::{DrawCommand, Renderer};
use next_gui_scene::{Color, DirtyRegion, Rect};

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

    fn fill_rect_clipped(&mut self, rect: &Rect, color: Color, clip: &Rect) {
        let area = rect.intersect(clip);
        let Some((x0, y0, w, h)) = area.clip_to_buffer(self.width, self.height) else {
            return;
        };
        let px = color.to_xrgb8888();
        for y in y0..y0 + h {
            let row = (y * self.width) as usize;
            for x in x0..x0 + w {
                self.pixels[row + x as usize] = px;
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
            self.fill_rect_clipped(d, self.clear, d);
            for cmd in commands {
                match cmd {
                    DrawCommand::FillRect { rect, color } => {
                        if rect.intersects(d) {
                            self.fill_rect_clipped(rect, *color, d);
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
    use next_gui_render::DrawCommand;
    use std::sync::Arc;

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
        assert_eq!(soft.pixel(12, 12), Some(Color::rgb(255, 0, 0).to_xrgb8888()));
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
}
