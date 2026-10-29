//! Text engine for next-gui: font load, metrics, simple layout, glyph masks.
//!
//! P2 baseline: TrueType/OpenType (incl. TTC) via a mature rasterizer (`fontdue`),
//! self-owned layout/measure API, Latin + CJK. No GPU. Shaping is left-to-right
//! advance sum (no complex-script harfbuzz yet).

#![deny(missing_docs)]

use std::path::Path;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

mod system_fonts;
pub use system_fonts::{load_system_font, system_font_families};

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

/// A loaded primary font face with optional faces for missing characters.
#[derive(Clone)]
pub struct Font {
    inner: Arc<fontdue::Font>,
    fallbacks: Vec<Font>,
    synthetic_bold: bool,
    synthetic_italic: bool,
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
            fallbacks: Vec::new(),
            synthetic_bold: false,
            synthetic_italic: false,
        })
    }

    /// Load from a filesystem path.
    pub fn from_path(path: impl AsRef<Path>, collection_index: u32) -> Result<Self, FontError> {
        let path = path.as_ref();
        let data = std::fs::read(path)
            .map_err(|e| FontError(format!("read {}: {e}", path.display())))?;
        Self::from_bytes(&data, collection_index)
    }

    /// Append a face (and its fallbacks) for characters absent from this face.
    /// Measurement, layout and rasterization use the same chosen face. Vertical
    /// line metrics and the baseline continue to come from the primary face.
    pub fn with_fallback(mut self, fallback: Font) -> Self {
        self.fallbacks.push(fallback);
        self
    }

    /// Synthesize missing bold/italic faces; existing glyph advances stay unchanged.
    /// Prefer an installed face through [`load_system_font`] when one is available.
    pub fn with_synthetic_style(mut self, bold: bool, italic: bool) -> Self {
        self.synthetic_bold = bold;
        self.synthetic_italic = italic;
        self
    }

    fn glyph_metrics(&self, ch: char, px: f32) -> fontdue::Metrics {
        let face = self.face_for(ch);
        let mut metrics = face.inner.metrics(ch, px);
        Self::style_metrics(&mut metrics, px, self.synthetic_bold || face.synthetic_bold, self.synthetic_italic || face.synthetic_italic);
        metrics
    }

    fn style_metrics(metrics: &mut fontdue::Metrics, px: f32, bold: bool, italic: bool) {
        if metrics.width == 0 || metrics.height == 0 { return; }
        if italic {
            let left = (metrics.ymin as f32 * 0.2).floor() as i32;
            let right = ((metrics.ymin as f32 + metrics.height as f32) * 0.2).ceil() as i32;
            metrics.xmin += left;
            metrics.width += (right - left).max(0) as usize;
        }
        if bold { metrics.width += (px / 24.0).ceil().max(1.0) as usize; }
    }

    fn rasterize_glyph(&self, ch: char, px: f32) -> (fontdue::Metrics, Vec<u8>) {
        let face = self.face_for(ch);
        let (mut metrics, bitmap) = face.inner.rasterize(ch, px);
        let bold = self.synthetic_bold || face.synthetic_bold;
        let italic = self.synthetic_italic || face.synthetic_italic;
        if (!bold && !italic) || bitmap.is_empty() { return (metrics, bitmap); }
        let original = metrics;
        Self::style_metrics(&mut metrics, px, bold, italic);
        let mut styled = vec![0; metrics.width * metrics.height];
        let bold = if bold { (px / 24.0).ceil().max(1.0) as usize } else { 0 };
        for y in 0..original.height {
            let shear = if italic {
                ((original.ymin as f32 + (original.height - y) as f32) * 0.2).round() as i32
            } else { 0 };
            let offset = (original.xmin - metrics.xmin + shear).max(0) as usize;
            for x in 0..original.width {
                let value = bitmap[y * original.width + x];
                for spread in 0..=bold {
                    let index = y * metrics.width + offset + x + spread;
                    if offset + x + spread < metrics.width { styled[index] = styled[index].max(value); }
                }
            }
        }
        (metrics, styled)
    }

    /// Whether any face in this stack contains a glyph for `ch`.
    pub fn has_glyph(&self, ch: char) -> bool {
        self.face_for(ch).inner.lookup_glyph_index(ch) != 0
    }

    fn face_for(&self, ch: char) -> &Self {
        if self.inner.lookup_glyph_index(ch) != 0 {
            return self;
        }
        self.fallbacks
            .iter()
            .map(|font| font.face_for(ch))
            .find(|font| font.inner.lookup_glyph_index(ch) != 0)
            .unwrap_or(self)
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
            let g = self.face_for(ch).inner.metrics(ch, px);
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
            let g = self.face_for(ch).inner.metrics(ch, px);
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

    /// Wrap at Unicode line opportunities, falling back to whole grapheme clusters.
    /// Keeps source scalars and whitespace intact; explicit newlines retain empty lines.
    /// A single cluster wider than the viewport may overflow and must be clipped by the caller.
    pub fn layout_word_wrapped(&self, text: &str, px: f32, width: f32) -> TextLayout {
        let mut layout=self.layout(text,px,None);
        let limit=if width.is_finite() {width.max(1.0)}else{f32::MAX};
        let ascent=self.metrics(px).ascent;
        let space=|ch:char|ch.is_whitespace() && !matches!(ch,'\u{a0}'|'\u{202f}');
        let (mut start,mut glyph,mut x,mut line,mut widest)=(0,0,0.0f32,0usize,0.0f32);
        for (end,_) in unicode_linebreak::linebreaks(text) {
            let segment=&text[start..end];start=end;
            let count=segment.trim_end_matches(space).chars().filter(|ch|*ch!='\n').count();
            let word_width=layout.glyphs[glyph..glyph+count].iter().map(|g|g.advance).sum::<f32>();
            if x>0.0 && word_width<=limit && x+word_width>limit {widest=widest.max(x);x=0.0;line+=1;}
            for cluster in segment.graphemes(true) {
                let count=cluster.chars().filter(|ch|*ch!='\n').count();
                let glyphs=&mut layout.glyphs[glyph..glyph+count];glyph+=count;
                let advance=glyphs.iter().map(|g|g.advance).sum::<f32>();
                let whitespace=cluster.chars().all(space);
                if !whitespace && x>0.0 && x+advance>limit {widest=widest.max(x);x=0.0;line+=1;}
                for glyph in glyphs {
                    // Trailing wrap whitespace retains its source position without creating an empty line.
                    if whitespace {glyph.advance=glyph.advance.min((limit-x).max(0.0));}
                    glyph.x=x;glyph.baseline_y=line as f32*layout.line_height+ascent;x+=glyph.advance;
                }
                if cluster.contains('\n') {widest=widest.max(x);x=0.0;line+=1;}
            }
        }
        layout.width=widest.max(x);layout.height=(line+1) as f32*layout.line_height;
        layout
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
            let (metrics, bitmap) = self.rasterize_glyph(g.ch, px);
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
            blit_alpha(AlphaBlit {
                dest: &mut buf,
                dest_w: w,
                dest_h: h,
                x: ox + pad_x,
                y: oy + pad_y,
                src_w: gw,
                src_h: gh,
                src: &bitmap,
            });
        }
        AlphaMask {
            width: w,
            height: h,
            pixels: Arc::from(buf),
        }
    }

    /// Rasterize into a fixed viewport without padding negative glyph bearings.
    ///
    /// Glyph positions are relative to the viewport origin; offscreen glyphs are
    /// skipped. Invalid geometry or masks above 64 MiB return an empty mask.
    pub fn rasterize_layout_clipped(&self, layout: &TextLayout, px: f32, width: u32, height: u32) -> AlphaMask {
        // shortcut: 64 MiB mask/glyph budget; tile viewports if larger text surfaces are needed.
        const MAX_PIXELS: usize = 64 * 1024 * 1024;
        let Some(count) = (width as usize).checked_mul(height as usize)
            .filter(|count| *count > 0 && *count <= MAX_PIXELS) else { return AlphaMask::empty(); };
        if !px.is_finite() || px <= 0.0 { return AlphaMask::empty(); }
        let mut pixels = vec![0; count];
        for glyph in &layout.glyphs {
            if !glyph.x.is_finite() || !glyph.baseline_y.is_finite() { continue; }
            let metrics = self.glyph_metrics(glyph.ch, px);
            let x = (glyph.x + metrics.xmin as f32).round();
            let y = (glyph.baseline_y - metrics.ymin as f32 - metrics.height as f32).round();
            if x >= width as f32 || y >= height as f32
                || x + metrics.width as f32 <= 0.0 || y + metrics.height as f32 <= 0.0 { continue; }
            if metrics.width.checked_mul(metrics.height).is_none_or(|count| count > MAX_PIXELS) { continue; }
            let (_, bitmap) = self.rasterize_glyph(glyph.ch, px);
            blit_alpha(AlphaBlit { dest: &mut pixels, dest_w: width, dest_h: height,
                x: x as i32, y: y as i32, src_w: metrics.width as u32,
                src_h: metrics.height as u32, src: &bitmap });
        }
        AlphaMask { width, height, pixels: Arc::from(pixels) }
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

impl TextLayout {
    /// Top-left of the insertion caret for a Unicode scalar index into `text`.
    ///
    /// `text` must be the same string passed to [`Font::layout`] / [`Font::rasterize_text`]
    /// that produced this layout (including the same wrap width). Explicit newlines and
    /// automatic wraps both advance the caret onto the next visual line.
    pub fn caret_top_left(&self, text: &str, caret: usize, ascent: f32) -> (f32, f32) {
        let positions = self.caret_positions(text, ascent);
        positions[caret.min(positions.len() - 1)]
    }

    /// Insertion positions for every Unicode scalar boundary, including end of text.
    /// Uses the same wrapping as glyph painting; suitable for selection and hit testing.
    pub fn caret_positions(&self, text: &str, ascent: f32) -> Vec<(f32, f32)> {
        let mut positions = Vec::with_capacity(text.chars().count() + 1);
        let mut glyphs = self.glyphs.iter();
        let (mut x, mut y) = (0.0, 0.0);
        for ch in text.chars() {
            if ch == '\n' {
                positions.push((x, y));
                x = 0.0;
                y += self.line_height;
            } else if let Some(g) = glyphs.next() {
                y = (g.baseline_y - ascent).max(0.0);
                positions.push((g.x, y));
                x = g.x + g.advance;
            }
        }
        positions.push((x, y));
        positions
    }

}

struct AlphaBlit<'a> {
    dest: &'a mut [u8],
    dest_w: u32,
    dest_h: u32,
    x: i32,
    y: i32,
    src_w: u32,
    src_h: u32,
    src: &'a [u8],
}

