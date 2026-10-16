//! Text engine for next-gui: font load, metrics, simple layout, glyph masks.
//!
//! P2 baseline: TrueType/OpenType (incl. TTC) via a mature rasterizer (`fontdue`),
//! self-owned layout/measure API, Latin + CJK. No GPU. Shaping is left-to-right
//! advance sum (no complex-script harfbuzz yet).

#![deny(missing_docs)]

use std::path::Path;
use std::sync::Arc;

/// Error loading or using a font.
#[derive(Debug, Clone)]
pub struct FontError(pub String);

impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for FontError {}

/// Vertical metrics at a given pixel size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontMetrics {
    /// Ascent above baseline (positive).
    pub ascent: f32,
    /// Descent below baseline (positive distance downward).
    pub descent: f32,
    /// Extra gap between lines.
    pub line_gap: f32,
    /// Recommended line height = ascent + descent + line_gap.
    pub line_height: f32,
}

/// Coverage mask (8-bit alpha) for a rasterized text run.
#[derive(Debug, Clone)]
pub struct AlphaMask {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row-major coverage, length = width * height.
    pub pixels: Arc<[u8]>,
}

impl AlphaMask {
    /// Empty mask.
    pub fn empty() -> Self {
        Self {
            width: 0,
            height: 0,
            pixels: Arc::from([]),
        }
    }

    /// Whether the mask has no pixels.
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0 || self.pixels.is_empty()
    }
}

/// A loaded font face (one index inside a TTC/TTF/OTF).
#[derive(Clone)]
pub struct Font {
    inner: Arc<fontdue::Font>,
}

impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Font").finish_non_exhaustive()
    }
}

impl Font {
    /// Load from bytes. `collection_index` selects a face inside a `.ttc`.
    pub fn from_bytes(data: &[u8], collection_index: u32) -> Result<Self, FontError> {
        let settings = fontdue::FontSettings {
            collection_index,
            scale: 40.0,
            ..fontdue::FontSettings::default()
        };
        let font = fontdue::Font::from_bytes(data, settings)
            .map_err(|e| FontError(format!("font parse: {e}")))?;
        Ok(Self {
            inner: Arc::new(font),
        })
    }

    /// Load from a filesystem path.
    pub fn from_path(path: impl AsRef<Path>, collection_index: u32) -> Result<Self, FontError> {
        let path = path.as_ref();
        let data = std::fs::read(path)
            .map_err(|e| FontError(format!("read {}: {e}", path.display())))?;
        Self::from_bytes(&data, collection_index)
    }

    /// Vertical metrics at `px` pixels.
    pub fn metrics(&self, px: f32) -> FontMetrics {
        let line = self.inner.horizontal_line_metrics(px);
        // fontdue: ascent positive up, descent negative down in some versions —
        // horizontal_line_metrics gives new_line_size etc.
        if let Some(m) = line {
            let ascent = m.ascent;
            let descent = m.descent.abs();
            let line_gap = m.line_gap.max(0.0);
            FontMetrics {
                ascent,
                descent,
                line_gap,
                line_height: (ascent + descent + line_gap).max(px),
            }
        } else {
            FontMetrics {
                ascent: px * 0.8,
                descent: px * 0.2,
                line_gap: 0.0,
                line_height: px,
            }
        }
    }

    /// Advance width / line height for a single line (no wrapping).
    pub fn measure_line(&self, text: &str, px: f32) -> (f32, f32) {
        let m = self.metrics(px);
        let mut width = 0.0f32;
        for ch in text.chars() {
            if ch == '\n' {
                continue;
            }
            let g = self.inner.metrics(ch, px);
            width += g.advance_width;
        }
        (width, m.line_height)
    }

    /// Measure with simple greedy wrap at `max_width` (character boundaries).
    pub fn measure_wrapped(&self, text: &str, px: f32, max_width: f32) -> (f32, f32) {
        let layout = self.layout(text, px, Some(max_width));
        (layout.width, layout.height)
    }

    /// Layout glyphs; optional wrap width. Returns positions relative to top-left
    /// of the first line's box (baseline offset included in glyph bitmaps).
    pub fn layout(&self, text: &str, px: f32, max_width: Option<f32>) -> TextLayout {
        let metrics = self.metrics(px);
        let mut glyphs = Vec::new();
        let mut line_widths = Vec::new();
        let mut x = 0.0f32;
        let mut line = 0u32;
        let mut max_w = 0.0f32;

        let push_line_break = |line_widths: &mut Vec<f32>,
                               max_w: &mut f32,
                               x: &mut f32,
                               line: &mut u32| {
            line_widths.push(*x);
            *max_w = max_w.max(*x);
            *x = 0.0;
            *line += 1;
        };

        for ch in text.chars() {
            if ch == '\n' {
                push_line_break(&mut line_widths, &mut max_w, &mut x, &mut line);
                continue;
            }
            let g = self.inner.metrics(ch, px);
            if let Some(limit) = max_width {
                if x > 0.0 && x + g.advance_width > limit {
                    push_line_break(&mut line_widths, &mut max_w, &mut x, &mut line);
                }
            }
            let baseline_y = line as f32 * metrics.line_height + metrics.ascent;
            glyphs.push(PlacedGlyph {
                ch,
                x,
                baseline_y,
                advance: g.advance_width,
            });
            x += g.advance_width;
        }
        line_widths.push(x);
        max_w = max_w.max(x);
        let lines = line + 1;
        let height = lines as f32 * metrics.line_height;
        TextLayout {
            width: max_w,
            height,
            line_height: metrics.line_height,
            glyphs,
        }
    }

