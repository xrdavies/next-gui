//! Office P1 deferred widgets: DateTime, Transfer, PropertyGrid, TreeTable.

use next_gui_scene::Color;

/// Civil date+time value for DateTime picker (local wall clock, no TZ).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTimeValue {
    /// Year (e.g. 2026).
    pub year: i32,
    /// Month 1..=12.
    pub month: u8,
    /// Day 1..=31 (clamped to month length on commit).
    pub day: u8,
    /// Hour 0..=23.
    pub hour: u8,
    /// Minute 0..=59.
    pub minute: u8,
}

impl DateTimeValue {
    /// Construct a value (does not validate day-of-month).
    pub fn new(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Self {
        Self {
            year,
            month: month.clamp(1, 12),
            day: day.clamp(1, 31),
            hour: hour.min(23),
            minute: minute.min(59),
        }
    }

    /// Format as `YYYY-MM-DD HH:MM`.
    pub fn format_display(self) -> String {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute
        )
    }

    /// Days in `month` for `year` (simple Gregorian leap).
    pub fn days_in_month(year: i32, month: u8) -> u8 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                let leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
                if leap {
                    29
                } else {
                    28
                }
            }
            _ => 30,
        }
    }

    /// Clamp day to the month length.
    pub fn clamp_day(self) -> Self {
        let max = Self::days_in_month(self.year, self.month);
        Self {
            day: self.day.clamp(1, max),
            ..self
        }
    }
}

impl Default for DateTimeValue {
    fn default() -> Self {
        Self::new(2026, 10, 18, 8, 30)
    }
}

/// Style for DateTime field + popup.
#[derive(Debug, Clone, Copy)]
pub struct DateTimeStyle {
    /// Closed field width.
    pub width: f32,
    /// Closed field height.
    pub height: f32,
    /// Font size.
    pub size: f32,
    /// Text color.
    pub fg: Color,
    /// Placeholder when empty (unused when value always set).
    pub placeholder_fg: Color,
    /// Field background.
    pub bg: Color,
    /// Field border.
    pub border: Color,
    /// Focus / open border.
    pub focus_border: Color,
    /// Popup panel background.
    pub popup_bg: Color,
    /// Selected day / confirm accent.
    pub accent: Color,
    /// Popup width.
    pub popup_width: f32,
    /// Calendar cell size.
    pub cell: f32,
    /// Horizontal padding in field.
    pub padding: f32,
}

impl Default for DateTimeStyle {
    fn default() -> Self {
        Self {
            width: 200.0,
            height: 30.0,
            size: 13.0,
            fg: Color::rgb(0xe8, 0xec, 0xf0),
            placeholder_fg: Color::rgb(0x78, 0x80, 0x8c),
            bg: Color::rgb(0x1e, 0x1e, 0x22),
            border: Color::rgb(0x4a, 0x4a, 0x54),
            focus_border: Color::rgb(0x3b, 0x82, 0xf6),
            popup_bg: Color::rgb(0x2a, 0x2a, 0x34),
            accent: Color::rgb(0x2f, 0x5b, 0xa8),
            popup_width: 280.0,
            cell: 32.0,
            padding: 8.0,
        }
    }
}

/// One Transfer list entry (id + label).
#[derive(Debug, Clone)]
pub struct TransferItem {
    /// Stable id.
    pub id: u64,
    /// Display label.
    pub label: String,
}

impl TransferItem {
    /// Create an item.
    pub fn new(id: u64, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
        }
    }
}

