//! Shared Cartesian renderer for the retained chart and exported images.

use crate::{chart, PlotAxis, PlotAxisFormat, PlotData, PlotLine, PlotMarker, PlotView};
use next_gui_scene::{Color, Rect, RgbaImage};
use next_gui_text::Font;

/// Colors and typography for a Cartesian plot.
#[derive(Debug, Clone, Copy)]
pub struct PlotStyle {
    /// Canvas background.
    pub bg: Color,
    /// Axis and legend text.
    pub fg: Color,
    /// Grid lines and inactive axes.
    pub grid: Color,
    /// Selected samples and active axis.
    pub selection: Color,
    /// Tick and legend font size in pixels.
    pub font_size: f32,
}

impl Default for PlotStyle {
    fn default() -> Self {
        Self {
            bg: Color::rgb(255, 255, 255),
            fg: Color::rgb(40, 48, 61),
            grid: Color::rgb(219, 224, 232),
            selection: Color::rgb(20, 105, 230),
            font_size: 11.0,
        }
    }
}

pub(crate) fn plot_area(width: f32, height: f32, data: &PlotData) -> Rect {
    let left = 76.0f32.min(width * 0.25);
    let right: f32 = if data.series.iter().any(|s| s.right_axis) {
        76.0
    } else {
        16.0
    };
    let right = right.min(width * 0.25);
    let bottom = if matches!(data.x_format, PlotAxisFormat::DateTime) {
        68.0
    } else {
        50.0
    };
    let legend_columns = ((width - 24.0) / 150.0).floor().max(1.0) as usize;
    let legend = if data.show_legend { (data.series.len().div_ceil(legend_columns) as f32 * 20.0).min(height * 0.25) } else { 0.0 };
    let top = 24.0 + legend;
    Rect::new(
        left,
        top.min(height * 0.5),
        (width - left - right).max(1.0),
        (height - top - bottom).max(1.0),
    )
}

/// Render the same axis, series, legend and selection pixels used by a Chart tile.
/// Invalid dimensions, allocation failures, buffers over 128 MiB and invalid font
/// sizes return `None`, matching the database example's image memory budget.
/// Unsorted or dense stacked bars may also allocate shared-source lookup indexes.
pub fn render_plot(
    font: &Font,
    data: &PlotData,
    style: PlotStyle,
    width: u32,
    height: u32,
) -> Option<RgbaImage> {
    render_plot_frame(font, data, style, width, height).map(|frame| frame.image)
}

#[derive(Debug, Clone)]
pub(crate) struct PlotFrame {
    pub image: RgbaImage,
    pub view: PlotView,
}