    /// Rasterize a layout into one alpha mask (top-left origin).
    pub fn rasterize_layout(&self, layout: &TextLayout, px: f32) -> AlphaMask {
        let w = layout.width.ceil().max(1.0) as u32;
        let h = layout.height.ceil().max(1.0) as u32;
        let mut buf = vec![0u8; (w as usize).saturating_mul(h as usize)];
        if buf.is_empty() {
            return AlphaMask::empty();
        }
        for g in &layout.glyphs {
            let (metrics, bitmap) = self.inner.rasterize(g.ch, px);
            if metrics.width == 0 || metrics.height == 0 {
                continue;
            }
            let ox = g.x + metrics.xmin as f32;
            let oy = g.baseline_y + metrics.ymin as f32;
            blit_alpha(
                &mut buf,
                w,
                h,
                ox.round() as i32,
                oy.round() as i32,
                metrics.width as u32,
                metrics.height as u32,
                &bitmap,
            );
        }
        AlphaMask {
            width: w,
            height: h,
            pixels: Arc::from(buf),
        }
    }

    /// Measure and rasterize in one step (optional wrap).
    pub fn rasterize_text(&self, text: &str, px: f32, max_width: Option<f32>) -> (TextLayout, AlphaMask) {
        let layout = self.layout(text, px, max_width);
        let mask = self.rasterize_layout(&layout, px);
        (layout, mask)
    }
}

/// One glyph in a [`TextLayout`].
#[derive(Debug, Clone, Copy)]
pub struct PlacedGlyph {
    /// Character.
    pub ch: char,
    /// Pen x on the line.
    pub x: f32,
    /// Baseline y from top of the layout box.
    pub baseline_y: f32,
    /// Advance width.
    pub advance: f32,
}

/// Laid-out text run.
#[derive(Debug, Clone)]
pub struct TextLayout {
    /// Tight width (max line advance).
    pub width: f32,
    /// Total height including all lines.
    pub height: f32,
    /// Line height used.
    pub line_height: f32,
    /// Glyphs in paint order.
    pub glyphs: Vec<PlacedGlyph>,
}

fn blit_alpha(
    dest: &mut [u8],
    dest_w: u32,
    dest_h: u32,
    x: i32,
    y: i32,
    src_w: u32,
    src_h: u32,
    src: &[u8],
) {
    for row in 0..src_h as i32 {
        let dy = y + row;
        if dy < 0 || dy >= dest_h as i32 {
            continue;
        }
        for col in 0..src_w as i32 {
            let dx = x + col;
            if dx < 0 || dx >= dest_w as i32 {
                continue;
            }
            let s = src[(row as u32 * src_w + col as u32) as usize];
            let di = (dy as u32 * dest_w + dx as u32) as usize;
            // max blend for overlapping coverage
            if s > dest[di] {
                dest[di] = s;
            }
        }
    }
}

/// Candidate system fonts for Latin + CJK (Linux paths first).
///
/// Each entry is `(path, collection_index)`. Index `3` is typically Simplified
/// Chinese in Noto Sans CJK Regular TTC collections used on Debian/Ubuntu.
pub const DEFAULT_CJK_FONT_CANDIDATES: &[(&str, u32)] = &[
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 3),
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc", 3),
    ("/System/Library/Fonts/PingFang.ttc", 0),
    ("/System/Library/Fonts/STHeiti Light.ttc", 0),
    ("C:\\Windows\\Fonts\\msyh.ttc", 0),
    ("C:\\Windows\\Fonts\\simsun.ttc", 0),
];

/// Try well-known system paths for a CJK-capable sans font.
pub fn load_default_cjk_font() -> Result<Font, FontError> {
    let mut errors = Vec::new();
    for &(path, index) in DEFAULT_CJK_FONT_CANDIDATES {
        match Font::from_path(path, index) {
            Ok(font) => {
                // Sanity: prefer a face that can draw a CJK ideograph.
                let (_, bitmap) = font.inner.rasterize('中', 24.0);
                if bitmap.iter().any(|p| *p > 0) {
                    return Ok(font);
                }
                errors.push(format!("{path}#{index}: loaded but '中' empty"));
            }
            Err(e) => errors.push(format!("{path}#{index}: {e}")),
        }
    }
    Err(FontError(format!(
        "no CJK font found; tried: {}",
        errors.join("; ")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_font() -> Font {
        load_default_cjk_font().expect("system CJK font required for tests")
    }

    #[test]
    fn metrics_positive() {
        let font = test_font();
        let m = font.metrics(24.0);
        assert!(m.line_height >= 24.0 * 0.8);
        assert!(m.ascent > 0.0);
    }

    #[test]
    fn latin_and_cjk_measure() {
        let font = test_font();
        let (lw, lh) = font.measure_line("Hello", 24.0);
        let (cw, ch) = font.measure_line("你好", 24.0);
        assert!(lw > 0.0 && lh > 0.0);
        assert!(cw > 0.0 && ch > 0.0);
        // Two CJK chars should be wider than a short Latin word at same size often —
        // at least both non-empty masks.
        let (_, mask) = font.rasterize_text("Hello 工业 你好", 28.0, None);
        assert!(!mask.is_empty());
        assert!(mask.pixels.iter().any(|p| *p > 32));
    }

    #[test]
    fn wrap_reduces_width() {
        let font = test_font();
        let (w1, h1) = font.measure_wrapped("ABCDEFGHIJKLMNOPQRST", 20.0, 80.0);
        let (w0, h0) = font.measure_line("ABCDEFGHIJKLMNOPQRST", 20.0);
        assert!(w1 <= 80.0 + 1.0);
        assert!(h1 >= h0);
        assert!(w1 < w0 || h1 > h0);
    }
}