/// Style for Transfer shuttle.
#[derive(Debug, Clone, Copy)]
pub struct TransferStyle {
    /// Total width.
    pub width: f32,
    /// Total height.
    pub height: f32,
    /// List panel width (each side).
    pub list_width: f32,
    /// Row height in lists.
    pub row_height: f32,
    /// Font size.
    pub size: f32,
    /// Text color.
    pub fg: Color,
    /// Selected text.
    pub selected_fg: Color,
    /// Panel background.
    pub bg: Color,
    /// Selected row background.
    pub selected_bg: Color,
    /// Header / title bar background.
    pub header_bg: Color,
    /// Button background.
    pub btn_bg: Color,
    /// Button text.
    pub btn_fg: Color,
    /// Gap between panels and buttons.
    pub gap: f32,
    /// Padding.
    pub padding: f32,
}

impl Default for TransferStyle {
    fn default() -> Self {
        Self {
            width: 520.0,
            height: 260.0,
            list_width: 200.0,
            row_height: 26.0,
            size: 12.0,
            fg: Color::rgb(0xd8, 0xdc, 0xe4),
            selected_fg: Color::rgb(0xff, 0xff, 0xff),
            bg: Color::rgb(0x22, 0x22, 0x28),
            selected_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            header_bg: Color::rgb(0x32, 0x32, 0x3c),
            btn_bg: Color::rgb(0x3a, 0x3a, 0x48),
            btn_fg: Color::rgb(0xf0, 0xf2, 0xf6),
            gap: 10.0,
            padding: 6.0,
        }
    }
}

/// Property value kinds for PropertyGrid editors.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// Free text.
    Text(String),
    /// Numeric (display + click cycles step stub via API).
    Number(f64),
    /// Boolean toggle.
    Bool(bool),
    /// Select among options; stores selected index.
    Select {
        /// Option labels.
        options: Vec<String>,
        /// Selected index.
        selected: usize,
    },
}

impl PropertyValue {
    /// Display string for the value cell.
    pub fn display(&self) -> String {
        match self {
            PropertyValue::Text(s) => s.clone(),
            PropertyValue::Number(n) => {
                if n.fract().abs() < 1e-9 {
                    format!("{:.0}", n)
                } else {
                    format!("{n:.3}")
                }
            }
            PropertyValue::Bool(b) => {
                if *b {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            PropertyValue::Select { options, selected } => options
                .get(*selected)
                .cloned()
                .unwrap_or_else(|| "—".into()),
        }
    }
}

/// One PropertyGrid row.
#[derive(Debug, Clone)]
pub struct PropertyRow {
    /// Property key / label.
    pub key: String,
    /// Editable value.
    pub value: PropertyValue,
}

impl PropertyRow {
    /// Create a row.
    pub fn new(key: impl Into<String>, value: PropertyValue) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }
}

/// Style for PropertyGrid.
#[derive(Debug, Clone, Copy)]
pub struct PropertyGridStyle {
    /// Width.
    pub width: f32,
    /// Height (viewport; scrolls if more rows).
    pub height: f32,
    /// Row height.
    pub row_height: f32,
    /// Key column width.
    pub key_width: f32,
    /// Font size.
    pub size: f32,
    /// Key text color.
    pub key_fg: Color,
    /// Value text color.
    pub value_fg: Color,
    /// Background.
    pub bg: Color,
    /// Selected row background.
    pub selected_bg: Color,
    /// Selected row text.
    pub selected_fg: Color,
    /// Header background.
    pub header_bg: Color,
    /// Padding.
    pub padding: f32,
}

impl Default for PropertyGridStyle {
    fn default() -> Self {
        Self {
            width: 320.0,
            height: 220.0,
            row_height: 28.0,
            key_width: 120.0,
            size: 12.0,
            key_fg: Color::rgb(0x90, 0x98, 0xa4),
            value_fg: Color::rgb(0xe0, 0xe4, 0xec),
            bg: Color::rgb(0x28, 0x28, 0x30),
            selected_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            selected_fg: Color::rgb(0xff, 0xff, 0xff),
            header_bg: Color::rgb(0x32, 0x32, 0x3c),
            padding: 4.0,
        }
    }
}

/// Tree-table node: tree label + additional cell strings (one per non-tree column).
#[derive(Debug, Clone)]
pub struct TreeTableNode {
    /// Stable id.
    pub id: u64,
    /// Tree column label.
    pub label: String,
    /// Extra cell values (aligned with columns after the tree column).
    pub cells: Vec<String>,
    /// Children.
    pub children: Vec<TreeTableNode>,
    /// Expanded.
    pub expanded: bool,
}

impl TreeTableNode {
    /// Branch or leaf; expanded when children exist.
    pub fn new(
        id: u64,
        label: impl Into<String>,
        cells: Vec<String>,
        children: Vec<TreeTableNode>,
    ) -> Self {
        let expanded = !children.is_empty();
        Self {
            id,
            label: label.into(),
            cells,
            children,
            expanded,
        }
    }

