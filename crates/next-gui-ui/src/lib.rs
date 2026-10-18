//! Compact declarative UI API for next-gui.
//!
//! Retained widget tree built with a short builder or loaded from a compact `.ngui` file,
//! synced into [`next_gui_scene::Scene`]. Text uses [`next_gui_text`] (font metrics + CJK).
//!
//! Office P0 + P1 widgets: shell/input/data/nav/feedback, TreeSelect/Form/AlarmList/Gauge/Trend,
//! plus Drawer, virtual list/table, TextArea, InputNumber, Notification, Spin, Empty, Collapse,
//! Descriptions, Card, Segmented, Breadcrumb, Pagination, DateTime, Transfer, PropertyGrid,
//! TreeTable, and multi-tile Chart. TextInput supports IME (CJK). Cascader stays deferred.
//! P6d a11y baseline: [`Ui::accessible_name`], [`Ui::focusable_widgets`] / Tab focus order
//! (Button included; [`Ui::take_button_activate`] for Space/Enter).

#![deny(missing_docs)]

mod dsl;
mod icon;
mod p1_deferred;
mod chart;
mod p1_raster;
mod p1_rest;

pub use dsl::{load_ngui, load_ngui_path, DslError, LoadedUi};
pub use icon::IconName;
pub use chart::{ChartKind, ChartStyle, ChartTile};
pub use p1_deferred::*;
pub use p1_rest::*;

use std::collections::HashMap;
use std::sync::Arc;

use next_gui_layout::{layout_flex, Axis, ChildMeasure, CrossAlign, FlexParams};
use next_gui_scene::{AlphaMask, Color, NodeId, Rect, Scene};
use next_gui_text::{load_default_cjk_font, Font};

use icon::rasterize_icon;

/// Stable widget identifier within a [`Ui`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetId(pub u64);

impl WidgetId {
    /// Raw id for debugging.
    pub fn raw(self) -> u64 {
        self.0
    }
}

/// Style for a column container.
#[derive(Debug, Clone, Copy)]
pub struct ColumnStyle {
    /// Outer x.
    pub x: f32,
    /// Outer y.
    pub y: f32,
    /// Max width available to the column.
    pub width: f32,
    /// Max height available.
    pub height: f32,
    /// Padding inside the column.
    pub padding: f32,
    /// Gap between children.
    pub gap: f32,
    /// Cross-axis alignment (horizontal for a column).
    pub cross_align: CrossAlign,
    /// Optional panel fill; `None` = host node stays invisible.
    pub background: Option<Color>,
}

impl Default for ColumnStyle {
    fn default() -> Self {
        Self {
            x: 24.0,
            y: 24.0,
            width: 400.0,
            height: 560.0,
            padding: 16.0,
            gap: 12.0,
            cross_align: CrossAlign::Start,
            background: Some(Color::rgb(0x2a, 0x2a, 0x2e)),
        }
    }
}

/// Style for a row container (toolbars, action strips).
#[derive(Debug, Clone, Copy)]
pub struct RowStyle {
    /// Outer x (ignored when nested; parent layout places the row).
    pub x: f32,
    /// Outer y (ignored when nested).
    pub y: f32,
    /// Max width available to the row.
    pub width: f32,
    /// Max height available.
    pub height: f32,
    /// Padding inside the row.
    pub padding: f32,
    /// Gap between children.
    pub gap: f32,
    /// Cross-axis alignment (vertical for a row).
    pub cross_align: CrossAlign,
    /// Optional strip fill.
    pub background: Option<Color>,
}

impl Default for RowStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 600.0,
            height: 48.0,
            padding: 8.0,
            gap: 8.0,
            cross_align: CrossAlign::Center,
            background: Some(Color::rgb(0x36, 0x36, 0x3a)),
        }
    }
}

/// Style for a first-class Toolbar strip.
#[derive(Debug, Clone, Copy)]
pub struct ToolbarStyle {
    /// Outer x (top-level only).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Width budget.
    pub width: f32,
    /// Height of the bar.
    pub height: f32,
    /// Inner padding.
    pub padding: f32,
    /// Gap between items.
    pub gap: f32,
    /// Vertical alignment of items.
    pub cross_align: CrossAlign,
    /// Bar fill.
    pub background: Option<Color>,
}

impl Default for ToolbarStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 40.0,
            padding: 6.0,
            gap: 6.0,
            cross_align: CrossAlign::Center,
            background: Some(Color::rgb(0x32, 0x32, 0x38)),
        }
    }
}

impl ToolbarStyle {
    fn to_row(self) -> RowStyle {
        RowStyle {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            padding: self.padding,
            gap: self.gap,
            cross_align: self.cross_align,
            background: self.background,
        }
    }
}

/// Style for a titled Panel.
#[derive(Debug, Clone, Copy)]
pub struct PanelStyle {
    /// Outer x (top-level only).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Outer width.
    pub width: f32,
    /// Outer height.
    pub height: f32,
    /// Content padding below the title bar.
    pub padding: f32,
    /// Gap between content children.
    pub gap: f32,
    /// Content cross-align.
    pub cross_align: CrossAlign,
    /// Panel body fill.
    pub background: Option<Color>,
    /// Title bar height.
    pub title_height: f32,
    /// Title bar fill.
    pub title_bg: Color,
    /// Title text color.
    pub title_fg: Color,
    /// Title font size (px).
    pub title_size: f32,
}

impl Default for PanelStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 320.0,
            height: 400.0,
            padding: 10.0,
            gap: 8.0,
            cross_align: CrossAlign::Start,
            background: Some(Color::rgb(0x28, 0x28, 0x2c)),
            title_height: 28.0,
            title_bg: Color::rgb(0x3a, 0x3a, 0x42),
            title_fg: Color::rgb(0xe8, 0xe8, 0xec),
            title_size: 14.0,
        }
    }
}

/// Style for a first-class StatusBar.
#[derive(Debug, Clone, Copy)]
pub struct StatusBarStyle {
    /// Outer x (top-level only).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Width budget.
    pub width: f32,
    /// Bar height.
    pub height: f32,
    /// Inner padding.
    pub padding: f32,
    /// Gap between sections.
    pub gap: f32,
    /// Vertical alignment.
    pub cross_align: CrossAlign,
    /// Bar fill.
    pub background: Option<Color>,
}

impl Default for StatusBarStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 26.0,
            padding: 6.0,
            gap: 12.0,
            cross_align: CrossAlign::Center,
            background: Some(Color::rgb(0x2c, 0x2c, 0x30)),
        }
    }
}

impl StatusBarStyle {
    fn to_row(self) -> RowStyle {
        RowStyle {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            padding: self.padding,
            gap: self.gap,
            cross_align: self.cross_align,
            background: self.background,
        }
    }
}

/// Style for a resizable Splitter.
#[derive(Debug, Clone, Copy)]
pub struct SplitterStyle {
    /// Outer x (top-level only).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Outer width.
    pub width: f32,
    /// Outer height.
    pub height: f32,
    /// Split axis: Vertical = left|right; Horizontal = top|bottom.
    pub axis: Axis,
    /// Fraction of space for the first pane (clamped on drag to ~0.1..0.9).
    pub ratio: f32,
    /// Sash thickness in px.
    pub sash_size: f32,
    /// Sash fill.
    pub sash_color: Color,
    /// Optional outer fill behind panes.
    pub background: Option<Color>,
}

impl Default for SplitterStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 480.0,
            axis: Axis::Vertical,
            ratio: 0.3,
            sash_size: 5.0,
            sash_color: Color::rgb(0x4a, 0x4a, 0x52),
            background: None,
        }
    }
}


/// Style for a ScrollArea viewport.
#[derive(Debug, Clone, Copy)]
pub struct ScrollAreaStyle {
    /// Outer x (top-level only).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Viewport width.
    pub width: f32,
    /// Viewport height.
    pub height: f32,
    /// Content padding.
    pub padding: f32,
    /// Gap between content children (vertical column).
    pub gap: f32,
    /// Content cross-align.
    pub cross_align: CrossAlign,
    /// Viewport fill.
    pub background: Option<Color>,
}

impl Default for ScrollAreaStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 280.0,
            height: 200.0,
            padding: 8.0,
            gap: 6.0,
            cross_align: CrossAlign::Start,
            background: Some(Color::rgb(0x24, 0x24, 0x28)),
        }
    }
}

/// Style for a single-line TextInput.
#[derive(Debug, Clone, Copy)]
pub struct TextInputStyle {
    /// Width of the field.
    pub width: f32,
    /// Height of the field.
    pub height: f32,
    /// Font size (px).
    pub size: f32,
    /// Text / caret color.
    pub fg: Color,
    /// Placeholder color.
    pub placeholder_fg: Color,
    /// Field background.
    pub bg: Color,
    /// Border / focus ring color.
    pub border: Color,
    /// Focused border color.
    pub focus_border: Color,
    /// Horizontal text padding.
    pub padding_x: f32,
}

impl Default for TextInputStyle {
    fn default() -> Self {
        Self {
            width: 220.0,
            height: 32.0,
            size: 14.0,
            fg: Color::rgb(0xe8, 0xec, 0xf0),
            placeholder_fg: Color::rgb(0x78, 0x80, 0x8c),
            bg: Color::rgb(0x1e, 0x1e, 0x22),
            border: Color::rgb(0x4a, 0x4a, 0x54),
            focus_border: Color::rgb(0x3b, 0x82, 0xf6),
            padding_x: 8.0,
        }
    }
}

/// Style for Checkbox.
#[derive(Debug, Clone, Copy)]
pub struct CheckboxStyle {
    /// Box size (px).
    pub box_size: f32,
    /// Label font size.
    pub size: f32,
    /// Label color.
    pub fg: Color,
    /// Box border / fill when unchecked.
    pub box_bg: Color,
    /// Box fill when checked.
    pub checked_bg: Color,
    /// Checkmark color.
    pub check_fg: Color,
    /// Gap between box and label.
    pub gap: f32,
}

impl Default for CheckboxStyle {
    fn default() -> Self {
        Self {
            box_size: 18.0,
            size: 14.0,
            fg: Color::rgb(0xe0, 0xe4, 0xec),
            box_bg: Color::rgb(0x2a, 0x2a, 0x30),
            checked_bg: Color::rgb(0x3b, 0x82, 0xf6),
            check_fg: Color::rgb(0xff, 0xff, 0xff),
            gap: 8.0,
        }
    }
}

/// Style for Radio.
#[derive(Debug, Clone, Copy)]
pub struct RadioStyle {
    /// Outer circle diameter.
    pub diameter: f32,
    /// Label font size.
    pub size: f32,
    /// Label color.
    pub fg: Color,
    /// Outer ring color.
    pub ring: Color,
    /// Selected dot color.
    pub dot: Color,
    /// Gap between control and label.
    pub gap: f32,
}

impl Default for RadioStyle {
    fn default() -> Self {
        Self {
            diameter: 18.0,
            size: 14.0,
            fg: Color::rgb(0xe0, 0xe4, 0xec),
            ring: Color::rgb(0x6a, 0x70, 0x7c),
            dot: Color::rgb(0x3b, 0x82, 0xf6),
            gap: 8.0,
        }
    }
}

/// Style for Switch (on/off toggle).
#[derive(Debug, Clone, Copy)]
pub struct SwitchStyle {
    /// Track width.
    pub width: f32,
    /// Track height.
    pub height: f32,
    /// Label font size.
    pub size: f32,
    /// Label color.
    pub fg: Color,
    /// Track when off.
    pub track_off: Color,
    /// Track when on.
    pub track_on: Color,
    /// Thumb color.
    pub thumb: Color,
    /// Gap between switch and label.
    pub gap: f32,
}

impl Default for SwitchStyle {
    fn default() -> Self {
        Self {
            width: 40.0,
            height: 22.0,
            size: 14.0,
            fg: Color::rgb(0xe0, 0xe4, 0xec),
            track_off: Color::rgb(0x3a, 0x3a, 0x42),
            track_on: Color::rgb(0x22, 0xc5, 0x5e),
            thumb: Color::rgb(0xf0, 0xf0, 0xf4),
            gap: 8.0,
        }
    }
}


/// Style for a selectable List.
#[derive(Debug, Clone, Copy)]
pub struct ListStyle {
    /// Outer x (top-level only).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Viewport width.
    pub width: f32,
    /// Viewport height.
    pub height: f32,
    /// Row height.
    pub item_height: f32,
    /// Inner padding.
    pub padding: f32,
    /// Font size (px).
    pub size: f32,
    /// Item text color.
    pub fg: Color,
    /// Selected item text color.
    pub selected_fg: Color,
    /// Viewport background.
    pub bg: Color,
    /// Selected row background.
    pub selected_bg: Color,
    /// Focus ring / border hint.
    pub focus_border: Color,
}

impl Default for ListStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 220.0,
            height: 200.0,
            item_height: 26.0,
            padding: 4.0,
            size: 13.0,
            fg: Color::rgb(0xd8, 0xdc, 0xe4),
            selected_fg: Color::rgb(0xff, 0xff, 0xff),
            bg: Color::rgb(0x22, 0x22, 0x28),
            selected_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            focus_border: Color::rgb(0x3b, 0x82, 0xf6),
        }
    }
}

/// One node in a [`Kind::Tree`] / builder tree.
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Stable id used for selection and expand APIs.
    pub id: u64,
    /// Display label.
    pub label: String,
    /// Child nodes.
    pub children: Vec<TreeNode>,
    /// Whether children are shown.
    pub expanded: bool,
}

impl TreeNode {
    /// Leaf or branch with `expanded` default true when children exist.
    pub fn new(id: u64, label: impl Into<String>, children: Vec<TreeNode>) -> Self {
        let expanded = !children.is_empty();
        Self {
            id,
            label: label.into(),
            children,
            expanded,
        }
    }

    /// Leaf node.
    pub fn leaf(id: u64, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            children: Vec::new(),
            expanded: false,
        }
    }
}

/// Style for a Tree.
#[derive(Debug, Clone, Copy)]
pub struct TreeStyle {
    /// Outer x (top-level only).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Viewport width.
    pub width: f32,
    /// Viewport height.
    pub height: f32,
    /// Row height.
    pub row_height: f32,
    /// Indent per depth level.
    pub indent: f32,
    /// Inner padding.
    pub padding: f32,
    /// Font size (px).
    pub size: f32,
    /// Label color.
    pub fg: Color,
    /// Selected label color.
    pub selected_fg: Color,
    /// Viewport background.
    pub bg: Color,
    /// Selected row background.
    pub selected_bg: Color,
    /// Focus border hint.
    pub focus_border: Color,
}

impl Default for TreeStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 240.0,
            height: 260.0,
            row_height: 26.0,
            indent: 14.0,
            padding: 4.0,
            size: 13.0,
            fg: Color::rgb(0xd8, 0xdc, 0xe4),
            selected_fg: Color::rgb(0xff, 0xff, 0xff),
            bg: Color::rgb(0x22, 0x22, 0x28),
            selected_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            focus_border: Color::rgb(0x3b, 0x82, 0xf6),
        }
    }
}

/// Column definition for a Table.
#[derive(Debug, Clone)]
pub struct TableColumn {
    /// Header title.
    pub title: String,
    /// Column width (px). Resize is stubbed — width is fixed.
    pub width: f32,
}

impl TableColumn {
    /// Column with title and width.
    pub fn new(title: impl Into<String>, width: f32) -> Self {
        Self {
            title: title.into(),
            width,
        }
    }
}

/// Style for a basic Table.
#[derive(Debug, Clone, Copy)]
pub struct TableStyle {
    /// Outer x (top-level only).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Viewport width.
    pub width: f32,
    /// Viewport height (includes header).
    pub height: f32,
    /// Header row height.
    pub header_height: f32,
    /// Body row height.
    pub row_height: f32,
    /// Inner padding.
    pub padding: f32,
    /// Font size (px).
    pub size: f32,
    /// Body text color.
    pub fg: Color,
    /// Header text color.
    pub header_fg: Color,
    /// Selected body text color.
    pub selected_fg: Color,
    /// Viewport / body background.
    pub bg: Color,
    /// Header background.
    pub header_bg: Color,
    /// Selected row background.
    pub selected_bg: Color,
    /// Grid / separator hint.
    pub grid: Color,
    /// Focus border hint.
    pub focus_border: Color,
}

impl Default for TableStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 420.0,
            height: 240.0,
            header_height: 28.0,
            row_height: 26.0,
            padding: 4.0,
            size: 13.0,
            fg: Color::rgb(0xd8, 0xdc, 0xe4),
            header_fg: Color::rgb(0xc0, 0xc8, 0xd4),
            selected_fg: Color::rgb(0xff, 0xff, 0xff),
            bg: Color::rgb(0x22, 0x22, 0x28),
            header_bg: Color::rgb(0x32, 0x32, 0x3a),
            selected_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            grid: Color::rgb(0x3a, 0x3a, 0x44),
            focus_border: Color::rgb(0x3b, 0x82, 0xf6),
        }
    }
}

/// One actionable entry in a [`Kind::Menu`] / [`Kind::ContextMenu`].
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// Stable id returned by [`Ui::take_menu_action`].
    pub id: u64,
    /// Display label.
    pub label: String,
    /// When false, item is drawn muted and not activatable.
    pub enabled: bool,
}

impl MenuItem {
    /// Enabled menu item.
    pub fn new(id: u64, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            enabled: true,
        }
    }

    /// Disabled menu item.
    pub fn disabled(id: u64, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            enabled: false,
        }
    }
}

/// Style for a dropdown Menu (title button + popup list).
#[derive(Debug, Clone, Copy)]
pub struct MenuStyle {
    /// Title button min width.
    pub min_width: f32,
    /// Title / item height.
    pub item_height: f32,
    /// Popup min width (falls back to title width).
    pub popup_width: f32,
    /// Font size (px).
    pub size: f32,
    /// Title / item text.
    pub fg: Color,
    /// Disabled item text.
    pub disabled_fg: Color,
    /// Title button background.
    pub title_bg: Color,
    /// Open title / hover row background.
    pub open_bg: Color,
    /// Popup panel background.
    pub popup_bg: Color,
    /// Selected / hover row background.
    pub hover_bg: Color,
}

impl Default for MenuStyle {
    fn default() -> Self {
        Self {
            min_width: 72.0,
            item_height: 28.0,
            popup_width: 160.0,
            size: 13.0,
            fg: Color::rgb(0xe0, 0xe4, 0xec),
            disabled_fg: Color::rgb(0x70, 0x74, 0x80),
            title_bg: Color::rgb(0x36, 0x36, 0x3e),
            open_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            popup_bg: Color::rgb(0x2a, 0x2a, 0x32),
            hover_bg: Color::rgb(0x2f, 0x5b, 0xa8),
        }
    }
}

/// Style for a ContextMenu host region + popup.
#[derive(Debug, Clone, Copy)]
pub struct ContextMenuStyle {
    /// Host width.
    pub width: f32,
    /// Host height.
    pub height: f32,
    /// Popup width.
    pub popup_width: f32,
    /// Item row height.
    pub item_height: f32,
    /// Font size (px).
    pub size: f32,
    /// Host fill.
    pub host_bg: Color,
    /// Host hint text color.
    pub hint_fg: Color,
    /// Popup panel background.
    pub popup_bg: Color,
    /// Item text.
    pub fg: Color,
    /// Disabled item text.
    pub disabled_fg: Color,
    /// Hover row background.
    pub hover_bg: Color,
}

impl Default for ContextMenuStyle {
    fn default() -> Self {
        Self {
            width: 280.0,
            height: 120.0,
            popup_width: 180.0,
            item_height: 28.0,
            size: 13.0,
            host_bg: Color::rgb(0x26, 0x26, 0x2c),
            hint_fg: Color::rgb(0xa0, 0xa8, 0xb4),
            popup_bg: Color::rgb(0x2a, 0x2a, 0x32),
            fg: Color::rgb(0xe0, 0xe4, 0xec),
            disabled_fg: Color::rgb(0x70, 0x74, 0x80),
            hover_bg: Color::rgb(0x2f, 0x5b, 0xa8),
        }
    }
}

/// Style for Tabs (strip + content pane).
#[derive(Debug, Clone, Copy)]
pub struct TabsStyle {
    /// Outer width.
    pub width: f32,
    /// Outer height (strip + content).
    pub height: f32,
    /// Tab strip height.
    pub strip_height: f32,
    /// Content padding.
    pub padding: f32,
    /// Gap between content children.
    pub gap: f32,
    /// Content cross-align.
    pub cross_align: CrossAlign,
    /// Font size (px).
    pub size: f32,
    /// Inactive tab text.
    pub fg: Color,
    /// Active tab text.
    pub active_fg: Color,
    /// Strip / inactive tab background.
    pub strip_bg: Color,
    /// Active tab background.
    pub active_bg: Color,
    /// Content pane background.
    pub content_bg: Color,
}

impl Default for TabsStyle {
    fn default() -> Self {
        Self {
            width: 400.0,
            height: 240.0,
            strip_height: 32.0,
            padding: 10.0,
            gap: 8.0,
            cross_align: CrossAlign::Start,
            size: 13.0,
            fg: Color::rgb(0xa8, 0xb0, 0xbc),
            active_fg: Color::rgb(0xff, 0xff, 0xff),
            strip_bg: Color::rgb(0x2c, 0x2c, 0x34),
            active_bg: Color::rgb(0x3a, 0x3a, 0x48),
            content_bg: Color::rgb(0x24, 0x24, 0x2a),
        }
    }
}

/// Style for a Select (combobox-lite).
#[derive(Debug, Clone, Copy)]
pub struct SelectStyle {
    /// Closed field width.
    pub width: f32,
    /// Closed field height.
    pub height: f32,
    /// Dropdown max height.
    pub popup_max_height: f32,
    /// Option row height.
    pub item_height: f32,
    /// Inner padding.
    pub padding: f32,
    /// Font size (px).
    pub size: f32,
    /// Value / option text.
    pub fg: Color,
    /// Placeholder when nothing selected.
    pub placeholder_fg: Color,
    /// Closed field background.
    pub bg: Color,
    /// Popup background.
    pub popup_bg: Color,
    /// Selected / hover row background.
    pub selected_bg: Color,
    /// Focus border hint.
    pub focus_border: Color,
}

impl Default for SelectStyle {
    fn default() -> Self {
        Self {
            width: 200.0,
            height: 30.0,
            popup_max_height: 160.0,
            item_height: 26.0,
            padding: 8.0,
            size: 13.0,
            fg: Color::rgb(0xe0, 0xe4, 0xec),
            placeholder_fg: Color::rgb(0x70, 0x74, 0x80),
            bg: Color::rgb(0x2a, 0x2a, 0x32),
            popup_bg: Color::rgb(0x2a, 0x2a, 0x32),
            selected_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            focus_border: Color::rgb(0x3b, 0x82, 0xf6),
        }
    }
}

/// Style for a horizontal Slider.
#[derive(Debug, Clone, Copy)]
pub struct SliderStyle {
    /// Track width.
    pub width: f32,
    /// Control height (thumb diameter budget).
    pub height: f32,
    /// Track thickness.
    pub track_thickness: f32,
    /// Thumb diameter.
    pub thumb_size: f32,
    /// Track background.
    pub track: Color,
    /// Filled portion of track.
    pub fill: Color,
    /// Thumb fill.
    pub thumb: Color,
    /// Focus ring hint.
    pub focus_border: Color,
}

impl Default for SliderStyle {
    fn default() -> Self {
        Self {
            width: 220.0,
            height: 28.0,
            track_thickness: 6.0,
            thumb_size: 16.0,
            track: Color::rgb(0x3a, 0x3a, 0x44),
            fill: Color::rgb(0x3b, 0x82, 0xf6),
            thumb: Color::rgb(0xe8, 0xec, 0xf4),
            focus_border: Color::rgb(0x3b, 0x82, 0xf6),
        }
    }
}

/// Style for a compact Tag chip.
#[derive(Debug, Clone, Copy)]
pub struct TagStyle {
    /// Chip height.
    pub height: f32,
    /// Horizontal padding.
    pub padding_x: f32,
    /// Font size (px).
    pub size: f32,
    /// Label color.
    pub fg: Color,
    /// Chip background.
    pub bg: Color,
    /// Close affordance color (when closable).
    pub close_fg: Color,
}

impl Default for TagStyle {
    fn default() -> Self {
        Self {
            height: 24.0,
            padding_x: 8.0,
            size: 12.0,
            fg: Color::rgb(0xe0, 0xe4, 0xec),
            bg: Color::rgb(0x3a, 0x3a, 0x48),
            close_fg: Color::rgb(0xa0, 0xa8, 0xb4),
        }
    }
}

/// Badge presentation: dot or numeric count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeKind {
    /// Small status dot.
    Dot,
    /// Numeric count (0 hides the badge chrome when wrapping a host).
    Count(u32),
}

/// Style for Badge (standalone or host overlay).
#[derive(Debug, Clone, Copy)]
pub struct BadgeStyle {
    /// Dot diameter.
    pub dot_size: f32,
    /// Count pill min height.
    pub height: f32,
    /// Horizontal padding for count pill.
    pub padding_x: f32,
    /// Font size for count.
    pub size: f32,
    /// Count text.
    pub fg: Color,
    /// Pill / dot fill.
    pub bg: Color,
    /// Offset from host top-right when wrapping.
    pub offset: f32,
}

impl Default for BadgeStyle {
    fn default() -> Self {
        Self {
            dot_size: 8.0,
            height: 18.0,
            padding_x: 6.0,
            size: 11.0,
            fg: Color::rgb(0xff, 0xff, 0xff),
            bg: Color::rgb(0xe0, 0x4f, 0x5f),
            offset: 4.0,
        }
    }
}

/// Style for Tooltip popup text.
#[derive(Debug, Clone, Copy)]
pub struct TooltipStyle {
    /// Font size (px).
    pub size: f32,
    /// Padding inside tip.
    pub padding: f32,
    /// Max tip width (text not wrapped in MVP; used as measure bias).
    pub max_width: f32,
    /// Tip text color.
    pub fg: Color,
    /// Tip background.
    pub bg: Color,
    /// Gap between host bottom and tip.
    pub gap: f32,
}

impl Default for TooltipStyle {
    fn default() -> Self {
        Self {
            size: 12.0,
            padding: 6.0,
            max_width: 280.0,
            fg: Color::rgb(0xf0, 0xf2, 0xf6),
            bg: Color::rgb(0x2a, 0x2a, 0x34),
            gap: 6.0,
        }
    }
}

/// Style for a Modal dialog.
#[derive(Debug, Clone, Copy)]
pub struct ModalStyle {
    /// Dialog panel width.
    pub width: f32,
    /// Dialog panel min height.
    pub height: f32,
    /// Title bar height.
    pub title_height: f32,
    /// Content padding.
    pub padding: f32,
    /// Title / body font sizes.
    pub title_size: f32,
    /// Body font size.
    pub body_size: f32,
    /// Action button height.
    pub action_height: f32,
    /// Title text.
    pub title_fg: Color,
    /// Body text.
    pub body_fg: Color,
    /// Panel background.
    pub panel_bg: Color,
    /// Title bar background.
    pub title_bg: Color,
    /// Dim / scrim fill (opaque dark for Soft MVP).
    pub scrim: Color,
    /// Primary action button background.
    pub primary_bg: Color,
    /// Secondary action button background.
    pub secondary_bg: Color,
    /// Action label color.
    pub action_fg: Color,
    /// Close on backdrop click.
    pub close_on_backdrop: bool,
}

impl Default for ModalStyle {
    fn default() -> Self {
        Self {
            width: 420.0,
            height: 200.0,
            title_height: 36.0,
            padding: 16.0,
            title_size: 15.0,
            body_size: 13.0,
            action_height: 32.0,
            title_fg: Color::rgb(0xf0, 0xf2, 0xf6),
            body_fg: Color::rgb(0xc0, 0xc8, 0xd0),
            panel_bg: Color::rgb(0x2a, 0x2a, 0x32),
            title_bg: Color::rgb(0x32, 0x32, 0x3c),
            scrim: Color::rgb(0x0c, 0x0c, 0x10),
            primary_bg: Color::rgb(0x3b, 0x82, 0xf6),
            secondary_bg: Color::rgb(0x3a, 0x3a, 0x48),
            action_fg: Color::rgb(0xff, 0xff, 0xff),
            close_on_backdrop: true,
        }
    }
}

/// Result of a Modal dismiss / action; drained by [`Ui::take_modal_action`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalAction {
    /// Primary button (e.g. OK / Delete).
    Primary,
    /// Secondary button (e.g. Cancel).
    Secondary,
    /// Escape, title close, or backdrop.
    Dismiss,
}

/// Style for a determinate Progress bar.
#[derive(Debug, Clone, Copy)]
pub struct ProgressStyle {
    /// Track width.
    pub width: f32,
    /// Track height.
    pub height: f32,
    /// Track background.
    pub track: Color,
    /// Filled portion.
    pub fill: Color,
}

impl Default for ProgressStyle {
    fn default() -> Self {
        Self {
            width: 240.0,
            height: 10.0,
            track: Color::rgb(0x3a, 0x3a, 0x44),
            fill: Color::rgb(0x3b, 0x82, 0xf6),
        }
    }
}

/// Alert severity / tone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertTone {
    /// Informational.
    Info,
    /// Success / OK.
    Success,
    /// Warning.
    Warning,
    /// Error / fault.
    Error,
}

/// Style for an inline Alert banner.
#[derive(Debug, Clone, Copy)]
pub struct AlertStyle {
    /// Banner width.
    pub width: f32,
    /// Min height.
    pub height: f32,
    /// Padding.
    pub padding: f32,
    /// Font size.
    pub size: f32,
    /// Message color.
    pub fg: Color,
    /// Info background.
    pub info_bg: Color,
    /// Success background.
    pub success_bg: Color,
    /// Warning background.
    pub warning_bg: Color,
    /// Error background.
    pub error_bg: Color,
    /// Dismiss affordance color.
    pub close_fg: Color,
}

impl Default for AlertStyle {
    fn default() -> Self {
        Self {
            width: 480.0,
            height: 36.0,
            padding: 10.0,
            size: 13.0,
            fg: Color::rgb(0xf0, 0xf2, 0xf6),
            info_bg: Color::rgb(0x2a, 0x3a, 0x55),
            success_bg: Color::rgb(0x1e, 0x3d, 0x2a),
            warning_bg: Color::rgb(0x4a, 0x3a, 0x18),
            error_bg: Color::rgb(0x4a, 0x22, 0x28),
            close_fg: Color::rgb(0xc0, 0xc8, 0xd0),
        }
    }
}

/// Validation rule for a Form field (TextInput values).
#[derive(Debug, Clone)]
pub enum FormRule {
    /// Value must be non-empty after trim.
    Required {
        /// Error message when violated.
        message: String,
    },
    /// Minimum character count (Unicode scalars).
    MinLength {
        /// Minimum length.
        min: usize,
        /// Error message when violated.
        message: String,
    },
    /// Maximum character count (Unicode scalars).
    MaxLength {
        /// Maximum length.
        max: usize,
        /// Error message when violated.
        message: String,
    },
    /// Every character must be an ASCII digit.
    Digits {
        /// Error message when violated.
        message: String,
    },
}

impl FormRule {
    /// Required field.
    pub fn required(message: impl Into<String>) -> Self {
        Self::Required {
            message: message.into(),
        }
    }

    /// Minimum length.
    pub fn min_length(min: usize, message: impl Into<String>) -> Self {
        Self::MinLength {
            min,
            message: message.into(),
        }
    }

    /// Maximum length.
    pub fn max_length(max: usize, message: impl Into<String>) -> Self {
        Self::MaxLength {
            max,
            message: message.into(),
        }
    }

    /// ASCII digits only.
    pub fn digits(message: impl Into<String>) -> Self {
        Self::Digits {
            message: message.into(),
        }
    }

    fn check(&self, value: &str) -> Option<&str> {
        let trimmed = value.trim();
        match self {
            FormRule::Required { message } => {
                if trimmed.is_empty() {
                    Some(message.as_str())
                } else {
                    None
                }
            }
            FormRule::MinLength { min, message } => {
                if trimmed.chars().count() < *min {
                    Some(message.as_str())
                } else {
                    None
                }
            }
            FormRule::MaxLength { max, message } => {
                if trimmed.chars().count() > *max {
                    Some(message.as_str())
                } else {
                    None
                }
            }
            FormRule::Digits { message } => {
                if trimmed.is_empty() || trimmed.bytes().all(|b| b.is_ascii_digit()) {
                    None
                } else {
                    Some(message.as_str())
                }
            }
        }
    }
}

/// One field binding inside a Form (control is typically a TextInput).
#[derive(Debug, Clone)]
pub struct FormFieldState {
    /// Stable key for [`Ui::form_field_error`].
    pub key: String,
    /// Bound control widget (usually TextInput).
    pub control: WidgetId,
    /// Error label Text widget under the control.
    pub error_label: WidgetId,
    /// Rules evaluated by [`Ui::validate_form`].
    pub rules: Vec<FormRule>,
}

/// Style for a Form container (vertical field stack).
#[derive(Debug, Clone, Copy)]
pub struct FormStyle {
    /// Outer x (top-level only).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Width budget.
    pub width: f32,
    /// Height budget.
    pub height: f32,
    /// Padding.
    pub padding: f32,
    /// Gap between field rows.
    pub gap: f32,
    /// Cross-axis alignment.
    pub cross_align: CrossAlign,
    /// Optional background.
    pub background: Option<Color>,
    /// Label text size.
    pub label_size: f32,
    /// Label color.
    pub label_fg: Color,
    /// Error text size.
    pub error_size: f32,
    /// Error text color.
    pub error_fg: Color,
}

impl Default for FormStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 320.0,
            height: 280.0,
            padding: 8.0,
            gap: 6.0,
            cross_align: CrossAlign::Start,
            background: Some(Color::rgb(0x28, 0x28, 0x30)),
            label_size: 12.0,
            label_fg: Color::rgb(0xb0, 0xb8, 0xc4),
            error_size: 11.0,
            error_fg: Color::rgb(0xf0, 0x70, 0x70),
        }
    }
}

/// Style for a TreeSelect (closed field + tree popup).
#[derive(Debug, Clone, Copy)]
pub struct TreeSelectStyle {
    /// Closed field width.
    pub width: f32,
    /// Closed field height.
    pub height: f32,
    /// Popup max height.
    pub popup_max_height: f32,
    /// Row height inside popup tree.
    pub row_height: f32,
    /// Indent per depth.
    pub indent: f32,
    /// Inner padding.
    pub padding: f32,
    /// Font size.
    pub size: f32,
    /// Value text.
    pub fg: Color,
    /// Placeholder.
    pub placeholder_fg: Color,
    /// Closed field background.
    pub bg: Color,
    /// Popup background.
    pub popup_bg: Color,
    /// Selected row background.
    pub selected_bg: Color,
    /// Focus border hint.
    pub focus_border: Color,
}

impl Default for TreeSelectStyle {
    fn default() -> Self {
        Self {
            width: 220.0,
            height: 30.0,
            popup_max_height: 200.0,
            row_height: 26.0,
            indent: 14.0,
            padding: 8.0,
            size: 13.0,
            fg: Color::rgb(0xe0, 0xe4, 0xec),
            placeholder_fg: Color::rgb(0x70, 0x74, 0x80),
            bg: Color::rgb(0x2a, 0x2a, 0x32),
            popup_bg: Color::rgb(0x2a, 0x2a, 0x32),
            selected_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            focus_border: Color::rgb(0x3b, 0x82, 0xf6),
        }
    }
}

/// Alarm severity for [`AlarmEntry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlarmSeverity {
    /// Highest — flash when unacked.
    Critical,
    /// High — flash when unacked.
    Major,
    /// Medium.
    Minor,
    /// Low warning.
    Warning,
    /// Informational.
    Info,
}

impl AlarmSeverity {
    fn label(self) -> &'static str {
        match self {
            AlarmSeverity::Critical => "CRIT",
            AlarmSeverity::Major => "MAJ",
            AlarmSeverity::Minor => "MIN",
            AlarmSeverity::Warning => "WARN",
            AlarmSeverity::Info => "INFO",
        }
    }

    fn color(self) -> Color {
        match self {
            AlarmSeverity::Critical => Color::rgb(0xe0, 0x3a, 0x4a),
            AlarmSeverity::Major => Color::rgb(0xf0, 0x8a, 0x2a),
            AlarmSeverity::Minor => Color::rgb(0xe0, 0xc0, 0x40),
            AlarmSeverity::Warning => Color::rgb(0x80, 0xb0, 0xe0),
            AlarmSeverity::Info => Color::rgb(0x90, 0x98, 0xa8),
        }
    }

    fn flashes(self) -> bool {
        matches!(self, AlarmSeverity::Critical | AlarmSeverity::Major)
    }
}

/// One alarm row.
#[derive(Debug, Clone)]
pub struct AlarmEntry {
    /// Stable id.
    pub id: u64,
    /// Severity.
    pub severity: AlarmSeverity,
    /// Message text.
    pub message: String,
    /// Source / point name.
    pub source: String,
    /// Timestamp display string.
    pub time: String,
    /// Acknowledged.
    pub acked: bool,
}

impl AlarmEntry {
    /// Build an alarm entry.
    pub fn new(
        id: u64,
        severity: AlarmSeverity,
        message: impl Into<String>,
        source: impl Into<String>,
        time: impl Into<String>,
    ) -> Self {
        Self {
            id,
            severity,
            message: message.into(),
            source: source.into(),
            time: time.into(),
            acked: false,
        }
    }
}

/// Style for an AlarmList.
#[derive(Debug, Clone, Copy)]
pub struct AlarmListStyle {
    /// Viewport width.
    pub width: f32,
    /// Viewport height.
    pub height: f32,
    /// Row height.
    pub row_height: f32,
    /// Inner padding.
    pub padding: f32,
    /// Font size.
    pub size: f32,
    /// Body text.
    pub fg: Color,
    /// Viewport background.
    pub bg: Color,
    /// Selected row background.
    pub selected_bg: Color,
    /// Unacked flash background (even phase).
    pub flash_bg: Color,
    /// Focus border hint.
    pub focus_border: Color,
}

impl Default for AlarmListStyle {
    fn default() -> Self {
        Self {
            width: 420.0,
            height: 200.0,
            row_height: 28.0,
            padding: 4.0,
            size: 12.0,
            fg: Color::rgb(0xe0, 0xe4, 0xec),
            bg: Color::rgb(0x1e, 0x1e, 0x24),
            selected_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            flash_bg: Color::rgb(0x5a, 0x22, 0x28),
            focus_border: Color::rgb(0x3b, 0x82, 0xf6),
        }
    }
}

/// Style for a Gauge dial.
#[derive(Debug, Clone, Copy)]
pub struct GaugeStyle {
    /// Dial size (square).
    pub size: f32,
    /// Track / fill tint (mask tint).
    pub dial_fg: Color,
    /// Value / unit text color.
    pub text_fg: Color,
    /// Value font size.
    pub value_size: f32,
    /// Label font size.
    pub label_size: f32,
}

impl Default for GaugeStyle {
    fn default() -> Self {
        Self {
            size: 120.0,
            dial_fg: Color::rgb(0x3b, 0xc4, 0x8c),
            text_fg: Color::rgb(0xe8, 0xec, 0xf4),
            value_size: 16.0,
            label_size: 11.0,
        }
    }
}

/// Style for a Trend / sparkline.
#[derive(Debug, Clone, Copy)]
pub struct TrendStyle {
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
    /// Line tint.
    pub line: Color,
    /// Soft under-fill tint.
    pub fill: Color,
    /// Background.
    pub bg: Color,
}

impl Default for TrendStyle {
    fn default() -> Self {
        Self {
            width: 220.0,
            height: 64.0,
            line: Color::rgb(0x4a, 0xc0, 0xf0),
            fill: Color::rgb(0x1a, 0x40, 0x55),
            bg: Color::rgb(0x22, 0x22, 0x28),
        }
    }
}

#[derive(Debug)]
enum Kind {
    Column {
        style: ColumnStyle,
        children: Vec<WidgetId>,
    },
    Row {
        style: RowStyle,
        children: Vec<WidgetId>,
    },
    Toolbar {
        style: ToolbarStyle,
        children: Vec<WidgetId>,
    },
    Panel {
        title: String,
        style: PanelStyle,
        children: Vec<WidgetId>,
    },
    StatusBar {
        style: StatusBarStyle,
        children: Vec<WidgetId>,
    },
    Splitter {
        style: SplitterStyle,
        children: Vec<WidgetId>,
    },
    Text {
        content: String,
        color: Color,
        size: f32,
    },
    Button {
        label: String,
        bg: Color,
        fg: Color,
        size: f32,
        min_width: f32,
        height: f32,
    },
    Icon {
        name: IconName,
        size: f32,
        color: Color,
    },
    Spacer {
        width: f32,
        height: f32,
    },
    Separator {
        thickness: f32,
        length: f32,
        horizontal: bool,
        color: Color,
    },
    ScrollArea {
        style: ScrollAreaStyle,
        children: Vec<WidgetId>,
        scroll_x: f32,
        scroll_y: f32,
        content_w: f32,
        content_h: f32,
    },
    TextInput {
        value: String,
        placeholder: String,
        caret: usize,
        select_all: bool,
        /// IME preedit / composition string (not yet committed into `value`).
        preedit: String,
        /// Byte-range caret inside `preedit` from the IME (`None` = hide caret in preedit).
        preedit_cursor: Option<(usize, usize)>,
        style: TextInputStyle,
    },
    Checkbox {
        label: String,
        checked: bool,
        style: CheckboxStyle,
    },
    Radio {
        group: u64,
        label: String,
        selected: bool,
        style: RadioStyle,
    },
    Switch {
        label: String,
        on: bool,
        style: SwitchStyle,
    },
    List {
        items: Vec<String>,
        selected: Option<usize>,
        scroll_y: f32,
        /// When true, only a visible window of scene aux nodes is allocated.
        virtualized: bool,
        style: ListStyle,
    },
    Tree {
        roots: Vec<TreeNode>,
        selected: Option<u64>,
        scroll_y: f32,
        style: TreeStyle,
    },
    Table {
        columns: Vec<TableColumn>,
        rows: Vec<Vec<String>>,
        selected_row: Option<usize>,
        scroll_y: f32,
        /// When true, only a visible window of body rows is materialized in scene aux.
        virtualized: bool,
        style: TableStyle,
    },
    Menu {
        title: String,
        items: Vec<MenuItem>,
        open: bool,
        style: MenuStyle,
    },
    ContextMenu {
        hint: String,
        items: Vec<MenuItem>,
        open: bool,
        /// Popup anchor in window coords (set on open).
        anchor_x: f32,
        anchor_y: f32,
        style: ContextMenuStyle,
        children: Vec<WidgetId>,
    },
    Tabs {
        titles: Vec<String>,
        selected: usize,
        style: TabsStyle,
        children: Vec<WidgetId>,
    },
    Select {
        options: Vec<String>,
        selected: Option<usize>,
        open: bool,
        placeholder: String,
        style: SelectStyle,
    },
    Slider {
        min: f32,
        max: f32,
        value: f32,
        style: SliderStyle,
    },
    Tag {
        label: String,
        closable: bool,
        style: TagStyle,
    },
    Badge {
        kind: BadgeKind,
        style: BadgeStyle,
        children: Vec<WidgetId>,
    },
    Tooltip {
        text: String,
        style: TooltipStyle,
        children: Vec<WidgetId>,
    },
    Modal {
        title: String,
        body: String,
        primary: Option<String>,
        secondary: Option<String>,
        open: bool,
        style: ModalStyle,
    },
    Progress {
        /// 0.0..=1.0 when determinate.
        value: f32,
        /// Reserved; painted as muted track only in MVP.
        indeterminate: bool,
        style: ProgressStyle,
    },
    Alert {
        tone: AlertTone,
        message: String,
        dismissible: bool,
        dismissed: bool,
        style: AlertStyle,
    },
    Form {
        style: FormStyle,
        fields: Vec<FormFieldState>,
        children: Vec<WidgetId>,
    },
    TreeSelect {
        roots: Vec<TreeNode>,
        selected: Option<u64>,
        open: bool,
        scroll_y: f32,
        placeholder: String,
        style: TreeSelectStyle,
    },
    AlarmList {
        entries: Vec<AlarmEntry>,
        selected: Option<usize>,
        scroll_y: f32,
        /// App-driven flash phase for unacked Critical/Major rows.
        flash_on: bool,
        style: AlarmListStyle,
    },
    Gauge {
        value: f32,
        min: f32,
        max: f32,
        unit: String,
        label: String,
        style: GaugeStyle,
    },
    Trend {
        points: Vec<f32>,
        style: TrendStyle,
    },
    Chart {
        tiles: Vec<ChartTile>,
        style: ChartStyle,
    },
    Drawer {
        title: String,
        body: String,
        open: bool,
        style: DrawerStyle,
    },
    TextArea {
        value: String,
        placeholder: String,
        caret: usize,
        scroll_y: f32,
        style: TextAreaStyle,
    },
    InputNumber {
        value: f64,
        min: f64,
        max: f64,
        step: f64,
        style: InputNumberStyle,
    },
    Spin {
        spinning: bool,
        style: SpinStyle,
        children: Vec<WidgetId>,
    },
    Empty {
        description: String,
        style: EmptyStyle,
    },
    Collapse {
        panels: Vec<CollapsePanel>,
        style: CollapseStyle,
    },
    Descriptions {
        items: Vec<DescriptionItem>,
        style: DescriptionsStyle,
    },
    Card {
        title: String,
        body: String,
        style: CardStyle,
    },
    Segmented {
        options: Vec<String>,
        selected: usize,
        style: SegmentedStyle,
    },
    Breadcrumb {
        items: Vec<String>,
        style: BreadcrumbStyle,
    },
    Pagination {
        page: usize,
        page_size: usize,
        total: usize,
        style: PaginationStyle,
    },
    DateTime {
        value: DateTimeValue,
        /// Draft while popup open (committed on Confirm/Enter).
        draft: DateTimeValue,
        open: bool,
        style: DateTimeStyle,
    },
    Transfer {
        source: Vec<TransferItem>,
        target: Vec<TransferItem>,
        source_selected: Option<usize>,
        target_selected: Option<usize>,
        /// Optional filter substring (source list); empty = show all.
        filter: String,
        style: TransferStyle,
    },
    PropertyGrid {
        rows: Vec<PropertyRow>,
        selected: Option<usize>,
        scroll_y: f32,
        style: PropertyGridStyle,
    },
    TreeTable {
        /// Extra column titles (after the tree column).
        columns: Vec<TableColumn>,
        roots: Vec<TreeTableNode>,
        selected: Option<u64>,
        scroll_y: f32,
        style: TreeTableStyle,
    },
}

#[derive(Debug)]
struct Widget {
    kind: Kind,
    /// Primary scene node (container / hit target / text mask host).
    node: Option<NodeId>,
    /// Label mask node for buttons / panel titles.
    label_node: Option<NodeId>,
    /// Extra chrome: panel title bar bg, splitter sash, data selection / header.
    chrome_node: Option<NodeId>,
    /// Row/cell nodes for List / Tree / Table (bg + text pairs, etc.).
    aux: Vec<NodeId>,
    /// Optional accessible name override (P6d). When `None`, [`Ui::accessible_name`]
    /// falls back to kind-specific label/title/placeholder text.
    accessible_name: Option<String>,
}

impl Widget {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            node: None,
            label_node: None,
            chrome_node: None,
            aux: Vec::new(),
            accessible_name: None,
        }
    }
}

#[derive(Debug, Clone)]
enum DragState {
    Splitter {
        splitter: WidgetId,
        start_pos: f32,
        start_ratio: f32,
        axis: Axis,
        bounds: Rect,
        sash_size: f32,
    },
    Scroll {
        area: WidgetId,
        start_x: f32,
        start_y: f32,
        start_scroll_x: f32,
        start_scroll_y: f32,
    },
    Slider {
        slider: WidgetId,
        track: Rect,
    },
}


#[derive(Debug, Clone)]
struct FlatTreeRow {
    id: u64,
    depth: usize,
    label: String,
    has_children: bool,
    expanded: bool,
}

fn flatten_tree(roots: &[TreeNode]) -> Vec<FlatTreeRow> {
    let mut out = Vec::new();
    fn walk(nodes: &[TreeNode], depth: usize, out: &mut Vec<FlatTreeRow>) {
        for n in nodes {
            let has = !n.children.is_empty();
            out.push(FlatTreeRow {
                id: n.id,
                depth,
                label: n.label.clone(),
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

fn set_expanded_in_forest(nodes: &mut [TreeNode], id: u64, expanded: bool) -> bool {
    for n in nodes {
        if n.id == id {
            n.expanded = expanded;
            return true;
        }
        if set_expanded_in_forest(&mut n.children, id, expanded) {
            return true;
        }
    }
    false
}

fn toggle_expanded_in_forest(nodes: &mut [TreeNode], id: u64) -> bool {
    for n in nodes {
        if n.id == id {
            if !n.children.is_empty() {
                n.expanded = !n.expanded;
                return true;
            }
            return false;
        }
        if toggle_expanded_in_forest(&mut n.children, id) {
            return true;
        }
    }
    false
}

/// Retained UI tree that mounts into a [`Scene`].
#[derive(Debug)]
pub struct Ui {
    next_id: u64,
    widgets: HashMap<WidgetId, Widget>,
    /// Top-level columns/rows/shell widgets (mount roots).
    roots: Vec<WidgetId>,
    /// scene node → widget (buttons, splitter sashes, inputs).
    node_to_widget: HashMap<NodeId, WidgetId>,
    font: Arc<Font>,
    drag: Option<DragState>,
    /// Focused widget (inputs + List / Tree / Table / Select / Slider / Tabs).
    focus: Option<WidgetId>,
    /// Currently open popup owner (Menu / ContextMenu / Select); at most one.
    open_popup: Option<WidgetId>,
    /// Last activated menu/context item; drained by [`Ui::take_menu_action`].
    pending_menu_action: Option<(WidgetId, u64)>,
    /// Last keyboard-activated Button; drained by [`Ui::take_button_activate`].
    pending_button_activate: Option<WidgetId>,
    /// Scene overlay host (last child of root) for popup panels.
    overlay_host: Option<NodeId>,
    /// Hovered Tooltip host (pointer enter); tip painted in sync.
    tooltip_hover: Option<WidgetId>,
    /// Last Modal action; drained by [`Ui::take_modal_action`].
    pending_modal_action: Option<(WidgetId, ModalAction)>,
    /// Last Tag close; drained by [`Ui::take_tag_close`].
    pending_tag_close: Option<WidgetId>,
    /// Last Alert dismiss; drained by [`Ui::take_alert_dismiss`].
    pending_alert_dismiss: Option<WidgetId>,
    /// Last Drawer action; drained by [`Ui::take_drawer_action`].
    pending_drawer_action: Option<(WidgetId, DrawerAction)>,
    /// Last Breadcrumb click index; drained by [`Ui::take_breadcrumb_click`].
    pending_breadcrumb_click: Option<(WidgetId, usize)>,
    /// Last Pagination page change; drained by [`Ui::take_pagination_change`].
    pending_pagination_change: Option<(WidgetId, usize)>,
    /// Queued notification toasts (global overlay).
    notifications: Vec<NotificationItem>,
    /// Next notification id.
    next_notification_id: u64,
    /// Notification toast style.
    notification_style: NotificationStyle,
    /// Overlay aux nodes for notification toasts (owned by a synthetic host).
    notification_aux: Vec<NodeId>,
    /// Window viewport for Modal/Drawer scrim (set via [`Ui::set_viewport`]).
    viewport: (f32, f32),
    /// Last pointer position for Tooltip hover resolution in [`Ui::sync`].
    last_pointer: Option<(f32, f32)>,
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

/// Keyboard key for [`Ui::key_input`] (apps map platform keys into this).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiKey {
    /// Printable character from the keyboard path (typically Latin/ASCII).
    /// CJK composition uses [`Ui::ime_preedit`] / [`Ui::ime_commit`], not this variant.
    Char(char),
    /// Backspace.
    Backspace,
    /// Forward delete.
    Delete,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Up arrow (list/tree/table selection).
    Up,
    /// Down arrow (list/tree/table selection).
    Down,
    /// Home.
    Home,
    /// End.
    End,
    /// Enter.
    Enter,
    /// Tab (move focus forward).
    Tab,
    /// Shift+Tab (move focus backward).
    BackTab,
    /// Space (toggle focused checkbox/switch/radio; activate focused Button).
    Space,
    /// Select-all in focused text input (Ctrl/Cmd+A).
    SelectAll,
    /// Escape (close open popup).
    Escape,
}

impl Ui {
    /// Empty UI using the default system CJK-capable font.
    pub fn new() -> Self {
        let font = load_default_cjk_font().unwrap_or_else(|e| {
            panic!("next-gui-ui: failed to load default CJK font: {e}");
        });
        Self::with_font(font)
    }

    /// UI with an explicit font face.
    pub fn with_font(font: Font) -> Self {
        Self {
            next_id: 1,
            widgets: HashMap::new(),
            roots: Vec::new(),
            node_to_widget: HashMap::new(),
            font: Arc::new(font),
            drag: None,
            focus: None,
            open_popup: None,
            pending_menu_action: None,
            pending_button_activate: None,
            overlay_host: None,
            tooltip_hover: None,
            pending_modal_action: None,
            pending_tag_close: None,
            pending_alert_dismiss: None,
            pending_drawer_action: None,
            pending_breadcrumb_click: None,
            pending_pagination_change: None,
            notifications: Vec::new(),
            next_notification_id: 1,
            notification_style: NotificationStyle::default(),
            notification_aux: Vec::new(),
            viewport: (800.0, 600.0),
            last_pointer: None,
        }
    }

    /// Borrow the active font.
    pub fn font(&self) -> &Font {
        &self.font
    }

    /// Set window size used for Modal scrim centering (call on resize).
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.viewport = (width.max(1.0), height.max(1.0));
    }

    fn alloc(&mut self) -> WidgetId {
        let id = WidgetId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Declare a top-level column and build its children.
    pub fn column<F>(&mut self, style: ColumnStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.alloc_container(Kind::Column {
            style,
            children: Vec::new(),
        });
        self.roots.push(id);
        {
            let mut builder = FlexBuilder { ui: self, parent: id };
            build(&mut builder);
        }
        id
    }

    /// Declare a top-level row (e.g. window toolbar).
    pub fn row<F>(&mut self, style: RowStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.alloc_container(Kind::Row {
            style,
            children: Vec::new(),
        });
        self.roots.push(id);
        {
            let mut builder = FlexBuilder { ui: self, parent: id };
            build(&mut builder);
        }
        id
    }

    /// Declare a top-level toolbar.
    pub fn toolbar<F>(&mut self, style: ToolbarStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.alloc_container(Kind::Toolbar {
            style,
            children: Vec::new(),
        });
        self.roots.push(id);
        {
            let mut builder = FlexBuilder { ui: self, parent: id };
            build(&mut builder);
        }
        id
    }

    /// Declare a top-level titled panel.
    pub fn panel<F>(&mut self, title: impl Into<String>, style: PanelStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.alloc_container(Kind::Panel {
            title: title.into(),
            style,
            children: Vec::new(),
        });
        self.roots.push(id);
        {
            let mut builder = FlexBuilder { ui: self, parent: id };
            build(&mut builder);
        }
        id
    }

    /// Declare a top-level status bar.
    pub fn status_bar<F>(&mut self, style: StatusBarStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.alloc_container(Kind::StatusBar {
            style,
            children: Vec::new(),
        });
        self.roots.push(id);
        {
            let mut builder = FlexBuilder { ui: self, parent: id };
            build(&mut builder);
        }
        id
    }

    /// Declare a top-level splitter; each closure should add one pane child (e.g. a panel).
    pub fn splitter<A, B>(&mut self, style: SplitterStyle, build_a: A, build_b: B) -> WidgetId
    where
        A: FnOnce(&mut FlexBuilder<'_>),
        B: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.alloc_container(Kind::Splitter {
            style,
            children: Vec::new(),
        });
        self.roots.push(id);
        {
            let mut builder = FlexBuilder { ui: self, parent: id };
            build_a(&mut builder);
        }
        {
            let mut builder = FlexBuilder { ui: self, parent: id };
            build_b(&mut builder);
        }
        id
    }


    /// Declare a top-level ScrollArea.
    pub fn scroll_area<F>(&mut self, style: ScrollAreaStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.alloc_container(Kind::ScrollArea {
            style,
            children: Vec::new(),
            scroll_x: 0.0,
            scroll_y: 0.0,
            content_w: style.width,
            content_h: style.height,
        });
        self.roots.push(id);
        {
            let mut builder = FlexBuilder { ui: self, parent: id };
            build(&mut builder);
        }
        id
    }

    fn alloc_container(&mut self, kind: Kind) -> WidgetId {
        let id = self.alloc();
        self.widgets.insert(id, Widget::new(kind));
        id
    }

    /// Update text content; call [`Ui::sync`] afterwards.
    pub fn set_text(&mut self, id: WidgetId, content: impl Into<String>) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Text { content: c, .. } = &mut w.kind {
                *c = content.into();
            }
        }
    }

    /// Update a panel title; call [`Ui::sync`] afterwards.
    pub fn set_panel_title(&mut self, id: WidgetId, title: impl Into<String>) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Panel { title: t, .. } = &mut w.kind {
                *t = title.into();
            }
        }
    }

    /// Set splitter ratio (0..1); call [`Ui::sync`] afterwards.
    pub fn set_splitter_ratio(&mut self, id: WidgetId, ratio: f32) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Splitter { style, .. } = &mut w.kind {
                style.ratio = ratio.clamp(0.05, 0.95);
            }
        }
    }

    /// Current splitter ratio, if the widget is a splitter.
    pub fn splitter_ratio(&self, id: WidgetId) -> Option<f32> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Splitter { style, .. }) => Some(style.ratio),
            _ => None,
        }
    }

    /// Read text content if the widget is a Text.
    pub fn text_content(&self, id: WidgetId) -> Option<&str> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Text { content, .. }) => Some(content.as_str()),
            _ => None,
        }
    }

    /// Whether this id is a button.
    pub fn is_button(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Button { .. })
        )
    }

    /// Whether this id is a splitter.
    pub fn is_splitter(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Splitter { .. })
        )
    }

    /// Map a scene node to a widget (buttons and splitter sashes are registered).
    pub fn widget_from_node(&self, node: NodeId) -> Option<WidgetId> {
        self.node_to_widget.get(&node).copied()
    }

    /// Hit-test widgets using the scene graph.
    pub fn hit_widget(&self, scene: &Scene, x: f32, y: f32) -> Option<WidgetId> {
        let node = scene.hit_test(x, y)?;
        if let Some(wid) = self.widget_from_node(node) {
            return Some(wid);
        }
        let mut cur = scene.get(node)?.parent;
        while let Some(pid) = cur {
            if let Some(wid) = self.widget_from_node(pid) {
                return Some(wid);
            }
            cur = scene.get(pid).and_then(|n| n.parent);
        }
        None
    }

    /// Begin a pointer press (primary button).
    ///
    /// Handles splitter sash drag, scroll-area drag, focus for inputs,
    /// toggle for Checkbox / Radio / Switch, data selection, Menu / Select
    /// open-close, Slider thumb, and Tabs. Returns true if the UI mutated
    /// (caller should [`Ui::sync`] + redraw).
    pub fn pointer_pressed(&mut self, scene: &mut Scene, x: f32, y: f32) -> bool {
        self.pointer_press(scene, x, y, false)
    }

    /// Begin a pointer press with primary (`secondary = false`) or secondary
    /// / right button (`secondary = true`, opens [`Kind::ContextMenu`]).
    pub fn pointer_press(&mut self, scene: &mut Scene, x: f32, y: f32, secondary: bool) -> bool {
        // Open popup hit first (overlay sits above everything).
        if let Some(pid) = self.open_popup {
            if self.popup_contains(scene, pid, x, y) {
                return self.pointer_popup_item(scene, pid, x, y);
            }
            // Click outside closes popup; hide overlay aux immediately so the
            // underlying widget can be hit without an intervening sync.
            self.hide_popup_aux_mut(scene, pid);
            self.close_popup_internal();
        }

        let Some(id) = self.hit_widget(scene, x, y) else {
            self.focus = None;
            return true;
        };

        if secondary {
            // Right-click: open context menu if host (or ancestor) is ContextMenu.
            if let Some(cm) = self.find_context_menu_ancestor(id) {
                self.focus = Some(cm);
                self.open_context_menu_at(cm, x, y);
                return true;
            }
            return false;
        }

        // Splitter sash
        if self.is_splitter(id) {
            let Some(w) = self.widgets.get(&id) else {
                return false;
            };
            let Kind::Splitter { style, .. } = &w.kind else {
                return false;
            };
            let bounds = w
                .node
                .and_then(|n| scene.get(n).map(|n| n.bounds))
                .unwrap_or_else(Rect::empty);
            let start_pos = match style.axis {
                Axis::Vertical => x,
                Axis::Horizontal => y,
            };
            self.drag = Some(DragState::Splitter {
                splitter: id,
                start_pos,
                start_ratio: style.ratio,
                axis: style.axis,
                bounds,
                sash_size: style.sash_size,
            });
            return true;
        }

        // ScrollArea: start drag-scroll
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::ScrollArea { .. })
        ) {
            let (sx, sy) = match self.widgets.get(&id).map(|w| &w.kind) {
                Some(Kind::ScrollArea {
                    scroll_x, scroll_y, ..
                }) => (*scroll_x, *scroll_y),
                _ => (0.0, 0.0),
            };
            self.drag = Some(DragState::Scroll {
                area: id,
                start_x: x,
                start_y: y,
                start_scroll_x: sx,
                start_scroll_y: sy,
            });
            self.focus = None;
            return true;
        }

        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::TextInput { .. }) => {
                self.focus = Some(id);
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::TextInput {
                        value,
                        caret,
                        select_all,
                        ..
                    } = &mut w.kind
                    {
                        *select_all = false;
                        *caret = value.chars().count();
                    }
                }
                true
            }
            Some(Kind::Checkbox { .. }) => {
                self.focus = Some(id);
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Checkbox { checked, .. } = &mut w.kind {
                        *checked = !*checked;
                    }
                }
                true
            }
            Some(Kind::Radio { .. }) => {
                self.focus = Some(id);
                self.select_radio(id);
                true
            }
            Some(Kind::Switch { .. }) => {
                self.focus = Some(id);
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Switch { on, .. } = &mut w.kind {
                        *on = !*on;
                    }
                }
                true
            }
            Some(Kind::List { .. }) => {
                self.focus = Some(id);
                self.pointer_list(scene, id, x, y)
            }
            Some(Kind::Tree { .. }) => {
                self.focus = Some(id);
                self.pointer_tree(scene, id, x, y)
            }
            Some(Kind::Table { .. }) => {
                self.focus = Some(id);
                self.pointer_table(scene, id, x, y)
            }
            Some(Kind::Menu { .. }) => {
                self.focus = Some(id);
                self.pointer_menu_title(scene, id)
            }
            Some(Kind::ContextMenu { .. }) => {
                // Left click on host: no menu; children handle via hit.
                self.focus = None;
                false
            }
            Some(Kind::Tabs { .. }) => {
                self.focus = Some(id);
                self.pointer_tabs(scene, id, x, y)
            }
            Some(Kind::Select { .. }) => {
                self.focus = Some(id);
                self.pointer_select_toggle(scene, id)
            }
            Some(Kind::TreeSelect { .. }) => {
                self.focus = Some(id);
                self.pointer_tree_select_toggle(scene, id)
            }
            Some(Kind::AlarmList { .. }) => {
                self.focus = Some(id);
                self.pointer_alarm_list(scene, id, x, y)
            }
            Some(Kind::Slider { .. }) => {
                self.focus = Some(id);
                self.pointer_slider(scene, id, x, y)
            }
            Some(Kind::Tag { closable: true, .. }) => {
                self.focus = None;
                self.pointer_tag_close(scene, id, x, y)
            }
            Some(Kind::Alert {
                dismissible: true,
                dismissed: false,
                ..
            }) => {
                self.focus = None;
                self.pointer_alert_dismiss(scene, id, x, y)
            }
            Some(Kind::TextArea { .. }) => {
                self.focus = Some(id);
                true
            }
            Some(Kind::InputNumber { .. }) => {
                self.focus = Some(id);
                self.pointer_input_number(scene, id, x, y)
            }
            Some(Kind::Segmented { .. }) => {
                self.focus = Some(id);
                self.pointer_segmented(scene, id, x, y)
            }
            Some(Kind::Breadcrumb { .. }) => {
                self.focus = None;
                self.pointer_breadcrumb(scene, id, x, y)
            }
            Some(Kind::Pagination { .. }) => {
                self.focus = Some(id);
                self.pointer_pagination(scene, id, x, y)
            }
            Some(Kind::Collapse { .. }) => {
                self.focus = Some(id);
                self.pointer_collapse(scene, id, x, y)
            }
            Some(Kind::DateTime { .. }) => {
                self.focus = Some(id);
                self.pointer_datetime_toggle(scene, id)
            }
            Some(Kind::Transfer { .. }) => {
                self.focus = Some(id);
                self.pointer_transfer(scene, id, x, y)
            }
            Some(Kind::PropertyGrid { .. }) => {
                self.focus = Some(id);
                self.pointer_property_grid(scene, id, x, y)
            }
            Some(Kind::TreeTable { .. }) => {
                self.focus = Some(id);
                self.pointer_tree_table(scene, id, x, y)
            }
            Some(Kind::Button { .. }) => {
                // Dual path: focus for Tab/Space/Enter; apps still activate via
                // `hit_widget` on pointer (this returns false so click semantics stay).
                self.focus = Some(id);
                false
            }
            _ => {
                self.focus = None;
                false
            }
        }
    }

    /// Continue an active drag, or update Tooltip hover when idle.
    pub fn pointer_moved(&mut self, x: f32, y: f32) -> bool {
        let Some(drag) = self.drag.clone() else {
            return self.update_tooltip_hover_xy(x, y);
        };
        match drag {
            DragState::Splitter {
                splitter,
                start_pos,
                start_ratio,
                axis,
                bounds,
                sash_size,
            } => {
                let span = match axis {
                    Axis::Vertical => (bounds.width - sash_size).max(1.0),
                    Axis::Horizontal => (bounds.height - sash_size).max(1.0),
                };
                let cur = match axis {
                    Axis::Vertical => x,
                    Axis::Horizontal => y,
                };
                let delta = cur - start_pos;
                let new_ratio = (start_ratio + delta / span).clamp(0.1, 0.9);
                if let Some(w) = self.widgets.get_mut(&splitter) {
                    if let Kind::Splitter { style, .. } = &mut w.kind {
                        if (style.ratio - new_ratio).abs() < 0.0005 {
                            return false;
                        }
                        style.ratio = new_ratio;
                        return true;
                    }
                }
                false
            }
            DragState::Scroll {
                area,
                start_x,
                start_y,
                start_scroll_x,
                start_scroll_y,
            } => {
                let dx = start_x - x;
                let dy = start_y - y;
                self.set_any_scroll(area, start_scroll_x + dx, start_scroll_y + dy)
            }
            DragState::Slider { slider, track } => self.apply_slider_x(slider, track, x),
        }
    }

    /// End an active drag.
    pub fn pointer_released(&mut self) {
        self.drag = None;
    }

    /// Whether a drag is active.
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Apply mouse-wheel scroll at (x,y); returns true if a scrollable scrolled.
    pub fn pointer_scroll(&mut self, scene: &Scene, x: f32, y: f32, dx: f32, dy: f32) -> bool {
        let Some(id) = self.hit_widget(scene, x, y) else {
            return false;
        };
        let area = self.find_scroll_ancestor(id).or_else(|| {
            if self.is_data_scrollable(id) {
                Some(id)
            } else {
                None
            }
        });
        let Some(area) = area else {
            return false;
        };
        let (sx, sy) = self.any_scroll_offset(area).unwrap_or((0.0, 0.0));
        self.set_any_scroll(area, sx + dx, sy + dy)
    }

    fn find_scroll_ancestor(&self, id: WidgetId) -> Option<WidgetId> {
        // If id is scroll, return; else search scroll areas for membership.
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::ScrollArea { .. })
        ) {
            return Some(id);
        }
        for (sid, w) in &self.widgets {
            if let Kind::ScrollArea { children, .. } = &w.kind {
                if self.subtree_contains(*sid, children, id) {
                    return Some(*sid);
                }
            }
        }
        None
    }

    fn subtree_contains(&self, _root: WidgetId, children: &[WidgetId], target: WidgetId) -> bool {
        for c in children {
            if *c == target {
                return true;
            }
            if let Some(w) = self.widgets.get(c) {
                let kids = match &w.kind {
                    Kind::Column { children, .. }
                    | Kind::Row { children, .. }
                    | Kind::Toolbar { children, .. }
                    | Kind::Panel { children, .. }
                    | Kind::StatusBar { children, .. }
                    | Kind::Splitter { children, .. }
                    | Kind::ScrollArea { children, .. }
                    | Kind::ContextMenu { children, .. }
                    | Kind::Tabs { children, .. }
                    | Kind::Badge { children, .. }
                    | Kind::Tooltip { children, .. }
                    | Kind::Form { children, .. } => children.as_slice(),
                    _ => &[],
                };
                if self.subtree_contains(*c, kids, target) {
                    return true;
                }
            }
        }
        false
    }

    fn set_scroll_offset(&mut self, id: WidgetId, sx: f32, sy: f32) -> bool {
        let Some(w) = self.widgets.get_mut(&id) else {
            return false;
        };
        let Kind::ScrollArea {
            style,
            scroll_x,
            scroll_y,
            content_w,
            content_h,
            ..
        } = &mut w.kind
        else {
            return false;
        };
        let max_x = (*content_w - style.width).max(0.0);
        let max_y = (*content_h - style.height).max(0.0);
        let nx = sx.clamp(0.0, max_x);
        let ny = sy.clamp(0.0, max_y);
        if (nx - *scroll_x).abs() < 0.1 && (ny - *scroll_y).abs() < 0.1 {
            return false;
        }
        *scroll_x = nx;
        *scroll_y = ny;
        true
    }

    /// Focused widget, if any.
    pub fn focused(&self) -> Option<WidgetId> {
        self.focus
    }

    /// Set focus explicitly.
    pub fn set_focus(&mut self, id: Option<WidgetId>) {
        self.focus = id;
    }

    /// Override the accessible name for `id` (P6d). Empty string clears to fallback.
    pub fn set_accessible_name(&mut self, id: WidgetId, name: impl Into<String>) {
        if let Some(w) = self.widgets.get_mut(&id) {
            let s = name.into();
            w.accessible_name = if s.is_empty() { None } else { Some(s) };
        }
    }

    /// Clear the accessible name override so kind fallback applies.
    pub fn clear_accessible_name(&mut self, id: WidgetId) {
        if let Some(w) = self.widgets.get_mut(&id) {
            w.accessible_name = None;
        }
    }

    /// Accessible name: override if set, else best-effort fallback from Kind.
    ///
    /// Fallbacks (when non-empty): Button/Checkbox/Switch/Radio/Tag `label`;
    /// Panel/Menu/Modal/Drawer/Card `title`; TextInput/TextArea/Select/TreeSelect
    /// `placeholder` (or current value for TextInput/TextArea when non-empty);
    /// Gauge `label`; Alert `message`; Empty `description`; Tabs selected title;
    /// Segmented selected option. Other kinds may return `None`.
    pub fn accessible_name(&self, id: WidgetId) -> Option<String> {
        let w = self.widgets.get(&id)?;
        if let Some(name) = &w.accessible_name {
            return Some(name.clone());
        }
        match &w.kind {
            Kind::Button { label, .. }
            | Kind::Checkbox { label, .. }
            | Kind::Switch { label, .. }
            | Kind::Radio { label, .. }
            | Kind::Tag { label, .. }
            | Kind::Gauge { label, .. } => {
                if label.is_empty() {
                    None
                } else {
                    Some(label.clone())
                }
            }
            Kind::Panel { title, .. }
            | Kind::Menu { title, .. }
            | Kind::Modal { title, .. }
            | Kind::Drawer { title, .. }
            | Kind::Card { title, .. } => {
                if title.is_empty() {
                    None
                } else {
                    Some(title.clone())
                }
            }
            Kind::TextInput {
                value,
                placeholder,
                ..
            }
            | Kind::TextArea {
                value,
                placeholder,
                ..
            } => {
                if !value.is_empty() {
                    Some(value.clone())
                } else if !placeholder.is_empty() {
                    Some(placeholder.clone())
                } else {
                    None
                }
            }
            Kind::Select { placeholder, selected, options, .. } => {
                if let Some(i) = *selected {
                    options.get(i).cloned()
                } else if !placeholder.is_empty() {
                    Some(placeholder.clone())
                } else {
                    None
                }
            }
            Kind::TreeSelect { placeholder, .. } => {
                if placeholder.is_empty() {
                    None
                } else {
                    Some(placeholder.clone())
                }
            }
            Kind::Alert { message, .. } => {
                if message.is_empty() {
                    None
                } else {
                    Some(message.clone())
                }
            }
            Kind::Empty { description, .. } => {
                if description.is_empty() {
                    None
                } else {
                    Some(description.clone())
                }
            }
            Kind::Tabs { titles, selected, .. } => titles.get(*selected).cloned(),
            Kind::Segmented { options, selected, .. } => options.get(*selected).cloned(),
            Kind::Text { content, .. } => {
                if content.is_empty() {
                    None
                } else {
                    Some(content.clone())
                }
            }
            _ => None,
        }
    }

    /// Whether this kind participates in Tab focus order (matches pointer focus).
    fn kind_is_focusable(kind: &Kind) -> bool {
        matches!(
            kind,
            Kind::Button { .. }
                | Kind::TextInput { .. }
                | Kind::TextArea { .. }
                | Kind::InputNumber { .. }
                | Kind::Checkbox { .. }
                | Kind::Radio { .. }
                | Kind::Switch { .. }
                | Kind::List { .. }
                | Kind::Tree { .. }
                | Kind::Table { .. }
                | Kind::Menu { .. }
                | Kind::Tabs { .. }
                | Kind::Select { .. }
                | Kind::TreeSelect { .. }
                | Kind::AlarmList { .. }
                | Kind::Slider { .. }
                | Kind::Segmented { .. }
                | Kind::Pagination { .. }
                | Kind::Collapse { .. }
                | Kind::DateTime { .. }
                | Kind::Transfer { .. }
                | Kind::PropertyGrid { .. }
                | Kind::TreeTable { .. }
        )
    }

    fn widget_children_ids(kind: &Kind) -> &[WidgetId] {
        match kind {
            Kind::Column { children, .. }
            | Kind::Row { children, .. }
            | Kind::Toolbar { children, .. }
            | Kind::Panel { children, .. }
            | Kind::StatusBar { children, .. }
            | Kind::Splitter { children, .. }
            | Kind::ScrollArea { children, .. }
            | Kind::ContextMenu { children, .. }
            | Kind::Tabs { children, .. }
            | Kind::Badge { children, .. }
            | Kind::Tooltip { children, .. }
            | Kind::Form { children, .. }
            | Kind::Spin { children, .. } => children.as_slice(),
            _ => &[],
        }
    }

    /// Focusable widgets in stable DFS order over [`Ui`] roots (visual tree order).
    ///
    /// Includes Button (`tool_button` shares [`Kind::Button`]) plus kinds that
    /// pointer click focuses (inputs, toggles, data/nav/industrial). See
    /// `docs/a11y-baseline.md`.
    pub fn focusable_widgets(&self) -> Vec<WidgetId> {
        let mut out = Vec::new();
        for &root in &self.roots {
            self.collect_focusable_dfs(root, &mut out);
        }
        out
    }

    fn collect_focusable_dfs(&self, id: WidgetId, out: &mut Vec<WidgetId>) {
        let Some(w) = self.widgets.get(&id) else {
            return;
        };
        if Self::kind_is_focusable(&w.kind) {
            out.push(id);
        }
        let kids: Vec<WidgetId> = Self::widget_children_ids(&w.kind).to_vec();
        for child in kids {
            self.collect_focusable_dfs(child, out);
        }
    }

    /// Move focus to the next focusable widget (wraps). No focus → first.
    /// Returns the new focus, or `None` if the focusable list is empty.
    pub fn focus_next(&mut self) -> Option<WidgetId> {
        let list = self.focusable_widgets();
        if list.is_empty() {
            return None;
        }
        let next = match self.focus.and_then(|f| list.iter().position(|&id| id == f)) {
            Some(i) => list[(i + 1) % list.len()],
            None => list[0],
        };
        self.focus = Some(next);
        Some(next)
    }

    /// Move focus to the previous focusable widget (wraps). No focus → last.
    pub fn focus_prev(&mut self) -> Option<WidgetId> {
        let list = self.focusable_widgets();
        if list.is_empty() {
            return None;
        }
        let prev = match self.focus.and_then(|f| list.iter().position(|&id| id == f)) {
            Some(i) => list[(i + list.len() - 1) % list.len()],
            None => list[list.len() - 1],
        };
        self.focus = Some(prev);
        Some(prev)
    }

    fn focus_trap_active(&self) -> bool {
        let Some(id) = self.focus else {
            return false;
        };
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Modal { open: true, .. }) | Some(Kind::Drawer { open: true, .. })
        )
    }




    /// Read TextInput value.
    pub fn text_input_value(&self, id: WidgetId) -> Option<&str> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::TextInput { value, .. }) => Some(value.as_str()),
            _ => None,
        }
    }

    /// Set TextInput value; resets caret to end.
    pub fn set_text_input(&mut self, id: WidgetId, value: impl Into<String>) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::TextInput {
                value: v,
                caret,
                select_all,
                preedit,
                preedit_cursor,
                ..
            } = &mut w.kind
            {
                *v = value.into();
                *caret = v.chars().count();
                *select_all = false;
                preedit.clear();
                *preedit_cursor = None;
            }
        }
    }


    /// Text to place on the OS clipboard from the focused TextInput / TextArea.
    ///
    /// When `select_all` is set, returns the full value; otherwise still returns the
    /// full value (MVP has no partial selection range). `None` if focus is not a
    /// text field or the value is empty.
    pub fn focused_text_for_copy(&self) -> Option<String> {
        let id = self.focus?;
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::TextInput { value, .. }) if !value.is_empty() => Some(value.clone()),
            Some(Kind::TextArea { value, .. }) if !value.is_empty() => Some(value.clone()),
            _ => None,
        }
    }

    /// Insert `text` into the focused TextInput / TextArea at the caret.
    ///
    /// Replaces the whole value when `select_all` is set. Clears IME preedit.
    pub fn paste_into_focused_text(&mut self, text: &str) -> bool {
        let Some(id) = self.focus else {
            return false;
        };
        if text.is_empty() {
            return false;
        }
        match self.widgets.get_mut(&id).map(|w| &mut w.kind) {
            Some(Kind::TextInput {
                value,
                caret,
                select_all,
                preedit,
                preedit_cursor,
                ..
            }) => {
                preedit.clear();
                *preedit_cursor = None;
                if *select_all {
                    *value = text.to_owned();
                    *caret = value.chars().count();
                    *select_all = false;
                } else {
                    let mut chars: Vec<char> = value.chars().collect();
                    *caret = (*caret).min(chars.len());
                    for (i, ch) in text.chars().enumerate() {
                        chars.insert(*caret + i, ch);
                    }
                    *caret += text.chars().count();
                    *value = chars.into_iter().collect();
                }
                true
            }
            Some(Kind::TextArea { value, caret, .. }) => {
                let mut chars: Vec<char> = value.chars().collect();
                *caret = (*caret).min(chars.len());
                for (i, ch) in text.chars().enumerate() {
                    chars.insert(*caret + i, ch);
                }
                *caret += text.chars().count();
                *value = chars.into_iter().collect();
                true
            }
            _ => false,
        }
    }

    /// True when focus is on a [`Kind::TextInput`] (app should enable OS IME).
    pub fn text_input_wants_ime(&self) -> bool {
        self.focus
            .map(|id| self.is_text_input(id))
            .unwrap_or(false)
    }

    /// Caret rectangle for the focused TextInput (physical px), for IME candidate placement.
    pub fn ime_cursor_area(&self, scene: &Scene) -> Option<(f32, f32, f32, f32)> {
        let id = self.focus.filter(|&id| self.is_text_input(id))?;
        let w = self.widgets.get(&id)?;
        let Kind::TextInput {
            value,
            caret,
            preedit,
            style,
            ..
        } = &w.kind
        else {
            return None;
        };
        let bounds = w.node.and_then(|n| scene.get(n).map(|n| n.bounds))?;
        let before: String = value.chars().take(*caret).collect();
        let (bw, _) = self.font.measure_line(&before, style.size);
        let (pw, _) = if preedit.is_empty() {
            (0.0, 0.0)
        } else {
            self.font.measure_line(preedit, style.size)
        };
        let cx = bounds.x + style.padding_x + bw + pw;
        let cy = bounds.y + 4.0;
        let ch = (bounds.height - 8.0).max(8.0);
        Some((cx, cy, 2.0_f32.max(1.0), ch))
    }

    /// Update IME preedit on the focused TextInput. Empty `text` clears composition.
    pub fn ime_preedit(&mut self, text: impl Into<String>, cursor: Option<(usize, usize)>) -> bool {
        let Some(id) = self.focus else {
            return false;
        };
        let Some(w) = self.widgets.get_mut(&id) else {
            return false;
        };
        let Kind::TextInput {
            preedit,
            preedit_cursor,
            ..
        } = &mut w.kind
        else {
            return false;
        };
        *preedit = text.into();
        *preedit_cursor = if preedit.is_empty() { None } else { cursor };
        true
    }

    /// Commit IME text into the focused TextInput at the caret (clears preedit).
    pub fn ime_commit(&mut self, text: impl AsRef<str>) -> bool {
        let Some(id) = self.focus else {
            return false;
        };
        let text = text.as_ref();
        if text.is_empty() {
            // Still clear preedit.
            return self.ime_preedit("", None);
        }
        let Some(w) = self.widgets.get_mut(&id) else {
            return false;
        };
        let Kind::TextInput {
            value,
            caret,
            select_all,
            preedit,
            preedit_cursor,
            ..
        } = &mut w.kind
        else {
            return false;
        };
        preedit.clear();
        *preedit_cursor = None;
        let mut chars: Vec<char> = value.chars().collect();
        *caret = (*caret).min(chars.len());
        if *select_all {
            chars.clear();
            *caret = 0;
            *select_all = false;
        }
        for c in text.chars() {
            chars.insert(*caret, c);
            *caret += 1;
        }
        *value = chars.into_iter().collect();
        true
    }

    /// Clear preedit on the focused TextInput (IME disabled / cancelled).
    pub fn ime_clear_preedit(&mut self) -> bool {
        self.ime_preedit("", None)
    }

    fn scrub_unfocused_ime(&mut self) {
        let focus = self.focus;
        for (id, w) in self.widgets.iter_mut() {
            if Some(*id) == focus {
                continue;
            }
            if let Kind::TextInput {
                preedit,
                preedit_cursor,
                ..
            } = &mut w.kind
            {
                if !preedit.is_empty() || preedit_cursor.is_some() {
                    preedit.clear();
                    *preedit_cursor = None;
                }
            }
        }
    }

    /// Checkbox / Switch / Radio selected state.
    pub fn is_checked(&self, id: WidgetId) -> Option<bool> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Checkbox { checked, .. }) => Some(*checked),
            Some(Kind::Switch { on, .. }) => Some(*on),
            Some(Kind::Radio { selected, .. }) => Some(*selected),
            _ => None,
        }
    }

    /// Set Checkbox or Switch checked/on state.
    pub fn set_checked(&mut self, id: WidgetId, checked: bool) {
        if let Some(w) = self.widgets.get_mut(&id) {
            match &mut w.kind {
                Kind::Checkbox { checked: c, .. } => *c = checked,
                Kind::Switch { on, .. } => *on = checked,
                _ => {}
            }
        }
    }

    /// Select a radio (clears others in the same group).
    pub fn select_radio(&mut self, id: WidgetId) {
        let group = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Radio { group, .. }) => *group,
            _ => return,
        };
        let ids: Vec<WidgetId> = self.widgets.keys().copied().collect();
        for wid in ids {
            if let Some(w) = self.widgets.get_mut(&wid) {
                if let Kind::Radio {
                    group: g,
                    selected,
                    ..
                } = &mut w.kind
                {
                    *selected = *g == group && wid == id;
                }
            }
        }
    }


    /// Scroll offsets for a ScrollArea.
    pub fn scroll_offset(&self, id: WidgetId) -> Option<(f32, f32)> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::ScrollArea {
                scroll_x, scroll_y, ..
            }) => Some((*scroll_x, *scroll_y)),
            _ => None,
        }
    }

    fn is_data_scrollable(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::List { .. } | Kind::Tree { .. } | Kind::Table { .. } | Kind::AlarmList { .. } | Kind::ScrollArea { .. } | Kind::PropertyGrid { .. } | Kind::TreeTable { .. })
        )
    }

    fn any_scroll_offset(&self, id: WidgetId) -> Option<(f32, f32)> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::ScrollArea {
                scroll_x, scroll_y, ..
            }) => Some((*scroll_x, *scroll_y)),
            Some(Kind::List { scroll_y, .. }) => Some((0.0, *scroll_y)),
            Some(Kind::Tree { scroll_y, .. }) => Some((0.0, *scroll_y)),
            Some(Kind::Table { scroll_y, .. }) => Some((0.0, *scroll_y)),
            Some(Kind::AlarmList { scroll_y, .. }) => Some((0.0, *scroll_y)),
            Some(Kind::TreeSelect { scroll_y, open: true, .. }) => Some((0.0, *scroll_y)),
            Some(Kind::PropertyGrid { scroll_y, .. }) => Some((0.0, *scroll_y)),
            Some(Kind::TreeTable { scroll_y, .. }) => Some((0.0, *scroll_y)),
            _ => None,
        }
    }

    fn set_any_scroll(&mut self, id: WidgetId, sx: f32, sy: f32) -> bool {
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::ScrollArea { .. })
        ) {
            return self.set_scroll_offset(id, sx, sy);
        }
        let max_y = self.data_max_scroll_y(id);
        let ny = sy.clamp(0.0, max_y);
        if let Some(w) = self.widgets.get_mut(&id) {
            match &mut w.kind {
                Kind::List { scroll_y, .. }
                | Kind::Tree { scroll_y, .. }
                | Kind::Table { scroll_y, .. }
                | Kind::AlarmList { scroll_y, .. }
                | Kind::TreeSelect { scroll_y, .. }
                | Kind::PropertyGrid { scroll_y, .. }
                | Kind::TreeTable { scroll_y, .. } => {
                    if (ny - *scroll_y).abs() < 0.1 {
                        return false;
                    }
                    *scroll_y = ny;
                    let _ = sx;
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    fn data_max_scroll_y(&self, id: WidgetId) -> f32 {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::List { items, style, .. }) => {
                let content = style.padding * 2.0 + items.len() as f32 * style.item_height;
                (content - style.height).max(0.0)
            }
            Some(Kind::Tree { roots, style, .. }) => {
                let n = flatten_tree(roots).len() as f32;
                let content = style.padding * 2.0 + n * style.row_height;
                (content - style.height).max(0.0)
            }
            Some(Kind::Table {
                rows, style, ..
            }) => {
                let body_h = style.height - style.header_height;
                let content = rows.len() as f32 * style.row_height + style.padding;
                (content - body_h).max(0.0)
            }
            Some(Kind::AlarmList { entries, style, .. }) => {
                let content = style.padding * 2.0 + entries.len() as f32 * style.row_height;
                (content - style.height).max(0.0)
            }
            Some(Kind::TreeSelect { roots, style, .. }) => {
                let n = flatten_tree(roots).len();
                let content = n as f32 * style.row_height;
                (content - style.popup_max_height).max(0.0)
            }
            Some(Kind::PropertyGrid { rows, style, .. }) => {
                let header = style.row_height;
                let content = header + rows.len() as f32 * style.row_height;
                (content - style.height).max(0.0)
            }
            Some(Kind::TreeTable { roots, style, .. }) => {
                let n = flatten_tree_table(roots).len() as f32;
                let body_h = style.height - style.header_height;
                let content = n * style.row_height + style.padding;
                (content - body_h).max(0.0)
            }
            _ => 0.0,
        }
    }

    /// Selected list index.
    pub fn list_selected(&self, id: WidgetId) -> Option<usize> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::List { selected, .. }) => *selected,
            _ => None,
        }
    }

    /// Set list selection.
    pub fn set_list_selected(&mut self, id: WidgetId, selected: Option<usize>) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::List {
                items, selected: s, ..
            } = &mut w.kind
            {
                *s = selected.filter(|i| *i < items.len());
            }
        }
    }

    /// Replace list items; clamps selection.
    pub fn set_list_items(&mut self, id: WidgetId, items: impl IntoIterator<Item = impl Into<String>>) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::List {
                items: slot,
                selected,
                scroll_y,
                ..
            } = &mut w.kind
            {
                *slot = items.into_iter().map(|s| s.into()).collect();
                if let Some(i) = *selected {
                    if i >= slot.len() {
                        *selected = slot.len().checked_sub(1);
                    }
                }
                *scroll_y = 0.0;
            }
        }
    }

    /// Selected tree node id.
    pub fn tree_selected(&self, id: WidgetId) -> Option<u64> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Tree { selected, .. }) => *selected,
            _ => None,
        }
    }

    /// Set tree selection by node id.
    pub fn set_tree_selected(&mut self, id: WidgetId, node_id: Option<u64>) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Tree { selected, .. } = &mut w.kind {
                *selected = node_id;
            }
        }
    }

    /// Expand or collapse a tree node by id.
    pub fn set_tree_expanded(&mut self, id: WidgetId, node_id: u64, expanded: bool) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Tree { roots, .. } = &mut w.kind {
                set_expanded_in_forest(roots, node_id, expanded);
            }
        }
    }

    /// Selected table row index.
    pub fn table_selected_row(&self, id: WidgetId) -> Option<usize> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Table { selected_row, .. }) => *selected_row,
            _ => None,
        }
    }

    /// Set table row selection.
    pub fn set_table_selected_row(&mut self, id: WidgetId, row: Option<usize>) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Table {
                rows,
                selected_row,
                ..
            } = &mut w.kind
            {
                *selected_row = row.filter(|i| *i < rows.len());
            }
        }
    }

    /// Replace table body rows.
    pub fn set_table_rows(&mut self, id: WidgetId, rows: Vec<Vec<String>>) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Table {
                rows: slot,
                selected_row,
                scroll_y,
                ..
            } = &mut w.kind
            {
                *slot = rows;
                if let Some(i) = *selected_row {
                    if i >= slot.len() {
                        *selected_row = slot.len().checked_sub(1);
                    }
                }
                *scroll_y = 0.0;
            }
        }
    }

    /// Whether id is a List.
    pub fn is_list(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::List { .. })
        )
    }

    /// Whether id is a Tree.
    pub fn is_tree(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Tree { .. })
        )
    }

    /// Whether id is a Table.
    pub fn is_table(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Table { .. })
        )
    }

    /// Deliver a key to the focused widget. Returns true if handled.
    pub fn key_input(&mut self, key: UiKey) -> bool {
        if matches!(key, UiKey::Escape) && self.open_popup.is_some() {
            if let Some(id) = self.open_popup {
                match self.widgets.get(&id).map(|w| &w.kind) {
                    Some(Kind::Modal { .. }) => {
                        self.pending_modal_action = Some((id, ModalAction::Dismiss));
                    }
                    Some(Kind::Drawer { .. }) => {
                        self.pending_drawer_action = Some((id, DrawerAction::Dismiss));
                    }
                    _ => {}
                }
                if self.focus == Some(id) {
                    self.focus = None;
                }
            }
            return self.close_popup_internal();
        }
        // P6d: Tab / BackTab move focus at the top (before widget dispatch / focus
        // requirement), so text fields do not swallow Tab. Modal/Drawer absorb.
        if matches!(key, UiKey::Tab | UiKey::BackTab) {
            if self.focus_trap_active() {
                return true;
            }
            return match key {
                UiKey::Tab => self.focus_next().is_some(),
                UiKey::BackTab => self.focus_prev().is_some(),
                _ => false,
            };
        }
        let Some(id) = self.focus else {
            return false;
        };
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::TextInput { .. }) => self.key_text_input(id, key),
            Some(Kind::TextArea { .. }) => self.key_text_area(id, key),
            Some(Kind::InputNumber { .. }) => self.key_input_number(id, key),
            Some(Kind::Segmented { .. }) => self.key_segmented(id, key),
            Some(Kind::Pagination { .. }) => self.key_pagination(id, key),
            Some(Kind::DateTime { .. }) => self.key_datetime(id, key),
            Some(Kind::Transfer { .. }) => self.key_transfer(id, key),
            Some(Kind::PropertyGrid { .. }) => self.key_property_grid(id, key),
            Some(Kind::TreeTable { .. }) => self.key_tree_table(id, key),
            Some(Kind::Button { .. }) if matches!(key, UiKey::Space | UiKey::Enter) => {
                self.pending_button_activate = Some(id);
                true
            }
            Some(Kind::Checkbox { .. }) if matches!(key, UiKey::Space) => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Checkbox { checked, .. } = &mut w.kind {
                        *checked = !*checked;
                        return true;
                    }
                }
                false
            }
            Some(Kind::Switch { .. }) if matches!(key, UiKey::Space) => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Switch { on, .. } = &mut w.kind {
                        *on = !*on;
                        return true;
                    }
                }
                false
            }
            Some(Kind::Radio { .. }) if matches!(key, UiKey::Space) => {
                self.select_radio(id);
                true
            }
            Some(Kind::List { .. }) => self.key_list(id, key),
            Some(Kind::Tree { .. }) => self.key_tree(id, key),
            Some(Kind::Table { .. }) => self.key_table(id, key),
            Some(Kind::Tabs { .. }) => self.key_tabs(id, key),
            Some(Kind::Select { .. }) => self.key_select(id, key),
            Some(Kind::TreeSelect { .. }) => self.key_tree_select(id, key),
            Some(Kind::AlarmList { .. }) => self.key_alarm_list(id, key),
            Some(Kind::Slider { .. }) => self.key_slider(id, key),
            Some(Kind::Modal { open: true, .. }) => self.key_modal(id, key),
            Some(Kind::Drawer { open: true, .. }) => self.key_drawer_overlay(id, key),
            _ => false,
        }
    }

    fn key_modal(&mut self, id: WidgetId, key: UiKey) -> bool {
        match key {
            UiKey::Enter => {
                self.pending_modal_action = Some((id, ModalAction::Primary));
                self.close_popup_internal();
                if self.focus == Some(id) {
                    self.focus = None;
                }
                true
            }
            UiKey::Tab | UiKey::BackTab => true, // focus trap while modal open
            _ => false,
        }
    }

    fn key_drawer_overlay(&mut self, id: WidgetId, key: UiKey) -> bool {
        match key {
            UiKey::Tab | UiKey::BackTab => true, // focus trap while drawer open
            UiKey::Enter => {
                // Enter does not dismiss; Esc / × / backdrop do.
                let _ = id;
                false
            }
            _ => false,
        }
    }

    fn key_tabs(&mut self, id: WidgetId, key: UiKey) -> bool {
        let n = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Tabs { titles, .. }) => titles.len(),
            _ => return false,
        };
        if n == 0 {
            return false;
        }
        let cur = self.tabs_selected(id).unwrap_or(0);
        match key {
            UiKey::Left | UiKey::Up => {
                let next = if cur == 0 { n - 1 } else { cur - 1 };
                self.set_tabs_selected(id, next)
            }
            UiKey::Right | UiKey::Down => self.set_tabs_selected(id, (cur + 1) % n),
            UiKey::Home => self.set_tabs_selected(id, 0),
            UiKey::End => self.set_tabs_selected(id, n - 1),
            _ => false,
        }
    }

    fn key_select(&mut self, id: WidgetId, key: UiKey) -> bool {
        let open = matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Select { open: true, .. })
        );
        match key {
            UiKey::Enter | UiKey::Space => {
                if open {
                    self.close_popup_internal()
                } else {
                    self.close_popup_internal();
                    if let Some(w) = self.widgets.get_mut(&id) {
                        if let Kind::Select { open, .. } = &mut w.kind {
                            *open = true;
                        }
                    }
                    self.open_popup = Some(id);
                    true
                }
            }
            UiKey::Down | UiKey::Up => {
                let _ = open;
                let n = match self.widgets.get(&id).map(|w| &w.kind) {
                    Some(Kind::Select { options, .. }) => options.len(),
                    _ => return false,
                };
                if n == 0 {
                    return false;
                }
                let cur = self.select_selected(id).unwrap_or(0);
                let next = match key {
                    UiKey::Down => (cur + 1) % n,
                    UiKey::Up => {
                        if cur == 0 {
                            n - 1
                        } else {
                            cur - 1
                        }
                    }
                    _ => return false,
                };
                if !open {
                    self.close_popup_internal();
                    if let Some(w) = self.widgets.get_mut(&id) {
                        if let Kind::Select { open, .. } = &mut w.kind {
                            *open = true;
                        }
                    }
                    self.open_popup = Some(id);
                }
                self.set_select_selected(id, Some(next));
                // keep open after set_select_selected closed it
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Select { open, .. } = &mut w.kind {
                        *open = true;
                    }
                }
                self.open_popup = Some(id);
                true
            }
            _ => false,
        }
    }

    fn key_slider(&mut self, id: WidgetId, key: UiKey) -> bool {
        let (min, max, value) = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Slider { min, max, value, .. }) => (*min, *max, *value),
            _ => return false,
        };
        let span = (max - min).abs().max(1e-6);
        let step = span / 20.0;
        let nv = match key {
            UiKey::Left | UiKey::Down => value - step,
            UiKey::Right | UiKey::Up => value + step,
            UiKey::Home => min,
            UiKey::End => max,
            _ => return false,
        };
        self.set_slider_value(id, nv)
    }

    fn key_text_input(&mut self, id: WidgetId, key: UiKey) -> bool {
        let Some(w) = self.widgets.get_mut(&id) else {
            return false;
        };
        let Kind::TextInput {
            value,
            caret,
            select_all,
            preedit,
            preedit_cursor,
            ..
        } = &mut w.kind
        else {
            return false;
        };
        // Composing: let the IME own Backspace/etc.; ignore direct Char inserts.
        if !preedit.is_empty() {
            match key {
                UiKey::Char(_) => return false,
                UiKey::Escape => {
                    preedit.clear();
                    *preedit_cursor = None;
                    return true;
                }
                _ => {
                    // Drop preedit on navigation / delete so state stays coherent if
                    // the OS still delivered the key.
                    preedit.clear();
                    *preedit_cursor = None;
                }
            }
        }
        let mut chars: Vec<char> = value.chars().collect();
        let len = chars.len();
        *caret = (*caret).min(len);
        match key {
            UiKey::SelectAll => {
                *select_all = !chars.is_empty();
                true
            }
            UiKey::Char(c) => {
                if *select_all {
                    chars.clear();
                    *caret = 0;
                    *select_all = false;
                }
                chars.insert(*caret, c);
                *caret += 1;
                *value = chars.into_iter().collect();
                true
            }
            UiKey::Backspace => {
                if *select_all {
                    chars.clear();
                    *caret = 0;
                    *select_all = false;
                    *value = String::new();
                    return true;
                }
                if *caret > 0 {
                    chars.remove(*caret - 1);
                    *caret -= 1;
                    *value = chars.into_iter().collect();
                    true
                } else {
                    false
                }
            }
            UiKey::Delete => {
                if *select_all {
                    chars.clear();
                    *caret = 0;
                    *select_all = false;
                    *value = String::new();
                    return true;
                }
                if *caret < chars.len() {
                    chars.remove(*caret);
                    *value = chars.into_iter().collect();
                    true
                } else {
                    false
                }
            }
            UiKey::Left => {
                *select_all = false;
                if *caret > 0 {
                    *caret -= 1;
                    true
                } else {
                    false
                }
            }
            UiKey::Right => {
                *select_all = false;
                if *caret < len {
                    *caret += 1;
                    true
                } else {
                    false
                }
            }
            UiKey::Home => {
                *select_all = false;
                if *caret != 0 {
                    *caret = 0;
                    true
                } else {
                    false
                }
            }
            UiKey::End => {
                *select_all = false;
                if *caret != len {
                    *caret = len;
                    true
                } else {
                    false
                }
            }
            UiKey::Enter | UiKey::Tab | UiKey::BackTab | UiKey::Space | UiKey::Up | UiKey::Down | UiKey::Escape => false,
        }
    }

    /// Whether id is a TextInput.
    pub fn is_text_input(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::TextInput { .. })
        )
    }

    /// Whether id is a ScrollArea.
    pub fn is_scroll_area(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::ScrollArea { .. })
        )
    }


    fn pointer_input_number(&mut self, scene: &Scene, id: WidgetId, x: f32, _y: f32) -> bool {
        let (style, node, min, max, step) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::InputNumber {
                        min,
                        max,
                        step,
                        style,
                        ..
                    },
                node,
                ..
            }) => (*style, *node, *min, *max, *step),
            _ => return false,
        };
        let bounds = node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        let step_w = 28.0;
        if x < bounds.x + step_w {
            let _ = self.set_input_number_value(
                id,
                self.input_number_value(id).unwrap_or(0.0) - step,
            );
            return true;
        }
        if x > bounds.x + bounds.width - step_w {
            let _ = self.set_input_number_value(
                id,
                self.input_number_value(id).unwrap_or(0.0) + step,
            );
            return true;
        }
        let _ = (min, max, style);
        true
    }

    fn pointer_segmented(&mut self, scene: &Scene, id: WidgetId, x: f32, _y: f32) -> bool {
        let (options, style, node) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Segmented { options, style, .. },
                node,
                ..
            }) => (options.len(), *style, *node),
            _ => return false,
        };
        let bounds = node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        if options == 0 {
            return false;
        }
        let seg_w = ((bounds.width - style.padding * 2.0) / options as f32).max(1.0);
        let idx = ((x - bounds.x - style.padding) / seg_w).floor() as isize;
        if idx >= 0 && (idx as usize) < options {
            return self.set_segmented_selected(id, idx as usize);
        }
        false
    }

    fn pointer_breadcrumb(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let aux = match self.widgets.get(&id) {
            Some(w) => w.aux.clone(),
            None => return false,
        };
        for (i, &nid) in aux.iter().enumerate() {
            if i % 2 != 0 {
                continue; // separators
            }
            if let Some(n) = scene.get(nid) {
                if n.visible && n.bounds.contains(x, y) {
                    self.pending_breadcrumb_click = Some((id, i / 2));
                    return true;
                }
            }
        }
        false
    }

    fn pointer_pagination(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let aux = match self.widgets.get(&id) {
            Some(w) => w.aux.clone(),
            None => return false,
        };
        let page = self.pagination_page(id).unwrap_or(1);
        // aux[0]=prev_bg, aux[2]=next_bg
        if let Some(&prev) = aux.first() {
            if let Some(n) = scene.get(prev) {
                if n.visible && n.bounds.contains(x, y) {
                    if page > 1 {
                        self.set_pagination_page(id, page - 1);
                        self.pending_pagination_change = Some((id, page - 1));
                    }
                    return true;
                }
            }
        }
        if let Some(&next) = aux.get(2) {
            if let Some(n) = scene.get(next) {
                if n.visible && n.bounds.contains(x, y) {
                    self.set_pagination_page(id, page + 1);
                    if let Some(np) = self.pagination_page(id) {
                        self.pending_pagination_change = Some((id, np));
                    }
                    return true;
                }
            }
        }
        false
    }

    fn pointer_collapse(&mut self, scene: &Scene, id: WidgetId, _x: f32, y: f32) -> bool {
        let (panels, style, node) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Collapse { panels, style, .. },
                node,
                ..
            }) => (panels.clone(), *style, *node),
            _ => return false,
        };
        let bounds = node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        let mut cy = bounds.y;
        for (i, p) in panels.iter().enumerate() {
            if y >= cy && y < cy + style.header_height {
                return self.set_collapse_open(id, i, !p.open);
            }
            cy += style.header_height;
            if p.open {
                let (_, bh) = self.font.measure_line(&p.body, style.size);
                cy += style.padding * 2.0 + bh;
            }
        }
        false
    }

    fn key_text_area(&mut self, id: WidgetId, key: UiKey) -> bool {
        let Some(Widget {
            kind:
                Kind::TextArea {
                    value,
                    caret,
                    ..
                },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        match key {
            UiKey::Char(c) => {
                let mut chars: Vec<char> = value.chars().collect();
                let i = (*caret).min(chars.len());
                chars.insert(i, c);
                *value = chars.into_iter().collect();
                *caret = i + 1;
                true
            }
            UiKey::Enter => {
                let mut chars: Vec<char> = value.chars().collect();
                let i = (*caret).min(chars.len());
                chars.insert(i, '\n');
                *value = chars.into_iter().collect();
                *caret = i + 1;
                true
            }
            UiKey::Backspace => {
                let mut chars: Vec<char> = value.chars().collect();
                if *caret > 0 && !chars.is_empty() {
                    let i = (*caret).min(chars.len());
                    chars.remove(i - 1);
                    *value = chars.into_iter().collect();
                    *caret = i - 1;
                    return true;
                }
                false
            }
            UiKey::Delete => {
                let mut chars: Vec<char> = value.chars().collect();
                let i = (*caret).min(chars.len());
                if i < chars.len() {
                    chars.remove(i);
                    *value = chars.into_iter().collect();
                    return true;
                }
                false
            }
            UiKey::Left => {
                if *caret > 0 {
                    *caret -= 1;
                    return true;
                }
                false
            }
            UiKey::Right => {
                let n = value.chars().count();
                if *caret < n {
                    *caret += 1;
                    return true;
                }
                false
            }
            UiKey::Home => {
                let chars: Vec<char> = value.chars().collect();
                let i = (*caret).min(chars.len());
                let start = chars[..i]
                    .iter()
                    .rposition(|c| *c == '\n')
                    .map(|p| p + 1)
                    .unwrap_or(0);
                if *caret != start {
                    *caret = start;
                    return true;
                }
                false
            }
            UiKey::End => {
                let chars: Vec<char> = value.chars().collect();
                let i = (*caret).min(chars.len());
                let end = chars[i..]
                    .iter()
                    .position(|c| *c == '\n')
                    .map(|p| i + p)
                    .unwrap_or(chars.len());
                if *caret != end {
                    *caret = end;
                    return true;
                }
                false
            }
            UiKey::Up => {
                let chars: Vec<char> = value.chars().collect();
                let i = (*caret).min(chars.len());
                let line_start = chars[..i]
                    .iter()
                    .rposition(|c| *c == '\n')
                    .map(|p| p + 1)
                    .unwrap_or(0);
                if line_start == 0 {
                    return false;
                }
                let col = i - line_start;
                let prev_end = line_start - 1;
                let prev_start = chars[..prev_end]
                    .iter()
                    .rposition(|c| *c == '\n')
                    .map(|p| p + 1)
                    .unwrap_or(0);
                let prev_len = prev_end - prev_start;
                *caret = prev_start + col.min(prev_len);
                true
            }
            UiKey::Down => {
                let chars: Vec<char> = value.chars().collect();
                let i = (*caret).min(chars.len());
                let line_start = chars[..i]
                    .iter()
                    .rposition(|c| *c == '\n')
                    .map(|p| p + 1)
                    .unwrap_or(0);
                let col = i - line_start;
                let line_end = chars[i..]
                    .iter()
                    .position(|c| *c == '\n')
                    .map(|p| i + p)
                    .unwrap_or(chars.len());
                if line_end >= chars.len() {
                    return false;
                }
                let next_start = line_end + 1;
                let next_end = chars[next_start..]
                    .iter()
                    .position(|c| *c == '\n')
                    .map(|p| next_start + p)
                    .unwrap_or(chars.len());
                let next_len = next_end - next_start;
                *caret = next_start + col.min(next_len);
                true
            }
            _ => false,
        }
    }

    fn key_input_number(&mut self, id: WidgetId, key: UiKey) -> bool {
        let (step, min, max) = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::InputNumber {
                step, min, max, ..
            }) => (*step, *min, *max),
            _ => return false,
        };
        let cur = self.input_number_value(id).unwrap_or(0.0);
        match key {
            UiKey::Up | UiKey::Right => self.set_input_number_value(id, cur + step),
            UiKey::Down | UiKey::Left => self.set_input_number_value(id, cur - step),
            UiKey::Home => self.set_input_number_value(id, min),
            UiKey::End => self.set_input_number_value(id, max),
            _ => false,
        }
    }

    fn key_segmented(&mut self, id: WidgetId, key: UiKey) -> bool {
        let (sel, n) = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Segmented {
                selected, options, ..
            }) => (*selected, options.len()),
            _ => return false,
        };
        if n == 0 {
            return false;
        }
        match key {
            UiKey::Left | UiKey::Up => {
                let next = if sel == 0 { n - 1 } else { sel - 1 };
                self.set_segmented_selected(id, next)
            }
            UiKey::Right | UiKey::Down => self.set_segmented_selected(id, (sel + 1) % n),
            _ => false,
        }
    }

    fn key_pagination(&mut self, id: WidgetId, key: UiKey) -> bool {
        let page = self.pagination_page(id).unwrap_or(1);
        match key {
            UiKey::Left => {
                if page > 1 {
                    self.set_pagination_page(id, page - 1);
                    self.pending_pagination_change = Some((id, page - 1));
                    return true;
                }
                false
            }
            UiKey::Right => {
                if self.set_pagination_page(id, page + 1) {
                    if let Some(np) = self.pagination_page(id) {
                        self.pending_pagination_change = Some((id, np));
                    }
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    fn pointer_list(&mut self, scene: &Scene, id: WidgetId, _x: f32, y: f32) -> bool {
        let (sy, style, n_items, node) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::List {
                    items,
                    scroll_y,
                    style,
                    ..
                },
                node,
                ..
            }) => (*scroll_y, *style, items.len(), *node),
            _ => return false,
        };
        let bounds = node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        let local_y = y - bounds.y - style.padding + sy;
        let idx = if style.item_height > 0.0 {
            (local_y / style.item_height).floor() as isize
        } else {
            -1
        };
        if idx >= 0 && (idx as usize) < n_items {
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::List { selected, .. } = &mut w.kind {
                    let new = Some(idx as usize);
                    if *selected != new {
                        *selected = new;
                    }
                }
            }
        }
        let (sx, scroll_y) = self.any_scroll_offset(id).unwrap_or((0.0, 0.0));
        self.drag = Some(DragState::Scroll {
            area: id,
            start_x: _x,
            start_y: y,
            start_scroll_x: sx,
            start_scroll_y: scroll_y,
        });
        true
    }

    fn pointer_tree(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let (sy, style, flats, node) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Tree {
                    roots,
                    scroll_y,
                    style,
                    ..
                },
                node,
                ..
            }) => (*scroll_y, *style, flatten_tree(roots), *node),
            _ => return false,
        };
        let bounds = node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        let local_y = y - bounds.y - style.padding + sy;
        let idx = if style.row_height > 0.0 {
            (local_y / style.row_height).floor() as isize
        } else {
            -1
        };
        if idx >= 0 && (idx as usize) < flats.len() {
            let row = &flats[idx as usize];
            let twisty_x = bounds.x + style.padding + row.depth as f32 * style.indent;
            let in_twisty = x >= twisty_x && x < twisty_x + 16.0;
            if in_twisty && row.has_children {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Tree { roots, .. } = &mut w.kind {
                        let _ = toggle_expanded_in_forest(roots, row.id);
                    }
                }
            } else if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Tree { selected, .. } = &mut w.kind {
                    *selected = Some(row.id);
                }
            }
        }
        let (sx, scroll_y) = self.any_scroll_offset(id).unwrap_or((0.0, 0.0));
        self.drag = Some(DragState::Scroll {
            area: id,
            start_x: x,
            start_y: y,
            start_scroll_x: sx,
            start_scroll_y: scroll_y,
        });
        true
    }

    fn pointer_table(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let (sy, style, n_rows, node) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Table {
                    rows,
                    scroll_y,
                    style,
                    ..
                },
                node,
                ..
            }) => (*scroll_y, *style, rows.len(), *node),
            _ => return false,
        };
        let bounds = node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        let body_top = bounds.y + style.header_height;
        if y < body_top {
            // header click — no-op (column resize stub)
            let (sx, scroll_y) = self.any_scroll_offset(id).unwrap_or((0.0, 0.0));
            self.drag = Some(DragState::Scroll {
                area: id,
                start_x: x,
                start_y: y,
                start_scroll_x: sx,
                start_scroll_y: scroll_y,
            });
            return true;
        }
        let local_y = y - body_top + sy;
        let idx = if style.row_height > 0.0 {
            (local_y / style.row_height).floor() as isize
        } else {
            -1
        };
        if idx >= 0 && (idx as usize) < n_rows {
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Table { selected_row, .. } = &mut w.kind {
                    *selected_row = Some(idx as usize);
                }
            }
        }
        let (sx, scroll_y) = self.any_scroll_offset(id).unwrap_or((0.0, 0.0));
        self.drag = Some(DragState::Scroll {
            area: id,
            start_x: x,
            start_y: y,
            start_scroll_x: sx,
            start_scroll_y: scroll_y,
        });
        true
    }

    fn ensure_selected_visible_list(&mut self, id: WidgetId) {
        let (sel, style, scroll_y) = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::List {
                selected,
                style,
                scroll_y,
                ..
            }) => (*selected, *style, *scroll_y),
            _ => return,
        };
        let Some(i) = sel else { return };
        let row_top = style.padding + i as f32 * style.item_height;
        let row_bot = row_top + style.item_height;
        let view_top = scroll_y;
        let view_bot = scroll_y + style.height;
        let mut ny = scroll_y;
        if row_top < view_top {
            ny = row_top;
        } else if row_bot > view_bot {
            ny = row_bot - style.height;
        }
        self.set_any_scroll(id, 0.0, ny);
    }

    fn ensure_selected_visible_tree(&mut self, id: WidgetId) {
        let (sel, style, scroll_y, flats) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Tree {
                    roots,
                    selected,
                    style,
                    scroll_y,
                    ..
                },
                ..
            }) => (*selected, *style, *scroll_y, flatten_tree(roots)),
            _ => return,
        };
        let Some(sid) = sel else { return };
        let Some(i) = flats.iter().position(|r| r.id == sid) else {
            return;
        };
        let row_top = style.padding + i as f32 * style.row_height;
        let row_bot = row_top + style.row_height;
        let mut ny = scroll_y;
        if row_top < scroll_y {
            ny = row_top;
        } else if row_bot > scroll_y + style.height {
            ny = row_bot - style.height;
        }
        self.set_any_scroll(id, 0.0, ny);
    }

    fn ensure_selected_visible_table(&mut self, id: WidgetId) {
        let (sel, style, scroll_y) = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Table {
                selected_row,
                style,
                scroll_y,
                ..
            }) => (*selected_row, *style, *scroll_y),
            _ => return,
        };
        let Some(i) = sel else { return };
        let body_h = style.height - style.header_height;
        let row_top = i as f32 * style.row_height;
        let row_bot = row_top + style.row_height;
        let mut ny = scroll_y;
        if row_top < scroll_y {
            ny = row_top;
        } else if row_bot > scroll_y + body_h {
            ny = row_bot - body_h;
        }
        self.set_any_scroll(id, 0.0, ny);
    }

    fn key_list(&mut self, id: WidgetId, key: UiKey) -> bool {
        let n = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::List { items, .. }) => items.len(),
            _ => return false,
        };
        if n == 0 {
            return false;
        }
        let cur = self.list_selected(id);
        match key {
            UiKey::Down => {
                let next = cur.map(|i| (i + 1).min(n - 1)).unwrap_or(0);
                self.set_list_selected(id, Some(next));
                self.ensure_selected_visible_list(id);
                true
            }
            UiKey::Up => {
                let next = cur.map(|i| i.saturating_sub(1)).unwrap_or(0);
                self.set_list_selected(id, Some(next));
                self.ensure_selected_visible_list(id);
                true
            }
            UiKey::Home => {
                self.set_list_selected(id, Some(0));
                self.ensure_selected_visible_list(id);
                true
            }
            UiKey::End => {
                self.set_list_selected(id, Some(n - 1));
                self.ensure_selected_visible_list(id);
                true
            }
            _ => false,
        }
    }

    fn key_tree(&mut self, id: WidgetId, key: UiKey) -> bool {
        let flats = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Tree { roots, .. }) => flatten_tree(roots),
            _ => return false,
        };
        if flats.is_empty() {
            return false;
        }
        let cur = self.tree_selected(id);
        let idx = cur.and_then(|cid| flats.iter().position(|r| r.id == cid));
        match key {
            UiKey::Down => {
                let next = idx.map(|i| (i + 1).min(flats.len() - 1)).unwrap_or(0);
                self.set_tree_selected(id, Some(flats[next].id));
                self.ensure_selected_visible_tree(id);
                true
            }
            UiKey::Up => {
                let next = idx.map(|i| i.saturating_sub(1)).unwrap_or(0);
                self.set_tree_selected(id, Some(flats[next].id));
                self.ensure_selected_visible_tree(id);
                true
            }
            UiKey::Right => {
                if let Some(i) = idx {
                    let row = &flats[i];
                    if row.has_children && !row.expanded {
                        self.set_tree_expanded(id, row.id, true);
                        return true;
                    }
                    if i + 1 < flats.len() {
                        self.set_tree_selected(id, Some(flats[i + 1].id));
                        self.ensure_selected_visible_tree(id);
                        return true;
                    }
                }
                false
            }
            UiKey::Left => {
                if let Some(i) = idx {
                    let row = &flats[i];
                    if row.has_children && row.expanded {
                        self.set_tree_expanded(id, row.id, false);
                        return true;
                    }
                    // move to parent: previous row with smaller depth
                    if let Some(p) = (0..i).rev().find(|&j| flats[j].depth < row.depth) {
                        self.set_tree_selected(id, Some(flats[p].id));
                        self.ensure_selected_visible_tree(id);
                        return true;
                    }
                }
                false
            }
            UiKey::Home => {
                self.set_tree_selected(id, Some(flats[0].id));
                self.ensure_selected_visible_tree(id);
                true
            }
            UiKey::End => {
                self.set_tree_selected(id, Some(flats[flats.len() - 1].id));
                self.ensure_selected_visible_tree(id);
                true
            }
            _ => false,
        }
    }

    fn key_table(&mut self, id: WidgetId, key: UiKey) -> bool {
        let n = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Table { rows, .. }) => rows.len(),
            _ => return false,
        };
        if n == 0 {
            return false;
        }
        let cur = self.table_selected_row(id);
        match key {
            UiKey::Down => {
                let next = cur.map(|i| (i + 1).min(n - 1)).unwrap_or(0);
                self.set_table_selected_row(id, Some(next));
                self.ensure_selected_visible_table(id);
                true
            }
            UiKey::Up => {
                let next = cur.map(|i| i.saturating_sub(1)).unwrap_or(0);
                self.set_table_selected_row(id, Some(next));
                self.ensure_selected_visible_table(id);
                true
            }
            UiKey::Home => {
                self.set_table_selected_row(id, Some(0));
                self.ensure_selected_visible_table(id);
                true
            }
            UiKey::End => {
                self.set_table_selected_row(id, Some(n - 1));
                self.ensure_selected_visible_table(id);
                true
            }
            _ => false,
        }
    }


    /// Close any open Menu / ContextMenu / Select popup.
    pub fn close_popup(&mut self) -> bool {
        self.close_popup_internal()
    }

    fn close_popup_internal(&mut self) -> bool {
        let Some(id) = self.open_popup.take() else {
            return false;
        };
        if let Some(w) = self.widgets.get_mut(&id) {
            match &mut w.kind {
                Kind::Menu { open, .. }
                | Kind::ContextMenu { open, .. }
                | Kind::Select { open, .. }
                | Kind::TreeSelect { open, .. }
                | Kind::Modal { open, .. }
                | Kind::Drawer { open, .. }
                | Kind::DateTime { open, .. } => {
                    *open = false;
                }
                _ => {}
            }
        }
        true
    }

    fn hide_popup_aux_mut(&self, scene: &mut Scene, id: WidgetId) {
        if let Some(w) = self.widgets.get(&id) {
            for &n in &w.aux {
                scene.set_visible(n, false);
            }
        }
    }

    /// Drain the last activated menu / context-menu item `(owner, item_id)`.
    pub fn take_menu_action(&mut self) -> Option<(WidgetId, u64)> {
        self.pending_menu_action.take()
    }

    /// Drain the last keyboard-activated Button (`Space` / `Enter` while focused).
    ///
    /// Pointer clicks are unchanged: apps keep using [`Ui::hit_widget`] after
    /// [`Ui::pointer_pressed`] returns `false` for buttons.
    pub fn take_button_activate(&mut self) -> Option<WidgetId> {
        self.pending_button_activate.take()
    }

    /// Selected tab index.
    pub fn tabs_selected(&self, id: WidgetId) -> Option<usize> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Tabs { selected, .. }) => Some(*selected),
            _ => None,
        }
    }

    /// Set selected tab index (clamped).
    pub fn set_tabs_selected(&mut self, id: WidgetId, index: usize) -> bool {
        let n = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Tabs { titles, .. }) => titles.len(),
            _ => return false,
        };
        if n == 0 {
            return false;
        }
        let index = index.min(n - 1);
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Tabs { selected, .. } = &mut w.kind {
                if *selected != index {
                    *selected = index;
                    return true;
                }
            }
        }
        false
    }

    /// Selected option index for a Select.
    pub fn select_selected(&self, id: WidgetId) -> Option<usize> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Select { selected, .. }) => *selected,
            _ => None,
        }
    }

    /// Set Select selection.
    pub fn set_select_selected(&mut self, id: WidgetId, index: Option<usize>) -> bool {
        let n = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Select { options, .. }) => options.len(),
            _ => return false,
        };
        if let Some(i) = index {
            if i >= n {
                return false;
            }
        }
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Select { selected, open, .. } = &mut w.kind {
                *selected = index;
                *open = false;
                if self.open_popup == Some(id) {
                    self.open_popup = None;
                }
                return true;
            }
        }
        false
    }

    /// Current slider value.
    pub fn slider_value(&self, id: WidgetId) -> Option<f32> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Slider { value, .. }) => Some(*value),
            _ => None,
        }
    }

    /// Set slider value (clamped to min..=max).
    pub fn set_slider_value(&mut self, id: WidgetId, value: f32) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Slider {
                min, max, value: v, ..
            } = &mut w.kind
            {
                let nv = value.clamp(*min, *max);
                if (*v - nv).abs() < 1e-6 {
                    return false;
                }
                *v = nv;
                return true;
            }
        }
        false
    }

    /// Whether id is a Menu.
    pub fn is_menu(&self, id: WidgetId) -> bool {
        matches!(self.widgets.get(&id).map(|w| &w.kind), Some(Kind::Menu { .. }))
    }

    /// Whether id is a ContextMenu host.
    pub fn is_context_menu(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::ContextMenu { .. })
        )
    }

    /// Whether id is Tabs.
    pub fn is_tabs(&self, id: WidgetId) -> bool {
        matches!(self.widgets.get(&id).map(|w| &w.kind), Some(Kind::Tabs { .. }))
    }

    /// Whether id is a Select.
    pub fn is_select(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Select { .. })
        )
    }

    /// Whether id is a Slider.
    pub fn is_slider(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Slider { .. })
        )
    }

    /// Whether id is a Tag.
    pub fn is_tag(&self, id: WidgetId) -> bool {
        matches!(self.widgets.get(&id).map(|w| &w.kind), Some(Kind::Tag { .. }))
    }

    /// Whether id is a Badge.
    pub fn is_badge(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Badge { .. })
        )
    }

    /// Whether id is a Tooltip host.
    pub fn is_tooltip(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Tooltip { .. })
        )
    }

    /// Whether id is a Modal.
    pub fn is_modal(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Modal { .. })
        )
    }

    /// Whether id is a Progress bar.
    pub fn is_progress(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Progress { .. })
        )
    }

    /// Whether id is an Alert.
    pub fn is_alert(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Alert { .. })
        )
    }

    /// Open or close a Modal (also sets / clears [`Ui`] popup ownership).
    pub fn set_modal_open(&mut self, id: WidgetId, open: bool) -> bool {
        let is_modal = matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Modal { .. })
        );
        if !is_modal {
            return false;
        }
        if open {
            if let Some(pid) = self.open_popup {
                if pid != id {
                    self.close_popup_internal();
                }
            }
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Modal { open: o, .. } = &mut w.kind {
                    *o = true;
                }
            }
            self.open_popup = Some(id);
            self.focus = Some(id);
        } else {
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Modal { open: o, .. } = &mut w.kind {
                    *o = false;
                }
            }
            if self.open_popup == Some(id) {
                self.open_popup = None;
            }
        }
        true
    }

    /// Whether the Modal is open.
    pub fn modal_open(&self, id: WidgetId) -> Option<bool> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Modal { open, .. }) => Some(*open),
            _ => None,
        }
    }

    /// Drain last Modal action `(owner, action)`.
    pub fn take_modal_action(&mut self) -> Option<(WidgetId, ModalAction)> {
        self.pending_modal_action.take()
    }

    /// Drain last Tag close click.
    pub fn take_tag_close(&mut self) -> Option<WidgetId> {
        self.pending_tag_close.take()
    }

    /// Drain last Alert dismiss.
    pub fn take_alert_dismiss(&mut self) -> Option<WidgetId> {
        self.pending_alert_dismiss.take()
    }

    // ----- Office P1 rest: Drawer / Notification / entry / structure APIs -----

    /// Open or close a Drawer (top-level overlay; like Modal).
    pub fn set_drawer_open(&mut self, id: WidgetId, open: bool) -> bool {
        if !matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Drawer { .. })
        ) {
            return false;
        }
        if open {
            if let Some(pid) = self.open_popup {
                if pid != id {
                    self.close_popup_internal();
                }
            }
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Drawer { open: o, .. } = &mut w.kind {
                    *o = true;
                }
            }
            self.open_popup = Some(id);
            self.focus = Some(id);
        } else {
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Drawer { open: o, .. } = &mut w.kind {
                    *o = false;
                }
            }
            if self.open_popup == Some(id) {
                self.open_popup = None;
            }
        }
        true
    }

    /// Whether the Drawer is open.
    pub fn drawer_open(&self, id: WidgetId) -> Option<bool> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Drawer { open, .. }) => Some(*open),
            _ => None,
        }
    }

    /// Drain last Drawer action.
    pub fn take_drawer_action(&mut self) -> Option<(WidgetId, DrawerAction)> {
        self.pending_drawer_action.take()
    }

    /// Declare a top-level Drawer (open via [`Ui::set_drawer_open`]).
    pub fn drawer(
        &mut self,
        title: impl Into<String>,
        body: impl Into<String>,
        style: DrawerStyle,
    ) -> WidgetId {
        let id = self.alloc();
        self.widgets.insert(
            id,
            Widget {
                kind: Kind::Drawer {
                    title: title.into(),
                    body: body.into(),
                    open: false,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.roots.push(id);
        id
    }

    /// Push a notification toast (rendered top-right in sync).
    pub fn notify(&mut self, message: impl Into<String>, tone: NotificationTone) -> u64 {
        let id = self.next_notification_id;
        self.next_notification_id = self.next_notification_id.saturating_add(1);
        self.notifications.push(NotificationItem {
            id,
            message: message.into(),
            tone,
        });
        let max = self.notification_style.max_visible.max(1);
        while self.notifications.len() > max {
            self.notifications.remove(0);
        }
        id
    }

    /// Dismiss a notification by id (or all if `None`).
    pub fn dismiss_notification(&mut self, id: Option<u64>) {
        if let Some(id) = id {
            self.notifications.retain(|n| n.id != id);
        } else {
            self.notifications.clear();
        }
    }

    /// Set notification toast style.
    pub fn set_notification_style(&mut self, style: NotificationStyle) {
        self.notification_style = style;
    }

    /// Drain last breadcrumb click `(owner, index)`.
    pub fn take_breadcrumb_click(&mut self) -> Option<(WidgetId, usize)> {
        self.pending_breadcrumb_click.take()
    }

    /// Drain last pagination page `(owner, page)` (1-based page).
    pub fn take_pagination_change(&mut self) -> Option<(WidgetId, usize)> {
        self.pending_pagination_change.take()
    }

    /// TextArea value.
    pub fn text_area_value(&self, id: WidgetId) -> Option<&str> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::TextArea { value, .. }) => Some(value.as_str()),
            _ => None,
        }
    }

    /// Set TextArea value.
    pub fn set_text_area_value(&mut self, id: WidgetId, value: impl Into<String>) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::TextArea {
                value: v, caret, ..
            } = &mut w.kind
            {
                *v = value.into();
                *caret = v.chars().count();
                return true;
            }
        }
        false
    }

    /// InputNumber value.
    pub fn input_number_value(&self, id: WidgetId) -> Option<f64> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::InputNumber { value, .. }) => Some(*value),
            _ => None,
        }
    }

    /// Set InputNumber value (clamped).
    pub fn set_input_number_value(&mut self, id: WidgetId, value: f64) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::InputNumber {
                value: v, min, max, ..
            } = &mut w.kind
            {
                let nv = value.clamp(*min, *max);
                if (*v - nv).abs() < 1e-12 {
                    return false;
                }
                *v = nv;
                return true;
            }
        }
        false
    }

    /// Segmented selected index.
    pub fn segmented_selected(&self, id: WidgetId) -> Option<usize> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Segmented { selected, .. }) => Some(*selected),
            _ => None,
        }
    }

    /// Set Segmented selected index.
    pub fn set_segmented_selected(&mut self, id: WidgetId, index: usize) -> bool {
        let n = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Segmented { options, .. }) => options.len(),
            _ => return false,
        };
        if n == 0 {
            return false;
        }
        let index = index.min(n - 1);
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Segmented { selected, .. } = &mut w.kind {
                if *selected == index {
                    return false;
                }
                *selected = index;
                return true;
            }
        }
        false
    }

    /// Pagination current page (1-based).
    pub fn pagination_page(&self, id: WidgetId) -> Option<usize> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Pagination { page, .. }) => Some(*page),
            _ => None,
        }
    }

    /// Set pagination page (1-based, clamped).
    pub fn set_pagination_page(&mut self, id: WidgetId, page: usize) -> bool {
        let (page_size, total) = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Pagination {
                page_size, total, ..
            }) => (*page_size, *total),
            _ => return false,
        };
        let pages = ((total + page_size.saturating_sub(1)) / page_size.max(1)).max(1);
        let page = page.clamp(1, pages);
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Pagination { page: p, .. } = &mut w.kind {
                if *p == page {
                    return false;
                }
                *p = page;
                return true;
            }
        }
        false
    }

    /// Toggle Collapse panel open state.
    pub fn set_collapse_open(&mut self, id: WidgetId, index: usize, open: bool) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Collapse { panels, .. } = &mut w.kind {
                if let Some(p) = panels.get_mut(index) {
                    if p.open == open {
                        return false;
                    }
                    p.open = open;
                    return true;
                }
            }
        }
        false
    }

    /// Set Spin spinning flag.
    pub fn set_spinning(&mut self, id: WidgetId, spinning: bool) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Spin { spinning: s, .. } = &mut w.kind {
                if *s == spinning {
                    return false;
                }
                *s = spinning;
                return true;
            }
        }
        false
    }



    // ----- Office P1 deferred: DateTime / Transfer / PropertyGrid / TreeTable -----

    /// Committed DateTime value.
    pub fn datetime_value(&self, id: WidgetId) -> Option<DateTimeValue> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::DateTime { value, .. }) => Some(*value),
            _ => None,
        }
    }

    /// Set DateTime value (closes popup).
    pub fn set_datetime_value(&mut self, id: WidgetId, value: DateTimeValue) -> bool {
        let value = value.clamp_day();
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::DateTime {
                value: v,
                draft,
                open,
                ..
            } = &mut w.kind
            {
                *v = value;
                *draft = value;
                *open = false;
                if self.open_popup == Some(id) {
                    self.open_popup = None;
                }
                return true;
            }
        }
        false
    }

    /// Whether DateTime popup is open.
    pub fn datetime_open(&self, id: WidgetId) -> Option<bool> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::DateTime { open, .. }) => Some(*open),
            _ => None,
        }
    }

    /// Open or close the DateTime picker popup.
    pub fn set_datetime_open(&mut self, id: WidgetId, open: bool) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::DateTime {
                value,
                draft,
                open: o,
                ..
            } = &mut w.kind
            {
                if *o == open {
                    return false;
                }
                if open {
                    *draft = *value;
                    *o = true;
                    self.open_popup = Some(id);
                    self.focus = Some(id);
                } else {
                    *o = false;
                    if self.open_popup == Some(id) {
                        self.open_popup = None;
                    }
                    if self.focus == Some(id) {
                        self.focus = None;
                    }
                }
                return true;
            }
        }
        false
    }

    /// Transfer target (right) items.
    pub fn transfer_target(&self, id: WidgetId) -> Option<&[TransferItem]> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Transfer { target, .. }) => Some(target.as_slice()),
            _ => None,
        }
    }

    /// Transfer source (left) items.
    pub fn transfer_source(&self, id: WidgetId) -> Option<&[TransferItem]> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Transfer { source, .. }) => Some(source.as_slice()),
            _ => None,
        }
    }

    /// Set Transfer filter (source list substring; empty = all).
    pub fn set_transfer_filter(&mut self, id: WidgetId, filter: impl Into<String>) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Transfer { filter: f, .. } = &mut w.kind {
                *f = filter.into();
                return true;
            }
        }
        false
    }

    /// PropertyGrid selected row index.
    pub fn property_grid_selected(&self, id: WidgetId) -> Option<usize> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::PropertyGrid { selected, .. }) => *selected,
            _ => None,
        }
    }

    /// PropertyGrid row value by index.
    pub fn property_grid_value(&self, id: WidgetId, index: usize) -> Option<&PropertyValue> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::PropertyGrid { rows, .. }) => rows.get(index).map(|r| &r.value),
            _ => None,
        }
    }

    /// Set a PropertyGrid row value.
    pub fn set_property_grid_value(
        &mut self,
        id: WidgetId,
        index: usize,
        value: PropertyValue,
    ) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::PropertyGrid { rows, .. } = &mut w.kind {
                if let Some(r) = rows.get_mut(index) {
                    r.value = value;
                    return true;
                }
            }
        }
        false
    }

    /// Selected TreeTable node id.
    pub fn tree_table_selected(&self, id: WidgetId) -> Option<u64> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::TreeTable { selected, .. }) => *selected,
            _ => None,
        }
    }

    /// Set TreeTable selection.
    pub fn set_tree_table_selected(&mut self, id: WidgetId, node: Option<u64>) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::TreeTable { selected, .. } = &mut w.kind {
                if *selected == node {
                    return false;
                }
                *selected = node;
                return true;
            }
        }
        false
    }

    fn pointer_datetime_toggle(&mut self, scene: &mut Scene, id: WidgetId) -> bool {
        let was_open = matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::DateTime { open: true, .. })
        );
        if let Some(pid) = self.open_popup {
            self.hide_popup_aux_mut(scene, pid);
        }
        self.close_popup_internal();
        if was_open {
            return true;
        }
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::DateTime {
                value, draft, open, ..
            } = &mut w.kind
            {
                *draft = *value;
                *open = true;
            }
        }
        self.open_popup = Some(id);
        true
    }

    fn pointer_datetime_popup(&mut self, scene: &mut Scene, id: WidgetId, x: f32, y: f32) -> bool {
        // Click on host while open → toggle close.
        if let Some(w) = self.widgets.get(&id) {
            if let Some(node) = w.node {
                if let Some(n) = scene.get(node) {
                    if n.bounds.contains(x, y) {
                        return self.pointer_datetime_toggle(scene, id);
                    }
                }
            }
        }
        let (draft, style, aux) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::DateTime { draft, style, open: true, .. },
                aux,
                ..
            }) => (*draft, *style, aux.clone()),
            _ => return false,
        };
        let Some(&panel) = aux.first() else {
            return true;
        };
        let Some(pb) = scene.get(panel).map(|n| n.bounds) else {
            return true;
        };
        if !pb.contains(x, y) {
            self.hide_popup_aux_mut(scene, id);
            self.close_popup_internal();
            return true;
        }
        // Layout: header (month nav) ~30, grid 6*cell, time row, confirm row.
        let header_h = 30.0;
        let grid_top = pb.y + header_h;
        let grid_h = style.cell * 6.0;
        let time_y = grid_top + grid_h + 4.0;
        let confirm_y = time_y + 28.0;
        let local_x = x - pb.x;
        let local_y = y - pb.y;

        // Month prev / next in header (left/right thirds of header height).
        if local_y < header_h {
            if local_x < pb.width * 0.25 {
                self.datetime_nudge_month(id, -1);
                return true;
            }
            if local_x > pb.width * 0.75 {
                self.datetime_nudge_month(id, 1);
                return true;
            }
            return true;
        }

        // Calendar grid: day cells.
        if y >= grid_top && y < grid_top + grid_h {
            let col = ((x - pb.x - 8.0) / style.cell).floor() as isize;
            let row = ((y - grid_top) / style.cell).floor() as isize;
            if (0..7).contains(&col) && (0..6).contains(&row) {
                let dim = DateTimeValue::days_in_month(draft.year, draft.month) as i32;
                // Simple: day = row*7+col+1 (no weekday offset MVP).
                let day = (row * 7 + col + 1) as i32;
                if day >= 1 && day <= dim {
                    if let Some(w) = self.widgets.get_mut(&id) {
                        if let Kind::DateTime { draft, .. } = &mut w.kind {
                            draft.day = day as u8;
                        }
                    }
                }
            }
            return true;
        }

        // Time: hour- / hour+ / min- / min+ in four quarters of time row.
        if y >= time_y && y < time_y + 28.0 {
            let q = ((x - pb.x) / (pb.width / 4.0)).floor() as isize;
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::DateTime { draft, .. } = &mut w.kind {
                    match q {
                        0 => draft.hour = draft.hour.saturating_sub(1),
                        1 => draft.hour = (draft.hour + 1).min(23),
                        2 => draft.minute = draft.minute.saturating_sub(1),
                        3 => draft.minute = (draft.minute + 1).min(59),
                        _ => {}
                    }
                }
            }
            return true;
        }

        // Confirm / Cancel bottom row.
        if y >= confirm_y && y < confirm_y + 28.0 {
            if local_x < pb.width * 0.5 {
                // Cancel
                self.hide_popup_aux_mut(scene, id);
                self.close_popup_internal();
            } else {
                // Confirm
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::DateTime {
                        value, draft, open, ..
                    } = &mut w.kind
                    {
                        *value = draft.clamp_day();
                        *draft = *value;
                        *open = false;
                    }
                }
                self.hide_popup_aux_mut(scene, id);
                self.open_popup = None;
            }
            return true;
        }
        true
    }

    fn datetime_nudge_month(&mut self, id: WidgetId, delta: i32) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::DateTime { draft, .. } = &mut w.kind {
                let mut m = draft.month as i32 + delta;
                let mut y = draft.year;
                while m < 1 {
                    m += 12;
                    y -= 1;
                }
                while m > 12 {
                    m -= 12;
                    y += 1;
                }
                draft.year = y;
                draft.month = m as u8;
                *draft = draft.clamp_day();
            }
        }
    }

    fn key_datetime(&mut self, id: WidgetId, key: UiKey) -> bool {
        let open = matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::DateTime { open: true, .. })
        );
        match key {
            UiKey::Enter if open => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::DateTime {
                        value, draft, open, ..
                    } = &mut w.kind
                    {
                        *value = draft.clamp_day();
                        *draft = *value;
                        *open = false;
                    }
                }
                self.open_popup = None;
                true
            }
            UiKey::Escape if open => self.close_popup_internal(),
            UiKey::Space | UiKey::Enter if !open => {
                // Open without scene — caller syncs next frame.
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::DateTime {
                        value, draft, open, ..
                    } = &mut w.kind
                    {
                        *draft = *value;
                        *open = true;
                    }
                }
                self.open_popup = Some(id);
                self.focus = Some(id);
                true
            }
            UiKey::Left if open => {
                self.datetime_nudge_month(id, -1);
                true
            }
            UiKey::Right if open => {
                self.datetime_nudge_month(id, 1);
                true
            }
            UiKey::Up if open => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::DateTime { draft, .. } = &mut w.kind {
                        draft.hour = (draft.hour + 1).min(23);
                    }
                }
                true
            }
            UiKey::Down if open => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::DateTime { draft, .. } = &mut w.kind {
                        draft.hour = draft.hour.saturating_sub(1);
                    }
                }
                true
            }
            _ => false,
        }
    }

    fn pointer_transfer(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let (style, node, source_len, target_len, filter) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::Transfer {
                        source,
                        target,
                        filter,
                        style,
                        ..
                    },
                node,
                ..
            }) => (*style, *node, source.len(), target.len(), filter.clone()),
            _ => return false,
        };
        let bounds = node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        let list_w = style.list_width;
        let btn_w = (style.width - list_w * 2.0 - style.gap * 2.0).max(40.0);
        let left_x = bounds.x;
        let btn_x = left_x + list_w + style.gap;
        let right_x = btn_x + btn_w + style.gap;
        let header_h = 28.0;
        let body_top = bounds.y + header_h;

        // Button column: four buttons stacked.
        if x >= btn_x && x < btn_x + btn_w && y >= body_top {
            let bh = 28.0;
            let gap = 6.0;
            let idx = ((y - body_top) / (bh + gap)).floor() as isize;
            match idx {
                0 => self.transfer_move_selected(id, true),  // >
                1 => self.transfer_move_selected(id, false), // <
                2 => self.transfer_move_all(id, true),       // >>
                3 => self.transfer_move_all(id, false),      // <<
                _ => false,
            };
            return true;
        }

        // Left list
        if x >= left_x && x < left_x + list_w && y >= body_top {
            let row = ((y - body_top) / style.row_height).floor() as isize;
            if row >= 0 {
                let visible = self.transfer_filtered_source_indices(id);
                if let Some(&si) = visible.get(row as usize) {
                    if let Some(w) = self.widgets.get_mut(&id) {
                        if let Kind::Transfer {
                            source_selected, ..
                        } = &mut w.kind
                        {
                            *source_selected = Some(si);
                        }
                    }
                    let _ = (source_len, filter);
                    return true;
                }
            }
            return true;
        }

        // Right list
        if x >= right_x && x < right_x + list_w && y >= body_top {
            let row = ((y - body_top) / style.row_height).floor() as isize;
            if row >= 0 && (row as usize) < target_len {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Transfer {
                        target_selected, ..
                    } = &mut w.kind
                    {
                        *target_selected = Some(row as usize);
                    }
                }
                return true;
            }
            return true;
        }
        true
    }

    fn transfer_filtered_source_indices(&self, id: WidgetId) -> Vec<usize> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Transfer { source, filter, .. }) => {
                let f = filter.to_lowercase();
                source
                    .iter()
                    .enumerate()
                    .filter(|(_, it)| f.is_empty() || it.label.to_lowercase().contains(&f))
                    .map(|(i, _)| i)
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn transfer_move_selected(&mut self, id: WidgetId, to_target: bool) -> bool {
        if to_target {
            let sel = match self.widgets.get(&id).map(|w| &w.kind) {
                Some(Kind::Transfer {
                    source_selected, ..
                }) => *source_selected,
                _ => return false,
            };
            let Some(i) = sel else { return false };
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Transfer {
                    source,
                    target,
                    source_selected,
                    ..
                } = &mut w.kind
                {
                    if i < source.len() {
                        let item = source.remove(i);
                        target.push(item);
                        *source_selected = None;
                        return true;
                    }
                }
            }
        } else {
            let sel = match self.widgets.get(&id).map(|w| &w.kind) {
                Some(Kind::Transfer {
                    target_selected, ..
                }) => *target_selected,
                _ => return false,
            };
            let Some(i) = sel else { return false };
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Transfer {
                    source,
                    target,
                    target_selected,
                    ..
                } = &mut w.kind
                {
                    if i < target.len() {
                        let item = target.remove(i);
                        source.push(item);
                        *target_selected = None;
                        return true;
                    }
                }
            }
        }
        false
    }

    fn transfer_move_all(&mut self, id: WidgetId, to_target: bool) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Transfer {
                source,
                target,
                source_selected,
                target_selected,
                ..
            } = &mut w.kind
            {
                if to_target {
                    target.append(source);
                    *source_selected = None;
                } else {
                    source.append(target);
                    *target_selected = None;
                }
                return true;
            }
        }
        false
    }

    fn key_transfer(&mut self, id: WidgetId, key: UiKey) -> bool {
        match key {
            UiKey::Right | UiKey::Enter => self.transfer_move_selected(id, true),
            UiKey::Left => self.transfer_move_selected(id, false),
            UiKey::Down => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Transfer {
                        source,
                        source_selected,
                        ..
                    } = &mut w.kind
                    {
                        let n = source.len();
                        if n == 0 {
                            return false;
                        }
                        let next = source_selected.map(|i| (i + 1).min(n - 1)).unwrap_or(0);
                        *source_selected = Some(next);
                        return true;
                    }
                }
                false
            }
            UiKey::Up => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Transfer {
                        source_selected, ..
                    } = &mut w.kind
                    {
                        let next = source_selected.map(|i| i.saturating_sub(1)).unwrap_or(0);
                        *source_selected = Some(next);
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn pointer_property_grid(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let (rows_len, style, node, scroll_y) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::PropertyGrid {
                        rows,
                        style,
                        scroll_y,
                        ..
                    },
                node,
                ..
            }) => (rows.len(), *style, *node, *scroll_y),
            _ => return false,
        };
        let bounds = node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        let header = style.row_height;
        if y < bounds.y + header {
            return true;
        }
        let body_y = y - (bounds.y + header) + scroll_y;
        let row = (body_y / style.row_height).floor() as isize;
        if row < 0 || (row as usize) >= rows_len {
            return true;
        }
        let i = row as usize;
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::PropertyGrid { selected, .. } = &mut w.kind {
                *selected = Some(i);
            }
        }
        // Click in value column → interact
        if x >= bounds.x + style.key_width {
            self.property_grid_activate(id, i);
        }
        true
    }

    fn property_grid_activate(&mut self, id: WidgetId, index: usize) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::PropertyGrid { rows, .. } = &mut w.kind {
                if let Some(r) = rows.get_mut(index) {
                    match &mut r.value {
                        PropertyValue::Bool(b) => {
                            *b = !*b;
                            return true;
                        }
                        PropertyValue::Number(n) => {
                            *n += 1.0;
                            return true;
                        }
                        PropertyValue::Select { options, selected } => {
                            if !options.is_empty() {
                                *selected = (*selected + 1) % options.len();
                                return true;
                            }
                        }
                        PropertyValue::Text(_) => {
                            // MVP: no inline edit; selection only.
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn key_property_grid(&mut self, id: WidgetId, key: UiKey) -> bool {
        let n = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::PropertyGrid { rows, .. }) => rows.len(),
            _ => return false,
        };
        if n == 0 {
            return false;
        }
        match key {
            UiKey::Down => {
                let cur = self.property_grid_selected(id).unwrap_or(0);
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::PropertyGrid { selected, .. } = &mut w.kind {
                        *selected = Some((cur + 1).min(n - 1));
                    }
                }
                self.ensure_selected_visible_property_grid(id);
                true
            }
            UiKey::Up => {
                let cur = self.property_grid_selected(id).unwrap_or(0);
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::PropertyGrid { selected, .. } = &mut w.kind {
                        *selected = Some(cur.saturating_sub(1));
                    }
                }
                self.ensure_selected_visible_property_grid(id);
                true
            }
            UiKey::Home => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::PropertyGrid { selected, .. } = &mut w.kind {
                        *selected = Some(0);
                    }
                }
                self.ensure_selected_visible_property_grid(id);
                true
            }
            UiKey::End => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::PropertyGrid { selected, .. } = &mut w.kind {
                        *selected = Some(n - 1);
                    }
                }
                self.ensure_selected_visible_property_grid(id);
                true
            }
            UiKey::Space | UiKey::Enter => {
                if let Some(i) = self.property_grid_selected(id) {
                    self.property_grid_activate(id, i)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn ensure_selected_visible_property_grid(&mut self, id: WidgetId) {
        let (sel, style, scroll_y) = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::PropertyGrid {
                selected,
                style,
                scroll_y,
                ..
            }) => (*selected, *style, *scroll_y),
            _ => return,
        };
        let Some(i) = sel else { return };
        let header = style.row_height;
        let row_top = header + i as f32 * style.row_height;
        let row_bot = row_top + style.row_height;
        let mut ny = scroll_y;
        if row_top < scroll_y {
            ny = row_top;
        } else if row_bot > scroll_y + style.height {
            ny = row_bot - style.height;
        }
        self.set_any_scroll(id, 0.0, ny);
    }

    fn pointer_tree_table(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let (flats, style, node, scroll_y) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::TreeTable {
                        roots,
                        style,
                        scroll_y,
                        ..
                    },
                node,
                ..
            }) => (flatten_tree_table(roots), *style, *node, *scroll_y),
            _ => return false,
        };
        let bounds = node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        if y < bounds.y + style.header_height {
            return true;
        }
        let body_y = y - (bounds.y + style.header_height) + scroll_y;
        let row = (body_y / style.row_height).floor() as isize;
        if row < 0 || (row as usize) >= flats.len() {
            return true;
        }
        let flat = &flats[row as usize];
        // Twisty hit: left indent zone
        let twisty_right =
            bounds.x + style.padding + flat.depth as f32 * style.indent + 18.0;
        if flat.has_children && x < twisty_right {
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::TreeTable { roots, .. } = &mut w.kind {
                    toggle_tree_table_expanded(roots, flat.id);
                }
            }
            return true;
        }
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::TreeTable { selected, .. } = &mut w.kind {
                *selected = Some(flat.id);
            }
        }
        true
    }

    fn key_tree_table(&mut self, id: WidgetId, key: UiKey) -> bool {
        let flats = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::TreeTable { roots, .. }) => flatten_tree_table(roots),
            _ => return false,
        };
        if flats.is_empty() {
            return false;
        }
        let sel = self.tree_table_selected(id);
        let idx = sel.and_then(|s| flats.iter().position(|r| r.id == s));
        match key {
            UiKey::Down => {
                let next = idx.map(|i| (i + 1).min(flats.len() - 1)).unwrap_or(0);
                self.set_tree_table_selected(id, Some(flats[next].id));
                true
            }
            UiKey::Up => {
                let next = idx.map(|i| i.saturating_sub(1)).unwrap_or(0);
                self.set_tree_table_selected(id, Some(flats[next].id));
                true
            }
            UiKey::Left => {
                if let Some(i) = idx {
                    if flats[i].has_children && flats[i].expanded {
                        if let Some(w) = self.widgets.get_mut(&id) {
                            if let Kind::TreeTable { roots, .. } = &mut w.kind {
                                toggle_tree_table_expanded(roots, flats[i].id);
                            }
                        }
                        return true;
                    }
                }
                false
            }
            UiKey::Right | UiKey::Enter => {
                if let Some(i) = idx {
                    if flats[i].has_children && !flats[i].expanded {
                        if let Some(w) = self.widgets.get_mut(&id) {
                            if let Kind::TreeTable { roots, .. } = &mut w.kind {
                                toggle_tree_table_expanded(roots, flats[i].id);
                            }
                        }
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }


    /// Progress value in 0..=1.
    pub fn progress_value(&self, id: WidgetId) -> Option<f32> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Progress { value, .. }) => Some(*value),
            _ => None,
        }
    }

    /// Set Progress value (clamped to 0..=1). Clears indeterminate.
    pub fn set_progress_value(&mut self, id: WidgetId, value: f32) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Progress {
                value: v,
                indeterminate,
                ..
            } = &mut w.kind
            {
                let nv = value.clamp(0.0, 1.0);
                *indeterminate = false;
                if (*v - nv).abs() < 1e-6 {
                    return false;
                }
                *v = nv;
                return true;
            }
        }
        false
    }

    /// Set Alert dismissed visibility (true = hidden).
    pub fn set_alert_dismissed(&mut self, id: WidgetId, dismissed: bool) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Alert {
                dismissed: d,
                ..
            } = &mut w.kind
            {
                if *d == dismissed {
                    return false;
                }
                *d = dismissed;
                return true;
            }
        }
        false
    }

    /// Whether Alert is currently dismissed (hidden).
    pub fn alert_dismissed(&self, id: WidgetId) -> Option<bool> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Alert { dismissed, .. }) => Some(*dismissed),
            _ => None,
        }
    }

    /// Set Badge count / dot kind.
    pub fn set_badge_kind(&mut self, id: WidgetId, kind: BadgeKind) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Badge { kind: k, .. } = &mut w.kind {
                *k = kind;
                return true;
            }
        }
        false
    }

    /// Set Tag label.
    pub fn set_tag_label(&mut self, id: WidgetId, label: impl Into<String>) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Tag { label: l, .. } = &mut w.kind {
                *l = label.into();
                return true;
            }
        }
        false
    }

    /// Declare a top-level Modal (not part of flex layout; open via [`Ui::set_modal_open`]).
    pub fn modal(
        &mut self,
        title: impl Into<String>,
        body: impl Into<String>,
        primary: impl Into<String>,
        secondary: Option<String>,
        style: ModalStyle,
    ) -> WidgetId {
        let id = self.alloc();
        self.widgets.insert(
            id,
            Widget {
                kind: Kind::Modal {
                    title: title.into(),
                    body: body.into(),
                    primary: Some(primary.into()),
                    secondary,
                    open: false,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.roots.push(id);
        id
    }

    fn find_context_menu_ancestor(&self, id: WidgetId) -> Option<WidgetId> {
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::ContextMenu { .. })
        ) {
            return Some(id);
        }
        // Search containers that list this id as child.
        for (wid, w) in &self.widgets {
            let kids = match &w.kind {
                Kind::Column { children, .. }
                | Kind::Row { children, .. }
                | Kind::Toolbar { children, .. }
                | Kind::Panel { children, .. }
                | Kind::StatusBar { children, .. }
                | Kind::Splitter { children, .. }
                | Kind::ScrollArea { children, .. }
                | Kind::ContextMenu { children, .. }
                | Kind::Tabs { children, .. }
                | Kind::Badge { children, .. }
                | Kind::Tooltip { children, .. }
                    | Kind::Form { children, .. }
                    | Kind::Spin { children, .. } => children.as_slice(),
                _ => continue,
            };
            if kids.contains(&id) {
                if matches!(w.kind, Kind::ContextMenu { .. }) {
                    return Some(*wid);
                }
                if let Some(found) = self.find_context_menu_ancestor(*wid) {
                    return Some(found);
                }
            }
        }
        None
    }

    fn open_context_menu_at(&mut self, id: WidgetId, x: f32, y: f32) {
        if self.open_popup != Some(id) {
            self.close_popup_internal();
        }
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::ContextMenu {
                open,
                anchor_x,
                anchor_y,
                ..
            } = &mut w.kind
            {
                *open = true;
                *anchor_x = x;
                *anchor_y = y;
            }
        }
        self.open_popup = Some(id);
    }

    fn pointer_menu_title(&mut self, scene: &mut Scene, id: WidgetId) -> bool {
        let was_open = matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Menu { open: true, .. })
        );
        if let Some(pid) = self.open_popup {
            self.hide_popup_aux_mut(scene, pid);
        }
        self.close_popup_internal();
        if was_open {
            return true;
        }
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Menu { open, .. } = &mut w.kind {
                *open = true;
            }
        }
        self.open_popup = Some(id);
        true
    }

    fn pointer_select_toggle(&mut self, scene: &mut Scene, id: WidgetId) -> bool {
        let was_open = matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Select { open: true, .. })
        );
        if let Some(pid) = self.open_popup {
            self.hide_popup_aux_mut(scene, pid);
        }
        self.close_popup_internal();
        if was_open {
            return true;
        }
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Select { open, .. } = &mut w.kind {
                *open = true;
            }
        }
        self.open_popup = Some(id);
        true
    }

    fn popup_contains(&self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let Some(w) = self.widgets.get(&id) else {
            return false;
        };
        if matches!(
            w.kind,
            Kind::Modal { open: true, .. } | Kind::Drawer { open: true, .. }
        ) {
            // Modal/Drawer owns the whole viewport while open.
            let (vw, vh) = self.viewport;
            return x >= 0.0 && y >= 0.0 && x < vw && y < vh;
        }
        // First aux node is the popup panel background when open.
        if let Some(&panel) = w.aux.first() {
            if let Some(n) = scene.get(panel) {
                if n.visible && n.bounds.contains(x, y) {
                    return true;
                }
            }
        }
        // Also treat title/host click as "inside" for Menu/Select so toggle works.
        if let Some(node) = w.node {
            if let Some(n) = scene.get(node) {
                if n.bounds.contains(x, y) {
                    return true;
                }
            }
        }
        false
    }

    fn pointer_popup_item(&mut self, scene: &mut Scene, id: WidgetId, x: f32, y: f32) -> bool {
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Modal { .. })
        ) {
            return self.pointer_modal(scene, id, x, y);
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Drawer { .. })
        ) {
            return self.pointer_drawer(scene, id, x, y);
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::TreeSelect { .. })
        ) {
            return self.pointer_tree_select_popup(scene, id, x, y);
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::DateTime { .. })
        ) {
            return self.pointer_datetime_popup(scene, id, x, y);
        }
        // Click on title/host while open → toggle close (Menu/Select).
        if let Some(w) = self.widgets.get(&id) {
            if let Some(node) = w.node {
                if let Some(n) = scene.get(node) {
                    if n.bounds.contains(x, y) {
                        match &w.kind {
                            Kind::Menu { .. } => return self.pointer_menu_title(scene, id),
                            Kind::Select { .. } => return self.pointer_select_toggle(scene, id),
                            Kind::TreeSelect { .. } => return self.pointer_tree_select_toggle(scene, id),
                            Kind::DateTime { .. } => return self.pointer_datetime_toggle(scene, id),
                            _ => {}
                        }
                    }
                }
            }
        }

        let (items_len, item_h, panel_bounds, kind_menu) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Menu { items, style, .. },
                aux,
                ..
            }) => {
                let panel = aux.first().and_then(|n| scene.get(*n)).map(|n| n.bounds);
                (
                    items.len(),
                    style.item_height,
                    panel.unwrap_or_else(Rect::empty),
                    0u8,
                )
            }
            Some(Widget {
                kind: Kind::ContextMenu { items, style, .. },
                aux,
                ..
            }) => {
                let panel = aux.first().and_then(|n| scene.get(*n)).map(|n| n.bounds);
                (
                    items.len(),
                    style.item_height,
                    panel.unwrap_or_else(Rect::empty),
                    1u8,
                )
            }
            Some(Widget {
                kind: Kind::Select {
                    options, style, ..
                },
                aux,
                ..
            }) => {
                let panel = aux.first().and_then(|n| scene.get(*n)).map(|n| n.bounds);
                (
                    options.len(),
                    style.item_height,
                    panel.unwrap_or_else(Rect::empty),
                    2u8,
                )
            }
            _ => return false,
        };

        if items_len == 0 || panel_bounds.is_empty() {
            self.close_popup_internal();
            return true;
        }
        let local_y = y - panel_bounds.y;
        let idx = if item_h > 0.0 {
            (local_y / item_h).floor() as isize
        } else {
            -1
        };
        if idx < 0 || (idx as usize) >= items_len {
            return true;
        }
        let i = idx as usize;
        match kind_menu {
            0 | 1 => {
                let (item_id, enabled) = match self.widgets.get(&id).map(|w| &w.kind) {
                    Some(Kind::Menu { items, .. }) | Some(Kind::ContextMenu { items, .. }) => {
                        (items[i].id, items[i].enabled)
                    }
                    _ => return false,
                };
                if enabled {
                    self.pending_menu_action = Some((id, item_id));
                }
                self.hide_popup_aux_mut(scene, id);
                self.close_popup_internal();
                true
            }
            2 => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Select { selected, open, .. } = &mut w.kind {
                        *selected = Some(i);
                        *open = false;
                    }
                }
                self.hide_popup_aux_mut(scene, id);
                self.open_popup = None;
                true
            }
            _ => false,
        }
    }

    fn pointer_tabs(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let (titles, style, node) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Tabs { titles, style, .. },
                node,
                ..
            }) => (titles.clone(), *style, *node),
            _ => return false,
        };
        let bounds = node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        if y > bounds.y + style.strip_height {
            return true; // content click — keep focus
        }
        if titles.is_empty() {
            return false;
        }
        let tab_w = (bounds.width / titles.len() as f32).max(40.0);
        let idx = ((x - bounds.x) / tab_w).floor() as isize;
        if idx < 0 || (idx as usize) >= titles.len() {
            return false;
        }
        self.set_tabs_selected(id, idx as usize)
    }

    fn pointer_slider(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let (style, node) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Slider { style, .. },
                node,
                ..
            }) => (*style, *node),
            _ => return false,
        };
        let bounds = node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        let track = Rect::new(
            bounds.x + style.thumb_size * 0.5,
            bounds.y + (bounds.height - style.track_thickness) * 0.5,
            (bounds.width - style.thumb_size).max(1.0),
            style.track_thickness,
        );
        let _ = y;
        let _ = self.apply_slider_x(id, track, x);
        self.drag = Some(DragState::Slider { slider: id, track });
        true
    }

    fn apply_slider_x(&mut self, id: WidgetId, track: Rect, x: f32) -> bool {
        let (min, max) = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Slider { min, max, .. }) => (*min, *max),
            _ => return false,
        };
        let t = if track.width <= 0.0 {
            0.0
        } else {
            ((x - track.x) / track.width).clamp(0.0, 1.0)
        };
        let value = min + t * (max - min);
        self.set_slider_value(id, value)
    }

    fn update_tooltip_hover_xy(&mut self, x: f32, y: f32) -> bool {
        self.last_pointer = Some((x, y));
        // Redraw when any Tooltip exists so sync can resolve hover with scene.
        self.widgets
            .values()
            .any(|w| matches!(w.kind, Kind::Tooltip { .. }))
    }

    fn find_tooltip_ancestor(&self, id: WidgetId) -> Option<WidgetId> {
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Tooltip { .. })
        ) {
            return Some(id);
        }
        for (wid, w) in &self.widgets {
            let kids = match &w.kind {
                Kind::Column { children, .. }
                | Kind::Row { children, .. }
                | Kind::Toolbar { children, .. }
                | Kind::Panel { children, .. }
                | Kind::StatusBar { children, .. }
                | Kind::Splitter { children, .. }
                | Kind::ScrollArea { children, .. }
                | Kind::ContextMenu { children, .. }
                | Kind::Tabs { children, .. }
                | Kind::Badge { children, .. }
                | Kind::Tooltip { children, .. }
                    | Kind::Form { children, .. } => children.as_slice(),
                _ => continue,
            };
            if kids.contains(&id) {
                if matches!(w.kind, Kind::Tooltip { .. }) {
                    return Some(*wid);
                }
                if let Some(found) = self.find_tooltip_ancestor(*wid) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// Update tooltip hover using scene hit-test.
    pub fn pointer_hover(&mut self, scene: &Scene, x: f32, y: f32) -> bool {
        let tip = self
            .hit_widget(scene, x, y)
            .and_then(|id| self.find_tooltip_ancestor(id));
        if tip != self.tooltip_hover {
            self.tooltip_hover = tip;
            true
        } else {
            false
        }
    }

    fn pointer_tag_close(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let Some(w) = self.widgets.get(&id) else {
            return false;
        };
        let Some(chrome) = w.chrome_node else {
            return false;
        };
        let Some(n) = scene.get(chrome) else {
            return false;
        };
        if n.visible && n.bounds.contains(x, y) {
            self.pending_tag_close = Some(id);
            return true;
        }
        false
    }

    fn pointer_alert_dismiss(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let Some(w) = self.widgets.get(&id) else {
            return false;
        };
        let Some(chrome) = w.chrome_node else {
            return false;
        };
        let Some(n) = scene.get(chrome) else {
            return false;
        };
        if n.visible && n.bounds.contains(x, y) {
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Alert { dismissed, .. } = &mut w.kind {
                    *dismissed = true;
                }
            }
            self.pending_alert_dismiss = Some(id);
            return true;
        }
        false
    }

    fn pointer_modal(&mut self, scene: &mut Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let (style, aux) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Modal { style, open: true, .. },
                aux,
                ..
            }) => (*style, aux.clone()),
            _ => return false,
        };
        // aux layout: 0=scrim, 1=panel, 2=title_bg, 3=title_label, 4=close,
        // 5=body, 6=primary btn, 7=primary label, 8=secondary btn, 9=secondary label
        let hit = |idx: usize| -> bool {
            aux.get(idx)
                .and_then(|n| scene.get(*n))
                .map(|n| n.visible && n.bounds.contains(x, y))
                .unwrap_or(false)
        };
        if hit(4) {
            self.pending_modal_action = Some((id, ModalAction::Dismiss));
            self.hide_popup_aux_mut(scene, id);
            self.close_popup_internal();
            return true;
        }
        if hit(6) {
            self.pending_modal_action = Some((id, ModalAction::Primary));
            self.hide_popup_aux_mut(scene, id);
            self.close_popup_internal();
            return true;
        }
        if hit(8) {
            self.pending_modal_action = Some((id, ModalAction::Secondary));
            self.hide_popup_aux_mut(scene, id);
            self.close_popup_internal();
            return true;
        }
        if hit(1) || hit(2) || hit(3) || hit(5) {
            // Click inside panel: consume, keep open.
            return true;
        }
        // Backdrop
        if style.close_on_backdrop {
            self.pending_modal_action = Some((id, ModalAction::Dismiss));
            self.hide_popup_aux_mut(scene, id);
            self.close_popup_internal();
        }
        true
    }

    fn ensure_overlay_host(&mut self, scene: &mut Scene) -> NodeId {
        if let Some(h) = self.overlay_host {
            if scene.get(h).is_some() {
                scene.raise(h);
                return h;
            }
        }
        let h = scene
            .insert(
                scene.root(),
                Rect::new(0.0, 0.0, 0.0, 0.0),
                Color::rgba(0, 0, 0, 0),
            )
            .expect("overlay host");
        scene.set_visible(h, false);
        scene.set_mask(h, Some(AlphaMask::empty()));
        self.overlay_host = Some(h);
        h
    }

    fn ensure_overlay_aux(
        &mut self,
        scene: &mut Scene,
        id: WidgetId,
        needed: usize,
    ) {
        let host = self.ensure_overlay_host(scene);
        self.ensure_aux_nodes(scene, host, id, needed);
        scene.raise(host);
    }

        fn ensure_aux_nodes(
        &mut self,
        scene: &mut Scene,
        host: NodeId,
        id: WidgetId,
        needed: usize,
    ) {
        let have = self.widgets.get(&id).map(|w| w.aux.len()).unwrap_or(0);
        for _ in have..needed {
            let nid = scene
                .insert(host, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgba(0, 0, 0, 0))
                .expect("insert aux");
            scene.set_visible(nid, false);
            if let Some(w) = self.widgets.get_mut(&id) {
                w.aux.push(nid);
            }
        }
        if let Some(w) = self.widgets.get(&id) {
            for (i, nid) in w.aux.iter().copied().enumerate() {
                if i >= needed {
                    scene.set_visible(nid, false);
                }
            }
        }
    }

    /// Ensure scene nodes exist, run layout, paint labels into the scene.
    pub fn sync(&mut self, scene: &mut Scene) {
        self.scrub_unfocused_ime();
        let parent = scene.root();
        for root in self.roots.clone() {
            self.ensure_mounted(scene, parent, root);
            self.layout_widget(scene, root);
            self.paint_widget(scene, root);
        }
        if let Some((x, y)) = self.last_pointer {
            let tip = self
                .hit_widget(scene, x, y)
                .and_then(|id| self.find_tooltip_ancestor(id));
            self.tooltip_hover = tip;
        }
        self.paint_open_popups(scene);
        self.paint_notifications(scene);
        self.paint_tooltip_overlay(scene);
    }

    fn ensure_mounted(&mut self, scene: &mut Scene, parent: NodeId, id: WidgetId) {
        let already = self.widgets.get(&id).and_then(|w| w.node).is_some();
        if !already {
            let (fill, visible) = match self.widgets.get(&id).map(|w| &w.kind) {
                Some(Kind::Column { style, .. }) => (
                    style.background.unwrap_or(Color::rgba(0, 0, 0, 0)),
                    style.background.is_some(),
                ),
                Some(Kind::Row { style, .. }) => (
                    style.background.unwrap_or(Color::rgba(0, 0, 0, 0)),
                    style.background.is_some(),
                ),
                Some(Kind::Toolbar { style, .. }) => (
                    style.background.unwrap_or(Color::rgba(0, 0, 0, 0)),
                    style.background.is_some(),
                ),
                Some(Kind::Panel { style, .. }) => (
                    style.background.unwrap_or(Color::rgba(0, 0, 0, 0)),
                    style.background.is_some(),
                ),
                Some(Kind::StatusBar { style, .. }) => (
                    style.background.unwrap_or(Color::rgba(0, 0, 0, 0)),
                    style.background.is_some(),
                ),
                Some(Kind::Splitter { style, .. }) => (
                    style.background.unwrap_or(Color::rgba(0, 0, 0, 0)),
                    style.background.is_some(),
                ),
                Some(Kind::Text { color, .. }) => (*color, true),
                Some(Kind::Button { bg, .. }) => (*bg, true),
                Some(Kind::Icon { color, .. }) => (*color, true),
                Some(Kind::Spacer { .. }) => (Color::rgba(0, 0, 0, 0), false),
                Some(Kind::Separator { color, .. }) => (*color, true),
                Some(Kind::ScrollArea { style, .. }) => (
                    style.background.unwrap_or(Color::rgba(0, 0, 0, 0)),
                    style.background.is_some(),
                ),
                Some(Kind::TextInput { style, .. }) => (style.bg, true),
                Some(Kind::Checkbox { style, .. }) => (style.box_bg, true),
                Some(Kind::Radio { style, .. }) => (style.ring, true),
                Some(Kind::Switch { style, .. }) => (style.track_off, true),
                Some(Kind::List { style, .. }) => (style.bg, true),
                Some(Kind::Tree { style, .. }) => (style.bg, true),
                Some(Kind::Table { style, .. }) => (style.bg, true),
                Some(Kind::Menu { style, .. }) => (style.title_bg, true),
                Some(Kind::ContextMenu { style, .. }) => (style.host_bg, true),
                Some(Kind::Tabs { style, .. }) => (style.content_bg, true),
                Some(Kind::Select { style, .. }) => (style.bg, true),
                Some(Kind::Slider { style, .. }) => (style.track, true),
                Some(Kind::Tag { style, .. }) => (style.bg, true),
                Some(Kind::Badge { style, .. }) => (style.bg, true),
                Some(Kind::Tooltip { .. }) => (Color::rgba(0, 0, 0, 0), false),
                Some(Kind::Modal { .. }) => (Color::rgba(0, 0, 0, 0), false),
                Some(Kind::Progress { style, .. }) => (style.track, true),
                Some(Kind::Alert { style, tone, .. }) => {
                    let bg = match tone {
                        AlertTone::Info => style.info_bg,
                        AlertTone::Success => style.success_bg,
                        AlertTone::Warning => style.warning_bg,
                        AlertTone::Error => style.error_bg,
                    };
                    (bg, true)
                }
                Some(Kind::Form { style, .. }) => (
                    style.background.unwrap_or(Color::rgba(0, 0, 0, 0)),
                    style.background.is_some(),
                ),
                Some(Kind::TreeSelect { style, .. }) => (style.bg, true),
                Some(Kind::AlarmList { style, .. }) => (style.bg, true),
                Some(Kind::Gauge { .. }) => (Color::rgba(0, 0, 0, 0), false),
                Some(Kind::Trend { style, .. }) => (style.bg, true),
                Some(Kind::Chart { style, .. }) => (style.bg, true),
                Some(Kind::Drawer { .. }) => (Color::rgba(0, 0, 0, 0), false),
                Some(Kind::TextArea { style, .. }) => (style.bg, true),
                Some(Kind::InputNumber { style, .. }) => (style.bg, true),
                Some(Kind::Spin { .. }) => (Color::rgba(0, 0, 0, 0), false),
                Some(Kind::Empty { style, .. }) => (style.bg, true),
                Some(Kind::Collapse { style, .. }) => (style.body_bg, true),
                Some(Kind::Descriptions { style, .. }) => (style.bg, true),
                Some(Kind::Card { style, .. }) => (style.bg, true),
                Some(Kind::Segmented { style, .. }) => (style.bg, true),
                Some(Kind::Breadcrumb { .. }) => (Color::rgba(0, 0, 0, 0), false),
                Some(Kind::Pagination { .. }) => (Color::rgba(0, 0, 0, 0), false),
                Some(Kind::DateTime { style, .. }) => (style.bg, true),
                Some(Kind::Transfer { style, .. }) => (style.bg, true),
                Some(Kind::PropertyGrid { style, .. }) => (style.bg, true),
                Some(Kind::TreeTable { style, .. }) => (style.bg, true),
                None => return,
            };

            let node = scene
                .insert(parent, Rect::new(0.0, 0.0, 1.0, 1.0), fill)
                .expect("insert widget node");
            if !visible {
                scene.set_visible(node, false);
            }
            if let Some(w) = self.widgets.get_mut(&id) {
                w.node = Some(node);
            }

            match self.widgets.get(&id).map(|w| &w.kind) {
                Some(Kind::Button { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert label node");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                    }
                }
                Some(Kind::Panel { style, .. }) => {
                    let title_bg = style.title_bg;
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), title_bg)
                        .expect("insert panel title bar");
                    let label = scene
                        .insert(chrome, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert panel title label");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.chrome_node = Some(chrome);
                        w.label_node = Some(label);
                    }
                }
                Some(Kind::Splitter { style, .. }) => {
                    let sash = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), style.sash_color)
                        .expect("insert splitter sash");
                    self.node_to_widget.insert(sash, id);
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.chrome_node = Some(sash);
                    }
                }
                Some(Kind::ScrollArea { .. }) => {
                    self.node_to_widget.insert(node, id);
                }
                Some(Kind::TextInput { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert textinput label");
                    let caret = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0x3b, 0x82, 0xf6))
                        .expect("insert caret");
                    let underline = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0x3b, 0x82, 0xf6))
                        .expect("insert preedit underline");
                    scene.set_visible(underline, false);
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                        w.chrome_node = Some(caret);
                        w.aux = vec![underline];
                    }
                }
                Some(Kind::Checkbox { .. }) | Some(Kind::Radio { .. }) | Some(Kind::Switch { .. }) => {
                    self.node_to_widget.insert(node, id);
                    // Empty mask: host is hit-target only (no solid fill).
                    scene.set_mask(node, Some(AlphaMask::empty()));
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert control label");
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert control chrome");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                        w.chrome_node = Some(chrome);
                    }
                }
                Some(Kind::List { .. }) | Some(Kind::Tree { .. }) | Some(Kind::Table { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0x2f, 0x5b, 0xa8))
                        .expect("insert data chrome");
                    scene.set_visible(chrome, false);
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.chrome_node = Some(chrome);
                    }
                }
                Some(Kind::Menu { .. }) | Some(Kind::Select { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert menu/select label");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                    }
                }
                Some(Kind::ContextMenu { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert ctx hint");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                    }
                }
                Some(Kind::Tabs { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0x2c, 0x2c, 0x34))
                        .expect("insert tabs strip");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.chrome_node = Some(chrome);
                    }
                }
                Some(Kind::Slider { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0x3b, 0x82, 0xf6))
                        .expect("insert slider fill");
                    let thumb = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0xe8, 0xec, 0xf4))
                        .expect("insert slider thumb");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.chrome_node = Some(chrome);
                        w.label_node = Some(thumb);
                    }
                }
                Some(Kind::Tag { .. }) | Some(Kind::Alert { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert tag/alert label");
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert tag/alert close");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                        w.chrome_node = Some(chrome);
                    }
                }
                Some(Kind::Badge { .. }) => {
                    self.node_to_widget.insert(node, id);
                    scene.set_mask(node, Some(AlphaMask::empty()));
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert badge label");
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0xe0, 0x4f, 0x5f))
                        .expect("insert badge chrome");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                        w.chrome_node = Some(chrome);
                    }
                }
                Some(Kind::Tooltip { .. }) => {
                    self.node_to_widget.insert(node, id);
                    scene.set_mask(node, Some(AlphaMask::empty()));
                }
                Some(Kind::Modal { .. }) => {
                    // Overlay-only when open; host stays invisible.
                    scene.set_visible(node, false);
                }
                Some(Kind::Progress { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0x3b, 0x82, 0xf6))
                        .expect("insert progress fill");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.chrome_node = Some(chrome);
                    }
                }
                Some(Kind::TreeSelect { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert tree_select label");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                    }
                }
                Some(Kind::AlarmList { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0x2f, 0x5b, 0xa8))
                        .expect("insert alarm chrome");
                    scene.set_visible(chrome, false);
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.chrome_node = Some(chrome);
                    }
                }
                Some(Kind::Gauge { .. }) => {
                    self.node_to_widget.insert(node, id);
                    scene.set_mask(node, Some(AlphaMask::empty()));
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0x3b, 0xc4, 0x8c))
                        .expect("insert gauge dial");
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert gauge value");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.chrome_node = Some(chrome);
                        w.label_node = Some(label);
                    }
                }
                Some(Kind::Trend { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0x1a, 0x40, 0x55))
                        .expect("insert trend fill");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.chrome_node = Some(chrome);
                    }
                }
                Some(Kind::Chart { .. }) => {
                    self.node_to_widget.insert(node, id);
                }
                Some(Kind::TextArea { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert textarea label");
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0x3b, 0x82, 0xf6))
                        .expect("insert textarea chrome");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                        w.chrome_node = Some(chrome);
                    }
                }
                Some(Kind::InputNumber { .. })
                | Some(Kind::Segmented { .. })
                | Some(Kind::Breadcrumb { .. })
                | Some(Kind::Pagination { .. })
                | Some(Kind::Collapse { .. })
                | Some(Kind::Descriptions { .. })
                | Some(Kind::DateTime { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert p1 label");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                    }
                }
                Some(Kind::Transfer { .. })
                | Some(Kind::PropertyGrid { .. })
                | Some(Kind::TreeTable { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0x2f, 0x5b, 0xa8))
                        .expect("insert deferred chrome");
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert deferred label");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.chrome_node = Some(chrome);
                        w.label_node = Some(label);
                    }
                }
                Some(Kind::Empty { .. }) | Some(Kind::Card { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0x32, 0x32, 0x3c))
                        .expect("insert card/empty chrome");
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert card/empty label");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.chrome_node = Some(chrome);
                        w.label_node = Some(label);
                    }
                }
                Some(Kind::Spin { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0x14, 0x14, 0x18))
                        .expect("insert spin overlay");
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert spin tip");
                    scene.set_visible(chrome, false);
                    scene.set_visible(label, false);
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.chrome_node = Some(chrome);
                        w.label_node = Some(label);
                    }
                }
                Some(Kind::Form { .. }) | Some(Kind::Drawer { .. }) => {}
                _ => {}
            }
        }

        let (host, children) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::Column { children, .. }
                    | Kind::Row { children, .. }
                    | Kind::Toolbar { children, .. }
                    | Kind::Panel { children, .. }
                    | Kind::StatusBar { children, .. }
                    | Kind::Splitter { children, .. }
                    | Kind::ScrollArea { children, .. }
                    | Kind::ContextMenu { children, .. }
                    | Kind::Tabs { children, .. }
                    | Kind::Badge { children, .. }
                    | Kind::Tooltip { children, .. }
                    | Kind::Form { children, .. }
                    | Kind::Spin { children, .. },
                node,
                ..
            }) => (node.unwrap_or(parent), children.clone()),
            _ => return,
        };
        for child in children {
            self.ensure_mounted(scene, host, child);
        }
    }

    fn measure_widget(&self, id: WidgetId) -> ChildMeasure {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Text { content, size, .. }) => {
                let (w, h) = self.font.measure_line(content, *size);
                // Small slack so descenders / AA rows are not flush-clipped by parents.
                ChildMeasure::new(w, h + 6.0)
            }
            Some(Kind::Button {
                label,
                size,
                min_width,
                height,
                ..
            }) => {
                let (tw, th) = self.font.measure_line(label, *size);
                let pad_x = 16.0;
                // Keep authored height, but never shorter than font line box + padding
                // (avoids clipping after correct baseline placement).
                let pad_y = 10.0;
                ChildMeasure::new(
                    (*min_width).max(tw + pad_x * 2.0),
                    (*height).max(th + pad_y),
                )
            }
            Some(Kind::Icon { size, .. }) => ChildMeasure::new(*size, *size),
            Some(Kind::Spacer { width, height }) => ChildMeasure::new(*width, *height),
            Some(Kind::Separator {
                thickness,
                length,
                horizontal,
                ..
            }) => {
                if *horizontal {
                    ChildMeasure::new(*length, *thickness)
                } else {
                    ChildMeasure::new(*thickness, *length)
                }
            }
            Some(Kind::Column {
                style, children, ..
            }) => self.measure_flex_axis(
                Axis::Vertical,
                style.width,
                style.height,
                style.padding,
                style.gap,
                style.cross_align,
                children,
            ),
            Some(Kind::Row {
                style, children, ..
            }) => self.measure_flex_axis(
                Axis::Horizontal,
                style.width,
                style.height,
                style.padding,
                style.gap,
                style.cross_align,
                children,
            ),
            Some(Kind::Toolbar {
                style, children, ..
            }) => {
                let m = self.measure_flex_axis(
                    Axis::Horizontal,
                    style.width,
                    style.height,
                    style.padding,
                    style.gap,
                    style.cross_align,
                    children,
                );
                ChildMeasure::new(m.width.max(style.width * 0.25), style.height.max(m.height))
            }
            Some(Kind::StatusBar {
                style, children, ..
            }) => {
                let m = self.measure_flex_axis(
                    Axis::Horizontal,
                    style.width,
                    style.height,
                    style.padding,
                    style.gap,
                    style.cross_align,
                    children,
                );
                ChildMeasure::new(m.width.max(style.width * 0.25), style.height.max(m.height))
            }
            Some(Kind::Panel {
                style, children, ..
            }) => {
                let content = self.measure_flex_axis(
                    Axis::Vertical,
                    (style.width - style.padding * 2.0).max(0.0),
                    (style.height - style.title_height - style.padding * 2.0).max(0.0),
                    0.0,
                    style.gap,
                    style.cross_align,
                    children,
                );
                ChildMeasure::new(
                    style.width.max(content.width + style.padding * 2.0),
                    (style.title_height + content.height + style.padding * 2.0)
                        .max(style.height * 0.5),
                )
            }
            Some(Kind::Splitter { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::ScrollArea { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::TextInput { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Checkbox { label, style, .. }) => {
                let (tw, th) = self.font.measure_line(label, style.size);
                let h = style.box_size.max(th + 4.0);
                let w = style.box_size + style.gap + tw;
                ChildMeasure::new(w, h)
            }
            Some(Kind::Radio { label, style, .. }) => {
                let (tw, th) = self.font.measure_line(label, style.size);
                let h = style.diameter.max(th + 4.0);
                let w = style.diameter + style.gap + tw;
                ChildMeasure::new(w, h)
            }
            Some(Kind::Switch { label, style, .. }) => {
                let (tw, th) = self.font.measure_line(label, style.size);
                let h = style.height.max(th + 4.0);
                let w = style.width + style.gap + tw;
                ChildMeasure::new(w, h)
            }
            Some(Kind::List { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Tree { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Table { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Menu { title, style, .. }) => {
                let (tw, _) = self.font.measure_line(title, style.size);
                ChildMeasure::new(style.min_width.max(tw + 24.0), style.item_height)
            }
            Some(Kind::ContextMenu { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Tabs { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Select { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Slider { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Tag { label, closable, style }) => {
                let (tw, _) = self.font.measure_line(label, style.size);
                let close_w = if *closable { style.height * 0.7 } else { 0.0 };
                ChildMeasure::new(
                    tw + style.padding_x * 2.0 + close_w,
                    style.height,
                )
            }
            Some(Kind::Badge { kind, style, children }) => {
                if children.is_empty() {
                    match kind {
                        BadgeKind::Dot => ChildMeasure::new(style.dot_size, style.dot_size),
                        BadgeKind::Count(c) => {
                            let text = if *c > 99 {
                                "99+".to_string()
                            } else {
                                c.to_string()
                            };
                            let (tw, _) = self.font.measure_line(&text, style.size);
                            let w = (tw + style.padding_x * 2.0).max(style.height);
                            ChildMeasure::new(w, style.height)
                        }
                    }
                } else {
                    let m = self.measure_widget(children[0]);
                    ChildMeasure::new(
                        m.width + style.offset,
                        m.height + style.offset,
                    )
                }
            }
            Some(Kind::Tooltip { children, .. }) => {
                if let Some(c) = children.first() {
                    self.measure_widget(*c)
                } else {
                    ChildMeasure::new(0.0, 0.0)
                }
            }
            Some(Kind::Modal { .. }) => ChildMeasure::new(0.0, 0.0),
            Some(Kind::Progress { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Alert {
                dismissed,
                message,
                dismissible,
                style,
                ..
            }) => {
                if *dismissed {
                    ChildMeasure::new(0.0, 0.0)
                } else {
                    let (tw, th) = self.font.measure_line(message, style.size);
                    let close = if *dismissible { 24.0 } else { 0.0 };
                    ChildMeasure::new(
                        style.width.max(tw + style.padding * 2.0 + close),
                        style.height.max(th + style.padding),
                    )
                }
            }
            Some(Kind::Form {
                style, children, ..
            }) => {
                let m = self.measure_flex_axis(
                    Axis::Vertical,
                    style.width,
                    style.height,
                    style.padding,
                    style.gap,
                    style.cross_align,
                    children,
                );
                ChildMeasure::new(style.width.max(m.width), m.height.max(40.0))
            }
            Some(Kind::TreeSelect { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::AlarmList { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Gauge { style, label, .. }) => {
                let (_, lh) = self.font.measure_line(label, style.label_size);
                ChildMeasure::new(style.size, style.size + lh + 6.0)
            }
            Some(Kind::Trend { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Chart { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Drawer { .. }) => ChildMeasure::new(0.0, 0.0),
            Some(Kind::TextArea { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::InputNumber { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Spin { style, children, .. }) => {
                if let Some(c) = children.first() {
                    let m = self.measure_widget(*c);
                    ChildMeasure::new(style.width.max(m.width), style.height.max(m.height))
                } else {
                    ChildMeasure::new(style.width, style.height)
                }
            }
            Some(Kind::Empty { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Collapse { panels, style, .. }) => {
                let mut h = 0.0;
                for p in panels {
                    h += style.header_height;
                    if p.open {
                        let (_, bh) = self.font.measure_line(&p.body, style.size);
                        h += style.padding * 2.0 + bh;
                    }
                }
                ChildMeasure::new(style.width, h.max(style.header_height))
            }
            Some(Kind::Descriptions { items, style, .. }) => ChildMeasure::new(
                style.width,
                style.padding * 2.0 + items.len() as f32 * style.row_height,
            ),
            Some(Kind::Card { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Segmented { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Breadcrumb { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Pagination { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::DateTime { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Transfer { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::PropertyGrid { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::TreeTable { style, .. }) => ChildMeasure::new(style.width, style.height),
            None => ChildMeasure::new(0.0, 0.0),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn measure_flex_axis(
        &self,
        axis: Axis,
        max_width: f32,
        max_height: f32,
        padding: f32,
        gap: f32,
        cross_align: CrossAlign,
        children: &[WidgetId],
    ) -> ChildMeasure {
        let measures: Vec<_> = children.iter().map(|c| self.measure_widget(*c)).collect();
        let params = FlexParams {
            axis,
            x: 0.0,
            y: 0.0,
            max_width,
            max_height,
            padding,
            gap,
            cross_align,
        };
        next_gui_layout::measure_flex(&params, &measures)
    }

    fn layout_widget(&mut self, scene: &mut Scene, id: WidgetId) {
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Splitter { .. })
        ) {
            self.layout_splitter(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::ScrollArea { .. })
        ) {
            self.layout_scroll_area(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Panel { .. })
        ) {
            self.layout_panel(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Tabs { .. })
        ) {
            self.layout_tabs(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::ContextMenu { .. })
        ) {
            self.layout_context_menu_host(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Badge { .. }) | Some(Kind::Tooltip { .. })
        ) {
            self.layout_host_child(scene, id);
            return;
        }

        let flex = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Column {
                    style, children, ..
                },
                node,
                ..
            }) => Some((
                FlexParams {
                    axis: Axis::Vertical,
                    x: style.x,
                    y: style.y,
                    max_width: style.width,
                    max_height: style.height,
                    padding: style.padding,
                    gap: style.gap,
                    cross_align: style.cross_align,
                },
                style.background.is_some(),
                children.clone(),
                *node,
            )),
            Some(Widget {
                kind: Kind::Form {
                    style, children, ..
                },
                node,
                ..
            }) => Some((
                FlexParams {
                    axis: Axis::Vertical,
                    x: style.x,
                    y: style.y,
                    max_width: style.width,
                    max_height: style.height,
                    padding: style.padding,
                    gap: style.gap,
                    cross_align: style.cross_align,
                },
                style.background.is_some(),
                children.clone(),
                *node,
            )),
            Some(Widget {
                kind: Kind::Row {
                    style, children, ..
                },
                node,
                ..
            }) => Some((
                FlexParams {
                    axis: Axis::Horizontal,
                    x: style.x,
                    y: style.y,
                    max_width: style.width,
                    max_height: style.height,
                    padding: style.padding,
                    gap: style.gap,
                    cross_align: style.cross_align,
                },
                style.background.is_some(),
                children.clone(),
                *node,
            )),
            Some(Widget {
                kind: Kind::Toolbar {
                    style, children, ..
                },
                node,
                ..
            }) => {
                let row = style.to_row();
                Some((
                    FlexParams {
                        axis: Axis::Horizontal,
                        x: row.x,
                        y: row.y,
                        max_width: row.width,
                        max_height: row.height,
                        padding: row.padding,
                        gap: row.gap,
                        cross_align: row.cross_align,
                    },
                    row.background.is_some(),
                    children.clone(),
                    *node,
                ))
            }
            Some(Widget {
                kind: Kind::StatusBar {
                    style, children, ..
                },
                node,
                ..
            }) => {
                let row = style.to_row();
                Some((
                    FlexParams {
                        axis: Axis::Horizontal,
                        x: row.x,
                        y: row.y,
                        max_width: row.width,
                        max_height: row.height,
                        padding: row.padding,
                        gap: row.gap,
                        cross_align: row.cross_align,
                    },
                    row.background.is_some(),
                    children.clone(),
                    *node,
                ))
            }
            _ => None,
        };

        let Some((params, has_bg, children, col_node)) = flex else {
            return;
        };

        let measures: Vec<_> = children
            .iter()
            .map(|c| self.measure_widget(*c))
            .collect();
        let total = next_gui_layout::measure_flex(&params, &measures);
        if let Some(node) = col_node {
            let (ox, oy) = scene
                .get(node)
                .map(|n| {
                    if self.roots.contains(&id) {
                        (params.x, params.y)
                    } else {
                        (n.bounds.x, n.bounds.y)
                    }
                })
                .unwrap_or((params.x, params.y));
            let mut placed = params;
            placed.x = ox;
            placed.y = oy;
            let out_w = match self.widgets.get(&id).map(|w| &w.kind) {
                Some(Kind::Toolbar { style, .. }) => style.width.max(total.width),
                Some(Kind::StatusBar { style, .. }) => style.width.max(total.width),
                _ => total.width.max(40.0),
            };
            let out_h = match self.widgets.get(&id).map(|w| &w.kind) {
                Some(Kind::Toolbar { style, .. }) => style.height.max(total.height),
                Some(Kind::StatusBar { style, .. }) => style.height.max(total.height),
                Some(Kind::Row { style, .. }) => style.height.max(total.height),
                _ => total.height.max(24.0),
            };
            placed.max_width = out_w;
            placed.max_height = out_h;
            scene.set_bounds(node, Rect::new(ox, oy, out_w, out_h));
            if has_bg {
                scene.set_visible(node, true);
            }
            let rects = layout_flex(&placed, &measures);
            for (child_id, rect) in children.iter().copied().zip(rects) {
                self.place_child(scene, child_id, rect);
            }
        }
    }

    fn place_child(&mut self, scene: &mut Scene, child_id: WidgetId, rect: Rect) {
        if let Some(w) = self.widgets.get(&child_id) {
            if let Some(node) = w.node {
                scene.set_bounds(node, rect);
                let visible = !matches!(w.kind, Kind::Spacer { .. });
                scene.set_visible(node, visible);
            }
        }
        if self.is_data_scrollable(child_id)
            && !matches!(
                self.widgets.get(&child_id).map(|w| &w.kind),
                Some(Kind::ScrollArea { .. })
            )
        {
            self.rebase_data_size(child_id, rect.width, rect.height);
        }
        if self.is_container(child_id) {
            self.rebase_container_origin(child_id, rect.x, rect.y);
            self.rebase_container_size(child_id, rect.width, rect.height);
            self.layout_widget(scene, child_id);
        }
    }

    fn rebase_data_size(&mut self, id: WidgetId, width: f32, height: f32) {
        if let Some(w) = self.widgets.get_mut(&id) {
            match &mut w.kind {
                Kind::List { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::Tree { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::Table { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::AlarmList { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                _ => {}
            }
        }
    }

    fn is_container(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(
                Kind::Column { .. }
                    | Kind::Row { .. }
                    | Kind::Toolbar { .. }
                    | Kind::Panel { .. }
                    | Kind::StatusBar { .. }
                    | Kind::Splitter { .. }
                    | Kind::ScrollArea { .. }
                    | Kind::ContextMenu { .. }
                    | Kind::Tabs { .. }
                    | Kind::Badge { .. }
                    | Kind::Tooltip { .. }
                    | Kind::Form { .. }
                    | Kind::Spin { .. }
            )
        )
    }

    fn layout_panel(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Panel {
                style, children, ..
            },
            node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let style = *style;
        let children = children.clone();
        let col_node = *node;
        let chrome = *chrome_node;

        let (ox, oy) = if let Some(node) = col_node {
            if self.roots.contains(&id) {
                (style.x, style.y)
            } else {
                scene
                    .get(node)
                    .map(|n| (n.bounds.x, n.bounds.y))
                    .unwrap_or((style.x, style.y))
            }
        } else {
            (style.x, style.y)
        };

        let (w, h) = if let Some(node) = col_node {
            if self.roots.contains(&id) {
                (style.width, style.height)
            } else {
                scene
                    .get(node)
                    .map(|n| {
                        (
                            n.bounds.width.max(style.width * 0.5),
                            n.bounds.height.max(style.height * 0.5),
                        )
                    })
                    .unwrap_or((style.width, style.height))
            }
        } else {
            (style.width, style.height)
        };

        if let Some(node) = col_node {
            scene.set_bounds(node, Rect::new(ox, oy, w, h));
            if style.background.is_some() {
                scene.set_visible(node, true);
            }
        }
        if let Some(chrome) = chrome {
            scene.set_bounds(chrome, Rect::new(ox, oy, w, style.title_height));
            scene.set_fill(chrome, style.title_bg);
            scene.set_visible(chrome, true);
        }

        let content_y = oy + style.title_height;
        let content_h = (h - style.title_height).max(0.0);
        let params = FlexParams {
            axis: Axis::Vertical,
            x: ox,
            y: content_y,
            max_width: w,
            max_height: content_h,
            padding: style.padding,
            gap: style.gap,
            cross_align: style.cross_align,
        };
        let measures: Vec<_> = children.iter().map(|c| self.measure_widget(*c)).collect();
        let rects = layout_flex(&params, &measures);
        for (child_id, rect) in children.iter().copied().zip(rects) {
            self.place_child(scene, child_id, rect);
        }
    }


    fn layout_tabs(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Tabs {
                    titles,
                    selected,
                    style,
                    children,
                },
            node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let style = *style;
        let selected = *selected;
        let children = children.clone();
        let titles = titles.clone();
        let col_node = *node;
        let chrome = *chrome_node;

        let (ox, oy, w, h) = if let Some(node) = col_node {
            scene
                .get(node)
                .map(|n| {
                    if self.roots.contains(&id) {
                        (0.0, 0.0, style.width, style.height)
                    } else {
                        (
                            n.bounds.x,
                            n.bounds.y,
                            n.bounds.width.max(40.0),
                            n.bounds.height.max(40.0),
                        )
                    }
                })
                .unwrap_or((0.0, 0.0, style.width, style.height))
        } else {
            (0.0, 0.0, style.width, style.height)
        };

        if let Some(node) = col_node {
            scene.set_bounds(node, Rect::new(ox, oy, w, h));
            scene.set_fill(node, style.content_bg);
            scene.set_visible(node, true);
        }
        if let Some(chrome) = chrome {
            scene.set_bounds(chrome, Rect::new(ox, oy, w, style.strip_height));
            scene.set_fill(chrome, style.strip_bg);
            scene.set_mask(chrome, None);
            scene.set_visible(chrome, true);
        }

        let content_y = oy + style.strip_height;
        let content_h = (h - style.strip_height).max(0.0);
        for (i, child_id) in children.iter().copied().enumerate() {
            if i == selected {
                self.place_child(
                    scene,
                    child_id,
                    Rect::new(ox, content_y, w, content_h),
                );
            } else if let Some(w) = self.widgets.get(&child_id) {
                if let Some(n) = w.node {
                    scene.set_visible(n, false);
                }
            }
        }
        let _ = titles;
    }

    fn layout_context_menu_host(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::ContextMenu {
                    style, children, ..
                },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let style = *style;
        let children = children.clone();
        let col_node = *node;
        let (ox, oy, w, h) = if let Some(node) = col_node {
            if self.roots.contains(&id) {
                (0.0, 0.0, style.width, style.height)
            } else {
                scene
                    .get(node)
                    .map(|n| (n.bounds.x, n.bounds.y, n.bounds.width.max(40.0), n.bounds.height.max(40.0)))
                    .unwrap_or((0.0, 0.0, style.width, style.height))
            }
        } else {
            (0.0, 0.0, style.width, style.height)
        };
        if let Some(node) = col_node {
            scene.set_bounds(node, Rect::new(ox, oy, w, h));
            scene.set_fill(node, style.host_bg);
            scene.set_visible(node, true);
        }
        let params = FlexParams {
            axis: Axis::Vertical,
            x: ox,
            y: oy,
            max_width: w,
            max_height: h,
            padding: 8.0,
            gap: 6.0,
            cross_align: CrossAlign::Start,
        };
        let measures: Vec<_> = children.iter().map(|c| self.measure_widget(*c)).collect();
        let rects = layout_flex(&params, &measures);
        for (child_id, rect) in children.iter().copied().zip(rects) {
            self.place_child(scene, child_id, rect);
        }
    }

    fn layout_host_child(&mut self, scene: &mut Scene, id: WidgetId) {
        let (children, pad) = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Badge { children, style, .. }) => (children.clone(), style.offset),
            Some(Kind::Tooltip { children, .. }) => (children.clone(), 0.0),
            _ => return,
        };
        let host = self.widgets.get(&id).and_then(|w| w.node);
        let (ox, oy, w, h) = if let Some(node) = host {
            scene
                .get(node)
                .map(|n| (n.bounds.x, n.bounds.y, n.bounds.width, n.bounds.height))
                .unwrap_or((0.0, 0.0, 0.0, 0.0))
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };
        if let Some(node) = host {
            // Transparent host hit target.
            scene.set_mask(node, Some(AlphaMask::empty()));
            scene.set_visible(node, true);
        }
        if let Some(child) = children.first().copied() {
            let cw = (w - pad).max(1.0);
            let ch = (h - pad).max(1.0);
            self.place_child(scene, child, Rect::new(ox, oy, cw, ch));
        }
    }

    fn layout_splitter(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Splitter { style, children },
            node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let style = *style;
        let children = children.clone();
        let col_node = *node;
        let sash = *chrome_node;

        let (ox, oy) = if let Some(node) = col_node {
            if self.roots.contains(&id) {
                (style.x, style.y)
            } else {
                scene
                    .get(node)
                    .map(|n| (n.bounds.x, n.bounds.y))
                    .unwrap_or((style.x, style.y))
            }
        } else {
            (style.x, style.y)
        };
        let (w, h) = if let Some(node) = col_node {
            if self.roots.contains(&id) {
                (style.width, style.height)
            } else {
                scene
                    .get(node)
                    .map(|n| {
                        (
                            if n.bounds.width > 1.0 {
                                n.bounds.width
                            } else {
                                style.width
                            },
                            if n.bounds.height > 1.0 {
                                n.bounds.height
                            } else {
                                style.height
                            },
                        )
                    })
                    .unwrap_or((style.width, style.height))
            }
        } else {
            (style.width, style.height)
        };

        if let Some(node) = col_node {
            scene.set_bounds(node, Rect::new(ox, oy, w, h));
            if style.background.is_some() {
                scene.set_visible(node, true);
            } else {
                scene.set_visible(node, false);
            }
        }

        let ratio = style.ratio.clamp(0.05, 0.95);
        let sash_size = style.sash_size.max(2.0);

        let (first_rect, sash_rect, second_rect) = match style.axis {
            Axis::Vertical => {
                let avail = (w - sash_size).max(0.0);
                let first_w = (avail * ratio).round();
                let second_w = (avail - first_w).max(0.0);
                (
                    Rect::new(ox, oy, first_w, h),
                    Rect::new(ox + first_w, oy, sash_size, h),
                    Rect::new(ox + first_w + sash_size, oy, second_w, h),
                )
            }
            Axis::Horizontal => {
                let avail = (h - sash_size).max(0.0);
                let first_h = (avail * ratio).round();
                let second_h = (avail - first_h).max(0.0);
                (
                    Rect::new(ox, oy, w, first_h),
                    Rect::new(ox, oy + first_h, w, sash_size),
                    Rect::new(ox, oy + first_h + sash_size, w, second_h),
                )
            }
        };

        if let Some(sash) = sash {
            scene.set_bounds(sash, sash_rect);
            scene.set_fill(sash, style.sash_color);
            scene.set_visible(sash, true);
        }

        if let Some(c0) = children.first().copied() {
            self.place_child(scene, c0, first_rect);
        }
        if let Some(c1) = children.get(1).copied() {
            self.place_child(scene, c1, second_rect);
        }
    }


    fn layout_scroll_area(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::ScrollArea {
                    style,
                    children,
                    scroll_x,
                    scroll_y,
                    ..
                },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let style = *style;
        let children = children.clone();
        let scroll_x = *scroll_x;
        let scroll_y = *scroll_y;
        let col_node = *node;

        let (ox, oy) = if let Some(node) = col_node {
            if self.roots.contains(&id) {
                (style.x, style.y)
            } else {
                scene
                    .get(node)
                    .map(|n| (n.bounds.x, n.bounds.y))
                    .unwrap_or((style.x, style.y))
            }
        } else {
            (style.x, style.y)
        };
        let (w, h) = if let Some(node) = col_node {
            if self.roots.contains(&id) {
                (style.width, style.height)
            } else {
                scene
                    .get(node)
                    .map(|n| {
                        (
                            if n.bounds.width > 1.0 {
                                n.bounds.width
                            } else {
                                style.width
                            },
                            if n.bounds.height > 1.0 {
                                n.bounds.height
                            } else {
                                style.height
                            },
                        )
                    })
                    .unwrap_or((style.width, style.height))
            }
        } else {
            (style.width, style.height)
        };

        if let Some(node) = col_node {
            scene.set_bounds(node, Rect::new(ox, oy, w, h));
            scene.set_clip(node, Some(Rect::new(ox, oy, w, h)));
            if style.background.is_some() {
                scene.set_visible(node, true);
            } else {
                scene.set_visible(node, false);
            }
        }

        let params = FlexParams {
            axis: Axis::Vertical,
            x: ox - scroll_x,
            y: oy - scroll_y,
            max_width: (w - style.padding * 2.0).max(0.0),
            max_height: 1_000_000.0,
            padding: style.padding,
            gap: style.gap,
            cross_align: style.cross_align,
        };
        let measures: Vec<_> = children.iter().map(|c| self.measure_widget(*c)).collect();
        let total = next_gui_layout::measure_flex(&params, &measures);
        let rects = layout_flex(&params, &measures);
        for (child_id, rect) in children.iter().copied().zip(rects) {
            self.place_child(scene, child_id, rect);
        }

        let content_w = total.width.max(w);
        let content_h = total.height.max(h);
        let max_x = (content_w - w).max(0.0);
        let max_y = (content_h - h).max(0.0);
        if let Some(wdig) = self.widgets.get_mut(&id) {
            if let Kind::ScrollArea {
                scroll_x: sx,
                scroll_y: sy,
                content_w: cw,
                content_h: ch,
                ..
            } = &mut wdig.kind
            {
                *cw = content_w;
                *ch = content_h;
                *sx = sx.clamp(0.0, max_x);
                *sy = sy.clamp(0.0, max_y);
            }
        }
    }

    fn rebase_container_origin(&mut self, id: WidgetId, x: f32, y: f32) {
        if let Some(w) = self.widgets.get_mut(&id) {
            match &mut w.kind {
                Kind::Column { style, .. } => {
                    style.x = x;
                    style.y = y;
                }
                Kind::Row { style, .. } => {
                    style.x = x;
                    style.y = y;
                }
                Kind::Toolbar { style, .. } => {
                    style.x = x;
                    style.y = y;
                }
                Kind::Panel { style, .. } => {
                    style.x = x;
                    style.y = y;
                }
                Kind::StatusBar { style, .. } => {
                    style.x = x;
                    style.y = y;
                }
                Kind::Splitter { style, .. } => {
                    style.x = x;
                    style.y = y;
                }
                Kind::ScrollArea { style, .. } => {
                    style.x = x;
                    style.y = y;
                }
                Kind::Form { style, .. } => {
                    style.x = x;
                    style.y = y;
                }
                _ => {}
            }
        }
    }

    fn rebase_container_size(&mut self, id: WidgetId, width: f32, height: f32) {
        if let Some(w) = self.widgets.get_mut(&id) {
            match &mut w.kind {
                Kind::Column { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::Row { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::Toolbar { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::Panel { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::StatusBar { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::Splitter { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::ScrollArea { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::Tabs { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::ContextMenu { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::Form { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                _ => {}
            }
        }
    }

    fn paint_widget(&mut self, scene: &mut Scene, id: WidgetId) {
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Spin { .. })
        ) {
            self.paint_spin(scene, id);
            return;
        }
        let container_children = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(
                Kind::Column { children, .. }
                | Kind::Row { children, .. }
                | Kind::Toolbar { children, .. }
                | Kind::Panel { children, .. }
                | Kind::StatusBar { children, .. }
                | Kind::Splitter { children, .. }
                | Kind::ScrollArea { children, .. }
                | Kind::ContextMenu { children, .. }
                | Kind::Tabs { children, .. }
                | Kind::Badge { children, .. }
                | Kind::Tooltip { children, .. }
                | Kind::Form { children, .. },
            ) => Some(children.clone()),
            _ => None,
        };
        if let Some(children) = container_children {
            if matches!(
                self.widgets.get(&id).map(|w| &w.kind),
                Some(Kind::Panel { .. })
            ) {
                self.paint_panel_title(scene, id);
            }
            if matches!(
                self.widgets.get(&id).map(|w| &w.kind),
                Some(Kind::Tabs { .. })
            ) {
                self.paint_tabs_strip(scene, id);
            }
            if matches!(
                self.widgets.get(&id).map(|w| &w.kind),
                Some(Kind::ContextMenu { .. })
            ) {
                self.paint_context_menu_hint(scene, id);
            }
            let selected_tab = match self.widgets.get(&id).map(|w| &w.kind) {
                Some(Kind::Tabs { selected, .. }) => Some(*selected),
                _ => None,
            };
            for (i, child) in children.into_iter().enumerate() {
                if let Some(sel) = selected_tab {
                    if i != sel {
                        continue;
                    }
                }
                self.paint_widget(scene, child);
            }
            if matches!(
                self.widgets.get(&id).map(|w| &w.kind),
                Some(Kind::Badge { .. })
            ) {
                self.paint_badge_overlay(scene, id);
            }
            return;
        }

        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::List { .. })
        ) {
            self.paint_list(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Tree { .. })
        ) {
            self.paint_tree(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Table { .. })
        ) {
            self.paint_table(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Menu { .. })
        ) {
            self.paint_menu_title(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Select { .. })
        ) {
            self.paint_select_field(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::TreeSelect { .. })
        ) {
            self.paint_tree_select_field(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::AlarmList { .. })
        ) {
            self.paint_alarm_list(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Gauge { .. })
        ) {
            self.paint_gauge(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Trend { .. })
        ) {
            self.paint_trend(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Chart { .. })
        ) {
            self.paint_chart(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::TextArea { .. })
        ) {
            self.paint_text_area(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::InputNumber { .. })
        ) {
            self.paint_input_number(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Empty { .. })
        ) {
            self.paint_empty(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Card { .. })
        ) {
            self.paint_card(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Segmented { .. })
        ) {
            self.paint_segmented(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Breadcrumb { .. })
        ) {
            self.paint_breadcrumb(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Pagination { .. })
        ) {
            self.paint_pagination(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Collapse { .. })
        ) {
            self.paint_collapse(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Descriptions { .. })
        ) {
            self.paint_descriptions(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::DateTime { .. })
        ) {
            self.paint_datetime_field(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Transfer { .. })
        ) {
            self.paint_transfer(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::PropertyGrid { .. })
        ) {
            self.paint_property_grid(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::TreeTable { .. })
        ) {
            self.paint_tree_table(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Slider { .. })
        ) {
            self.paint_slider(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Tag { .. })
        ) {
            self.paint_tag(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Progress { .. })
        ) {
            self.paint_progress(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Alert { .. })
        ) {
            self.paint_alert(scene, id);
            return;
        }
        // Standalone Badge (no children) painted here.
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Badge { children, .. }) if children.is_empty()
        ) {
            self.paint_badge_overlay(scene, id);
            return;
        }

        let Some(w) = self.widgets.get(&id) else {
            return;
        };
        let Some(node) = w.node else {
            return;
        };
        let label_node = w.label_node;
        let bounds = scene
            .get(node)
            .map(|n| n.bounds)
            .unwrap_or_else(Rect::empty);

        match &w.kind {
            Kind::Text {
                content,
                color,
                size,
            } => {
                let content = content.clone();
                let color = *color;
                let size = *size;
                let (_, mask) = self.font.rasterize_text(&content, size, None);
                scene.set_fill(node, color);
                scene.set_mask(
                    node,
                    Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                );
                let bw = bounds.width.max(mask.width as f32);
                let bh = bounds.height.max(mask.height as f32);
                scene.set_bounds(node, Rect::new(bounds.x, bounds.y, bw, bh));
            }
            Kind::Button {
                label,
                fg,
                size,
                bg,
                ..
            } => {
                let label = label.clone();
                let fg = *fg;
                let size = *size;
                let bg = *bg;
                scene.set_fill(node, bg);
                scene.set_mask(node, None);
                let (_, mask) = self.font.rasterize_text(&label, size, None);
                if let Some(lid) = label_node {
                    let lx = bounds.x + 16.0;
                    let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
                    scene.set_bounds(
                        lid,
                        Rect::new(lx, ly, mask.width as f32, mask.height as f32),
                    );
                    scene.set_fill(lid, fg);
                    scene.set_mask(
                        lid,
                        Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                    );
                    scene.set_visible(lid, true);
                }
            }
            Kind::Icon { name, size, color } => {
                let name = *name;
                let size = *size;
                let color = *color;
                let mask = rasterize_icon(name, size);
                scene.set_fill(node, color);
                scene.set_mask(node, Some(mask.clone()));
                scene.set_bounds(
                    node,
                    Rect::new(bounds.x, bounds.y, mask.width as f32, mask.height as f32),
                );
                scene.set_visible(node, true);
            }
            Kind::Separator { color, .. } => {
                scene.set_fill(node, *color);
                scene.set_mask(node, None);
                scene.set_visible(node, true);
            }
            Kind::TextInput {
                value,
                placeholder,
                caret,
                select_all,
                preedit,
                preedit_cursor,
                style,
            } => {
                let value = value.clone();
                let placeholder = placeholder.clone();
                let caret = *caret;
                let select_all = *select_all;
                let preedit = preedit.clone();
                let preedit_cursor = *preedit_cursor;
                let style = *style;
                let focused = self.focus == Some(id);
                let border = if focused {
                    style.focus_border
                } else {
                    style.border
                };
                scene.set_fill(node, style.bg);
                scene.set_mask(node, None);
                let _ = border;
                let before: String = value.chars().take(caret).collect();
                let after: String = value.chars().skip(caret).collect();
                let composing = focused && !preedit.is_empty();
                let (display, fg) = if !value.is_empty() || composing {
                    let mut s = String::with_capacity(value.len() + preedit.len());
                    s.push_str(&before);
                    if composing {
                        s.push_str(&preedit);
                    }
                    s.push_str(&after);
                    (s, style.fg)
                } else {
                    (placeholder.clone(), style.placeholder_fg)
                };
                let (_, mask) = self.font.rasterize_text(&display, style.size, None);
                let lx = bounds.x + style.padding_x;
                let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
                if let Some(lid) = label_node {
                    scene.set_bounds(
                        lid,
                        Rect::new(lx, ly, mask.width as f32, mask.height as f32),
                    );
                    scene.set_fill(lid, fg);
                    if select_all && !value.is_empty() && !composing {
                        scene.set_fill(lid, Color::rgb(0xff, 0xff, 0xff));
                    }
                    scene.set_mask(
                        lid,
                        Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                    );
                    scene.set_visible(lid, true);
                }
                // Preedit underline (MVP): bar under the composition span.
                let underline = self.widgets.get(&id).and_then(|w| w.aux.first().copied());
                if let Some(uid) = underline {
                    if composing {
                        let (bw, _) = self.font.measure_line(&before, style.size);
                        let (pw, _) = self.font.measure_line(&preedit, style.size);
                        let uy = (ly + mask.height as f32 + 1.0).min(bounds.y + bounds.height - 2.0);
                        scene.set_bounds(uid, Rect::new(lx + bw, uy, pw.max(1.0), 2.0));
                        scene.set_fill(uid, style.focus_border);
                        scene.set_mask(uid, None);
                        scene.set_visible(uid, true);
                    } else {
                        scene.set_visible(uid, false);
                    }
                }
                if let Some(cid) = self.widgets.get(&id).and_then(|w| w.chrome_node) {
                    if focused {
                        let caret_chars = if composing {
                            let pre_chars = preedit.chars().count();
                            let at = match preedit_cursor {
                                Some((b, _)) => {
                                    // byte index → char index inside preedit
                                    preedit
                                        .get(..b.min(preedit.len()))
                                        .map(|s| s.chars().count())
                                        .unwrap_or(pre_chars)
                                }
                                None => pre_chars,
                            };
                            before.chars().count() + at
                        } else {
                            caret
                        };
                        let prefix: String = display.chars().take(caret_chars).collect();
                        let (cw, _) = self.font.measure_line(&prefix, style.size);
                        let cx = bounds.x + style.padding_x + cw;
                        let cy = bounds.y + 4.0;
                        let ch = (bounds.height - 8.0).max(8.0);
                        // Hide caret when IME says so (cursor = None) during preedit.
                        let show_caret = !(composing && preedit_cursor.is_none());
                        scene.set_bounds(cid, Rect::new(cx, cy, 2.0, ch));
                        scene.set_fill(cid, style.focus_border);
                        scene.set_mask(cid, None);
                        scene.set_visible(cid, show_caret);
                    } else {
                        scene.set_visible(cid, false);
                    }
                }
                scene.set_visible(node, true);
            }
            Kind::Checkbox {
                label,
                checked,
                style,
            } => {
                let label = label.clone();
                let checked = *checked;
                let style = *style;
                let box_rect = Rect::new(
                    bounds.x,
                    bounds.y + ((bounds.height - style.box_size) * 0.5).max(0.0),
                    style.box_size,
                    style.box_size,
                );
                scene.set_bounds(node, box_rect);
                scene.set_fill(
                    node,
                    if checked {
                        style.checked_bg
                    } else {
                        style.box_bg
                    },
                );
                scene.set_mask(node, None);
                scene.set_visible(node, true);
                if let Some(chrome) = self.widgets.get(&id).and_then(|w| w.chrome_node) {
                    if checked {
                        let (_, cm) = self.font.rasterize_text("✓", style.box_size * 0.7, None);
                        let cx =
                            box_rect.x + ((box_rect.width - cm.width as f32) * 0.5).max(0.0);
                        let cy =
                            box_rect.y + ((box_rect.height - cm.height as f32) * 0.5).max(0.0);
                        scene.set_bounds(
                            chrome,
                            Rect::new(cx, cy, cm.width as f32, cm.height as f32),
                        );
                        scene.set_fill(chrome, style.check_fg);
                        scene.set_mask(
                            chrome,
                            Some(AlphaMask::new(cm.width, cm.height, cm.pixels)),
                        );
                        scene.set_visible(chrome, true);
                    } else {
                        scene.set_visible(chrome, false);
                    }
                }
                let (_, mask) = self.font.rasterize_text(&label, style.size, None);
                if let Some(lid) = label_node {
                    let lx = bounds.x + style.box_size + style.gap;
                    let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
                    scene.set_bounds(
                        lid,
                        Rect::new(lx, ly, mask.width as f32, mask.height as f32),
                    );
                    scene.set_fill(lid, style.fg);
                    scene.set_mask(
                        lid,
                        Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                    );
                    scene.set_visible(lid, true);
                }
            }
            Kind::Radio {
                label,
                selected,
                style,
                ..
            } => {
                let label = label.clone();
                let selected = *selected;
                let style = *style;
                let d = style.diameter;
                let ring = Rect::new(
                    bounds.x,
                    bounds.y + ((bounds.height - d) * 0.5).max(0.0),
                    d,
                    d,
                );
                scene.set_bounds(node, ring);
                scene.set_fill(node, style.ring);
                scene.set_mask(node, None);
                scene.set_visible(node, true);
                if let Some(chrome) = self.widgets.get(&id).and_then(|w| w.chrome_node) {
                    if selected {
                        let inset = d * 0.28;
                        scene.set_bounds(
                            chrome,
                            Rect::new(
                                ring.x + inset,
                                ring.y + inset,
                                d - inset * 2.0,
                                d - inset * 2.0,
                            ),
                        );
                        scene.set_fill(chrome, style.dot);
                        scene.set_mask(chrome, None);
                        scene.set_visible(chrome, true);
                    } else {
                        scene.set_visible(chrome, false);
                    }
                }
                let (_, mask) = self.font.rasterize_text(&label, style.size, None);
                if let Some(lid) = label_node {
                    let lx = bounds.x + d + style.gap;
                    let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
                    scene.set_bounds(
                        lid,
                        Rect::new(lx, ly, mask.width as f32, mask.height as f32),
                    );
                    scene.set_fill(lid, style.fg);
                    scene.set_mask(
                        lid,
                        Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                    );
                    scene.set_visible(lid, true);
                }
            }
            Kind::Switch { label, on, style } => {
                let label = label.clone();
                let on = *on;
                let style = *style;
                let track = Rect::new(
                    bounds.x,
                    bounds.y + ((bounds.height - style.height) * 0.5).max(0.0),
                    style.width,
                    style.height,
                );
                scene.set_bounds(node, track);
                scene.set_fill(node, if on { style.track_on } else { style.track_off });
                scene.set_mask(node, None);
                scene.set_visible(node, true);
                if let Some(chrome) = self.widgets.get(&id).and_then(|w| w.chrome_node) {
                    let thumb_d = style.height - 4.0;
                    let tx = if on {
                        track.x + track.width - thumb_d - 2.0
                    } else {
                        track.x + 2.0
                    };
                    let ty = track.y + 2.0;
                    scene.set_bounds(chrome, Rect::new(tx, ty, thumb_d, thumb_d));
                    scene.set_fill(chrome, style.thumb);
                    scene.set_mask(chrome, None);
                    scene.set_visible(chrome, true);
                }
                let (_, mask) = self.font.rasterize_text(&label, style.size, None);
                if let Some(lid) = label_node {
                    let lx = bounds.x + style.width + style.gap;
                    let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
                    scene.set_bounds(
                        lid,
                        Rect::new(lx, ly, mask.width as f32, mask.height as f32),
                    );
                    scene.set_fill(lid, style.fg);
                    scene.set_mask(
                        lid,
                        Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                    );
                    scene.set_visible(lid, true);
                }
            }
            Kind::Spacer { .. }
            | Kind::Column { .. }
            | Kind::Row { .. }
            | Kind::Toolbar { .. }
            | Kind::Panel { .. }
            | Kind::StatusBar { .. }
            | Kind::Splitter { .. }
            | Kind::ScrollArea { .. }
            | Kind::List { .. }
            | Kind::Tree { .. }
            | Kind::Table { .. }
            | Kind::Menu { .. }
            | Kind::ContextMenu { .. }
            | Kind::Tabs { .. }
            | Kind::Select { .. }
            | Kind::Slider { .. }
            | Kind::Tag { .. }
            | Kind::Badge { .. }
            | Kind::Tooltip { .. }
            | Kind::Modal { .. }
            | Kind::Progress { .. }
            | Kind::Alert { .. }
            | Kind::Form { .. }
            | Kind::TreeSelect { .. }
            | Kind::AlarmList { .. }
            | Kind::Gauge { .. }
            | Kind::Trend { .. }
            | Kind::Chart { .. }
            | Kind::Drawer { .. }
            | Kind::TextArea { .. }
            | Kind::InputNumber { .. }
            | Kind::Spin { .. }
            | Kind::Empty { .. }
            | Kind::Collapse { .. }
            | Kind::Descriptions { .. }
            | Kind::Card { .. }
            | Kind::Segmented { .. }
            | Kind::Breadcrumb { .. }
            | Kind::Pagination { .. }
            | Kind::DateTime { .. }
            | Kind::Transfer { .. }
            | Kind::PropertyGrid { .. }
            | Kind::TreeTable { .. } => {}
        }
    }


    fn paint_list(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::List {
                    items,
                    selected,
                    scroll_y,
                    virtualized,
                    style,
                },
            node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let items = items.clone();
        let selected = *selected;
        let scroll_y = *scroll_y;
        let virtualized = *virtualized;
        let style = *style;
        let host = match *node {
            Some(n) => n,
            None => return,
        };
        let chrome = *chrome_node;
        let bounds = scene
            .get(host)
            .map(|n| n.bounds)
            .unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_clip(host, Some(bounds));
        scene.set_visible(host, true);

        let (first, count) = if virtualized {
            p1_rest::virtual_window(
                scroll_y,
                (bounds.height - style.padding * 2.0).max(0.0),
                style.item_height,
                items.len(),
                2,
            )
        } else {
            (0, items.len())
        };
        let needed = count * 2;
        self.ensure_aux_nodes(scene, host, id, needed);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();

        if let Some(chrome) = chrome {
            if let Some(sel) = selected {
                let y = bounds.y + style.padding + sel as f32 * style.item_height - scroll_y;
                let onscreen = y + style.item_height >= bounds.y - 1.0
                    && y <= bounds.y + bounds.height + 1.0;
                if onscreen {
                    scene.set_bounds(
                        chrome,
                        Rect::new(
                            bounds.x + 1.0,
                            y,
                            (bounds.width - 2.0).max(1.0),
                            style.item_height,
                        ),
                    );
                    scene.set_fill(chrome, style.selected_bg);
                    scene.set_mask(chrome, None);
                    scene.set_visible(chrome, true);
                } else {
                    scene.set_visible(chrome, false);
                }
            } else {
                scene.set_visible(chrome, false);
            }
        }

        for slot in 0..count {
            let i = first + slot;
            let Some(text) = items.get(i) else { break };
            let y = bounds.y + style.padding + i as f32 * style.item_height - scroll_y;
            let row = Rect::new(
                bounds.x + style.padding,
                y,
                (bounds.width - style.padding * 2.0).max(1.0),
                style.item_height,
            );
            if !virtualized
                && (row.y + row.height < bounds.y - style.item_height
                    || row.y > bounds.y + bounds.height + style.item_height)
            {
                if let Some(&bg) = aux.get(slot * 2) {
                    scene.set_visible(bg, false);
                }
                if let Some(&lab) = aux.get(slot * 2 + 1) {
                    scene.set_visible(lab, false);
                }
                continue;
            }
            let is_sel = selected == Some(i);
            if let Some(&bg) = aux.get(slot * 2) {
                scene.set_visible(bg, false);
                let _ = bg;
            }
            let (_, mask) = self.font.rasterize_text(text, style.size, None);
            if let Some(&lab) = aux.get(slot * 2 + 1) {
                let ly = row.y + ((style.item_height - mask.height as f32) * 0.5).max(0.0);
                scene.set_bounds(
                    lab,
                    Rect::new(row.x + 4.0, ly, mask.width as f32, mask.height as f32),
                );
                scene.set_fill(lab, if is_sel { style.selected_fg } else { style.fg });
                scene.set_mask(
                    lab,
                    Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                );
                scene.set_visible(lab, true);
            }
        }
        // Hide unused aux slots when window shrinks
        for slot in count..(aux.len() / 2) {
            if let Some(&bg) = aux.get(slot * 2) {
                scene.set_visible(bg, false);
            }
            if let Some(&lab) = aux.get(slot * 2 + 1) {
                scene.set_visible(lab, false);
            }
        }
    }

    fn paint_tree(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Tree {
                    roots,
                    selected,
                    scroll_y,
                    style,
                },
            node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let flats = flatten_tree(roots);
        let selected = *selected;
        let scroll_y = *scroll_y;
        let style = *style;
        let host = match *node {
            Some(n) => n,
            None => return,
        };
        let chrome = *chrome_node;
        let bounds = scene
            .get(host)
            .map(|n| n.bounds)
            .unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_clip(host, Some(bounds));
        scene.set_visible(host, true);

        let needed = flats.len() * 2;
        self.ensure_aux_nodes(scene, host, id, needed);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();

        if let Some(chrome) = chrome {
            if let Some(sel) = selected {
                if let Some(i) = flats.iter().position(|r| r.id == sel) {
                    let y = bounds.y + style.padding + i as f32 * style.row_height - scroll_y;
                    scene.set_bounds(
                        chrome,
                        Rect::new(
                            bounds.x + 1.0,
                            y,
                            (bounds.width - 2.0).max(1.0),
                            style.row_height,
                        ),
                    );
                    scene.set_fill(chrome, style.selected_bg);
                    scene.set_mask(chrome, None);
                    scene.set_visible(chrome, true);
                } else {
                    scene.set_visible(chrome, false);
                }
            } else {
                scene.set_visible(chrome, false);
            }
        }

        for (i, row) in flats.iter().enumerate() {
            let y = bounds.y + style.padding + i as f32 * style.row_height - scroll_y;
            if y + style.row_height < bounds.y - style.row_height
                || y > bounds.y + bounds.height + style.row_height
            {
                if let Some(&a) = aux.get(i * 2) {
                    scene.set_visible(a, false);
                }
                if let Some(&a) = aux.get(i * 2 + 1) {
                    scene.set_visible(a, false);
                }
                continue;
            }
            let twisty = if row.has_children {
                if row.expanded {
                    "▾"
                } else {
                    "▸"
                }
            } else {
                "·"
            };
            let label = format!("{twisty} {}", row.label);
            let x = bounds.x + style.padding + row.depth as f32 * style.indent;
            let is_sel = selected == Some(row.id);
            if let Some(&bg) = aux.get(i * 2) {
                scene.set_visible(bg, false);
            }
            let (_, mask) = self.font.rasterize_text(&label, style.size, None);
            if let Some(&lab) = aux.get(i * 2 + 1) {
                let ly = y + ((style.row_height - mask.height as f32) * 0.5).max(0.0);
                scene.set_bounds(
                    lab,
                    Rect::new(x, ly, mask.width as f32, mask.height as f32),
                );
                scene.set_fill(lab, if is_sel { style.selected_fg } else { style.fg });
                scene.set_mask(
                    lab,
                    Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                );
                scene.set_visible(lab, true);
            }
        }
    }

    fn paint_table(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Table {
                    columns,
                    rows,
                    selected_row,
                    scroll_y,
                    virtualized,
                    style,
                },
            node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let columns = columns.clone();
        let rows = rows.clone();
        let selected_row = *selected_row;
        let scroll_y = *scroll_y;
        let virtualized = *virtualized;
        let style = *style;
        let host = match *node {
            Some(n) => n,
            None => return,
        };
        let chrome = *chrome_node;
        let bounds = scene
            .get(host)
            .map(|n| n.bounds)
            .unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_clip(host, Some(bounds));
        scene.set_visible(host, true);

        let ncols = columns.len().max(1);
        let body_h = (bounds.height - style.header_height).max(0.0);
        let (first, count) = if virtualized {
            p1_rest::virtual_window(scroll_y, body_h, style.row_height, rows.len(), 2)
        } else {
            (0, rows.len())
        };
        // aux: [header x ncols] + window slots [row_bg + cells x ncols]
        let needed = ncols + count * (1 + ncols);
        self.ensure_aux_nodes(scene, host, id, needed);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();

        // header bar via chrome
        if let Some(chrome) = chrome {
            scene.set_bounds(
                chrome,
                Rect::new(bounds.x, bounds.y, bounds.width, style.header_height),
            );
            scene.set_fill(chrome, style.header_bg);
            scene.set_mask(chrome, None);
            scene.set_visible(chrome, true);
        }

        let mut x = bounds.x + style.padding;
        for (ci, col) in columns.iter().enumerate() {
            let (_, mask) = self.font.rasterize_text(&col.title, style.size, None);
            if let Some(&lab) = aux.get(ci) {
                let ly =
                    bounds.y + ((style.header_height - mask.height as f32) * 0.5).max(0.0);
                scene.set_bounds(
                    lab,
                    Rect::new(x + 4.0, ly, mask.width as f32, mask.height as f32),
                );
                scene.set_fill(lab, style.header_fg);
                scene.set_mask(
                    lab,
                    Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                );
                scene.set_visible(lab, true);
            }
            x += col.width;
        }

        let body_top = bounds.y + style.header_height;
        for slot in 0..count {
            let ri = first + slot;
            let Some(row) = rows.get(ri) else { break };
            let base = ncols + slot * (1 + ncols);
            let y = body_top + ri as f32 * style.row_height - scroll_y;
            if !virtualized
                && (y + style.row_height < body_top - style.row_height
                    || y > bounds.y + bounds.height + style.row_height)
            {
                for k in 0..(1 + ncols) {
                    if let Some(&n) = aux.get(base + k) {
                        scene.set_visible(n, false);
                    }
                }
                continue;
            }
            let is_sel = selected_row == Some(ri);
            if let Some(&bg) = aux.get(base) {
                scene.set_bounds(
                    bg,
                    Rect::new(
                        bounds.x + 1.0,
                        y,
                        (bounds.width - 2.0).max(1.0),
                        style.row_height,
                    ),
                );
                if is_sel {
                    scene.set_fill(bg, style.selected_bg);
                    scene.set_mask(bg, None);
                    scene.set_visible(bg, true);
                } else {
                    scene.set_visible(bg, false);
                }
            }
            let mut cx = bounds.x + style.padding;
            for (ci, col) in columns.iter().enumerate() {
                let cell = row.get(ci).map(|s| s.as_str()).unwrap_or("");
                let (_, mask) = self.font.rasterize_text(cell, style.size, None);
                if let Some(&lab) = aux.get(base + 1 + ci) {
                    let ly = y + ((style.row_height - mask.height as f32) * 0.5).max(0.0);
                    let max_w = (col.width - 8.0).max(4.0);
                    scene.set_bounds(
                        lab,
                        Rect::new(
                            cx + 4.0,
                            ly,
                            (mask.width as f32).min(max_w),
                            mask.height as f32,
                        ),
                    );
                    scene.set_fill(lab, if is_sel { style.selected_fg } else { style.fg });
                    scene.set_mask(
                        lab,
                        Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                    );
                    scene.set_visible(lab, true);
                }
                cx += col.width;
            }
        }
        // Hide leftover slots beyond current window
        let used = ncols + count * (1 + ncols);
        for i in used..aux.len() {
            if let Some(&n) = aux.get(i) {
                scene.set_visible(n, false);
            }
        }
    }

    fn paint_panel_title(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(w) = self.widgets.get(&id) else {
            return;
        };
        let Kind::Panel { title, style, .. } = &w.kind else {
            return;
        };
        let title = title.clone();
        let title_fg = style.title_fg;
        let title_size = style.title_size;
        let Some(chrome) = w.chrome_node else {
            return;
        };
        let Some(label) = w.label_node else {
            return;
        };
        let bounds = scene
            .get(chrome)
            .map(|n| n.bounds)
            .unwrap_or_else(Rect::empty);
        let (_, mask) = self.font.rasterize_text(&title, title_size, None);
        let lx = bounds.x + 10.0;
        let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
        scene.set_bounds(
            label,
            Rect::new(lx, ly, mask.width as f32, mask.height as f32),
        );
        scene.set_fill(label, title_fg);
        scene.set_mask(
            label,
            Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
        );
        scene.set_visible(label, true);
    }

    fn paint_menu_title(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Menu { title, open, style, .. },
            node,
            label_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let title = title.clone();
        let open = *open;
        let style = *style;
        let Some(node) = *node else { return };
        let label_node = *label_node;
        let bounds = scene.get(node).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        let bg = if open { style.open_bg } else { style.title_bg };
        scene.set_fill(node, bg);
        scene.set_mask(node, None);
        scene.set_visible(node, true);
        let (_, mask) = self.font.rasterize_text(&title, style.size, None);
        if let Some(lid) = label_node {
            let lx = bounds.x + 10.0;
            let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
            scene.set_bounds(lid, Rect::new(lx, ly, mask.width as f32, mask.height as f32));
            scene.set_fill(lid, style.fg);
            scene.set_mask(lid, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(lid, true);
        }
    }

    fn paint_select_field(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Select {
                    options,
                    selected,
                    open,
                    placeholder,
                    style,
                },
            node,
            label_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let text = match *selected {
            Some(i) if i < options.len() => options[i].clone(),
            _ => placeholder.clone(),
        };
        let fg = match *selected {
            Some(i) if i < options.len() => style.fg,
            _ => style.placeholder_fg,
        };
        let open = *open;
        let style = *style;
        let Some(node) = *node else { return };
        let label_node = *label_node;
        let bounds = scene.get(node).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(node, if open { style.selected_bg } else { style.bg });
        scene.set_mask(node, None);
        scene.set_visible(node, true);
        let display = format!("{text}  ▾");
        let (_, mask) = self.font.rasterize_text(&display, style.size, None);
        if let Some(lid) = label_node {
            let lx = bounds.x + style.padding;
            let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
            scene.set_bounds(lid, Rect::new(lx, ly, mask.width as f32, mask.height as f32));
            scene.set_fill(lid, fg);
            scene.set_mask(lid, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(lid, true);
        }
    }

    fn paint_slider(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Slider { min, max, value, style },
            node,
            chrome_node,
            label_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let min = *min;
        let max = *max;
        let value = *value;
        let style = *style;
        let Some(node) = *node else { return };
        let chrome = *chrome_node;
        let thumb = *label_node;
        let bounds = scene.get(node).map(|n| n.bounds).unwrap_or_else(Rect::empty);

        // Invisible full hit target.
        scene.set_bounds(node, bounds);
        scene.set_fill(node, Color::rgba(0, 0, 0, 0));
        scene.set_mask(node, Some(AlphaMask::empty()));
        scene.set_visible(node, true);

        let track_y = bounds.y + (bounds.height - style.track_thickness) * 0.5;
        let track_x = bounds.x + style.thumb_size * 0.5;
        let track_w = (bounds.width - style.thumb_size).max(1.0);
        let t = if (max - min).abs() < 1e-9 {
            0.0
        } else {
            ((value - min) / (max - min)).clamp(0.0, 1.0)
        };

        self.ensure_aux_nodes(scene, node, id, 1);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        if let Some(&track_n) = aux.first() {
            scene.set_bounds(
                track_n,
                Rect::new(track_x, track_y, track_w, style.track_thickness),
            );
            scene.set_fill(track_n, style.track);
            scene.set_mask(track_n, None);
            scene.set_visible(track_n, true);
        }
        if let Some(chrome) = chrome {
            let fill_w = (track_w * t).max(0.001);
            scene.set_bounds(
                chrome,
                Rect::new(track_x, track_y, fill_w, style.track_thickness),
            );
            scene.set_fill(chrome, style.fill);
            scene.set_mask(chrome, None);
            scene.set_visible(chrome, true);
        }
        if let Some(thumb_n) = thumb {
            let tx = track_x + track_w * t - style.thumb_size * 0.5;
            let ty = bounds.y + (bounds.height - style.thumb_size) * 0.5;
            scene.set_bounds(
                thumb_n,
                Rect::new(tx, ty, style.thumb_size, style.thumb_size),
            );
            scene.set_fill(thumb_n, style.thumb);
            scene.set_mask(thumb_n, None);
            scene.set_visible(thumb_n, true);
        }
    }

    fn paint_tabs_strip(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Tabs { titles, selected, style, .. },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let titles = titles.clone();
        let selected = *selected;
        let style = *style;
        let Some(host) = *node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        if titles.is_empty() {
            return;
        }
        let tab_w = (bounds.width / titles.len() as f32).max(40.0);
        let needed = titles.len() * 2;
        self.ensure_aux_nodes(scene, host, id, needed);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        for (i, title) in titles.iter().enumerate() {
            let x = bounds.x + i as f32 * tab_w;
            let active = i == selected;
            if let Some(&bg) = aux.get(i * 2) {
                scene.set_bounds(bg, Rect::new(x, bounds.y, tab_w, style.strip_height));
                scene.set_fill(bg, if active { style.active_bg } else { style.strip_bg });
                scene.set_mask(bg, None);
                scene.set_visible(bg, true);
            }
            let (_, mask) = self.font.rasterize_text(title, style.size, None);
            if let Some(&lab) = aux.get(i * 2 + 1) {
                let lx = x + 10.0;
                let ly = bounds.y + ((style.strip_height - mask.height as f32) * 0.5).max(0.0);
                scene.set_bounds(lab, Rect::new(lx, ly, mask.width as f32, mask.height as f32));
                scene.set_fill(lab, if active { style.active_fg } else { style.fg });
                scene.set_mask(lab, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
                scene.set_visible(lab, true);
            }
        }
    }

    fn paint_context_menu_hint(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::ContextMenu { hint, style, .. },
            node,
            label_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let hint = hint.clone();
        let style = *style;
        let Some(node) = *node else { return };
        let label_node = *label_node;
        let bounds = scene.get(node).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(node, style.host_bg);
        scene.set_mask(node, None);
        scene.set_visible(node, true);
        if hint.is_empty() {
            if let Some(lid) = label_node {
                scene.set_visible(lid, false);
            }
            return;
        }
        let (_, mask) = self.font.rasterize_text(&hint, style.size, None);
        if let Some(lid) = label_node {
            let lx = bounds.x + 8.0;
            let ly = bounds.y + bounds.height - mask.height as f32 - 6.0;
            scene.set_bounds(lid, Rect::new(lx, ly, mask.width as f32, mask.height as f32));
            scene.set_fill(lid, style.hint_fg);
            scene.set_mask(lid, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(lid, true);
        }
    }

    fn paint_drawer_popup(&mut self, scene: &mut Scene, id: WidgetId) {
        let (title, body, style) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::Drawer {
                        title,
                        body,
                        open: true,
                        style,
                    },
                ..
            }) => (title.clone(), body.clone(), *style),
            _ => return,
        };
        let (vw, vh) = self.viewport;
        let panel_w = style.width.min(vw - 20.0).max(180.0);
        let panel_h = vh;
        let px = match style.placement {
            DrawerPlacement::Left => 0.0,
            DrawerPlacement::Right => (vw - panel_w).max(0.0),
        };
        let panel = Rect::new(px, 0.0, panel_w, panel_h);

        self.ensure_overlay_aux(scene, id, 6);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        if aux.len() < 6 {
            return;
        }
        let scrim = aux[0];
        let panel_n = aux[1];
        let title_bg = aux[2];
        let title_l = aux[3];
        let close_n = aux[4];
        let body_n = aux[5];

        scene.set_bounds(scrim, Rect::new(0.0, 0.0, vw, vh));
        scene.set_fill(scrim, style.backdrop.scrim);
        scene.set_mask(scrim, None);
        scene.set_visible(scrim, true);

        scene.set_bounds(panel_n, panel);
        scene.set_fill(panel_n, style.panel_bg);
        scene.set_mask(panel_n, None);
        scene.set_visible(panel_n, true);

        let title_rect = Rect::new(panel.x, panel.y, panel.width, style.title_height);
        scene.set_bounds(title_bg, title_rect);
        scene.set_fill(title_bg, style.title_bg);
        scene.set_mask(title_bg, None);
        scene.set_visible(title_bg, true);

        let (_, tm) = self.font.rasterize_text(&title, style.title_size, None);
        scene.set_bounds(
            title_l,
            Rect::new(
                panel.x + style.padding,
                panel.y + ((style.title_height - tm.height as f32) * 0.5).max(0.0),
                tm.width as f32,
                tm.height as f32,
            ),
        );
        scene.set_fill(title_l, style.title_fg);
        scene.set_mask(title_l, Some(AlphaMask::new(tm.width, tm.height, tm.pixels)));
        scene.set_visible(title_l, true);

        let (_, cm) = self.font.rasterize_text("×", style.title_size, None);
        let close_x = panel.x + panel.width - style.padding - cm.width as f32;
        let close_y = panel.y + ((style.title_height - cm.height as f32) * 0.5).max(0.0);
        scene.set_bounds(
            close_n,
            Rect::new(
                close_x - 4.0,
                close_y - 4.0,
                cm.width as f32 + 8.0,
                cm.height as f32 + 8.0,
            ),
        );
        scene.set_fill(close_n, style.title_fg);
        scene.set_mask(close_n, Some(AlphaMask::new(cm.width, cm.height, cm.pixels)));
        scene.set_visible(close_n, true);

        let (_, bm) = self.font.rasterize_text(&body, style.body_size, None);
        scene.set_bounds(
            body_n,
            Rect::new(
                panel.x + style.padding,
                panel.y + style.title_height + style.padding,
                bm.width as f32,
                bm.height as f32,
            ),
        );
        scene.set_fill(body_n, style.body_fg);
        scene.set_mask(body_n, Some(AlphaMask::new(bm.width, bm.height, bm.pixels)));
        scene.set_visible(body_n, true);

        if let Some(h) = self.overlay_host {
            scene.raise(h);
        }
    }

    fn pointer_drawer(&mut self, scene: &mut Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let (style, aux) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Drawer {
                    style,
                    open: true,
                    ..
                },
                aux,
                ..
            }) => (*style, aux.clone()),
            _ => return false,
        };
        // aux: 0=scrim, 1=panel, 2=title_bg, 3=title, 4=close, 5=body
        let hit = |idx: usize| -> bool {
            aux.get(idx)
                .and_then(|n| scene.get(*n))
                .map(|n| n.visible && n.bounds.contains(x, y))
                .unwrap_or(false)
        };
        if hit(4) {
            self.pending_drawer_action = Some((id, DrawerAction::Dismiss));
            self.hide_popup_aux_mut(scene, id);
            self.close_popup_internal();
            return true;
        }
        if hit(1) || hit(2) || hit(3) || hit(5) {
            return true;
        }
        if style.backdrop.close_on_click {
            self.pending_drawer_action = Some((id, DrawerAction::Dismiss));
            self.hide_popup_aux_mut(scene, id);
            self.close_popup_internal();
        }
        true
    }

    fn paint_notifications(&mut self, scene: &mut Scene) {
        let style = self.notification_style;
        let items = self.notifications.clone();
        let (vw, _vh) = self.viewport;
        let host = self.ensure_overlay_host(scene);
        let needed = items.len() * 3; // accent + bg + label
        let have = self.notification_aux.len();
        for _ in have..needed {
            let nid = scene
                .insert(host, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgba(0, 0, 0, 0))
                .expect("notif aux");
            scene.set_visible(nid, false);
            self.notification_aux.push(nid);
        }
        for (i, n) in self.notification_aux.iter().copied().enumerate() {
            if i >= needed {
                scene.set_visible(n, false);
            }
        }
        let mut y = style.margin;
        for (i, item) in items.iter().enumerate() {
            let base = i * 3;
            let Some(&accent) = self.notification_aux.get(base) else {
                break;
            };
            let Some(&bg) = self.notification_aux.get(base + 1) else {
                break;
            };
            let Some(&lab) = self.notification_aux.get(base + 2) else {
                break;
            };
            let x = (vw - style.width - style.margin).max(0.0);
            let rect = Rect::new(x, y, style.width, style.height);
            scene.set_bounds(bg, rect);
            scene.set_fill(bg, style.bg);
            scene.set_mask(bg, None);
            scene.set_visible(bg, true);
            scene.set_bounds(accent, Rect::new(x, y, 4.0, style.height));
            scene.set_fill(accent, item.tone.accent());
            scene.set_mask(accent, None);
            scene.set_visible(accent, true);
            let (_, mask) = self.font.rasterize_text(&item.message, style.size, None);
            scene.set_bounds(
                lab,
                Rect::new(
                    x + 12.0,
                    y + ((style.height - mask.height as f32) * 0.5).max(0.0),
                    (mask.width as f32).min(style.width - 20.0),
                    mask.height as f32,
                ),
            );
            scene.set_fill(lab, style.fg);
            scene.set_mask(lab, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(lab, true);
            y += style.height + style.gap;
        }
        scene.raise(host);
    }

    fn paint_text_area(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::TextArea {
                    value,
                    placeholder,
                    caret,
                    scroll_y,
                    style,
                },
            node,
            label_node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let value = value.clone();
        let placeholder = placeholder.clone();
        let caret = *caret;
        let scroll_y = *scroll_y;
        let style = *style;
        let Some(host) = *node else { return };
        let label = *label_node;
        let chrome = *chrome_node;
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        let focused = self.focus == Some(id);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_clip(host, Some(bounds));
        scene.set_visible(host, true);
        if let Some(chrome) = chrome {
            // focus ring stub: thin top bar
            scene.set_bounds(
                chrome,
                Rect::new(bounds.x, bounds.y, bounds.width, 2.0),
            );
            scene.set_fill(
                chrome,
                if focused {
                    style.focus_border
                } else {
                    style.border
                },
            );
            scene.set_mask(chrome, None);
            scene.set_visible(chrome, true);
        }
        let display = if value.is_empty() && !focused {
            placeholder.as_str()
        } else {
            value.as_str()
        };
        let fg = if value.is_empty() && !focused {
            style.placeholder_fg
        } else {
            style.fg
        };
        let max_w = (bounds.width - style.padding_x * 2.0).max(1.0);
        let (layout, mask) = self.font.rasterize_text(display, style.size, Some(max_w));
        if let Some(lab) = label {
            scene.set_bounds(
                lab,
                Rect::new(
                    bounds.x + style.padding_x,
                    bounds.y + style.padding_y - scroll_y,
                    mask.width as f32,
                    mask.height as f32,
                ),
            );
            scene.set_fill(lab, fg);
            scene.set_mask(lab, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(lab, true);
        }
        if let Some(chrome) = chrome {
            if focused {
                let metrics = self.font.metrics(style.size);
                // Caret must use the same wrapped layout as the mask, not only explicit
                // newlines + unwrapped measure_line (which clips past the host).
                let (ox, oy) = layout.caret_top_left(&value, caret, metrics.ascent);
                let cx = bounds.x + style.padding_x + ox;
                let cy = bounds.y + style.padding_y - scroll_y + oy;
                let ch = metrics.ascent.max(style.size * 0.8);
                scene.set_bounds(chrome, Rect::new(cx, cy, 2.0, ch));
                scene.set_fill(chrome, style.focus_border);
                scene.set_mask(chrome, None);
                scene.set_visible(chrome, true);
            } else {
                scene.set_visible(chrome, false);
            }
        }
    }

    fn paint_input_number(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::InputNumber {
                    value,
                    style,
                    ..
                },
            node,
            label_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let value = *value;
        let style = *style;
        let Some(host) = *node else { return };
        let label = *label_node;
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        let step_w = 28.0;
        self.ensure_aux_nodes(scene, host, id, 4); // -bg, -lab, +bg, +lab
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        if aux.len() >= 4 {
            let minus_bg = aux[0];
            let minus_l = aux[1];
            let plus_bg = aux[2];
            let plus_l = aux[3];
            scene.set_bounds(
                minus_bg,
                Rect::new(bounds.x, bounds.y, step_w, bounds.height),
            );
            scene.set_fill(minus_bg, style.step_bg);
            scene.set_mask(minus_bg, None);
            scene.set_visible(minus_bg, true);
            let (_, mm) = self.font.rasterize_text("−", style.size, None);
            scene.set_bounds(
                minus_l,
                Rect::new(
                    bounds.x + ((step_w - mm.width as f32) * 0.5).max(0.0),
                    bounds.y + ((bounds.height - mm.height as f32) * 0.5).max(0.0),
                    mm.width as f32,
                    mm.height as f32,
                ),
            );
            scene.set_fill(minus_l, style.step_fg);
            scene.set_mask(minus_l, Some(AlphaMask::new(mm.width, mm.height, mm.pixels)));
            scene.set_visible(minus_l, true);

            scene.set_bounds(
                plus_bg,
                Rect::new(bounds.x + bounds.width - step_w, bounds.y, step_w, bounds.height),
            );
            scene.set_fill(plus_bg, style.step_bg);
            scene.set_mask(plus_bg, None);
            scene.set_visible(plus_bg, true);
            let (_, pm) = self.font.rasterize_text("+", style.size, None);
            scene.set_bounds(
                plus_l,
                Rect::new(
                    bounds.x + bounds.width - step_w + ((step_w - pm.width as f32) * 0.5).max(0.0),
                    bounds.y + ((bounds.height - pm.height as f32) * 0.5).max(0.0),
                    pm.width as f32,
                    pm.height as f32,
                ),
            );
            scene.set_fill(plus_l, style.step_fg);
            scene.set_mask(plus_l, Some(AlphaMask::new(pm.width, pm.height, pm.pixels)));
            scene.set_visible(plus_l, true);
        }
        let text = if value.fract().abs() < 1e-9 {
            format!("{:.0}", value)
        } else {
            format!("{:.2}", value)
        };
        let (_, mask) = self.font.rasterize_text(&text, style.size, None);
        if let Some(lab) = label {
            let field_x = bounds.x + step_w + style.padding_x;
            scene.set_bounds(
                lab,
                Rect::new(
                    field_x,
                    bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0),
                    mask.width as f32,
                    mask.height as f32,
                ),
            );
            scene.set_fill(lab, style.fg);
            scene.set_mask(lab, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(lab, true);
        }
    }

    fn paint_empty(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Empty { description, style },
            node,
            label_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let description = description.clone();
        let style = *style;
        let Some(host) = *node else { return };
        let label = *label_node;
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        let (_, mask) = self.font.rasterize_text(&description, style.size, None);
        if let Some(lab) = label {
            scene.set_bounds(
                lab,
                Rect::new(
                    bounds.x + ((bounds.width - mask.width as f32) * 0.5).max(0.0),
                    bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0),
                    mask.width as f32,
                    mask.height as f32,
                ),
            );
            scene.set_fill(lab, style.fg);
            scene.set_mask(lab, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(lab, true);
        }
    }

    fn paint_card(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Card {
                    title,
                    body,
                    style,
                },
            node,
            label_node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let title = title.clone();
        let body = body.clone();
        let style = *style;
        let Some(host) = *node else { return };
        let label = *label_node;
        let chrome = *chrome_node;
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        if let Some(chrome) = chrome {
            scene.set_bounds(
                chrome,
                Rect::new(bounds.x, bounds.y, bounds.width, style.title_height),
            );
            scene.set_fill(chrome, style.title_bg);
            scene.set_mask(chrome, None);
            scene.set_visible(chrome, true);
        }
        let (_, tm) = self.font.rasterize_text(&title, style.title_size, None);
        if let Some(lab) = label {
            scene.set_bounds(
                lab,
                Rect::new(
                    bounds.x + style.padding,
                    bounds.y + ((style.title_height - tm.height as f32) * 0.5).max(0.0),
                    tm.width as f32,
                    tm.height as f32,
                ),
            );
            scene.set_fill(lab, style.title_fg);
            scene.set_mask(lab, Some(AlphaMask::new(tm.width, tm.height, tm.pixels)));
            scene.set_visible(lab, true);
        }
        self.ensure_aux_nodes(scene, host, id, 1);
        if let Some(&body_n) = self.widgets.get(&id).and_then(|w| w.aux.first()) {
            let (_, bm) = self.font.rasterize_text(&body, style.body_size, None);
            scene.set_bounds(
                body_n,
                Rect::new(
                    bounds.x + style.padding,
                    bounds.y + style.title_height + style.padding,
                    bm.width as f32,
                    bm.height as f32,
                ),
            );
            scene.set_fill(body_n, style.body_fg);
            scene.set_mask(body_n, Some(AlphaMask::new(bm.width, bm.height, bm.pixels)));
            scene.set_visible(body_n, true);
        }
    }

    fn paint_segmented(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Segmented {
                    options,
                    selected,
                    style,
                },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let options = options.clone();
        let selected = *selected;
        let style = *style;
        let Some(host) = *node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        let n = options.len().max(1);
        let seg_w = ((bounds.width - style.padding * 2.0) / n as f32).max(1.0);
        self.ensure_aux_nodes(scene, host, id, n * 2);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        for (i, opt) in options.iter().enumerate() {
            let x = bounds.x + style.padding + i as f32 * seg_w;
            let is_sel = i == selected;
            if let Some(&bg) = aux.get(i * 2) {
                scene.set_bounds(
                    bg,
                    Rect::new(x, bounds.y + style.padding, seg_w, (bounds.height - style.padding * 2.0).max(1.0)),
                );
                scene.set_fill(bg, if is_sel { style.selected_bg } else { style.bg });
                scene.set_mask(bg, None);
                scene.set_visible(bg, is_sel);
            }
            let (_, mask) = self.font.rasterize_text(opt, style.size, None);
            if let Some(&lab) = aux.get(i * 2 + 1) {
                scene.set_bounds(
                    lab,
                    Rect::new(
                        x + ((seg_w - mask.width as f32) * 0.5).max(0.0),
                        bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0),
                        mask.width as f32,
                        mask.height as f32,
                    ),
                );
                scene.set_fill(lab, if is_sel { style.selected_fg } else { style.fg });
                scene.set_mask(lab, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
                scene.set_visible(lab, true);
            }
        }
    }

    fn paint_breadcrumb(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Breadcrumb { items, style },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let items = items.clone();
        let style = *style;
        let Some(host) = *node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, Color::rgba(0, 0, 0, 0));
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        // aux pairs: label (+ optional sep after non-last)
        let needed = items.len() * 2;
        self.ensure_aux_nodes(scene, host, id, needed);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        let mut x = bounds.x;
        for (i, item) in items.iter().enumerate() {
            let is_last = i + 1 == items.len();
            let (_, mask) = self.font.rasterize_text(item, style.size, None);
            if let Some(&lab) = aux.get(i * 2) {
                scene.set_bounds(
                    lab,
                    Rect::new(
                        x,
                        bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0),
                        mask.width as f32,
                        mask.height as f32,
                    ),
                );
                scene.set_fill(lab, if is_last { style.current_fg } else { style.fg });
                scene.set_mask(lab, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
                scene.set_visible(lab, true);
            }
            x += mask.width as f32 + 6.0;
            if !is_last {
                let (_, sm) = self.font.rasterize_text("/", style.size, None);
                if let Some(&sep) = aux.get(i * 2 + 1) {
                    scene.set_bounds(
                        sep,
                        Rect::new(
                            x,
                            bounds.y + ((bounds.height - sm.height as f32) * 0.5).max(0.0),
                            sm.width as f32,
                            sm.height as f32,
                        ),
                    );
                    scene.set_fill(sep, style.sep_fg);
                    scene.set_mask(sep, Some(AlphaMask::new(sm.width, sm.height, sm.pixels)));
                    scene.set_visible(sep, true);
                }
                x += sm.width as f32 + 6.0;
            } else if let Some(&sep) = aux.get(i * 2 + 1) {
                scene.set_visible(sep, false);
            }
        }
    }

    fn paint_pagination(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Pagination {
                    page,
                    page_size,
                    total,
                    style,
                },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let page = *page;
        let page_size = (*page_size).max(1);
        let total = *total;
        let style = *style;
        let Some(host) = *node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, Color::rgba(0, 0, 0, 0));
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        let pages = total.div_ceil(page_size).max(1);
        // aux: prev_bg, prev_l, next_bg, next_l, info
        self.ensure_aux_nodes(scene, host, id, 5);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        if aux.len() < 5 {
            return;
        }
        let btn_w = 56.0;
        let btn_h = bounds.height;
        // prev
        scene.set_bounds(aux[0], Rect::new(bounds.x, bounds.y, btn_w, btn_h));
        scene.set_fill(aux[0], style.btn_bg);
        scene.set_mask(aux[0], None);
        scene.set_visible(aux[0], true);
        let (_, pm) = self.font.rasterize_text("Prev", style.size, None);
        scene.set_bounds(
            aux[1],
            Rect::new(
                bounds.x + ((btn_w - pm.width as f32) * 0.5).max(0.0),
                bounds.y + ((btn_h - pm.height as f32) * 0.5).max(0.0),
                pm.width as f32,
                pm.height as f32,
            ),
        );
        scene.set_fill(aux[1], style.fg);
        scene.set_mask(aux[1], Some(AlphaMask::new(pm.width, pm.height, pm.pixels)));
        scene.set_visible(aux[1], true);
        // info
        let info = format!("{page} / {pages}  ({total})");
        let (_, im) = self.font.rasterize_text(&info, style.size, None);
        let info_x = bounds.x + btn_w + style.gap;
        scene.set_bounds(
            aux[4],
            Rect::new(
                info_x,
                bounds.y + ((btn_h - im.height as f32) * 0.5).max(0.0),
                im.width as f32,
                im.height as f32,
            ),
        );
        scene.set_fill(aux[4], style.fg);
        scene.set_mask(aux[4], Some(AlphaMask::new(im.width, im.height, im.pixels)));
        scene.set_visible(aux[4], true);
        // next
        let nx = bounds.x + bounds.width - btn_w;
        scene.set_bounds(aux[2], Rect::new(nx, bounds.y, btn_w, btn_h));
        scene.set_fill(aux[2], style.btn_bg);
        scene.set_mask(aux[2], None);
        scene.set_visible(aux[2], true);
        let (_, nm) = self.font.rasterize_text("Next", style.size, None);
        scene.set_bounds(
            aux[3],
            Rect::new(
                nx + ((btn_w - nm.width as f32) * 0.5).max(0.0),
                bounds.y + ((btn_h - nm.height as f32) * 0.5).max(0.0),
                nm.width as f32,
                nm.height as f32,
            ),
        );
        scene.set_fill(aux[3], style.fg);
        scene.set_mask(aux[3], Some(AlphaMask::new(nm.width, nm.height, nm.pixels)));
        scene.set_visible(aux[3], true);
    }

    fn paint_collapse(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Collapse { panels, style },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let panels = panels.clone();
        let style = *style;
        let Some(host) = *node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.body_bg);
        scene.set_mask(host, None);
        scene.set_clip(host, Some(bounds));
        scene.set_visible(host, true);
        // per panel: header_bg, header_lab, body_lab
        let needed = panels.len() * 3;
        self.ensure_aux_nodes(scene, host, id, needed);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        let mut y = bounds.y;
        for (i, p) in panels.iter().enumerate() {
            let base = i * 3;
            let twisty = if p.open { "▾ " } else { "▸ " };
            let title = format!("{twisty}{}", p.title);
            if let Some(&hbg) = aux.get(base) {
                scene.set_bounds(
                    hbg,
                    Rect::new(bounds.x, y, bounds.width, style.header_height),
                );
                scene.set_fill(hbg, style.header_bg);
                scene.set_mask(hbg, None);
                scene.set_visible(hbg, true);
            }
            let (_, tm) = self.font.rasterize_text(&title, style.size, None);
            if let Some(&lab) = aux.get(base + 1) {
                scene.set_bounds(
                    lab,
                    Rect::new(
                        bounds.x + style.padding,
                        y + ((style.header_height - tm.height as f32) * 0.5).max(0.0),
                        tm.width as f32,
                        tm.height as f32,
                    ),
                );
                scene.set_fill(lab, style.fg);
                scene.set_mask(lab, Some(AlphaMask::new(tm.width, tm.height, tm.pixels)));
                scene.set_visible(lab, true);
            }
            y += style.header_height;
            if p.open {
                let (_, bm) = self.font.rasterize_text(&p.body, style.size, None);
                if let Some(&blab) = aux.get(base + 2) {
                    scene.set_bounds(
                        blab,
                        Rect::new(
                            bounds.x + style.padding,
                            y + style.padding,
                            bm.width as f32,
                            bm.height as f32,
                        ),
                    );
                    scene.set_fill(blab, style.fg);
                    scene.set_mask(blab, Some(AlphaMask::new(bm.width, bm.height, bm.pixels)));
                    scene.set_visible(blab, true);
                }
                y += style.padding * 2.0 + bm.height as f32;
            } else if let Some(&blab) = aux.get(base + 2) {
                scene.set_visible(blab, false);
            }
        }
    }

    fn paint_descriptions(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Descriptions { items, style },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let items = items.clone();
        let style = *style;
        let Some(host) = *node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        self.ensure_aux_nodes(scene, host, id, items.len() * 2);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        for (i, item) in items.iter().enumerate() {
            let y = bounds.y + style.padding + i as f32 * style.row_height;
            let (_, lm) = self.font.rasterize_text(&item.label, style.size, None);
            if let Some(&lab) = aux.get(i * 2) {
                scene.set_bounds(
                    lab,
                    Rect::new(
                        bounds.x + style.padding,
                        y + ((style.row_height - lm.height as f32) * 0.5).max(0.0),
                        lm.width as f32,
                        lm.height as f32,
                    ),
                );
                scene.set_fill(lab, style.label_fg);
                scene.set_mask(lab, Some(AlphaMask::new(lm.width, lm.height, lm.pixels)));
                scene.set_visible(lab, true);
            }
            let (_, vm) = self.font.rasterize_text(&item.value, style.size, None);
            if let Some(&val) = aux.get(i * 2 + 1) {
                scene.set_bounds(
                    val,
                    Rect::new(
                        bounds.x + style.padding + style.label_width,
                        y + ((style.row_height - vm.height as f32) * 0.5).max(0.0),
                        vm.width as f32,
                        vm.height as f32,
                    ),
                );
                scene.set_fill(val, style.value_fg);
                scene.set_mask(val, Some(AlphaMask::new(vm.width, vm.height, vm.pixels)));
                scene.set_visible(val, true);
            }
        }
    }

    fn paint_spin(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Spin {
                    spinning,
                    style,
                    children,
                },
            node,
            label_node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let spinning = *spinning;
        let style = *style;
        let children = children.clone();
        let Some(host) = *node else { return };
        let label = *label_node;
        let chrome = *chrome_node;
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, Color::rgba(0, 0, 0, 0));
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        for c in children {
            self.paint_widget(scene, c);
        }
        if spinning {
            if let Some(chrome) = chrome {
                scene.set_bounds(chrome, bounds);
                scene.set_fill(chrome, style.overlay);
                scene.set_mask(chrome, None);
                scene.set_visible(chrome, true);
            }
            let (_, mask) = self.font.rasterize_text(style.tip, style.size, None);
            if let Some(lab) = label {
                scene.set_bounds(
                    lab,
                    Rect::new(
                        bounds.x + ((bounds.width - mask.width as f32) * 0.5).max(0.0),
                        bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0),
                        mask.width as f32,
                        mask.height as f32,
                    ),
                );
                scene.set_fill(lab, style.fg);
                scene.set_mask(lab, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
                scene.set_visible(lab, true);
            }
        } else {
            if let Some(chrome) = chrome {
                scene.set_visible(chrome, false);
            }
            if let Some(lab) = label {
                scene.set_visible(lab, false);
            }
        }
    }

    fn paint_datetime_field(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::DateTime { value, open, style, .. },
            node,
            label_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let text = format!("{}  ▾", value.format_display());
        let open = *open;
        let style = *style;
        let Some(node) = *node else { return };
        let label_node = *label_node;
        let bounds = scene.get(node).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(node, if open { style.focus_border } else { style.bg });
        scene.set_mask(node, None);
        scene.set_visible(node, true);
        let (_, mask) = self.font.rasterize_text(&text, style.size, None);
        if let Some(lid) = label_node {
            let lx = bounds.x + style.padding;
            let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
            scene.set_bounds(lid, Rect::new(lx, ly, mask.width as f32, mask.height as f32));
            scene.set_fill(lid, style.fg);
            scene.set_mask(lid, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(lid, true);
        }
    }

    fn paint_datetime_popup(&mut self, scene: &mut Scene, id: WidgetId) {
        let (draft, style, host_bounds) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::DateTime { draft, style, open: true, .. },
                node,
                ..
            }) => {
                let b = node
                    .and_then(|n| scene.get(n).map(|n| n.bounds))
                    .unwrap_or_else(Rect::empty);
                (*draft, *style, b)
            }
            _ => return,
        };
        let header_h = 30.0;
        let grid_h = style.cell * 6.0;
        let popup_h = header_h + grid_h + 28.0 + 28.0 + 16.0;
        let panel = Rect::new(
            host_bounds.x,
            host_bounds.y + host_bounds.height,
            style.popup_width,
            popup_h,
        );
        // aux: 0=panel, 1=header text, 2..=43 day cells (labels), then time + confirm labels
        // Keep it simpler: panel + header + time + confirm + up to 31 day labels
        let needed = 1 + 1 + 31 + 2 + 2;
        // Mount on the raised overlay host so later siblings cannot cover the calendar (F06).
        let overlay = self.ensure_overlay_host(scene);
        let misplaced = self
            .widgets
            .get(&id)
            .and_then(|w| w.aux.first().copied())
            .and_then(|n| scene.get(n))
            .map(|n| n.parent != Some(overlay))
            .unwrap_or(false);
        if misplaced {
            if let Some(w) = self.widgets.get_mut(&id) {
                for &n in &w.aux {
                    scene.set_visible(n, false);
                }
                w.aux.clear();
            }
        }
        self.ensure_overlay_aux(scene, id, needed);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        if let Some(&p) = aux.first() {
            scene.set_bounds(p, panel);
            scene.set_fill(p, style.popup_bg);
            scene.set_mask(p, None);
            scene.set_visible(p, true);
            self.node_to_widget.insert(p, id);
        }
        let header = format!("‹  {:04}-{:02}  ›", draft.year, draft.month);
        let (_, hm) = self.font.rasterize_text(&header, style.size, None);
        if let Some(&hlab) = aux.get(1) {
            scene.set_bounds(
                hlab,
                Rect::new(
                    panel.x + ((panel.width - hm.width as f32) * 0.5).max(0.0),
                    panel.y + ((header_h - hm.height as f32) * 0.5).max(0.0),
                    hm.width as f32,
                    hm.height as f32,
                ),
            );
            scene.set_fill(hlab, style.fg);
            scene.set_mask(hlab, Some(AlphaMask::new(hm.width, hm.height, hm.pixels)));
            scene.set_visible(hlab, true);
        }
        let dim = DateTimeValue::days_in_month(draft.year, draft.month) as usize;
        let grid_top = panel.y + header_h;
        for d in 1..=31 {
            let slot = 1 + d; // aux[2..] days
            if d > dim {
                if let Some(&n) = aux.get(slot) {
                    scene.set_visible(n, false);
                }
                continue;
            }
            let idx = d - 1;
            let col = (idx % 7) as f32;
            let row = (idx / 7) as f32;
            let label = format!("{d}");
            let (_, dm) = self.font.rasterize_text(&label, style.size, None);
            if let Some(&n) = aux.get(slot) {
                let cx = panel.x + 8.0 + col * style.cell;
                let cy = grid_top + row * style.cell;
                scene.set_bounds(
                    n,
                    Rect::new(
                        cx + ((style.cell - dm.width as f32) * 0.5).max(0.0),
                        cy + ((style.cell - dm.height as f32) * 0.5).max(0.0),
                        dm.width as f32,
                        dm.height as f32,
                    ),
                );
                scene.set_fill(
                    n,
                    if d as u8 == draft.day {
                        style.accent
                    } else {
                        style.fg
                    },
                );
                scene.set_mask(n, Some(AlphaMask::new(dm.width, dm.height, dm.pixels)));
                scene.set_visible(n, true);
            }
        }
        let time_y = grid_top + grid_h + 4.0;
        let time = format!(
            "−H  {:02}  +H   −M  {:02}  +M",
            draft.hour, draft.minute
        );
        let (_, tm) = self.font.rasterize_text(&time, style.size, None);
        if let Some(&n) = aux.get(33) {
            scene.set_bounds(
                n,
                Rect::new(panel.x + 8.0, time_y + 4.0, tm.width as f32, tm.height as f32),
            );
            scene.set_fill(n, style.fg);
            scene.set_mask(n, Some(AlphaMask::new(tm.width, tm.height, tm.pixels)));
            scene.set_visible(n, true);
        }
        let confirm_y = time_y + 28.0;
        let actions = "Cancel            OK";
        let (_, am) = self.font.rasterize_text(actions, style.size, None);
        if let Some(&n) = aux.get(34) {
            scene.set_bounds(
                n,
                Rect::new(panel.x + 8.0, confirm_y + 4.0, am.width as f32, am.height as f32),
            );
            scene.set_fill(n, style.fg);
            scene.set_mask(n, Some(AlphaMask::new(am.width, am.height, am.pixels)));
            scene.set_visible(n, true);
        }
        // Hide unused
        for &n in aux.iter().skip(35) {
            scene.set_visible(n, false);
        }
    }

    fn paint_transfer(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Transfer {
                    source,
                    target,
                    source_selected,
                    target_selected,
                    filter,
                    style,
                },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let source = source.clone();
        let target = target.clone();
        let source_selected = *source_selected;
        let target_selected = *target_selected;
        let filter = filter.clone();
        let style = *style;
        let Some(host) = *node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_clip(host, Some(bounds));
        scene.set_visible(host, true);

        let list_w = style.list_width;
        let btn_w = (style.width - list_w * 2.0 - style.gap * 2.0).max(40.0);
        let left_x = bounds.x;
        let btn_x = left_x + list_w + style.gap;
        let right_x = btn_x + btn_w + style.gap;
        let header_h = 28.0;

        let filtered: Vec<(usize, &TransferItem)> = source
            .iter()
            .enumerate()
            .filter(|(_, it)| {
                filter.is_empty() || it.label.to_lowercase().contains(&filter.to_lowercase())
            })
            .collect();

        // aux budget: headers(2) + btns(4) + left rows*2 + right rows*2
        let needed = 2 + 4 + filtered.len() * 2 + target.len() * 2 + 4;
        self.ensure_aux_nodes(scene, host, id, needed);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();

        // Headers
        for (i, (title, x0)) in [("Source", left_x), ("Target", right_x)]
            .iter()
            .enumerate()
        {
            if let Some(&n) = aux.get(i) {
                let (_, m) = self.font.rasterize_text(title, style.size, None);
                scene.set_bounds(
                    n,
                    Rect::new(
                        *x0 + style.padding,
                        bounds.y + ((header_h - m.height as f32) * 0.5).max(0.0),
                        m.width as f32,
                        m.height as f32,
                    ),
                );
                scene.set_fill(n, style.fg);
                scene.set_mask(n, Some(AlphaMask::new(m.width, m.height, m.pixels)));
                scene.set_visible(n, true);
            }
        }
        // Buttons
        let labels = [">", "<", ">>", "<<"];
        for (i, lab) in labels.iter().enumerate() {
            if let Some(&n) = aux.get(2 + i) {
                let by = bounds.y + header_h + i as f32 * 34.0;
                let (_, m) = self.font.rasterize_text(lab, style.size, None);
                scene.set_bounds(
                    n,
                    Rect::new(
                        btn_x + ((btn_w - m.width as f32) * 0.5).max(0.0),
                        by + 4.0,
                        m.width as f32,
                        m.height as f32,
                    ),
                );
                scene.set_fill(n, style.btn_fg);
                scene.set_mask(n, Some(AlphaMask::new(m.width, m.height, m.pixels)));
                scene.set_visible(n, true);
            }
        }
        let mut slot = 6;
        let body_top = bounds.y + header_h;
        for (vis_i, (si, it)) in filtered.iter().enumerate() {
            let y = body_top + vis_i as f32 * style.row_height;
            let selected = source_selected == Some(*si);
            // bg
            if let Some(&bg) = aux.get(slot) {
                if selected {
                    scene.set_bounds(
                        bg,
                        Rect::new(left_x + 1.0, y, list_w - 2.0, style.row_height),
                    );
                    scene.set_fill(bg, style.selected_bg);
                    scene.set_mask(bg, None);
                    scene.set_visible(bg, true);
                } else {
                    scene.set_visible(bg, false);
                }
            }
            slot += 1;
            let (_, m) = self.font.rasterize_text(&it.label, style.size, None);
            if let Some(&lab) = aux.get(slot) {
                scene.set_bounds(
                    lab,
                    Rect::new(
                        left_x + style.padding,
                        y + ((style.row_height - m.height as f32) * 0.5).max(0.0),
                        m.width as f32,
                        m.height as f32,
                    ),
                );
                scene.set_fill(
                    lab,
                    if selected {
                        style.selected_fg
                    } else {
                        style.fg
                    },
                );
                scene.set_mask(lab, Some(AlphaMask::new(m.width, m.height, m.pixels)));
                scene.set_visible(lab, true);
            }
            slot += 1;
        }
        for (ri, it) in target.iter().enumerate() {
            let y = body_top + ri as f32 * style.row_height;
            let selected = target_selected == Some(ri);
            if let Some(&bg) = aux.get(slot) {
                if selected {
                    scene.set_bounds(
                        bg,
                        Rect::new(right_x + 1.0, y, list_w - 2.0, style.row_height),
                    );
                    scene.set_fill(bg, style.selected_bg);
                    scene.set_mask(bg, None);
                    scene.set_visible(bg, true);
                } else {
                    scene.set_visible(bg, false);
                }
            }
            slot += 1;
            let (_, m) = self.font.rasterize_text(&it.label, style.size, None);
            if let Some(&lab) = aux.get(slot) {
                scene.set_bounds(
                    lab,
                    Rect::new(
                        right_x + style.padding,
                        y + ((style.row_height - m.height as f32) * 0.5).max(0.0),
                        m.width as f32,
                        m.height as f32,
                    ),
                );
                scene.set_fill(
                    lab,
                    if selected {
                        style.selected_fg
                    } else {
                        style.fg
                    },
                );
                scene.set_mask(lab, Some(AlphaMask::new(m.width, m.height, m.pixels)));
                scene.set_visible(lab, true);
            }
            slot += 1;
        }
        for &n in aux.iter().skip(slot) {
            scene.set_visible(n, false);
        }
    }

    fn paint_property_grid(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::PropertyGrid {
                    rows,
                    selected,
                    scroll_y,
                    style,
                },
            node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let rows = rows.clone();
        let selected = *selected;
        let scroll_y = *scroll_y;
        let style = *style;
        let Some(host) = *node else { return };
        let chrome = *chrome_node;
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_clip(host, Some(bounds));
        scene.set_visible(host, true);

        let header_h = style.row_height;
        // header bg via chrome
        if let Some(chrome) = chrome {
            scene.set_bounds(
                chrome,
                Rect::new(bounds.x, bounds.y, bounds.width, header_h),
            );
            scene.set_fill(chrome, style.header_bg);
            scene.set_mask(chrome, None);
            scene.set_visible(chrome, true);
        }
        let needed = 2 + rows.len() * 3; // header labels + per row: sel_bg, key, value
        self.ensure_aux_nodes(scene, host, id, needed);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        let (_, km) = self.font.rasterize_text("Property", style.size, None);
        if let Some(&n) = aux.first() {
            scene.set_bounds(
                n,
                Rect::new(
                    bounds.x + style.padding,
                    bounds.y + ((header_h - km.height as f32) * 0.5).max(0.0),
                    km.width as f32,
                    km.height as f32,
                ),
            );
            scene.set_fill(n, style.key_fg);
            scene.set_mask(n, Some(AlphaMask::new(km.width, km.height, km.pixels)));
            scene.set_visible(n, true);
        }
        let (_, vm) = self.font.rasterize_text("Value", style.size, None);
        if let Some(&n) = aux.get(1) {
            scene.set_bounds(
                n,
                Rect::new(
                    bounds.x + style.key_width,
                    bounds.y + ((header_h - vm.height as f32) * 0.5).max(0.0),
                    vm.width as f32,
                    vm.height as f32,
                ),
            );
            scene.set_fill(n, style.key_fg);
            scene.set_mask(n, Some(AlphaMask::new(vm.width, vm.height, vm.pixels)));
            scene.set_visible(n, true);
        }
        let body_top = bounds.y + header_h;
        for (i, row) in rows.iter().enumerate() {
            let y = body_top + i as f32 * style.row_height - scroll_y;
            let base = 2 + i * 3;
            if y + style.row_height < bounds.y + header_h || y > bounds.y + bounds.height {
                for k in 0..3 {
                    if let Some(&n) = aux.get(base + k) {
                        scene.set_visible(n, false);
                    }
                }
                continue;
            }
            let is_sel = selected == Some(i);
            if let Some(&bg) = aux.get(base) {
                if is_sel {
                    scene.set_bounds(
                        bg,
                        Rect::new(bounds.x + 1.0, y, bounds.width - 2.0, style.row_height),
                    );
                    scene.set_fill(bg, style.selected_bg);
                    scene.set_mask(bg, None);
                    scene.set_visible(bg, true);
                } else {
                    scene.set_visible(bg, false);
                }
            }
            let (_, kl) = self.font.rasterize_text(&row.key, style.size, None);
            if let Some(&n) = aux.get(base + 1) {
                scene.set_bounds(
                    n,
                    Rect::new(
                        bounds.x + style.padding,
                        y + ((style.row_height - kl.height as f32) * 0.5).max(0.0),
                        kl.width as f32,
                        kl.height as f32,
                    ),
                );
                scene.set_fill(n, if is_sel { style.selected_fg } else { style.key_fg });
                scene.set_mask(n, Some(AlphaMask::new(kl.width, kl.height, kl.pixels)));
                scene.set_visible(n, true);
            }
            let val = row.value.display();
            let (_, vl) = self.font.rasterize_text(&val, style.size, None);
            if let Some(&n) = aux.get(base + 2) {
                scene.set_bounds(
                    n,
                    Rect::new(
                        bounds.x + style.key_width,
                        y + ((style.row_height - vl.height as f32) * 0.5).max(0.0),
                        vl.width as f32,
                        vl.height as f32,
                    ),
                );
                scene.set_fill(
                    n,
                    if is_sel {
                        style.selected_fg
                    } else {
                        style.value_fg
                    },
                );
                scene.set_mask(n, Some(AlphaMask::new(vl.width, vl.height, vl.pixels)));
                scene.set_visible(n, true);
            }
        }
    }

    fn paint_tree_table(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::TreeTable {
                    columns,
                    roots,
                    selected,
                    scroll_y,
                    style,
                },
            node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let columns = columns.clone();
        let flats = flatten_tree_table(roots);
        let selected = *selected;
        let scroll_y = *scroll_y;
        let style = *style;
        let Some(host) = *node else { return };
        let chrome = *chrome_node;
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_clip(host, Some(bounds));
        scene.set_visible(host, true);

        if let Some(chrome) = chrome {
            scene.set_bounds(
                chrome,
                Rect::new(bounds.x, bounds.y, bounds.width, style.header_height),
            );
            scene.set_fill(chrome, style.header_bg);
            scene.set_mask(chrome, None);
            scene.set_visible(chrome, true);
        }

        // header labels: tree + columns; then rows: sel_bg + tree_lab + cell labs
        let col_n = columns.len();
        let needed = 1 + col_n + flats.len() * (2 + col_n);
        self.ensure_aux_nodes(scene, host, id, needed);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();

        let (_, tm) = self.font.rasterize_text("Name", style.size, None);
        if let Some(&n) = aux.first() {
            scene.set_bounds(
                n,
                Rect::new(
                    bounds.x + style.padding,
                    bounds.y + ((style.header_height - tm.height as f32) * 0.5).max(0.0),
                    tm.width as f32,
                    tm.height as f32,
                ),
            );
            scene.set_fill(n, style.header_fg);
            scene.set_mask(n, Some(AlphaMask::new(tm.width, tm.height, tm.pixels)));
            scene.set_visible(n, true);
        }
        let mut cx = bounds.x + style.tree_col_width;
        for (ci, col) in columns.iter().enumerate() {
            let (_, m) = self.font.rasterize_text(&col.title, style.size, None);
            if let Some(&n) = aux.get(1 + ci) {
                scene.set_bounds(
                    n,
                    Rect::new(
                        cx + 4.0,
                        bounds.y + ((style.header_height - m.height as f32) * 0.5).max(0.0),
                        m.width as f32,
                        m.height as f32,
                    ),
                );
                scene.set_fill(n, style.header_fg);
                scene.set_mask(n, Some(AlphaMask::new(m.width, m.height, m.pixels)));
                scene.set_visible(n, true);
            }
            cx += col.width;
        }

        let body_top = bounds.y + style.header_height;
        let row_stride = 2 + col_n;
        let header_slots = 1 + col_n;
        for (ri, row) in flats.iter().enumerate() {
            let y = body_top + ri as f32 * style.row_height - scroll_y;
            let base = header_slots + ri * row_stride;
            if y + style.row_height < body_top || y > bounds.y + bounds.height {
                for k in 0..row_stride {
                    if let Some(&n) = aux.get(base + k) {
                        scene.set_visible(n, false);
                    }
                }
                continue;
            }
            let is_sel = selected == Some(row.id);
            if let Some(&bg) = aux.get(base) {
                if is_sel {
                    scene.set_bounds(
                        bg,
                        Rect::new(bounds.x + 1.0, y, bounds.width - 2.0, style.row_height),
                    );
                    scene.set_fill(bg, style.selected_bg);
                    scene.set_mask(bg, None);
                    scene.set_visible(bg, true);
                } else {
                    scene.set_visible(bg, false);
                }
            }
            let twisty = if row.has_children {
                if row.expanded {
                    "▾"
                } else {
                    "▸"
                }
            } else {
                "·"
            };
            let label = format!("{twisty} {}", row.label);
            let (_, lm) = self.font.rasterize_text(&label, style.size, None);
            let lx = bounds.x + style.padding + row.depth as f32 * style.indent;
            if let Some(&n) = aux.get(base + 1) {
                scene.set_bounds(
                    n,
                    Rect::new(
                        lx,
                        y + ((style.row_height - lm.height as f32) * 0.5).max(0.0),
                        lm.width as f32,
                        lm.height as f32,
                    ),
                );
                scene.set_fill(n, if is_sel { style.selected_fg } else { style.fg });
                scene.set_mask(n, Some(AlphaMask::new(lm.width, lm.height, lm.pixels)));
                scene.set_visible(n, true);
            }
            let mut cx = bounds.x + style.tree_col_width;
            for (ci, col) in columns.iter().enumerate() {
                let cell = row.cells.get(ci).map(|s| s.as_str()).unwrap_or("");
                let (_, cm) = self.font.rasterize_text(cell, style.size, None);
                if let Some(&n) = aux.get(base + 2 + ci) {
                    scene.set_bounds(
                        n,
                        Rect::new(
                            cx + 4.0,
                            y + ((style.row_height - cm.height as f32) * 0.5).max(0.0),
                            cm.width as f32,
                            cm.height as f32,
                        ),
                    );
                    scene.set_fill(n, if is_sel { style.selected_fg } else { style.fg });
                    scene.set_mask(n, Some(AlphaMask::new(cm.width, cm.height, cm.pixels)));
                    scene.set_visible(n, true);
                }
                cx += col.width;
            }
        }
    }


    fn paint_open_popups(&mut self, scene: &mut Scene) {
        let Some(id) = self.open_popup else {
            // Hide any leftover popup aux on closed widgets — walk menus/selects/ctx/modal
            for wid in self.widgets.keys().copied().collect::<Vec<_>>() {
                if matches!(
                    self.widgets.get(&wid).map(|w| &w.kind),
                    Some(Kind::Menu { open: false, .. })
                        | Some(Kind::ContextMenu { open: false, .. })
                        | Some(Kind::Select { open: false, .. })
                        | Some(Kind::TreeSelect { open: false, .. })
                        | Some(Kind::Modal { open: false, .. })
                        | Some(Kind::Drawer { open: false, .. })
                        | Some(Kind::DateTime { open: false, .. })
                ) {
                    if let Some(w) = self.widgets.get(&wid) {
                        for &n in &w.aux {
                            scene.set_visible(n, false);
                        }
                    }
                }
            }
            return;
        };
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Menu { .. }) => self.paint_menu_popup(scene, id),
            Some(Kind::ContextMenu { .. }) => self.paint_context_popup(scene, id),
            Some(Kind::Select { .. }) => self.paint_select_popup(scene, id),
            Some(Kind::TreeSelect { .. }) => self.paint_tree_select_popup(scene, id),
            Some(Kind::DateTime { .. }) => self.paint_datetime_popup(scene, id),
            Some(Kind::Modal { .. }) => self.paint_modal_popup(scene, id),
            Some(Kind::Drawer { .. }) => self.paint_drawer_popup(scene, id),
            _ => {}
        }
    }

    fn paint_menu_popup(&mut self, scene: &mut Scene, id: WidgetId) {
        let (items, style, host_bounds) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Menu { items, style, open, .. },
                node,
                ..
            }) if *open => {
                let b = node.and_then(|n| scene.get(n).map(|n| n.bounds)).unwrap_or_else(Rect::empty);
                (items.clone(), *style, b)
            }
            _ => return,
        };
        let popup_w = style.popup_width.max(host_bounds.width);
        let popup_h = style.item_height * items.len().max(1) as f32;
        let panel = Rect::new(host_bounds.x, host_bounds.y + host_bounds.height, popup_w, popup_h);
        self.paint_item_popup(scene, id, panel, &items, style.item_height, style.size, style.popup_bg, style.fg, style.disabled_fg, style.hover_bg);
    }

    fn paint_context_popup(&mut self, scene: &mut Scene, id: WidgetId) {
        let (items, style, ax, ay) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::ContextMenu {
                        items,
                        style,
                        open,
                        anchor_x,
                        anchor_y,
                        ..
                    },
                ..
            }) if *open => (items.clone(), *style, *anchor_x, *anchor_y),
            _ => return,
        };
        let popup_h = style.item_height * items.len().max(1) as f32;
        let panel = Rect::new(ax, ay, style.popup_width, popup_h);
        self.paint_item_popup(scene, id, panel, &items, style.item_height, style.size, style.popup_bg, style.fg, style.disabled_fg, style.hover_bg);
    }

    fn paint_select_popup(&mut self, scene: &mut Scene, id: WidgetId) {
        let (options, selected, style, host_bounds) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::Select {
                        options,
                        selected,
                        style,
                        open,
                        ..
                    },
                node,
                ..
            }) if *open => {
                let b = node.and_then(|n| scene.get(n).map(|n| n.bounds)).unwrap_or_else(Rect::empty);
                (options.clone(), *selected, *style, b)
            }
            _ => return,
        };
        let n = options.len().max(1);
        let popup_h = (style.item_height * n as f32).min(style.popup_max_height);
        let panel = Rect::new(host_bounds.x, host_bounds.y + host_bounds.height, host_bounds.width.max(style.width), popup_h);
        let items: Vec<MenuItem> = options
            .iter()
            .enumerate()
            .map(|(i, l)| MenuItem {
                id: i as u64,
                label: l.clone(),
                enabled: true,
            })
            .collect();
        // highlight selected via hover_bg on that row — pass selected index through paint
        self.ensure_overlay_aux(scene, id, 1 + items.len() * 2);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        if let Some(&bg) = aux.first() {
            scene.set_bounds(bg, panel);
            scene.set_fill(bg, style.popup_bg);
            scene.set_mask(bg, None);
            scene.set_visible(bg, true);
            self.node_to_widget.insert(bg, id);
        }
        for (i, item) in items.iter().enumerate() {
            let y = panel.y + i as f32 * style.item_height;
            let row = Rect::new(panel.x, y, panel.width, style.item_height);
            let hi = selected == Some(i);
            if let Some(&rbg) = aux.get(1 + i * 2) {
                scene.set_bounds(rbg, row);
                scene.set_fill(rbg, if hi { style.selected_bg } else { style.popup_bg });
                scene.set_mask(rbg, None);
                scene.set_visible(rbg, true);
                self.node_to_widget.insert(rbg, id);
            }
            let (_, mask) = self.font.rasterize_text(&item.label, style.size, None);
            if let Some(&lab) = aux.get(1 + i * 2 + 1) {
                let lx = row.x + style.padding;
                let ly = row.y + ((row.height - mask.height as f32) * 0.5).max(0.0);
                scene.set_bounds(lab, Rect::new(lx, ly, mask.width as f32, mask.height as f32));
                scene.set_fill(lab, style.fg);
                scene.set_mask(lab, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
                scene.set_visible(lab, true);
            }
        }
    }

    fn paint_tag(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Tag {
                label,
                closable,
                style,
            },
            node,
            label_node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let label = label.clone();
        let closable = *closable;
        let style = *style;
        let Some(node) = *node else {
            return;
        };
        let bounds = scene.get(node).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(node, style.bg);
        scene.set_mask(node, None);
        scene.set_visible(node, true);
        let close_w = if closable { style.height * 0.65 } else { 0.0 };
        let (_, mask) = self.font.rasterize_text(&label, style.size, None);
        if let Some(lid) = *label_node {
            let lx = bounds.x + style.padding_x;
            let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
            scene.set_bounds(lid, Rect::new(lx, ly, mask.width as f32, mask.height as f32));
            scene.set_fill(lid, style.fg);
            scene.set_mask(lid, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(lid, true);
        }
        if let Some(cid) = *chrome_node {
            if closable {
                let (_, cm) = self.font.rasterize_text("×", style.size, None);
                let cx = bounds.x + bounds.width - close_w;
                let cy = bounds.y + ((bounds.height - cm.height as f32) * 0.5).max(0.0);
                scene.set_bounds(cid, Rect::new(cx, cy, close_w.max(cm.width as f32), bounds.height));
                // Use label-sized glyph centered in close hit area.
                let gx = cx + ((close_w - cm.width as f32) * 0.5).max(0.0);
                scene.set_bounds(cid, Rect::new(gx, cy, cm.width as f32, cm.height as f32));
                scene.set_fill(cid, style.close_fg);
                scene.set_mask(cid, Some(AlphaMask::new(cm.width, cm.height, cm.pixels)));
                scene.set_visible(cid, true);
            } else {
                scene.set_visible(cid, false);
            }
        }
    }

    fn paint_badge_overlay(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Badge { kind, style, children },
            node,
            label_node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let kind = *kind;
        let style = *style;
        let has_host = !children.is_empty();
        let Some(node) = *node else {
            return;
        };
        let bounds = scene.get(node).map(|n| n.bounds).unwrap_or_else(Rect::empty);

        let (text, show) = match kind {
            BadgeKind::Dot => (String::new(), true),
            BadgeKind::Count(0) if has_host => (String::new(), false),
            BadgeKind::Count(c) => {
                let t = if c > 99 { "99+".into() } else { c.to_string() };
                (t, true)
            }
        };

        if !show {
            if let Some(cid) = *chrome_node {
                scene.set_visible(cid, false);
            }
            if let Some(lid) = *label_node {
                scene.set_visible(lid, false);
            }
            if !has_host {
                scene.set_visible(node, false);
            }
            return;
        }

        let (bw, bh) = if matches!(kind, BadgeKind::Dot) {
            (style.dot_size, style.dot_size)
        } else {
            let (tw, _) = self.font.measure_line(&text, style.size);
            ((tw + style.padding_x * 2.0).max(style.height), style.height)
        };

        let (bx, by) = if has_host {
            (
                bounds.x + bounds.width - bw * 0.55,
                bounds.y - bh * 0.35 + style.offset * 0.25,
            )
        } else {
            (bounds.x, bounds.y + ((bounds.height - bh) * 0.5).max(0.0))
        };

        if has_host {
            scene.set_mask(node, Some(AlphaMask::empty()));
            scene.set_visible(node, true);
            if let Some(cid) = *chrome_node {
                scene.set_bounds(cid, Rect::new(bx, by, bw, bh));
                scene.set_fill(cid, style.bg);
                scene.set_mask(cid, None);
                scene.set_visible(cid, true);
            }
        } else {
            scene.set_bounds(node, Rect::new(bx, by, bw, bh));
            scene.set_fill(node, style.bg);
            scene.set_mask(node, None);
            scene.set_visible(node, true);
            if let Some(cid) = *chrome_node {
                scene.set_visible(cid, false);
            }
        }

        if let Some(lid) = *label_node {
            if text.is_empty() {
                scene.set_visible(lid, false);
            } else {
                let (_, mask) = self.font.rasterize_text(&text, style.size, None);
                let lx = bx + ((bw - mask.width as f32) * 0.5).max(0.0);
                let ly = by + ((bh - mask.height as f32) * 0.5).max(0.0);
                scene.set_bounds(lid, Rect::new(lx, ly, mask.width as f32, mask.height as f32));
                scene.set_fill(lid, style.fg);
                scene.set_mask(lid, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
                scene.set_visible(lid, true);
            }
        }
    }

    fn paint_progress(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Progress {
                value,
                indeterminate,
                style,
            },
            node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let value = *value;
        let indeterminate = *indeterminate;
        let style = *style;
        let Some(node) = *node else {
            return;
        };
        let bounds = scene.get(node).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        let track = Rect::new(
            bounds.x,
            bounds.y + ((bounds.height - style.height) * 0.5).max(0.0),
            bounds.width.max(style.width * 0.5),
            style.height,
        );
        scene.set_bounds(node, track);
        scene.set_fill(node, style.track);
        scene.set_mask(node, None);
        scene.set_visible(node, true);
        if let Some(cid) = *chrome_node {
            let fill_w = if indeterminate {
                (track.width * 0.35).max(8.0)
            } else {
                track.width * value.clamp(0.0, 1.0)
            };
            scene.set_bounds(cid, Rect::new(track.x, track.y, fill_w, track.height));
            scene.set_fill(cid, style.fill);
            scene.set_mask(cid, None);
            scene.set_visible(cid, fill_w > 0.5);
        }
    }

    fn paint_alert(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Alert {
                    tone,
                    message,
                    dismissible,
                    dismissed,
                    style,
                },
            node,
            label_node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let tone = *tone;
        let message = message.clone();
        let dismissible = *dismissible;
        let dismissed = *dismissed;
        let style = *style;
        let Some(node) = *node else {
            return;
        };
        if dismissed {
            scene.set_visible(node, false);
            if let Some(lid) = *label_node {
                scene.set_visible(lid, false);
            }
            if let Some(cid) = *chrome_node {
                scene.set_visible(cid, false);
            }
            return;
        }
        let bounds = scene.get(node).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        let bg = match tone {
            AlertTone::Info => style.info_bg,
            AlertTone::Success => style.success_bg,
            AlertTone::Warning => style.warning_bg,
            AlertTone::Error => style.error_bg,
        };
        scene.set_fill(node, bg);
        scene.set_mask(node, None);
        scene.set_visible(node, true);
        let (_, mask) = self.font.rasterize_text(&message, style.size, None);
        if let Some(lid) = *label_node {
            let lx = bounds.x + style.padding;
            let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
            scene.set_bounds(lid, Rect::new(lx, ly, mask.width as f32, mask.height as f32));
            scene.set_fill(lid, style.fg);
            scene.set_mask(lid, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(lid, true);
        }
        if let Some(cid) = *chrome_node {
            if dismissible {
                let (_, cm) = self.font.rasterize_text("×", style.size, None);
                let cx = bounds.x + bounds.width - style.padding - cm.width as f32;
                let cy = bounds.y + ((bounds.height - cm.height as f32) * 0.5).max(0.0);
                scene.set_bounds(cid, Rect::new(cx, cy, cm.width as f32 + 4.0, cm.height as f32 + 4.0));
                scene.set_fill(cid, style.close_fg);
                scene.set_mask(cid, Some(AlphaMask::new(cm.width, cm.height, cm.pixels)));
                scene.set_visible(cid, true);
            } else {
                scene.set_visible(cid, false);
            }
        }
    }

    fn paint_tooltip_overlay(&mut self, scene: &mut Scene) {
        let Some(id) = self.tooltip_hover else {
            // Hide tooltip aux on all tooltip widgets.
            for wid in self.widgets.keys().copied().collect::<Vec<_>>() {
                if matches!(
                    self.widgets.get(&wid).map(|w| &w.kind),
                    Some(Kind::Tooltip { .. })
                ) {
                    self.hide_popup_aux_mut(scene, wid);
                }
            }
            return;
        };
        let (text, style, host_bounds) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Tooltip { text, style, .. },
                node,
                ..
            }) => {
                let b = node
                    .and_then(|n| scene.get(n).map(|n| n.bounds))
                    .unwrap_or_else(Rect::empty);
                (text.clone(), *style, b)
            }
            _ => return,
        };
        if text.is_empty() || host_bounds.is_empty() {
            return;
        }
        let (tw, th) = self.font.measure_line(&text, style.size);
        let tip_w = (tw + style.padding * 2.0).min(style.max_width.max(tw + style.padding * 2.0));
        let tip_h = th + style.padding * 2.0;
        let mut tip_x = host_bounds.x;
        let tip_y = host_bounds.y + host_bounds.height + style.gap;
        let (vw, _) = self.viewport;
        if tip_x + tip_w > vw {
            tip_x = (vw - tip_w).max(0.0);
        }
        let tip = Rect::new(tip_x, tip_y, tip_w, tip_h);
        self.ensure_overlay_aux(scene, id, 2);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        if aux.len() < 2 {
            return;
        }
        let bg = aux[0];
        let label = aux[1];
        scene.set_bounds(bg, tip);
        scene.set_fill(bg, style.bg);
        scene.set_mask(bg, None);
        scene.set_visible(bg, true);
        let (_, mask) = self.font.rasterize_text(&text, style.size, None);
        scene.set_bounds(
            label,
            Rect::new(
                tip.x + style.padding,
                tip.y + style.padding,
                mask.width as f32,
                mask.height as f32,
            ),
        );
        scene.set_fill(label, style.fg);
        scene.set_mask(label, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
        scene.set_visible(label, true);
        // Raise overlay host above content.
        if let Some(h) = self.overlay_host {
            scene.raise(h);
        }
    }

    fn paint_modal_popup(&mut self, scene: &mut Scene, id: WidgetId) {
        let (title, body, primary, secondary, style) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::Modal {
                        title,
                        body,
                        primary,
                        secondary,
                        open: true,
                        style,
                    },
                ..
            }) => (
                title.clone(),
                body.clone(),
                primary.clone(),
                secondary.clone(),
                *style,
            ),
            _ => return,
        };
        let (vw, vh) = self.viewport;
        let panel_w = style.width.min(vw - 40.0).max(200.0);
        let action_h = if primary.is_some() || secondary.is_some() {
            style.action_height + style.padding
        } else {
            0.0
        };
        let (_, body_th) = self.font.measure_line(&body, style.body_size);
        let panel_h = (style.title_height
            + style.padding * 2.0
            + body_th.max(40.0)
            + action_h)
            .max(style.height)
            .min(vh - 40.0);
        let px = ((vw - panel_w) * 0.5).max(0.0);
        let py = ((vh - panel_h) * 0.5).max(0.0);
        let panel = Rect::new(px, py, panel_w, panel_h);

        // 10 aux slots
        self.ensure_overlay_aux(scene, id, 10);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        if aux.len() < 10 {
            return;
        }
        let scrim = aux[0];
        let panel_n = aux[1];
        let title_bg = aux[2];
        let title_l = aux[3];
        let close_n = aux[4];
        let body_n = aux[5];
        let primary_btn = aux[6];
        let primary_l = aux[7];
        let secondary_btn = aux[8];
        let secondary_l = aux[9];

        scene.set_bounds(scrim, Rect::new(0.0, 0.0, vw, vh));
        scene.set_fill(scrim, style.scrim);
        scene.set_mask(scrim, None);
        scene.set_visible(scrim, true);

        scene.set_bounds(panel_n, panel);
        scene.set_fill(panel_n, style.panel_bg);
        scene.set_mask(panel_n, None);
        scene.set_visible(panel_n, true);

        let title_rect = Rect::new(panel.x, panel.y, panel.width, style.title_height);
        scene.set_bounds(title_bg, title_rect);
        scene.set_fill(title_bg, style.title_bg);
        scene.set_mask(title_bg, None);
        scene.set_visible(title_bg, true);

        let (_, tm) = self.font.rasterize_text(&title, style.title_size, None);
        scene.set_bounds(
            title_l,
            Rect::new(
                panel.x + style.padding,
                panel.y + ((style.title_height - tm.height as f32) * 0.5).max(0.0),
                tm.width as f32,
                tm.height as f32,
            ),
        );
        scene.set_fill(title_l, style.title_fg);
        scene.set_mask(title_l, Some(AlphaMask::new(tm.width, tm.height, tm.pixels)));
        scene.set_visible(title_l, true);

        let (_, cm) = self.font.rasterize_text("×", style.title_size, None);
        let close_x = panel.x + panel.width - style.padding - cm.width as f32;
        let close_y = panel.y + ((style.title_height - cm.height as f32) * 0.5).max(0.0);
        scene.set_bounds(
            close_n,
            Rect::new(close_x - 4.0, close_y - 4.0, cm.width as f32 + 8.0, cm.height as f32 + 8.0),
        );
        scene.set_fill(close_n, style.title_fg);
        scene.set_mask(close_n, Some(AlphaMask::new(cm.width, cm.height, cm.pixels)));
        scene.set_visible(close_n, true);

        let (_, bm) = self.font.rasterize_text(&body, style.body_size, None);
        scene.set_bounds(
            body_n,
            Rect::new(
                panel.x + style.padding,
                panel.y + style.title_height + style.padding,
                bm.width as f32,
                bm.height as f32,
            ),
        );
        scene.set_fill(body_n, style.body_fg);
        scene.set_mask(body_n, Some(AlphaMask::new(bm.width, bm.height, bm.pixels)));
        scene.set_visible(body_n, true);

        let btn_y = panel.y + panel.height - style.padding - style.action_height;
        let mut cursor_x = panel.x + panel.width - style.padding;

        if let Some(ref p) = primary {
            let (tw, _) = self.font.measure_line(p, style.body_size);
            let bw = (tw + 24.0).max(72.0);
            let bx = cursor_x - bw;
            scene.set_bounds(primary_btn, Rect::new(bx, btn_y, bw, style.action_height));
            scene.set_fill(primary_btn, style.primary_bg);
            scene.set_mask(primary_btn, None);
            scene.set_visible(primary_btn, true);
            let (_, lm) = self.font.rasterize_text(p, style.body_size, None);
            scene.set_bounds(
                primary_l,
                Rect::new(
                    bx + ((bw - lm.width as f32) * 0.5).max(0.0),
                    btn_y + ((style.action_height - lm.height as f32) * 0.5).max(0.0),
                    lm.width as f32,
                    lm.height as f32,
                ),
            );
            scene.set_fill(primary_l, style.action_fg);
            scene.set_mask(primary_l, Some(AlphaMask::new(lm.width, lm.height, lm.pixels)));
            scene.set_visible(primary_l, true);
            cursor_x = bx - 8.0;
        } else {
            scene.set_visible(primary_btn, false);
            scene.set_visible(primary_l, false);
        }
        if let Some(ref s) = secondary {
            let (tw, _) = self.font.measure_line(s, style.body_size);
            let bw = (tw + 24.0).max(72.0);
            let bx = cursor_x - bw;
            scene.set_bounds(secondary_btn, Rect::new(bx, btn_y, bw, style.action_height));
            scene.set_fill(secondary_btn, style.secondary_bg);
            scene.set_mask(secondary_btn, None);
            scene.set_visible(secondary_btn, true);
            let (_, lm) = self.font.rasterize_text(s, style.body_size, None);
            scene.set_bounds(
                secondary_l,
                Rect::new(
                    bx + ((bw - lm.width as f32) * 0.5).max(0.0),
                    btn_y + ((style.action_height - lm.height as f32) * 0.5).max(0.0),
                    lm.width as f32,
                    lm.height as f32,
                ),
            );
            scene.set_fill(secondary_l, style.action_fg);
            scene.set_mask(secondary_l, Some(AlphaMask::new(lm.width, lm.height, lm.pixels)));
            scene.set_visible(secondary_l, true);
        } else {
            scene.set_visible(secondary_btn, false);
            scene.set_visible(secondary_l, false);
        }

        if let Some(h) = self.overlay_host {
            scene.raise(h);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_item_popup(
        &mut self,
        scene: &mut Scene,
        id: WidgetId,
        panel: Rect,
        items: &[MenuItem],
        item_height: f32,
        size: f32,
        popup_bg: Color,
        fg: Color,
        disabled_fg: Color,
        _hover_bg: Color,
    ) {
        self.ensure_overlay_aux(scene, id, 1 + items.len() * 2);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        if let Some(&bg) = aux.first() {
            scene.set_bounds(bg, panel);
            scene.set_fill(bg, popup_bg);
            scene.set_mask(bg, None);
            scene.set_visible(bg, true);
            self.node_to_widget.insert(bg, id);
        }
        for (i, item) in items.iter().enumerate() {
            let y = panel.y + i as f32 * item_height;
            let row = Rect::new(panel.x, y, panel.width, item_height);
            if let Some(&rbg) = aux.get(1 + i * 2) {
                scene.set_bounds(rbg, row);
                scene.set_fill(rbg, popup_bg);
                scene.set_mask(rbg, None);
                scene.set_visible(rbg, true);
                self.node_to_widget.insert(rbg, id);
            }
            let (_, mask) = self.font.rasterize_text(&item.label, size, None);
            if let Some(&lab) = aux.get(1 + i * 2 + 1) {
                let lx = row.x + 10.0;
                let ly = row.y + ((row.height - mask.height as f32) * 0.5).max(0.0);
                scene.set_bounds(lab, Rect::new(lx, ly, mask.width as f32, mask.height as f32));
                scene.set_fill(lab, if item.enabled { fg } else { disabled_fg });
                scene.set_mask(lab, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
                scene.set_visible(lab, true);
            }
        }
    }

// Office P1 methods (included inside `impl Ui`).

    fn tree_label_by_id(roots: &[TreeNode], id: u64) -> Option<String> {
        for n in roots {
            if n.id == id {
                return Some(n.label.clone());
            }
            if let Some(l) = Self::tree_label_by_id(&n.children, id) {
                return Some(l);
            }
        }
        None
    }

    /// Whether id is a Form.
    pub fn is_form(&self, id: WidgetId) -> bool {
        matches!(self.widgets.get(&id).map(|w| &w.kind), Some(Kind::Form { .. }))
    }

    /// Whether id is a TreeSelect.
    pub fn is_tree_select(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::TreeSelect { .. })
        )
    }

    /// Whether id is an AlarmList.
    pub fn is_alarm_list(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::AlarmList { .. })
        )
    }

    /// Whether id is a Gauge.
    pub fn is_gauge(&self, id: WidgetId) -> bool {
        matches!(self.widgets.get(&id).map(|w| &w.kind), Some(Kind::Gauge { .. }))
    }

    /// Whether id is a Trend.
    pub fn is_trend(&self, id: WidgetId) -> bool {
        matches!(self.widgets.get(&id).map(|w| &w.kind), Some(Kind::Trend { .. }))
    }

    /// Whether id is a multi-tile Chart.
    pub fn is_chart(&self, id: WidgetId) -> bool {
        matches!(self.widgets.get(&id).map(|w| &w.kind), Some(Kind::Chart { .. }))
    }

    /// Selected TreeSelect node id.
    pub fn tree_select_selected(&self, id: WidgetId) -> Option<u64> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::TreeSelect { selected, .. }) => *selected,
            _ => None,
        }
    }

    /// Set TreeSelect selection (closes popup).
    pub fn set_tree_select_selected(&mut self, id: WidgetId, node_id: Option<u64>) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::TreeSelect {
                selected, open, ..
            } = &mut w.kind
            {
                *selected = node_id;
                *open = false;
                if self.open_popup == Some(id) {
                    self.open_popup = None;
                }
                return true;
            }
        }
        false
    }

    /// Selected AlarmList row index.
    pub fn alarm_list_selected(&self, id: WidgetId) -> Option<usize> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::AlarmList { selected, .. }) => *selected,
            _ => None,
        }
    }

    /// Set AlarmList selection.
    pub fn set_alarm_list_selected(&mut self, id: WidgetId, index: Option<usize>) -> bool {
        let n = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::AlarmList { entries, .. }) => entries.len(),
            _ => return false,
        };
        if let Some(i) = index {
            if i >= n {
                return false;
            }
        }
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::AlarmList { selected, .. } = &mut w.kind {
                *selected = index;
                return true;
            }
        }
        false
    }

    /// Drive unacked Critical/Major flash phase.
    pub fn set_alarm_list_flash(&mut self, id: WidgetId, flash_on: bool) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::AlarmList { flash_on: f, .. } = &mut w.kind {
                if *f == flash_on {
                    return false;
                }
                *f = flash_on;
                return true;
            }
        }
        false
    }

    /// Acknowledge the selected alarm row; returns entry id if newly acked.
    pub fn ack_alarm_list_selected(&mut self, id: WidgetId) -> Option<u64> {
        let sel = self.alarm_list_selected(id)?;
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::AlarmList { entries, .. } = &mut w.kind {
                if let Some(e) = entries.get_mut(sel) {
                    if !e.acked {
                        e.acked = true;
                        return Some(e.id);
                    }
                }
            }
        }
        None
    }

    /// Replace AlarmList entries.
    pub fn set_alarm_list_entries(&mut self, id: WidgetId, entries: Vec<AlarmEntry>) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::AlarmList {
                entries: slot,
                selected,
                scroll_y,
                ..
            } = &mut w.kind
            {
                *slot = entries;
                if let Some(i) = *selected {
                    if i >= slot.len() {
                        *selected = slot.len().checked_sub(1);
                    }
                }
                *scroll_y = 0.0;
                return true;
            }
        }
        false
    }

    /// Gauge value.
    pub fn gauge_value(&self, id: WidgetId) -> Option<f32> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Gauge { value, .. }) => Some(*value),
            _ => None,
        }
    }

    /// Set Gauge value (clamped).
    pub fn set_gauge_value(&mut self, id: WidgetId, value: f32) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Gauge {
                value: v, min, max, ..
            } = &mut w.kind
            {
                let nv = value.clamp(*min, *max);
                if (*v - nv).abs() < 1e-6 {
                    return false;
                }
                *v = nv;
                return true;
            }
        }
        false
    }

    /// Replace Trend sample points.
    pub fn set_trend_points(&mut self, id: WidgetId, points: Vec<f32>) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Trend { points: slot, .. } = &mut w.kind {
                *slot = points;
                return true;
            }
        }
        false
    }

    /// Number of tiles in a Chart.
    pub fn chart_tile_count(&self, id: WidgetId) -> Option<usize> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Chart { tiles, .. }) => Some(tiles.len()),
            _ => None,
        }
    }

    /// Set a Chart tile Gauge value by index (no-op if not a Gauge tile).
    pub fn set_chart_gauge_value(&mut self, id: WidgetId, tile: usize, value: f32) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Chart { tiles, .. } = &mut w.kind {
                if let Some(t) = tiles.get_mut(tile) {
                    if let ChartKind::Gauge {
                        value: v, min, max, ..
                    } = &mut t.kind
                    {
                        let nv = value.clamp(*min, *max);
                        if (*v - nv).abs() < 1e-6 {
                            return false;
                        }
                        *v = nv;
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Replace Chart tile Trend points by index.
    pub fn set_chart_trend_points(
        &mut self,
        id: WidgetId,
        tile: usize,
        points: Vec<f32>,
    ) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Chart { tiles, .. } = &mut w.kind {
                if let Some(t) = tiles.get_mut(tile) {
                    if let ChartKind::Trend { points: slot, .. } = &mut t.kind {
                        *slot = points;
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Replace all Chart tiles.
    pub fn set_chart_tiles(&mut self, id: WidgetId, tiles: Vec<ChartTile>) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Chart { tiles: slot, .. } = &mut w.kind {
                *slot = tiles;
                return true;
            }
        }
        false
    }

    /// Run Form validation against bound TextInput values. Updates error labels.
    /// Returns `true` when all fields pass.
    pub fn validate_form(&mut self, id: WidgetId) -> bool {
        let fields = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Form { fields, .. }) => fields.clone(),
            _ => return false,
        };
        let mut ok = true;
        for field in &fields {
            let value = self
                .text_input_value(field.control)
                .unwrap_or("")
                .to_string();
            let mut err: Option<String> = None;
            for rule in &field.rules {
                if let Some(msg) = rule.check(&value) {
                    err = Some(msg.to_string());
                    break;
                }
            }
            if err.is_some() {
                ok = false;
            }
            self.set_text(field.error_label, err.unwrap_or_default());
        }
        ok
    }

    /// Clear all Form field error labels.
    pub fn clear_form_errors(&mut self, id: WidgetId) -> bool {
        let fields = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Form { fields, .. }) => fields.clone(),
            _ => return false,
        };
        for field in fields {
            self.set_text(field.error_label, "");
        }
        true
    }

    /// Current error text for a Form field key (empty if none).
    pub fn form_field_error(&self, id: WidgetId, key: &str) -> Option<String> {
        let fields = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Form { fields, .. }) => fields,
            _ => return None,
        };
        let field = fields.iter().find(|f| f.key == key)?;
        self.text_content(field.error_label).map(|s| s.to_string())
    }

    /// Form field control widget id for `key`.
    pub fn form_field_control(&self, id: WidgetId, key: &str) -> Option<WidgetId> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Form { fields, .. }) => fields.iter().find(|f| f.key == key).map(|f| f.control),
            _ => None,
        }
    }

    fn pointer_tree_select_toggle(&mut self, scene: &mut Scene, id: WidgetId) -> bool {
        let was_open = matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::TreeSelect { open: true, .. })
        );
        if let Some(pid) = self.open_popup {
            self.hide_popup_aux_mut(scene, pid);
        }
        self.close_popup_internal();
        if was_open {
            return true;
        }
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::TreeSelect { open, .. } = &mut w.kind {
                *open = true;
            }
        }
        self.open_popup = Some(id);
        true
    }

    fn pointer_tree_select_popup(&mut self, scene: &mut Scene, id: WidgetId, x: f32, y: f32) -> bool {
        // Toggle close if clicking host.
        if let Some(w) = self.widgets.get(&id) {
            if let Some(node) = w.node {
                if let Some(n) = scene.get(node) {
                    if n.bounds.contains(x, y) {
                        return self.pointer_tree_select_toggle(scene, id);
                    }
                }
            }
        }
        let (flats, style, scroll_y, panel) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::TreeSelect {
                        roots,
                        style,
                        scroll_y,
                        open: true,
                        ..
                    },
                aux,
                ..
            }) => {
                let panel = aux
                    .first()
                    .and_then(|n| scene.get(*n))
                    .map(|n| n.bounds)
                    .unwrap_or_else(Rect::empty);
                (flatten_tree(roots), *style, *scroll_y, panel)
            }
            _ => return false,
        };
        if panel.is_empty() || flats.is_empty() {
            self.close_popup_internal();
            return true;
        }
        if !panel.contains(x, y) {
            self.hide_popup_aux_mut(scene, id);
            self.close_popup_internal();
            return true;
        }
        let local_y = y - panel.y + scroll_y;
        let idx = if style.row_height > 0.0 {
            (local_y / style.row_height).floor() as isize
        } else {
            -1
        };
        if idx < 0 || (idx as usize) >= flats.len() {
            return true;
        }
        let row = &flats[idx as usize];
        let click_x = x - panel.x;
        let twiddle_w = style.indent.max(12.0);
        let depth_x = style.padding + row.depth as f32 * style.indent;
        if row.has_children && click_x >= depth_x && click_x < depth_x + twiddle_w {
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::TreeSelect { roots, .. } = &mut w.kind {
                    toggle_expanded_in_forest(roots, row.id);
                }
            }
            return true;
        }
        self.set_tree_select_selected(id, Some(row.id));
        self.hide_popup_aux_mut(scene, id);
        true
    }

    fn pointer_alarm_list(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let (style, scroll_y, n, node) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::AlarmList {
                        entries,
                        style,
                        scroll_y,
                        ..
                    },
                node,
                ..
            }) => (*style, *scroll_y, entries.len(), *node),
            _ => return false,
        };
        let bounds = node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        let local_y = y - bounds.y - style.padding + scroll_y;
        let idx = if style.row_height > 0.0 {
            (local_y / style.row_height).floor() as isize
        } else {
            -1
        };
        if idx < 0 || (idx as usize) >= n {
            return true;
        }
        let _ = x;
        self.set_alarm_list_selected(id, Some(idx as usize));
        true
    }

    fn key_alarm_list(&mut self, id: WidgetId, key: UiKey) -> bool {
        let n = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::AlarmList { entries, .. }) => entries.len(),
            _ => return false,
        };
        if n == 0 {
            return false;
        }
        let cur = self.alarm_list_selected(id);
        match key {
            UiKey::Down => {
                let next = cur.map(|i| (i + 1).min(n - 1)).unwrap_or(0);
                self.set_alarm_list_selected(id, Some(next));
                true
            }
            UiKey::Up => {
                let next = cur.map(|i| i.saturating_sub(1)).unwrap_or(0);
                self.set_alarm_list_selected(id, Some(next));
                true
            }
            UiKey::Enter | UiKey::Space => {
                let _ = self.ack_alarm_list_selected(id);
                true
            }
            _ => false,
        }
    }

    fn key_tree_select(&mut self, id: WidgetId, key: UiKey) -> bool {
        let open = matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::TreeSelect { open: true, .. })
        );
        match key {
            UiKey::Escape if open => {
                self.close_popup_internal();
                true
            }
            UiKey::Enter | UiKey::Space | UiKey::Down if !open => {
                // Open without scene — caller syncs; use empty hide path.
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::TreeSelect { open, .. } = &mut w.kind {
                        *open = true;
                    }
                }
                self.open_popup = Some(id);
                true
            }
            UiKey::Up | UiKey::Down if open => {
                let flats = match self.widgets.get(&id).map(|w| &w.kind) {
                    Some(Kind::TreeSelect { roots, .. }) => flatten_tree(roots),
                    _ => return false,
                };
                if flats.is_empty() {
                    return false;
                }
                let cur = self.tree_select_selected(id);
                let idx = cur
                    .and_then(|cid| flats.iter().position(|r| r.id == cid))
                    .unwrap_or(0);
                let next = match key {
                    UiKey::Down => (idx + 1).min(flats.len() - 1),
                    _ => idx.saturating_sub(1),
                };
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::TreeSelect { selected, .. } = &mut w.kind {
                        *selected = Some(flats[next].id);
                    }
                }
                true
            }
            UiKey::Enter if open => {
                // Keep selection, close.
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::TreeSelect { open, .. } = &mut w.kind {
                        *open = false;
                    }
                }
                self.open_popup = None;
                true
            }
            _ => false,
        }
    }

    fn paint_tree_select_field(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::TreeSelect {
                    roots,
                    selected,
                    open,
                    placeholder,
                    style,
                    ..
                },
            node,
            label_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let text = match *selected {
            Some(sid) => Self::tree_label_by_id(roots, sid).unwrap_or_else(|| placeholder.clone()),
            None => placeholder.clone(),
        };
        let fg = match *selected {
            Some(_) => style.fg,
            None => style.placeholder_fg,
        };
        let open = *open;
        let style = *style;
        let Some(node) = *node else { return };
        let label_node = *label_node;
        let bounds = scene.get(node).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(node, if open { style.selected_bg } else { style.bg });
        scene.set_mask(node, None);
        scene.set_visible(node, true);
        let display = format!("{text}  ▾");
        let (_, mask) = self.font.rasterize_text(&display, style.size, None);
        if let Some(lid) = label_node {
            let lx = bounds.x + style.padding;
            let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
            scene.set_bounds(lid, Rect::new(lx, ly, mask.width as f32, mask.height as f32));
            scene.set_fill(lid, fg);
            scene.set_mask(lid, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(lid, true);
        }
    }

    fn paint_tree_select_popup(&mut self, scene: &mut Scene, id: WidgetId) {
        let (flats, selected, style, scroll_y, host_bounds) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::TreeSelect {
                        roots,
                        selected,
                        style,
                        scroll_y,
                        open,
                        ..
                    },
                node,
                ..
            }) if *open => {
                let b = node
                    .and_then(|n| scene.get(n).map(|n| n.bounds))
                    .unwrap_or_else(Rect::empty);
                (flatten_tree(roots), *selected, *style, *scroll_y, b)
            }
            _ => return,
        };
        let n = flats.len().max(1);
        let content_h = n as f32 * style.row_height;
        let popup_h = content_h.min(style.popup_max_height);
        let panel = Rect::new(
            host_bounds.x,
            host_bounds.y + host_bounds.height,
            host_bounds.width.max(style.width),
            popup_h,
        );
        self.ensure_overlay_aux(scene, id, 1 + flats.len() * 2);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        if let Some(&bg) = aux.first() {
            scene.set_bounds(bg, panel);
            scene.set_fill(bg, style.popup_bg);
            scene.set_mask(bg, None);
            scene.set_clip(bg, Some(panel));
            scene.set_visible(bg, true);
            self.node_to_widget.insert(bg, id);
        }
        for (i, row) in flats.iter().enumerate() {
            let y = panel.y + i as f32 * style.row_height - scroll_y;
            let row_r = Rect::new(panel.x, y, panel.width, style.row_height);
            if row_r.y + row_r.height < panel.y || row_r.y > panel.y + panel.height {
                if let Some(&rbg) = aux.get(1 + i * 2) {
                    scene.set_visible(rbg, false);
                }
                if let Some(&lab) = aux.get(1 + i * 2 + 1) {
                    scene.set_visible(lab, false);
                }
                continue;
            }
            let hi = selected == Some(row.id);
            if let Some(&rbg) = aux.get(1 + i * 2) {
                scene.set_bounds(rbg, row_r);
                scene.set_fill(rbg, if hi { style.selected_bg } else { style.popup_bg });
                scene.set_mask(rbg, None);
                scene.set_visible(rbg, true);
                self.node_to_widget.insert(rbg, id);
            }
            let twiddle = if row.has_children {
                if row.expanded { "▾ " } else { "▸ " }
            } else {
                "  "
            };
            let label = format!(
                "{}{}{}",
                " ".repeat(row.depth.saturating_mul(2)),
                twiddle,
                row.label
            );
            let (_, mask) = self.font.rasterize_text(&label, style.size, None);
            if let Some(&lab) = aux.get(1 + i * 2 + 1) {
                let lx = row_r.x + style.padding;
                let ly = row_r.y + ((style.row_height - mask.height as f32) * 0.5).max(0.0);
                scene.set_bounds(lab, Rect::new(lx, ly, mask.width as f32, mask.height as f32));
                scene.set_fill(lab, style.fg);
                scene.set_mask(lab, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
                scene.set_visible(lab, true);
            }
        }
    }

    fn paint_alarm_list(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::AlarmList {
                    entries,
                    selected,
                    scroll_y,
                    flash_on,
                    style,
                },
            node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let entries = entries.clone();
        let selected = *selected;
        let scroll_y = *scroll_y;
        let flash_on = *flash_on;
        let style = *style;
        let Some(host) = *node else { return };
        let chrome = *chrome_node;
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_clip(host, Some(bounds));
        scene.set_visible(host, true);

        let needed = entries.len() * 2;
        self.ensure_aux_nodes(scene, host, id, needed);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();

        if let Some(chrome) = chrome {
            if let Some(sel) = selected {
                let y = bounds.y + style.padding + sel as f32 * style.row_height - scroll_y;
                scene.set_bounds(
                    chrome,
                    Rect::new(
                        bounds.x + 1.0,
                        y,
                        (bounds.width - 2.0).max(1.0),
                        style.row_height,
                    ),
                );
                scene.set_fill(chrome, style.selected_bg);
                scene.set_mask(chrome, None);
                scene.set_visible(chrome, true);
            } else {
                scene.set_visible(chrome, false);
            }
        }

        for (i, e) in entries.iter().enumerate() {
            let y = bounds.y + style.padding + i as f32 * style.row_height - scroll_y;
            let row = Rect::new(
                bounds.x + style.padding,
                y,
                (bounds.width - style.padding * 2.0).max(1.0),
                style.row_height,
            );
            if row.y + row.height < bounds.y - style.row_height
                || row.y > bounds.y + bounds.height + style.row_height
            {
                if let Some(&bg) = aux.get(i * 2) {
                    scene.set_visible(bg, false);
                }
                if let Some(&lab) = aux.get(i * 2 + 1) {
                    scene.set_visible(lab, false);
                }
                continue;
            }
            let flash = !e.acked && e.severity.flashes() && flash_on;
            if let Some(&bg) = aux.get(i * 2) {
                scene.set_bounds(bg, row);
                scene.set_fill(bg, if flash { style.flash_bg } else { style.bg });
                scene.set_mask(bg, None);
                scene.set_visible(bg, flash);
            }
            let ack = if e.acked { "ACK" } else { "NEW" };
            let line = format!(
                "[{}] {} · {} · {} · {}",
                e.severity.label(),
                e.time,
                e.source,
                e.message,
                ack
            );
            let (_, mask) = self.font.rasterize_text(&line, style.size, None);
            if let Some(&lab) = aux.get(i * 2 + 1) {
                let ly = row.y + ((style.row_height - mask.height as f32) * 0.5).max(0.0);
                scene.set_bounds(
                    lab,
                    Rect::new(row.x + 4.0, ly, mask.width as f32, mask.height as f32),
                );
                scene.set_fill(
                    lab,
                    if selected == Some(i) {
                        Color::rgb(0xff, 0xff, 0xff)
                    } else {
                        e.severity.color()
                    },
                );
                scene.set_mask(lab, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
                scene.set_visible(lab, true);
            }
        }
    }

    fn paint_gauge(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Gauge {
                    value,
                    min,
                    max,
                    unit,
                    label,
                    style,
                },
            node,
            label_node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let value = *value;
        let min = *min;
        let max = *max;
        let unit = unit.clone();
        let label = label.clone();
        let style = *style;
        let Some(node) = *node else { return };
        let label_node = *label_node;
        let chrome = *chrome_node;
        let bounds = scene.get(node).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(node, Color::rgba(0, 0, 0, 0));
        scene.set_mask(node, Some(AlphaMask::empty()));
        scene.set_visible(node, true);

        self.ensure_aux_nodes(scene, node, id, 1);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        let dial_node = chrome;
        let value_node = label_node;
        let caption_node = aux.first().copied();
        self.paint_gauge_tile(
            scene,
            bounds,
            value,
            min,
            max,
            &unit,
            &label,
            style,
            dial_node,
            value_node,
            caption_node,
        );
    }

    fn paint_trend(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Trend { points, style },
            node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let points = points.clone();
        let style = *style;
        let Some(node) = *node else { return };
        let chrome = *chrome_node;
        let bounds = scene.get(node).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_bounds(node, bounds);
        scene.set_fill(node, style.bg);
        scene.set_mask(node, None);
        scene.set_visible(node, true);

        self.ensure_aux_nodes(scene, node, id, 1);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        let line_node = aux.first().copied();
        self.paint_trend_tile(scene, bounds, &points, style, chrome, line_node);
    }

    /// Shared Gauge dial paint (standalone Gauge + Chart tiles).
    #[allow(clippy::too_many_arguments)]
    fn paint_gauge_tile(
        &mut self,
        scene: &mut Scene,
        bounds: Rect,
        value: f32,
        min: f32,
        max: f32,
        unit: &str,
        label: &str,
        style: GaugeStyle,
        dial_node: Option<NodeId>,
        value_node: Option<NodeId>,
        caption_node: Option<NodeId>,
    ) {
        let dial = style.size.min(bounds.width).min(bounds.height.max(style.size * 0.55));
        let mask = p1_raster::rasterize_gauge(
            dial.round() as u32,
            value,
            min,
            max,
            55,
            210,
            255,
        );
        if let Some(chrome) = dial_node {
            let dx = bounds.x + (bounds.width - dial) * 0.5;
            let dy = bounds.y;
            scene.set_bounds(chrome, Rect::new(dx, dy, dial, dial));
            scene.set_fill(chrome, style.dial_fg);
            scene.set_mask(chrome, Some(mask));
            scene.set_visible(chrome, true);
        }
        let value_text = format!("{value:.1}{unit}");
        let (_, vmask) = self.font.rasterize_text(&value_text, style.value_size, None);
        if let Some(lid) = value_node {
            let lx = bounds.x + (bounds.width - vmask.width as f32) * 0.5;
            let ly = bounds.y + dial * 0.55;
            scene.set_bounds(
                lid,
                Rect::new(lx, ly, vmask.width as f32, vmask.height as f32),
            );
            scene.set_fill(lid, style.text_fg);
            scene.set_mask(
                lid,
                Some(AlphaMask::new(vmask.width, vmask.height, vmask.pixels)),
            );
            scene.set_visible(lid, true);
        }
        if let Some(lab) = caption_node {
            let (_, lmask) = self.font.rasterize_text(label, style.label_size, None);
            let lx = bounds.x + (bounds.width - lmask.width as f32) * 0.5;
            let ly = bounds.y + dial + 2.0;
            scene.set_bounds(
                lab,
                Rect::new(lx, ly, lmask.width as f32, lmask.height as f32),
            );
            scene.set_fill(lab, style.text_fg);
            scene.set_mask(
                lab,
                Some(AlphaMask::new(lmask.width, lmask.height, lmask.pixels)),
            );
            scene.set_visible(lab, true);
        }
    }

    /// Shared Trend paint (standalone Trend + Chart tiles).
    fn paint_trend_tile(
        &mut self,
        scene: &mut Scene,
        bounds: Rect,
        points: &[f32],
        style: TrendStyle,
        fill_node: Option<NodeId>,
        line_node: Option<NodeId>,
    ) {
        let w = bounds.width.max(8.0).round() as u32;
        let h = bounds.height.max(8.0).round() as u32;
        let line_mask = p1_raster::rasterize_trend(w, h, points, 255, 0);
        let fill_mask = p1_raster::rasterize_trend(w, h, points, 0, 70);
        if let Some(chrome) = fill_node {
            scene.set_bounds(chrome, bounds);
            scene.set_fill(chrome, style.fill);
            scene.set_mask(chrome, Some(fill_mask));
            scene.set_visible(chrome, true);
        }
        if let Some(line_n) = line_node {
            scene.set_bounds(line_n, bounds);
            scene.set_fill(line_n, style.line);
            scene.set_mask(line_n, Some(line_mask));
            scene.set_visible(line_n, true);
        }
    }

    fn paint_chart(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Chart { tiles, style },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let tiles = tiles.clone();
        let style = *style;
        let Some(node) = *node else { return };
        let bounds = scene.get(node).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_bounds(node, bounds);
        scene.set_fill(node, style.bg);
        scene.set_mask(node, None);
        scene.set_visible(node, true);

        // 4 aux nodes per tile: title, dial/fill, value/line, caption
        let needed = tiles.len() * 4;
        self.ensure_aux_nodes(scene, node, id, needed);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        // Hide unused
        for &n in &aux {
            scene.set_visible(n, false);
        }

        let rects = chart::chart_tile_rects(style, tiles.len());
        for (i, tile) in tiles.iter().enumerate() {
            let Some(&(lx, ly, cw, ch)) = rects.get(i) else {
                break;
            };
            let cell = Rect::new(bounds.x + lx, bounds.y + ly, cw, ch);
            let base = i * 4;
            let title_n = aux.get(base).copied();
            let a = aux.get(base + 1).copied();
            let b = aux.get(base + 2).copied();
            let c = aux.get(base + 3).copied();

            let title_h = style.title_size + 6.0;
            if let Some(tn) = title_n {
                let (_, tmask) = self.font.rasterize_text(&tile.title, style.title_size, None);
                scene.set_bounds(
                    tn,
                    Rect::new(
                        cell.x + 2.0,
                        cell.y + 2.0,
                        tmask.width as f32,
                        tmask.height as f32,
                    ),
                );
                scene.set_fill(tn, style.title_fg);
                scene.set_mask(
                    tn,
                    Some(AlphaMask::new(tmask.width, tmask.height, tmask.pixels)),
                );
                scene.set_visible(tn, true);
            }
            let body = Rect::new(
                cell.x,
                cell.y + title_h,
                cell.width,
                (cell.height - title_h).max(8.0),
            );
            match &tile.kind {
                ChartKind::Gauge {
                    value,
                    min,
                    max,
                    unit,
                    label,
                    style: gs,
                } => {
                    self.paint_gauge_tile(
                        scene, body, *value, *min, *max, unit, label, *gs, a, b, c,
                    );
                }
                ChartKind::Trend { points, style: ts } => {
                    // Subtle per-tile bg via fill layer under line
                    if let Some(fill_n) = a {
                        scene.set_bounds(fill_n, body);
                        scene.set_fill(fill_n, ts.bg);
                        scene.set_mask(fill_n, None);
                        scene.set_visible(fill_n, true);
                    }
                    // paint_trend_tile uses fill for under-curve; use b as fill, c as line
                    // Re-paint over solid bg: allocate fill as under-curve on b, line on c
                    let inset = Rect::new(
                        body.x + 2.0,
                        body.y + 2.0,
                        (body.width - 4.0).max(4.0),
                        (body.height - 4.0).max(4.0),
                    );
                    self.paint_trend_tile(scene, inset, points, *ts, b, c);
                }
            }
        }
    }

}

/// Compact builder for column/row/shell children.
pub struct FlexBuilder<'a> {
    ui: &'a mut Ui,
    parent: WidgetId,
}

impl FlexBuilder<'_> {
    /// Add a text label; returns its [`WidgetId`].
    pub fn text(&mut self, content: impl Into<String>) -> WidgetId {
        self.text_styled(content, Color::rgb(0xe5, 0xe5, 0xe5), 22.0)
    }

    /// Add a text label with explicit color/size (px).
    pub fn text_styled(
        &mut self,
        content: impl Into<String>,
        color: Color,
        size: f32,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Text {
                    content: content.into(),
                    color,
                    size,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Add a button; returns its [`WidgetId`] for hit-testing.
    pub fn button(&mut self, label: impl Into<String>) -> WidgetId {
        self.button_styled(
            label,
            Color::rgb(0x3b, 0x82, 0xf6),
            Color::rgb(0xff, 0xff, 0xff),
        )
    }

    /// Button with custom colors.
    pub fn button_styled(
        &mut self,
        label: impl Into<String>,
        bg: Color,
        fg: Color,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Button {
                    label: label.into(),
                    bg,
                    fg,
                    size: 18.0,
                    min_width: 88.0,
                    height: 36.0,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Compact toolbar button (smaller height).
    pub fn tool_button(&mut self, label: impl Into<String>) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Button {
                    label: label.into(),
                    bg: Color::rgb(0x3b, 0x82, 0xf6),
                    fg: Color::rgb(0xff, 0xff, 0xff),
                    size: 14.0,
                    min_width: 64.0,
                    height: 28.0,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Built-in icon glyph.
    pub fn icon(&mut self, name: IconName) -> WidgetId {
        self.icon_styled(name, 18.0, Color::rgb(0xd0, 0xd4, 0xdc))
    }

    /// Icon from a stable name string (`"save"`, …).
    pub fn icon_named(&mut self, name: &str) -> WidgetId {
        self.icon(IconName::from_name(name))
    }

    /// Icon with size and color.
    pub fn icon_styled(&mut self, name: IconName, size: f32, color: Color) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Icon { name, size, color },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Fixed-size spacer (invisible).
    pub fn spacer(&mut self, width: f32, height: f32) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Spacer { width, height },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Horizontal separator (thin bar along the main cross of a column).
    pub fn separator(&mut self) -> WidgetId {
        self.separator_styled(1.0, 200.0, true, Color::rgb(0x55, 0x55, 0x5a))
    }

    /// Vertical separator for rows (thin upright bar).
    pub fn separator_vertical(&mut self, length: f32) -> WidgetId {
        self.separator_styled(1.0, length, false, Color::rgb(0x55, 0x55, 0x5a))
    }

    /// Separator with explicit geometry.
    pub fn separator_styled(
        &mut self,
        thickness: f32,
        length: f32,
        horizontal: bool,
        color: Color,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Separator {
                    thickness,
                    length,
                    horizontal,
                    color,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Nest a column.
    pub fn column<F>(&mut self, style: ColumnStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc_container(Kind::Column {
            style,
            children: Vec::new(),
        });
        self.push_child(id);
        {
            let mut builder = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut builder);
        }
        id
    }

    /// Nest a row (toolbar strip).
    pub fn row<F>(&mut self, style: RowStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc_container(Kind::Row {
            style,
            children: Vec::new(),
        });
        self.push_child(id);
        {
            let mut builder = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut builder);
        }
        id
    }

    /// Nest a first-class toolbar.
    pub fn toolbar<F>(&mut self, style: ToolbarStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc_container(Kind::Toolbar {
            style,
            children: Vec::new(),
        });
        self.push_child(id);
        {
            let mut builder = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut builder);
        }
        id
    }

    /// Nest a titled panel.
    pub fn panel<F>(&mut self, title: impl Into<String>, style: PanelStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc_container(Kind::Panel {
            title: title.into(),
            style,
            children: Vec::new(),
        });
        self.push_child(id);
        {
            let mut builder = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut builder);
        }
        id
    }

    /// Nest a status bar.
    pub fn status_bar<F>(&mut self, style: StatusBarStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc_container(Kind::StatusBar {
            style,
            children: Vec::new(),
        });
        self.push_child(id);
        {
            let mut builder = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut builder);
        }
        id
    }

    /// Nest a splitter; each closure should add one pane (panel/column).
    pub fn splitter<A, B>(&mut self, style: SplitterStyle, build_a: A, build_b: B) -> WidgetId
    where
        A: FnOnce(&mut FlexBuilder<'_>),
        B: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc_container(Kind::Splitter {
            style,
            children: Vec::new(),
        });
        self.push_child(id);
        {
            let mut builder = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build_a(&mut builder);
        }
        {
            let mut builder = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build_b(&mut builder);
        }
        id
    }


    /// Nest a ScrollArea (clipped viewport; wheel / drag to scroll).
    pub fn scroll_area<F>(&mut self, style: ScrollAreaStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc_container(Kind::ScrollArea {
            style,
            children: Vec::new(),
            scroll_x: 0.0,
            scroll_y: 0.0,
            content_w: style.width,
            content_h: style.height,
        });
        self.push_child(id);
        {
            let mut builder = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut builder);
        }
        id
    }

    /// Single-line text field with optional placeholder.
    pub fn text_input(&mut self, placeholder: impl Into<String>) -> WidgetId {
        self.text_input_styled("", placeholder, TextInputStyle::default())
    }

    /// TextInput with initial value and style.
    pub fn text_input_styled(
        &mut self,
        value: impl Into<String>,
        placeholder: impl Into<String>,
        style: TextInputStyle,
    ) -> WidgetId {
        let value = value.into();
        let caret = value.chars().count();
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::TextInput {
                    value,
                    placeholder: placeholder.into(),
                    caret,
                    select_all: false,
                    preedit: String::new(),
                    preedit_cursor: None,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Checkbox with label.
    pub fn checkbox(&mut self, label: impl Into<String>, checked: bool) -> WidgetId {
        self.checkbox_styled(label, checked, CheckboxStyle::default())
    }

    /// Checkbox with style.
    pub fn checkbox_styled(
        &mut self,
        label: impl Into<String>,
        checked: bool,
        style: CheckboxStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Checkbox {
                    label: label.into(),
                    checked,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Radio button in `group` (mutually exclusive within the group).
    pub fn radio(&mut self, group: u64, label: impl Into<String>, selected: bool) -> WidgetId {
        self.radio_styled(group, label, selected, RadioStyle::default())
    }

    /// Radio with style.
    pub fn radio_styled(
        &mut self,
        group: u64,
        label: impl Into<String>,
        selected: bool,
        style: RadioStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Radio {
                    group,
                    label: label.into(),
                    selected,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// On/off switch with label.
    pub fn switch(&mut self, label: impl Into<String>, on: bool) -> WidgetId {
        self.switch_styled(label, on, SwitchStyle::default())
    }

    /// Switch with style.
    pub fn switch_styled(
        &mut self,
        label: impl Into<String>,
        on: bool,
        style: SwitchStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Switch {
                    label: label.into(),
                    on,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }


    /// Selectable vertical list (built-in scroll; click / ↑↓).
    pub fn list(
        &mut self,
        items: impl IntoIterator<Item = impl Into<String>>,
        style: ListStyle,
    ) -> WidgetId {
        let items: Vec<String> = items.into_iter().map(|s| s.into()).collect();
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::List {
                    items,
                    selected: None,
                    scroll_y: 0.0,
                    virtualized: false,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Hierarchical tree (expand/collapse, click / ↑↓/←→).
    pub fn tree(&mut self, roots: Vec<TreeNode>, style: TreeStyle) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Tree {
                    roots,
                    selected: None,
                    scroll_y: 0.0,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Basic text table (header + rows; row select; column resize stubbed).
    pub fn table(
        &mut self,
        columns: Vec<TableColumn>,
        rows: Vec<Vec<String>>,
        style: TableStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Table {
                    columns,
                    rows,
                    selected_row: None,
                    scroll_y: 0.0,
                    virtualized: false,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Dropdown Menu (title button; click to open item list).
    pub fn menu(
        &mut self,
        title: impl Into<String>,
        items: Vec<MenuItem>,
        style: MenuStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Menu {
                    title: title.into(),
                    items,
                    open: false,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Context-menu host: children layout inside; right-click opens `items`.
    pub fn context_menu<F>(
        &mut self,
        hint: impl Into<String>,
        items: Vec<MenuItem>,
        style: ContextMenuStyle,
        build: F,
    ) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::ContextMenu {
                    hint: hint.into(),
                    items,
                    open: false,
                    anchor_x: 0.0,
                    anchor_y: 0.0,
                    style,
                    children: Vec::new(),
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        {
            let mut b = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut b);
        }
        id
    }

    /// Tabs strip + pages. Each `page` call adds one title and one content child.
    pub fn tabs<F>(&mut self, style: TabsStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut TabsBuilder<'_>),
    {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Tabs {
                    titles: Vec::new(),
                    selected: 0,
                    style,
                    children: Vec::new(),
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        {
            let mut b = TabsBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut b);
        }
        id
    }

    /// Combobox-lite Select (single option from list).
    pub fn select(
        &mut self,
        options: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<usize>,
        placeholder: impl Into<String>,
        style: SelectStyle,
    ) -> WidgetId {
        let options: Vec<String> = options.into_iter().map(|s| s.into()).collect();
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Select {
                    options,
                    selected,
                    open: false,
                    placeholder: placeholder.into(),
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Horizontal Slider in `min..=max`.
    pub fn slider(&mut self, min: f32, max: f32, value: f32, style: SliderStyle) -> WidgetId {
        let lo = min.min(max);
        let hi = min.max(max);
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Slider {
                    min: lo,
                    max: hi,
                    value: value.clamp(lo, hi),
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Compact status Tag chip.
    pub fn tag(&mut self, label: impl Into<String>) -> WidgetId {
        self.tag_styled(label, false, TagStyle::default())
    }

    /// Tag with optional close affordance and style.
    pub fn tag_styled(
        &mut self,
        label: impl Into<String>,
        closable: bool,
        style: TagStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Tag {
                    label: label.into(),
                    closable,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Standalone count Badge pill.
    pub fn badge_count(&mut self, count: u32) -> WidgetId {
        self.badge_styled(BadgeKind::Count(count), BadgeStyle::default())
    }

    /// Standalone Badge dot.
    pub fn badge_dot(&mut self) -> WidgetId {
        self.badge_styled(BadgeKind::Dot, BadgeStyle::default())
    }

    /// Badge with style (standalone).
    pub fn badge_styled(&mut self, kind: BadgeKind, style: BadgeStyle) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Badge {
                    kind,
                    style,
                    children: Vec::new(),
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Badge overlay on a single child (count `0` hides overlay).
    pub fn badge_host<F>(&mut self, kind: BadgeKind, style: BadgeStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Badge {
                    kind,
                    style,
                    children: Vec::new(),
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        {
            let mut b = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut b);
        }
        id
    }

    /// Tooltip host: hover child to show `text` (immediate MVP).
    pub fn tooltip<F>(&mut self, text: impl Into<String>, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        self.tooltip_styled(text, TooltipStyle::default(), build)
    }

    /// Tooltip with style.
    pub fn tooltip_styled<F>(
        &mut self,
        text: impl Into<String>,
        style: TooltipStyle,
        build: F,
    ) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Tooltip {
                    text: text.into(),
                    style,
                    children: Vec::new(),
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        {
            let mut b = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut b);
        }
        id
    }

    /// Determinate Progress bar (`value` in 0..=1).
    pub fn progress(&mut self, value: f32) -> WidgetId {
        self.progress_styled(value, false, ProgressStyle::default())
    }

    /// Progress with style; `indeterminate` paints a stub partial fill.
    pub fn progress_styled(
        &mut self,
        value: f32,
        indeterminate: bool,
        style: ProgressStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Progress {
                    value: value.clamp(0.0, 1.0),
                    indeterminate,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Inline Alert banner.
    pub fn alert(&mut self, tone: AlertTone, message: impl Into<String>) -> WidgetId {
        self.alert_styled(tone, message, true, AlertStyle::default())
    }

    /// Alert with dismissibility and style.
    pub fn alert_styled(
        &mut self,
        tone: AlertTone,
        message: impl Into<String>,
        dismissible: bool,
        style: AlertStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Alert {
                    tone,
                    message: message.into(),
                    dismissible,
                    dismissed: false,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }


    /// TreeSelect: closed field + hierarchical popup (P1; Cascader deferred).
    pub fn tree_select(
        &mut self,
        roots: Vec<TreeNode>,
        selected: Option<u64>,
        placeholder: impl Into<String>,
        style: TreeSelectStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::TreeSelect {
                    roots,
                    selected,
                    open: false,
                    scroll_y: 0.0,
                    placeholder: placeholder.into(),
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Form container with validation rules (P1).
    pub fn form<F>(&mut self, style: FormStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FormBuilder<'_>),
    {
        let id = self.ui.alloc_container(Kind::Form {
            style,
            fields: Vec::new(),
            children: Vec::new(),
        });
        self.push_child(id);
        {
            let mut b = FormBuilder {
                ui: self.ui,
                form: id,
            };
            build(&mut b);
        }
        id
    }

    /// Industrial alarm list with severity + ack (P1).
    pub fn alarm_list(&mut self, entries: Vec<AlarmEntry>, style: AlarmListStyle) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::AlarmList {
                    entries,
                    selected: None,
                    scroll_y: 0.0,
                    flash_on: false,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Dial gauge for a live value (P1).
    pub fn gauge(
        &mut self,
        value: f32,
        min: f32,
        max: f32,
        unit: impl Into<String>,
        label: impl Into<String>,
        style: GaugeStyle,
    ) -> WidgetId {
        let lo = min.min(max);
        let hi = min.max(max);
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Gauge {
                    value: value.clamp(lo, hi),
                    min: lo,
                    max: hi,
                    unit: unit.into(),
                    label: label.into(),
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Trend / sparkline from sample points (P1).
    pub fn trend(&mut self, points: Vec<f32>, style: TrendStyle) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Trend { points, style },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Multi-tile Chart panel (Gauge + Trend grid; Office enhancement).
    pub fn chart(&mut self, tiles: Vec<ChartTile>, style: ChartStyle) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Chart { tiles, style },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    fn push_child(&mut self, child: WidgetId) {
        if let Some(w) = self.ui.widgets.get_mut(&self.parent) {
            match &mut w.kind {
                Kind::Column { children, .. }
                | Kind::Row { children, .. }
                | Kind::Toolbar { children, .. }
                | Kind::Panel { children, .. }
                | Kind::StatusBar { children, .. }
                | Kind::Splitter { children, .. }
                | Kind::ScrollArea { children, .. }
                | Kind::ContextMenu { children, .. }
                | Kind::Tabs { children, .. }
                | Kind::Badge { children, .. }
                | Kind::Tooltip { children, .. }
                | Kind::Form { children, .. }
                | Kind::Spin { children, .. } => {
                    children.push(child);
                }
                _ => {}
            }
        }
    }

    /// Virtualized list: only visible rows allocate scene aux nodes.
    pub fn virtual_list(
        &mut self,
        items: impl IntoIterator<Item = impl Into<String>>,
        style: ListStyle,
    ) -> WidgetId {
        let items: Vec<String> = items.into_iter().map(|s| s.into()).collect();
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::List {
                    items,
                    selected: None,
                    scroll_y: 0.0,
                    virtualized: true,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Virtualized table: windowed body rows.
    pub fn virtual_table(
        &mut self,
        columns: Vec<TableColumn>,
        rows: Vec<Vec<String>>,
        style: TableStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Table {
                    columns,
                    rows,
                    selected_row: None,
                    scroll_y: 0.0,
                    virtualized: true,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Multi-line TextArea.
    pub fn text_area(
        &mut self,
        value: impl Into<String>,
        placeholder: impl Into<String>,
        style: TextAreaStyle,
    ) -> WidgetId {
        let value = value.into();
        let caret = value.chars().count();
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::TextArea {
                    value,
                    placeholder: placeholder.into(),
                    caret,
                    scroll_y: 0.0,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Numeric stepper InputNumber.
    pub fn input_number(
        &mut self,
        value: f64,
        min: f64,
        max: f64,
        step: f64,
        style: InputNumberStyle,
    ) -> WidgetId {
        let lo = min.min(max);
        let hi = min.max(max);
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::InputNumber {
                    value: value.clamp(lo, hi),
                    min: lo,
                    max: hi,
                    step: step.abs().max(1e-9),
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Spin / loading overlay host with optional child.
    pub fn spin<F>(&mut self, spinning: bool, style: SpinStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Spin {
                    spinning,
                    style,
                    children: Vec::new(),
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        {
            let mut b = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut b);
        }
        id
    }

    /// Empty state placeholder.
    pub fn empty(&mut self, description: impl Into<String>, style: EmptyStyle) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Empty {
                    description: description.into(),
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Collapse accordion.
    pub fn collapse(&mut self, panels: Vec<CollapsePanel>, style: CollapseStyle) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Collapse { panels, style },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Descriptions key-value list.
    pub fn descriptions(
        &mut self,
        items: Vec<DescriptionItem>,
        style: DescriptionsStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Descriptions { items, style },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Card with title + body.
    pub fn card(
        &mut self,
        title: impl Into<String>,
        body: impl Into<String>,
        style: CardStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Card {
                    title: title.into(),
                    body: body.into(),
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Segmented control.
    pub fn segmented(
        &mut self,
        options: impl IntoIterator<Item = impl Into<String>>,
        selected: usize,
        style: SegmentedStyle,
    ) -> WidgetId {
        let options: Vec<String> = options.into_iter().map(|s| s.into()).collect();
        let selected = if options.is_empty() {
            0
        } else {
            selected.min(options.len() - 1)
        };
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Segmented {
                    options,
                    selected,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Breadcrumb path.
    pub fn breadcrumb(
        &mut self,
        items: impl IntoIterator<Item = impl Into<String>>,
        style: BreadcrumbStyle,
    ) -> WidgetId {
        let items: Vec<String> = items.into_iter().map(|s| s.into()).collect();
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Breadcrumb { items, style },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Pagination control (`page` is 1-based).
    pub fn pagination(
        &mut self,
        page: usize,
        page_size: usize,
        total: usize,
        style: PaginationStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Pagination {
                    page: page.max(1),
                    page_size: page_size.max(1),
                    total,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// DateTime picker (field + calendar/time popup).
    pub fn datetime(&mut self, value: DateTimeValue, style: DateTimeStyle) -> WidgetId {
        let value = value.clamp_day();
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::DateTime {
                    value,
                    draft: value,
                    open: false,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Transfer / shuttle dual list.
    pub fn transfer(
        &mut self,
        source: Vec<TransferItem>,
        target: Vec<TransferItem>,
        style: TransferStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Transfer {
                    source,
                    target,
                    source_selected: None,
                    target_selected: None,
                    filter: String::new(),
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// PropertyGrid key-value editor.
    pub fn property_grid(
        &mut self,
        rows: Vec<PropertyRow>,
        style: PropertyGridStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::PropertyGrid {
                    rows,
                    selected: None,
                    scroll_y: 0.0,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

    /// TreeTable hierarchical rows + columns.
    pub fn tree_table(
        &mut self,
        columns: Vec<TableColumn>,
        roots: Vec<TreeTableNode>,
        style: TreeTableStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::TreeTable {
                    columns,
                    roots,
                    selected: None,
                    scroll_y: 0.0,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }

}


/// Builder for [`FlexBuilder::form`] fields.
pub struct FormBuilder<'a> {
    ui: &'a mut Ui,
    form: WidgetId,
}

impl FormBuilder<'_> {
    /// Add a labeled field with validation rules; `build` should create exactly one control
    /// (typically [`FlexBuilder::text_input`]). Returns the control [`WidgetId`].
    pub fn field<F>(
        &mut self,
        key: impl Into<String>,
        label: impl Into<String>,
        rules: Vec<FormRule>,
        build: F,
    ) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let key = key.into();
        let label = label.into();
        let (label_size, label_fg, error_size, error_fg, before) =
            match self.ui.widgets.get(&self.form) {
                Some(Widget {
                    kind: Kind::Form { style, children, .. },
                    ..
                }) => (
                    style.label_size,
                    style.label_fg,
                    style.error_size,
                    style.error_fg,
                    children.len(),
                ),
                _ => (
                    12.0,
                    Color::rgb(0xb0, 0xb8, 0xc4),
                    11.0,
                    Color::rgb(0xf0, 0x70, 0x70),
                    0,
                ),
            };

        {
            let mut b = FlexBuilder {
                ui: self.ui,
                parent: self.form,
            };
            b.text_styled(label, label_fg, label_size);
            build(&mut b);
            b.text_styled("", error_fg, error_size);
        }

        let (control, error_label) = match self.ui.widgets.get(&self.form) {
            Some(Widget {
                kind: Kind::Form { children, .. },
                ..
            }) => {
                // label at before, control at before+1, error at before+2
                let control = children.get(before + 1).copied().unwrap_or(WidgetId(0));
                let error_label = children.get(before + 2).copied().unwrap_or(WidgetId(0));
                (control, error_label)
            }
            _ => (WidgetId(0), WidgetId(0)),
        };
        if let Some(w) = self.ui.widgets.get_mut(&self.form) {
            if let Kind::Form { fields, .. } = &mut w.kind {
                fields.push(FormFieldState {
                    key,
                    control,
                    error_label,
                    rules,
                });
            }
        }
        control
    }
}

/// Builder for [`FlexBuilder::tabs`] pages.
pub struct TabsBuilder<'a> {
    ui: &'a mut Ui,
    parent: WidgetId,
}

impl TabsBuilder<'_> {
    /// Add a tab page with `title` and content built into a nested column.
    pub fn page<F>(&mut self, title: impl Into<String>, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let title = title.into();
        let page_style = {
            let Some(w) = self.ui.widgets.get_mut(&self.parent) else {
                return WidgetId(0);
            };
            let Kind::Tabs { titles, style, .. } = &mut w.kind else {
                return WidgetId(0);
            };
            titles.push(title);
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: (style.width - style.padding * 2.0).max(40.0),
                height: (style.height - style.strip_height - style.padding * 2.0).max(40.0),
                padding: style.padding,
                gap: style.gap,
                cross_align: style.cross_align,
                background: None,
            }
        };
        let id = self.ui.alloc_container(Kind::Column {
            style: page_style,
            children: Vec::new(),
        });
        if let Some(w) = self.ui.widgets.get_mut(&self.parent) {
            if let Kind::Tabs { children, .. } = &mut w.kind {
                children.push(id);
            }
        }
        {
            let mut b = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut b);
        }
        id
    }
}

/// Alias kept for call sites that used the P1 name.
pub type ColumnBuilder<'a> = FlexBuilder<'a>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_column_and_hit_button() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut label = WidgetId(0);
        let mut inc = WidgetId(0);
        ui.column(ColumnStyle::default(), |col| {
            label = col.text("Count: 0");
            inc = col.button("Increment");
        });
        ui.sync(&mut scene);
        let btn_node = ui.widgets.get(&inc).unwrap().node.unwrap();
        let b = scene.get(btn_node).unwrap().bounds;
        let hit = ui
            .hit_widget(&scene, b.x + 5.0, b.y + 5.0)
            .expect("hit");
        assert_eq!(hit, inc);
        assert!(ui.is_button(inc));
        ui.set_text(label, "Count: 1");
        ui.sync(&mut scene);
        assert_eq!(ui.text_content(label), Some("Count: 1"));
    }

    #[test]
    fn row_toolbar_lays_out_buttons() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut open = WidgetId(0);
        let mut save = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 640.0,
                height: 400.0,
                padding: 12.0,
                gap: 10.0,
                ..ColumnStyle::default()
            },
            |col| {
                col.row(
                    RowStyle {
                        width: 600.0,
                        height: 44.0,
                        ..RowStyle::default()
                    },
                    |row| {
                        open = row.button("Open");
                        row.spacer(12.0, 1.0);
                        row.separator_vertical(24.0);
                        row.spacer(12.0, 1.0);
                        save = row.button("Save");
                    },
                );
                col.text("Workspace");
            },
        );
        ui.sync(&mut scene);
        let a = scene
            .get(ui.widgets.get(&open).unwrap().node.unwrap())
            .unwrap()
            .bounds;
        let b = scene
            .get(ui.widgets.get(&save).unwrap().node.unwrap())
            .unwrap()
            .bounds;
        assert!(b.x > a.x + a.width, "Save should sit to the right of Open");
        assert!(ui.is_button(open) && ui.is_button(save));
    }

    #[test]
    fn cjk_text_gets_mask() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut label = WidgetId(0);
        ui.column(ColumnStyle::default(), |col| {
            label = col.text("你好工业 Hello");
        });
        ui.sync(&mut scene);
        let node = ui.widgets.get(&label).unwrap().node.unwrap();
        let n = scene.get(node).unwrap();
        assert!(n.mask.as_ref().is_some_and(|m| !m.is_empty()));
    }

    #[test]
    fn office_shell_panel_toolbar_splitter_statusbar() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut splitter = WidgetId(0);
        let mut status = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 560.0,
                padding: 8.0,
                gap: 6.0,
                background: Some(Color::rgb(0x1a, 0x1a, 0x1c)),
                ..ColumnStyle::default()
            },
            |col| {
                col.toolbar(
                    ToolbarStyle {
                        width: 780.0,
                        ..ToolbarStyle::default()
                    },
                    |tb| {
                        tb.icon(IconName::Menu);
                        tb.spacer(8.0, 1.0);
                        tb.tool_button("Open");
                        tb.icon(IconName::Save);
                        tb.tool_button("Save");
                    },
                );
                splitter = col.splitter(
                    SplitterStyle {
                        width: 780.0,
                        height: 460.0,
                        axis: Axis::Vertical,
                        ratio: 0.28,
                        ..SplitterStyle::default()
                    },
                    |left| {
                        left.panel(
                            "Navigator",
                            PanelStyle {
                                width: 200.0,
                                height: 450.0,
                                ..PanelStyle::default()
                            },
                            |p| {
                                p.text_styled("Plant / Bay", Color::rgb(0xc0, 0xc4, 0xcc), 14.0);
                            },
                        );
                    },
                    |right| {
                        right.panel(
                            "Workspace",
                            PanelStyle {
                                width: 500.0,
                                height: 450.0,
                                ..PanelStyle::default()
                            },
                            |p| {
                                p.text_styled(
                                    "Document body",
                                    Color::rgb(0xc0, 0xc4, 0xcc),
                                    14.0,
                                );
                            },
                        );
                    },
                );
                col.status_bar(
                    StatusBarStyle {
                        width: 780.0,
                        ..StatusBarStyle::default()
                    },
                    |sb| {
                        status = sb.text_styled("Ready", Color::rgb(0x8a, 0xe6, 0x9a), 12.0);
                        sb.spacer(24.0, 1.0);
                        sb.text_styled("Soft", Color::rgb(0xa0, 0xa8, 0xb4), 12.0);
                    },
                );
            },
        );
        ui.sync(&mut scene);
        assert!(ui.is_splitter(splitter));
        assert_eq!(ui.text_content(status), Some("Ready"));

        let sash = ui.widgets.get(&splitter).unwrap().chrome_node.unwrap();
        let sb = scene.get(sash).unwrap().bounds;
        let cx = sb.x + sb.width * 0.5;
        let cy = sb.y + sb.height * 0.5;
        assert!(ui.pointer_pressed(&mut scene, cx, cy));
        assert!(ui.pointer_moved(cx + 40.0, cy));
        ui.pointer_released();
        ui.sync(&mut scene);
        let ratio = ui.splitter_ratio(splitter).unwrap();
        assert!(
            ratio > 0.28,
            "drag right should grow first pane, got {ratio}"
        );
    }

    #[test]
    fn office_inputs_scroll_text_toggles() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut scroll = WidgetId(0);
        let mut input = WidgetId(0);
        let mut check = WidgetId(0);
        let mut radio_a = WidgetId(0);
        let mut radio_b = WidgetId(0);
        let mut sw = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 400.0,
                height: 500.0,
                ..ColumnStyle::default()
            },
            |col| {
                scroll = col.scroll_area(
                    ScrollAreaStyle {
                        width: 360.0,
                        height: 120.0,
                        ..ScrollAreaStyle::default()
                    },
                    |sa| {
                        for i in 0..12 {
                            sa.text_styled(
                                format!("row {i}"),
                                Color::rgb(0xc0, 0xc4, 0xcc),
                                13.0,
                            );
                        }
                    },
                );
                input = col.text_input("Type here");
                check = col.checkbox("Enable", false);
                radio_a = col.radio(1, "Opt A", true);
                radio_b = col.radio(1, "Opt B", false);
                sw = col.switch("Power", false);
            },
        );
        ui.sync(&mut scene);
        assert!(ui.is_scroll_area(scroll));
        assert!(ui.is_text_input(input));
        assert_eq!(ui.is_checked(check), Some(false));
        assert_eq!(ui.is_checked(radio_a), Some(true));

        let inode = ui.widgets.get(&input).unwrap().node.unwrap();
        let b = scene.get(inode).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, b.x + 4.0, b.y + 4.0));
        assert_eq!(ui.focused(), Some(input));
        assert!(ui.key_input(UiKey::Char('H')));
        assert!(ui.key_input(UiKey::Char('i')));
        assert_eq!(ui.text_input_value(input), Some("Hi"));
        assert!(ui.key_input(UiKey::SelectAll));
        assert!(ui.key_input(UiKey::Backspace));
        assert_eq!(ui.text_input_value(input), Some(""));

        // IME: preedit does not mutate value; commit inserts CJK at caret.
        assert!(ui.ime_preedit("ni", Some((2, 2))));
        assert_eq!(ui.text_input_value(input), Some(""));
        assert!(ui.ime_commit("你"));
        assert_eq!(ui.text_input_value(input), Some("你"));
        assert!(ui.ime_preedit("hao", Some((3, 3))));
        assert!(ui.ime_commit("好"));
        assert_eq!(ui.text_input_value(input), Some("你好"));
        assert!(ui.text_input_wants_ime());
        assert!(ui.ime_clear_preedit());

        // Clipboard helpers (platform owns OS clipboard; Ui only supplies text).
        assert!(ui.paste_into_focused_text("AB"));
        assert_eq!(ui.text_input_value(input), Some("你好AB"));
        assert_eq!(ui.focused_text_for_copy().as_deref(), Some("你好AB"));
        assert!(ui.key_input(UiKey::SelectAll));
        assert!(ui.paste_into_focused_text("XY"));
        assert_eq!(ui.text_input_value(input), Some("XY"));

        let cnode = ui.widgets.get(&check).unwrap().node.unwrap();
        let cb = scene.get(cnode).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, cb.x + 2.0, cb.y + 2.0));
        assert_eq!(ui.is_checked(check), Some(true));

        let rnode = ui.widgets.get(&radio_b).unwrap().node.unwrap();
        let rb = scene.get(rnode).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, rb.x + 2.0, rb.y + 2.0));
        assert_eq!(ui.is_checked(radio_a), Some(false));
        assert_eq!(ui.is_checked(radio_b), Some(true));

        let snode = ui.widgets.get(&sw).unwrap().node.unwrap();
        let sb = scene.get(snode).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, sb.x + 2.0, sb.y + 2.0));
        assert_eq!(ui.is_checked(sw), Some(true));

        let sarea = ui.widgets.get(&scroll).unwrap().node.unwrap();
        let sa = scene.get(sarea).unwrap().bounds;
        assert!(ui.pointer_scroll(&scene, sa.x + 10.0, sa.y + 10.0, 0.0, 40.0));
        let (_, sy) = ui.scroll_offset(scroll).unwrap();
        assert!(sy > 0.0, "expected scroll_y > 0, got {sy}");
        ui.sync(&mut scene);

        assert!(scene.get(sarea).unwrap().clip.is_some());
    }

    #[test]
    fn text_input_ime_preedit_paints_cjk_and_underline() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut input = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 400.0,
                height: 120.0,
                ..ColumnStyle::default()
            },
            |col| {
                input = col.text_input("type");
            },
        );
        ui.sync(&mut scene);
        let inode = ui.widgets.get(&input).unwrap().node.unwrap();
        let b = scene.get(inode).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, b.x + 4.0, b.y + 4.0));
        assert!(ui.ime_commit("你好"));
        assert!(ui.ime_preedit("世界", Some((0, 6))));
        ui.sync(&mut scene);
        assert_eq!(ui.text_input_value(input), Some("你好"));
        let label = ui.widgets.get(&input).unwrap().label_node.unwrap();
        let ln = scene.get(label).unwrap();
        assert!(ln.visible);
        let mask = ln.mask.as_ref().expect("label mask");
        assert!(mask.width > 8 && mask.height > 8, "CJK+preedit mask too small");
        // Underline aux should be visible while composing.
        let ul = ui.widgets.get(&input).unwrap().aux[0];
        let un = scene.get(ul).unwrap();
        assert!(un.visible, "expected preedit underline visible");
        assert!(un.bounds.width > 1.0);
        // Clear preedit hides underline.
        assert!(ui.ime_clear_preedit());
        ui.sync(&mut scene);
        assert!(!scene.get(ul).unwrap().visible);
    }

    #[test]
    fn office_data_list_tree_table() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut list = WidgetId(0);
        let mut tree = WidgetId(0);
        let mut table = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 640.0,
                height: 480.0,
                ..ColumnStyle::default()
            },
            |col| {
                list = col.list(
                    (1..=8).map(|i| format!("Tag-{i:02}")),
                    ListStyle {
                        width: 200.0,
                        height: 140.0,
                        ..ListStyle::default()
                    },
                );
                tree = col.tree(
                    vec![TreeNode::new(
                        1,
                        "Plant A",
                        vec![
                            TreeNode::leaf(2, "Bay-01"),
                            TreeNode::new(3, "Bay-02", vec![TreeNode::leaf(4, "CB-1")]),
                        ],
                    )],
                    TreeStyle {
                        width: 220.0,
                        height: 160.0,
                        ..TreeStyle::default()
                    },
                );
                table = col.table(
                    vec![
                        TableColumn::new("Tag", 100.0),
                        TableColumn::new("Status", 80.0),
                        TableColumn::new("Value", 80.0),
                    ],
                    vec![
                        vec!["CB-1".into(), "Closed".into(), "0.0".into()],
                        vec!["CB-2".into(), "Open".into(), "1.2".into()],
                        vec!["DS-1".into(), "Closed".into(), "0.0".into()],
                    ],
                    TableStyle {
                        width: 300.0,
                        height: 140.0,
                        ..TableStyle::default()
                    },
                );
            },
        );
        ui.sync(&mut scene);
        assert!(ui.is_list(list));
        assert!(ui.is_tree(tree));
        assert!(ui.is_table(table));

        let lnode = ui.widgets.get(&list).unwrap().node.unwrap();
        let lb = scene.get(lnode).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, lb.x + 8.0, lb.y + 8.0));
        assert_eq!(ui.list_selected(list), Some(0));
        assert_eq!(ui.focused(), Some(list));
        assert!(ui.key_input(UiKey::Down));
        assert_eq!(ui.list_selected(list), Some(1));

        let tnode = ui.widgets.get(&tree).unwrap().node.unwrap();
        let tb = scene.get(tnode).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, tb.x + 20.0, tb.y + 8.0));
        assert_eq!(ui.tree_selected(tree), Some(1));
        ui.set_focus(Some(tree));
        assert!(ui.key_input(UiKey::Down));
        assert_eq!(ui.tree_selected(tree), Some(2));

        let tabn = ui.widgets.get(&table).unwrap().node.unwrap();
        let tab = scene.get(tabn).unwrap().bounds;
        let style_h = 28.0; // header
        assert!(ui.pointer_pressed(
            &mut scene,
            tab.x + 10.0,
            tab.y + style_h + 8.0
        ));
        assert_eq!(ui.table_selected_row(table), Some(0));
        ui.set_focus(Some(table));
        assert!(ui.key_input(UiKey::Down));
        assert_eq!(ui.table_selected_row(table), Some(1));

        assert!(ui.pointer_scroll(&scene, lb.x + 10.0, lb.y + 10.0, 0.0, 40.0));
        ui.sync(&mut scene);
        assert!(scene.get(lnode).unwrap().clip.is_some());
    }

    #[test]
    fn office_nav_menu_tabs_select_slider() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut menu = WidgetId(0);
        let mut ctx = WidgetId(0);
        let mut tabs = WidgetId(0);
        let mut sel = WidgetId(0);
        let mut slider = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 520.0,
                height: 480.0,
                ..ColumnStyle::default()
            },
            |col| {
                col.row(
                    RowStyle {
                        width: 480.0,
                        height: 32.0,
                        ..RowStyle::default()
                    },
                    |row| {
                        menu = row.menu(
                            "File",
                            vec![
                                MenuItem::new(1, "Open"),
                                MenuItem::new(2, "Save"),
                                MenuItem::disabled(3, "Export"),
                            ],
                            MenuStyle::default(),
                        );
                    },
                );
                ctx = col.context_menu(
                    "Right-click me",
                    vec![MenuItem::new(10, "Inspect"), MenuItem::new(11, "Delete")],
                    ContextMenuStyle {
                        width: 260.0,
                        height: 80.0,
                        ..ContextMenuStyle::default()
                    },
                    |c| {
                        c.text_styled("Host area", Color::rgb(0xc0, 0xc4, 0xcc), 13.0);
                    },
                );
                tabs = col.tabs(
                    TabsStyle {
                        width: 400.0,
                        height: 140.0,
                        ..TabsStyle::default()
                    },
                    |tb| {
                        tb.page("General", |p| {
                            p.text_styled("General page", Color::rgb(0xc0, 0xc4, 0xcc), 13.0);
                        });
                        tb.page("Alarms", |p| {
                            p.text_styled("Alarms page", Color::rgb(0xc0, 0xc4, 0xcc), 13.0);
                        });
                    },
                );
                sel = col.select(
                    ["Auto", "Manual", "Off"],
                    Some(0),
                    "Mode",
                    SelectStyle::default(),
                );
                slider = col.slider(0.0, 100.0, 40.0, SliderStyle::default());
            },
        );
        ui.sync(&mut scene);
        assert!(ui.is_menu(menu));
        assert!(ui.is_context_menu(ctx));
        assert!(ui.is_tabs(tabs));
        assert!(ui.is_select(sel));
        assert!(ui.is_slider(slider));
        assert_eq!(ui.tabs_selected(tabs), Some(0));
        assert_eq!(ui.select_selected(sel), Some(0));
        assert!((ui.slider_value(slider).unwrap() - 40.0).abs() < 0.01);

        // Open menu and activate Save
        let mnode = ui.widgets.get(&menu).unwrap().node.unwrap();
        let mb = scene.get(mnode).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, mb.x + 4.0, mb.y + 4.0));
        ui.sync(&mut scene);
        let popup = ui.widgets.get(&menu).unwrap().aux[0];
        let pb = scene.get(popup).unwrap().bounds;
        assert!(scene.get(popup).unwrap().visible);
        // second item (Save)
        assert!(ui.pointer_press(&mut scene, pb.x + 8.0, pb.y + 28.0 + 8.0, false));
        assert_eq!(ui.take_menu_action(), Some((menu, 2)));

        // Tabs click second
        let tnode = ui.widgets.get(&tabs).unwrap().node.unwrap();
        let tb = scene.get(tnode).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, tb.x + tb.width * 0.75, tb.y + 8.0));
        assert_eq!(ui.tabs_selected(tabs), Some(1));

        // Select open + pick Manual
        let snode = ui.widgets.get(&sel).unwrap().node.unwrap();
        let sb = scene.get(snode).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, sb.x + 4.0, sb.y + 4.0));
        ui.sync(&mut scene);
        let sp = ui.widgets.get(&sel).unwrap().aux[0];
        let spb = scene.get(sp).unwrap().bounds;
        assert!(ui.pointer_press(&mut scene, spb.x + 8.0, spb.y + 26.0 + 4.0, false));
        assert_eq!(ui.select_selected(sel), Some(1));

        // Context menu secondary
        let cnode = ui.widgets.get(&ctx).unwrap().node.unwrap();
        let cb = scene.get(cnode).unwrap().bounds;
        assert!(ui.pointer_press(&mut scene, cb.x + 20.0, cb.y + 20.0, true));
        ui.sync(&mut scene);
        let cp = ui.widgets.get(&ctx).unwrap().aux[0];
        assert!(scene.get(cp).unwrap().visible);
        let cpb = scene.get(cp).unwrap().bounds;
        assert!(ui.pointer_press(&mut scene, cpb.x + 8.0, cpb.y + 8.0, false));
        assert_eq!(ui.take_menu_action(), Some((ctx, 10)));

        // Slider drag toward max
        let sln = ui.widgets.get(&slider).unwrap().node.unwrap();
        let slb = scene.get(sln).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, slb.x + slb.width * 0.8, slb.y + slb.height * 0.5));
        ui.pointer_released();
        let v = ui.slider_value(slider).unwrap();
        assert!(v > 50.0, "expected slider > 50 after click, got {v}");

        ui.set_focus(Some(slider));
        assert!(ui.key_input(UiKey::Right));
        assert!(ui.slider_value(slider).unwrap() > v);
    }

    #[test]
    fn office_feedback_tag_badge_tooltip_modal_progress_alert() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        ui.set_viewport(640.0, 480.0);
        let mut tag = WidgetId(0);
        let mut badge = WidgetId(0);
        let mut tip = WidgetId(0);
        let mut progress = WidgetId(0);
        let mut alert = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 600.0,
                height: 400.0,
                ..ColumnStyle::default()
            },
            |col| {
                col.row(
                    RowStyle {
                        width: 560.0,
                        height: 32.0,
                        gap: 8.0,
                        ..RowStyle::default()
                    },
                    |r| {
                        tag = r.tag_styled(
                            "Running",
                            true,
                            TagStyle {
                                bg: Color::rgb(0x1e, 0x3d, 0x2a),
                                ..TagStyle::default()
                            },
                        );
                        badge = r.badge_count(3);
                        tip = r.tooltip("Save document", |t| {
                            t.icon(IconName::Save);
                        });
                    },
                );
                progress = col.progress(0.4);
                alert = col.alert(AlertTone::Warning, "Threshold near limit");
            },
        );
        let modal = ui.modal(
            "Confirm",
            "Delete this tag from the list?",
            "Delete",
            Some("Cancel".into()),
            ModalStyle::default(),
        );
        ui.sync(&mut scene);
        assert!(ui.is_tag(tag));
        assert!(ui.is_badge(badge));
        assert!(ui.is_tooltip(tip));
        assert!(ui.is_progress(progress));
        assert!(ui.is_alert(alert));
        assert!(ui.is_modal(modal));
        assert!((ui.progress_value(progress).unwrap() - 0.4).abs() < 0.01);
        assert!(ui.set_progress_value(progress, 0.75));
        assert!((ui.progress_value(progress).unwrap() - 0.75).abs() < 0.01);

        // Tooltip hover
        let tnode = ui.widgets.get(&tip).unwrap().node.unwrap();
        let tb = scene.get(tnode).unwrap().bounds;
        assert!(ui.pointer_hover(&scene, tb.x + 4.0, tb.y + 4.0));
        ui.sync(&mut scene);
        assert!(ui.widgets.get(&tip).unwrap().aux.iter().any(|&n| {
            scene.get(n).map(|n| n.visible).unwrap_or(false)
        }));

        // Alert dismiss
        let anode = ui.widgets.get(&alert).unwrap().chrome_node.unwrap();
        let ab = scene.get(anode).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, ab.x + 1.0, ab.y + 1.0));
        assert_eq!(ui.take_alert_dismiss(), Some(alert));
        assert_eq!(ui.alert_dismissed(alert), Some(true));

        // Modal open / primary
        assert!(ui.set_modal_open(modal, true));
        assert_eq!(ui.modal_open(modal), Some(true));
        ui.sync(&mut scene);
        let aux = &ui.widgets.get(&modal).unwrap().aux;
        assert!(scene.get(aux[0]).unwrap().visible); // scrim
        let primary = scene.get(aux[6]).unwrap().bounds;
        assert!(ui.pointer_press(
            &mut scene,
            primary.x + primary.width * 0.5,
            primary.y + primary.height * 0.5,
            false
        ));
        assert_eq!(ui.take_modal_action(), Some((modal, ModalAction::Primary)));
        assert_eq!(ui.modal_open(modal), Some(false));

        // Tag close
        ui.sync(&mut scene);
        let cnode = ui.widgets.get(&tag).unwrap().chrome_node.unwrap();
        let cb = scene.get(cnode).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, cb.x + 1.0, cb.y + 1.0));
        assert_eq!(ui.take_tag_close(), Some(tag));
    }

    /// Headless soft-render of a P1-like counter: label/button glyphs must not be
    /// bottom-clipped (regression for fontdue ymin placement).
    #[test]
    fn headless_counter_text_not_vertically_clipped() {
        use next_gui_render::{encode_scene, Renderer};
        use next_gui_render_soft::SoftRenderer;

        let w = 480u32;
        let h = 420u32;
        let clear = Color::rgb(0x20, 0x20, 0x20);
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut label = WidgetId(0);
        let mut btn = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 40.0,
                y: 40.0,
                width: 360.0,
                height: 520.0,
                padding: 20.0,
                gap: 14.0,
                background: Some(Color::rgb(0x2a, 0x2a, 0x2e)),
                ..ColumnStyle::default()
            },
            |col| {
                label = col.text("Count: 0");
                btn = col.button("Increment");
            },
        );
        ui.sync(&mut scene);
        scene.mark_full_dirty(w as f32, h as f32);
        let dirty = scene.take_dirty();
        let cmds = encode_scene(&scene, &dirty);
        let mut soft = SoftRenderer::new(w, h, clear);
        soft.render(&cmds, &dirty);
        let (pixels, fw, fh) = soft.frame_pixels().expect("cpu pixels");
        assert_eq!((fw, fh), (w, h));

        let node = ui.widgets.get(&label).unwrap().node.unwrap();
        let b = scene.get(node).unwrap().bounds;
        let x0 = b.x.floor().max(0.0) as u32;
        let y0 = b.y.floor().max(0.0) as u32;
        let x1 = ((b.x + b.width).ceil() as u32).min(w);
        let y1 = ((b.y + b.height).ceil() as u32).min(h);
        let mut ink_rows = Vec::new();
        for y in y0..y1 {
            let mut ink = false;
            for x in x0..x1 {
                let px = pixels[(y * fw + x) as usize];
                let r = (px >> 16) & 0xff;
                let g = (px >> 8) & 0xff;
                let bl = px & 0xff;
                if r > 0x60 || g > 0x60 || bl > 0x60 {
                    ink = true;
                    break;
                }
            }
            if ink {
                ink_rows.push(y);
            }
        }
        assert!(
            !ink_rows.is_empty(),
            "label bounds ({x0},{y0})-({x1},{y1}) have no bright pixels"
        );
        let first = *ink_rows.first().unwrap();
        let last = *ink_rows.last().unwrap();
        let span = last - first + 1;
        let box_h = y1 - y0;
        assert!(
            first < y0 + box_h / 2,
            "label ink starts at bottom half (first={first} y0={y0} h={box_h})"
        );
        assert!(
            span >= 14,
            "label ink span {span} too thin — still vertically clipped?"
        );

        let lid = ui.widgets.get(&btn).unwrap().label_node.unwrap();
        let lb = scene.get(lid).unwrap().bounds;
        let lx0 = lb.x.floor().max(0.0) as u32;
        let ly0 = lb.y.floor().max(0.0) as u32;
        let lx1 = ((lb.x + lb.width).ceil() as u32).min(w);
        let ly1 = ((lb.y + lb.height).ceil() as u32).min(h);
        let mut b_rows = Vec::new();
        for y in ly0..ly1 {
            let mut ink = false;
            for x in lx0..lx1 {
                let px = pixels[(y * fw + x) as usize];
                let r = (px >> 16) & 0xff;
                if r > 0x80 {
                    ink = true;
                    break;
                }
            }
            if ink {
                b_rows.push(y);
            }
        }
        assert!(!b_rows.is_empty(), "button label has no bright pixels");
        let bspan = b_rows.last().unwrap() - b_rows.first().unwrap() + 1;
        assert!(
            bspan >= 12,
            "button label ink span {bspan} too thin — clipped?"
        );

        let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/text-qa");
        let _ = std::fs::create_dir_all(&out_dir);
        let path = out_dir.join("headless_counter.ppm");
        let mut body = format!("P3\n{w} {h}\n255\n");
        for y in 0..h {
            for x in 0..w {
                let px = pixels[(y * fw + x) as usize];
                let r = (px >> 16) & 0xff;
                let g = (px >> 8) & 0xff;
                let bchan = px & 0xff;
                body.push_str(&format!("{r} {g} {bchan} "));
            }
            body.push('\n');
        }
        std::fs::write(&path, body).expect("write ppm");
        eprintln!(
            "headless counter QA: label span={span} button span={bspan} -> {}",
            path.display()
        );
    }


    #[test]
    fn office_p1_tree_select_form_alarm_gauge_trend() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut tree_sel = WidgetId(0);
        let mut form = WidgetId(0);
        let mut alarms = WidgetId(0);
        let mut gauge = WidgetId(0);
        let mut trend = WidgetId(0);
        let plant = vec![TreeNode::new(
            1,
            "Plant",
            vec![
                TreeNode::leaf(11, "Bay-01"),
                TreeNode::leaf(12, "Bay-02"),
            ],
        )];
        ui.column(
            ColumnStyle {
                width: 640.0,
                height: 720.0,
                ..ColumnStyle::default()
            },
            |col| {
                tree_sel = col.tree_select(
                    plant,
                    None,
                    "Pick bay…",
                    TreeSelectStyle::default(),
                );
                form = col.form(FormStyle::default(), |f| {
                    f.field(
                        "name",
                        "Name",
                        vec![FormRule::required("name required")],
                        |b| {
                            b.text_input("device name");
                        },
                    );
                    f.field(
                        "code",
                        "Code",
                        vec![
                            FormRule::required("code required"),
                            FormRule::digits("digits only"),
                            FormRule::min_length(3, "min 3"),
                        ],
                        |b| {
                            b.text_input("code");
                        },
                    );
                });
                alarms = col.alarm_list(
                    vec![
                        AlarmEntry::new(
                            1,
                            AlarmSeverity::Critical,
                            "Trip",
                            "CB-31",
                            "10:01:02",
                        ),
                        AlarmEntry::new(
                            2,
                            AlarmSeverity::Info,
                            "Comm OK",
                            "GW-1",
                            "10:01:05",
                        ),
                    ],
                    AlarmListStyle {
                        height: 120.0,
                        ..AlarmListStyle::default()
                    },
                );
                gauge = col.gauge(72.5, 0.0, 100.0, "%", "Load", GaugeStyle::default());
                trend = col.trend(
                    vec![1.0, 2.0, 1.5, 3.0, 2.5, 4.0],
                    TrendStyle::default(),
                );
            },
        );
        ui.sync(&mut scene);
        assert!(ui.is_tree_select(tree_sel));
        assert!(ui.is_form(form));
        assert!(ui.is_alarm_list(alarms));
        assert!(ui.is_gauge(gauge));
        assert!(ui.is_trend(trend));

        assert!(!ui.validate_form(form));
        let name = ui.form_field_control(form, "name").unwrap();
        let code = ui.form_field_control(form, "code").unwrap();
        ui.set_text_input(name, "Feeder-1");
        ui.set_text_input(code, "12"); // too short
        assert!(!ui.validate_form(form));
        ui.set_text_input(code, "123");
        assert!(ui.validate_form(form));

        let tnode = ui.widgets.get(&tree_sel).unwrap().node.unwrap();
        let tb = scene.get(tnode).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, tb.x + 4.0, tb.y + 4.0));
        ui.sync(&mut scene);
        // Click first leaf-ish row in popup (index 1 after root)
        let aux0 = ui.widgets.get(&tree_sel).unwrap().aux[0];
        let panel = scene.get(aux0).unwrap().bounds;
        assert!(ui.pointer_pressed(
            &mut scene,
            panel.x + 20.0,
            panel.y + TreeSelectStyle::default().row_height * 1.5
        ));
        ui.sync(&mut scene);
        assert_eq!(ui.tree_select_selected(tree_sel), Some(11));

        let anode = ui.widgets.get(&alarms).unwrap().node.unwrap();
        let ab = scene.get(anode).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, ab.x + 8.0, ab.y + 8.0));
        assert_eq!(ui.alarm_list_selected(alarms), Some(0));
        assert_eq!(ui.ack_alarm_list_selected(alarms), Some(1));
        assert!(ui.set_alarm_list_flash(alarms, true));

        assert!(ui.set_gauge_value(gauge, 80.0));
        assert_eq!(ui.gauge_value(gauge), Some(80.0));
        assert!(ui.set_trend_points(trend, vec![0.0, 1.0, 0.5]));
        ui.sync(&mut scene);
    }

    #[test]
    fn office_charts_multi_tile_gauge_trend() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut chart = WidgetId(0);
        let mut gauge = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 700.0,
                height: 500.0,
                ..ColumnStyle::default()
            },
            |col| {
                chart = col.chart(
                    vec![
                        ChartTile::gauge(
                            "A",
                            40.0,
                            0.0,
                            100.0,
                            "%",
                            "Load",
                            GaugeStyle::default(),
                        ),
                        ChartTile::trend(
                            "B",
                            vec![1.0, 2.0, 3.0, 2.5, 4.0],
                            TrendStyle::default(),
                        ),
                        ChartTile::gauge(
                            "C",
                            70.0,
                            0.0,
                            100.0,
                            "°C",
                            "Temp",
                            GaugeStyle::default(),
                        ),
                        ChartTile::trend(
                            "D",
                            vec![5.0, 4.0, 6.0, 7.0, 5.5],
                            TrendStyle {
                                line: Color::rgb(0xe0, 0x70, 0xb0),
                                ..TrendStyle::default()
                            },
                        ),
                    ],
                    ChartStyle {
                        width: 640.0,
                        height: 320.0,
                        columns: 2,
                        ..ChartStyle::default()
                    },
                );
                gauge = col.gauge(55.0, 0.0, 100.0, "%", "PF", GaugeStyle::default());
            },
        );
        ui.sync(&mut scene);
        assert!(ui.is_chart(chart));
        assert_eq!(ui.chart_tile_count(chart), Some(4));
        assert!(ui.set_chart_gauge_value(chart, 0, 88.0));
        assert!(ui.set_chart_trend_points(chart, 1, vec![1.0, 1.5, 2.0]));
        assert!(!ui.set_chart_gauge_value(chart, 1, 10.0)); // trend tile
        assert!(ui.is_gauge(gauge));
        assert!(ui.set_gauge_value(gauge, 66.0));
    }

    #[test]
    fn office_p1_rest_drawer_virtual_list_and_structure() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        ui.set_viewport(900.0, 640.0);
        let mut vlist = WidgetId(0);
        let mut vtable = WidgetId(0);
        let mut area = WidgetId(0);
        let mut num = WidgetId(0);
        let mut seg = WidgetId(0);
        let mut crumb = WidgetId(0);
        let mut page = WidgetId(0);
        let mut card = WidgetId(0);
        let mut empty = WidgetId(0);
        let mut collapse = WidgetId(0);
        let mut desc = WidgetId(0);
        let mut spin = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 880.0,
                height: 620.0,
                padding: 8.0,
                gap: 6.0,
                ..ColumnStyle::default()
            },
            |col| {
                crumb = col.breadcrumb(
                    ["Plant", "Bay-03", "Meter"],
                    BreadcrumbStyle::default(),
                );
                seg = col.segmented(["Table", "Chart", "Log"], 0, SegmentedStyle::default());
                let items: Vec<String> = (0..5000).map(|i| format!("Point-{i:04}")).collect();
                vlist = col.virtual_list(
                    items,
                    ListStyle {
                        width: 220.0,
                        height: 120.0,
                        ..ListStyle::default()
                    },
                );
                let rows: Vec<Vec<String>> = (0..2000)
                    .map(|i| vec![format!("{i}"), format!("Tag-{i}"), "OK".into()])
                    .collect();
                vtable = col.virtual_table(
                    vec![
                        TableColumn::new("Id", 48.0),
                        TableColumn::new("Name", 100.0),
                        TableColumn::new("St", 40.0),
                    ],
                    rows,
                    TableStyle {
                        width: 260.0,
                        height: 120.0,
                        ..TableStyle::default()
                    },
                );
                area = col.text_area("", "notes…", TextAreaStyle::default());
                num = col.input_number(42.0, 0.0, 100.0, 1.0, InputNumberStyle::default());
                page = col.pagination(1, 50, 500, PaginationStyle::default());
                card = col.card("Bay", "Voltage nominal", CardStyle::default());
                empty = col.empty("No alarms", EmptyStyle::default());
                collapse = col.collapse(
                    vec![
                        CollapsePanel::new("Group A", "Settings A", true),
                        CollapsePanel::new("Group B", "Settings B", false),
                    ],
                    CollapseStyle::default(),
                );
                desc = col.descriptions(
                    vec![
                        DescriptionItem::new("Plant", "North"),
                        DescriptionItem::new("Bay", "03"),
                    ],
                    DescriptionsStyle::default(),
                );
                spin = col.spin(true, SpinStyle::default(), |s| {
                    s.text("content");
                });
            },
        );
        let drawer = ui.drawer(
            "Config",
            "Side panel body",
            DrawerStyle {
                placement: DrawerPlacement::Right,
                ..DrawerStyle::default()
            },
        );
        ui.notify("Saved", NotificationTone::Success);
        ui.sync(&mut scene);
        assert!(ui.is_list(vlist));
        assert!(ui.is_table(vtable));
        assert_eq!(ui.segmented_selected(seg), Some(0));
        assert!(ui.set_segmented_selected(seg, 1));
        assert_eq!(ui.input_number_value(num), Some(42.0));
        assert!(ui.set_input_number_value(num, 43.0));
        assert!(ui.set_text_area_value(area, "hello\nworld"));
        assert_eq!(ui.text_area_value(area), Some("hello\nworld"));
        ui.set_focus(Some(area));
        ui.sync(&mut scene);
        // F07: multiline mask taller than one line; caret chrome visible when focused.
        let lab = ui.widgets.get(&area).unwrap().label_node.expect("textarea label");
        let mask_h = scene
            .get(lab)
            .and_then(|n| n.mask.as_ref())
            .map(|m| m.height)
            .unwrap_or(0);
        assert!(mask_h > 20, "multiline textarea mask height={mask_h}");
        let chrome = ui.widgets.get(&area).unwrap().chrome_node.expect("textarea chrome");
        let host = ui.widgets.get(&area).unwrap().node.expect("textarea host");
        let host_bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        let caret_bounds = scene.get(chrome).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        assert!(
            scene.get(chrome).map(|n| n.visible).unwrap_or(false),
            "textarea caret chrome visible"
        );
        assert!(
            host_bounds.intersects(&caret_bounds) && !caret_bounds.is_empty(),
            "short multiline caret bounds {caret_bounds:?} must intersect host {host_bounds:?}"
        );
        // Auto-wrap: long run with no newlines must keep caret inside the host clip.
        let wrapped: String = format!("line1\nline2{}", "W".repeat(44));
        assert!(ui.set_text_area_value(area, wrapped));
        ui.set_focus(Some(area));
        ui.sync(&mut scene);
        let caret_bounds = scene.get(chrome).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        let host_bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        assert!(
            scene.get(chrome).map(|n| n.visible).unwrap_or(false),
            "wrapped textarea caret chrome visible"
        );
        assert!(
            !caret_bounds.is_empty(),
            "wrapped caret should have non-empty bounds"
        );
        assert!(
            caret_bounds.x >= host_bounds.x - 0.5
                && caret_bounds.x + caret_bounds.width <= host_bounds.x + host_bounds.width + 0.5,
            "wrapped caret x clipped: caret={caret_bounds:?} host={host_bounds:?}"
        );
        assert!(
            caret_bounds.y + caret_bounds.height > host_bounds.y
                && caret_bounds.y < host_bounds.y + host_bounds.height,
            "wrapped caret y offscreen: caret={caret_bounds:?} host={host_bounds:?}"
        );
        assert_eq!(ui.pagination_page(page), Some(1));
        assert!(ui.set_pagination_page(page, 2));
        assert!(ui.set_drawer_open(drawer, true));
        assert_eq!(ui.drawer_open(drawer), Some(true));
        ui.sync(&mut scene);
        // Virtual list should not allocate 5000*2 aux nodes
        let aux_n = ui.widgets.get(&vlist).map(|w| w.aux.len()).unwrap_or(0);
        assert!(aux_n < 200, "virtual list aux={aux_n}");
        let taux = ui.widgets.get(&vtable).map(|w| w.aux.len()).unwrap_or(0);
        assert!(taux < 400, "virtual table aux={taux}");
        let _ = (card, empty, collapse, desc, spin, crumb);
        ui.set_drawer_open(drawer, false);
        assert!(ui.take_drawer_action().is_none());
    }

    #[test]
    fn office_charts_soft_raster_qa_ppm() {
        use next_gui_render::{encode_scene, Renderer};
        use next_gui_render_soft::SoftRenderer;
        use std::io::Write;

        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let w = 1120u32;
        let h = 820u32;
        ui.set_viewport(w as f32, h as f32);
        ui.column(
            ColumnStyle {
                x: 12.0,
                y: 12.0,
                width: w as f32 - 24.0,
                height: h as f32 - 24.0,
                padding: 8.0,
                gap: 8.0,
                background: Some(Color::rgb(0x1e, 0x1e, 0x22)),
                ..ColumnStyle::default()
            },
            |col| {
                col.text_styled(
                    "Office charts QA",
                    Color::rgb(0xa0, 0xa8, 0xb4),
                    14.0,
                );
                col.chart(
                    vec![
                        ChartTile::gauge(
                            "Load",
                            62.0,
                            0.0,
                            100.0,
                            "%",
                            "MW",
                            GaugeStyle {
                                size: 110.0,
                                ..GaugeStyle::default()
                            },
                        ),
                        ChartTile::gauge(
                            "Temp",
                            48.0,
                            0.0,
                            120.0,
                            "°C",
                            "Oil",
                            GaugeStyle {
                                size: 110.0,
                                dial_fg: Color::rgb(0xf0, 0xa0, 0x4a),
                                ..GaugeStyle::default()
                            },
                        ),
                        ChartTile::trend(
                            "Voltage A",
                            vec![
                                9.5, 9.8, 10.1, 10.0, 10.2, 9.9, 10.3, 10.4, 10.1, 10.0, 9.8,
                                10.2,
                            ],
                            TrendStyle {
                                line: Color::rgb(0x4a, 0xc0, 0xf0),
                                ..TrendStyle::default()
                            },
                        ),
                        ChartTile::trend(
                            "Current B",
                            vec![
                                70.0, 75.0, 80.0, 78.0, 85.0, 90.0, 88.0, 92.0, 95.0, 91.0, 87.0,
                                93.0,
                            ],
                            TrendStyle {
                                line: Color::rgb(0xe0, 0x70, 0xb0),
                                fill: Color::rgb(0x40, 0x1a, 0x35),
                                ..TrendStyle::default()
                            },
                        ),
                    ],
                    ChartStyle {
                        width: w as f32 - 48.0,
                        height: 340.0,
                        columns: 2,
                        ..ChartStyle::default()
                    },
                );
                col.row(
                    RowStyle {
                        width: w as f32 - 48.0,
                        height: 180.0,
                        gap: 12.0,
                        background: None,
                        ..RowStyle::default()
                    },
                    |r| {
                        r.gauge(
                            55.0,
                            0.0,
                            100.0,
                            "%",
                            "PF",
                            GaugeStyle {
                                size: 120.0,
                                dial_fg: Color::rgb(0x6a, 0xb0, 0xf0),
                                ..GaugeStyle::default()
                            },
                        );
                        r.trend(
                            vec![40.0, 45.0, 50.0, 48.0, 55.0, 60.0, 58.0, 62.0],
                            TrendStyle {
                                width: 400.0,
                                height: 120.0,
                                ..TrendStyle::default()
                            },
                        );
                    },
                );
            },
        );
        ui.sync(&mut scene);
        scene.mark_full_dirty(w as f32, h as f32);
        let dirty = scene.take_dirty();
        let cmds = encode_scene(&scene, &dirty);
        let clear = Color {
            r: 0x1a,
            g: 0x1a,
            b: 0x1c,
            a: 255,
        };
        let mut soft = SoftRenderer::new(w, h, clear);
        soft.render(&cmds, &dirty);
        let (pixels, pw, ph) = soft.frame_pixels().expect("pixels");
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tmp/demo-shots/23-office_charts.ppm");
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).expect("create QA directory");
        }
        let mut f = std::fs::File::create(&out).expect("create ppm");
        writeln!(f, "P6\n{pw} {ph}\n255").unwrap();
        for px in pixels {
            let r = ((px >> 16) & 0xff) as u8;
            let g = ((px >> 8) & 0xff) as u8;
            let b = (px & 0xff) as u8;
            f.write_all(&[r, g, b]).unwrap();
        }
        assert!(out.exists());
        assert_eq!((pw, ph), (w, h));
        // Chart grid may leave the absolute center empty; sample a known chrome pixel.
        let sample = pixels[(120usize) * pw as usize + 80];
        assert_ne!(sample, 0);
        assert!(pixels.iter().any(|&p| p != 0));
    }

    #[test]
    fn office_p1_rest_soft_raster_qa_ppm() {
        use next_gui_render::{encode_scene, Renderer};
        use next_gui_render_soft::SoftRenderer;
        use std::io::Write;

        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let w = 1100u32;
        let h = 780u32;
        ui.set_viewport(w as f32, h as f32);
        ui.column(
            ColumnStyle {
                x: 12.0,
                y: 12.0,
                width: w as f32 - 24.0,
                height: h as f32 - 24.0,
                padding: 8.0,
                gap: 8.0,
                background: Some(Color::rgb(0x1e, 0x1e, 0x22)),
                ..ColumnStyle::default()
            },
            |col| {
                col.breadcrumb(["Plant", "Bay-03", "Meter"], BreadcrumbStyle::default());
                col.segmented(["Table", "Chart", "Log"], 1, SegmentedStyle::default());
                let items: Vec<String> = (0..200).map(|i| format!("P-{i}")).collect();
                col.virtual_list(
                    items,
                    ListStyle {
                        width: 240.0,
                        height: 140.0,
                        ..ListStyle::default()
                    },
                );
                col.text_area("hello\nline2", "notes", TextAreaStyle::default());
                col.input_number(12.0, 0.0, 100.0, 1.0, InputNumberStyle::default());
                col.card("Card", "Body text", CardStyle::default());
                col.empty("Empty state", EmptyStyle::default());
                col.pagination(2, 20, 200, PaginationStyle::default());
            },
        );
        let drawer = ui.drawer(
            "Config",
            "Side drawer body for QA",
            DrawerStyle {
                placement: DrawerPlacement::Right,
                ..DrawerStyle::default()
            },
        );
        let _ = ui.set_drawer_open(drawer, true);
        ui.notify("QA toast", NotificationTone::Success);
        ui.sync(&mut scene);
        scene.mark_full_dirty(w as f32, h as f32);
        let dirty = scene.take_dirty();
        let cmds = encode_scene(&scene, &dirty);
        let clear = Color {
            r: 0x1a,
            g: 0x1a,
            b: 0x1c,
            a: 255,
        };
        let mut soft = SoftRenderer::new(w, h, clear);
        soft.render(&cmds, &dirty);
        let (pixels, pw, ph) = soft.frame_pixels().expect("pixels");
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tmp/demo-shots/21-office_p1_rest.ppm");
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).expect("create QA directory");
        }
        let mut f = std::fs::File::create(&out).expect("create ppm");
        writeln!(f, "P6\n{pw} {ph}\n255").unwrap();
        for px in pixels {
            let r = ((px >> 16) & 0xff) as u8;
            let g = ((px >> 8) & 0xff) as u8;
            let b = (px & 0xff) as u8;
            f.write_all(&[r, g, b]).unwrap();
        }
        assert!(out.exists());
        assert_eq!((pw, ph), (w, h));
        // Non-empty: sample a mid-frame pixel should not be pure clear for all
        let mid = pixels[(ph as usize / 2) * pw as usize + pw as usize / 2];
        assert_ne!(mid, 0);
    }

    #[test]
    fn office_p1_deferred_datetime_transfer_property_tree_table() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        ui.set_viewport(900.0, 700.0);
        let mut dt = WidgetId(0);
        let mut xfer = WidgetId(0);
        let mut grid = WidgetId(0);
        let mut tt = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 880.0,
                height: 680.0,
                padding: 8.0,
                gap: 8.0,
                ..ColumnStyle::default()
            },
            |col| {
                dt = col.datetime(
                    DateTimeValue::new(2026, 10, 18, 9, 15),
                    DateTimeStyle::default(),
                );
                xfer = col.transfer(
                    vec![TransferItem::new(1, "A"), TransferItem::new(2, "B")],
                    vec![TransferItem::new(3, "C")],
                    TransferStyle {
                        width: 480.0,
                        height: 200.0,
                        ..TransferStyle::default()
                    },
                );
                grid = col.property_grid(
                    vec![
                        PropertyRow::new("On", PropertyValue::Bool(false)),
                        PropertyRow::new("N", PropertyValue::Number(1.0)),
                        PropertyRow::new(
                            "Mode",
                            PropertyValue::Select {
                                options: vec!["A".into(), "B".into()],
                                selected: 0,
                            },
                        ),
                    ],
                    PropertyGridStyle {
                        width: 300.0,
                        height: 140.0,
                        ..PropertyGridStyle::default()
                    },
                );
                tt = col.tree_table(
                    vec![TableColumn::new("Type", 80.0)],
                    vec![TreeTableNode::new(
                        1,
                        "Root",
                        vec!["Plant".into()],
                        vec![TreeTableNode::leaf(2, "Leaf", vec!["AI".into()])],
                    )],
                    TreeTableStyle {
                        width: 400.0,
                        height: 180.0,
                        ..TreeTableStyle::default()
                    },
                );
            },
        );
        ui.sync(&mut scene);

        assert_eq!(
            ui.datetime_value(dt).unwrap().format_display(),
            "2026-10-18 09:15"
        );
        assert!(ui.set_datetime_open(dt, true));
        assert_eq!(ui.datetime_open(dt), Some(true));
        ui.sync(&mut scene);
        // F06: calendar aux must live under the raised overlay host, not the field.
        let overlay = ui.overlay_host.expect("overlay host after open datetime");
        let aux0 = ui.widgets.get(&dt).unwrap().aux[0];
        assert_eq!(
            scene.get(aux0).and_then(|n| n.parent),
            Some(overlay),
            "datetime popup aux parent"
        );
        ui.set_focus(Some(dt));
        assert!(ui.key_input(UiKey::Enter));
        assert_eq!(ui.datetime_open(dt), Some(false));
        ui.set_focus(None);
        ui.sync(&mut scene);

        // Transfer: select first source row then move to target.
        let xn = ui.widgets.get(&xfer).unwrap().node.unwrap();
        let xb = scene.get(xn).unwrap().bounds;
        let style = TransferStyle {
            width: 480.0,
            height: 200.0,
            ..TransferStyle::default()
        };
        let body_y = xb.y + 28.0 + 4.0;
        assert!(ui.pointer_pressed(&mut scene, xb.x + 8.0, body_y));
        ui.set_focus(Some(xfer));
        assert!(ui.key_input(UiKey::Right));
        assert_eq!(ui.transfer_target(xfer).unwrap().len(), 2);

        // PropertyGrid: click value of row 0 (bool) to toggle.
        let gn = ui.widgets.get(&grid).unwrap().node.unwrap();
        let gb = scene.get(gn).unwrap().bounds;
        let row0_y = gb.y + PropertyGridStyle::default().row_height + 4.0;
        assert!(ui.pointer_pressed(
            &mut scene,
            gb.x + PropertyGridStyle::default().key_width + 8.0,
            row0_y
        ));
        match ui.property_grid_value(grid, 0) {
            Some(PropertyValue::Bool(true)) => {}
            other => panic!("expected bool true, got {other:?}"),
        }

        assert!(ui.set_tree_table_selected(tt, Some(2)));
        assert_eq!(ui.tree_table_selected(tt), Some(2));
        ui.set_focus(Some(tt));
        assert!(ui.key_input(UiKey::Up));
        assert_eq!(ui.tree_table_selected(tt), Some(1));
        ui.sync(&mut scene);
        let _ = style;
    }

    #[test]
    fn office_p1_deferred_soft_raster_qa_ppm() {
        use next_gui_render::{encode_scene, Renderer};
        use next_gui_render_soft::SoftRenderer;
        use std::io::Write;

        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let w = 1120u32;
        let h = 820u32;
        ui.set_viewport(w as f32, h as f32);
        let mut dt = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 12.0,
                y: 12.0,
                width: w as f32 - 24.0,
                height: h as f32 - 24.0,
                padding: 8.0,
                gap: 8.0,
                background: Some(Color::rgb(0x1e, 0x1e, 0x22)),
                ..ColumnStyle::default()
            },
            |col| {
                col.text_styled(
                    "P1 deferred QA",
                    Color::rgb(0xa0, 0xa8, 0xb4),
                    12.0,
                );
                dt = col.datetime(
                    DateTimeValue::new(2026, 10, 18, 8, 30),
                    DateTimeStyle::default(),
                );
                col.transfer(
                    vec![
                        TransferItem::new(1, "Point-01"),
                        TransferItem::new(2, "Point-02"),
                        TransferItem::new(3, "Point-03"),
                    ],
                    vec![TransferItem::new(9, "Chosen")],
                    TransferStyle {
                        width: 520.0,
                        height: 200.0,
                        ..TransferStyle::default()
                    },
                );
                col.property_grid(
                    vec![
                        PropertyRow::new("Name", PropertyValue::Text("Bay-03".into())),
                        PropertyRow::new("Enabled", PropertyValue::Bool(true)),
                        PropertyRow::new("kV", PropertyValue::Number(10.5)),
                    ],
                    PropertyGridStyle {
                        width: 320.0,
                        height: 120.0,
                        ..PropertyGridStyle::default()
                    },
                );
                col.tree_table(
                    vec![
                        TableColumn::new("Type", 80.0),
                        TableColumn::new("Value", 80.0),
                    ],
                    vec![TreeTableNode::new(
                        1,
                        "Plant",
                        vec!["Plant".into(), "—".into()],
                        vec![TreeTableNode::leaf(
                            2,
                            "Ua",
                            vec!["AI".into(), "10.2".into()],
                        )],
                    )],
                    TreeTableStyle {
                        width: 480.0,
                        height: 180.0,
                        ..TreeTableStyle::default()
                    },
                );
            },
        );
        let _ = ui.set_datetime_open(dt, true);
        ui.sync(&mut scene);
        scene.mark_full_dirty(w as f32, h as f32);
        let dirty = scene.take_dirty();
        let cmds = encode_scene(&scene, &dirty);
        let clear = Color {
            r: 0x1a,
            g: 0x1a,
            b: 0x1c,
            a: 255,
        };
        let mut soft = SoftRenderer::new(w, h, clear);
        soft.render(&cmds, &dirty);
        let (pixels, pw, ph) = soft.frame_pixels().expect("pixels");
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tmp/demo-shots/22-office_p1_deferred.ppm");
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).expect("create QA directory");
        }
        let mut f = std::fs::File::create(&out).expect("create ppm");
        writeln!(f, "P6\n{pw} {ph}\n255").unwrap();
        for px in pixels {
            let r = ((px >> 16) & 0xff) as u8;
            let g = ((px >> 8) & 0xff) as u8;
            let b = (px & 0xff) as u8;
            f.write_all(&[r, g, b]).unwrap();
        }
        assert!(out.exists());
        assert_eq!((pw, ph), (w, h));
        let mid = pixels[(ph as usize / 2) * pw as usize + pw as usize / 2];
        assert_ne!(mid, 0);
    }

    #[test]
    fn accessible_name_override_and_button_fallback() {
        let mut ui = Ui::new();
        let mut btn = WidgetId(0);
        let mut input = WidgetId(0);
        ui.column(ColumnStyle::default(), |col| {
            btn = col.button("Save");
            input = col.text_input("Name…");
        });
        assert_eq!(ui.accessible_name(btn).as_deref(), Some("Save"));
        assert_eq!(ui.accessible_name(input).as_deref(), Some("Name…"));
        ui.set_accessible_name(btn, "保存文件");
        assert_eq!(ui.accessible_name(btn).as_deref(), Some("保存文件"));
        ui.clear_accessible_name(btn);
        assert_eq!(ui.accessible_name(btn).as_deref(), Some("Save"));
        ui.set_text_input(input, "Alice");
        assert_eq!(ui.accessible_name(input).as_deref(), Some("Alice"));
    }

    #[test]
    fn focusable_order_and_tab_wrap() {
        let mut ui = Ui::new();
        let mut a = WidgetId(0);
        let mut btn = WidgetId(0);
        let mut b = WidgetId(0);
        let mut c = WidgetId(0);
        ui.column(ColumnStyle::default(), |col| {
            a = col.text_input("A");
            btn = col.button("Go"); // Button participates in Tab order
            b = col.checkbox("B", false);
            c = col.switch("C", false);
        });
        let order = ui.focusable_widgets();
        assert_eq!(order, vec![a, btn, b, c], "DFS roots order; button included");
        assert!(ui.focused().is_none());
        assert_eq!(ui.focus_next(), Some(a));
        assert_eq!(ui.focus_next(), Some(btn));
        assert_eq!(ui.focus_next(), Some(b));
        assert_eq!(ui.focus_next(), Some(c));
        assert_eq!(ui.focus_next(), Some(a), "wrap forward");
        assert_eq!(ui.focus_prev(), Some(c), "wrap backward");
        assert_eq!(ui.focus_prev(), Some(b));
        ui.set_focus(None);
        assert_eq!(ui.focus_prev(), Some(c), "no focus + prev → last");
    }

    #[test]
    fn button_tab_reach_and_keyboard_activate() {
        let mut ui = Ui::new();
        let mut input = WidgetId(0);
        let mut btn = WidgetId(0);
        ui.column(ColumnStyle::default(), |col| {
            input = col.text_input("x");
            btn = col.button("Do it");
        });
        assert!(ui.key_input(UiKey::Tab));
        assert_eq!(ui.focused(), Some(input));
        assert!(ui.key_input(UiKey::Tab));
        assert_eq!(ui.focused(), Some(btn));
        assert!(ui.take_button_activate().is_none());
        assert!(ui.key_input(UiKey::Space));
        assert_eq!(ui.take_button_activate(), Some(btn));
        assert!(ui.take_button_activate().is_none(), "drain once");
        assert_eq!(ui.focused(), Some(btn));
        assert!(ui.key_input(UiKey::Enter));
        assert_eq!(ui.take_button_activate(), Some(btn));
    }

    #[test]
    fn pointer_on_button_sets_focus_but_returns_false() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut btn = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 80.0,
                ..ColumnStyle::default()
            },
            |col| {
                btn = col.button("Click");
            },
        );
        ui.sync(&mut scene);
        let node = ui.widgets.get(&btn).unwrap().node.unwrap();
        let b = scene.get(node).unwrap().bounds;
        assert!(!ui.pointer_pressed(&mut scene, b.x + 4.0, b.y + 4.0));
        assert_eq!(ui.focused(), Some(btn));
    }

    #[test]
    fn key_input_tab_moves_focus_without_prior_focus() {
        let mut ui = Ui::new();
        let mut a = WidgetId(0);
        let mut b = WidgetId(0);
        ui.column(ColumnStyle::default(), |col| {
            a = col.text_input("one");
            b = col.text_input("two");
        });
        assert!(ui.key_input(UiKey::Tab));
        assert_eq!(ui.focused(), Some(a));
        assert!(ui.key_input(UiKey::Tab));
        assert_eq!(ui.focused(), Some(b));
        assert!(ui.key_input(UiKey::BackTab));
        assert_eq!(ui.focused(), Some(a));
        // Tab while focused on text field must move focus, not insert tab.
        assert!(ui.key_input(UiKey::Tab));
        assert_eq!(ui.focused(), Some(b));
        assert_eq!(ui.text_input_value(a).unwrap(), "");
    }

    #[test]
    fn key_input_tab_absorbed_by_modal_trap() {
        let mut ui = Ui::new();
        let mut input = WidgetId(0);
        ui.column(ColumnStyle::default(), |col| {
            input = col.text_input("x");
        });
        let modal = ui.modal("Title", "Body", "OK", None, ModalStyle::default());
        ui.set_modal_open(modal, true);
        assert_eq!(ui.focused(), Some(modal));
        assert!(ui.key_input(UiKey::Tab));
        assert_eq!(ui.focused(), Some(modal), "trap keeps focus");
        assert!(ui.key_input(UiKey::BackTab));
        assert_eq!(ui.focused(), Some(modal));
    }
}
