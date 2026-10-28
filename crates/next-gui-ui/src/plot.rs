//! Cartesian plot data, axes and retained interaction state.

use crate::{ChartKind, DragState, Kind, Ui, UiKey, WidgetId};
use next_gui_scene::{Color, Rect, RgbaImage};
use std::{collections::BTreeSet, sync::Arc};

/// A plotted sample linked to its source row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlotPoint {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
    /// Stable row index in the source snapshot.
    pub row: usize,
}

/// Connected-line style.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlotLine {
    /// Draw only point markers.
    None,
    /// Join adjacent samples.
    #[default]
    Line,
    /// Hold each value until the next horizontal coordinate.
    StepLeft,
    /// Change to the next value at the previous horizontal coordinate.
    StepRight,
    /// Change values halfway between adjacent samples.
    StepCenter,
    /// Draw a vertical stroke from zero for each sample.
    Impulse,
    /// Draw categorical bars from zero.
    Bars,
}

/// Shape drawn at each sample.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlotMarker {
    /// No marker.
    #[default]
    None,
    /// Diagonal cross.
    Cross,
    /// Upright cross.
    Plus,
    /// Circle outline.
    Circle,
    /// Filled circle.
    Disc,
    /// Square outline.
    Square,
    /// Diamond outline.
    Diamond,
    /// Combined upright and diagonal cross.
    Star,
    /// Upward triangle.
    Triangle,
    /// Downward triangle.
    TriangleInverted,
    /// Cross inside a square.
    CrossSquare,
    /// Plus inside a square.
    PlusSquare,
    /// Cross inside a circle.
    CrossCircle,
    /// Plus inside a circle.
    PlusCircle,
    /// Circle with three radial strokes.
    Peace,
}

/// One independently colored series, on either vertical axis.
#[derive(Debug, Clone)]
pub struct PlotSeries {
    /// Legend entry.
    pub name: String,
    /// Stroke, marker and bar color.
    pub color: Color,
    /// Use the right vertical axis instead of the left.
    pub right_axis: bool,
    /// Shared samples; repainting does not copy the source data.
    pub points: Arc<[PlotPoint]>,
    /// Line or bar presentation.
    pub line: PlotLine,
    /// Point presentation.
    pub marker: PlotMarker,
}

/// Tick formatting and parsing for the horizontal axis.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum PlotAxisFormat {
    /// Finite numbers.
    #[default]
    Numeric,
    /// ISO dates, represented as seconds since 1970-01-01.
    Date,
    /// Wall-clock seconds since midnight.
    Time,
    /// ISO date and time, with a space or `T` separator; no timezone conversion.
    DateTime,
    /// Category names at integer coordinates.
    Labels(Arc<[String]>),
}

impl PlotAxisFormat {
    /// Parse a finite number or a strict ISO date/time in this axis format.
    pub fn parse(&self, value: &str) -> Option<f64> {
        match self {
            Self::Numeric => value.trim().parse::<f64>().ok().filter(|v| v.is_finite()),
            Self::Date => parse_date(value).map(|days| days as f64 * 86400.0),
            Self::Time => parse_time(value).map(|seconds| seconds as f64),
            Self::DateTime => {
                let (date, time) = value.split_once(['T', ' '])?;
                Some(parse_date(date)? as f64 * 86400.0 + f64::from(parse_time(time)?))
            }
            Self::Labels(labels) => labels
                .iter()
                .position(|label| label == value)
                .map(|i| i as f64),
        }
    }

    /// Format a finite axis coordinate; invalid/unrepresentable coordinates are empty.
    pub fn label(&self, value: f64) -> String {
        if !value.is_finite() {
            return String::new();
        }
        match self {
            Self::Numeric => numeric_label(value),
            Self::Labels(labels) => {
                if value < 0.0 || (value - value.round()).abs() > 1e-6 {
                    return String::new();
                }
                labels
                    .get(value.round() as usize)
                    .cloned()
                    .unwrap_or_default()
            }
            Self::Time => {
                let seconds = value.round().rem_euclid(86400.0) as i64;
                format!(
                    "{:02}:{:02}:{:02}",
                    seconds / 3600,
                    seconds / 60 % 60,
                    seconds % 60
                )
            }
            Self::Date | Self::DateTime => {
                if !(-62135596800.0..=253402300799.0).contains(&value) {
                    return String::new();
                }
                let seconds = value.floor() as i64;
                let (year, month, day) = civil_date(seconds.div_euclid(86400));
                let date = format!("{year:04}-{month:02}-{day:02}");
                if *self == Self::Date {
                    date
                } else {
                    format!(
                        "{date} {}",
                        Self::Time.label(seconds.rem_euclid(86400) as f64)
                    )
                }
            }
        }
    }
}