    /// Leaf node.
    pub fn leaf(id: u64, label: impl Into<String>, cells: Vec<String>) -> Self {
        Self {
            id,
            label: label.into(),
            cells,
            children: Vec::new(),
            expanded: false,
        }
    }
}

/// Style for TreeTable.
#[derive(Debug, Clone, Copy)]
pub struct TreeTableStyle {
    /// Outer x (top-level only).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Viewport width.
    pub width: f32,
    /// Viewport height.
    pub height: f32,
    /// Header row height.
    pub header_height: f32,
    /// Body row height.
    pub row_height: f32,
    /// Indent per depth.
    pub indent: f32,
    /// Tree column width.
    pub tree_col_width: f32,
    /// Font size.
    pub size: f32,
    /// Text color.
    pub fg: Color,
    /// Selected text.
    pub selected_fg: Color,
    /// Header text.
    pub header_fg: Color,
    /// Background.
    pub bg: Color,
    /// Header background.
    pub header_bg: Color,
    /// Selected row background.
    pub selected_bg: Color,
    /// Padding.
    pub padding: f32,
}

impl Default for TreeTableStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 480.0,
            height: 280.0,
            header_height: 28.0,
            row_height: 26.0,
            indent: 14.0,
            tree_col_width: 160.0,
            size: 12.0,
            fg: Color::rgb(0xd8, 0xdc, 0xe4),
            selected_fg: Color::rgb(0xff, 0xff, 0xff),
            header_fg: Color::rgb(0xb0, 0xb8, 0xc4),
            bg: Color::rgb(0x22, 0x22, 0x28),
            header_bg: Color::rgb(0x32, 0x32, 0x3c),
            selected_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            padding: 4.0,
        }
    }
}

/// Flat tree-table row for paint / hit.
#[derive(Debug, Clone)]
pub(crate) struct FlatTreeTableRow {
    pub id: u64,
    pub depth: usize,
    pub label: String,
    pub cells: Vec<String>,
    pub has_children: bool,
    pub expanded: bool,
}

/// Flatten expanded tree-table nodes.
pub(crate) fn flatten_tree_table(roots: &[TreeTableNode]) -> Vec<FlatTreeTableRow> {
    let mut out = Vec::new();
    fn walk(nodes: &[TreeTableNode], depth: usize, out: &mut Vec<FlatTreeTableRow>) {
        for n in nodes {
            let has = !n.children.is_empty();
            out.push(FlatTreeTableRow {
                id: n.id,
                depth,
                label: n.label.clone(),
                cells: n.cells.clone(),
                has_children: has,
                expanded: n.expanded,
            });
            if has && n.expanded {
                walk(&n.children, depth + 1, out);
            }
        }
    }
    walk(roots, 0, &mut out);
    out
}

pub(crate) fn toggle_tree_table_expanded(nodes: &mut [TreeTableNode], id: u64) -> bool {
    for n in nodes {
        if n.id == id {
            if !n.children.is_empty() {
                n.expanded = !n.expanded;
                return true;
            }
            return false;
        }
        if toggle_tree_table_expanded(&mut n.children, id) {
            return true;
        }
    }
    false
}