pub(crate) fn render_plot_frame(
    font: &Font,
    data: &PlotData,
    style: PlotStyle,
    width: u32,
    height: u32,
) -> Option<PlotFrame> {
    if width == 0
        || height == 0
        || width > i32::MAX as u32
        || height > i32::MAX as u32
        || !style.font_size.is_finite()
        || style.font_size <= 0.0
    {
        return None;
    }
    let len = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    if len > 128 * 1024 * 1024 {
        return None;
    }
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(len).ok()?;
    pixels.resize(len, 0);
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[style.bg.r, style.bg.g, style.bg.b, style.bg.a]);
    }
    let mut canvas = Canvas {
        width: width as i32,
        height: height as i32,
        pixels,
        clip: Rect::new(0.0, 0.0, width as f32, height as f32),
    };
    let area = plot_area(width as f32, height as f32, data);
    let bars=crate::plot::PlotBars::new(data)?;
    let view = data.view_with_bars(&bars);
    let xticks = ticks(
        view.x,
        &data.x_format,
        (area.width / 105.0).max(2.0) as usize,
        data.fixed_number_format,
    );
    let yticks = ticks(
        view.y1,
        &PlotAxisFormat::Numeric,
        (area.height / 52.0).max(3.0) as usize,
        data.fixed_number_format,
    );
    for &(value, ref label) in &xticks {
        let x = f64::from(area.x) + unit(value, view.x) * f64::from(area.width);
        canvas.line(
            (x, f64::from(area.y)),
            (x, f64::from(area.y + area.height)),
            style.grid,
        );
        canvas.text(
            font,
            label,
            style.font_size,
            Rect::new(x as f32 - 51.0, area.y + area.height + 7.0, 102.0, 30.0),
            style.fg,
            true,
        );
    }
    for &(value, ref label) in &yticks {
        let y = f64::from(area.y + area.height) - unit(value, view.y1) * f64::from(area.height);
        canvas.line(
            (f64::from(area.x), y),
            (f64::from(area.x + area.width), y),
            style.grid,
        );
        canvas.text(
            font,
            label,
            style.font_size,
            Rect::new(2.0, y as f32 - 8.0, area.x - 10.0, 18.0),
            style.fg,
            false,
        );
    }
    let has_right = data.series.iter().any(|s| s.right_axis);
    if has_right {
        for (value, label) in ticks(
            view.y2,
            &PlotAxisFormat::Numeric,
            (area.height / 52.0).max(3.0) as usize,
            data.fixed_number_format,
        ) {
            let y = f64::from(area.y + area.height) - unit(value, view.y2) * f64::from(area.height);
            canvas.text(
                font,
                &label,
                style.font_size,
                Rect::new(
                    area.x + area.width + 7.0,
                    y as f32 - 8.0,
                    width as f32 - area.x - area.width - 9.0,
                    18.0,
                ),
                style.fg,
                false,
            );
        }
    }
    canvas.text(
        font,
        &data.x_label,
        style.font_size + 1.0,
        Rect::new(area.x, height as f32 - 19.0, area.width, 18.0),
        style.fg,
        true,
    );
    canvas.text(
        font,
        &data.y1_label,
        style.font_size,
        Rect::new(2.0, area.y - 21.0, (area.width * 0.5).max(area.x), 18.0),
        style.fg,
        false,
    );
    if has_right {
        canvas.text(
            font,
            &data.y2_label,
            style.font_size,
            Rect::new(
                area.x + area.width * 0.5,
                area.y - 21.0,
                width as f32 - area.x - area.width * 0.5 - 2.0,
                18.0,
            ),
            style.fg,
            false,
        );
    }
    let columns = ((width as f32 - 24.0) / 150.0).floor().max(1.0) as usize;
    let cell_width = (width as f32 - 16.0) / columns as f32;
    for (i, series) in data.series.iter().take(if data.show_legend { data.series.len() } else { 0 }).enumerate() {
        let x = 8.0 + (i % columns) as f32 * cell_width;
        let y = 5.0 + (i / columns) as f32 * 20.0;
        if y + 18.0 > area.y - 20.0 {
            break;
        }
        canvas.line(
            (f64::from(x), f64::from(y + 7.0)),
            (f64::from(x + 17.0), f64::from(y + 7.0)),
            series.color,
        );
        canvas.text(
            font,
            &series.name,
            style.font_size,
            Rect::new(x + 23.0, y, cell_width - 26.0, 18.0),
            style.fg,
            false,
        );
    }
    canvas.clip = area;
    for (series_index,series) in data.series.iter().enumerate() {
        let yrange = if series.right_axis { view.y2 } else { view.y1 };
        let map = |x, y| {
            (
                f64::from(area.x) + unit(x, view.x) * f64::from(area.width),
                f64::from(area.y + area.height) - unit(y, yrange) * f64::from(area.height),
            )
        };
        let mut previous = None;
        let sampled = (series.line == PlotLine::Line
            && series.points.len() > area.width as usize * 4)
            .then(|| line_samples(&series.points, view.x, area.width));
        for index in 0..sampled.as_ref().map_or(series.points.len(), Vec::len) {
            let point = &series.points[sampled.as_ref().map_or(index, |indices| indices[index])];
            if !point.x.is_finite() || !point.y.is_finite() {
                previous = None;
                continue;
            }
            let current = map(point.x, point.y);
            match series.line {
                PlotLine::Impulse => canvas.line(map(point.x, 0.0), current, series.color),
                PlotLine::Bars => {
                    let bar=bars.bar(series_index,point);
                    let left = map(bar.left, bar.top);
                    let right = map(bar.right, bar.base);
                    canvas.fill_rect(left, right, series.color);
                    let current=map(bar.left/2.0+bar.right/2.0,bar.top);
                    canvas.marker(current,series.marker,series.color,3.5);
                    if data.state.selected_rows.contains(&point.row) {
                        canvas.marker(current,PlotMarker::Circle,style.selection,6.0);
                        canvas.marker(current,PlotMarker::Disc,style.selection,2.0);
                    }
                }
                PlotLine::None => {}
                line => {
                    if let Some((px, py)) = previous {
                        let (x, y) = current;
                        match line {
                            PlotLine::StepLeft => {
                                canvas.line((px, py), (x, py), series.color);
                                canvas.line((x, py), current, series.color);
                            }
                            PlotLine::StepRight => {
                                canvas.line((px, py), (px, y), series.color);
                                canvas.line((px, y), current, series.color);
                            }
                            PlotLine::StepCenter => {
                                let middle = px / 2.0 + x / 2.0;
                                canvas.line((px, py), (middle, py), series.color);
                                canvas.line((middle, py), (middle, y), series.color);
                                canvas.line((middle, y), current, series.color);
                            }
                            _ => canvas.line((px, py), current, series.color),
                        }
                    }
                }
            }
            previous = Some(current);
        }
        if series.line!=PlotLine::Bars && (series.marker != PlotMarker::None || !data.state.selected_rows.is_empty()) {
            for point in series
                .points
                .iter()
                .filter(|p| p.x.is_finite() && p.y.is_finite())
            {
                let current = map(point.x, point.y);
                canvas.marker(current, series.marker, series.color, 3.5);
                if data.state.selected_rows.contains(&point.row) {
                    canvas.marker(current, PlotMarker::Circle, style.selection, 6.0);
                    canvas.marker(current, PlotMarker::Disc, style.selection, 2.0);
                }
            }
        }
    }
    canvas.clip = Rect::new(0.0, 0.0, width as f32, height as f32);
    let axis_color = |axis| {
        if data.state.active_axis == axis {
            style.selection
        } else {
            style.fg
        }
    };
    canvas.line(
        (f64::from(area.x), f64::from(area.y + area.height)),
        (
            f64::from(area.x + area.width),
            f64::from(area.y + area.height),
        ),
        axis_color(PlotAxis::X),
    );
    canvas.line(
        (f64::from(area.x), f64::from(area.y)),
        (f64::from(area.x), f64::from(area.y + area.height)),
        axis_color(PlotAxis::Y1),
    );
    if has_right {
        canvas.line(
            (f64::from(area.x + area.width), f64::from(area.y)),
            (
                f64::from(area.x + area.width),
                f64::from(area.y + area.height),
            ),
            axis_color(PlotAxis::Y2),
        );
    }
    RgbaImage::new(width, height, canvas.pixels).map(|image| PlotFrame { image, view })
}

