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
        // fontdue LineMetrics: ascent > 0 above baseline, descent < 0 below.
        // new_line_size = ascent - descent + line_gap (= ascent + |descent| + gap).
        if let Some(m) = line {
            let ascent = m.ascent.max(0.0);
            let descent = m.descent.abs();
            let line_gap = m.line_gap; // may be negative in broken faces; keep signed for sum
            let line_height = if m.new_line_size > 0.0 {
                m.new_line_size
            } else {
                (ascent + descent + line_gap.max(0.0)).max(px)
            };
            FontMetrics {
                ascent,
                descent,
                line_gap: line_gap.max(0.0),
                line_height: line_height.max(px),
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
    ///
    /// fontdue `Metrics::ymin` is the **bottom** edge of the bitmap relative to the
    /// baseline (PositiveYUp font space). In our PositiveYDown mask, the top of the
    /// bitmap is therefore `baseline_y - ymin - height`.
    pub fn rasterize_layout(&self, layout: &TextLayout, px: f32) -> AlphaMask {
        if layout.glyphs.is_empty() {
            return AlphaMask::empty();
        }

        // Collect glyph bitmaps and pixel rects (layout origin = top-left of line box).
        let mut parts: Vec<(i32, i32, u32, u32, Vec<u8>)> = Vec::with_capacity(layout.glyphs.len());
        let mut min_x = 0i32;
        let mut min_y = 0i32;
        let mut max_x = layout.width.ceil().max(1.0) as i32;
        let mut max_y = layout.height.ceil().max(1.0) as i32;

        for g in &layout.glyphs {
            let (metrics, bitmap) = self.inner.rasterize(g.ch, px);
            if metrics.width == 0 || metrics.height == 0 {
                continue;
            }
            let ox = (g.x + metrics.xmin as f32).round() as i32;
            // ymin = bottom bearing; bitmap rows go top → bottom.
            let oy = (g.baseline_y - metrics.ymin as f32 - metrics.height as f32).round() as i32;
            let w = metrics.width as u32;
            let h = metrics.height as u32;
            min_x = min_x.min(ox);
            min_y = min_y.min(oy);
            max_x = max_x.max(ox + w as i32);
            max_y = max_y.max(oy + h as i32);
            parts.push((ox, oy, w, h, bitmap));
        }

        if parts.is_empty() {
            return AlphaMask::empty();
        }

        // Pad so negative bearings still land inside the mask; layout (0,0) stays
        // the semantic top-left (ascent space above the baseline is preserved).
        let pad_x = (-min_x).max(0);
        let pad_y = (-min_y).max(0);
        let w = (max_x + pad_x).max(1) as u32;
        let h = (max_y + pad_y).max(1) as u32;
        let mut buf = vec![0u8; (w as usize).saturating_mul(h as usize)];
        for (ox, oy, gw, gh, bitmap) in parts {
            blit_alpha(
                &mut buf,
                w,
                h,
                ox + pad_x,
                oy + pad_y,
                gw,
                gh,
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

    fn ink_rows(mask: &AlphaMask, thresh: u8) -> Vec<u32> {
        let mut rows = Vec::new();
        for y in 0..mask.height {
            let start = (y * mask.width) as usize;
            let end = start + mask.width as usize;
            if mask.pixels[start..end].iter().any(|p| *p > thresh) {
                rows.push(y);
            }
        }
        rows
    }

    #[test]
    fn metrics_positive() {
        let font = test_font();
        let m = font.metrics(24.0);
        assert!(m.line_height >= 24.0 * 0.8);
        assert!(m.ascent > 0.0);
        assert!(m.descent >= 0.0);
        // line_height must cover ascent + descent
        assert!(m.line_height + 0.5 >= m.ascent + m.descent);
    }

    #[test]
    fn latin_and_cjk_measure() {
        let font = test_font();
        let (lw, lh) = font.measure_line("Hello", 24.0);
        let (cw, ch) = font.measure_line("你好", 24.0);
        assert!(lw > 0.0 && lh > 0.0);
        assert!(cw > 0.0 && ch > 0.0);
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

    /// Regression: glyphs must sit on the baseline (not be pushed below it and
    /// clipped to a thin strip at the bottom of the mask).
    #[test]
    fn rasterize_places_glyphs_on_baseline_not_clipped() {
        let font = test_font();
        let px = 22.0;
        let m = font.metrics(px);
        let (_, mask) = font.rasterize_text("Count: 0", px, None);
        assert!(mask.height >= 8);
        let rows = ink_rows(&mask, 16);
        assert!(
            !rows.is_empty(),
            "expected ink in rasterized Latin text"
        );
        let first = *rows.first().unwrap();
        let last = *rows.last().unwrap();
        // Cap-height glyphs occupy roughly [ascent - cap, ascent] in the line box.
        // Require ink to start well above the bottom half (the old ymin bug left
        // ink only in the last ~25% of rows).
        assert!(
            first < mask.height / 2,
            "ink starts too low (row {first}/{}); ymin sign likely wrong",
            mask.height
        );
        // Ink should reach near the baseline band (around ascent), not stop early.
        let baseline_band = m.ascent.round() as u32;
        assert!(
            last + 2 >= baseline_band.min(mask.height - 1),
            "ink ends at row {last}, expected near baseline ~{baseline_band}"
        );
        // Decent coverage: more than a few tip pixels.
        assert!(
            last - first + 1 >= (px * 0.45) as u32,
            "glyph span too thin ({} rows) — still clipped?",
            last - first + 1
        );
    }

    #[test]
    fn rasterize_cjk_not_torn() {
        let font = test_font();
        let px = 26.0;
        let (_, mask) = font.rasterize_text("你好世界", px, None);
        let rows = ink_rows(&mask, 16);
        assert!(!rows.is_empty());
        let span = rows.last().unwrap() - rows.first().unwrap() + 1;
        // CJK ideographs are roughly em-square tall; require substantial span.
        assert!(
            span as f32 >= px * 0.55,
            "CJK ink span {span} too small for px={px} (torn/clipped?)"
        );
        assert!(
            *rows.first().unwrap() < mask.height / 3,
            "CJK ink starts too low — vertical placement bug"
        );
    }

    #[test]
    fn glyph_bitmap_fits_inside_mask() {
        let font = test_font();
        let px = 24.0;
        let layout = font.layout("Hg中y", px, None);
        // Mirror placement math and ensure every glyph rect is in-range after pad.
        let mut min_x = 0i32;
        let mut min_y = 0i32;
        let mut max_x = layout.width.ceil() as i32;
        let mut max_y = layout.height.ceil() as i32;
        let mut rects = Vec::new();
        for g in &layout.glyphs {
            let (metrics, _) = font.inner.rasterize(g.ch, px);
            if metrics.width == 0 || metrics.height == 0 {
                continue;
            }
            let ox = (g.x + metrics.xmin as f32).round() as i32;
            let oy = (g.baseline_y - metrics.ymin as f32 - metrics.height as f32).round() as i32;
            min_x = min_x.min(ox);
            min_y = min_y.min(oy);
            max_x = max_x.max(ox + metrics.width as i32);
            max_y = max_y.max(oy + metrics.height as i32);
            rects.push((ox, oy, metrics.width as i32, metrics.height as i32));
        }
        let pad_x = (-min_x).max(0);
        let pad_y = (-min_y).max(0);
        let w = max_x + pad_x;
        let h = max_y + pad_y;
        for (ox, oy, gw, gh) in rects {
            let x0 = ox + pad_x;
            let y0 = oy + pad_y;
            assert!(x0 >= 0 && y0 >= 0, "glyph origin negative after pad");
            assert!(x0 + gw <= w && y0 + gh <= h, "glyph overflows mask");
        }
        let (_, mask) = font.rasterize_text("Hg中y", px, None);
        assert_eq!(mask.width as i32, w);
        assert_eq!(mask.height as i32, h);
    }

    /// Write PPM dumps under tmp/text-qa for human/agent visual checks.
    #[test]
    fn write_qa_ppm_samples() {
        let font = test_font();
        let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tmp/text-qa");
        let _ = std::fs::create_dir_all(&out_dir);
        let samples: &[(&str, f32, &str)] = &[
            ("Count: 0", 22.0, "latin_count"),
            ("Increment", 18.0, "btn_increment"),
            ("Hello 工业界面 · 你好世界", 22.0, "cjk_mixed"),
            ("你好世界", 26.0, "cjk_only"),
            ("Hg中y", 24.0, "descenders"),
        ];
        for &(text, px, name) in samples {
            let (layout, mask) = font.rasterize_text(text, px, None);
            let path = out_dir.join(format!("{name}.ppm"));
            write_mask_ppm(&path, &mask, layout.width, layout.height);
            let rows = ink_rows(&mask, 16);
            let span = if rows.is_empty() {
                0
            } else {
                rows.last().unwrap() - rows.first().unwrap() + 1
            };
            eprintln!(
                "QA {name}: mask={}x{} layout={:.1}x{:.1} ink_rows={} span={} file={}",
                mask.width,
                mask.height,
                layout.width,
                layout.height,
                rows.len(),
                span,
                path.display()
            );
            assert!(!rows.is_empty(), "{name} has no ink");
            assert!(
                *rows.first().unwrap() < mask.height / 2,
                "{name}: ink still bottom-clipped"
            );
        }
    }

    fn write_mask_ppm(path: &std::path::Path, mask: &AlphaMask, _lw: f32, _lh: f32) {
        // White text on dark gray background for contrast check.
        let mut body = format!("P3\n{} {}\n255\n", mask.width, mask.height);
        for y in 0..mask.height {
            for x in 0..mask.width {
                let a = mask.pixels[(y * mask.width + x) as usize] as u32;
                let r = (0x20 * (255 - a) + 0xf2 * a) / 255;
                let g = (0x20 * (255 - a) + 0xf2 * a) / 255;
                let b = (0x22 * (255 - a) + 0xf5 * a) / 255;
                body.push_str(&format!("{r} {g} {b} "));
            }
            body.push('\n');
        }
        std::fs::write(path, body).expect("write ppm");
    }
}