fn numeric_label(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if value.abs() >= 1e7 || value.abs() < 1e-4 {
        return format!("{value:.3e}");
    }
    let text = format!("{value:.6}");
    text.trim_end_matches('0').trim_end_matches('.').into()
}

fn parse_date(value: &str) -> Option<i64> {
    let b = value.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let number = |slice: &[u8]| {
        slice.iter().try_fold(0i64, |n, &b| {
            b.is_ascii_digit().then(|| n * 10 + i64::from(b - b'0'))
        })
    };
    let year = number(&b[..4])?;
    let month = number(&b[5..7])?;
    let day = number(&b[8..])?;
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let length = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ][month as usize - 1];
    if day < 1 || day > length {
        return None;
    }
    let adjusted = year - i64::from(month <= 2);
    let era = adjusted.div_euclid(400);
    let within = adjusted - era * 400;
    let march = month + if month > 2 { -3 } else { 9 };
    Some(
        era * 146097 + within * 365 + within / 4 - within / 100 + (153 * march + 2) / 5 + day
            - 1
            - 719468,
    )
}

fn parse_time(value: &str) -> Option<u32> {
    let b = value.as_bytes();
    if b.len() != 8
        || b[2] != b':'
        || b[5] != b':'
        || [0, 1, 3, 4, 6, 7].iter().any(|&i| !b[i].is_ascii_digit())
    {
        return None;
    }
    let number = |i: usize| u32::from(b[i] - b'0') * 10 + u32::from(b[i + 1] - b'0');
    let (h, m, s) = (number(0), number(3), number(6));
    (h < 24 && m < 60 && s < 60).then_some(h * 3600 + m * 60 + s)
}

fn civil_date(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719468;
    let era = shifted.div_euclid(146097);
    let within = shifted - era * 146097;
    let year = (within - within / 1460 + within / 36524 - within / 146096) / 365;
    let day = within - (365 * year + year / 4 - year / 100);
    let march = (5 * day + 2) / 153;
    let month = march + if march < 10 { 3 } else { -9 };
    (
        year + era * 400 + i64::from(month <= 2),
        month,
        day - (153 * march + 2) / 5 + 1,
    )
}

/// Axes affected by dragging and wheel zoom.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlotAxis {
    /// Horizontal and both vertical axes.
    #[default]
    All,
    /// Only the horizontal axis.
    X,
    /// Only the left vertical axis.
    Y1,
    /// Only the right vertical axis.
    Y2,
}

/// Visible coordinate ranges; each pair must be finite and increasing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlotView {
    /// Horizontal range.
    pub x: (f64, f64),
    /// Left vertical range.
    pub y1: (f64, f64),
    /// Right vertical range.
    pub y2: (f64, f64),
}

impl PlotView {
    /// Whether every axis has a finite, increasing interval.
    pub fn is_valid(&self) -> bool {
        [self.x, self.y1, self.y2]
            .iter()
            .all(|&(lo, hi)| lo.is_finite() && hi.is_finite() && lo < hi)
    }
}

/// Retained viewport, selected source rows and axis constraint.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlotState {
    /// Explicit viewport, or automatic limits.
    pub view: Option<PlotView>,
    /// Source row indexes highlighted in every series.
    pub selected_rows: Arc<BTreeSet<usize>>,
    /// Axes affected by subsequent navigation.
    pub active_axis: PlotAxis,
    anchor: Option<usize>,
}