pub(crate) fn unit(value: f64, (lo, hi): (f64, f64)) -> f64 {
    let span = hi - lo;
    let value = if span.is_finite() {
        (value - lo) / span
    } else {
        (value / 2.0 - lo / 2.0) / (hi / 2.0 - lo / 2.0)
    };
    value.clamp(-1e100, 1e100)
}

pub(crate) fn sample_pixel(
    point: &crate::PlotPoint,
    right_axis: bool,
    view: PlotView,
    area: Rect,
) -> (f64, f64) {
    (
        f64::from(area.x) + unit(point.x, view.x) * f64::from(area.width),
        f64::from(area.y + area.height)
            - unit(point.y, if right_axis { view.y2 } else { view.y1 }) * f64::from(area.height),
    )
}

// Keep each screen column's endpoints and extrema, in source order, including gaps.
fn line_samples(points: &[crate::PlotPoint], range: (f64, f64), width: f32) -> Vec<usize> {
    let mut output = Vec::new();
    let mut group: Option<(i32, [usize; 4])> = None;
    let flush = |indices: [usize; 4], output: &mut Vec<usize>| {
        let mut indices = indices;
        indices.sort_unstable();
        for index in indices {
            if output.last() != Some(&index) {
                output.push(index);
            }
        }
    };
    for (index, point) in points.iter().enumerate() {
        if !point.x.is_finite() || !point.y.is_finite() {
            if let Some((_, indices)) = group.take() {
                flush(indices, &mut output);
            }
            if output
                .last()
                .is_none_or(|&i| points[i].x.is_finite() && points[i].y.is_finite())
            {
                output.push(index);
            }
            continue;
        }
        let column = (unit(point.x, range) * f64::from(width))
            .floor()
            .clamp(-1.0, f64::from(width)) as i32;
        if group.as_ref().is_some_and(|(old, _)| *old != column) {
            let (_, indices) = group.take().unwrap();
            flush(indices, &mut output);
        }
        let (_, indices) = group.get_or_insert((column, [index; 4]));
        if point.y < points[indices[1]].y {
            indices[1] = index;
        }
        if point.y > points[indices[2]].y {
            indices[2] = index;
        }
        indices[3] = index;
    }
    if let Some((_, indices)) = group {
        flush(indices, &mut output);
    }
    output
}