fn blit_alpha(b: AlphaBlit<'_>) {
    for row in 0..b.src_h as i32 {
        let dy = b.y + row;
        if dy < 0 || dy >= b.dest_h as i32 {
            continue;
        }
        for col in 0..b.src_w as i32 {
            let dx = b.x + col;
            if dx < 0 || dx >= b.dest_w as i32 {
                continue;
            }
            let s = b.src[(row as u32 * b.src_w + col as u32) as usize];
            let di = (dy as u32 * b.dest_w + dx as u32) as usize;
            // max blend for overlapping coverage
            if s > b.dest[di] {
                b.dest[di] = s;
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

/// Load a system monospace face for byte grids and other fixed-column ASCII content.
pub fn load_default_monospace_font() -> Result<Font, FontError> {
    for path in ["/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationMono-Regular.ttf",
        "/System/Library/Fonts/Menlo.ttc", "/System/Library/Fonts/Monaco.ttf",
        "C:\\Windows\\Fonts\\consola.ttf", "C:\\Windows\\Fonts\\cour.ttf"] {
        if let Ok(font)=Font::from_path(path,0) {return Ok(font);}
    }
    Err(FontError("No system monospace font found".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_font() -> Font {
        load_default_cjk_font().expect("system CJK font required for tests")
    }

    #[test]
    fn word_layout_preserves_words_punctuation_clusters_and_exact_source_carets() {
        let font=test_font();let size=18.0;let ascent=font.metrics(size).ascent;
        let text="first second";let width=font.measure_line("first ",size).0.max(font.measure_line("second",size).0)+2.0;
        let layout=font.layout_word_wrapped(text,size,width);let points=layout.caret_positions(text,ascent);
        assert_eq!(font.layout(text,size,Some(width)).caret_positions(text,ascent)[6].1,0.0);
        assert!(points[6].1>points[0].1);assert!(points[6..12].iter().all(|point|point.1==points[6].1));
        assert_eq!(layout.glyphs.iter().map(|glyph|glyph.ch).collect::<String>(),text);
        for text in ["", "\n", "\n\n", "first\n\nsecond\n", "  first  second  ", "中文（标点）继续", "a\u{a0}b c\u{202f}d", "first\r\nsecond"] {
            let layout=font.layout_word_wrapped(text,size,2000.0);let original=font.layout(text,size,None);
            assert_eq!(layout.glyphs.iter().map(|g|(g.ch,g.x,g.baseline_y,g.advance)).collect::<Vec<_>>(),original.glyphs.iter().map(|g|(g.ch,g.x,g.baseline_y,g.advance)).collect::<Vec<_>>(),"{text:?}");
            assert_eq!(layout.height,original.height);assert_eq!(layout.caret_positions(text,ascent).len(),text.chars().count()+1);
        }
        for text in ["abcdefghij".repeat(15),"e\u{301}".repeat(25),"👩\u{200d}💻".repeat(15),"🇨🇳🇯🇵".repeat(12)] {
            let layout=font.layout_word_wrapped(&text,size,35.0);let points=layout.caret_positions(&text,ascent);let mut scalar=0;
            assert!(layout.height>layout.line_height);assert_eq!(layout.glyphs.iter().map(|g|g.ch).collect::<String>(),text);
            for cluster in text.graphemes(true) {let end=scalar+cluster.chars().count();assert!(points[scalar..end].iter().all(|point|point.1==points[scalar].1),"Split {cluster:?}");scalar=end;}
        }
        for text in ["中文（标点）继续", "中文，继续。测试！", "日本語「引用」続く"] {
            let layout=font.layout_word_wrapped(text,size,font.measure_line("中文（",size).0+0.5);
            for pair in layout.glyphs.windows(2) {if pair[0].baseline_y!=pair[1].baseline_y {
                assert!(!matches!(pair[0].ch,'（'|'「'));assert!(!matches!(pair[1].ch,'）'|'」'|'，'|'。'|'！'));
            }}
        }
        for width in [0.0,-1.0,f32::NAN,f32::INFINITY] {let layout=font.layout_word_wrapped("a\u{301}中文",size,width);assert!(layout.width.is_finite() && layout.height.is_finite());}
        let width=font.measure_line("second",size).0;
        for text in ["second ","second   ","second \n","second   \n"] {
            let layout=font.layout_word_wrapped(text,size,width);
            assert_eq!(layout.height,layout.line_height*if text.ends_with('\n') {2.0}else{1.0});assert!(layout.width<=width);
            assert_eq!(layout.glyphs.iter().map(|g|g.ch).collect::<String>(),text.replace('\n',""));
        }
    }

    #[test]
    fn clipped_raster_matches_crop_and_bounds_long_offscreen_lines() {
        let font=test_font();
        let mut layout=font.layout("中文 Hello world — Latin\n第二行 alpha beta 中文",24.0,None);
        let full=font.rasterize_layout(&layout,24.0);
        for glyph in &mut layout.glyphs {glyph.x-=19.0;glyph.baseline_y-=3.0;}
        let clipped=font.rasterize_layout_clipped(&layout,24.0,120,40);
        assert_eq!((clipped.width,clipped.height),(120,40));
        assert!(clipped.pixels.iter().any(|p|*p>0));
        for y in 0..40 {for x in 0..120 {
            assert_eq!(clipped.pixels[(y*120+x) as usize],full.pixels[((y+3)*full.width+x+19) as usize]);
        }}
        let mut long=font.layout(&"中".repeat(100_000),24.0,None);
        let end=long.glyphs[99_990].x;
        for glyph in &mut long.glyphs {glyph.x-=end;}
        let clipped=font.rasterize_layout_clipped(&long,24.0,120,40);
        assert_eq!(clipped.pixels.len(),120*40);
        assert!(clipped.pixels.iter().any(|p|*p>0));
        assert!(font.rasterize_layout_clipped(&long,24.0,u32::MAX,u32::MAX).is_empty());
        assert!(font.rasterize_layout_clipped(&long,f32::NAN,120,40).is_empty());
    }

    #[test]
    fn fallback_keeps_primary_metrics_and_glyphs() {
        let font = test_font();
        let stacked = font.clone().with_fallback(font.clone());
        assert_eq!(font.metrics(18.0), stacked.metrics(18.0));
        assert_eq!(font.measure_line("Latin 中文", 18.0), stacked.measure_line("Latin 中文", 18.0));
        let (_, original) = font.rasterize_text("Latin 中文", 18.0, None);
        let (_, actual) = stacked.rasterize_text("Latin 中文", 18.0, None);
        assert_eq!(original.pixels, actual.pixels);
        assert!(!stacked.has_glyph('\u{10ffff}'));
    }

    #[test]
    fn synthetic_faces_keep_advances_and_clipped_metrics_consistent() {
        let regular = test_font();
        let text = "Ag jp 中文\nStyle";
        let base = regular.rasterize_text(text, 24.0, None).1;
        for (bold, italic) in [(true, false), (false, true), (true, true)] {
            let font = regular.clone().with_synthetic_style(bold, italic);
            assert_eq!(regular.measure_line(text, 24.0), font.measure_line(text, 24.0));
            let mut layout = font.layout(text, 24.0, None);
            let full = font.rasterize_layout(&layout, 24.0);
            assert!(full.width != base.width || full.pixels != base.pixels);
            if bold { assert!(full.pixels.iter().map(|value| *value as u64).sum::<u64>() >= base.pixels.iter().map(|value| *value as u64).sum()); }
            // Shift negative-bearing glyphs into the viewport before comparing paths.
            for glyph in &mut layout.glyphs { glyph.x += 12.0; glyph.baseline_y += 6.0; }
            layout.width += 24.0; layout.height += 12.0;
            let full = font.rasterize_layout(&layout, 24.0);
            let clipped = font.rasterize_layout_clipped(&layout, 24.0, full.width, full.height);
            assert_eq!(full.pixels, clipped.pixels);
            assert!(font.rasterize_text("  ", 24.0, None).1.is_empty());
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn missing_cjk_symbols_use_the_fallback_for_measure_layout_and_pixels() {
        let primary = Font::from_path("/System/Library/Fonts/STHeiti Medium.ttc", 1).unwrap();
        let symbols = Font::from_path("/System/Library/Fonts/Apple Symbols.ttf", 0).unwrap();
        let ch = '▾';
        assert!(!primary.has_glyph(ch));
        assert!(symbols.has_glyph(ch));
        let stacked = primary.clone().with_fallback(symbols.clone());
        assert!(stacked.has_glyph(ch));
        assert_eq!(stacked.measure_line("▾", 18.0).0, symbols.measure_line("▾", 18.0).0);
        let layout = stacked.layout("A▾中", 18.0, None);
        assert_eq!(layout.glyphs[1].advance, symbols.measure_line("▾", 18.0).0);
        let (metrics, expected) = symbols.inner.rasterize(ch, 18.0);
        let mut isolated = stacked.layout("▾", 18.0, None);
        isolated.glyphs[0].baseline_y = metrics.height as f32 + metrics.ymin as f32;
        isolated.height = metrics.height as f32;
        isolated.width = metrics.width as f32;
        isolated.glyphs[0].x = -(metrics.xmin as f32);
        let mask = stacked.rasterize_layout(&isolated, 18.0);
        assert_eq!(mask.pixels.as_ref(), expected);
        assert_eq!(stacked.measure_line("A中", 18.0), primary.measure_line("A中", 18.0));
        let styled = primary.with_fallback(symbols.clone().with_synthetic_style(true, true));
        let normal = stacked.rasterize_text("▾", 18.0, None).1;
        let mask = styled.rasterize_text("▾", 18.0, None).1;
        assert!(normal.width != mask.width || normal.pixels != mask.pixels);
        assert_eq!(styled.measure_line("▾", 18.0), stacked.measure_line("▾", 18.0));
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
    fn wrapped_caret_starts_at_next_glyph_and_keeps_empty_lines() {
        let font = load_default_cjk_font().unwrap();
        let text = "中文WWWWWW\n\nZ";
        let layout = font.layout(text, 18.0, Some(55.0));
        let positions = layout.caret_positions(text, font.metrics(18.0).ascent);
        assert_eq!(positions.len(), text.chars().count() + 1);
        for (index, glyph) in layout.glyphs.iter().enumerate().take(8) {
            assert_eq!(positions[index].0, glyph.x);
            assert!((positions[index].1 + font.metrics(18.0).ascent - glyph.baseline_y).abs() < 0.01);
        }
        assert!(positions[10].1 > positions[9].1, "empty line remains navigable");
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

    #[test]
    fn caret_follows_wrapped_layout() {
        let font = test_font();
        let px = 20.0;
        let max_w = 80.0;
        let text = "W".repeat(40);
        let layout = font.layout(&text, px, Some(max_w));
        assert!(layout.height > layout.line_height * 1.5, "expected wrap");
        let m = font.metrics(px);
        let end = text.chars().count();
        let (cx, cy) = layout.caret_top_left(&text, end, m.ascent);
        assert!(
            cx <= max_w + 1.0,
            "wrapped end caret x={cx} should stay within wrap width {max_w}"
        );
        assert!(
            cy >= layout.line_height * 0.5,
            "wrapped end caret y={cy} should be on a wrapped line"
        );
        let (x0, y0) = layout.caret_top_left(&text, 0, m.ascent);
        assert_eq!((x0, y0), (0.0, 0.0));
        // Explicit newline still advances a visual line.
        let nl = "ab\nc";
        let layout_nl = font.layout(nl, px, Some(max_w));
        let (cx_nl, cy_nl) = layout_nl.caret_top_left(nl, 3, m.ascent); // after '\n'
        assert!(cy_nl >= layout_nl.line_height * 0.9, "caret after newline y={cy_nl}");
        assert!(cx_nl < 1.0, "caret after newline should be at line start, x={cx_nl}");
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