/// Shared plot model; point buffers survive cheap model clones and UI rebuilds.
#[derive(Debug, Clone)]
pub struct PlotData {
    /// Horizontal axis caption.
    pub x_label: String,
    /// Left axis caption.
    pub y1_label: String,
    /// Right axis caption.
    pub y2_label: String,
    /// Horizontal tick representation.
    pub x_format: PlotAxisFormat,
    /// Independent left/right series.
    pub series: Vec<PlotSeries>,
    /// Navigation and selection session.
    pub state: PlotState,
    /// Reserve space for series labels above the axes (enabled by default).
    pub show_legend: bool,
    /// Use fixed decimal numeric ticks, with precision derived from the visible range.
    pub fixed_number_format: bool,
}

impl Default for PlotData {
    fn default() -> Self {
        Self { x_label: String::new(), y1_label: String::new(), y2_label: String::new(),
            x_format: PlotAxisFormat::default(), series: Vec::new(), state: PlotState::default(), show_legend: true, fixed_number_format: false }
    }
}

impl PlotData {
    /// Automatic finite limits, including zero for bars and impulses.
    pub fn auto_view(&self) -> PlotView {
        let mut ranges = [(f64::INFINITY, f64::NEG_INFINITY); 3];
        for series in &self.series {
            let axis = 1 + usize::from(series.right_axis);
            if matches!(series.line, PlotLine::Bars | PlotLine::Impulse) {
                ranges[axis].0 = ranges[axis].0.min(0.0);
                ranges[axis].1 = ranges[axis].1.max(0.0);
            }
            for point in series
                .points
                .iter()
                .filter(|p| p.x.is_finite() && p.y.is_finite())
            {
                ranges[0].0 = ranges[0].0.min(point.x);
                ranges[0].1 = ranges[0].1.max(point.x);
                ranges[axis].0 = ranges[axis].0.min(point.y);
                ranges[axis].1 = ranges[axis].1.max(point.y);
            }
        }
        PlotView {
            x: limits([ranges[0].0, ranges[0].1].into_iter()),
            y1: limits([ranges[1].0, ranges[1].1].into_iter()),
            y2: limits([ranges[2].0, ranges[2].1].into_iter()),
        }
    }

    /// Zoom the active axes around fractional plot coordinates (bottom-left is zero).
    pub fn zoom(&mut self, factor: f64, anchor_x: f64, anchor_y: f64) -> bool {
        if !factor.is_finite() || factor <= 0.0 || !anchor_x.is_finite() || !anchor_y.is_finite() {
            return false;
        }
        let current = self.view();
        let mut next = current;
        let zoom = |(lo, hi): (f64, f64), position: f64| {
            let position = position.clamp(0.0, 1.0);
            let anchor = lo * (1.0 - position) + hi * position;
            (
                lo * factor + anchor * (1.0 - factor),
                hi * factor + anchor * (1.0 - factor),
            )
        };
        if matches!(self.state.active_axis, PlotAxis::All | PlotAxis::X) {
            next.x = zoom(current.x, anchor_x);
        }
        if matches!(self.state.active_axis, PlotAxis::All | PlotAxis::Y1) {
            next.y1 = zoom(current.y1, anchor_y);
        }
        if matches!(self.state.active_axis, PlotAxis::All | PlotAxis::Y2) {
            next.y2 = zoom(current.y2, anchor_y);
        }
        if !next.is_valid() || next == current {
            return false;
        }
        self.state.view = Some(next);
        true
    }

    /// Pan active axes by fractions of their visible intervals.
    pub fn pan(&mut self, horizontal: f64, vertical: f64) -> bool {
        if !horizontal.is_finite() || !vertical.is_finite() {
            return false;
        }
        let current = self.view();
        let mut next = current;
        let shift = |(lo, hi): (f64, f64), amount: f64| {
            let delta = hi * amount - lo * amount;
            (lo + delta, hi + delta)
        };
        if matches!(self.state.active_axis, PlotAxis::All | PlotAxis::X) {
            next.x = shift(current.x, horizontal);
        }
        if matches!(self.state.active_axis, PlotAxis::All | PlotAxis::Y1) {
            next.y1 = shift(current.y1, vertical);
        }
        if matches!(self.state.active_axis, PlotAxis::All | PlotAxis::Y2) {
            next.y2 = shift(current.y2, vertical);
        }
        if !next.is_valid() || next == current {
            return false;
        }
        self.state.view = Some(next);
        true
    }

    /// Restore automatic limits while keeping the selected source rows.
    pub fn reset_view(&mut self) -> bool {
        self.state.view.take().is_some()
    }

