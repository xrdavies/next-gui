//! Plot the current typed result without executing its SQL again.
use crate::{cell_snapshot::CellSnapshot, db::CellValue};
use next_gui_scene::Color;
use next_gui_ui::{
    PlotAxisFormat, PlotData, PlotLine, PlotMarker, PlotPoint, PlotSeries, WidgetId,
};
use std::{collections::BTreeSet, sync::Arc};

pub const LINE_NAMES: [&str; 7] = [
    "No line",
    "Line",
    "Step left",
    "Step right",
    "Step center",
    "Impulse",
    "Bars",
];
pub const LINES: [PlotLine; 7] = [
    PlotLine::None,
    PlotLine::Line,
    PlotLine::StepLeft,
    PlotLine::StepRight,
    PlotLine::StepCenter,
    PlotLine::Impulse,
    PlotLine::Bars,
];
pub const MARKER_NAMES: [&str; 15] = [
    "No marker",
    "Cross",
    "Plus",
    "Circle",
    "Disc",
    "Square",
    "Diamond",
    "Star",
    "Triangle",
    "Inverted triangle",
    "Cross square",
    "Plus square",
    "Cross circle",
    "Plus circle",
    "Peace",
];
const MARKERS: [PlotMarker; 15] = [
    PlotMarker::None,
    PlotMarker::Cross,
    PlotMarker::Plus,
    PlotMarker::Circle,
    PlotMarker::Disc,
    PlotMarker::Square,
    PlotMarker::Diamond,
    PlotMarker::Star,
    PlotMarker::Triangle,
    PlotMarker::TriangleInverted,
    PlotMarker::CrossSquare,
    PlotMarker::PlusSquare,
    PlotMarker::CrossCircle,
    PlotMarker::PlusCircle,
    PlotMarker::Peace,
];
const PALETTE: [[u8; 3]; 8] = [
    [51, 133, 235],
    [233, 116, 79],
    [49, 172, 139],
    [170, 113, 222],
    [215, 161, 50],
    [48, 169, 193],
    [220, 106, 156],
    [116, 143, 72],
];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Source {
    Table { schema: String, table: String },
    Query(usize),
}