fn ticks((lo, hi): (f64, f64), format: &PlotAxisFormat, count: usize, fixed: bool) -> Vec<(f64, String)> {
    let count = count.clamp(2, 12);
    if let PlotAxisFormat::Labels(labels)=format {
        if labels.is_empty() || hi<0.0 {return Vec::new();}
        let first=lo.ceil().max(0.0) as usize;
        let last=(hi.floor() as usize).min(labels.len()-1);
        if first>last {return Vec::new();}
        let stride=(last-first+1).div_ceil(count);
        return (first..=last).step_by(stride).map(|i|(i as f64,labels[i].clone())).collect();
    }
    let raw = (hi / 2.0 - lo / 2.0) / count as f64 * 2.0;
    let power = 10.0f64.powf(raw.log10().floor());
    let mut step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .into_iter()
        .map(|n| n * power)
        .find(|&n| n >= raw)
        .unwrap_or(raw);
    if matches!(
        format,
        PlotAxisFormat::Date | PlotAxisFormat::Time | PlotAxisFormat::DateTime
    ) {
        step = [
            1.0, 5.0, 15.0, 30.0, 60.0, 300.0, 900.0, 1800.0, 3600.0, 10800.0, 21600.0, 43200.0,
            86400.0, 172800.0, 259200.0, 432000.0, 604800.0, 1209600.0, 2592000.0, 31536000.0,
        ]
        .into_iter()
        .find(|&n| n >= raw)
        .unwrap_or(step);
        if matches!(format, PlotAxisFormat::Date) {
            step = step.max(86400.0);
        }
    }
    let start = (lo / step).ceil() * step;
    let mut output = Vec::new();
    if step.is_finite() && step > 0.0 && start.is_finite() {
        for i in 0..=count * 3 {
            let value = start + i as f64 * step;
            if value > hi || !value.is_finite() {
                break;
            }
            if value < lo {
                continue;
            }
            let label = if matches!(format, PlotAxisFormat::Numeric) {
                if fixed { fixed_number(value, (lo,hi)) } else { tick_number(value, step) }
            } else {
                format.label(value)
            };
            if !label.is_empty() && output.last().is_none_or(|(_, old)| old != &label) {
                output.push((value, label));
            }
        }
    }
    if output.is_empty() && matches!(format, PlotAxisFormat::Numeric) {
        for i in 0..=count {
            let t = i as f64 / count as f64;
            let value = lo * (1.0 - t) + hi * t;
            let label = if fixed { fixed_number(value,(lo,hi)) } else { format!("{value:.15e}") };
            if output.last().is_none_or(|(_, old)| old != &label) {
                output.push((value, label));
            }
        }
    }
    output
}

fn fixed_number(value:f64,(lo,hi):(f64,f64)) -> String {
    let integer=lo/2.0+hi/2.0>999.0 && hi-lo>10.0;
    let precision=if integer {0} else {2};
    let value=if value.abs()<if integer {0.5} else {0.005} {0.0} else {value};
    format!("{value:.precision$}")
}

fn tick_number(value: f64, step: f64) -> String {
    let value = if value.abs() < step * 1e-8 {
        0.0
    } else {
        value
    };
    let exponent = step.log10().floor();
    let mantissa = step / 10.0f64.powf(exponent);
    let extra = usize::from((mantissa - mantissa.round()).abs() > 1e-8);
    let decimals = if exponent < 0.0 {
        (-exponent) as usize + extra
    } else {
        usize::from(step.fract() != 0.0)
    };
    if value != 0.0 && (value.abs() >= 1e7 || value.abs() < 1e-4) {
        let precision = (value.abs().log10().floor() - exponent).max(0.0) as usize + extra;
        format!("{value:.precision$e}", precision = precision.min(16))
    } else {
        format!("{value:.decimals$}", decimals = decimals.min(16))
    }
}