    /// Select a source row, optionally extending from the previous anchor.
    pub fn select_row(&mut self, row: usize, extend: bool) -> bool {
        if !self
            .series
            .iter()
            .flat_map(|s| s.points.iter())
            .any(|p| p.row == row)
        {
            return false;
        }
        let anchor = if extend {
            self.state.anchor.unwrap_or(row)
        } else {
            row
        };
        let (lo, hi) = (anchor.min(row), anchor.max(row));
        let selected = Arc::new(
            self.series
                .iter()
                .flat_map(|s| s.points.iter())
                .filter(|p| (lo..=hi).contains(&p.row))
                .map(|p| p.row)
                .collect::<BTreeSet<_>>(),
        );
        let changed = self.state.selected_rows != selected;
        self.state.selected_rows = selected;
        self.state.anchor = Some(anchor);
        changed
    }

    /// Current valid viewport, falling back to finite automatic limits.
    pub fn view(&self) -> PlotView {
        self.state
            .view
            .filter(PlotView::is_valid)
            .unwrap_or_else(|| self.auto_view())
    }
}

/// A captured plot session, restored only for the same shared source point buffers.
#[derive(Debug, Clone)]
pub struct PlotSession {
    series: Vec<(Arc<[PlotPoint]>, bool)>,
    state: PlotState,
}

impl Ui {
    /// Cartesian data for a Chart tile, or `None` for another tile kind.
    pub fn chart_plot(&self, id: WidgetId, tile: usize) -> Option<&PlotData> {
        let Kind::Chart { tiles, .. } = &self.widgets.get(&id)?.kind else {
            return None;
        };
        let ChartKind::Plot { data, .. } = &tiles.get(tile)?.kind else {
            return None;
        };
        Some(data)
    }

    /// Edit a Cartesian model; sync the UI after changing it.
    pub fn chart_plot_mut(&mut self, id: WidgetId, tile: usize) -> Option<&mut PlotData> {
        let Kind::Chart {
            tiles, plot_images, ..
        } = &mut self.widgets.get_mut(&id)?.kind
        else {
            return None;
        };
        let ChartKind::Plot { data, .. } = &mut tiles.get_mut(tile)?.kind else {
            return None;
        };
        if let Some(image) = plot_images.get_mut(tile) {
            *image = None;
        }
        Some(data)
    }

    /// Capture navigation and source selection for a subsequent UI rebuild.
    pub fn plot_session(&self, id: WidgetId, tile: usize) -> Option<PlotSession> {
        let data = self.chart_plot(id, tile)?;
        Some(PlotSession {
            series: data
                .series
                .iter()
                .map(|s| (s.points.clone(), s.right_axis))
                .collect(),
            state: data.state.clone(),
        })
    }

    /// Restore a session only when every source buffer and axis assignment still matches.
    pub fn restore_plot_session(
        &mut self,
        id: WidgetId,
        tile: usize,
        session: &PlotSession,
    ) -> bool {
        let Some(data) = self.chart_plot_mut(id, tile) else {
            return false;
        };
        if data.series.len() != session.series.len()
            || !data
                .series
                .iter()
                .zip(&session.series)
                .all(|(s, (points, right))| {
                    s.right_axis == *right && Arc::ptr_eq(&s.points, points)
                })
        {
            return false;
        }
        data.state = session.state.clone();
        true
    }

    /// Dimensions of its last laid-out drawing surface, excluding the tile caption.
    pub fn chart_plot_size(&self, id: WidgetId, tile: usize) -> Option<(u32, u32)> {
        let Kind::Chart { tiles, plot_bounds, .. } = &self.widgets.get(&id)?.kind else { return None; };
        if !matches!(tiles.get(tile)?.kind, ChartKind::Plot { .. }) { return None; }
        let bounds = plot_bounds.get(tile)?;
        if bounds.width <= 0.0 || bounds.height <= 0.0 { return None; }
        Some((bounds.width.round().max(1.0) as u32, bounds.height.round().max(1.0) as u32))
    }