/// Names survive column reordering; occurrence distinguishes duplicate SQL headers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Column {
    Row,
    Field { name: String, occurrence: usize },
}
impl Column {
    pub fn name(&self) -> &str {
        match self {
            Self::Row => "Row #",
            Self::Field { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Series {
    pub column: Column,
    pub right: bool,
    pub color: [u8; 3],
    pub active: bool,
    #[serde(default)]
    pub line: Option<usize>,
    /// QCustomPlot shape number; its hidden ssDot occupies index 1.
    #[serde(default)]
    pub point_shape: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub source: Source,
    pub x: Option<Column>,
    pub series: Vec<Series>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub manual: bool,
    pub line: usize,
    pub marker: usize,
    pub show_legend: bool,
    pub configs: Vec<Config>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            manual: false,
            line: 1,
            marker: 0,
            show_legend: false,
            configs: Vec::new(),
        }
    }
}
impl Settings {
    pub fn rename_table(&mut self, from: (&str, &str), to: (&str, &str)) {
        for config in &mut self.configs {
            if matches!(&config.source, Source::Table { schema, table } if schema == from.0 && table == from.1)
            {
                config.source = Source::Table {
                    schema: to.0.into(),
                    table: to.1.into(),
                };
            }
        }
    }

    pub fn rename_columns(&mut self, schema: &str, table: &str, renames: &[(String, String)]) {
        for config in self.configs.iter_mut().filter(|c| matches!(&c.source, Source::Table { schema: s, table: t } if s == schema && t == table)) {
            for column in config.x.iter_mut().chain(config.series.iter_mut().map(|s| &mut s.column)) {
                if let Column::Field { name, .. } = column {
                    if let Some((_, to)) = renames.iter().find(|(from, _)| from.eq_ignore_ascii_case(name)) { *name = to.clone(); }
                }
            }
        }
    }

    pub fn swap_queries(&mut self, a: usize, b: usize) {
        for config in &mut self.configs {
            if config.source == Source::Query(a) {
                config.source = Source::Query(b);
            } else if config.source == Source::Query(b) {
                config.source = Source::Query(a);
            }
        }
    }

    pub fn close_query(&mut self, index: usize) {
        self.configs.retain(|c| c.source != Source::Query(index));
        for config in &mut self.configs {
            if let Source::Query(i) = &mut config.source {
                if *i > index {
                    *i -= 1;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    Numeric,
    Date,
    Time,
    DateTime,
    Label,
}
impl DataType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Numeric => "Numeric",
            Self::Date => "Date",
            Self::Time => "Time",
            Self::DateTime => "Date/Time",
            Self::Label => "Label",
        }
    }
    fn format(self) -> PlotAxisFormat {
        match self {
            Self::Numeric | Self::Label => PlotAxisFormat::Numeric,
            Self::Date => PlotAxisFormat::Date,
            Self::Time => PlotAxisFormat::Time,
            Self::DateTime => PlotAxisFormat::DateTime,
        }
    }
}

#[derive(Debug, Default)]
pub struct Model {
    pub settings: Settings,
    pub data: PlotData,
    pub columns: Vec<(Column, DataType)>,
    pub title: String,
    pub summary: String,
    pub selected: Option<(usize, usize)>,
    pub source: Option<Source>,
    pub linked_rows: Arc<BTreeSet<usize>>,
    snapshot: Option<Arc<()>>,
    offset: usize,
    dirty: bool,
}

impl Model {
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    pub fn clear(&mut self) {
        self.data = PlotData::default();
        self.columns.clear();
        self.source = None;
        self.snapshot = None;
        self.selected = None;
        self.linked_rows = Arc::default();
        self.title.clear();
        self.summary = "Select a table or execute a query".into();
    }

    fn config(&self) -> Option<&Config> {
        self.settings
            .configs
            .iter()
            .find(|c| Some(&c.source) == self.source.as_ref())
    }

    pub fn checked(&self, row: usize, axis: usize) -> Option<bool> {
        let (column, kind) = self.columns.get(row)?;
        let config = self.config();
        match axis {
            1 => Some(config.is_some_and(|c| c.x.as_ref() == Some(column))),
            2 | 3 if *kind == DataType::Numeric => Some(config.is_some_and(|c| {
                c.series
                    .iter()
                    .any(|s| &s.column == column && s.right == (axis == 3) && s.active)
            })),
            _ => None,
        }
    }

    pub fn check(&mut self, row: usize, axis: usize, checked: bool) -> bool {
        if self.checked(row, axis).is_none() {
            return false;
        }
        let Some(source) = self.source.clone() else {
            return false;
        };
        let (line, marker) = self.styles();
        let column = self.columns[row].0.clone();
        if self.config().is_none() {
            self.settings.configs.push(Config {
                source: source.clone(),
                x: None,
                series: Vec::new(),
            });
        }
        let config = self
            .settings
            .configs
            .iter_mut()
            .find(|c| c.source == source)
            .unwrap();
        if axis == 1 {
            config.x = checked.then_some(column);
        } else if let Some(series) = config
            .series
            .iter_mut()
            .find(|s| s.column == column && s.right == (axis == 3))
        {
            series.active = checked;
        } else {
            config.series.push(Series {
                column,
                right: axis == 3,
                color: PALETTE[(row + axis - 2) % PALETTE.len()],
                active: checked,
                line: Some(line),
                point_shape: Some(point_shape(marker)),
            });
        }
        self.selected = Some((row, axis));
        self.invalidate();
        true
    }

    pub fn selected_color(&self) -> Option<Color> {
        let (row, axis @ (2 | 3)) = self.selected? else {
            return None;
        };
        let column = &self.columns.get(row)?.0;
        self.config()?
            .series
            .iter()
            .find(|s| &s.column == column && s.right == (axis == 3))
            .map(|s| Color::rgb(s.color[0], s.color[1], s.color[2]))
    }

    pub fn styles(&self) -> (usize, usize) {
        let series = self.config().and_then(|config| {
            self.selected.and_then(|(row, axis)| self.columns.get(row).and_then(|(column, _)| {
                config.series.iter().find(|s| matches!(axis, 2 | 3) && &s.column == column && s.right == (axis == 3))
            })).or_else(|| config.series.iter().find(|s| s.active))
        });
        let line = series.and_then(|s| s.line).unwrap_or(self.settings.line);
        let shape = series.and_then(|s| s.point_shape).unwrap_or_else(|| point_shape(self.settings.marker));
        (if line < LINES.len() { line } else { 1 }, if shape < 16 { shape.saturating_sub(1) } else { 0 })
    }

    pub fn set_styles(&mut self, line: usize, marker: usize) -> bool {
        let previous = self.styles();
        let line_changed = previous.0 != line;
        let marker_changed = previous.1 != marker;
        if !line_changed && !marker_changed { return false; }
        for config in &mut self.settings.configs {
            for series in &mut config.series {
                series.line.get_or_insert(self.settings.line);
                series.point_shape.get_or_insert(point_shape(self.settings.marker));
            }
        }
        self.settings.line = line;
        self.settings.marker = marker;
        if let Some(config) = self.settings.configs.iter_mut().find(|c| Some(&c.source) == self.source.as_ref()) {
            for series in &mut config.series {
                if line_changed { series.line = Some(line); }
                if marker_changed { series.point_shape = Some(point_shape(marker)); }
            }
        }
        self.invalidate();
        true
    }

    pub fn set_color(&mut self, color: Color) -> bool {
        let Some((row, axis @ (2 | 3))) = self.selected else {
            return false;
        };
        let Some((column, _)) = self.columns.get(row) else {
            return false;
        };
        let Some(series) = self
            .settings
            .configs
            .iter_mut()
            .find(|c| Some(&c.source) == self.source.as_ref())
            .and_then(|c| {
                c.series
                    .iter_mut()
                    .find(|s| &s.column == column && s.right == (axis == 3))
            })
        else {
            return false;
        };
        series.color = [color.r, color.g, color.b];
        self.invalidate();
        true
    }

    pub fn matches_rendered(&self, data: &PlotData) -> bool {
        data.series.len() == self.data.series.len()
            && data
                .series
                .iter()
                .zip(&self.data.series)
                .all(|(a, b)| a.right_axis == b.right_axis && Arc::ptr_eq(&a.points, &b.points))
    }

    pub fn sync(
        &mut self,
        source: Source,
        result: &crate::db::QueryResult,
        offset: usize,
        total: Option<usize>,
    ) -> Result<(), String> {
        self.data.show_legend = self.settings.show_legend;
        let identity = result.values.identity();
        self.summary = if let Some(total) = total {
            format!(
                "{}–{} of {total} rows",
                if result.values.is_empty() {
                    0
                } else {
                    offset.saturating_add(1)
                },
                offset.saturating_add(result.values.len())
            )
        } else {
            format!(
                "{} rows{}",
                result.values.len(),
                if result.truncated { " · preview" } else { "" }
            )
        };
        if !self.dirty
            && self.offset == offset
            && self.source.as_ref() == Some(&source)
            && self
                .snapshot
                .as_ref()
                .is_some_and(|old| Arc::ptr_eq(old, &identity))
        {
            return Ok(());
        }
        if self.source.as_ref() != Some(&source) {
            self.selected = None;
        }
        let kinds = guess(&result.values, result.columns.len())?;
        let mut columns = vec![(Column::Row, DataType::Numeric)];
        for (index, name) in result.columns.iter().enumerate() {
            let occurrence = result.columns[..index]
                .iter()
                .filter(|s| *s == name)
                .count();
            columns.push((
                Column::Field {
                    name: name.clone(),
                    occurrence,
                },
                kinds[index],
            ));
        }
        let empty = Config {
            source: source.clone(),
            x: None,
            series: Vec::new(),
        };
        let config = self
            .settings
            .configs
            .iter()
            .find(|c| c.source == source)
            .unwrap_or(&empty);
        let x_index = config
            .x
            .as_ref()
            .and_then(|x| columns.iter().position(|(c, _)| c == x));
        let mut data = PlotData {
            show_legend: self.settings.show_legend,
            ..PlotData::default()
        };
        if let Some(x_index) = x_index {
            let x_type = columns[x_index].1;
            data.x_label = columns[x_index].0.name().into();
            data.x_format = x_type.format();
            let active: Vec<_> = config
                .series
                .iter()
                .filter(|s| s.active)
                .filter_map(|series| {
                    columns
                        .iter()
                        .position(|(c, kind)| c == &series.column && *kind == DataType::Numeric)
                        .map(|i| (series, i))
                })
                .collect();
            let mut points: Vec<Vec<PlotPoint>> = active
                .iter()
                .map(|_| Vec::with_capacity(result.values.len()))
                .collect();
            let mut labels = Vec::new();
            for index in 0..result.values.len() {
                let cells = result.values.row(index)?;
                let row = offset.checked_add(index).ok_or("Plot row index overflow")?;
                let ordinal = row.saturating_add(1) as f64;
                let x = if x_index == 0 {
                    ordinal
                } else if x_type == DataType::Label {
                    labels.push(label(&cells[x_index - 1]));
                    // Framework category coordinates start at zero; labels retain duplicates.
                    index as f64
                } else {
                    value(&cells[x_index - 1], x_type)
                };
                for ((_, column), points) in active.iter().zip(&mut points) {
                    let y = if *column == 0 {
                        ordinal
                    } else {
                        number(&cells[column - 1]).unwrap_or(f64::NAN)
                    };
                    points.push(PlotPoint { x, y, row });
                }
            }
            if x_type == DataType::Label {
                data.x_format = PlotAxisFormat::Labels(labels.into());
            }
            for ((series, _), points) in active.into_iter().zip(points) {
                let caption = if series.right {
                    &mut data.y2_label
                } else {
                    &mut data.y1_label
                };
                if !caption.is_empty() {
                    caption.push_str(" | ");
                }
                caption.push_str(series.column.name());
                data.series.push(PlotSeries {
                    name: series.column.name().into(),
                    color: Color::rgb(series.color[0], series.color[1], series.color[2]),
                    right_axis: series.right,
                    points: points.into(),
                    line: if x_type == DataType::Label { PlotLine::Bars } else {
                        *LINES.get(series.line.unwrap_or(self.settings.line)).unwrap_or(&PlotLine::Line)
                    },
                    marker: if x_type == DataType::Label { PlotMarker::None } else {
                        let shape = series.point_shape.unwrap_or_else(|| point_shape(self.settings.marker));
                        // shortcut: legacy ssDot uses Disc; add a distinct glyph if dot projects require it.
                        if shape == 1 { PlotMarker::Disc } else {
                            *MARKERS.get(shape.saturating_sub(1)).unwrap_or(&PlotMarker::None)
                        }
                    },
                });
            }
        }
        self.title = match &source {
            Source::Table { schema, table } => crate::db::qualified_name(schema, table),
            Source::Query(index) => format!("SQL {} · result", index + 1),
        };
        self.columns = columns;
        self.data = data;
        self.linked_rows = Arc::default();
        self.source = Some(source);
        self.snapshot = Some(identity);
        self.offset = offset;
        self.dirty = false;
        Ok(())
    }
}

pub fn point_shape(marker: usize) -> usize {
    if marker == 0 { 0 } else { marker.saturating_add(1) }
}

fn number(value: &CellValue) -> Option<f64> {
    let value = match value {
        CellValue::Integer(n) => *n as f64,
        CellValue::Real(n) => *n,
        CellValue::Text(bytes) => std::str::from_utf8(bytes).ok()?.trim().parse().ok()?,
        _ => return None,
    };
    value.is_finite().then_some(value)
}

fn label(value: &CellValue) -> String {
    match value {
        CellValue::Null => "NULL".into(),
        CellValue::Blob(_) => "BLOB".into(),
        CellValue::Text(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        CellValue::Integer(n) => n.to_string(),
        CellValue::Real(n) => n.to_string(),
    }
}

fn value(value: &CellValue, kind: DataType) -> f64 {
    // shortcut: strict wall-clock dates; add ISO offsets/fractions for complete DB4S date parity.
    if kind == DataType::Numeric {
        return number(value).unwrap_or(f64::NAN);
    }
    let CellValue::Text(bytes) = value else {
        return f64::NAN;
    };
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| kind.format().parse(text.trim()))
        .unwrap_or(f64::NAN)
}

fn guess(values: &CellSnapshot, columns: usize) -> Result<Vec<DataType>, String> {
    let mut kinds = vec![DataType::Numeric; columns];
    for row in 0..values.len().min(10) {
        let cells = values.row(row)?;
        if cells.len() != columns {
            return Err("Plot source columns do not match its row values".into());
        }
        for (cell, kind) in cells.iter().zip(&mut kinds) {
            if *kind == DataType::Label
                || matches!(
                    cell,
                    CellValue::Null | CellValue::Integer(_) | CellValue::Real(_)
                )
                || number(cell).is_some()
            {
                continue;
            }
            if value(cell, DataType::DateTime).is_finite() {
                *kind = DataType::DateTime;
            } else if value(cell, DataType::Date).is_finite() {
                if *kind != DataType::DateTime {
                    *kind = DataType::Date;
                }
            } else if value(cell, DataType::Time).is_finite() {
                *kind = DataType::Time;
            } else {
                *kind = DataType::Label;
            }
        }
    }
    Ok(kinds)
}

#[derive(Clone, Copy)]
pub struct Controls {
    pub mode: WidgetId,
    pub columns: WidgetId,
    pub line: WidgetId,
    pub marker: WidgetId,
    pub color: WidgetId,
    pub fit: WidgetId,
    pub legend: WidgetId,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            mode: WidgetId(0),
            columns: WidgetId(0),
            line: WidgetId(0),
            marker: WidgetId(0),
            color: WidgetId(0),
            fit: WidgetId(0),
            legend: WidgetId(0),
        }
    }
}

pub fn build(
    builder: &mut next_gui_ui::FlexBuilder<'_>,
    model: &Model,
    width: f32,
    height: f32,
    theme: &next_gui_ui::Theme,
) -> Controls {
    use next_gui_ui::{ColorPickerStyle, RowStyle, SelectStyle, TableColumn, TableStyle};
    let mut ids = Controls::default();
    let mut select = SelectStyle::from_theme(theme);
    select.width = 140.0;
    let mut row = RowStyle::from_theme(theme);
    row.width = width;
    row.height = 32.0;
    row.padding = 0.0;
    row.gap = 4.0;
    builder.row(row, |r| {
        ids.mode = r.select(
            ["Current data", "Table preview"],
            Some(usize::from(model.settings.manual)),
            "source",
            select,
        );
        if !model.settings.manual {
            ids.legend = r.checkbox("Legend", model.settings.show_legend);
        }
    });
    if model.settings.manual {
        return ids;
    }
    let mut table = TableStyle::from_theme(theme);
    table.width = width;
    table.height = heights(height).0;
    table.row_height = 26.0;
    table.header_height = 28.0;
    let name_width = (width - 210.0).max(120.0);
    ids.columns = builder.table(
        vec![
            TableColumn::new("Columns", name_width),
            TableColumn::new("X", 36.0),
            TableColumn::new("Y1", 36.0),
            TableColumn::new("Y2", 36.0),
            TableColumn::new("Axis type", 100.0),
        ],
        model
            .columns
            .iter()
            .enumerate()
            .map(|(row, (column, kind))| {
                vec![
                    column.name().into(),
                    if model.checked(row, 1) == Some(true) {
                        "✓".into()
                    } else {
                        String::new()
                    },
                    if model.checked(row, 2) == Some(true) {
                        "✓".into()
                    } else {
                        String::new()
                    },
                    if model.checked(row, 3) == Some(true) {
                        "✓".into()
                    } else {
                        String::new()
                    },
                    kind.name().into(),
                ]
            })
            .collect(),
        table,
    );
    row.height = 34.0;
    builder.row(row, |r| {
        let mut style = select;
        style.width = ((width - 164.0) / 2.0).max(64.0);
        let (line, marker) = model.styles();
        ids.line = r.select(
            LINE_NAMES,
            Some(line),
            "Line",
            style,
        );
        ids.marker = r.select(
            MARKER_NAMES,
            Some(marker),
            "Marker",
            style,
        );
        ids.fit = r.tool_button("Fit");
        if let Some(color) = model.selected_color() {
            let mut style = ColorPickerStyle::from_theme(theme);
            style.width = 84.0;
            style.height = 30.0;
            style.hex_width = 78.0;
            style.size = 11.0;
            style.padding = 0.0;
            ids.color = r.color_picker_with_presets(color, [], style);
        }
    });
    ids
}

pub fn heights(available: f32) -> (f32, f32) {
    let columns = (available * 0.25 - 12.0).clamp(64.0, 154.0);
    (columns, (available - columns - 90.0).max(160.0))
}

pub fn configure(ui: &mut next_gui_ui::Ui, ids: Controls, model: &Model, bg: Color) {
    if ids.columns.0 != 0 {
        ui.set_table_check_cells(
            ids.columns,
            (0..model.columns.len())
                .map(|row| (0..5).map(|axis| model.checked(row, axis)).collect())
                .collect(),
        );
        if let Some((row, column)) = model.selected {
            ui.set_table_selected_row(ids.columns, Some(row));
            ui.set_table_selected_col(ids.columns, Some(column));
        }
        let colors = model
            .columns
            .iter()
            .map(|(column, _)| {
                (0..5)
                    .map(|axis| {
                        if !matches!(axis, 2 | 3) {
                            return None;
                        }
                        model
                            .config()
                            .and_then(|c| {
                                c.series
                                    .iter()
                                    .find(|s| &s.column == column && s.right == (axis == 3))
                            })
                            .map(|s| {
                                let mix = |base: u8, color: u8| {
                                    ((u16::from(base) * 4 + u16::from(color)) / 5) as u8
                                };
                                Color::rgb(
                                    mix(bg.r, s.color[0]),
                                    mix(bg.g, s.color[1]),
                                    mix(bg.b, s.color[2]),
                                )
                            })
                    })
                    .collect()
            })
            .collect();
        ui.set_table_cell_bg(ids.columns, colors);
    }
    for (id, name) in [
        (ids.mode, "Plot data source"),
        (
            ids.columns,
            "Plot columns: choose one X axis and numeric Y1/Y2 series",
        ),
        (ids.line, "Plot line style"),
        (ids.marker, "Plot marker shape"),
        (ids.color, "Selected plot series color"),
        (ids.fit, "Fit plot to all loaded points"),
        (ids.legend, "Show plot legend"),
    ] {
        if id.0 != 0 {
            ui.set_accessible_name(id, name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn table() -> Source {
        Source::Table {
            schema: "aux.\"db".into(),
            table: "客户".into(),
        }
    }
    fn result(sql: &str) -> crate::db::QueryResult {
        crate::db::execute_sql(&rusqlite::Connection::open_in_memory().unwrap(), sql).unwrap()
    }

    #[test]
    fn current_result_keeps_types_gaps_row_order_dual_axes_and_cached_buffers() {
        let rows = result("SELECT 3 AS x, 9007199254740993 AS y, '2026-10-28' AS day UNION ALL SELECT 1, NULL, '2026-10-29' UNION ALL SELECT 2, 1e100, NULL");
        let mut model = Model::default();
        model.sync(table(), &rows, 500, Some(1003)).unwrap();
        assert!(model.settings.configs.is_empty());
        assert_eq!(model.columns[3].1, DataType::Date);
        assert_eq!(model.checked(3, 2), None);
        assert!(model.check(1, 1, true));
        assert!(model.check(2, 2, true));
        assert!(model.check(2, 3, true));
        model.sync(table(), &rows, 500, Some(1003)).unwrap();
        assert_eq!(model.data.series.len(), 2);
        assert!(!model.data.series[0].right_axis);
        assert!(model.data.series[1].right_axis);
        let points = model.data.series[0].points.clone();
        assert_eq!(
            points.iter().map(|p| p.x).collect::<Vec<_>>(),
            [3.0, 1.0, 2.0]
        );
        assert_eq!(
            points.iter().map(|p| p.row).collect::<Vec<_>>(),
            [500, 501, 502]
        );
        assert_eq!(points[0].y, 9007199254740993i64 as f64);
        assert!(points[1].y.is_nan());
        assert_eq!(points[2].y, 1e100);
        model.data.zoom(0.5, 0.5, 0.5);
        let view = model.data.view();
        model.sync(table(), &rows, 500, Some(2003)).unwrap();
        assert!(Arc::ptr_eq(&points, &model.data.series[0].points));
        assert_eq!(model.data.view(), view);
        assert!(model.summary.ends_with("2003 rows"));
        model.sync(Source::Query(0), &rows, 0, None).unwrap();
        assert!(model.data.series.is_empty());
        model.sync(table(), &rows, 500, Some(1003)).unwrap();
        assert_eq!(model.data.series.len(), 2);
        assert!(model.check(0, 1, true));
        model.sync(table(), &rows, 500, None).unwrap();
        assert_eq!(model.checked(1, 1), Some(false));
        assert_eq!(model.data.series[0].points[0].x, 501.0);
        assert!(model.check(0, 1, false));
        model.sync(table(), &rows, 500, None).unwrap();
        assert!(model.data.series.is_empty());
    }

    #[test]
    fn returning_is_a_snapshot_and_replaced_results_invalidate_plot_sessions() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE counter(value);INSERT INTO counter VALUES(0)")
            .unwrap();
        let first =
            crate::db::execute_sql(&conn, "UPDATE counter SET value=value+1 RETURNING value")
                .unwrap();
        let mut model = Model::default();
        model.sync(Source::Query(0), &first, 0, None).unwrap();
        model.check(0, 1, true);
        model.check(1, 2, true);
        model.sync(Source::Query(0), &first, 0, None).unwrap();
        let points = model.data.series[0].points.clone();
        model.data.zoom(0.5, 0.5, 0.5);
        model
            .sync(Source::Query(0), &first.clone(), 0, None)
            .unwrap();
        assert!(Arc::ptr_eq(&points, &model.data.series[0].points));
        assert_eq!(
            conn.query_row("SELECT value FROM counter", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        let second =
            crate::db::execute_sql(&conn, "UPDATE counter SET value=value+1 RETURNING value")
                .unwrap();
        model.sync(Source::Query(0), &second, 0, None).unwrap();
        assert!(!Arc::ptr_eq(&points, &model.data.series[0].points));
        assert_eq!(model.data.series[0].points[0].y, 2.0);
        assert!(model.data.state.view.is_none());
        let mut changed = second.clone();
        changed.values.push(vec![CellValue::Integer(3)]).unwrap();
        assert!(!Arc::ptr_eq(
            &second.values.identity(),
            &changed.values.identity()
        ));
        model.sync(Source::Query(0), &changed, 0, None).unwrap();
        assert_eq!(model.data.series[0].points.len(), 2);
        let nonfinite = crate::db::execute_sql(&conn, "SELECT 1e999 AS value").unwrap();
        model.sync(Source::Query(1), &nonfinite, 0, None).unwrap();
        assert_eq!(model.columns[1].1, DataType::Numeric);
        model.check(0, 1, true);
        model.check(1, 2, true);
        model.sync(Source::Query(1), &nonfinite, 0, None).unwrap();
        assert!(model.data.series[0].points[0].y.is_nan());
    }

    #[test]
    fn category_duplicates_nulls_blobs_and_duplicate_headers_stay_distinct() {
        let rows = result("SELECT 'A' AS v, 2 AS v, x'3132' AS blob UNION ALL SELECT 'A', -3, NULL UNION ALL SELECT NULL, 4, x'3334'");
        let mut model = Model::default();
        model.sync(Source::Query(2), &rows, 0, None).unwrap();
        assert_eq!(model.columns[1].1, DataType::Label);
        assert_eq!(model.columns[3].1, DataType::Label);
        assert_ne!(model.columns[1].0, model.columns[2].0);
        assert!(model.check(1, 1, true));
        assert!(model.check(2, 2, true));
        assert!(!model.check(3, 2, true));
        assert!(model.set_color(Color::rgb(18, 52, 86)));
        model.settings.line = 6;
        model.settings.marker = 4;
        model.sync(Source::Query(2), &rows, 0, None).unwrap();
        assert_eq!(
            model.data.x_format,
            PlotAxisFormat::Labels(Arc::from(["A".into(), "A".into(), "NULL".into()]))
        );
        assert_eq!(
            model.data.series[0]
                .points
                .iter()
                .map(|p| p.x)
                .collect::<Vec<_>>(),
            [0.0, 1.0, 2.0]
        );
        assert_eq!(model.data.series[0].color, Color::rgb(18, 52, 86));
        assert_eq!(model.data.series[0].line, PlotLine::Bars);
        assert_eq!(model.data.series[0].marker, PlotMarker::None);
        let settings: Settings =
            serde_json::from_str(&serde_json::to_string(&model.settings).unwrap()).unwrap();
        assert_eq!(settings, model.settings);
    }

    #[test]
    fn native_series_styles_and_current_source_style_changes_remain_independent() {
        let rows = result("SELECT 1 AS x, 3 AS y UNION ALL SELECT 2, 4");
        let mut model = Model::default();
        model.sync(table(), &rows, 0, None).unwrap();
        model.check(1, 1, true);
        model.check(2, 2, true);
        model.check(2, 3, true);
        model.settings.configs[0].series[0].line = Some(2);
        model.settings.configs[0].series[0].point_shape = Some(5);
        model.settings.configs[0].series[1].line = Some(5);
        model.settings.configs[0].series[1].point_shape = Some(0);
        model.sync(table(), &rows, 0, None).unwrap();
        assert_eq!((model.data.series[0].line, model.data.series[0].marker), (PlotLine::StepLeft, PlotMarker::Disc));
        assert_eq!((model.data.series[1].line, model.data.series[1].marker), (PlotLine::Impulse, PlotMarker::None));
        assert_eq!(model.styles(), (5, 0));
        assert!(!model.set_styles(5, 0));
        let mut legacy = model.settings.configs[0].clone();
        legacy.source = Source::Query(9);
        for series in &mut legacy.series { series.line = None; series.point_shape = None; }
        model.settings.configs.push(legacy);
        model.sync(Source::Query(0), &rows, 0, None).unwrap();
        model.check(0, 1, true);
        model.check(2, 2, true);
        assert!(model.set_styles(4, 14));
        let legacy = model.settings.configs.iter().find(|c| c.source == Source::Query(9)).unwrap();
        assert!(legacy.series.iter().all(|s| s.line == Some(1) && s.point_shape == Some(0)));
        model.sync(Source::Query(0), &rows, 0, None).unwrap();
        assert_eq!((model.data.series[0].line, model.data.series[0].marker), (PlotLine::StepCenter, PlotMarker::Peace));
        model.sync(table(), &rows, 0, None).unwrap();
        assert_eq!((model.data.series[0].line, model.data.series[0].marker), (PlotLine::StepLeft, PlotMarker::Disc));
        assert!(model.set_styles(3, 1));
        model.sync(table(), &rows, 0, None).unwrap();
        assert!(model.data.series.iter().all(|s| s.line == PlotLine::StepRight && s.marker == PlotMarker::Cross));
        let categories = result("SELECT 'same' AS x, 3 AS y UNION ALL SELECT 'same', 4");
        model.sync(table(), &categories, 0, None).unwrap();
        assert!(model.data.series.iter().all(|s| s.line == PlotLine::Bars && s.marker == PlotMarker::None));
    }

    #[test]
    fn settings_follow_column_moves_renames_namespaces_and_sql_tab_lifecycle() {
        let rows = result("SELECT 5 AS x, 9 AS y");
        let mut model = Model::default();
        model.sync(table(), &rows, 0, None).unwrap();
        model.check(1, 1, true);
        model.check(2, 3, true);
        model
            .settings
            .rename_table(("aux.\"db", "客户"), ("main", "new"));
        model
            .settings
            .rename_columns("main", "new", &[("y".into(), "renamed".into())]);
        let reordered = result("SELECT 9 AS renamed, 5 AS x");
        model
            .sync(
                Source::Table {
                    schema: "main".into(),
                    table: "new".into(),
                },
                &reordered,
                0,
                None,
            )
            .unwrap();
        assert_eq!(model.checked(2, 1), Some(true));
        assert_eq!(model.checked(1, 3), Some(true));
        assert_eq!(
            model.data.series[0].points[0],
            PlotPoint {
                x: 5.0,
                y: 9.0,
                row: 0
            }
        );
        model.sync(Source::Query(1), &rows, 0, None).unwrap();
        model.check(0, 1, true);
        model.check(2, 2, true);
        model.settings.swap_queries(1, 3);
        assert!(model
            .settings
            .configs
            .iter()
            .any(|c| c.source == Source::Query(3)));
        model.settings.close_query(0);
        assert!(model
            .settings
            .configs
            .iter()
            .any(|c| c.source == Source::Query(2)));
        model.settings.close_query(2);
        assert!(model
            .settings
            .configs
            .iter()
            .all(|c| !matches!(c.source, Source::Query(_))));
    }
}
