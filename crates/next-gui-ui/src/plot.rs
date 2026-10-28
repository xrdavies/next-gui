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
    /// Draw grouped or stacked bars.
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
    /// Stack bars at matching X coordinates separately on each vertical axis.
    pub stacked_bars: bool,
}

impl Default for PlotData {
    fn default() -> Self {
        Self { x_label: String::new(), y1_label: String::new(), y2_label: String::new(),
            x_format: PlotAxisFormat::default(), series: Vec::new(), state: PlotState::default(), show_legend: true, fixed_number_format: false, stacked_bars: false }
    }
}

impl PlotData {
    /// Automatic finite limits, including grouped edges and stacked totals.
    /// Returns a unit viewport if an unsorted bar index cannot be allocated.
    pub fn auto_view(&self) -> PlotView {
        PlotBars::new(self).map(|bars| self.auto_view_with_bars(&bars)).unwrap_or(PlotView {x:(0.0,1.0),y1:(0.0,1.0),y2:(0.0,1.0)})
    }

    fn auto_view_with_bars(&self, bars: &PlotBars<'_>) -> PlotView {
        let mut ranges = [(f64::INFINITY, f64::NEG_INFINITY); 3];
        for (index,series) in self.series.iter().enumerate() {
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
                let (left,right,base,top)=if series.line==PlotLine::Bars {
                    let bar=bars.bar(index,point);(bar.left,bar.right,bar.base,bar.top)
                }else{(point.x,point.x,point.y,point.y)};
                ranges[0].0 = ranges[0].0.min(left);
                ranges[0].1 = ranges[0].1.max(right);
                ranges[axis].0 = ranges[axis].0.min(base.min(top));
                ranges[axis].1 = ranges[axis].1.max(base.max(top));
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

    pub(crate) fn view_with_bars(&self,bars:&PlotBars<'_>) -> PlotView {
        self.state.view.filter(PlotView::is_valid).unwrap_or_else(||self.auto_view_with_bars(bars))
    }
}

pub(crate) struct PlotBar {pub left:f64,pub right:f64,pub base:f64,pub top:f64}

// Sorted sources need no index; arbitrary numeric X keeps source order via an index.
pub(crate) struct PlotBars<'a> {
    data:&'a PlotData,
    orders:Vec<BarOrder>,
    count:usize,
}

struct BarOrder {rank:usize,indices:Option<Vec<usize>>,extrema:Vec<(f64,f64)>}

impl BarOrder {
    fn point<'a>(&self,series:&'a PlotSeries,index:usize) -> &'a PlotPoint {
        &series.points[self.indices.as_ref().map_or(index,|order|order[index])]
    }

    fn extreme(&self,series:&PlotSeries,lo:usize,hi:usize,positive:bool) -> f64 {
        let pick=|pair:(f64,f64)|if positive {pair.0}else{pair.1};
        let combine=|a:f64,b:f64|if positive {a.max(b)}else{a.min(b)};
        let scan=|range:std::ops::Range<usize>|range.map(|i|self.point(series,i).y).filter(|y|y.is_finite()).fold(0.0,combine);
        if self.extrema.is_empty() {return scan(lo..hi);}
        let start=lo.div_ceil(32)*32;let end=hi/32*32;
        if start>=end {return scan(lo..hi);}
        let mut value=combine(scan(lo..start),scan(end..hi));
        let leaves=self.extrema.len()/2;let mut a=leaves+start/32;let mut b=leaves+end/32;
        while a<b {
            if a%2==1 {value=combine(value,pick(self.extrema[a]));a+=1;}
            if b%2==1 {b-=1;value=combine(value,pick(self.extrema[b]));}
            a/=2;b/=2;
        }
        value
    }
}