struct Canvas {
    width: i32,
    height: i32,
    pixels: Vec<u8>,
    clip: Rect,
}
impl Canvas {
    fn pixel(&mut self, x: i32, y: i32, color: Color, coverage: f64) {
        if x < 0
            || y < 0
            || x >= self.width
            || y >= self.height
            || !self.clip.contains(x as f32 + 0.5, y as f32 + 0.5)
        {
            return;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let alpha = (f64::from(color.a) / 255.0 * coverage).clamp(0.0, 1.0);
        let old_alpha = f64::from(self.pixels[i + 3]) / 255.0;
        let result_alpha = alpha + old_alpha * (1.0 - alpha);
        if result_alpha == 0.0 {
            return;
        }
        for (channel, source) in [color.r, color.g, color.b].into_iter().enumerate() {
            self.pixels[i + channel] = ((f64::from(source) * alpha
                + f64::from(self.pixels[i + channel]) * old_alpha * (1.0 - alpha))
                / result_alpha)
                .round() as u8;
        }
        self.pixels[i + 3] = (result_alpha * 255.0).round() as u8;
    }
    fn line(&mut self, from: (f64, f64), to: (f64, f64), color: Color) {
        let Some((from, to)) = clip_line(from, to, self.clip) else {
            return;
        };
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let length = dx.abs().max(dy.abs()).ceil().max(1.0) as usize;
        for i in 0..=length {
            let t = i as f64 / length as f64;
            let (x, y) = (from.0 + dx * t, from.1 + dy * t);
            let (ix, iy) = (x.floor() as i32, y.floor() as i32);
            let (fx, fy) = (x.fract(), y.fract());
            for (ox, oy, coverage) in [
                (0, 0, (1.0 - fx) * (1.0 - fy)),
                (1, 0, fx * (1.0 - fy)),
                (0, 1, (1.0 - fx) * fy),
                (1, 1, fx * fy),
            ] {
                self.pixel(ix + ox, iy + oy, color, coverage);
            }
        }
    }
    fn fill_rect(&mut self, a: (f64, f64), b: (f64, f64), color: Color) {
        if ![a.0, a.1, b.0, b.1].iter().all(|v| v.is_finite()) {
            return;
        }
        let left = a.0.min(b.0).max(f64::from(self.clip.x)).floor().max(0.0) as i32;
        let right =
            a.0.max(b.0)
                .min(f64::from(self.clip.x + self.clip.width))
                .ceil()
                .min(f64::from(self.width)) as i32;
        let top = a.1.min(b.1).max(f64::from(self.clip.y)).floor().max(0.0) as i32;
        let bottom =
            a.1.max(b.1)
                .min(f64::from(self.clip.y + self.clip.height))
                .ceil()
                .min(f64::from(self.height)) as i32;
        for y in top..bottom {
            for x in left..right {
                self.pixel(x, y, color, 0.7);
            }
        }
    }
    fn text(
        &mut self,
        font: &Font,
        text: &str,
        size: f32,
        bounds: Rect,
        color: Color,
        center: bool,
    ) {
        let layout = chart::title_layout(
            font,
            text,
            size,
            bounds.width.max(0.0),
            bounds.height.max(0.0),
        );
        let mask = font.rasterize_layout(&layout, size);
        let x = bounds.x
            + if center {
                (bounds.width - mask.width as f32).max(0.0) / 2.0
            } else {
                0.0
            };
        for y in 0..mask.height {
            for column in 0..mask.width {
                let coverage = mask.pixels[(y * mask.width + column) as usize];
                if coverage > 0 {
                    self.pixel(
                        x.round() as i32 + column as i32,
                        bounds.y.round() as i32 + y as i32,
                        color,
                        f64::from(coverage) / 255.0,
                    );
                }
            }
        }
    }
    fn marker(&mut self, (x, y): (f64, f64), marker: PlotMarker, color: Color, r: f64) {
        if marker == PlotMarker::None
            || !x.is_finite()
            || !y.is_finite()
            || x < f64::from(self.clip.x) - r
            || x > f64::from(self.clip.x + self.clip.width) + r
            || y < f64::from(self.clip.y) - r
            || y > f64::from(self.clip.y + self.clip.height) + r
        {
            return;
        }
        use PlotMarker::*;
        if matches!(marker, Circle | Disc | CrossCircle | PlusCircle | Peace) {
            if marker == Disc {
                for py in (y - r).floor() as i32..=(y + r).ceil() as i32 {
                    for px in (x - r).floor() as i32..=(x + r).ceil() as i32 {
                        let distance = ((f64::from(px) + 0.5 - x).powi(2)
                            + (f64::from(py) + 0.5 - y).powi(2))
                        .sqrt();
                        self.pixel(px, py, color, (r + 0.5 - distance).clamp(0.0, 1.0));
                    }
                }
            } else {
                for i in 0..24 {
                    let a = i as f64 * std::f64::consts::TAU / 24.0;
                    let b = (i + 1) as f64 * std::f64::consts::TAU / 24.0;
                    self.line(
                        (x + r * a.cos(), y + r * a.sin()),
                        (x + r * b.cos(), y + r * b.sin()),
                        color,
                    );
                }
            }
        }
        if matches!(marker, Square | CrossSquare | PlusSquare) {
            for (a, b) in [
                ((-r, -r), (r, -r)),
                ((r, -r), (r, r)),
                ((r, r), (-r, r)),
                ((-r, r), (-r, -r)),
            ] {
                self.line((x + a.0, y + a.1), (x + b.0, y + b.1), color);
            }
        }
        if matches!(marker, Cross | Star | CrossCircle | CrossSquare) {
            self.line((x - r, y - r), (x + r, y + r), color);
            self.line((x - r, y + r), (x + r, y - r), color);
        }
        if matches!(marker, Plus | Star | PlusCircle | PlusSquare) {
            self.line((x - r, y), (x + r, y), color);
            self.line((x, y - r), (x, y + r), color);
        }
        let (vertices, count) = match marker {
            Diamond => ([(0.0, -r), (r, 0.0), (0.0, r), (-r, 0.0)], 4),
            Triangle => ([(0.0, -r), (r, r), (-r, r), (0.0, 0.0)], 3),
            TriangleInverted => ([(0.0, r), (r, -r), (-r, -r), (0.0, 0.0)], 3),
            _ => ([(0.0, 0.0); 4], 0),
        };
        for i in 0..count {
            let a = vertices[i];
            let b = vertices[(i + 1) % count];
            self.line((x + a.0, y + a.1), (x + b.0, y + b.1), color);
        }
        if marker == Peace {
            self.line((x, y - r), (x, y + r), color);
            self.line((x, y), (x - r * 0.7, y + r * 0.7), color);
            self.line((x, y), (x + r * 0.7, y + r * 0.7), color);
        }
    }
}

fn clip_line(a: (f64, f64), b: (f64, f64), clip: Rect) -> Option<((f64, f64), (f64, f64))> {
    if ![a.0, a.1, b.0, b.1].iter().all(|v| v.is_finite()) {
        return None;
    }
    let (left, right, top, bottom) = (
        f64::from(clip.x),
        f64::from(clip.x + clip.width),
        f64::from(clip.y),
        f64::from(clip.y + clip.height),
    );
    let code = |(x, y): (f64, f64)| {
        u8::from(x < left)
            | (u8::from(x > right) << 1)
            | (u8::from(y < top) << 2)
            | (u8::from(y > bottom) << 3)
    };
    let (mut a, mut b) = (a, b);
    for _ in 0..8 {
        let (ca, cb) = (code(a), code(b));
        if ca | cb == 0 {
            return Some((a, b));
        }
        if ca & cb != 0 {
            return None;
        }
        let outside = if ca != 0 { ca } else { cb };
        let point = if outside & 12 != 0 {
            let y = if outside & 4 != 0 { top } else { bottom };
            (a.0 + (b.0 - a.0) * ((y - a.1) / (b.1 - a.1)), y)
        } else {
            let x = if outside & 1 != 0 { left } else { right };
            (x, a.1 + (b.1 - a.1) * ((x - a.0) / (b.0 - a.0)))
        };
        if ca != 0 {
            a = point;
        } else {
            b = point;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_ticks_follow_visible_range_precision_without_changing_dates_or_categories() {
        assert_eq!(fixed_number(1234.56,(1000.0,2000.0)),"1235");
        assert_eq!(fixed_number(1234.56,(1234.0,1235.0)),"1234.56");
        assert_eq!(fixed_number(-1234.56,(-2000.0,-1000.0)),"-1234.56");
        assert_eq!(fixed_number(-0.001,(-1.0,1.0)),"0.00");
        assert_eq!(fixed_number(1.0,(0.0,2.0)),"1.00");
        let large=fixed_number(1e100,(1e99,2e100));assert!(!large.contains('e') && large.len()>90);
        for range in [(1.0,1.000000000001),(-f64::MAX,f64::MAX),(-1e-310,1e-310)] {
            let labels=ticks(range,&PlotAxisFormat::Numeric,6,true);
            assert!(!labels.is_empty() && labels.iter().all(|(v,_)|v.is_finite()));
            assert!(labels.windows(2).all(|pair|pair[0].1!=pair[1].1));
        }
        for format in [PlotAxisFormat::Date,PlotAxisFormat::Time,PlotAxisFormat::Labels(vec!["a".into(),"b".into()].into())] {
            assert_eq!(ticks((0.0,86400.0),&format,6,true),ticks((0.0,86400.0),&format,6,false));
        }
        let labels=PlotAxisFormat::Labels(vec!["Q1".into(),"Q2".into(),"Q3".into(),"Q4".into()].into());
        assert_eq!(ticks((-0.7,3.7),&labels,4,false).iter().map(|(x,_)|*x).collect::<Vec<_>>(),[0.0,1.0,2.0,3.0]);
        assert_eq!(ticks((-0.7,3.7),&labels,2,false).iter().map(|(x,_)|*x).collect::<Vec<_>>(),[0.0,2.0]);
        for range in [(-5.0,-1.0),(5.0,10.0),(1.1,1.9)] {assert!(ticks(range,&labels,4,false).is_empty());}
    }

    #[test]
    fn fixed_format_changes_all_three_axis_labels_and_preserves_the_plot_pixels() {
        let font=next_gui_text::load_default_cjk_font().unwrap();
        let mut data=PlotData {show_legend:false,series:[false,true].into_iter().map(|right_axis|crate::PlotSeries {
            name:"Y".into(),color:Color::rgb(200,40,30),right_axis,
            points:vec![crate::PlotPoint{x:0.0,y:0.1,row:0},crate::PlotPoint{x:1.0,y:0.9,row:1}].into(),line:PlotLine::Line,marker:PlotMarker::None,
        }).collect(),..PlotData::default()};
        data.state.view=Some(PlotView{x:(0.0,2.0),y1:(0.0,2.0),y2:(0.0,2.0)});
        let normal=render_plot(&font,&data,PlotStyle::default(),600,320).unwrap();
        data.fixed_number_format=true;
        let fixed=render_plot(&font,&data,PlotStyle::default(),600,320).unwrap();
        let area=plot_area(600.0,320.0,&data);
        let mut changes=[0;3];
        for y in 0..320 {for x in 0..600 {
            let i=(y*600+x)*4;
            if normal.pixels()[i..i+4]!=fixed.pixels()[i..i+4] {
                assert!(!area.contains(x as f32,y as f32),"Formatting must not change curves or grid");
                if x<area.x as usize {changes[0]+=1;}else if x>=(area.x+area.width) as usize {changes[1]+=1;}else if y>=(area.y+area.height) as usize {changes[2]+=1;}
            }
        }}
        assert!(changes.iter().all(|count|*count>10),"All numeric axes must use fixed labels: {changes:?}");
    }

    #[test]
    fn ticks_remain_distinct_after_deep_zoom_and_extreme_ranges() {
        assert_eq!(tick_number(2.5, 2.5), "2.5");
        assert_eq!(tick_number(12.5, 2.5), "12.5");
        assert_eq!(tick_number(0.025, 0.025), "0.025");
        assert_eq!(tick_number(2.5e10, 2.5e10), "2.5e10");
        for range in [
            (1.0, 1.000000000001),
            (-f64::MAX, f64::MAX),
            (-1e-310, 1e-310),
            (100.0, 120.0),
        ] {
            let labels = ticks(range, &PlotAxisFormat::Numeric, 6, false);
            assert!(!labels.is_empty(), "{range:?}");
            assert!(labels
                .iter()
                .all(|(v, _)| v.is_finite() && *v >= range.0 && *v <= range.1));
            assert!(labels.windows(2).all(|pair| pair[0].1 != pair[1].1));
        }
        assert_eq!(unit(0.0, (-f64::MAX, f64::MAX)), 0.5);
        let area = Rect::new(10.0, 10.0, 100.0, 80.0);
        let clipped = clip_line((-1e100, 40.0), (1e100, 40.0), area).unwrap();
        assert_eq!(clipped, ((10.0, 40.0), (110.0, 40.0)));
        assert!(clip_line((0.0, 0.0), (5.0, 5.0), area).is_none());
    }

    #[test]
    fn dense_line_sampling_preserves_single_sample_spikes_gaps_and_source_order() {
        let mut points: Vec<_> = (0..10_000)
            .map(|row| crate::PlotPoint {
                x: row as f64,
                y: 0.0,
                row,
            })
            .collect();
        points[333].y = 1e100;
        points[337].y = -1e100;
        points[4000].y = f64::NAN;
        let indices = line_samples(&points, (0.0, 9999.0), 100.0);
        assert!(indices.len() < 420);
        for retained in [0, 333, 337, 3999, 4000, 4001, 9999] {
            assert!(
                indices.contains(&retained),
                "missing spike/gap endpoint {retained}"
            );
        }
        assert!(indices.windows(2).all(|pair| pair[0] < pair[1]));
        let reverse: Vec<_> = points.into_iter().rev().collect();
        let indices = line_samples(&reverse, (0.0, 9999.0), 100.0);
        assert!(indices.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!((indices[0], indices[indices.len() - 1]), (0, 9999));
    }

    #[test]
    fn rendered_step_heights_follow_the_left_and_right_samples() {
        let font = next_gui_text::load_default_cjk_font().unwrap();
        let mut data = PlotData {
            series: vec![crate::PlotSeries {
                name: String::new(),
                color: Color::rgb(240, 0, 0),
                right_axis: false,
                points: vec![
                    crate::PlotPoint {
                        x: 2.0,
                        y: 2.0,
                        row: 0,
                    },
                    crate::PlotPoint {
                        x: 8.0,
                        y: 8.0,
                        row: 1,
                    },
                ]
                .into(),
                line: PlotLine::StepLeft,
                marker: PlotMarker::None,
            }],
            ..PlotData::default()
        };
        data.state.view = Some(PlotView {
            x: (0.0, 10.0),
            y1: (0.0, 10.0),
            y2: (0.0, 10.0),
        });
        let style = PlotStyle {
            grid: Color::rgba(0, 0, 0, 0),
            ..PlotStyle::default()
        };
        let area = plot_area(400.0, 300.0, &data);
        let view = data.view();
        let red_near = |image: &RgbaImage, x: f64, y: f64| {
            let p = sample_pixel(&crate::PlotPoint { x, y, row: 0 }, false, view, area);
            (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    let i = (((p.1.round() as i32 + dy) as usize * 400)
                        + (p.0.round() as i32 + dx) as usize)
                        * 4;
                    let pixel = &image.pixels()[i..i + 4];
                    pixel[0] > 150 && pixel[1] < 100
                })
            })
        };
        let left = render_plot(&font, &data, style, 400, 300).unwrap();
        assert!(red_near(&left, 5.0, 2.0));
        assert!(!red_near(&left, 5.0, 8.0));
        data.series[0].line = PlotLine::StepRight;
        let right = render_plot(&font, &data, style, 400, 300).unwrap();
        assert!(red_near(&right, 5.0, 8.0));
        assert!(!red_near(&right, 5.0, 2.0));
    }

    #[test]
    fn hiding_legend_releases_axis_space_and_removes_only_legend_ink() {
        let font = next_gui_text::load_default_cjk_font().unwrap();
        let mut data = PlotData {
            series: vec![crate::PlotSeries {
                name: "Legend sample".into(), color: Color::rgb(255, 0, 0), right_axis: false,
                points: std::sync::Arc::from([crate::PlotPoint { x: 1.0, y: 2.0, row: 0 }]),
                line: PlotLine::None, marker: PlotMarker::None,
            }], ..PlotData::default()
        };
        let area = plot_area(400.0, 300.0, &data);
        let visible = render_plot(&font, &data, PlotStyle::default(), 400, 300).unwrap();
        data.show_legend = false;
        let hidden = render_plot(&font, &data, PlotStyle::default(), 400, 300).unwrap();
        assert!(plot_area(400.0, 300.0, &data).y < area.y);
        let red = |image: &RgbaImage| image.pixels().chunks_exact(4).filter(|p| p[0] > 200 && p[1] < 40 && p[2] < 40).count();
        assert!(red(&visible) > 0);
        assert_eq!(red(&hidden), 0);
    }

    #[test]
    fn categorical_bars_keep_duplicate_labels_and_negative_values() {
        let font = next_gui_text::load_default_cjk_font().unwrap();
        let format =
            PlotAxisFormat::Labels(vec!["East".into(), "West".into(), "East".into()].into());
        assert_eq!(
            ticks((-0.5, 2.5), &format, 6, false)
                .iter()
                .map(|(_, s)| s.as_str())
                .collect::<Vec<_>>(),
            ["East", "West", "East"]
        );
        let mut data = PlotData {
            x_format: format,
            series: vec![crate::PlotSeries {
                name: "Balance".into(),
                color: Color::rgb(240, 0, 0),
                right_axis: false,
                points: vec![
                    crate::PlotPoint {
                        x: 0.0,
                        y: 2.0,
                        row: 0,
                    },
                    crate::PlotPoint {
                        x: 1.0,
                        y: -3.0,
                        row: 1,
                    },
                    crate::PlotPoint {
                        x: 2.0,
                        y: 1.0,
                        row: 2,
                    },
                ]
                .into(),
                line: PlotLine::Bars,
                marker: PlotMarker::None,
            }],
            ..PlotData::default()
        };
        data.state.view = Some(PlotView {
            x: (-0.5, 2.5),
            y1: (-4.0, 4.0),
            y2: (-1.0, 1.0),
        });
        let image = render_plot(&font, &data, PlotStyle::default(), 400, 300).unwrap();
        let area = plot_area(400.0, 300.0, &data);
        for (x, y, inside) in [
            (0.0, 1.0, true),
            (1.0, -2.0, true),
            (1.0, 1.0, false),
            (2.0, 0.5, true),
        ] {
            let point = sample_pixel(&crate::PlotPoint { x, y, row: 0 }, false, data.view(), area);
            let i = (point.1 as usize * 400 + point.0 as usize) * 4;
            let pixel = &image.pixels()[i..i + 4];
            assert_eq!(pixel[0] > 200 && pixel[1] < 100, inside, "bar at {x}/{y}");
        }
    }

    #[test]
    fn markers_alpha_and_plot_image_bounds_are_observable() {
        use PlotMarker::*;
        let mut shapes = Vec::new();
        for marker in [
            None,
            Cross,
            Plus,
            Circle,
            Disc,
            Square,
            Diamond,
            Star,
            Triangle,
            TriangleInverted,
            CrossSquare,
            PlusSquare,
            CrossCircle,
            PlusCircle,
            Peace,
        ] {
            let mut canvas = Canvas {
                width: 24,
                height: 24,
                pixels: vec![0; 24 * 24 * 4],
                clip: Rect::new(0.0, 0.0, 24.0, 24.0),
            };
            canvas.marker((12.0, 12.0), marker, Color::rgba(230, 40, 90, 128), 5.0);
            assert!(
                shapes.iter().all(|old| old != &canvas.pixels),
                "indistinguishable {marker:?}"
            );
            if marker != None {
                assert!(canvas.pixels.chunks_exact(4).any(|p| p[3] > 0));
            }
            shapes.push(canvas.pixels);
        }
        let mut canvas = Canvas {
            width: 1,
            height: 1,
            pixels: vec![0; 4],
            clip: Rect::new(0.0, 0.0, 1.0, 1.0),
        };
        canvas.pixel(0, 0, Color::rgba(255, 0, 0, 128), 1.0);
        canvas.pixel(0, 0, Color::rgba(0, 0, 255, 128), 1.0);
        assert_eq!(canvas.pixels, [85, 0, 170, 192]);
        let font = next_gui_text::load_default_cjk_font().unwrap();
        assert!(render_plot(&font, &PlotData::default(), PlotStyle::default(), 0, 100).is_none());
        assert!(render_plot(
            &font,
            &PlotData::default(),
            PlotStyle::default(),
            100_000,
            100_000
        )
        .is_none());
        let image =
            render_plot(&font, &PlotData::default(), PlotStyle::default(), 220, 160).unwrap();
        assert_eq!(image.pixels().len(), 220 * 160 * 4);
        assert!(image.pixels().chunks_exact(4).all(|p| p[3] == 255));
    }
}
