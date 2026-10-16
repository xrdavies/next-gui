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
}