    /// Export a Cartesian tile with its current viewport and selection.
    pub fn chart_plot_image(
        &self,
        id: WidgetId,
        tile: usize,
        width: u32,
        height: u32,
    ) -> Option<RgbaImage> {
        let Kind::Chart { tiles, .. } = &self.widgets.get(&id)?.kind else {
            return None;
        };
        let ChartKind::Plot { data, style } = &tiles.get(tile)?.kind else {
            return None;
        };
        crate::render_plot(&self.font, data, *style, width, height)
    }

    fn plot_hit(&self, id: WidgetId, x: f32, y: f32) -> Option<(usize, Rect)> {
        let Kind::Chart {
            tiles, plot_bounds, ..
        } = &self.widgets.get(&id)?.kind
        else {
            return None;
        };
        plot_bounds
            .iter()
            .enumerate()
            .find(|(i, bounds)| {
                bounds.contains(x, y)
                    && matches!(tiles.get(*i).map(|t| &t.kind), Some(ChartKind::Plot { .. }))
            })
            .map(|(i, bounds)| (i, *bounds))
    }

    pub(crate) fn pointer_plot(&mut self, id: WidgetId, x: f32, y: f32, extend: bool) -> bool {
        let Some((tile, body)) = self.plot_hit(id, x, y) else {
            return false;
        };
        self.drag = None;
        let Some(data) = self.chart_plot_mut(id, tile) else {
            return false;
        };
        let mut area = crate::plot_raster::plot_area(body.width, body.height, data);
        area.x += body.x;
        area.y += body.y;
        let inside = area.contains(x, y);
        if !inside {
            data.state.active_axis = if y >= area.y + area.height {
                PlotAxis::X
            } else if x < area.x {
                PlotAxis::Y1
            } else if x > area.x + area.width {
                PlotAxis::Y2
            } else {
                PlotAxis::All
            };
        }
        let view = data.view();
        let mut nearest = (64.0, None);
        if inside {
            for series in &data.series {
                for point in series
                    .points
                    .iter()
                    .filter(|p| p.x.is_finite() && p.y.is_finite())
                {
                    let pixel =
                        crate::plot_raster::sample_pixel(point, series.right_axis, view, area);
                    let distance =
                        (pixel.0 - f64::from(x)).powi(2) + (pixel.1 - f64::from(y)).powi(2);
                    if distance < nearest.0 {
                        nearest = (distance, Some(point.row));
                    }
                }
            }
        }
        self.focus = Some(id);
        if inside {
            self.drag = Some(DragState::Plot {
                chart: id,
                tile,
                bounds: area,
                start_x: x,
                start_y: y,
                view,
                moved: false,
                row: nearest.1,
                extend,
            });
        }
        true
    }

    pub(crate) fn move_plot(&mut self, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let Some(DragState::Plot {
            chart,
            tile,
            bounds,
            start_x,
            start_y,
            view,
            moved,
            ..
        }) = self.drag.clone()
        else {
            return false;
        };
        let moved = moved || (x - start_x).hypot(y - start_y) >= 3.0;
        if !moved {
            return false;
        }
        if let Some(DragState::Plot { moved, .. }) = &mut self.drag {
            *moved = true;
        }
        let Some(data) = self.chart_plot_mut(chart, tile) else {
            return false;
        };
        let previous = data.state.view;
        data.state.view = Some(view);
        if !data.pan(
            f64::from(start_x - x) / f64::from(bounds.width),
            f64::from(y - start_y) / f64::from(bounds.height),
        ) {
            data.state.view = previous;
            return false;
        }
        data.state.view != previous
    }

    pub(crate) fn wheel_plot(&mut self, id: WidgetId, x: f32, y: f32, dy: f32) -> bool {
        let Some((tile, body)) = self.plot_hit(id, x, y) else {
            return false;
        };
        let Some(data) = self.chart_plot_mut(id, tile) else {
            return false;
        };
        let area = crate::plot_raster::plot_area(body.width, body.height, data);
        let factor = (f64::from(dy) * 0.002).clamp(-2.0, 2.0).exp();
        data.zoom(
            factor,
            f64::from(x - body.x - area.x) / f64::from(area.width),
            1.0 - f64::from(y - body.y - area.y) / f64::from(area.height),
        );
        true
    }