impl<'a> PlotBars<'a> {
    pub(crate) fn new(data:&'a PlotData) -> Option<Self> {
        let count=data.series.iter().filter(|s|s.line==PlotLine::Bars).count();
        let mut orders=Vec::new();
        if count>0 {
            orders.try_reserve_exact(data.series.len()).ok()?;
            let mut rank=0;
            for series in &data.series {
                let order=if data.stacked_bars && series.line==PlotLine::Bars
                    && (!series.points.iter().all(|p|p.x.is_finite()) || !series.points.windows(2).all(|p|p[0].x<=p[1].x)) {
                    let mut order=Vec::new();order.try_reserve_exact(series.points.len()).ok()?;
                    order.extend((0..series.points.len()).filter(|&i|series.points[i].x.is_finite()));
                    order.sort_unstable_by(|&a,&b|series.points[a].x.total_cmp(&series.points[b].x));Some(order)
                }else{None};
                let mut order=BarOrder {rank,indices:order,extrema:Vec::new()};
                let len=order.indices.as_ref().map_or(series.points.len(),Vec::len);
                if data.stacked_bars && series.line==PlotLine::Bars && len>128
                    && (1..len).any(|i| {let a=order.point(series,i-1).x;let b=order.point(series,i).x;(b-a).abs()<=a.abs().max(b.abs()).max(1.0)*2e-14}) {
                    let leaves=len.div_ceil(32).checked_next_power_of_two()?;
                    let nodes=leaves.checked_mul(2)?;order.extrema.try_reserve_exact(nodes).ok()?;
                    order.extrema.resize(nodes,(0.0,0.0));
                    for i in 0..len {let y=order.point(series,i).y;if y.is_finite() {let node=&mut order.extrema[leaves+i/32];node.0=node.0.max(y);node.1=node.1.min(y);}}
                    for i in (1..leaves).rev() {order.extrema[i]=(order.extrema[i*2].0.max(order.extrema[i*2+1].0),order.extrema[i*2].1.min(order.extrema[i*2+1].1));}
                }
                orders.push(order);
                if series.line==PlotLine::Bars {rank+=1;}
            }
        }
        Some(Self {data,orders,count})
    }

