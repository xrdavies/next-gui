//! Simple flex/box layout helpers for next-gui.
//!
//! P1: column/row placement from child measures into absolute [`Rect`]s.
//! Does not touch rendering backends.

#![deny(missing_docs)]

use next_gui_scene::Rect;

/// Main axis direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Top → bottom.
    Vertical,
    /// Left → right.
    Horizontal,
}

/// Parameters for a single flex layout pass.
#[derive(Debug, Clone, Copy)]
pub struct FlexParams {
    /// Main axis.
    pub axis: Axis,
    /// Origin x (content box includes padding inside).
    pub x: f32,
    /// Origin y.
    pub y: f32,
    /// Max width of the layout area (including padding).
    pub max_width: f32,
    /// Max height of the layout area (including padding).
    pub max_height: f32,
    /// Uniform padding inside the area.
    pub padding: f32,
    /// Gap between children along the main axis.
    pub gap: f32,
}

impl FlexParams {
    /// Column convenience.
    pub fn column(x: f32, y: f32, max_width: f32, max_height: f32) -> Self {
        Self {
            axis: Axis::Vertical,
            x,
            y,
            max_width,
            max_height,
            padding: 0.0,
            gap: 0.0,
        }
    }

    /// Row convenience.
    pub fn row(x: f32, y: f32, max_width: f32, max_height: f32) -> Self {
        Self {
            axis: Axis::Horizontal,
            x,
            y,
            max_width,
            max_height,
            padding: 0.0,
            gap: 0.0,
        }
    }
}

/// Desired size of one child before allocation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChildMeasure {
    /// Desired width.
    pub width: f32,
    /// Desired height.
    pub height: f32,
}

impl ChildMeasure {
    /// Construct.
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// Lay out children along the main axis; returns absolute bounds per child (same order).
///
/// Cross-axis: left/top aligned within the content box. Children are not clipped here;
/// callers may clip when syncing to the scene.
pub fn layout_flex(params: &FlexParams, children: &[ChildMeasure]) -> Vec<Rect> {
    let content_x = params.x + params.padding;
    let content_y = params.y + params.padding;
    let inner_w = (params.max_width - 2.0 * params.padding).max(0.0);
    let inner_h = (params.max_height - 2.0 * params.padding).max(0.0);

    let mut out = Vec::with_capacity(children.len());
    let mut cursor = 0.0f32;

    match params.axis {
        Axis::Vertical => {
            for (i, child) in children.iter().enumerate() {
                if i > 0 {
                    cursor += params.gap;
                }
                let w = child.width.min(inner_w);
                let h = child.height.min(inner_h.max(child.height));
                out.push(Rect::new(content_x, content_y + cursor, w, child.height));
                cursor += h.min(child.height);
                let _ = inner_h; // available height documented for future wrap/scroll
            }
        }
        Axis::Horizontal => {
            for (i, child) in children.iter().enumerate() {
                if i > 0 {
                    cursor += params.gap;
                }
                let w = child.width;
                let h = child.height.min(inner_h);
                out.push(Rect::new(content_x + cursor, content_y, w, h));
                cursor += w;
                let _ = inner_w;
            }
        }
    }
    out
}

/// Total size occupied by a flex layout (padding + children + gaps).
pub fn measure_flex(params: &FlexParams, children: &[ChildMeasure]) -> ChildMeasure {
    if children.is_empty() {
        return ChildMeasure::new(params.padding * 2.0, params.padding * 2.0);
    }
    let gaps = params.gap * (children.len().saturating_sub(1) as f32);
    match params.axis {
        Axis::Vertical => {
            let w = children
                .iter()
                .map(|c| c.width)
                .fold(0.0f32, f32::max)
                + params.padding * 2.0;
            let h = children.iter().map(|c| c.height).sum::<f32>() + gaps + params.padding * 2.0;
            ChildMeasure::new(w, h)
        }
        Axis::Horizontal => {
            let h = children
                .iter()
                .map(|c| c.height)
                .fold(0.0f32, f32::max)
                + params.padding * 2.0;
            let w = children.iter().map(|c| c.width).sum::<f32>() + gaps + params.padding * 2.0;
            ChildMeasure::new(w, h)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_stacks_with_gap() {
        let params = FlexParams {
            axis: Axis::Vertical,
            x: 10.0,
            y: 20.0,
            max_width: 200.0,
            max_height: 400.0,
            padding: 8.0,
            gap: 4.0,
        };
        let kids = [
            ChildMeasure::new(100.0, 20.0),
            ChildMeasure::new(80.0, 30.0),
        ];
        let rects = layout_flex(&params, &kids);
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], Rect::new(18.0, 28.0, 100.0, 20.0));
        assert_eq!(rects[1], Rect::new(18.0, 52.0, 80.0, 30.0));
    }

    #[test]
    fn row_places_left_to_right() {
        let params = FlexParams {
            padding: 0.0,
            gap: 10.0,
            ..FlexParams::row(0.0, 0.0, 500.0, 40.0)
        };
        let kids = [ChildMeasure::new(50.0, 20.0), ChildMeasure::new(40.0, 20.0)];
        let rects = layout_flex(&params, &kids);
        assert_eq!(rects[0].x, 0.0);
        assert_eq!(rects[1].x, 60.0);
    }
}