    pub(crate) fn key_plot(&mut self, id: WidgetId, key: UiKey) -> bool {
        let tile = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Chart { tiles, .. }) => tiles
                .iter()
                .position(|t| matches!(t.kind, ChartKind::Plot { .. })),
            _ => None,
        };
        let Some(data) = tile.and_then(|tile| self.chart_plot_mut(id, tile)) else {
            return false;
        };
        match key {
            UiKey::Home | UiKey::Char('0') => data.reset_view(),
            UiKey::Char('+' | '=') => data.zoom(0.8, 0.5, 0.5),
            UiKey::Char('-') => data.zoom(1.25, 0.5, 0.5),
            UiKey::Left => data.pan(-0.1, 0.0),
            UiKey::Right => data.pan(0.1, 0.0),
            UiKey::Up => data.pan(0.0, 0.1),
            UiKey::Down => data.pan(0.0, -0.1),
            UiKey::Escape => {
                let changed = data.state.active_axis != PlotAxis::All;
                data.state.active_axis = PlotAxis::All;
                changed
            }
            UiKey::Char('x' | 'X') => {
                data.state.active_axis = PlotAxis::X;
                true
            }
            UiKey::Char('y' | 'Y') => {
                data.state.active_axis = PlotAxis::Y1;
                true
            }
            UiKey::Char('r' | 'R') => {
                data.state.active_axis = PlotAxis::Y2;
                true
            }
            _ => false,
        }
    }
}