    pub(crate) fn bar(&self,index:usize,point:&PlotPoint) -> PlotBar {
        let n=self.count.max(1) as f64;
        let (offset,half)=if self.data.stacked_bars {(0.0,0.425)}else{
            // Keep the original spacing, with a positive width even for seven or more series.
            ((self.orders[index].rank as f64+0.5)/n-0.5,0.5*(1.0/n-0.15).max(0.1/n))
        };
        let mut base=0.0;
        if self.data.stacked_bars {
            let epsilon=if point.x==0.0 {1e-14}else{(point.x.abs()*1e-14).max(f64::from_bits(1))};
            for (prior,series) in self.data.series[..index].iter().enumerate().filter(|(_,s)|s.line==PlotLine::Bars && s.right_axis==self.data.series[index].right_axis) {
                let order=&self.orders[prior];
                let len=order.indices.as_ref().map_or(series.points.len(),Vec::len);
                let sample=|i|order.point(series,i);
                let mut lo=0;let mut hi=len;
                while lo<hi {let mid=lo+(hi-lo)/2;if sample(mid).x<=point.x-epsilon {lo=mid+1;}else{hi=mid;}}
                let start=lo;hi=len;
                while lo<hi {let mid=lo+(hi-lo)/2;if sample(mid).x<point.x+epsilon {lo=mid+1;}else{hi=mid;}}
                let extreme=order.extreme(series,start,lo,point.y>=0.0);
                base=(base+extreme).clamp(-f64::MAX,f64::MAX);
            }
        }
        let center=point.x+offset;
        PlotBar {left:(center-half).max(-f64::MAX),right:(center+half).min(f64::MAX),base,top:(base+point.y).clamp(-f64::MAX,f64::MAX)}
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

    pub(super) fn plot_coordinate_text(&self, id: WidgetId, x: f32, y: f32) -> Option<String> {
        if !x.is_finite() || !y.is_finite() { return None; }
        let (tile, body) = self.plot_hit(id, x, y)?;
        let Kind::Chart { plot_images, .. } = &self.widgets.get(&id)?.kind else { return None; };
        let frame = plot_images.get(tile)?.as_ref()?;
        let data = self.chart_plot(id, tile)?;
        let area = crate::plot_raster::plot_area(frame.image.width() as f32, frame.image.height() as f32, data);
        let px = (x - body.x) * frame.image.width() as f32 / body.width;
        let py = (y - body.y) * frame.image.height() as f32 / body.height;
        if !area.contains(px, py) { return None; }
        let at = |(lo, hi): (f64, f64), t: f64| {
            let span = hi - lo;
            let value = if span.is_finite() { lo + span * t } else { lo * (1.0 - t) + hi * t };
            value.clamp(lo, hi)
        };
        let tx = f64::from((px - area.x) / area.width);
        let ty = 1.0 - f64::from((py - area.y) / area.height);
        let mut lines = Vec::new();
        if !matches!(data.x_format, PlotAxisFormat::Labels(_)) {
            let x = data.x_format.label(at(frame.view.x, tx));
            if !x.is_empty() {
                lines.push(format!("{}: {x}", if data.x_label.is_empty() { "X" } else { &data.x_label }));
            }
        }
        let y = PlotAxisFormat::Numeric.label(at(frame.view.y1, ty));
        lines.push(format!("{}: {y}", if data.y1_label.is_empty() { "Y1" } else { &data.y1_label }));
        if data.series.iter().any(|series| series.right_axis) {
            let y = PlotAxisFormat::Numeric.label(at(frame.view.y2, ty));
            lines.push(format!("{}: {y}", if data.y2_label.is_empty() { "Y2" } else { &data.y2_label }));
        }
        Some(lines.join("\n"))
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
        let Some(bars)=PlotBars::new(data) else {return true;};
        let view = data.view_with_bars(&bars);
        let mut nearest = (64.0, None);
        if inside {
            for (index,series) in data.series.iter().enumerate() {
                for point in series
                    .points
                    .iter()
                    .filter(|p| p.x.is_finite() && p.y.is_finite())
                {
                    let distance=if series.line==PlotLine::Bars {
                        let bar=bars.bar(index,point);
                        let pixel=|x,y|crate::plot_raster::sample_pixel(&PlotPoint{x,y,row:point.row},series.right_axis,view,area);
                        let a=pixel(bar.left,bar.base);let b=pixel(bar.right,bar.top);
                        let dx=f64::from(x)-f64::from(x).clamp(a.0.min(b.0),a.0.max(b.0));
                        let dy=f64::from(y)-f64::from(y).clamp(a.1.min(b.1),a.1.max(b.1));
                        dx*dx+dy*dy
                    }else{
                        let pixel=crate::plot_raster::sample_pixel(point,series.right_axis,view,area);
                        (pixel.0-f64::from(x)).powi(2)+(pixel.1-f64::from(y)).powi(2)
                    };
                    if distance < nearest.0 || (series.line==PlotLine::Bars && distance==nearest.0 && distance<64.0) {
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
    fn coordinate_position(ui: &Ui, id: WidgetId, tile: usize, tx: f32, ty: f32) -> (f32, f32) {
        let Kind::Chart { plot_bounds, plot_images, .. } = &ui.widgets[&id].kind else { panic!() };
        let body = plot_bounds[tile];
        let image = &plot_images[tile].as_ref().unwrap().image;
        let area = crate::plot_raster::plot_area(image.width() as f32, image.height() as f32, ui.chart_plot(id, tile).unwrap());
        (body.x + (area.x + area.width * tx) * body.width / image.width() as f32,
         body.y + (area.y + area.height * (1.0 - ty)) * body.height / image.height() as f32)
    }

    #[test]
    fn coordinate_hover_uses_cached_pixels_survives_navigation_and_does_not_steal_hits() {
        use crate::{ChartStyle, ChartTile, ColumnStyle, PlotStyle, Theme};
        use next_gui_scene::Scene;
        let points: Arc<[PlotPoint]> = vec![PlotPoint { x: 0.0, y: 0.0, row: 3 }].into();
        for theme in [Theme::dark(), Theme::light()] {
            let mut data = PlotData { show_legend: false, series: vec![PlotSeries {
                name: "Right".into(), color: Color::rgb(30, 100, 200), right_axis: true,
                points: points.clone(), line: PlotLine::Line, marker: PlotMarker::None,
            }], ..PlotData::default() };
            data.state.view = Some(PlotView { x: (0.0, 40.0), y1: (-100.0, 100.0), y2: (0.0, 1000.0) });
            let mut ui = Ui::new(); ui.set_theme(theme); let mut scene = Scene::new();
            ui.set_viewport(480.0, 320.0);
            let mut id = WidgetId(0);
            ui.column(ColumnStyle { width: 480.0, ..ColumnStyle::default() }, |c| {
                id = c.chart(vec![ChartTile::plot("Hover", data.clone(), PlotStyle::default())],
                    ChartStyle { width: 479.4, height: 319.7, columns: 1, show_plot_coordinates: true, ..ChartStyle::default() });
            });
            ui.sync(&mut scene);
            let tip = ui.find_tooltip_ancestor(id).unwrap();
            let caption = scene.get(ui.widgets[&id].aux[0]).unwrap().bounds;
            ui.pointer_hover(&scene, caption.x + 1.0, caption.y + 1.0); ui.sync(&mut scene);
            let label = scene.get(ui.widgets[&tip].aux[1]).unwrap();
            let expected = ui.font.rasterize_text("Hover", ui.tooltip_style().size, None).1;
            assert_eq!(label.mask.as_ref().unwrap().pixels, expected.pixels);
            let cached = match &ui.widgets[&id].kind {
                Kind::Chart { plot_images, .. } => plot_images[0].as_ref().unwrap().image.clone(), _ => panic!(),
            };
            let (x, y) = coordinate_position(&ui, id, 0, 0.25, 0.75);
            let text = ui.plot_coordinate_text(id, x, y).unwrap();
            for (line, expected) in text.lines().zip([10.0, 50.0, 750.0]) {
                assert!((line.split_once(": ").unwrap().1.parse::<f64>().unwrap() - expected).abs() < 0.001, "{text}");
            }
            assert_eq!(text.lines().count(), 3);
            for tx in [0.25, 0.5, 0.75, 0.999] {
                let (x, y) = coordinate_position(&ui, id, 0, tx, 0.01);
                assert!(ui.pointer_hover(&scene, x, y)); ui.sync(&mut scene);
                let panel = scene.get(ui.widgets[&tip].aux[0]).unwrap();
                assert!(panel.visible);
                assert!(panel.bounds.x >= 0.0 && panel.bounds.y >= 0.0 && panel.bounds.x + panel.bounds.width <= 480.0 && panel.bounds.y + panel.bounds.height <= 320.0);
                let px = panel.bounds.x + panel.bounds.width * 0.5;
                let py = panel.bounds.y + panel.bounds.height * 0.5;
                assert_eq!(ui.hit_widget(&scene, px, py), Some(id), "A passive overlay must not intercept the underlying Chart");
                assert_eq!(ui.hit_widget(&scene, x, y), Some(id));
                let Kind::Chart { plot_images, .. } = &ui.widgets[&id].kind else { panic!() };
                assert!(Arc::ptr_eq(cached.pixels(), plot_images[0].as_ref().unwrap().image.pixels()));
            }
            ui.set_focus(Some(id)); ui.key_input(UiKey::Char('+')); ui.sync(&mut scene);
            let (x, y) = coordinate_position(&ui, id, 0, 0.25, 0.75);
            assert_ne!(ui.plot_coordinate_text(id, x, y).unwrap(), text);
            ui.pointer_pressed(&mut scene, x, y); ui.pointer_moved(x + 10.0, y - 5.0); ui.sync(&mut scene);
            assert_eq!(ui.last_pointer, Some((x + 10.0, y - 5.0)));
            assert!(scene.get(ui.widgets[&tip].aux[0]).unwrap().visible);
            ui.pointer_released();
            assert!(Arc::ptr_eq(&points, &ui.chart_plot(id, 0).unwrap().series[0].points));
            let menu = ui.context_menu_popup(ui.context_menu_style());
            ui.open_context_menu(menu, vec![crate::MenuItem::new(1, "Action")], x, y); ui.sync(&mut scene);
            assert!(!scene.get(ui.widgets[&tip].aux[0]).unwrap().visible);
            ui.key_input(UiKey::Escape); assert!(ui.pointer_left()); ui.sync(&mut scene);
            assert_eq!(ui.last_pointer, None);
            assert!(!scene.get(ui.widgets[&tip].aux[0]).unwrap().visible);
            assert!(ui.plot_coordinate_text(id, f32::NAN, y).is_none());
            assert!(ui.plot_coordinate_text(id, 0.0, 0.0).is_none());
            assert!(ui.plot_coordinate_text(WidgetId(0), x, y).is_none());
            ui.set_chart_tiles(id, vec![ChartTile::gauge("Gauge", 50.0, 0.0, 100.0, "%", "", crate::GaugeStyle::default())]);
            ui.pointer_moved(x, y); ui.sync(&mut scene);
            assert!(ui.plot_coordinate_text(id, x, y).is_none());
            assert!(!scene.get(ui.widgets[&tip].aux[0]).unwrap().visible);
        }
    }

    #[test]
    fn coordinate_hover_formats_dates_categories_and_extreme_ranges_after_model_changes() {
        use crate::{ChartStyle, ChartTile, ColumnStyle, PlotStyle};
        use next_gui_scene::Scene;
        let mut ui = Ui::new(); let mut scene = Scene::new(); let mut id = WidgetId(0);
        ui.column(ColumnStyle::default(), |c| {
            id = c.chart(vec![ChartTile::plot("Left", PlotData::default(), PlotStyle::default()),
                ChartTile::plot("Right", PlotData::default(), PlotStyle::default())],
                ChartStyle { columns: 2, show_plot_coordinates: true, ..ChartStyle::default() });
        });
        for (format, range, expected) in [
            (PlotAxisFormat::Date, (86400.0, 172800.0), Some("X: 1970-01-02")),
            (PlotAxisFormat::Time, (45266.0, 45326.0), Some("X: 12:34:56")),
            (PlotAxisFormat::DateTime, (86400.0, 172800.0), Some("X: 1970-01-02 12:00:00")),
            (PlotAxisFormat::Labels(vec!["A".into(), "B".into()].into()), (0.0, 1.0), None),
            (PlotAxisFormat::Numeric, (-f64::MAX, f64::MAX), Some("X: 0")),
        ] {
            let data = ui.chart_plot_mut(id, 1).unwrap(); data.x_format = format;
            data.y1_label = "Amount".into();
            data.state.view = Some(PlotView { x: range, y1: (-f64::MAX, f64::MAX), y2: (0.0, 1.0) });
            ui.sync(&mut scene);
            let (x, y) = coordinate_position(&ui, id, 1, 0.5, 0.5);
            let text = ui.plot_coordinate_text(id, x, y).unwrap();
            if let Some(expected) = expected { assert!(text.starts_with(expected), "{text}"); }
            else { assert!(!text.contains("X:")); }
            assert!(text.ends_with("Amount: 0"), "{text}"); assert!(!text.contains("Y2:"));
            assert_eq!(ui.plot_hit(id, x, y).unwrap().0, 1);
        }
        ui.chart_plot_mut(id, 1).unwrap().y1_label = "Long column 顧客 ".repeat(40);
        if let Kind::Chart { style, .. } = &mut ui.widgets.get_mut(&id).unwrap().kind {
            style.width = 319.4; style.height = 159.7;
        }
        ui.set_viewport(320.0, 160.0); ui.sync(&mut scene);
        let (x, y) = coordinate_position(&ui, id, 1, 0.5, 0.5);
        ui.pointer_moved(x, y); ui.sync(&mut scene);
        let tip = ui.find_tooltip_ancestor(id).unwrap();
        let aux = &ui.widgets[&tip].aux;
        let panel = scene.get(aux[0]).unwrap();
        assert!(panel.visible);
        assert!(panel.bounds.x >= 0.0 && panel.bounds.y >= 0.0 && panel.bounds.x + panel.bounds.width <= 320.0 && panel.bounds.y + panel.bounds.height <= 160.0);
        let label = scene.get(aux[1]).unwrap();
        assert!(label.bounds.height > panel.bounds.height);
        let clip = label.clip.unwrap();
        assert!(clip.x >= 0.0 && clip.y >= 0.0 && clip.x + clip.width <= 320.0 && clip.y + clip.height <= 160.0);
    }

    fn bar_series(points:&[(f64,f64,usize)],right_axis:bool,color:Color) -> PlotSeries {
        PlotSeries {name:"Bars".into(),color,right_axis,points:points.iter().map(|&(x,y,row)|PlotPoint{x,y,row}).collect::<Vec<_>>().into(),line:PlotLine::Bars,marker:PlotMarker::None}
    }

    #[test]
    fn grouped_and_stacked_bars_keep_signed_duplicate_keys_axes_and_source_order() {
        let mut data=PlotData {show_legend:false,series:vec![
            bar_series(&[(2.0,4.0,0),(0.0,2.0,1),(0.0,3.0,2),(0.0,-5.0,3),(f64::NAN,1.0,4),(0.0,f64::NAN,5)],false,Color::rgb(200,40,20)),
            bar_series(&[(0.0,7.0,6),(0.0,-2.0,7),(2.0,6.0,8)],false,Color::rgb(20,140,40)),
            bar_series(&[(0.0,50.0,9)],true,Color::rgb(40,20,200)),
            bar_series(&[(0.0,1.0,10),(0.0,-1.0,11),(2.0+1e-15,1.0,12)],false,Color::rgb(140,20,140)),
        ],..PlotData::default()};
        let source=data.series[0].points.clone();
        let bars=PlotBars::new(&data).unwrap();
        assert!(bars.orders.iter().all(|order|order.indices.is_none()),"Grouping needs no sorted index");
        let a=bars.bar(0,&data.series[0].points[1]);let b=bars.bar(1,&data.series[1].points[0]);
        assert!(a.left<a.right && a.right<b.left && b.left<b.right);
        let grouped=data.auto_view();assert!(grouped.x.0<a.left && grouped.y1.1>7.0);
        data.stacked_bars=true;
        let bars=PlotBars::new(&data).unwrap();assert!(bars.orders[0].indices.is_some());
        let positive=bars.bar(1,&data.series[1].points[0]);assert_eq!((positive.base,positive.top),(3.0,10.0));
        let negative=bars.bar(1,&data.series[1].points[1]);assert_eq!((negative.base,negative.top),(-5.0,-7.0));
        let right=bars.bar(2,&data.series[2].points[0]);assert_eq!((right.base,right.top),(0.0,50.0));
        let last=bars.bar(3,&data.series[3].points[0]);assert_eq!((last.base,last.top),(10.0,11.0));
        let nearby=bars.bar(3,&data.series[3].points[1]);assert_eq!((nearby.base,nearby.top),(-7.0,-8.0));
        assert_eq!(bars.bar(3,&data.series[3].points[2]).top,11.0);
        assert_eq!(positive.left,-0.425);assert_eq!(positive.right,0.425);
        let stacked=data.auto_view();assert!(stacked.y1.0< -8.0 && stacked.y1.1>11.0 && stacked.y2.1>50.0);
        assert!(Arc::ptr_eq(&source,&data.series[0].points));assert_eq!(source[0].x,2.0);
        data.series=vec![bar_series(&[(1.0,f64::MAX,0)],false,Color::rgb(1,2,3));8];
        assert!(data.auto_view().is_valid());let bars=PlotBars::new(&data).unwrap();assert_eq!(bars.bar(7,&data.series[7].points[0]).top,f64::MAX);
        data.stacked_bars=false;let bars=PlotBars::new(&data).unwrap();
        for i in 0..8 {let b=bars.bar(i,&data.series[i].points[0]);assert!(b.left<b.right);}
    }

    #[test]
    fn bar_interiors_select_the_rendered_source_row_in_grouped_and_stacked_modes() {
        use crate::{ChartStyle,ChartTile,ColumnStyle,PlotStyle};
        use next_gui_scene::Scene;
        for stacked_bars in [false,true] {
            let data=PlotData {show_legend:false,stacked_bars,series:vec![
                bar_series(&[(0.0,2.0,10)],false,Color::rgb(200,40,20)),
                bar_series(&[(0.0,4.0,20)],false,Color::rgb(20,140,40)),
            ],..PlotData::default()};
            let mut ui=Ui::new();let mut scene=Scene::new();let mut id=WidgetId(0);
            ui.column(ColumnStyle {width:600.0,..ColumnStyle::default()},|c|{id=c.chart(vec![ChartTile::plot("Bars",data.clone(),PlotStyle::default())],ChartStyle {width:580.0,height:360.0,columns:1,..ChartStyle::default()});});
            ui.sync(&mut scene);
            let Kind::Chart{plot_bounds,..}=&ui.widgets[&id].kind else {unreachable!()};let body=plot_bounds[0];
            let mut area=crate::plot_raster::plot_area(body.width,body.height,&data);area.x+=body.x;area.y+=body.y;
            let bars=PlotBars::new(&data).unwrap();let bar=bars.bar(1,&data.series[1].points[0]);
            let probe=PlotPoint{x:bar.left/2.0+bar.right/2.0,y:bar.base/2.0+bar.top/2.0,row:20};
            let (x,y)=crate::plot_raster::sample_pixel(&probe,false,data.view(),area);
            assert!(ui.pointer_pressed(&mut scene,x as f32,y as f32));ui.pointer_released();
            assert_eq!(ui.chart_plot(id,0).unwrap().state.selected_rows.as_ref(),&BTreeSet::from([20]));
            let (width,height)=ui.chart_plot_size(id,0).unwrap();let image=ui.chart_plot_image(id,0,width,height).unwrap();
            let area=crate::plot_raster::plot_area(width as f32,height as f32,&data);
            let (x,y)=crate::plot_raster::sample_pixel(&probe,false,data.view(),area);let pixel=((y as usize*width as usize)+x as usize)*4;
            let rgb=&image.pixels()[pixel..pixel+3];assert!(rgb[1]>rgb[0] && rgb[1]>rgb[2],"The selected bar interior must retain its green series color");
            let endpoint=PlotPoint{y:bar.top,..probe};let (x,y)=crate::plot_raster::sample_pixel(&endpoint,false,data.view(),area);
            let pixel=((y as usize*width as usize)+x as usize)*4;let selected=PlotStyle::default().selection;
            assert_eq!(&image.pixels()[pixel..pixel+3],&[selected.r,selected.g,selected.b]);
        }
    }

    #[test]
    fn dense_duplicate_bar_keys_use_range_extrema_without_changing_stack_heights() {
        let points=(0..100_000).map(|row|PlotPoint{x:0.0,y:if row%113==0 {f64::NAN}else{(row%31) as f64-15.0},row}).collect::<Vec<_>>();
        let data=PlotData {stacked_bars:true,series:vec![
            PlotSeries {points:points.into(),..bar_series(&[],false,Color::rgb(1,2,3))},
            bar_series(&[(0.0,2.0,100001),(0.0,-2.0,100002)],false,Color::rgb(3,2,1)),
        ],..PlotData::default()};
        let bars=PlotBars::new(&data).unwrap();let order=&bars.orders[0];
        assert!(order.indices.is_none() && !order.extrema.is_empty());
        assert!(order.extrema.len()*std::mem::size_of::<(f64,f64)>()<data.series[0].points.len()*2);
        for (lo,hi) in [(0,100000),(1,99999),(32,64),(33,63),(1000,1001),(99999,100000),(5,5)] {
            for positive in [false,true] {
                let expected=data.series[0].points[lo..hi].iter().map(|p|p.y).filter(|y|y.is_finite()).fold(0.0f64,|a,b|if positive {a.max(b)}else{a.min(b)});
                assert_eq!(order.extreme(&data.series[0],lo,hi,positive),expected);
            }
        }
        assert_eq!(bars.bar(1,&data.series[1].points[0]).top,17.0);
        assert_eq!(bars.bar(1,&data.series[1].points[1]).top,-17.0);
    }

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
            cached.image.pixels(),
            plot_images[0].as_ref().unwrap().image.pixels()
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
            cached.image.pixels(),
            plot_images[0].as_ref().unwrap().image.pixels()
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
