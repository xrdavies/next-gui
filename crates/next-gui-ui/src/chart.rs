//! Retained multi-chart panel (Office enhancement on top of Gauge / Trend).
//!
//! [`Chart`] hosts a grid of [`ChartTile`]s (Gauge dials and/or Trend sparklines).
//! Standalone [`crate::Ui`] `gauge` / `trend` builders remain; paint/raster helpers are shared.

use next_gui_scene::Color;
use next_gui_text::{Font, TextLayout};
use unicode_segmentation::UnicodeSegmentation;

use crate::{GaugeStyle, TrendStyle};

/// One tile inside a [`Chart`] panel.
#[derive(Debug, Clone)]
pub struct ChartTile {
    /// Title above the body; wraps to at most three lines and half the tile height.
    /// Longer titles use a grapheme-safe ellipsis; the full text remains accessible.
    pub title: String,
    /// Gauge dial or Trend series.
    pub kind: ChartKind,
}

/// Tile content kind.
#[derive(Debug, Clone)]
pub enum ChartKind {
    /// Dial gauge (same semantics as standalone Gauge).
    Gauge {
        /// Current value.
        value: f32,
        /// Range minimum.
        min: f32,
        /// Range maximum.
        max: f32,
        /// Unit suffix (e.g. `%`).
        unit: String,
        /// Caption under the dial.
        label: String,
        /// Dial styling.
        style: GaugeStyle,
    },
    /// Sparkline / trend series.
    Trend {
        /// Sample points (oldest → newest).
        points: Vec<f32>,
        /// Series styling.
        style: TrendStyle,
    },
}

impl ChartTile {
    /// Gauge tile.
    pub fn gauge(
        title: impl Into<String>,
        value: f32,
        min: f32,
        max: f32,
        unit: impl Into<String>,
        label: impl Into<String>,
        style: GaugeStyle,
    ) -> Self {
        let lo = min.min(max);
        let hi = min.max(max);
        Self {
            title: title.into(),
            kind: ChartKind::Gauge {
                value: value.clamp(lo, hi),
                min: lo,
                max: hi,
                unit: unit.into(),
                label: label.into(),
                style,
            },
        }
    }

    /// Trend tile.
    pub fn trend(title: impl Into<String>, points: Vec<f32>, style: TrendStyle) -> Self {
        Self {
            title: title.into(),
            kind: ChartKind::Trend { points, style },
        }
    }
}

/// Style for a multi-tile Chart panel.
#[derive(Debug, Clone, Copy)]
pub struct ChartStyle {
    /// Outer width.
    pub width: f32,
    /// Outer height.
    pub height: f32,
    /// Grid column count (≥ 1).
    pub columns: usize,
    /// Gap between tiles.
    pub gap: f32,
    /// Inner padding.
    pub padding: f32,
    /// Panel background.
    pub bg: Color,
    /// Tile title color.
    pub title_fg: Color,
    /// Tile title font size.
    pub title_size: f32,
}

impl Default for ChartStyle {
    fn default() -> Self {
        Self {
            width: 640.0,
            height: 320.0,
            columns: 2,
            gap: 10.0,
            padding: 10.0,
            bg: Color::rgb(0x22, 0x22, 0x28),
            title_fg: Color::rgb(0xa0, 0xa8, 0xb4),
            title_size: 12.0,
        }
    }
}

pub(crate) fn title_layout(
    font: &Font,
    text: &str,
    size: f32,
    width: f32,
    height: f32,
) -> TextLayout {
    let height = height.min(font.metrics(size).line_height * 3.0);
    let layout = font.layout(text, size, Some(width));
    if layout.width <= width && layout.height <= height {
        return layout;
    }
    let mut fitted = font.layout("…", size, Some(width));
    if fitted.width > width || fitted.height > height {
        return font.layout("", size, None);
    }
    let boundaries: Vec<_> = text
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect();
    let (mut low, mut high) = (0, boundaries.len().saturating_sub(1));
    while low < high {
        let mid = low + (high - low).div_ceil(2);
        let candidate = font.layout(&format!("{}…", &text[..boundaries[mid]]), size, Some(width));
        if candidate.width <= width && candidate.height <= height {
            low = mid;
            fitted = candidate;
        } else {
            high = mid - 1;
        }
    }
    fitted
}

/// Compute grid cell rects inside a Chart panel (local to panel origin).
pub(crate) fn chart_tile_rects(style: ChartStyle, tile_count: usize) -> Vec<(f32, f32, f32, f32)> {
    let cols = style.columns.max(1);
    let n = tile_count;
    if n == 0 {
        return Vec::new();
    }
    let rows = n.div_ceil(cols);
    let inner_w = (style.width - style.padding * 2.0).max(8.0);
    let inner_h = (style.height - style.padding * 2.0).max(8.0);
    let cell_w = if cols > 1 {
        (inner_w - style.gap * (cols - 1) as f32) / cols as f32
    } else {
        inner_w
    };
    let cell_h = if rows > 1 {
        (inner_h - style.gap * (rows - 1) as f32) / rows as f32
    } else {
        inner_h
    };
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let c = i % cols;
        let r = i / cols;
        let x = style.padding + c as f32 * (cell_w + style.gap);
        let y = style.padding + r as f32 * (cell_h + style.gap);
        out.push((x, y, cell_w.max(8.0), cell_h.max(8.0)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_wraps_and_bounds_ellipsis_without_splitting_graphemes() {
        let font = next_gui_text::load_default_cjk_font().unwrap();
        let size = 16.0;
        let line = font.metrics(size).line_height;
        let short = title_layout(&font, "Revenue", size, 200.0, line * 3.0);
        assert_eq!(
            short.glyphs.iter().map(|g| g.ch).collect::<String>(),
            "Revenue"
        );
        let text = "季度收入 e\u{301} 👩‍💻 🇯🇵 ".repeat(12);
        let wrapped = title_layout(&font, &text, size, 120.0, line * 2.0 + 0.01);
        let shown: String = wrapped.glyphs.iter().map(|g| g.ch).collect();
        assert!(shown.ends_with('…'));
        let prefix = shown.strip_suffix('…').unwrap();
        assert!(text.starts_with(prefix));
        assert!(text.grapheme_indices(true).any(|(i, _)| i == prefix.len()));
        assert!(wrapped.width <= 120.0 && wrapped.height <= line * 2.0 + 0.01);
        let three = title_layout(&font, &text, size, 120.0, 1000.0);
        assert!(three.height <= line * 3.0);
        assert!(title_layout(&font, &text, size, 1.0, line)
            .glyphs
            .is_empty());
        assert!(title_layout(&font, &text, size, 120.0, 0.0)
            .glyphs
            .is_empty());
        let newline = title_layout(&font, "one\ntwo", size, 200.0, line * 3.0);
        assert_eq!(newline.height, line * 2.0);
    }

    #[test]
    fn tile_rects_two_by_two() {
        let style = ChartStyle {
            width: 420.0,
            height: 220.0,
            columns: 2,
            gap: 10.0,
            padding: 10.0,
            ..ChartStyle::default()
        };
        let rects = chart_tile_rects(style, 4);
        assert_eq!(rects.len(), 4);
        assert!((rects[0].0 - 10.0).abs() < 0.1);
        assert!(rects[1].0 > rects[0].0);
        assert!(rects[2].1 > rects[0].1);
    }
}