fn limits(values: impl Iterator<Item = f64>) -> (f64, f64) {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for v in values {
        if v.is_finite() {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    if !lo.is_finite() {
        return (-1.0, 1.0);
    }
    let pad = if lo == hi {
        (lo.abs() * 0.05).max(1.0)
    } else {
        (hi / 2.0 - lo / 2.0) * 0.1
    };
    ((lo - pad).max(-f64::MAX), (hi + pad).min(f64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_plot_selects_on_release_cancels_drags_and_keeps_only_matching_sessions() {
        use crate::{ChartStyle, ChartTile, ColumnStyle, PlotStyle};
        use next_gui_scene::Scene;
        let points: Arc<[PlotPoint]> = vec![
            PlotPoint {
                x: 0.0,
                y: 0.0,
                row: 4,
            },
            PlotPoint {
                x: 1.0,
                y: 1.0,
                row: 9,
            },
            PlotPoint {
                x: 2.0,
                y: 0.0,
                row: 18,
            },
        ]
        .into();
        let data = PlotData {
            x_label: "time".into(),
            y1_label: "values".into(),
            series: vec![PlotSeries {
                name: "series".into(),
                color: Color::rgb(20, 100, 240),
                right_axis: false,
                points: points.clone(),
                line: PlotLine::StepLeft,
                marker: PlotMarker::Circle,
            }],
            ..PlotData::default()
        };
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut id = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 600.0,
                ..ColumnStyle::default()
            },
            |c| {
                id = c.chart(
                    vec![ChartTile::plot("Chart", data.clone(), PlotStyle::default())],
                    ChartStyle {
                        width: 580.0,
                        height: 360.0,
                        columns: 1,
                        ..ChartStyle::default()
                    },
                )
            },
        );
        ui.sync(&mut scene);
        assert!(ui.focusable_widgets().contains(&id));
        let cached = match &ui.widgets[&id].kind {
            Kind::Chart { plot_images, .. } => plot_images[0].clone().unwrap(),
            _ => unreachable!(),
        };
        ui.sync(&mut scene);
        let Kind::Chart { plot_images, .. } = &ui.widgets[&id].kind else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(
            cached.pixels(),
            plot_images[0].as_ref().unwrap().pixels()
        ));
        assert!(ui.accessible_name(id).unwrap().contains("X: time"));
        let Kind::Chart { plot_bounds, .. } = &ui.widgets[&id].kind else {
            unreachable!()
        };
        let body = plot_bounds[0];
        let mut area = crate::plot_raster::plot_area(body.width, body.height, &data);
        area.x += body.x;
        area.y += body.y;
        let pixel = |i: usize| {
            let (x, y) = crate::plot_raster::sample_pixel(&points[i], false, data.view(), area);
            (x as f32, y as f32)
        };
        let (x, y) = pixel(0);
        assert!(ui.pointer_pressed(&mut scene, x, y));
        ui.sync(&mut scene);
        assert!(scene.get(ui.widgets[&id].aux[2]).unwrap().visible);
        let Kind::Chart { plot_images, .. } = &ui.widgets[&id].kind else {
            unreachable!()
        };
        assert!(!Arc::ptr_eq(
            cached.pixels(),
            plot_images[0].as_ref().unwrap().pixels()
        ));
        assert!(ui.chart_plot(id, 0).unwrap().state.selected_rows.is_empty());
        ui.cancel_pointer_capture();
        ui.pointer_released();
        assert!(ui.chart_plot(id, 0).unwrap().state.selected_rows.is_empty());
        assert!(ui.pointer_pressed(&mut scene, x, y));
        ui.pointer_released();
        assert_eq!(
            *ui.chart_plot(id, 0).unwrap().state.selected_rows,
            BTreeSet::from([4])
        );
        let (x, y) = pixel(2);
        assert!(ui.pointer_pressed_with_range_selection(&mut scene, x, y, true));
        ui.pointer_released();
        assert_eq!(
            *ui.chart_plot(id, 0).unwrap().state.selected_rows,
            BTreeSet::from([4, 9, 18])
        );
        let (x, y) = pixel(1);
        assert!(ui.pointer_pressed(&mut scene, x, y));
        assert!(ui.pointer_moved(x + 40.0, y + 20.0));
        ui.pointer_released();
        assert_eq!(
            *ui.chart_plot(id, 0).unwrap().state.selected_rows,
            BTreeSet::from([4, 9, 18])
        );
        let panned = ui.chart_plot(id, 0).unwrap().view();
        assert_ne!(panned, data.view());
        assert!(ui.pointer_scroll(&scene, x, y, 0.0, -100.0));
        let zoomed = ui.chart_plot(id, 0).unwrap().view();
        assert!(zoomed.x.1 - zoomed.x.0 < panned.x.1 - panned.x.0);
        let session = ui.plot_session(id, 0).unwrap();
        ui.set_chart_tiles(
            id,
            vec![ChartTile::plot(
                "Renamed",
                data.clone(),
                PlotStyle::default(),
            )],
        );
        assert!(ui.restore_plot_session(id, 0, &session));
        assert_eq!(ui.chart_plot(id, 0).unwrap().view(), zoomed);
        ui.set_focus(Some(id));
        assert!(ui.key_input(UiKey::Char('x')));
        assert!(ui.key_input(UiKey::Char('+')));
        assert_eq!(ui.chart_plot(id, 0).unwrap().view().y1, zoomed.y1);
        let constrained = ui.chart_plot(id, 0).unwrap().view();
        ui.sync(&mut scene);
        let center = (area.x + area.width / 2.0, area.y + area.height / 2.0);
        assert!(ui.pointer_pressed(&mut scene, center.0, center.1));
        assert!(ui.pointer_moved(center.0 + 20.0, center.1 + 20.0));
        ui.pointer_released();
        assert_eq!(ui.chart_plot(id, 0).unwrap().state.active_axis, PlotAxis::X);
        assert_eq!(ui.chart_plot(id, 0).unwrap().view().y1, constrained.y1);
        assert_ne!(ui.chart_plot(id, 0).unwrap().view().x, constrained.x);
        assert!(ui.key_input(UiKey::Escape));
        assert!(ui.pointer_pressed(&mut scene, center.0, center.1));
        assert!(ui.pointer_moved(center.0 + 20.0, center.1 + 20.0));
        ui.pointer_released();
        assert_ne!(ui.chart_plot(id, 0).unwrap().view().y1, constrained.y1);
        assert!(ui.key_input(UiKey::Home));
        assert_eq!(ui.chart_plot(id, 0).unwrap().view(), data.view());
        assert!(ui.chart_plot_size(id, 0).is_some_and(|(width, height)| width > 0 && height > 0));
        assert_eq!(ui.chart_plot_size(id, 1), None);
        assert_eq!(ui.chart_plot_size(WidgetId(0), 0), None);
        assert!(ui.chart_plot_image(id, 0, 400, 260).is_some());
        let mut changed = data;
        changed.series[0].points = points.to_vec().into();
        ui.set_chart_tiles(
            id,
            vec![ChartTile::plot(
                "Other snapshot",
                changed,
                PlotStyle::default(),
            )],
        );
        assert!(!ui.restore_plot_session(id, 0, &session));
    }
    #[test]
    fn calendar_axes_validate_and_roundtrip_centuries_and_negative_epochs() {
        for date in [
            "0001-01-01",
            "1900-02-28",
            "1969-12-31",
            "1970-01-01",
            "2000-02-29",
            "2026-10-28",
            "9999-12-31",
        ] {
            let value = PlotAxisFormat::Date.parse(date).unwrap();
            assert_eq!(PlotAxisFormat::Date.label(value), date);
        }
        for date in [
            "0000-01-01",
            "1900-02-29",
            "2026-02-29",
            "2026-13-01",
            "2026-04-31",
            "2026-1-01",
            "２０２６-01-01",
        ] {
            assert!(PlotAxisFormat::Date.parse(date).is_none(), "{date}");
        }
        assert_eq!(
            PlotAxisFormat::DateTime.parse("1969-12-31T23:59:59"),
            Some(-1.0)
        );
        assert_eq!(PlotAxisFormat::DateTime.label(-1.0), "1969-12-31 23:59:59");
        assert!(PlotAxisFormat::Time.parse("24:00:00").is_none());
        assert!(PlotAxisFormat::Time.parse("12:59:60").is_none());
        assert_eq!(PlotAxisFormat::Time.label(-1.0), "23:59:59");
        assert_eq!(PlotAxisFormat::Numeric.label(1e-8), "1.000e-8");
    }
    #[test]
    fn navigation_preserves_anchors_axes_and_sparse_source_selection() {
        let mut data = PlotData {
            series: vec![PlotSeries {
                name: "left".into(),
                color: Color::rgb(1, 2, 3),
                right_axis: false,
                points: vec![
                    PlotPoint {
                        x: 0.0,
                        y: -10.0,
                        row: 4,
                    },
                    PlotPoint {
                        x: 10.0,
                        y: 10.0,
                        row: usize::MAX,
                    },
                ]
                .into(),
                line: PlotLine::Line,
                marker: PlotMarker::None,
            }],
            ..Default::default()
        };
        let initial = data.view();
        data.state.active_axis = PlotAxis::X;
        assert!(data.zoom(0.5, 0.25, 0.75));
        let zoomed = data.view();
        assert_eq!(zoomed.y1, initial.y1);
        assert_eq!(zoomed.y2, initial.y2);
        let coordinate = |pair: (f64, f64)| pair.0 * 0.75 + pair.1 * 0.25;
        assert!((coordinate(initial.x) - coordinate(zoomed.x)).abs() < 1e-12);
        assert!(data.pan(0.1, 0.2));
        assert_eq!(data.view().y1, initial.y1);
        let valid = data.view();
        assert!(!data.zoom(f64::NAN, 0.5, 0.5));
        assert!(!data.pan(f64::INFINITY, 0.0));
        assert_eq!(data.view(), valid);
        assert!(data.select_row(4, false));
        assert!(data.select_row(usize::MAX, true));
        assert_eq!(data.state.selected_rows.len(), 2);
        assert!(!data.select_row(8, false));
        assert_eq!(data.state.selected_rows.len(), 2);
        assert!(data.reset_view());
        assert_eq!(data.view(), initial);
        assert_eq!(data.state.selected_rows.len(), 2);
    }

    #[test]
    fn auto_ranges_are_finite_for_missing_extreme_and_constant_data() {
        for samples in [
            vec![],
            vec![1.0],
            vec![f64::MAX],
            vec![-f64::MAX, f64::MAX],
            vec![f64::NAN, f64::INFINITY],
        ] {
            let data = PlotData {
                series: vec![PlotSeries {
                    name: "test".into(),
                    color: Color::rgb(1, 2, 3),
                    right_axis: false,
                    points: samples
                        .iter()
                        .enumerate()
                        .map(|(row, &y)| PlotPoint {
                            x: row as f64,
                            y,
                            row,
                        })
                        .collect::<Vec<_>>()
                        .into(),
                    line: PlotLine::Line,
                    marker: PlotMarker::Disc,
                }],
                ..Default::default()
            };
            assert!(data.auto_view().is_valid());
        }
    }
}
