//! Compact declarative UI API for next-gui.
//!
//! Retained widget tree built with a short builder or loaded from a compact `.ngui` file,
//! synced into [`next_gui_scene::Scene`]. Text uses [`next_gui_text`] (font metrics + CJK).
//!
//! Office P0 + P1 widgets: shell/input/data/nav/feedback, TreeSelect/Form/AlarmList/Gauge/Trend,
//! plus Drawer, virtual list/table, TextArea, InputNumber, Notification, Spin, Empty, Collapse,
//! Descriptions, Card, Segmented, Breadcrumb, Pagination, DateTime, Transfer, PropertyGrid,
//! TreeTable, and multi-tile Chart. TextInput supports IME (CJK). Cascader stays deferred.
//! Office P2a–P2e (`button_group` … `config_provider`): see `docs/office-p2-design.md`.
//! ScrollArea, Table, List, Tree and TextArea display overlay scrollbars when needed.
//! Drag the thumb or click its track; focus navigation reveals enclosing scroll viewports.
//! Theme tokens (MVP): [`Theme`] / [`Ui::set_theme`]; see `docs/theme-tokens.md`.
//! P6d a11y baseline: [`Ui::accessible_name`], [`Ui::focusable_widgets`] / Tab focus order
//! (Button included; [`Ui::take_button_activate`] for Space/Enter).

#![deny(missing_docs)]

mod dsl;
mod icon;
mod p1_deferred;
mod chart;
mod p1_raster;
mod p1_rest;
mod office_p2;
mod theme;
mod text_area;
mod scrollbars;
pub use text_area::TextAreaState;
use text_area::TextAreaEdit;

pub use dsl::{load_ngui, load_ngui_path, DslError, LoadedUi};
pub use icon::IconName;
pub use chart::{ChartKind, ChartStyle, ChartTile};
pub use p1_deferred::*;
pub use p1_rest::*;
pub use office_p2::*;
pub use theme::{Theme, ThemeColors, ThemeMode, ThemeRadii, ThemeSpacing, ThemeTypography};

use std::collections::HashMap;
use std::sync::Arc;

use next_gui_layout::{layout_flex, Axis, ChildMeasure, CrossAlign, FlexParams};
use next_gui_scene::{AlphaMask, Color, NodeId, Rect, RgbaImage, Scene};
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


/// Retained image magnification and pan, independent of pixel resources.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ImageView {
    /// None fits the image; Some(1.0) displays one image pixel per scene pixel.
    pub scale: Option<f32>,
    /// Horizontal pan from the centered image, positive toward the right edge.
    pub offset_x: f32,
    /// Vertical pan from the centered image, positive toward the bottom edge.
    pub offset_y: f32,
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
    /// Column width (px). Drag a header's right edge to resize it.
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

// Shared geometry keeps painting, sorting and resizing aligned behind frozen columns.
fn table_column_rects(
    bounds: Rect,
    columns: &[TableColumn],
    padding: f32,
    scroll_x: f32,
    frozen: usize,
) -> Vec<(f32, Rect)> {
    let left = bounds.x + padding;
    let frozen_width: f32 = columns.iter().take(frozen).map(|c| c.width.max(1.0)).sum();
    let mut x = left;
    columns
        .iter()
        .enumerate()
        .map(|(i, col)| {
            let width = col.width.max(1.0);
            let origin = if i < frozen { x } else { x - scroll_x };
            let clip_left = if i < frozen {
                left
            } else {
                left + frozen_width
            };
            let viewport = Rect::new(
                clip_left,
                bounds.y,
                ((bounds.x + bounds.width) - padding - clip_left).max(0.0),
                bounds.height,
            );
            x += width;
            (
                origin,
                Rect::new(origin, bounds.y, width, bounds.height).intersect(&viewport),
            )
        })
        .collect()
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
    /// Drag across body cells to select a rectangle instead of panning.
    pub drag_select: bool,
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
            drag_select: false,
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
            min_width: 88.0,
            item_height: 28.0,
            popup_width: 240.0,
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
    /// Optional third button (e.g. Cancel in Save / Discard / Cancel).
    Tertiary,
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

    /// Historical dark-aligned severity color (dark preset).
    pub fn color(self) -> Color {
        self.color_for_theme(&Theme::dark())
    }

    /// Severity color from theme tokens.
    pub fn color_for_theme(self, theme: &Theme) -> Color {
        let c = &theme.colors;
        match self {
            AlarmSeverity::Critical => c.danger,
            AlarmSeverity::Major => c.danger_soft,
            AlarmSeverity::Minor => c.warning,
            AlarmSeverity::Warning => c.info,
            AlarmSeverity::Info => c.text_secondary,
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
        enabled: bool,
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
        secret: bool,
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
        /// Column index of the last body click (for cell editors).
        selected_col: Option<usize>,
        /// Inclusive row range selected with Shift+Up/Down.
        selected_range: Option<(usize, usize)>,
        /// Inclusive column range selected with Shift+Left/Right.
        selected_col_range: Option<(usize, usize)>,
        scroll_y: f32,
        /// Horizontal scroll for columns beyond [`frozen_cols`].
        scroll_x: f32,
        /// Pin the first N columns so they stay visible while scrolling horizontally.
        frozen_cols: usize,
        /// Optional per-cell foreground tint (conditional formatting).
        cell_bg: Vec<Vec<Option<Color>>>,
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
        tertiary: Option<String>,
        action_focus: usize,
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
        read_only: bool,
        placeholder: String,
        caret: usize,
        preedit: String,
        preedit_cursor: Option<(usize, usize)>,
        scroll_y: f32,
        edit: TextAreaEdit,
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
    /// Exclusive button group (Office P2a); one focusable unit like Segmented.
    ButtonGroup {
        labels: Vec<String>,
        selected: Option<usize>,
        style: ButtonGroupStyle,
    },
    /// Text-styled hyperlink (Office P2a).
    Link {
        label: String,
        style: LinkStyle,
    },
    /// Step progress list (Office P2b).
    Steps {
        items: Vec<StepItem>,
        current: usize,
        style: StepsStyle,
    },
    /// KPI statistic block (Office P2b); display-only.
    Statistic {
        title: String,
        value: String,
        prefix: Option<String>,
        suffix: Option<String>,
        hint: Option<String>,
        style: StatisticStyle,
    },
    /// Status result page block (Office P2b).
    Result {
        status: ResultStatus,
        title: String,
        subtitle: Option<String>,
        primary_action: Option<String>,
        secondary_action: Option<String>,
        style: ResultStyle,
    },
    /// Click-toggle popover bubble (Office P2c); opens via `open_popup`.
    Popover {
        trigger: String,
        title: Option<String>,
        body: String,
        open: bool,
        style: PopoverStyle,
    },
    /// Confirm bubble with OK/Cancel (Office P2c); opens via `open_popup`.
    Popconfirm {
        trigger: String,
        message: String,
        ok_label: String,
        cancel_label: String,
        open: bool,
        style: PopconfirmStyle,
    },
    /// File path field + Browse (Office P2d); OS dialog is app-stubbed.
    Upload {
        path: Option<String>,
        placeholder: String,
        browse_label: String,
        clear_label: String,
        style: UploadStyle,
    },
    /// Preset swatches + hex field (Office P2d).
    ColorPicker {
        value: Color,
        hex: String,
        selected_swatch: Option<usize>,
        presets: Vec<Color>,
        style: ColorPickerStyle,
    },
    /// Text field + filtered suggestion dropdown (Office P2d).
    Autocomplete {
        options: Vec<String>,
        value: String,
        caret: usize,
        open: bool,
        highlight: Option<usize>,
        placeholder: String,
        style: AutocompleteStyle,
    },
    /// Fixed-column grid container (Office P2e).
    Grid {
        columns: usize,
        style: GridStyle,
        children: Vec<WidgetId>,
    },
    /// Vertical event timeline (Office P2e).
    Timeline {
        items: Vec<TimelineItem>,
        selected: Option<usize>,
        style: TimelineStyle,
    },
    /// Month calendar (Office P2e); week starts Sunday.
    Calendar {
        year: i32,
        month: u8,
        selected_day: u8,
        style: CalendarStyle,
    },
    /// Raster preview or placeholder (Office P2e).
    Image {
        px_w: u32,
        px_h: u32,
        pixels: Option<ImagePixels>,
        view: ImageView,
        caption: String,
        style: ImageStyle,
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
    TableSelection { table: WidgetId, bounds: Rect },
    TextArea { area: WidgetId, bounds: Rect },
    TableColumn {
        table: WidgetId,
        column: usize,
        start_x: f32,
        start_width: f32,
    },
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
    Scrollbar {
        area: WidgetId,
        axis: Axis,
        start_pos: f32,
        start_scroll: f32,
        scroll_per_pixel: f32,
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

// Fields use the table's column geometry while retaining standard text input behavior.
#[derive(Debug, Clone)]
struct TableFilterRow {
    table: WidgetId,
    fields: Vec<WidgetId>,
    last_focus: Option<WidgetId>,
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
    table_filters: HashMap<WidgetId, TableFilterRow>,
    scrollbars: HashMap<WidgetId, [NodeId; 4]>,
    revealed_scroll_focus: Option<WidgetId>,
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
    /// Last ButtonGroup selection change; drained by [`Ui::take_button_group_change`].
    pending_button_group_change: Option<(WidgetId, Option<usize>)>,
    /// Last keyboard-activated Link; drained by [`Ui::take_link_activate`].
    pending_link_activate: Option<WidgetId>,
    /// Last Steps current change; drained by [`Ui::take_steps_change`].
    pending_steps_change: Option<(WidgetId, usize)>,
    /// Last Result action; drained by [`Ui::take_result_action`].
    pending_result_action: Option<(WidgetId, ResultAction)>,
    /// Last Popover open-state change; drained by [`Ui::take_popover_open_change`].
    pending_popover_open_change: Option<(WidgetId, bool)>,
    /// Last Popconfirm action; drained by [`Ui::take_popconfirm_action`].
    pending_popconfirm_action: Option<(WidgetId, PopconfirmAction)>,
    /// Last Upload Browse click; drained by [`Ui::take_upload_browse`].
    pending_upload_browse: Option<WidgetId>,
    /// Last Upload Clear click; drained by [`Ui::take_upload_clear`].
    pending_upload_clear: Option<WidgetId>,
    /// Last ColorPicker value change; drained by [`Ui::take_color_change`].
    pending_color_change: Option<(WidgetId, Color)>,
    /// Last Autocomplete commit; drained by [`Ui::take_autocomplete_change`].
    pending_autocomplete_change: Option<(WidgetId, String)>,
    /// Last Timeline row select; drained by [`Ui::take_timeline_select`].
    pending_timeline_select: Option<(WidgetId, usize)>,
    /// Last Calendar date change; drained by [`Ui::take_calendar_change`].
    pending_calendar_change: Option<(WidgetId, CalendarDate)>,
    /// Ui-level theme/locale bag (Office P2e config_provider — not a Kind).
    config: UiConfig,
    /// Appearance tokens (light/dark + scales). Profile stays separate.
    theme: Theme,
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
    /// Last Table header column click; drained by [`Ui::take_table_header_click`].
    pending_table_header_click: Option<(WidgetId, usize)>,
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
    /// Undo the last text edit.
    Undo,
    /// Redo the last undone text edit.
    Redo,
    /// Extend a TextArea selection with an arrow, Home or End key.
    SelectLeft,
    /// Extend selection right.
    SelectRight,
    /// Extend selection up one visual line.
    SelectUp,
    /// Extend selection down one visual line.
    SelectDown,
    /// Extend selection to the start of the visual line.
    SelectHome,
    /// Extend selection to the end of the visual line.
    SelectEnd,
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
            table_filters: HashMap::new(),
            scrollbars: HashMap::new(),
            revealed_scroll_focus: None,
            font: Arc::new(font),
            drag: None,
            focus: None,
            open_popup: None,
            pending_menu_action: None,
            pending_button_activate: None,
            pending_button_group_change: None,
            pending_link_activate: None,
            pending_steps_change: None,
            pending_result_action: None,
            pending_popover_open_change: None,
            pending_popconfirm_action: None,
            pending_upload_browse: None,
            pending_upload_clear: None,
            pending_color_change: None,
            pending_autocomplete_change: None,
            pending_timeline_select: None,
            pending_calendar_change: None,
            config: UiConfig::default(),
            theme: Theme::dark(),
            overlay_host: None,
            tooltip_hover: None,
            pending_modal_action: None,
            pending_tag_close: None,
            pending_alert_dismiss: None,
            pending_drawer_action: None,
            pending_breadcrumb_click: None,
            pending_pagination_change: None,
            pending_table_header_click: None,
            notifications: Vec::new(),
            next_notification_id: 1,
            notification_style: NotificationStyle::from_theme(&Theme::dark()),
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

    /// Office P2e `config_provider`: replace the Ui-level defaults bag.
    pub fn set_config(&mut self, config: UiConfig) {
        self.config = config;
        self.notification_style =
            NotificationStyle::from_theme_with_density(&self.theme, self.config.density);
    }

    /// Borrow the current [`UiConfig`] (locale / density / accent).
    pub fn config(&self) -> &UiConfig {
        &self.config
    }

    /// Replace the active [`Theme`] (colors / type / spacing / radii).
    ///
    /// Does not rebuild existing widgets; new `*_style()` helpers and unstyled
    /// constructors pick up the theme. Pass an explicit Style to override.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.notification_style =
            NotificationStyle::from_theme_with_density(&theme, self.config.density);
    }

    /// Borrow the active [`Theme`].
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Effective accent: [`UiConfig::accent`] if set, else theme accent.
    pub fn resolve_accent(&self) -> Color {
        self.config.accent.unwrap_or(self.theme.colors.accent)
    }

    /// [`ColumnStyle`] from current theme + density.
    pub fn column_style(&self) -> ColumnStyle {
        ColumnStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`RowStyle`] from current theme + density.
    pub fn row_style(&self) -> RowStyle {
        RowStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`ToolbarStyle`] from current theme + density.
    pub fn toolbar_style(&self) -> ToolbarStyle {
        ToolbarStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`PanelStyle`] from current theme + density.
    pub fn panel_style(&self) -> PanelStyle {
        PanelStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`StatusBarStyle`] from current theme + density.
    pub fn status_bar_style(&self) -> StatusBarStyle {
        StatusBarStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`SplitterStyle`] from current theme + density.
    pub fn splitter_style(&self) -> SplitterStyle {
        SplitterStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`ScrollAreaStyle`] from current theme + density.
    pub fn scroll_area_style(&self) -> ScrollAreaStyle {
        ScrollAreaStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`TextInputStyle`] from current theme + density.
    pub fn text_input_style(&self) -> TextInputStyle {
        TextInputStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`CheckboxStyle`] from current theme + density.
    pub fn checkbox_style(&self) -> CheckboxStyle {
        CheckboxStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`RadioStyle`] from current theme + density.
    pub fn radio_style(&self) -> RadioStyle {
        RadioStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`SwitchStyle`] from current theme + density.
    pub fn switch_style(&self) -> SwitchStyle {
        SwitchStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`ListStyle`] from current theme + density.
    pub fn list_style(&self) -> ListStyle {
        ListStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`TreeStyle`] from current theme + density.
    pub fn tree_style(&self) -> TreeStyle {
        TreeStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`TableStyle`] from current theme + density.
    pub fn table_style(&self) -> TableStyle {
        TableStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`MenuStyle`] from current theme + density.
    pub fn menu_style(&self) -> MenuStyle {
        MenuStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`ContextMenuStyle`] from current theme + density.
    pub fn context_menu_style(&self) -> ContextMenuStyle {
        ContextMenuStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`TabsStyle`] from current theme + density.
    pub fn tabs_style(&self) -> TabsStyle {
        TabsStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`SelectStyle`] from current theme + density.
    pub fn select_style(&self) -> SelectStyle {
        SelectStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`SliderStyle`] from current theme + density.
    pub fn slider_style(&self) -> SliderStyle {
        SliderStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`TagStyle`] from current theme + density.
    pub fn tag_style(&self) -> TagStyle {
        TagStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`BadgeStyle`] from current theme + density.
    pub fn badge_style(&self) -> BadgeStyle {
        BadgeStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`TooltipStyle`] from current theme + density.
    pub fn tooltip_style(&self) -> TooltipStyle {
        TooltipStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`ModalStyle`] from current theme + density.
    pub fn modal_style(&self) -> ModalStyle {
        ModalStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`ProgressStyle`] from current theme + density.
    pub fn progress_style(&self) -> ProgressStyle {
        ProgressStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`AlertStyle`] from current theme + density.
    pub fn alert_style(&self) -> AlertStyle {
        AlertStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`FormStyle`] from current theme + density.
    pub fn form_style(&self) -> FormStyle {
        FormStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`TreeSelectStyle`] from current theme + density.
    pub fn tree_select_style(&self) -> TreeSelectStyle {
        TreeSelectStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`AlarmListStyle`] from current theme + density.
    pub fn alarm_list_style(&self) -> AlarmListStyle {
        AlarmListStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`GaugeStyle`] from current theme + density.
    pub fn gauge_style(&self) -> GaugeStyle {
        GaugeStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`TrendStyle`] from current theme + density.
    pub fn trend_style(&self) -> TrendStyle {
        TrendStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`ChartStyle`] from current theme + density.
    pub fn chart_style(&self) -> ChartStyle {
        ChartStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`BackdropStyle`] from current theme + density.
    pub fn backdrop_style(&self) -> BackdropStyle {
        BackdropStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`DrawerStyle`] from current theme + density.
    pub fn drawer_style(&self) -> DrawerStyle {
        DrawerStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`TextAreaStyle`] from current theme + density.
    pub fn text_area_style(&self) -> TextAreaStyle {
        TextAreaStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`InputNumberStyle`] from current theme + density.
    pub fn input_number_style(&self) -> InputNumberStyle {
        InputNumberStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`NotificationStyle`] from current theme + density.
    pub fn notification_toast_style(&self) -> NotificationStyle {
        NotificationStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`SpinStyle`] from current theme + density.
    pub fn spin_style(&self) -> SpinStyle {
        SpinStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`EmptyStyle`] from current theme + density.
    pub fn empty_style(&self) -> EmptyStyle {
        EmptyStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`CollapseStyle`] from current theme + density.
    pub fn collapse_style(&self) -> CollapseStyle {
        CollapseStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`DescriptionsStyle`] from current theme + density.
    pub fn descriptions_style(&self) -> DescriptionsStyle {
        DescriptionsStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`CardStyle`] from current theme + density.
    pub fn card_style(&self) -> CardStyle {
        CardStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`SegmentedStyle`] from current theme + density.
    pub fn segmented_style(&self) -> SegmentedStyle {
        SegmentedStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`BreadcrumbStyle`] from current theme + density.
    pub fn breadcrumb_style(&self) -> BreadcrumbStyle {
        BreadcrumbStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`PaginationStyle`] from current theme + density.
    pub fn pagination_style(&self) -> PaginationStyle {
        PaginationStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`DateTimeStyle`] from current theme + density.
    pub fn datetime_style(&self) -> DateTimeStyle {
        DateTimeStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`TransferStyle`] from current theme + density.
    pub fn transfer_style(&self) -> TransferStyle {
        TransferStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`PropertyGridStyle`] from current theme + density.
    pub fn property_grid_style(&self) -> PropertyGridStyle {
        PropertyGridStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`TreeTableStyle`] from current theme + density.
    pub fn tree_table_style(&self) -> TreeTableStyle {
        TreeTableStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`SpaceStyle`] from current theme + density.
    pub fn space_style(&self) -> SpaceStyle {
        SpaceStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`ButtonGroupStyle`] from current theme + density.
    pub fn button_group_style(&self) -> ButtonGroupStyle {
        ButtonGroupStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`LinkStyle`] from current theme + density.
    pub fn link_style(&self) -> LinkStyle {
        LinkStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`StepsStyle`] from current theme + density.
    pub fn steps_style(&self) -> StepsStyle {
        StepsStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`StatisticStyle`] from current theme + density.
    pub fn statistic_style(&self) -> StatisticStyle {
        StatisticStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`ResultStyle`] from current theme + density.
    pub fn result_style(&self) -> ResultStyle {
        ResultStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`PopoverStyle`] from current theme + density.
    pub fn popover_style(&self) -> PopoverStyle {
        PopoverStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`PopconfirmStyle`] from current theme + density.
    pub fn popconfirm_style(&self) -> PopconfirmStyle {
        PopconfirmStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`UploadStyle`] from current theme + density.
    pub fn upload_style(&self) -> UploadStyle {
        UploadStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`ColorPickerStyle`] from current theme + density.
    pub fn color_picker_style(&self) -> ColorPickerStyle {
        ColorPickerStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`AutocompleteStyle`] from current theme + density.
    pub fn autocomplete_style(&self) -> AutocompleteStyle {
        AutocompleteStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`GridStyle`] from current theme + density.
    pub fn grid_style(&self) -> GridStyle {
        GridStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`TimelineStyle`] from current theme + density.
    pub fn timeline_style(&self) -> TimelineStyle {
        TimelineStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`CalendarStyle`] from current theme + density.
    pub fn calendar_style(&self) -> CalendarStyle {
        CalendarStyle::from_theme_with_density(&self.theme, self.config.density)
    }

    /// [`ImageStyle`] from current theme + density.
    pub fn image_style(&self) -> ImageStyle {
        ImageStyle::from_theme_with_density(&self.theme, self.config.density)
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


    /// Top-level spacing container (Office P2a sugar over row/column).
    pub fn space<F>(&mut self, style: SpaceStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        match style.direction {
            SpaceDirection::Horizontal => {
                let row = RowStyle {
                    x: style.x,
                    y: style.y,
                    width: style.width,
                    height: style.height,
                    padding: style.padding,
                    gap: style.gap,
                    cross_align: style.cross_align,
                    background: style.background,
                };
                self.row(row, build)
            }
            SpaceDirection::Vertical => {
                let col = ColumnStyle {
                    x: style.x,
                    y: style.y,
                    width: style.width,
                    height: style.height,
                    padding: style.padding,
                    gap: style.gap,
                    cross_align: style.cross_align,
                    background: style.background,
                };
                self.column(col, build)
            }
        }
    }

    /// Top-level fixed-column grid (Office P2e). Children L→R, wrap by `columns`.
    pub fn grid<F>(&mut self, columns: usize, style: GridStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let columns = columns.max(1);
        let id = self.alloc_container(Kind::Grid {
            columns,
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

    /// Update a button label without replacing its focus or the surrounding editor.
    pub fn set_button_label(&mut self, id: WidgetId, value: impl Into<String>) -> bool {
        if let Some(Widget { kind: Kind::Button { label, .. }, .. }) = self.widgets.get_mut(&id) {
            *label = value.into();
            return true;
        }
        false
    }

    /// Whether a button accepts pointer, keyboard and focus navigation.
    pub fn button_enabled(&self, id: WidgetId) -> Option<bool> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Button { enabled, .. }) => Some(*enabled),
            _ => None,
        }
    }

    /// Enable or disable a button; call [`Ui::sync`] to update its appearance.
    pub fn set_button_enabled(&mut self, id: WidgetId, value: bool) -> bool {
        let Some(Widget { kind: Kind::Button { enabled, .. }, .. }) = self.widgets.get_mut(&id) else {
            return false;
        };
        let changed = *enabled != value;
        *enabled = value;
        if !value {
            if self.focus == Some(id) {
                self.focus = None;
            }
            if self.pending_button_activate == Some(id) {
                self.pending_button_activate = None;
            }
        }
        changed
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
            self.emit_overlay_dismiss(pid);
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

        if self.pointer_scrollbar(scene, id, x, y) {
            return true;
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

        if let Some(view) = self.image_view(id) {
            self.drag = Some(DragState::Scroll {
                area: id, start_x: x, start_y: y,
                start_scroll_x: view.offset_x, start_scroll_y: view.offset_y,
            });
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
            Some(Kind::Autocomplete { .. }) => {
                self.focus = Some(id);
                self.pointer_autocomplete_focus(scene, id)
            }
            Some(Kind::Upload { .. }) => {
                self.focus = Some(id);
                self.pointer_upload(scene, id, x, y)
            }
            Some(Kind::ColorPicker { .. }) => {
                self.focus = Some(id);
                self.pointer_color_picker(scene, id, x, y)
            }
            Some(Kind::Timeline { style, .. }) if style.clickable => {
                self.focus = Some(id);
                self.pointer_timeline(scene, id, x, y)
            }
            Some(Kind::Calendar { .. }) => {
                self.focus = Some(id);
                self.pointer_calendar(scene, id, x, y)
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
                let bounds = self.widgets.get(&id).and_then(|w| w.node).and_then(|n| scene.get(n).map(|n| n.bounds)).unwrap_or_else(Rect::empty);
                self.drag = Some(DragState::TextArea { area: id, bounds });
                self.pointer_text_area(id, bounds, x, y, false)
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
            Some(Kind::Button { enabled: false, .. }) => true,
            Some(Kind::Button { .. }) => {
                // Dual path: focus for Tab/Space/Enter; apps still activate via
                // `hit_widget` on pointer (this returns false so click semantics stay).
                self.focus = Some(id);
                false
            }
            Some(Kind::ButtonGroup { .. }) => {
                self.focus = Some(id);
                self.pointer_button_group(scene, id, x, y)
            }
            Some(Kind::Link { .. }) => {
                // Same dual path as Button.
                self.focus = Some(id);
                false
            }
            Some(Kind::Steps { .. }) => {
                self.focus = Some(id);
                self.pointer_steps(scene, id, x, y)
            }
            Some(Kind::Result { .. }) => {
                self.focus = Some(id);
                self.pointer_result(scene, id, x, y)
            }
            Some(Kind::Popover { .. }) => {
                self.focus = Some(id);
                self.pointer_popover_toggle(scene, id)
            }
            Some(Kind::Popconfirm { .. }) => {
                self.focus = Some(id);
                self.pointer_popconfirm_toggle(scene, id)
            }
            Some(Kind::Statistic { .. }) => {
                // Display-only; not focusable.
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
            DragState::TableSelection { table, bounds } => self.drag_table_selection(table, bounds, x, y),
            DragState::TextArea { area, bounds } => self.pointer_text_area(area, bounds, x, y, true),
            DragState::TableColumn {
                table,
                column,
                start_x,
                start_width,
            } => self.set_table_column_width(table, column, start_width + x - start_x),
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
            DragState::Scrollbar { area, axis, start_pos, start_scroll, scroll_per_pixel } => {
                let pos = if axis == Axis::Horizontal { x } else { y };
                let offset = start_scroll + (pos - start_pos) * scroll_per_pixel;
                let (sx, sy) = self.any_scroll_offset(area).unwrap_or_default();
                if axis == Axis::Horizontal { self.set_any_scroll(area, offset, sy) }
                else { self.set_any_scroll(area, sx, offset) }
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
        if !dx.is_finite() || !dy.is_finite() {
            return false;
        }
        let Some(id) = self.hit_widget(scene, x, y) else {
            return false;
        };
        let mut node = self.widgets.get(&id).and_then(|w| w.node);
        // Start at the hit control, then bubble at its limits to enclosing viewports.
        while let Some(n) = node.and_then(|n| scene.get(n)) {
            if let Some(id) = self.widget_from_node(n.id) {
                let id = self.table_filters.iter()
                    .find(|(row, group)| **row == id || group.fields.contains(&id))
                    .map(|(_, group)| group.table).unwrap_or(id);
                if let Some((sx, sy)) = self.any_scroll_offset(id) {
                    let changed = self.set_any_scroll(id, sx + dx, sy + dy);
                    if changed {
                        return true;
                    }
                }
            }
            node = n.parent;
        }
        false
    }

    /// Restore a ScrollArea position, clamped to its laid-out content. Rejects non-finite offsets.
    pub fn set_scroll_offset(&mut self, id: WidgetId, sx: f32, sy: f32) -> bool {
        if !sx.is_finite() || !sy.is_finite() {
            return false;
        }
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
        self.focus = id.filter(|id| self.button_enabled(*id) != Some(false));
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
            Kind::TextInput { secret: true, placeholder, .. } => Some(placeholder.clone()),
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
            Kind::ButtonGroup { labels, selected, .. } => {
                selected.and_then(|i| labels.get(i).cloned())
            }
            Kind::Link { label, .. } => {
                if label.is_empty() {
                    None
                } else {
                    Some(label.clone())
                }
            }
            Kind::Steps { items, current, .. } => items.get(*current).map(|s| s.title.clone()),
            Kind::Statistic { title, value, .. } => {
                if title.is_empty() && value.is_empty() {
                    None
                } else if title.is_empty() {
                    Some(value.clone())
                } else if value.is_empty() {
                    Some(title.clone())
                } else {
                    Some(format!("{title}: {value}"))
                }
            }
            Kind::Result { title, .. } => {
                if title.is_empty() {
                    None
                } else {
                    Some(title.clone())
                }
            }
            Kind::Popover { trigger, title, .. } => title
                .as_ref()
                .filter(|s| !s.is_empty())
                .cloned()
                .or_else(|| {
                    if trigger.is_empty() {
                        None
                    } else {
                        Some(trigger.clone())
                    }
                }),
            Kind::Popconfirm { trigger, message, .. } => {
                if !message.is_empty() {
                    Some(message.clone())
                } else if trigger.is_empty() {
                    None
                } else {
                    Some(trigger.clone())
                }
            }
            Kind::Upload { path, placeholder, .. } => {
                if let Some(p) = path {
                    if !p.is_empty() {
                        return Some(p.clone());
                    }
                }
                if placeholder.is_empty() {
                    None
                } else {
                    Some(placeholder.clone())
                }
            }
            Kind::ColorPicker { hex, .. } => {
                if hex.is_empty() {
                    None
                } else {
                    Some(hex.clone())
                }
            }
            Kind::Autocomplete {
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
            Kind::Timeline { items, selected, .. } => {
                if let Some(i) = selected {
                    items.get(*i).map(|it| format!("{}: {}", it.time, it.content))
                } else if let Some(it) = items.first() {
                    Some(format!("{}: {}", it.time, it.content))
                } else {
                    Some("Timeline".into())
                }
            }
            Kind::Calendar {
                year,
                month,
                selected_day,
                ..
            } => Some(format!("{:04}-{:02}-{:02}", year, month, selected_day)),
            Kind::Image { caption, px_w, px_h, .. } => {
                if !caption.is_empty() {
                    Some(caption.clone())
                } else {
                    Some(format!("Image {px_w}×{px_h}"))
                }
            }
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
    ///
    /// Office P2b: [`Kind::Steps`] is always focusable; [`Kind::Statistic`] is not;
    /// [`Kind::Result`] is focusable only when it has at least one action label
    /// (actions are hit targets on the Result host — no separate child widgets).
    fn kind_is_focusable(kind: &Kind) -> bool {
        match kind {
            Kind::Result {
                primary_action,
                secondary_action,
                ..
            } => primary_action.is_some() || secondary_action.is_some(),
            Kind::Button { enabled, .. } => *enabled,
            Kind::TextInput { .. }
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
            | Kind::ButtonGroup { .. }
            | Kind::Link { .. }
            | Kind::Steps { .. }
            | Kind::Popover { .. }
            | Kind::Popconfirm { .. }
            | Kind::Upload { .. }
            | Kind::ColorPicker { .. }
            | Kind::Autocomplete { .. }
            | Kind::Calendar { .. } => true,
            Kind::Timeline { style, .. } => style.clickable,
            _ => false,
        }
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
            | Kind::Spin { children, .. }
            | Kind::Grid { children, .. } => children.as_slice(),
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

    /// Mask a password input and exclude its value from copy and accessible labels.
    pub fn set_text_input_secret(&mut self, id: WidgetId, secret: bool) {
        if let Some(Widget {
            kind: Kind::TextInput { secret: value, .. },
            ..
        }) = self.widgets.get_mut(&id)
        {
            *value = secret;
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
    /// TextArea returns only its selection. TextInput returns its full non-secret
    /// value. `None` when no copyable text is selected.
    pub fn focused_text_for_copy(&self) -> Option<String> {
        let id = self.focus?;
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::TextInput {
                value,
                secret: false,
                ..
            }) if !value.is_empty() => Some(value.clone()),
            Some(Kind::TextArea { .. }) => self.text_area_selected_text(id),
            _ => None,
        }
    }

    /// Insert `text` into the focused TextInput / TextArea at the caret.
    ///
    /// Replaces the TextArea selection or the whole TextInput when `select_all` is set.
    /// Clears IME preedit.
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
            Some(Kind::TextArea { .. }) => self.replace_text_area_selection(id, text),
            _ => false,
        }
    }

    /// True when a text editor is focused (app should enable OS IME).
    pub fn text_input_wants_ime(&self) -> bool {
        self.focus
            .is_some_and(|id| matches!(self.widgets.get(&id).map(|w| &w.kind), Some(Kind::TextInput { .. } | Kind::TextArea { read_only: false, .. })))
    }

    /// Caret rectangle for the focused TextInput / TextArea (physical px), for IME candidate placement.
    pub fn ime_cursor_area(&self, scene: &Scene) -> Option<(f32, f32, f32, f32)> {
        let id = self.focus?;
        let w = self.widgets.get(&id)?;
        if let Kind::TextArea { value, caret, preedit, preedit_cursor, scroll_y, style, edit, .. } = &w.kind {
            let bounds = w.node.and_then(|n| scene.get(n).map(|n| n.bounds))?;
            let (display, caret) = text_area::display_text(value, *caret, edit, preedit, *preedit_cursor);
            let layout = self.font.layout(&display, style.size, Some((bounds.width - style.padding_x * 2.0).max(1.0)));
            let (x, y) = layout.caret_top_left(&display, caret, self.font.metrics(style.size).ascent);
            return Some(((bounds.x + style.padding_x + x).floor(), (bounds.y + style.padding_y - scroll_y + y).floor(), 2.0, self.font.metrics(style.size).ascent.ceil()));
        }
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

    /// Update IME preedit on the focused TextInput / TextArea. Empty `text` clears composition.
    pub fn ime_preedit(&mut self, text: impl Into<String>, cursor: Option<(usize, usize)>) -> bool {
        let Some(id) = self.focus else {
            return false;
        };
        let Some(w) = self.widgets.get_mut(&id) else {
            return false;
        };
        if matches!(w.kind, Kind::TextArea { read_only: true, .. }) { return false; }
        let (Kind::TextInput { preedit, preedit_cursor, .. }
            | Kind::TextArea { preedit, preedit_cursor, .. }) = &mut w.kind
        else {
            return false;
        };
        *preedit = text.into();
        *preedit_cursor = if preedit.is_empty() { None } else { cursor };
        if let Kind::TextArea { edit, .. } = &mut w.kind { edit.reveal = true; }
        true
    }

    /// Commit IME text into the focused TextInput / TextArea at the caret (clears preedit).
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
        if let Kind::TextArea { preedit, preedit_cursor, .. } = &mut w.kind {
            preedit.clear();
            *preedit_cursor = None;
            return self.paste_into_focused_text(text);
        }
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

    /// Clear preedit on the focused TextInput / TextArea (IME disabled / cancelled).
    pub fn ime_clear_preedit(&mut self) -> bool {
        self.ime_preedit("", None)
    }

    fn scrub_unfocused_ime(&mut self) {
        let focus = self.focus;
        for (id, w) in self.widgets.iter_mut() {
            if Some(*id) == focus {
                continue;
            }
            if let Kind::TextInput { preedit, preedit_cursor, .. }
                | Kind::TextArea { preedit, preedit_cursor, .. } = &mut w.kind
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

    /// Current image magnification and pan, for restoration after rebuilding a UI.
    pub fn image_view(&self, id: WidgetId) -> Option<ImageView> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Image { view, .. }) => Some(*view),
            _ => None,
        }
    }

    /// Set image magnification (1/64 through 64) and bounded pan. Invalid input is ignored.
    pub fn set_image_view(&mut self, id: WidgetId, mut view: ImageView) -> bool {
        if view
            .scale
            .is_some_and(|s| !s.is_finite() || !(1.0 / 64.0..=64.0).contains(&s))
            || !view.offset_x.is_finite()
            || !view.offset_y.is_finite()
        {
            return false;
        }
        let Some(Widget {
            kind:
                Kind::Image {
                    view: current,
                    px_w,
                    px_h,
                    caption,
                    style,
                    ..
                },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        let scale = view.scale.unwrap_or(0.0);
        let cap_h = if caption.is_empty() {
            0.0
        } else {
            18.0f32.min(style.height)
        };
        let max_x = ((*px_w as f32 * scale - style.width) * 0.5).max(0.0);
        let max_y = ((*px_h as f32 * scale - (style.height - cap_h)) * 0.5).max(0.0);
        view.offset_x = view.offset_x.clamp(-max_x, max_x);
        view.offset_y = view.offset_y.clamp(-max_y, max_y);
        if *current == view {
            return false;
        }
        *current = view;
        true
    }

    fn is_data_scrollable(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::List { .. } | Kind::Tree { .. } | Kind::Table { .. } | Kind::AlarmList { .. } | Kind::ScrollArea { .. } | Kind::PropertyGrid { .. } | Kind::TreeTable { .. } | Kind::TextArea { .. })
        )
    }

    fn any_scroll_offset(&self, id: WidgetId) -> Option<(f32, f32)> {
        if let Some(view) = self.image_view(id) { return Some((view.offset_x, view.offset_y)); }
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::ScrollArea {
                scroll_x, scroll_y, ..
            }) => Some((*scroll_x, *scroll_y)),
            Some(Kind::TextArea { scroll_y, .. }) => Some((0.0, *scroll_y)),
            Some(Kind::List { scroll_y, .. }) => Some((0.0, *scroll_y)),
            Some(Kind::Tree { scroll_y, .. }) => Some((0.0, *scroll_y)),
            Some(Kind::Table { scroll_x, scroll_y, .. }) => Some((*scroll_x, *scroll_y)),
            Some(Kind::AlarmList { scroll_y, .. }) => Some((0.0, *scroll_y)),
            Some(Kind::TreeSelect { scroll_y, open: true, .. }) => Some((0.0, *scroll_y)),
            Some(Kind::PropertyGrid { scroll_y, .. }) => Some((0.0, *scroll_y)),
            Some(Kind::TreeTable { scroll_y, .. }) => Some((0.0, *scroll_y)),
            _ => None,
        }
    }

    fn set_any_scroll(&mut self, id: WidgetId, sx: f32, sy: f32) -> bool {
        if !sx.is_finite() || !sy.is_finite() { return false; }
        if matches!(self.widgets.get(&id).map(|w| &w.kind), Some(Kind::TextArea { .. })) {
            return self.set_text_area_scroll(id, sy);
        }
        if let Some(mut view) = self.image_view(id) {
            view.offset_x = sx;
            view.offset_y = sy;
            return self.set_image_view(id, view);
        }
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
                Kind::Table {
                    scroll_x,
                    scroll_y,
                    columns,
                    frozen_cols,
                    style,
                    ..
                } => {
                    let total_w: f32 = columns.iter().map(|c| c.width).sum();
                    let frozen_w: f32 = columns.iter().take(*frozen_cols).map(|c| c.width).sum();
                    let view_w = (style.width - style.padding * 2.0 - frozen_w).max(1.0);
                    let max_x = (total_w - frozen_w - view_w).max(0.0);
                    let nx = sx.clamp(0.0, max_x);
                    let changed = (ny - *scroll_y).abs() >= 0.1 || (nx - *scroll_x).abs() >= 0.1;
                    if !changed {
                        return false;
                    }
                    *scroll_y = ny;
                    *scroll_x = nx;
                    return true;
                }
                Kind::List { scroll_y, .. }
                | Kind::Tree { scroll_y, .. }
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

    /// Column index of the last body click on a Table, if any.
    pub fn table_selected_col(&self, id: WidgetId) -> Option<usize> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Table { selected_col, .. }) => *selected_col,
            _ => None,
        }
    }

    /// Inclusive selected row bounds, ordered from top to bottom.
    pub fn table_selected_rows(&self, id: WidgetId) -> Option<(usize, usize)> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Table { selected_range, selected_row, .. }) => selected_range
                .or_else(|| selected_row.map(|row| (row, row)))
                .map(|(a, b)| (a.min(b), a.max(b))),
            _ => None,
        }
    }

    /// Inclusive selected column bounds, ordered from left to right.
    pub fn table_selected_cols(&self, id: WidgetId) -> Option<(usize, usize)> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Table { selected_col_range, selected_col, .. }) => selected_col_range
                .or_else(|| selected_col.map(|col| (col, col)))
                .map(|(a, b)| (a.min(b), a.max(b))),
            _ => None,
        }
    }

    /// Fixed selection anchor, for preserving Shift selection across rebuilds.
    /// A missing column denotes a whole-row selection.
    pub fn table_selection_anchor(&self, id: WidgetId) -> Option<(usize, Option<usize>)> {
        let Kind::Table { selected_range, selected_col_range, selected_row, selected_col, .. } =
            &self.widgets.get(&id)?.kind else { return None; };
        Some((selected_range.map(|r| r.0).or(*selected_row)?,
            selected_col_range.map(|r| r.0).or(*selected_col)))
    }

    /// Copy the selected cells as quoted tab-separated text.
    pub fn table_selected_text(&self, id: WidgetId) -> Option<String> {
        let Kind::Table { rows, columns, .. } = &self.widgets.get(&id)?.kind else { return None; };
        let (start, end) = self.table_selected_rows(id)?;
        let (col_start, col_end) = self.table_selected_cols(id)
            .unwrap_or((0, columns.len().saturating_sub(1)));
        if end >= rows.len() || columns.is_empty() || col_end >= columns.len() { return None; }
        Some((start..=end).map(|row| (col_start..=col_end).map(|col| {
            let value = rows[row].get(col).map(String::as_str).unwrap_or("");
            if value.contains(['\t', '\r', '\n', '"']) {
                format!("\"{}\"", value.replace('"', "\"\""))
            } else {
                value.to_string()
            }
        }).collect::<Vec<_>>().join("\t")).collect::<Vec<_>>().join("\n"))
    }

    /// Drain last Table header click as `(table_id, column_index)`.
    pub fn take_table_header_click(&mut self) -> Option<(WidgetId, usize)> {
        self.pending_table_header_click.take()
    }

    /// Current table column widths, including pointer resizing.
    pub fn table_column_widths(&self, id: WidgetId) -> Option<Vec<f32>> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Table { columns, .. }) => Some(columns.iter().map(|c| c.width).collect()),
            _ => None,
        }
    }

    /// Resize a column, with a 24 px minimum. Rejects non-finite widths.
    pub fn set_table_column_width(&mut self, id: WidgetId, column: usize, width: f32) -> bool {
        if !width.is_finite() {
            return false;
        }
        let Some(Widget {
            kind: Kind::Table { columns, .. },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        let Some(col) = columns.get_mut(column) else {
            return false;
        };
        let width = width.max(24.0);
        if col.width == width {
            return false;
        }
        col.width = width;
        let (sx, sy) = self.table_scroll_offset(id).unwrap();
        self.set_any_scroll(id, sx, sy);
        true
    }

    /// Table viewport offset, for preserving position across UI rebuilds.
    pub fn table_scroll_offset(&self, id: WidgetId) -> Option<(f32, f32)> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Table {
                scroll_x, scroll_y, ..
            }) => Some((*scroll_x, *scroll_y)),
            _ => None,
        }
    }

    /// Restore a table viewport offset, clamped to its current content.
    pub fn set_table_scroll_offset(&mut self, id: WidgetId, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() || !self.is_table(id) {
            return false;
        }
        self.set_any_scroll(id, x, y)
    }

    /// Set table row selection.
    pub fn set_table_selected_row(&mut self, id: WidgetId, row: Option<usize>) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Table {
                rows,
                selected_row,
                selected_range,
                selected_col,
                selected_col_range,
                ..
            } = &mut w.kind
            {
                *selected_row = row.filter(|i| *i < rows.len());
                *selected_range = selected_row.map(|i| (i, i));
                if selected_row.is_none() { *selected_col = None; }
                *selected_col_range = selected_col.map(|i| (i, i));
            }
        }
    }

    /// Set the active table column and reset its range to one cell.
    pub fn set_table_selected_col(&mut self, id: WidgetId, col: Option<usize>) -> bool {
        let Some(Widget { kind: Kind::Table { columns, selected_col, selected_col_range, .. }, .. }) = self.widgets.get_mut(&id) else { return false; };
        let Some(col) = col else { *selected_col = None; *selected_col_range = None; return true; };
        if col >= columns.len() { return false; }
        *selected_col = Some(col);
        *selected_col_range = Some((col, col));
        true
    }

    /// Set a cell selection from anchor to active cell, preserving its direction.
    pub fn set_table_selected_cells(&mut self, id: WidgetId, rows: Option<(usize, usize)>, cols: Option<(usize, usize)>) -> bool {
        let Some(Widget { kind: Kind::Table { columns, rows: body, selected_row, selected_col, selected_range, selected_col_range, .. }, .. }) = self.widgets.get_mut(&id) else { return false; };
        let Some((row_start, row_end)) = rows else { *selected_row = None; *selected_range = None; *selected_col = None; *selected_col_range = None; return true; };
        let Some((col_start, col_end)) = cols else { return false; };
        if row_start >= body.len() || row_end >= body.len() || col_start >= columns.len() || col_end >= columns.len() { return false; }
        *selected_row = Some(row_end);
        *selected_range = Some((row_start, row_end));
        *selected_col = Some(col_end);
        *selected_col_range = Some((col_start, col_end));
        true
    }

    /// Select rows from anchor to active row (the second endpoint).
    pub fn set_table_selected_rows(&mut self, id: WidgetId, range: Option<(usize, usize)>) -> bool {
        let Some(Widget { kind: Kind::Table { rows, selected_row, selected_range, .. }, .. }) = self.widgets.get_mut(&id) else { return false; };
        let Some((start, end)) = range else { self.set_table_selected_row(id, None); return true; };
        if start >= rows.len() || end >= rows.len() { return false; }
        *selected_range = Some((start, end));
        *selected_row = Some(end);
        true
    }

    /// Replace table body rows, clamping both selection endpoints.
    pub fn set_table_rows(&mut self, id: WidgetId, rows: Vec<Vec<String>>) {
        if let Some(Widget { kind: Kind::Table {
            rows: slot, selected_row, selected_range, selected_col, selected_col_range, scroll_y, ..
        }, .. }) = self.widgets.get_mut(&id) {
            *slot = rows;
            if let Some(last) = slot.len().checked_sub(1) {
                *selected_row = selected_row.map(|i| i.min(last));
                *selected_range = selected_range.map(|(a, b)| (a.min(last), b.min(last)));
            } else {
                *selected_row = None;
                *selected_range = None;
                *selected_col = None;
                *selected_col_range = None;
            }
            *scroll_y = 0.0;
        }
    }

    /// Pin the first `n` columns (horizontal freeze).
    pub fn set_table_frozen_cols(&mut self, id: WidgetId, n: usize) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Table {
                frozen_cols,
                columns,
                ..
            } = &mut w.kind
            {
                *frozen_cols = n.min(columns.len());
            }
        }
        if let Some((sx, sy)) = self.table_scroll_offset(id) {
            self.set_table_scroll_offset(id, sx, sy);
        }
    }

    /// Set optional per-cell background fill used for conditional formatting.
    pub fn set_table_cell_bg(&mut self, id: WidgetId, cell_bg: Vec<Vec<Option<Color>>>) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Table { cell_bg: dest, .. } = &mut w.kind {
                *dest = cell_bg;
            }
        }
    }

    /// Horizontal scroll offset for a table.
    pub fn set_table_scroll_x(&mut self, id: WidgetId, sx: f32) {
        if let Some((_, sy)) = self.table_scroll_offset(id) {
            self.set_table_scroll_offset(id, sx, sy);
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
                    Some(Kind::Popconfirm { .. }) => {
                        self.pending_popconfirm_action =
                            Some((id, PopconfirmAction::Dismiss));
                    }
                    Some(Kind::Popover { .. }) => {
                        self.pending_popover_open_change = Some((id, false));
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
                if let Some(id) = self.focus {
                    if self.is_modal(id) { return self.key_modal(id, key); }
                }
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
        let key = if key == UiKey::Space
            && matches!(
                self.widgets.get(&id).map(|w| &w.kind),
                Some(Kind::TextInput { .. } | Kind::TextArea { .. })
            ) {
            UiKey::Char(' ')
        } else {
            key
        };
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::TextInput { .. }) => self.key_text_input(id, key),
            Some(Kind::TextArea { .. }) => self.key_text_area(id, key),
            Some(Kind::InputNumber { .. }) => self.key_input_number(id, key),
            Some(Kind::Segmented { .. }) => self.key_segmented(id, key),
            Some(Kind::ButtonGroup { .. }) => self.key_button_group(id, key),
            Some(Kind::Steps { .. }) => self.key_steps(id, key),
            Some(Kind::Result { .. }) if matches!(key, UiKey::Space | UiKey::Enter) => {
                self.key_result_activate(id)
            }
            Some(Kind::Popover { .. }) if matches!(key, UiKey::Space | UiKey::Enter) => {
                self.key_popover_toggle(id)
            }
            Some(Kind::Popconfirm { .. }) if matches!(key, UiKey::Space | UiKey::Enter) => {
                self.key_popconfirm_toggle(id)
            }
            Some(Kind::Link { .. }) if matches!(key, UiKey::Space | UiKey::Enter) => {
                self.pending_link_activate = Some(id);
                true
            }
            Some(Kind::Pagination { .. }) => self.key_pagination(id, key),
            Some(Kind::DateTime { .. }) => self.key_datetime(id, key),
            Some(Kind::Transfer { .. }) => self.key_transfer(id, key),
            Some(Kind::PropertyGrid { .. }) => self.key_property_grid(id, key),
            Some(Kind::TreeTable { .. }) => self.key_tree_table(id, key),
            Some(Kind::Button { enabled: true, .. }) if matches!(key, UiKey::Space | UiKey::Enter) => {
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
            Some(Kind::Autocomplete { .. }) => self.key_autocomplete(id, key),
            Some(Kind::Upload { .. }) if matches!(key, UiKey::Space | UiKey::Enter) => {
                self.pending_upload_browse = Some(id);
                true
            }
            Some(Kind::ColorPicker { .. }) => self.key_color_picker(id, key),
            Some(Kind::Calendar { .. }) => self.key_calendar(id, key),
            Some(Kind::TreeSelect { .. }) => self.key_tree_select(id, key),
            Some(Kind::AlarmList { .. }) => self.key_alarm_list(id, key),
            Some(Kind::Slider { .. }) => self.key_slider(id, key),
            Some(Kind::Modal { open: true, .. }) => self.key_modal(id, key),
            Some(Kind::Drawer { open: true, .. }) => self.key_drawer_overlay(id, key),
            _ => false,
        }
    }

    fn key_modal(&mut self, id: WidgetId, key: UiKey) -> bool {
        let Some(Widget { kind: Kind::Modal { secondary, tertiary, action_focus, .. }, .. }) = self.widgets.get_mut(&id) else { return false; };
        let mut actions = vec![ModalAction::Primary];
        if secondary.is_some() { actions.push(ModalAction::Secondary); }
        if tertiary.is_some() { actions.push(ModalAction::Tertiary); }
        match key {
            UiKey::Tab | UiKey::BackTab | UiKey::Left | UiKey::Right => {
                let step = if matches!(key, UiKey::BackTab | UiKey::Left) { actions.len() - 1 } else { 1 };
                *action_focus = (*action_focus + step) % actions.len();
                true
            }
            UiKey::Enter | UiKey::Space => {
                self.pending_modal_action = Some((id, actions[*action_focus % actions.len()]));
                self.close_popup_internal();
                if self.focus == Some(id) { self.focus = None; }
                true
            }
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
            _ => false,
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

    fn key_button_group(&mut self, id: WidgetId, key: UiKey) -> bool {
        let (sel, n) = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::ButtonGroup {
                selected, labels, ..
            }) => (*selected, labels.len()),
            _ => return false,
        };
        if n == 0 {
            return false;
        }
        match key {
            UiKey::Left | UiKey::Up => {
                let next = match sel {
                    None => n - 1,
                    Some(0) => n - 1,
                    Some(s) => s - 1,
                };
                self.set_button_group_selected(id, Some(next))
            }
            UiKey::Right | UiKey::Down => {
                let next = match sel {
                    None => 0,
                    Some(s) => (s + 1) % n,
                };
                self.set_button_group_selected(id, Some(next))
            }
            _ => false,
        }
    }

    fn pointer_button_group(&mut self, scene: &Scene, id: WidgetId, x: f32, _y: f32) -> bool {
        let (n, style, node) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::ButtonGroup { labels, style, .. },
                node,
                ..
            }) => (labels.len(), *style, *node),
            _ => return false,
        };
        let bounds = node
            .and_then(|nid| scene.get(nid).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        if n == 0 {
            return false;
        }
        let gaps = style.gap * (n.saturating_sub(1) as f32);
        let inner_w = (bounds.width - style.padding * 2.0 - gaps).max(1.0);
        let btn_w = inner_w / n as f32;
        let step = btn_w + style.gap;
        let local = x - bounds.x - style.padding;
        if local < 0.0 || local > inner_w + gaps {
            return false;
        }
        let idx = (local / step).floor() as isize;
        if idx >= 0 && (idx as usize) < n {
            return self.set_button_group_selected(id, Some(idx as usize));
        }
        false
    }

    fn key_steps(&mut self, id: WidgetId, key: UiKey) -> bool {
        let (cur, n, dir) = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Steps {
                items, current, style, ..
            }) => (*current, items.len(), style.direction),
            _ => return false,
        };
        if n == 0 {
            return false;
        }
        let prev = matches!(
            (dir, key),
            (StepsDirection::Horizontal, UiKey::Left)
                | (StepsDirection::Vertical, UiKey::Up)
                | (StepsDirection::Horizontal, UiKey::Up)
                | (StepsDirection::Vertical, UiKey::Left)
        );
        let next = matches!(
            (dir, key),
            (StepsDirection::Horizontal, UiKey::Right)
                | (StepsDirection::Vertical, UiKey::Down)
                | (StepsDirection::Horizontal, UiKey::Down)
                | (StepsDirection::Vertical, UiKey::Right)
        );
        if prev {
            let i = if cur == 0 { n - 1 } else { cur - 1 };
            self.set_steps_current(id, i)
        } else if next {
            self.set_steps_current(id, (cur + 1) % n)
        } else {
            false
        }
    }

    fn pointer_steps(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let (n, style, node, clickable) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Steps { items, style, .. },
                node,
                ..
            }) => (items.len(), *style, *node, style.clickable),
            _ => return false,
        };
        if !clickable || n == 0 {
            return false;
        }
        let bounds = node
            .and_then(|nid| scene.get(nid).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        let pad = style.padding;
        match style.direction {
            StepsDirection::Horizontal => {
                let inner_w = (bounds.width - pad * 2.0).max(1.0);
                let slot = inner_w / n as f32;
                let local = x - bounds.x - pad;
                if local < 0.0 || local > inner_w {
                    return false;
                }
                let idx = (local / slot).floor() as isize;
                if idx >= 0 && (idx as usize) < n {
                    return self.set_steps_current(id, idx as usize);
                }
            }
            StepsDirection::Vertical => {
                let inner_h = (bounds.height - pad * 2.0).max(1.0);
                let slot = inner_h / n as f32;
                let local = y - bounds.y - pad;
                if local < 0.0 || local > inner_h {
                    return false;
                }
                let idx = (local / slot).floor() as isize;
                if idx >= 0 && (idx as usize) < n {
                    return self.set_steps_current(id, idx as usize);
                }
            }
        }
        false
    }

    fn key_result_activate(&mut self, id: WidgetId) -> bool {
        let (primary, secondary) = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Result {
                primary_action,
                secondary_action,
                ..
            }) => (primary_action.is_some(), secondary_action.is_some()),
            _ => return false,
        };
        if primary {
            self.pending_result_action = Some((id, ResultAction::Primary));
            true
        } else if secondary {
            self.pending_result_action = Some((id, ResultAction::Secondary));
            true
        } else {
            false
        }
    }

    fn pointer_result(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let (primary, secondary, style, node) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::Result {
                        primary_action,
                        secondary_action,
                        style,
                        ..
                    },
                node,
                ..
            }) => (
                primary_action.clone(),
                secondary_action.clone(),
                *style,
                *node,
            ),
            _ => return false,
        };
        let bounds = node
            .and_then(|nid| scene.get(nid).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        let btn_y = bounds.y + bounds.height - style.padding - style.action_height;
        let btn_h = style.action_height;
        if y < btn_y || y > btn_y + btn_h {
            return false;
        }
        // Layout: [secondary?][gap][primary?] right-aligned from padding.
        let mut cursor = bounds.x + bounds.width - style.padding;
        if let Some(label) = &primary {
            let (tw, _) = self.font.measure_line(label, style.action_size);
            let bw = (tw + 24.0).max(72.0);
            let bx = cursor - bw;
            if x >= bx && x <= cursor {
                self.pending_result_action = Some((id, ResultAction::Primary));
                return true;
            }
            cursor = bx - style.gap;
        }
        if let Some(label) = &secondary {
            let (tw, _) = self.font.measure_line(label, style.action_size);
            let bw = (tw + 24.0).max(72.0);
            let bx = cursor - bw;
            if x >= bx && x <= cursor {
                self.pending_result_action = Some((id, ResultAction::Secondary));
                return true;
            }
        }
        false
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
        let Some(Kind::Table {
            columns,
            scroll_x,
            frozen_cols,
            ..
        }) = self.widgets.get(&id).map(|w| &w.kind)
        else {
            return false;
        };
        let cells = table_column_rects(bounds, columns, style.padding, *scroll_x, *frozen_cols);
        let column_hit = cells
            .iter()
            .position(|(_, clip)| !clip.is_empty() && x >= clip.x && x < (clip.x + clip.width));
        if y < body_top {
            for (i, (origin, clip)) in cells.iter().enumerate() {
                let edge = origin + columns[i].width;
                if !clip.is_empty() && edge <= (clip.x + clip.width) && (x - edge).abs() <= 4.0 {
                    self.drag = Some(DragState::TableColumn {
                        table: id,
                        column: i,
                        start_x: x,
                        start_width: columns[i].width,
                    });
                    return true;
                }
            }
            if let Some(ci) = column_hit {
                self.pending_table_header_click = Some((id, ci));
            }
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
                if let Kind::Table {
                    selected_row,
                    selected_col,
                    selected_range,
                    selected_col_range,
                    ..
                } = &mut w.kind
                {
                    *selected_row = Some(idx as usize);
                    *selected_col = column_hit;
                    *selected_range = Some((idx as usize, idx as usize));
                    *selected_col_range = column_hit.map(|col| (col, col));
                }
            }
        }
        if style.drag_select && idx >= 0 && (idx as usize) < n_rows {
            self.drag = Some(DragState::TableSelection { table: id, bounds });
            return true;
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

    fn drag_table_selection(&mut self, id: WidgetId, bounds: Rect, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() { return false; }
        let Some(Kind::Table { style, scroll_x, scroll_y, .. }) = self.widgets.get(&id).map(|w| &w.kind) else { return false; };
        let body_top = bounds.y + style.header_height;
        let left = bounds.x + style.padding;
        let right = (bounds.x + bounds.width - style.padding - 1.0).max(left);
        let bottom = (bounds.y + bounds.height - 1.0).max(body_top);
        let scroll_changed = self.set_any_scroll(id,
            scroll_x + x - x.clamp(left, right), scroll_y + y - y.clamp(body_top, bottom));
        let Some(Widget { kind: Kind::Table {
            rows, columns, style, scroll_x, scroll_y, frozen_cols,
            selected_row, selected_col, selected_range, selected_col_range, ..
        }, .. }) = self.widgets.get_mut(&id) else { return scroll_changed; };
        if rows.is_empty() || style.row_height <= 0.0 { return scroll_changed; }
        let row = (((y.clamp(body_top, bottom) - body_top + *scroll_y) / style.row_height).floor() as usize).min(rows.len() - 1);
        let anchor_row = selected_range.map_or(row, |r| r.0);
        let col = if selected_col_range.is_some() {
            let px = x.clamp(left, right);
            table_column_rects(bounds, columns, style.padding, *scroll_x, *frozen_cols).iter()
                .position(|(_, clip)| !clip.is_empty() && px >= clip.x && px < clip.x + clip.width)
                .or(*selected_col)
        } else { None };
        let changed = *selected_row != Some(row) || *selected_col != col;
        *selected_row = Some(row);
        *selected_range = Some((anchor_row, row));
        *selected_col = col;
        if let (Some((anchor, _)), Some(col)) = (*selected_col_range, col) {
            *selected_col_range = Some((anchor, col));
        }
        changed || scroll_changed
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
        let Some(Kind::Table {
            selected_row, selected_col, columns, frozen_cols, style, scroll_x, scroll_y, ..
        }) = self.widgets.get(&id).map(|w| &w.kind) else { return; };
        let (mut nx, mut ny) = (*scroll_x, *scroll_y);
        if let Some(i) = selected_row {
            let body_h = (style.height - style.header_height).max(0.0);
            let row_top = *i as f32 * style.row_height;
            let row_bot = row_top + style.row_height;
            if row_top < ny { ny = row_top; }
            else if row_bot > ny + body_h { ny = row_bot - body_h; }
        }
        if let Some(i) = selected_col.filter(|i| *i >= *frozen_cols && *i < columns.len()) {
            let frozen_w: f32 = columns.iter().take(*frozen_cols).map(|c| c.width).sum();
            let col_left: f32 = columns.iter().take(i).map(|c| c.width).sum();
            let view_right = style.width - style.padding * 2.0;
            let view_width = (view_right - frozen_w).max(0.0);
            if col_left - nx < frozen_w || columns[i].width > view_width {
                nx = col_left - frozen_w;
            } else if col_left + columns[i].width - nx > view_right {
                nx = col_left + columns[i].width - view_right;
            }
        }
        self.set_any_scroll(id, nx, ny);
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
        let Some(Kind::Table {
            rows, columns, selected_row, selected_col, selected_range, selected_col_range, ..
        }) = self.widgets.get(&id).map(|w| &w.kind) else { return false; };
        if rows.is_empty() { return false; }
        let (mut row, mut col) = (selected_row.unwrap_or(0), *selected_col);
        let extend = matches!(key, UiKey::SelectUp | UiKey::SelectDown | UiKey::SelectLeft | UiKey::SelectRight);
        match key {
            UiKey::Down | UiKey::SelectDown => row = selected_row.map_or(0, |i| (i + 1).min(rows.len() - 1)),
            UiKey::Up | UiKey::SelectUp => row = row.saturating_sub(1),
            UiKey::Left | UiKey::SelectLeft if !columns.is_empty() => col = Some(col.unwrap_or(0).saturating_sub(1)),
            UiKey::Right | UiKey::SelectRight if !columns.is_empty() => col = Some(col.map_or(0, |i| (i + 1).min(columns.len() - 1))),
            UiKey::Home => row = 0,
            UiKey::End => row = rows.len() - 1,
            _ => return false,
        }
        let anchor_row = if extend { selected_range.map(|r| r.0).or(*selected_row).unwrap_or(row) } else { row };
        let anchor_col = if extend { selected_col_range.map(|r| r.0).or(*selected_col).or(col) } else { col };
        if let Some(Widget { kind: Kind::Table { selected_row, selected_col, selected_range, selected_col_range, .. }, .. }) = self.widgets.get_mut(&id) {
            *selected_row = Some(row);
            *selected_col = col;
            *selected_range = Some((anchor_row, row));
            *selected_col_range = anchor_col.zip(col);
        }
        self.ensure_selected_visible_table(id);
        true
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
                | Kind::DateTime { open, .. }
                | Kind::Popover { open, .. }
                | Kind::Popconfirm { open, .. }
                | Kind::Autocomplete { open, .. } => {
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

    /// Drain the last ButtonGroup selection change `(id, selected)`.
    pub fn take_button_group_change(&mut self) -> Option<(WidgetId, Option<usize>)> {
        self.pending_button_group_change.take()
    }

    /// Drain the last keyboard-activated Link (`Space` / `Enter` while focused).
    ///
    /// Pointer clicks: apps keep using [`Ui::hit_widget`] after [`Ui::pointer_pressed`]
    /// returns `false` for links (same dual path as Button).
    pub fn take_link_activate(&mut self) -> Option<WidgetId> {
        self.pending_link_activate.take()
    }

    /// Drain the last Steps current-index change `(id, current)`.
    pub fn take_steps_change(&mut self) -> Option<(WidgetId, usize)> {
        self.pending_steps_change.take()
    }

    /// Drain the last Result action `(id, Primary|Secondary)`.
    pub fn take_result_action(&mut self) -> Option<(WidgetId, ResultAction)> {
        self.pending_result_action.take()
    }

    /// Drain the last Popover open-state change `(id, open)`.
    pub fn take_popover_open_change(&mut self) -> Option<(WidgetId, bool)> {
        self.pending_popover_open_change.take()
    }

    /// Whether the Popover is open.
    pub fn popover_is_open(&self, id: WidgetId) -> Option<bool> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Popover { open, .. }) => Some(*open),
            _ => None,
        }
    }

    /// Open or close a Popover (also sets / clears popup ownership).
    pub fn set_popover_open(&mut self, id: WidgetId, open: bool) -> bool {
        if !matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Popover { .. })
        ) {
            return false;
        }
        if open {
            if let Some(pid) = self.open_popup {
                if pid != id {
                    self.emit_overlay_dismiss(pid);
                    self.close_popup_internal();
                }
            }
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Popover { open: o, .. } = &mut w.kind {
                    *o = true;
                }
            }
            self.open_popup = Some(id);
            self.focus = Some(id);
            self.pending_popover_open_change = Some((id, true));
        } else {
            let was = matches!(
                self.widgets.get(&id).map(|w| &w.kind),
                Some(Kind::Popover { open: true, .. })
            );
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Popover { open: o, .. } = &mut w.kind {
                    *o = false;
                }
            }
            if self.open_popup == Some(id) {
                self.open_popup = None;
            }
            if was {
                self.pending_popover_open_change = Some((id, false));
            }
        }
        true
    }

    /// Drain the last Popconfirm action `(id, Confirm|Cancel|Dismiss)`.
    pub fn take_popconfirm_action(&mut self) -> Option<(WidgetId, PopconfirmAction)> {
        self.pending_popconfirm_action.take()
    }

    /// Whether the Popconfirm is open.
    pub fn popconfirm_is_open(&self, id: WidgetId) -> Option<bool> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Popconfirm { open, .. }) => Some(*open),
            _ => None,
        }
    }

    /// Open or close a Popconfirm (also sets / clears popup ownership).
    pub fn set_popconfirm_open(&mut self, id: WidgetId, open: bool) -> bool {
        if !matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Popconfirm { .. })
        ) {
            return false;
        }
        if open {
            if let Some(pid) = self.open_popup {
                if pid != id {
                    self.emit_overlay_dismiss(pid);
                    self.close_popup_internal();
                }
            }
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Popconfirm { open: o, .. } = &mut w.kind {
                    *o = true;
                }
            }
            self.open_popup = Some(id);
            self.focus = Some(id);
        } else {
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Popconfirm { open: o, .. } = &mut w.kind {
                    *o = false;
                }
            }
            if self.open_popup == Some(id) {
                self.open_popup = None;
            }
        }
        true
    }

    /// Drain Browse request from an Upload (`Space`/`Enter` or Browse hit).
    ///
    /// OS file dialog is **app responsibility** (or a future platform helper).
    /// Demo apps stub a path via [`Ui::set_upload_path`].
    pub fn take_upload_browse(&mut self) -> Option<WidgetId> {
        self.pending_upload_browse.take()
    }

    /// Drain Clear request from an Upload.
    pub fn take_upload_clear(&mut self) -> Option<WidgetId> {
        self.pending_upload_clear.take()
    }

    /// Current upload path (`None` = empty / cleared).
    pub fn upload_path(&self, id: WidgetId) -> Option<Option<&str>> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Upload { path, .. }) => Some(path.as_deref()),
            _ => None,
        }
    }

    /// Set or clear the upload path (does not emit browse/clear pending).
    pub fn set_upload_path(&mut self, id: WidgetId, path: Option<impl Into<String>>) -> bool {
        let path = path.map(|s| s.into());
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Upload { path: p, .. } = &mut w.kind {
                *p = path;
                return true;
            }
        }
        false
    }

    /// Drain ColorPicker value change `(id, Color)`.
    pub fn take_color_change(&mut self) -> Option<(WidgetId, Color)> {
        self.pending_color_change.take()
    }

    /// Current ColorPicker value.
    pub fn color_picker_value(&self, id: WidgetId) -> Option<Color> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::ColorPicker { value, .. }) => Some(*value),
            _ => None,
        }
    }

    /// Set ColorPicker value (updates hex + matching swatch when possible).
    pub fn set_color_picker_value(&mut self, id: WidgetId, color: Color) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::ColorPicker {
                value,
                hex,
                selected_swatch,
                presets,
                ..
            } = &mut w.kind
            {
                *value = color;
                *hex = color_to_hex(color);
                *selected_swatch = presets.iter().position(|c| {
                    c.r == color.r && c.g == color.g && c.b == color.b
                });
                return true;
            }
        }
        false
    }

    /// Drain Autocomplete commit `(id, value)` after picking a suggestion or
    /// Enter with a non-empty value.
    pub fn take_autocomplete_change(&mut self) -> Option<(WidgetId, String)> {
        self.pending_autocomplete_change.take()
    }

    /// Drain Timeline row select `(id, index)`.
    pub fn take_timeline_select(&mut self) -> Option<(WidgetId, usize)> {
        self.pending_timeline_select.take()
    }

    /// Selected timeline row index.
    pub fn timeline_selected(&self, id: WidgetId) -> Option<Option<usize>> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Timeline { selected, .. }) => Some(*selected),
            _ => None,
        }
    }

    /// Set timeline selection without emitting.
    pub fn set_timeline_selected(&mut self, id: WidgetId, selected: Option<usize>) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Timeline {
                items,
                selected: s,
                ..
            } = &mut w.kind
            {
                *s = selected.and_then(|i| if i < items.len() { Some(i) } else { None });
                return true;
            }
        }
        false
    }

    /// Drain Calendar date change `(id, date)`.
    pub fn take_calendar_change(&mut self) -> Option<(WidgetId, CalendarDate)> {
        self.pending_calendar_change.take()
    }

    /// Current calendar date.
    pub fn calendar_date(&self, id: WidgetId) -> Option<CalendarDate> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Calendar {
                year,
                month,
                selected_day,
                ..
            }) => Some(CalendarDate::new(*year, *month, *selected_day)),
            _ => None,
        }
    }

    /// Set calendar year/month/day (clamps day; does not emit).
    pub fn set_calendar_date(&mut self, id: WidgetId, date: CalendarDate) -> bool {
        let date = date.clamp();
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Calendar {
                year,
                month,
                selected_day,
                ..
            } = &mut w.kind
            {
                *year = date.year;
                *month = date.month;
                *selected_day = date.day;
                return true;
            }
        }
        false
    }

    /// Current autocomplete text.
    pub fn autocomplete_value(&self, id: WidgetId) -> Option<&str> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Autocomplete { value, .. }) => Some(value.as_str()),
            _ => None,
        }
    }

    /// Set autocomplete text (closes popup; does not emit change).
    pub fn set_autocomplete_value(&mut self, id: WidgetId, value: impl Into<String>) -> bool {
        let value = value.into();
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Autocomplete {
                value: v,
                caret,
                open,
                highlight,
                ..
            } = &mut w.kind
            {
                *caret = value.chars().count();
                *v = value;
                *open = false;
                *highlight = None;
                if self.open_popup == Some(id) {
                    self.open_popup = None;
                }
                return true;
            }
        }
        false
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
                if let Kind::Modal { open: o, action_focus, .. } = &mut w.kind {
                    *o = true;
                    *action_focus = 0;
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

    /// Add or remove a third modal action, reported as [`ModalAction::Tertiary`].
    /// Tab/Shift-Tab and arrows move between actions; Enter/Space activates one.
    pub fn set_modal_tertiary(&mut self, id: WidgetId, label: Option<String>) -> bool {
        if let Some(Widget { kind: Kind::Modal { tertiary, action_focus, .. }, .. }) = self.widgets.get_mut(&id) {
            *tertiary = label;
            *action_focus = 0;
            return true;
        }
        false
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

    /// TextArea caret offset in Unicode characters.
    pub fn text_area_caret(&self, id: WidgetId) -> Option<usize> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::TextArea { caret, .. }) => Some(*caret),
            _ => None,
        }
    }

    /// Set TextArea value.
    pub fn set_text_area_value(&mut self, id: WidgetId, value: impl Into<String>) -> bool {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::TextArea {
                value: v, caret, preedit, preedit_cursor, edit, ..
            } = &mut w.kind
            {
                *v = value.into();
                *caret = v.chars().count();
                *edit = TextAreaEdit::default();
                edit.reveal = true;
                preedit.clear();
                *preedit_cursor = None;
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

    /// ButtonGroup selected index (`None` if unset or wrong kind).
    pub fn button_group_selected(&self, id: WidgetId) -> Option<Option<usize>> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::ButtonGroup { selected, .. }) => Some(*selected),
            _ => None,
        }
    }

    /// Set ButtonGroup selected index (`None` clears). Returns true if changed.
    pub fn set_button_group_selected(&mut self, id: WidgetId, index: Option<usize>) -> bool {
        let n = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::ButtonGroup { labels, .. }) => labels.len(),
            _ => return false,
        };
        if let Some(i) = index {
            if n == 0 || i >= n {
                return false;
            }
        }
        let changed = if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::ButtonGroup { selected, .. } = &mut w.kind {
                if *selected == index {
                    false
                } else {
                    *selected = index;
                    true
                }
            } else {
                false
            }
        } else {
            false
        };
        if changed {
            self.pending_button_group_change = Some((id, index));
        }
        changed
    }

    /// Steps current index (`None` if wrong kind).
    pub fn steps_current(&self, id: WidgetId) -> Option<usize> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Steps { current, .. }) => Some(*current),
            _ => None,
        }
    }

    /// Set Steps current index. Returns true if changed.
    pub fn set_steps_current(&mut self, id: WidgetId, index: usize) -> bool {
        let n = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Steps { items, .. }) => items.len(),
            _ => return false,
        };
        if n == 0 || index >= n {
            return false;
        }
        let Some(w) = self.widgets.get_mut(&id) else {
            return false;
        };
        if let Kind::Steps { current, .. } = &mut w.kind {
            if *current == index {
                return false;
            }
            *current = index;
            self.pending_steps_change = Some((id, index));
            true
        } else {
            false
        }
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
                    tertiary: None,
                    action_focus: 0,
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
                    | Kind::Grid { children, .. }
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


    fn emit_overlay_dismiss(&mut self, id: WidgetId) {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Popconfirm { .. }) => {
                self.pending_popconfirm_action = Some((id, PopconfirmAction::Dismiss));
            }
            Some(Kind::Popover { open: true, .. }) => {
                self.pending_popover_open_change = Some((id, false));
            }
            Some(Kind::Modal { .. }) => {
                self.pending_modal_action = Some((id, ModalAction::Dismiss));
            }
            Some(Kind::Drawer { .. }) => {
                self.pending_drawer_action = Some((id, DrawerAction::Dismiss));
            }
            _ => {}
        }
    }

    fn pointer_popover_toggle(&mut self, scene: &mut Scene, id: WidgetId) -> bool {
        let was_open = matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Popover { open: true, .. })
        );
        if let Some(pid) = self.open_popup {
            if pid != id {
                self.emit_overlay_dismiss(pid);
            }
            self.hide_popup_aux_mut(scene, pid);
        }
        self.close_popup_internal();
        if was_open {
            self.pending_popover_open_change = Some((id, false));
            return true;
        }
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Popover { open, .. } = &mut w.kind {
                *open = true;
            }
        }
        self.open_popup = Some(id);
        self.pending_popover_open_change = Some((id, true));
        true
    }

    fn pointer_popconfirm_toggle(&mut self, scene: &mut Scene, id: WidgetId) -> bool {
        let was_open = matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Popconfirm { open: true, .. })
        );
        if let Some(pid) = self.open_popup {
            if pid != id {
                self.emit_overlay_dismiss(pid);
            }
            self.hide_popup_aux_mut(scene, pid);
        }
        self.close_popup_internal();
        if was_open {
            self.pending_popconfirm_action = Some((id, PopconfirmAction::Dismiss));
            return true;
        }
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Popconfirm { open, .. } = &mut w.kind {
                *open = true;
            }
        }
        self.open_popup = Some(id);
        true
    }

    fn key_popover_toggle(&mut self, id: WidgetId) -> bool {
        let was_open = matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Popover { open: true, .. })
        );
        if was_open {
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Popover { open, .. } = &mut w.kind {
                    *open = false;
                }
            }
            if self.open_popup == Some(id) {
                self.open_popup = None;
            }
            self.pending_popover_open_change = Some((id, false));
            return true;
        }
        if let Some(pid) = self.open_popup {
            if pid != id {
                self.emit_overlay_dismiss(pid);
                self.close_popup_internal();
            }
        }
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Popover { open, .. } = &mut w.kind {
                *open = true;
            }
        }
        self.open_popup = Some(id);
        self.pending_popover_open_change = Some((id, true));
        true
    }

    fn key_popconfirm_toggle(&mut self, id: WidgetId) -> bool {
        let was_open = matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Popconfirm { open: true, .. })
        );
        if was_open {
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Popconfirm { open, .. } = &mut w.kind {
                    *open = false;
                }
            }
            if self.open_popup == Some(id) {
                self.open_popup = None;
            }
            self.pending_popconfirm_action = Some((id, PopconfirmAction::Dismiss));
            return true;
        }
        if let Some(pid) = self.open_popup {
            if pid != id {
                self.emit_overlay_dismiss(pid);
                self.close_popup_internal();
            }
        }
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Popconfirm { open, .. } = &mut w.kind {
                *open = true;
            }
        }
        self.open_popup = Some(id);
        true
    }

    fn pointer_popover_popup(&mut self, scene: &mut Scene, id: WidgetId, x: f32, y: f32) -> bool {
        // Host click while open → toggle close.
        if let Some(w) = self.widgets.get(&id) {
            if let Some(node) = w.node {
                if let Some(n) = scene.get(node) {
                    if n.bounds.contains(x, y) {
                        return self.pointer_popover_toggle(scene, id);
                    }
                }
            }
        }
        // Click inside panel: keep open.
        if let Some(w) = self.widgets.get(&id) {
            if let Some(&panel) = w.aux.first() {
                if let Some(n) = scene.get(panel) {
                    if n.visible && n.bounds.contains(x, y) {
                        return true;
                    }
                }
            }
        }
        false
    }


    fn upload_button_rects(bounds: Rect, style: UploadStyle, has_path: bool) -> (Rect, Option<Rect>) {
        let mut right = bounds.x + bounds.width - style.padding;
        let browse = Rect::new(
            right - style.browse_width,
            bounds.y + 2.0,
            style.browse_width,
            (bounds.height - 4.0).max(1.0),
        );
        right = browse.x - style.gap;
        let clear = if has_path && style.show_clear {
            Some(Rect::new(
                right - style.clear_width,
                bounds.y + 2.0,
                style.clear_width,
                (bounds.height - 4.0).max(1.0),
            ))
        } else {
            None
        };
        (browse, clear)
    }

    fn pointer_upload(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let (style, path, node) = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Upload { style, path, .. }) => {
                let node = self.widgets.get(&id).and_then(|w| w.node);
                (*style, path.clone(), node)
            }
            _ => return false,
        };
        let bounds = node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        let (browse, clear) = Self::upload_button_rects(bounds, style, path.is_some());
        if let Some(cr) = clear {
            if cr.contains(x, y) {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Upload { path, .. } = &mut w.kind {
                        *path = None;
                    }
                }
                self.pending_upload_clear = Some(id);
                return true;
            }
        }
        if browse.contains(x, y) {
            self.pending_upload_browse = Some(id);
            return true;
        }
        // Path area: focus only.
        true
    }

    fn pointer_color_picker(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let (presets, style, node) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::ColorPicker { presets, style, .. },
                node,
                ..
            }) => (presets.clone(), *style, *node),
            _ => return false,
        };
        let bounds = node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        let mut sx = bounds.x + style.padding;
        let sy = bounds.y + ((bounds.height - style.swatch_size) * 0.5).max(0.0);
        for (i, c) in presets.iter().enumerate() {
            let r = Rect::new(sx, sy, style.swatch_size, style.swatch_size);
            if r.contains(x, y) {
                let color = *c;
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::ColorPicker {
                        value,
                        hex,
                        selected_swatch,
                        ..
                    } = &mut w.kind
                    {
                        *value = color;
                        *hex = color_to_hex(color);
                        *selected_swatch = Some(i);
                    }
                }
                self.pending_color_change = Some((id, color));
                return true;
            }
            sx += style.swatch_size + style.swatch_gap;
        }
        true
    }

    fn pointer_autocomplete_focus(&mut self, _scene: &mut Scene, id: WidgetId) -> bool {
        // Focus field; open filtered popup when there are matches.
        let value = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Autocomplete { value, .. }) => value.clone(),
            _ => return false,
        };
        let filtered = self.autocomplete_filtered(id);
        if !value.is_empty() && !filtered.is_empty() {
            self.close_popup_internal();
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::Autocomplete { open, highlight, .. } = &mut w.kind {
                    *open = true;
                    *highlight = Some(0);
                }
            }
            self.open_popup = Some(id);
        }
        true
    }

    fn autocomplete_filtered(&self, id: WidgetId) -> Vec<(usize, String)> {
        let (options, value) = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Autocomplete { options, value, .. }) => (options.clone(), value.clone()),
            _ => return Vec::new(),
        };
        let needle = value.to_ascii_lowercase();
        options
            .into_iter()
            .enumerate()
            .filter(|(_, o)| {
                if needle.is_empty() {
                    true
                } else {
                    o.to_ascii_lowercase().contains(&needle)
                }
            })
            .collect()
    }

    fn pointer_autocomplete_popup(&mut self, scene: &mut Scene, id: WidgetId, x: f32, y: f32) -> bool {
        // Host click while open: keep focus / caret, do not close.
        if let Some(w) = self.widgets.get(&id) {
            if let Some(node) = w.node {
                if let Some(n) = scene.get(node) {
                    if n.bounds.contains(x, y) {
                        return true;
                    }
                }
            }
        }
        let filtered = self.autocomplete_filtered(id);
        let (style, aux) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Autocomplete { style, .. },
                aux,
                ..
            }) => (*style, aux.clone()),
            _ => return false,
        };
        let panel = aux
            .first()
            .and_then(|n| scene.get(*n))
            .map(|n| n.bounds)
            .unwrap_or_else(Rect::empty);
        if panel.is_empty() || !panel.contains(x, y) {
            self.hide_popup_aux_mut(scene, id);
            self.close_popup_internal();
            return true;
        }
        let local_y = y - panel.y;
        let idx = if style.item_height > 0.0 {
            (local_y / style.item_height).floor() as isize
        } else {
            -1
        };
        if idx < 0 || (idx as usize) >= filtered.len() {
            return true;
        }
        let chosen = filtered[idx as usize].1.clone();
        self.commit_autocomplete(id, chosen);
        self.hide_popup_aux_mut(scene, id);
        true
    }

    fn commit_autocomplete(&mut self, id: WidgetId, value: String) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Autocomplete {
                value: v,
                caret,
                open,
                highlight,
                ..
            } = &mut w.kind
            {
                *caret = value.chars().count();
                *v = value.clone();
                *open = false;
                *highlight = None;
            }
        }
        if self.open_popup == Some(id) {
            self.open_popup = None;
        }
        self.pending_autocomplete_change = Some((id, value));
    }

    fn key_color_picker(&mut self, id: WidgetId, key: UiKey) -> bool {
        let n = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::ColorPicker { presets, .. }) => presets.len(),
            _ => return false,
        };
        if n == 0 {
            return false;
        }
        match key {
            UiKey::Left | UiKey::Right | UiKey::Up | UiKey::Down => {
                let cur = match self.widgets.get(&id).map(|w| &w.kind) {
                    Some(Kind::ColorPicker { selected_swatch, .. }) => {
                        selected_swatch.unwrap_or(0)
                    }
                    _ => 0,
                };
                let next = match key {
                    UiKey::Right | UiKey::Down => (cur + 1) % n,
                    UiKey::Left | UiKey::Up => {
                        if cur == 0 {
                            n - 1
                        } else {
                            cur - 1
                        }
                    }
                    _ => return false,
                };
                let color = match self.widgets.get(&id).map(|w| &w.kind) {
                    Some(Kind::ColorPicker { presets, .. }) => presets[next],
                    _ => return false,
                };
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::ColorPicker {
                        value,
                        hex,
                        selected_swatch,
                        ..
                    } = &mut w.kind
                    {
                        *value = color;
                        *hex = color_to_hex(color);
                        *selected_swatch = Some(next);
                    }
                }
                self.pending_color_change = Some((id, color));
                true
            }
            UiKey::Char(c) => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::ColorPicker { hex, .. } = &mut w.kind {
                        if hex.len() >= 7 && c != '#' {
                            // keep typing into a fresh draft starting with #
                            if c.is_ascii_hexdigit() {
                                *hex = format!("#{c}");
                                return true;
                            }
                            return false;
                        }
                        if *hex == "#" && c == '#' {
                            return true;
                        }
                        if hex.is_empty() && c == '#' {
                            hex.push('#');
                            return true;
                        }
                        if hex.is_empty() && c.is_ascii_hexdigit() {
                            hex.push('#');
                            hex.push(c.to_ascii_uppercase());
                            return true;
                        }
                        if hex.starts_with('#') && hex.len() < 7 && c.is_ascii_hexdigit() {
                            hex.push(c.to_ascii_uppercase());
                            return true;
                        }
                    }
                }
                false
            }
            UiKey::Backspace => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::ColorPicker { hex, .. } = &mut w.kind {
                        if hex.len() > 1 {
                            hex.pop();
                            return true;
                        }
                        if hex.len() == 1 {
                            hex.clear();
                            return true;
                        }
                    }
                }
                false
            }
            UiKey::Enter => {
                let hex = match self.widgets.get(&id).map(|w| &w.kind) {
                    Some(Kind::ColorPicker { hex, .. }) => hex.clone(),
                    _ => return false,
                };
                let Some(color) = parse_hex_color(&hex) else {
                    return false;
                };
                let _ = self.set_color_picker_value(id, color);
                self.pending_color_change = Some((id, color));
                true
            }
            _ => false,
        }
    }

    fn key_autocomplete(&mut self, id: WidgetId, key: UiKey) -> bool {
        let open = matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Autocomplete { open: true, .. })
        );
        match key {
            UiKey::Escape if open => {
                self.close_popup_internal();
                true
            }
            UiKey::Down | UiKey::Up => {
                let filtered = self.autocomplete_filtered(id);
                if filtered.is_empty() {
                    return false;
                }
                if !open {
                    self.close_popup_internal();
                    if let Some(w) = self.widgets.get_mut(&id) {
                        if let Kind::Autocomplete { open, highlight, .. } = &mut w.kind {
                            *open = true;
                            *highlight = Some(0);
                        }
                    }
                    self.open_popup = Some(id);
                    return true;
                }
                let cur = match self.widgets.get(&id).map(|w| &w.kind) {
                    Some(Kind::Autocomplete { highlight, .. }) => highlight.unwrap_or(0),
                    _ => 0,
                };
                let n = filtered.len();
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
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Autocomplete { highlight, .. } = &mut w.kind {
                        *highlight = Some(next);
                    }
                }
                true
            }
            UiKey::Enter => {
                if open {
                    let filtered = self.autocomplete_filtered(id);
                    let hi = match self.widgets.get(&id).map(|w| &w.kind) {
                        Some(Kind::Autocomplete { highlight, .. }) => highlight.unwrap_or(0),
                        _ => 0,
                    };
                    if let Some((_, s)) = filtered.get(hi) {
                        self.commit_autocomplete(id, s.clone());
                        return true;
                    }
                }
                let value = match self.widgets.get(&id).map(|w| &w.kind) {
                    Some(Kind::Autocomplete { value, .. }) => value.clone(),
                    _ => return false,
                };
                if value.is_empty() {
                    return false;
                }
                self.pending_autocomplete_change = Some((id, value));
                if open {
                    self.close_popup_internal();
                }
                true
            }
            UiKey::Char(c) => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Autocomplete {
                        value,
                        caret,
                        open,
                        highlight,
                        ..
                    } = &mut w.kind
                    {
                        let mut chars: Vec<char> = value.chars().collect();
                        let i = (*caret).min(chars.len());
                        chars.insert(i, c);
                        *value = chars.into_iter().collect();
                        *caret = i + 1;
                        *open = true;
                        *highlight = Some(0);
                    }
                }
                self.close_popup_internal();
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Autocomplete { open, .. } = &mut w.kind {
                        *open = true;
                    }
                }
                self.open_popup = Some(id);
                // If filter empty, still leave open with no rows (or close).
                if self.autocomplete_filtered(id).is_empty() {
                    if let Some(w) = self.widgets.get_mut(&id) {
                        if let Kind::Autocomplete { open, highlight, .. } = &mut w.kind {
                            *open = false;
                            *highlight = None;
                        }
                    }
                    self.open_popup = None;
                }
                true
            }
            UiKey::Backspace => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Autocomplete {
                        value,
                        caret,
                        open,
                        highlight,
                        ..
                    } = &mut w.kind
                    {
                        if *caret == 0 {
                            return false;
                        }
                        let mut chars: Vec<char> = value.chars().collect();
                        let i = *caret - 1;
                        chars.remove(i);
                        *value = chars.into_iter().collect();
                        *caret = i;
                        *highlight = Some(0);
                        *open = !value.is_empty();
                    }
                }
                let should_open = matches!(
                    self.widgets.get(&id).map(|w| &w.kind),
                    Some(Kind::Autocomplete { open: true, .. })
                ) && !self.autocomplete_filtered(id).is_empty();
                if should_open {
                    self.close_popup_internal();
                    if let Some(w) = self.widgets.get_mut(&id) {
                        if let Kind::Autocomplete { open, .. } = &mut w.kind {
                            *open = true;
                        }
                    }
                    self.open_popup = Some(id);
                } else {
                    self.close_popup_internal();
                }
                true
            }
            UiKey::Left => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Autocomplete { caret, .. } = &mut w.kind {
                        if *caret > 0 {
                            *caret -= 1;
                            return true;
                        }
                    }
                }
                false
            }
            UiKey::Right => {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Autocomplete { value, caret, .. } = &mut w.kind {
                        let n = value.chars().count();
                        if *caret < n {
                            *caret += 1;
                            return true;
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }


    fn pointer_popconfirm_popup(&mut self, scene: &mut Scene, id: WidgetId, x: f32, y: f32) -> bool {
        // Host click while open → dismiss.
        if let Some(w) = self.widgets.get(&id) {
            if let Some(node) = w.node {
                if let Some(n) = scene.get(node) {
                    if n.bounds.contains(x, y) {
                        return self.pointer_popconfirm_toggle(scene, id);
                    }
                }
            }
        }
        let aux = self
            .widgets
            .get(&id)
            .map(|w| w.aux.clone())
            .unwrap_or_default();
        // aux: 0=panel, 1=message, 2=ok btn, 3=ok label, 4=cancel btn, 5=cancel label
        let hit = |idx: usize| -> bool {
            aux.get(idx)
                .and_then(|n| scene.get(*n))
                .map(|n| n.visible && n.bounds.contains(x, y))
                .unwrap_or(false)
        };
        if hit(2) || hit(3) {
            self.pending_popconfirm_action = Some((id, PopconfirmAction::Confirm));
            self.hide_popup_aux_mut(scene, id);
            self.close_popup_internal();
            return true;
        }
        if hit(4) || hit(5) {
            self.pending_popconfirm_action = Some((id, PopconfirmAction::Cancel));
            self.hide_popup_aux_mut(scene, id);
            self.close_popup_internal();
            return true;
        }
        if hit(0) || hit(1) {
            return true;
        }
        false
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
            Some(Kind::Popover { .. })
        ) {
            return self.pointer_popover_popup(scene, id, x, y);
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Popconfirm { .. })
        ) {
            return self.pointer_popconfirm_popup(scene, id, x, y);
        }
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
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Autocomplete { .. })
        ) {
            return self.pointer_autocomplete_popup(scene, id, x, y);
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
                            Kind::Popover { .. } => return self.pointer_popover_toggle(scene, id),
                            Kind::Popconfirm { .. } => return self.pointer_popconfirm_toggle(scene, id),
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
                    | Kind::Form { children, .. }
                    | Kind::Grid { children, .. } => children.as_slice(),
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
        // 5=body; action background/label pairs start at 6 (primary, secondary, tertiary).
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
        if hit(10) {
            self.pending_modal_action = Some((id, ModalAction::Tertiary));
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
        if self.reveal_scroll_focus(scene) {
            for root in self.roots.clone() {
                self.layout_widget(scene, root);
                self.paint_widget(scene, root);
            }
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
                Some(Kind::ButtonGroup { style, .. }) => (style.bg, true),
                Some(Kind::Link { .. }) => (Color::rgba(0, 0, 0, 0), false),
                Some(Kind::Steps { style, .. }) => (
                    style.bg,
                    style.bg.a > 0,
                ),
                Some(Kind::Statistic { style, .. }) => (style.bg, true),
                Some(Kind::Result { style, .. }) => (style.bg, true),
                Some(Kind::Popover { style, .. }) => (style.trigger_bg, true),
                Some(Kind::Popconfirm { style, .. }) => (style.trigger_bg, true),
                Some(Kind::Upload { style, .. }) => (style.bg, true),
                Some(Kind::ColorPicker { style, .. }) => (style.bg, style.bg.a > 0),
                Some(Kind::Autocomplete { style, .. }) => (style.bg, true),
                Some(Kind::Grid { style, .. }) => (
                    style.background.unwrap_or(Color::rgba(0, 0, 0, 0)),
                    style.background.is_some(),
                ),
                Some(Kind::Timeline { style, .. }) => (style.bg, true),
                Some(Kind::Calendar { style, .. }) => (style.bg, true),
                Some(Kind::Image { style, .. }) => (style.bg, true),
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
                Some(Kind::ButtonGroup { .. }) => {
                    self.node_to_widget.insert(node, id);
                }
                Some(Kind::Link { style, .. }) => {
                    self.node_to_widget.insert(node, id);
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), style.fg)
                        .expect("insert link label");
                    let underline = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), style.fg)
                        .expect("insert link underline");
                    scene.set_visible(underline, false);
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                        w.chrome_node = Some(underline);
                    }
                }
                Some(Kind::Steps { .. }) | Some(Kind::Statistic { .. }) | Some(Kind::Result { .. }) => {
                    self.node_to_widget.insert(node, id);
                }
                Some(Kind::Popover { .. }) | Some(Kind::Popconfirm { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let fg = match self.widgets.get(&id).map(|w| &w.kind) {
                        Some(Kind::Popover { style, .. }) => style.trigger_fg,
                        Some(Kind::Popconfirm { style, .. }) => style.trigger_fg,
                        _ => Color::rgb(255, 255, 255),
                    };
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), fg)
                        .expect("insert trigger label");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                    }
                }
                Some(Kind::Upload { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert upload path label");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                    }
                }
                Some(Kind::ColorPicker { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert color hex label");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                    }
                }
                Some(Kind::Autocomplete { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert autocomplete label");
                    let caret = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(0x3b, 0x82, 0xf6))
                        .expect("insert autocomplete caret");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                        w.chrome_node = Some(caret);
                    }
                }
                Some(Kind::Timeline { .. }) | Some(Kind::Calendar { .. }) | Some(Kind::Image { .. }) => {
                    self.node_to_widget.insert(node, id);
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
                    | Kind::Grid { children, .. }
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
            Some(Kind::ButtonGroup { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Link { label, style, .. }) => {
                let (w, h) = self.font.measure_line(label, style.size);
                let uh = if style.underline { 3.0 } else { 0.0 };
                ChildMeasure::new(w, h + uh + 4.0)
            },
            Some(Kind::Steps { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Statistic { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Result { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Popover { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Popconfirm { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Upload { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::ColorPicker { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Autocomplete { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Grid { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Timeline { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Calendar { style, .. }) => ChildMeasure::new(style.width, style.height),
            Some(Kind::Image { style, .. }) => ChildMeasure::new(style.width, style.height),
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
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Grid { .. })
        ) {
            self.layout_grid(scene, id);
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
            scene.set_visible(node, has_bg);
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
                Kind::TextArea { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
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
                    | Kind::Grid { .. }
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
            scene.set_visible(node, style.background.is_some());
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


    /// Recursively hide a widget and all descendants (scene nodes + aux/chrome/label).
    /// Used so inactive Tabs pages do not leak painted children.
    fn hide_widget_subtree(&mut self, scene: &mut Scene, id: WidgetId) {
        let (node, chrome, label, aux, children) = match self.widgets.get(&id) {
            Some(w) => {
                let children = match &w.kind {
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
                    | Kind::Grid { children, .. }
                    | Kind::Spin { children, .. } => children.clone(),
                    _ => Vec::new(),
                };
                (w.node, w.chrome_node, w.label_node, w.aux.clone(), children)
            }
            None => return,
        };
        if let Some(n) = node {
            scene.set_visible(n, false);
        }
        if let Some(n) = chrome {
            scene.set_visible(n, false);
        }
        if let Some(n) = label {
            scene.set_visible(n, false);
        }
        for &n in &aux {
            scene.set_visible(n, false);
        }
        if let Some(nodes) = self.scrollbars.get(&id) {
            for &n in nodes { scene.set_visible(n, false); }
        }
        for child in children {
            self.hide_widget_subtree(scene, child);
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
            } else {
                self.hide_widget_subtree(scene, child_id);
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

        let mut content_w = total.width.max(w);
        let mut content_h = total.height.max(h);
        // Nested flex containers can expand beyond their requested size during layout.
        for child in &children {
            if let Some(bounds) = self
                .widgets
                .get(child)
                .and_then(|w| w.node)
                .and_then(|n| scene.get(n))
                .map(|n| n.bounds)
            {
                content_w = content_w.max(bounds.x + bounds.width - ox + scroll_x + style.padding);
                content_h = content_h.max(bounds.y + bounds.height - oy + scroll_y + style.padding);
            }
        }
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
                Kind::Grid { style, .. } => {
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
                Kind::Grid { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                _ => {}
            }
        }
    }

    /// Apply ThemeRadii to a chrome node (`control` or `panel`).
    ///
    /// Soft paints rounded chrome for Button/TextInput/Panel/Card/Modal·Drawer plus
    /// Tag/Alert/Select·TreeSelect/Autocomplete/Checkbox/Switch/Tooltip/Popover/List/
    /// Badge/Progress/Tabs/Toolbar·StatusBar/TextArea/InputNumber/Notification
    /// (Radio uses geometric circles; Switch thumb is circular).
    fn apply_corner_radius(&self, scene: &mut Scene, node: NodeId, panel: bool) {
        let r = if panel {
            self.theme.radii.panel
        } else {
            self.theme.radii.control
        };
        scene.set_corner_radius(node, r);
    }

    fn align_table_filters(&mut self, scene: &mut Scene, row: WidgetId) {
        let Some(group) = self.table_filters.get(&row).cloned() else {
            return;
        };
        let Some(Widget {
            kind:
                Kind::Table {
                    columns,
                    scroll_x,
                    scroll_y,
                    frozen_cols,
                    style,
                    ..
                },
            node: Some(node),
            ..
        }) = self.widgets.get(&group.table)
        else {
            return;
        };
        let columns = columns.clone();
        let frozen = *frozen_cols;
        let padding = style.padding;
        let (mut sx, sy) = (*scroll_x, *scroll_y);
        let bounds = scene
            .get(*node)
            .map(|n| n.bounds)
            .unwrap_or_else(Rect::empty);
        let focused = self.focus.filter(|id| group.fields.contains(id));
        if focused != group.last_focus {
            if let Some(index) = focused
                .and_then(|id| group.fields.iter().position(|&field| field == id))
                .filter(|&i| i >= frozen)
            {
                let left: f32 = columns.iter().take(index).map(|c| c.width.max(1.0)).sum();
                let frozen_w: f32 = columns.iter().take(frozen).map(|c| c.width.max(1.0)).sum();
                let viewport = (bounds.width - 2.0 * padding).max(0.0);
                let width = columns[index].width.max(1.0);
                if left - sx < frozen_w || width > viewport - frozen_w {
                    sx = left - frozen_w;
                } else if left + width - sx > viewport {
                    sx = left + width - viewport;
                }
                self.set_table_scroll_offset(group.table, sx.max(0.0), sy);
                sx = self.table_scroll_offset(group.table).unwrap().0;
            }
            self.table_filters.get_mut(&row).unwrap().last_focus = focused;
        }
        let geometry = table_column_rects(bounds, &columns, padding, sx, frozen);
        for (index, field) in group.fields.iter().enumerate() {
            let Some(node) = self.widgets.get(field).and_then(|w| w.node) else {
                continue;
            };
            let Some(old) = scene.get(node).map(|n| n.bounds) else {
                continue;
            };
            let (origin, clip) = geometry[index];
            let field_bounds = Rect::new(
                origin + 2.0,
                old.y,
                (columns[index].width - 4.0).max(1.0),
                old.height,
            );
            scene.set_bounds(node, field_bounds);
            scene.set_clip(
                node,
                Some(field_bounds.intersect(&Rect::new(clip.x, old.y, clip.width, old.height))),
            );
        }
    }

    fn paint_widget(&mut self, scene: &mut Scene, id: WidgetId) {
        self.paint_widget_content(scene, id);
        self.paint_scrollbars(scene, id);
    }

    fn paint_widget_content(&mut self, scene: &mut Scene, id: WidgetId) {
        if self.table_filters.contains_key(&id) {
            self.align_table_filters(scene, id);
        }
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
                | Kind::Form { children, .. }
                | Kind::Grid { children, .. }
                | Kind::Spin { children, .. },
            ) => Some(children.clone()),
            _ => None,
        };
        if let Some(children) = container_children {
            if matches!(
                self.widgets.get(&id).map(|w| &w.kind),
                Some(Kind::Panel { .. })
            ) {
                if let Some(node) = self.widgets.get(&id).and_then(|w| w.node) {
                    self.apply_corner_radius(scene, node, true);
                }
                self.paint_panel_title(scene, id);
            }
            if matches!(
                self.widgets.get(&id).map(|w| &w.kind),
                Some(Kind::Toolbar { .. } | Kind::StatusBar { .. })
            ) {
                if let Some(node) = self.widgets.get(&id).and_then(|w| w.node) {
                    self.apply_corner_radius(scene, node, false);
                }
            }
            if matches!(
                self.widgets.get(&id).map(|w| &w.kind),
                Some(Kind::Form { .. } | Kind::Grid { .. })
            ) {
                if let Some(node) = self.widgets.get(&id).and_then(|w| w.node) {
                    self.apply_corner_radius(scene, node, true);
                }
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
                        // Enforce every frame: inactive pages (incl. Table aux) stay hidden.
                        self.hide_widget_subtree(scene, child);
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
            Some(Kind::ButtonGroup { .. })
        ) {
            self.paint_button_group(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Link { .. })
        ) {
            self.paint_link(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Steps { .. })
        ) {
            self.paint_steps(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Statistic { .. })
        ) {
            self.paint_statistic(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Result { .. })
        ) {
            self.paint_result(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Popover { .. })
        ) {
            self.paint_popover_trigger(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Popconfirm { .. })
        ) {
            self.paint_popconfirm_trigger(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Upload { .. })
        ) {
            self.paint_upload(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::ColorPicker { .. })
        ) {
            self.paint_color_picker(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Autocomplete { .. })
        ) {
            self.paint_autocomplete_field(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Timeline { .. })
        ) {
            self.paint_timeline(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Calendar { .. })
        ) {
            self.paint_calendar(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Image { .. })
        ) {
            self.paint_image(scene, id);
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
                enabled,
                ..
            } => {
                let label = label.clone();
                let fg = if *enabled { *fg } else { self.theme.colors.disabled };
                let size = *size;
                let bg = if *enabled { *bg } else { self.theme.colors.muted };
                scene.set_fill(node, bg);
                scene.set_mask(node, None);
                self.apply_corner_radius(scene, node, false);
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
                secret,
                placeholder,
                caret,
                select_all,
                preedit,
                preedit_cursor,
                style,
            } => {
                let value = if *secret {
                    "*".repeat(value.chars().count())
                } else {
                    value.clone()
                };
                let placeholder = placeholder.clone();
                let caret = *caret;
                let select_all = *select_all;
                let preedit = if *secret {
                    "*".repeat(preedit.chars().count())
                } else {
                    preedit.clone()
                };
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
                self.apply_corner_radius(scene, node, false);
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
                self.apply_corner_radius(scene, node, false);
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
                scene.set_corner_radius(node, d * 0.5);
                if let Some(chrome) = self.widgets.get(&id).and_then(|w| w.chrome_node) {
                    if selected {
                        let inset = d * 0.28;
                        let dd = d - inset * 2.0;
                        scene.set_bounds(
                            chrome,
                            Rect::new(ring.x + inset, ring.y + inset, dd, dd),
                        );
                        scene.set_fill(chrome, style.dot);
                        scene.set_mask(chrome, None);
                        scene.set_visible(chrome, true);
                        scene.set_corner_radius(chrome, dd * 0.5);
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
                self.apply_corner_radius(scene, node, false);
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
                    scene.set_corner_radius(chrome, thumb_d * 0.5);
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
            | Kind::TreeTable { .. }
            | Kind::ButtonGroup { .. }
            | Kind::Link { .. }
            | Kind::Steps { .. }
            | Kind::Statistic { .. }
            | Kind::Result { .. }
            | Kind::Popover { .. }
            | Kind::Popconfirm { .. }
            | Kind::Upload { .. }
            | Kind::ColorPicker { .. }
            | Kind::Autocomplete { .. }
            | Kind::Grid { .. }
            | Kind::Timeline { .. }
            | Kind::Calendar { .. }
            | Kind::Image { .. } => {}
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
        self.apply_corner_radius(scene, host, true);

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
                    self.apply_corner_radius(scene, chrome, false);
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
        self.apply_corner_radius(scene, host, true);

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
                    self.apply_corner_radius(scene, chrome, false);
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
                    selected_col,
                    selected_range,
                    selected_col_range,
                    scroll_y,
                    scroll_x,
                    frozen_cols,
                    cell_bg,
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
        let selected_col = *selected_col;
        let selected_range = *selected_range;
        let selected_col_range = *selected_col_range;
        let scroll_y = *scroll_y;
        let scroll_x = *scroll_x;
        let frozen_cols = *frozen_cols;
        let cell_bg = cell_bg.clone();
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
        self.apply_corner_radius(scene, host, true);

        let ncols = columns.len().max(1);
        let body_h = (bounds.height - style.header_height).max(0.0);
        let (first, count) = if virtualized {
            p1_rest::virtual_window(scroll_y, body_h, style.row_height, rows.len(), 2)
        } else {
            (0, rows.len())
        };
        // aux: [header x ncols] + window slots [row_bg + cell_bg x ncols + cell_text x ncols]
        let needed = ncols + count * (1 + 2 * ncols);
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

        let cells = table_column_rects(bounds, &columns, style.padding, scroll_x, frozen_cols);
        for (ci, col) in columns.iter().enumerate() {
            let (ox, clip) = cells[ci];
            let (_, mask) = self.font.rasterize_text(&col.title, style.size, None);
            if let Some(&lab) = aux.get(ci) {
                let ly = bounds.y + ((style.header_height - mask.height as f32) * 0.5).max(0.0);
                scene.set_bounds(
                    lab,
                    Rect::new(ox + 4.0, ly, mask.width as f32, mask.height as f32),
                );
                scene.set_clip(
                    lab,
                    Some(clip.intersect(&Rect::new(
                        bounds.x,
                        bounds.y,
                        bounds.width,
                        style.header_height,
                    ))),
                );
                scene.set_fill(lab, style.header_fg);
                scene.set_mask(
                    lab,
                    Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                );
                scene.set_visible(lab, !clip.is_empty());
            }
        }

        let body_top = bounds.y + style.header_height;
        let body_clip = Rect::new(bounds.x, body_top, bounds.width, body_h);
        for slot in 0..count {
            let ri = first + slot;
            let Some(row) = rows.get(ri) else { break };
            let base = ncols + slot * (1 + 2 * ncols);
            let y = body_top + ri as f32 * style.row_height - scroll_y;
            if !virtualized
                && (y + style.row_height < body_top - style.row_height
                    || y > bounds.y + bounds.height + style.row_height)
            {
                for k in 0..(1 + 2 * ncols) {
                    if let Some(&n) = aux.get(base + k) {
                        scene.set_visible(n, false);
                    }
                }
                continue;
            }
            let row_sel = selected_range.map_or(selected_row == Some(ri), |(start, end)| ri >= start.min(end) && ri <= start.max(end));
            let has_col_selection = selected_col_range.is_some() || selected_col.is_some();
            let full_row_sel = row_sel && !has_col_selection;
            if let Some(&bg) = aux.get(base) {
                scene.set_clip(bg, Some(body_clip));
                scene.set_bounds(
                    bg,
                    Rect::new(
                        bounds.x + 1.0,
                        y,
                        (bounds.width - 2.0).max(1.0),
                        style.row_height,
                    ),
                );
                if full_row_sel {
                    scene.set_fill(bg, style.selected_bg);
                    scene.set_mask(bg, None);
                    scene.set_visible(bg, true);
                    self.apply_corner_radius(scene, bg, false);
                } else {
                    scene.set_visible(bg, false);
                }
            }
            for (ci, col) in columns.iter().enumerate() {
                let cell = row.get(ci).map(|s| s.as_str()).unwrap_or("");
                let fmt = cell_bg.get(ri).and_then(|r| r.get(ci)).and_then(|c| *c);
                let (ox, clip) = cells[ci];
                let clip = clip.intersect(&body_clip);
                let visible = !clip.is_empty();
                let col_sel = selected_col_range.map_or(selected_col == Some(ci), |(start, end)| ci >= start.min(end) && ci <= start.max(end));
                let is_sel = row_sel && (!has_col_selection || col_sel);
                // cell background node
                if let Some(&cbg) = aux.get(base + 1 + ci) {
                    scene.set_clip(cbg, Some(clip));
                    if visible {
                        if is_sel {
                            scene.set_bounds(
                                cbg,
                                Rect::new(ox, y, col.width.max(1.0), style.row_height),
                            );
                            scene.set_fill(cbg, style.selected_bg);
                            scene.set_mask(cbg, None);
                            scene.set_visible(cbg, true);
                        } else if let Some(fill) = fmt {
                            scene.set_bounds(
                                cbg,
                                Rect::new(ox, y, col.width.max(1.0), style.row_height),
                            );
                            scene.set_fill(cbg, fill);
                            scene.set_mask(cbg, None);
                            scene.set_visible(cbg, true);
                        } else {
                            scene.set_visible(cbg, false);
                        }
                    } else {
                        scene.set_visible(cbg, false);
                    }
                }
                let (_, mask) = self.font.rasterize_text(cell, style.size, None);
                if let Some(&lab) = aux.get(base + 1 + ncols + ci) {
                    scene.set_clip(lab, Some(clip));
                    let ly = y + ((style.row_height - mask.height as f32) * 0.5).max(0.0);
                    let max_w = (col.width - 8.0).max(4.0);
                    if visible {
                        scene.set_bounds(
                            lab,
                            Rect::new(
                                ox + 4.0,
                                ly,
                                (mask.width as f32).min(max_w),
                                mask.height as f32,
                            ),
                        );
                        let fg = if is_sel {
                            style.selected_fg
                        } else if fmt.is_some() {
                            // Contrasting text on filled cell
                            Color::rgb(0x10, 0x10, 0x18)
                        } else {
                            style.fg
                        };
                        scene.set_fill(lab, fg);
                        scene.set_mask(
                            lab,
                            Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                        );
                        scene.set_visible(lab, true);
                    } else {
                        scene.set_visible(lab, false);
                    }
                }
            }
        }

        // Hide leftover slots beyond current window
        let used = ncols + count * (1 + 2 * ncols);
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
        self.apply_corner_radius(scene, node, false);
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
        self.apply_corner_radius(scene, node, false);
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
            self.apply_corner_radius(scene, track_n, false);
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
                self.apply_corner_radius(scene, bg, false);
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
        self.apply_corner_radius(scene, node, true);
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
        self.apply_corner_radius(scene, panel_n, true);

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
            self.apply_corner_radius(scene, bg, false);
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
        self.apply_corner_radius(scene, host, false);
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
        self.apply_corner_radius(scene, host, true);
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
        self.apply_corner_radius(scene, host, true);
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
        self.apply_corner_radius(scene, host, false);
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
                if is_sel {
                    self.apply_corner_radius(scene, bg, false);
                }
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

    fn paint_button_group(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::ButtonGroup {
                    labels,
                    selected,
                    style,
                },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let labels = labels.clone();
        let selected = *selected;
        let style = *style;
        let Some(host) = *node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        self.apply_corner_radius(scene, host, false);
        let n = labels.len().max(1);
        let gaps = style.gap * (n.saturating_sub(1) as f32);
        let inner_w = (bounds.width - style.padding * 2.0 - gaps).max(1.0);
        let btn_w = inner_w / n as f32;
        let btn_h = (bounds.height - style.padding * 2.0).max(1.0);
        // aux: per button [bg, label]
        self.ensure_aux_nodes(scene, host, id, n * 2);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        for (i, lab) in labels.iter().enumerate() {
            let x = bounds.x + style.padding + i as f32 * (btn_w + style.gap);
            let y = bounds.y + style.padding;
            let is_sel = selected == Some(i);
            if let Some(&bg) = aux.get(i * 2) {
                scene.set_bounds(bg, Rect::new(x, y, btn_w, btn_h));
                scene.set_fill(
                    bg,
                    if is_sel {
                        style.selected_bg
                    } else {
                        style.border
                    },
                );
                scene.set_mask(bg, None);
                scene.set_visible(bg, true);
                self.apply_corner_radius(scene, bg, false);
            }
            let (_, mask) = self.font.rasterize_text(lab, style.size, None);
            if let Some(&lab_n) = aux.get(i * 2 + 1) {
                scene.set_bounds(
                    lab_n,
                    Rect::new(
                        x + ((btn_w - mask.width as f32) * 0.5).max(0.0),
                        y + ((btn_h - mask.height as f32) * 0.5).max(0.0),
                        mask.width as f32,
                        mask.height as f32,
                    ),
                );
                scene.set_fill(
                    lab_n,
                    if is_sel {
                        style.selected_fg
                    } else {
                        style.fg
                    },
                );
                scene.set_mask(
                    lab_n,
                    Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                );
                scene.set_visible(lab_n, true);
            }
        }
    }

    fn paint_link(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Link { label, style },
            node,
            label_node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let label = label.clone();
        let style = *style;
        let Some(host) = *node else { return };
        let Some(lab_n) = *label_node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, Color::rgba(0, 0, 0, 0));
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        let (_, mask) = self.font.rasterize_text(&label, style.size, None);
        let lx = bounds.x;
        let ly = bounds.y + ((bounds.height - mask.height as f32 - if style.underline { 3.0 } else { 0.0 }) * 0.5).max(0.0);
        scene.set_bounds(
            lab_n,
            Rect::new(lx, ly, mask.width as f32, mask.height as f32),
        );
        scene.set_fill(lab_n, style.fg);
        scene.set_mask(lab_n, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
        scene.set_visible(lab_n, true);
        if let Some(under) = *chrome_node {
            if style.underline {
                scene.set_bounds(
                    under,
                    Rect::new(lx, ly + mask.height as f32 + 1.0, mask.width as f32, 1.0),
                );
                scene.set_fill(under, style.fg);
                scene.set_mask(under, None);
                scene.set_visible(under, true);
            } else {
                scene.set_visible(under, false);
            }
        }
    }


    fn paint_steps(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Steps {
                    items,
                    current,
                    style,
                },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let items = items.clone();
        let current = *current;
        let style = *style;
        let Some(host) = *node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        if style.bg.a > 0 {
            self.apply_corner_radius(scene, host, true);
        }
        let n = items.len();
        if n == 0 {
            return;
        }
        // aux per step: [marker_bg, marker_label, title, desc?, connector?]
        // Fixed: marker, mlabel, title, desc, connector = 5 per step
        self.ensure_aux_nodes(scene, host, id, n * 5);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        let pad = style.padding;
        let marker = style.marker;
        match style.direction {
            StepsDirection::Horizontal => {
                let inner_w = (bounds.width - pad * 2.0).max(1.0);
                let slot = inner_w / n as f32;
                for (i, item) in items.iter().enumerate() {
                    let cx = bounds.x + pad + slot * (i as f32 + 0.5);
                    let my = bounds.y + pad;
                    let (bg_c, fg_c) = match i.cmp(&current) {
                        std::cmp::Ordering::Less => (style.done_bg, style.done_fg),
                        std::cmp::Ordering::Equal => (style.process_bg, style.process_fg),
                        std::cmp::Ordering::Greater => (style.wait_bg, style.wait_fg),
                    };
                    let mx = cx - marker * 0.5;
                    if let Some(&mbg) = aux.get(i * 5) {
                        scene.set_bounds(mbg, Rect::new(mx, my, marker, marker));
                        scene.set_fill(mbg, bg_c);
                        scene.set_mask(mbg, None);
                        scene.set_visible(mbg, true);
                        scene.set_corner_radius(mbg, marker * 0.5);
                    }
                    let num = format!("{}", i + 1);
                    let (_, nm) = self.font.rasterize_text(&num, style.size, None);
                    if let Some(&mlab) = aux.get(i * 5 + 1) {
                        scene.set_bounds(
                            mlab,
                            Rect::new(
                                mx + ((marker - nm.width as f32) * 0.5).max(0.0),
                                my + ((marker - nm.height as f32) * 0.5).max(0.0),
                                nm.width as f32,
                                nm.height as f32,
                            ),
                        );
                        scene.set_fill(mlab, Color::rgb(0xff, 0xff, 0xff));
                        scene.set_mask(
                            mlab,
                            Some(AlphaMask::new(nm.width, nm.height, nm.pixels)),
                        );
                        scene.set_visible(mlab, true);
                    }
                    let (_, tm) = self.font.rasterize_text(&item.title, style.size, None);
                    let ty = my + marker + 4.0;
                    if let Some(&title_n) = aux.get(i * 5 + 2) {
                        scene.set_bounds(
                            title_n,
                            Rect::new(
                                cx - tm.width as f32 * 0.5,
                                ty,
                                tm.width as f32,
                                tm.height as f32,
                            ),
                        );
                        scene.set_fill(title_n, fg_c);
                        scene.set_mask(
                            title_n,
                            Some(AlphaMask::new(tm.width, tm.height, tm.pixels)),
                        );
                        scene.set_visible(title_n, true);
                    }
                    if let Some(&desc_n) = aux.get(i * 5 + 3) {
                        if let Some(desc) = &item.description {
                            let (_, dm) = self.font.rasterize_text(desc, style.desc_size, None);
                            scene.set_bounds(
                                desc_n,
                                Rect::new(
                                    cx - dm.width as f32 * 0.5,
                                    ty + tm.height as f32 + 2.0,
                                    dm.width as f32,
                                    dm.height as f32,
                                ),
                            );
                            scene.set_fill(desc_n, style.wait_fg);
                            scene.set_mask(
                                desc_n,
                                Some(AlphaMask::new(dm.width, dm.height, dm.pixels)),
                            );
                            scene.set_visible(desc_n, true);
                        } else {
                            scene.set_visible(desc_n, false);
                        }
                    }
                    if let Some(&conn) = aux.get(i * 5 + 4) {
                        if i + 1 < n {
                            let x0 = mx + marker + 2.0;
                            let x1 = bounds.x + pad + slot * (i as f32 + 1.5) - marker * 0.5 - 2.0;
                            let w = (x1 - x0).max(1.0);
                            scene.set_bounds(
                                conn,
                                Rect::new(x0, my + marker * 0.5 - 1.0, w, 2.0),
                            );
                            scene.set_fill(
                                conn,
                                if i < current {
                                    style.done_bg
                                } else {
                                    style.connector
                                },
                            );
                            scene.set_mask(conn, None);
                            scene.set_visible(conn, true);
                        } else {
                            scene.set_visible(conn, false);
                        }
                    }
                }
            }
            StepsDirection::Vertical => {
                let inner_h = (bounds.height - pad * 2.0).max(1.0);
                let slot = inner_h / n as f32;
                for (i, item) in items.iter().enumerate() {
                    let cy = bounds.y + pad + slot * (i as f32 + 0.5);
                    let mx = bounds.x + pad;
                    let my = cy - marker * 0.5;
                    let (bg_c, fg_c) = match i.cmp(&current) {
                        std::cmp::Ordering::Less => (style.done_bg, style.done_fg),
                        std::cmp::Ordering::Equal => (style.process_bg, style.process_fg),
                        std::cmp::Ordering::Greater => (style.wait_bg, style.wait_fg),
                    };
                    if let Some(&mbg) = aux.get(i * 5) {
                        scene.set_bounds(mbg, Rect::new(mx, my, marker, marker));
                        scene.set_fill(mbg, bg_c);
                        scene.set_mask(mbg, None);
                        scene.set_visible(mbg, true);
                        scene.set_corner_radius(mbg, marker * 0.5);
                    }
                    let num = format!("{}", i + 1);
                    let (_, nm) = self.font.rasterize_text(&num, style.size, None);
                    if let Some(&mlab) = aux.get(i * 5 + 1) {
                        scene.set_bounds(
                            mlab,
                            Rect::new(
                                mx + ((marker - nm.width as f32) * 0.5).max(0.0),
                                my + ((marker - nm.height as f32) * 0.5).max(0.0),
                                nm.width as f32,
                                nm.height as f32,
                            ),
                        );
                        scene.set_fill(mlab, Color::rgb(0xff, 0xff, 0xff));
                        scene.set_mask(
                            mlab,
                            Some(AlphaMask::new(nm.width, nm.height, nm.pixels)),
                        );
                        scene.set_visible(mlab, true);
                    }
                    let tx = mx + marker + 8.0;
                    let (_, tm) = self.font.rasterize_text(&item.title, style.size, None);
                    if let Some(&title_n) = aux.get(i * 5 + 2) {
                        scene.set_bounds(
                            title_n,
                            Rect::new(tx, cy - tm.height as f32 * 0.5, tm.width as f32, tm.height as f32),
                        );
                        scene.set_fill(title_n, fg_c);
                        scene.set_mask(
                            title_n,
                            Some(AlphaMask::new(tm.width, tm.height, tm.pixels)),
                        );
                        scene.set_visible(title_n, true);
                    }
                    if let Some(&desc_n) = aux.get(i * 5 + 3) {
                        if let Some(desc) = &item.description {
                            let (_, dm) = self.font.rasterize_text(desc, style.desc_size, None);
                            scene.set_bounds(
                                desc_n,
                                Rect::new(
                                    tx,
                                    cy + tm.height as f32 * 0.5 + 2.0,
                                    dm.width as f32,
                                    dm.height as f32,
                                ),
                            );
                            scene.set_fill(desc_n, style.wait_fg);
                            scene.set_mask(
                                desc_n,
                                Some(AlphaMask::new(dm.width, dm.height, dm.pixels)),
                            );
                            scene.set_visible(desc_n, true);
                        } else {
                            scene.set_visible(desc_n, false);
                        }
                    }
                    if let Some(&conn) = aux.get(i * 5 + 4) {
                        if i + 1 < n {
                            let y0 = my + marker + 2.0;
                            let y1 = bounds.y + pad + slot * (i as f32 + 1.5) - marker * 0.5 - 2.0;
                            let h = (y1 - y0).max(1.0);
                            scene.set_bounds(
                                conn,
                                Rect::new(mx + marker * 0.5 - 1.0, y0, 2.0, h),
                            );
                            scene.set_fill(
                                conn,
                                if i < current {
                                    style.done_bg
                                } else {
                                    style.connector
                                },
                            );
                            scene.set_mask(conn, None);
                            scene.set_visible(conn, true);
                        } else {
                            scene.set_visible(conn, false);
                        }
                    }
                }
            }
        }
    }

    fn paint_statistic(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Statistic {
                    title,
                    value,
                    prefix,
                    suffix,
                    hint,
                    style,
                },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let title = title.clone();
        let value = value.clone();
        let prefix = prefix.clone();
        let suffix = suffix.clone();
        let hint = hint.clone();
        let style = *style;
        let Some(host) = *node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        self.apply_corner_radius(scene, host, true);
        // aux: title, value_line, hint
        self.ensure_aux_nodes(scene, host, id, 3);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        let x = bounds.x + style.padding;
        let mut y = bounds.y + style.padding;
        let (_, tm) = self.font.rasterize_text(&title, style.title_size, None);
        if let Some(&tn) = aux.first() {
            scene.set_bounds(tn, Rect::new(x, y, tm.width as f32, tm.height as f32));
            scene.set_fill(tn, style.title_fg);
            scene.set_mask(tn, Some(AlphaMask::new(tm.width, tm.height, tm.pixels)));
            scene.set_visible(tn, true);
        }
        y += tm.height as f32 + 6.0;
        let mut value_line = String::new();
        if let Some(p) = &prefix {
            value_line.push_str(p);
        }
        value_line.push_str(&value);
        if let Some(s) = &suffix {
            value_line.push_str(s);
        }
        let (_, vm) = self.font.rasterize_text(&value_line, style.value_size, None);
        if let Some(&vn) = aux.get(1) {
            scene.set_bounds(vn, Rect::new(x, y, vm.width as f32, vm.height as f32));
            scene.set_fill(vn, style.value_fg);
            scene.set_mask(vn, Some(AlphaMask::new(vm.width, vm.height, vm.pixels)));
            scene.set_visible(vn, true);
        }
        y += vm.height as f32 + 4.0;
        if let Some(&hn) = aux.get(2) {
            if let Some(h) = &hint {
                let (_, hm) = self.font.rasterize_text(h, style.hint_size, None);
                scene.set_bounds(hn, Rect::new(x, y, hm.width as f32, hm.height as f32));
                scene.set_fill(hn, style.hint_fg);
                scene.set_mask(hn, Some(AlphaMask::new(hm.width, hm.height, hm.pixels)));
                scene.set_visible(hn, true);
            } else {
                scene.set_visible(hn, false);
            }
        }
    }

    fn paint_result(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Result {
                    status,
                    title,
                    subtitle,
                    primary_action,
                    secondary_action,
                    style,
                },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let status = *status;
        let title = title.clone();
        let subtitle = subtitle.clone();
        let primary_action = primary_action.clone();
        let secondary_action = secondary_action.clone();
        let style = *style;
        let Some(host) = *node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        self.apply_corner_radius(scene, host, true);
        // aux: status, title, subtitle, primary_bg, primary_lab, secondary_bg, secondary_lab
        self.ensure_aux_nodes(scene, host, id, 7);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        let cx = bounds.x + bounds.width * 0.5;
        let mut y = bounds.y + style.padding;
        let status_lab = status.label();
        let (_, sm) = self.font.rasterize_text(status_lab, style.status_size, None);
        if let Some(&sn) = aux.first() {
            scene.set_bounds(
                sn,
                Rect::new(cx - sm.width as f32 * 0.5, y, sm.width as f32, sm.height as f32),
            );
            scene.set_fill(sn, status.color_for_theme(&self.theme));
            scene.set_mask(sn, Some(AlphaMask::new(sm.width, sm.height, sm.pixels)));
            scene.set_visible(sn, true);
        }
        y += sm.height as f32 + 10.0;
        let (_, tm) = self.font.rasterize_text(&title, style.title_size, None);
        if let Some(&tn) = aux.get(1) {
            scene.set_bounds(
                tn,
                Rect::new(cx - tm.width as f32 * 0.5, y, tm.width as f32, tm.height as f32),
            );
            scene.set_fill(tn, style.title_fg);
            scene.set_mask(tn, Some(AlphaMask::new(tm.width, tm.height, tm.pixels)));
            scene.set_visible(tn, true);
        }
        y += tm.height as f32 + 8.0;
        if let Some(&sub_n) = aux.get(2) {
            if let Some(sub) = &subtitle {
                let (_, dm) = self.font.rasterize_text(sub, style.subtitle_size, None);
                scene.set_bounds(
                    sub_n,
                    Rect::new(cx - dm.width as f32 * 0.5, y, dm.width as f32, dm.height as f32),
                );
                scene.set_fill(sub_n, style.subtitle_fg);
                scene.set_mask(sub_n, Some(AlphaMask::new(dm.width, dm.height, dm.pixels)));
                scene.set_visible(sub_n, true);
            } else {
                scene.set_visible(sub_n, false);
            }
        }
        // Action buttons bottom-right (match pointer_result layout).
        let btn_y = bounds.y + bounds.height - style.padding - style.action_height;
        let btn_h = style.action_height;
        let mut cursor = bounds.x + bounds.width - style.padding;
        // Hide all action nodes first
        for i in 3..7 {
            if let Some(&n) = aux.get(i) {
                scene.set_visible(n, false);
            }
        }
        if let Some(label) = &primary_action {
            let (tw, _) = self.font.measure_line(label, style.action_size);
            let bw = (tw + 24.0).max(72.0);
            let bx = cursor - bw;
            if let Some(&bg) = aux.get(3) {
                scene.set_bounds(bg, Rect::new(bx, btn_y, bw, btn_h));
                scene.set_fill(bg, style.primary_bg);
                scene.set_mask(bg, None);
                scene.set_visible(bg, true);
                self.apply_corner_radius(scene, bg, false);
            }
            let (_, lm) = self.font.rasterize_text(label, style.action_size, None);
            if let Some(&lab) = aux.get(4) {
                scene.set_bounds(
                    lab,
                    Rect::new(
                        bx + ((bw - lm.width as f32) * 0.5).max(0.0),
                        btn_y + ((btn_h - lm.height as f32) * 0.5).max(0.0),
                        lm.width as f32,
                        lm.height as f32,
                    ),
                );
                scene.set_fill(lab, style.primary_fg);
                scene.set_mask(lab, Some(AlphaMask::new(lm.width, lm.height, lm.pixels)));
                scene.set_visible(lab, true);
            }
            cursor = bx - style.gap;
        }
        if let Some(label) = &secondary_action {
            let (tw, _) = self.font.measure_line(label, style.action_size);
            let bw = (tw + 24.0).max(72.0);
            let bx = cursor - bw;
            if let Some(&bg) = aux.get(5) {
                scene.set_bounds(bg, Rect::new(bx, btn_y, bw, btn_h));
                scene.set_fill(bg, style.secondary_bg);
                scene.set_mask(bg, None);
                scene.set_visible(bg, true);
                self.apply_corner_radius(scene, bg, false);
            }
            let (_, lm) = self.font.rasterize_text(label, style.action_size, None);
            if let Some(&lab) = aux.get(6) {
                scene.set_bounds(
                    lab,
                    Rect::new(
                        bx + ((bw - lm.width as f32) * 0.5).max(0.0),
                        btn_y + ((btn_h - lm.height as f32) * 0.5).max(0.0),
                        lm.width as f32,
                        lm.height as f32,
                    ),
                );
                scene.set_fill(lab, style.secondary_fg);
                scene.set_mask(lab, Some(AlphaMask::new(lm.width, lm.height, lm.pixels)));
                scene.set_visible(lab, true);
            }
        }
    }


    fn paint_popover_trigger(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Popover { trigger, style, .. },
            node,
            label_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let trigger = trigger.clone();
        let style = *style;
        let Some(host) = *node else { return };
        let Some(lab_n) = *label_node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.trigger_bg);
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        self.apply_corner_radius(scene, host, false);
        let (_, mask) = self.font.rasterize_text(&trigger, style.size, None);
        scene.set_bounds(
            lab_n,
            Rect::new(
                bounds.x + ((bounds.width - mask.width as f32) * 0.5).max(0.0),
                bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0),
                mask.width as f32,
                mask.height as f32,
            ),
        );
        scene.set_fill(lab_n, style.trigger_fg);
        scene.set_mask(lab_n, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
        scene.set_visible(lab_n, true);
    }

    fn paint_popconfirm_trigger(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Popconfirm { trigger, style, .. },
            node,
            label_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let trigger = trigger.clone();
        let style = *style;
        let Some(host) = *node else { return };
        let Some(lab_n) = *label_node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.trigger_bg);
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        self.apply_corner_radius(scene, host, false);
        let (_, mask) = self.font.rasterize_text(&trigger, style.size, None);
        scene.set_bounds(
            lab_n,
            Rect::new(
                bounds.x + ((bounds.width - mask.width as f32) * 0.5).max(0.0),
                bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0),
                mask.width as f32,
                mask.height as f32,
            ),
        );
        scene.set_fill(lab_n, style.trigger_fg);
        scene.set_mask(lab_n, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
        scene.set_visible(lab_n, true);
    }

    fn paint_popover_popup(&mut self, scene: &mut Scene, id: WidgetId) {
        let (title, body, style, host_bounds) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::Popover {
                        title,
                        body,
                        style,
                        open: true,
                        ..
                    },
                node,
                ..
            }) => {
                let b = node
                    .and_then(|n| scene.get(n).map(|n| n.bounds))
                    .unwrap_or_else(Rect::empty);
                (title.clone(), body.clone(), *style, b)
            }
            _ => return,
        };
        if host_bounds.is_empty() {
            return;
        }
        let title_h = if let Some(ref ti) = title {
            if ti.is_empty() {
                0.0
            } else {
                let (_, th) = self.font.measure_line(ti, style.title_size);
                th + 4.0
            }
        } else {
            0.0
        };
        let (bw, bh) = self.font.measure_line(&body, style.body_size);
        let content_w = bw.max(
            title
                .as_ref()
                .map(|ti| self.font.measure_line(ti, style.title_size).0)
                .unwrap_or(0.0),
        );
        let panel_w = (content_w + style.padding * 2.0)
            .min(style.max_width)
            .max(80.0);
        let panel_h = style.padding * 2.0 + title_h + bh;
        let mut px = host_bounds.x;
        let py = host_bounds.y + host_bounds.height + style.gap;
        let (vw, _) = self.viewport;
        if px + panel_w > vw {
            px = (vw - panel_w).max(0.0);
        }
        let panel = Rect::new(px, py, panel_w, panel_h);
        // aux: 0=panel, 1=title, 2=body
        self.ensure_overlay_aux(scene, id, 3);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        if aux.len() < 3 {
            return;
        }
        scene.set_bounds(aux[0], panel);
        scene.set_fill(aux[0], style.popup_bg);
        scene.set_mask(aux[0], None);
        scene.set_visible(aux[0], true);
        self.apply_corner_radius(scene, aux[0], true);
        self.node_to_widget.insert(aux[0], id);

        let mut cy = panel.y + style.padding;
        if let Some(ref ti) = title {
            if !ti.is_empty() {
                let (_, tm) = self.font.rasterize_text(ti, style.title_size, None);
                scene.set_bounds(
                    aux[1],
                    Rect::new(panel.x + style.padding, cy, tm.width as f32, tm.height as f32),
                );
                scene.set_fill(aux[1], style.title_fg);
                scene.set_mask(aux[1], Some(AlphaMask::new(tm.width, tm.height, tm.pixels)));
                scene.set_visible(aux[1], true);
                cy += tm.height as f32 + 4.0;
            } else {
                scene.set_visible(aux[1], false);
            }
        } else {
            scene.set_visible(aux[1], false);
        }
        let (_, bm) = self.font.rasterize_text(&body, style.body_size, None);
        scene.set_bounds(
            aux[2],
            Rect::new(panel.x + style.padding, cy, bm.width as f32, bm.height as f32),
        );
        scene.set_fill(aux[2], style.body_fg);
        scene.set_mask(aux[2], Some(AlphaMask::new(bm.width, bm.height, bm.pixels)));
        scene.set_visible(aux[2], true);
        if let Some(h) = self.overlay_host {
            scene.raise(h);
        }
    }

    fn paint_popconfirm_popup(&mut self, scene: &mut Scene, id: WidgetId) {
        let (message, ok_label, cancel_label, style, host_bounds) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::Popconfirm {
                        message,
                        ok_label,
                        cancel_label,
                        style,
                        open: true,
                        ..
                    },
                node,
                ..
            }) => {
                let b = node
                    .and_then(|n| scene.get(n).map(|n| n.bounds))
                    .unwrap_or_else(Rect::empty);
                (
                    message.clone(),
                    ok_label.clone(),
                    cancel_label.clone(),
                    *style,
                    b,
                )
            }
            _ => return,
        };
        if host_bounds.is_empty() {
            return;
        }
        let (mw, mh) = self.font.measure_line(&message, style.size);
        let (ok_w, _) = self.font.measure_line(&ok_label, style.size);
        let (cancel_w, _) = self.font.measure_line(&cancel_label, style.size);
        let ok_bw = (ok_w + 20.0).max(56.0);
        let cancel_bw = (cancel_w + 20.0).max(56.0);
        let actions_w = ok_bw + cancel_bw + style.action_gap;
        let panel_w = (mw.max(actions_w) + style.padding * 2.0)
            .min(style.max_width)
            .max(120.0);
        let panel_h = style.padding * 2.0 + mh + 8.0 + style.action_height;
        let mut px = host_bounds.x;
        let py = host_bounds.y + host_bounds.height + style.gap;
        let (vw, _) = self.viewport;
        if px + panel_w > vw {
            px = (vw - panel_w).max(0.0);
        }
        let panel = Rect::new(px, py, panel_w, panel_h);
        // aux: 0=panel, 1=message, 2=ok btn, 3=ok label, 4=cancel btn, 5=cancel label
        self.ensure_overlay_aux(scene, id, 6);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        if aux.len() < 6 {
            return;
        }
        scene.set_bounds(aux[0], panel);
        scene.set_fill(aux[0], style.popup_bg);
        scene.set_mask(aux[0], None);
        scene.set_visible(aux[0], true);
        self.apply_corner_radius(scene, aux[0], true);
        self.node_to_widget.insert(aux[0], id);

        let (_, mm) = self.font.rasterize_text(&message, style.size, None);
        scene.set_bounds(
            aux[1],
            Rect::new(
                panel.x + style.padding,
                panel.y + style.padding,
                mm.width as f32,
                mm.height as f32,
            ),
        );
        scene.set_fill(aux[1], style.message_fg);
        scene.set_mask(aux[1], Some(AlphaMask::new(mm.width, mm.height, mm.pixels)));
        scene.set_visible(aux[1], true);

        let btn_y = panel.y + panel.height - style.padding - style.action_height;
        let mut cursor = panel.x + panel.width - style.padding;

        // OK (right)
        let ok_x = cursor - ok_bw;
        scene.set_bounds(aux[2], Rect::new(ok_x, btn_y, ok_bw, style.action_height));
        scene.set_fill(aux[2], style.ok_bg);
        scene.set_mask(aux[2], None);
        scene.set_visible(aux[2], true);
        self.apply_corner_radius(scene, aux[2], false);
        self.node_to_widget.insert(aux[2], id);
        let (_, olm) = self.font.rasterize_text(&ok_label, style.size, None);
        scene.set_bounds(
            aux[3],
            Rect::new(
                ok_x + ((ok_bw - olm.width as f32) * 0.5).max(0.0),
                btn_y + ((style.action_height - olm.height as f32) * 0.5).max(0.0),
                olm.width as f32,
                olm.height as f32,
            ),
        );
        scene.set_fill(aux[3], style.ok_fg);
        scene.set_mask(aux[3], Some(AlphaMask::new(olm.width, olm.height, olm.pixels)));
        scene.set_visible(aux[3], true);
        cursor = ok_x - style.action_gap;

        // Cancel (left of OK)
        let cancel_x = cursor - cancel_bw;
        scene.set_bounds(
            aux[4],
            Rect::new(cancel_x, btn_y, cancel_bw, style.action_height),
        );
        scene.set_fill(aux[4], style.cancel_bg);
        scene.set_mask(aux[4], None);
        scene.set_visible(aux[4], true);
        self.apply_corner_radius(scene, aux[4], false);
        self.node_to_widget.insert(aux[4], id);
        let (_, clm) = self.font.rasterize_text(&cancel_label, style.size, None);
        scene.set_bounds(
            aux[5],
            Rect::new(
                cancel_x + ((cancel_bw - clm.width as f32) * 0.5).max(0.0),
                btn_y + ((style.action_height - clm.height as f32) * 0.5).max(0.0),
                clm.width as f32,
                clm.height as f32,
            ),
        );
        scene.set_fill(aux[5], style.cancel_fg);
        scene.set_mask(aux[5], Some(AlphaMask::new(clm.width, clm.height, clm.pixels)));
        scene.set_visible(aux[5], true);

        if let Some(h) = self.overlay_host {
            scene.raise(h);
        }
    }



    fn layout_grid(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Grid {
                columns,
                style,
                children,
            },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let columns = (*columns).max(1);
        let style = *style;
        let children = children.clone();
        let host = *node;
        let (ox, oy) = if let Some(n) = host {
            if self.roots.contains(&id) {
                (style.x, style.y)
            } else {
                scene
                    .get(n)
                    .map(|nn| (nn.bounds.x, nn.bounds.y))
                    .unwrap_or((style.x, style.y))
            }
        } else {
            (style.x, style.y)
        };
        let (w, h) = if let Some(n) = host {
            if self.roots.contains(&id) {
                (style.width, style.height)
            } else {
                scene
                    .get(n)
                    .map(|nn| (nn.bounds.width.max(1.0), nn.bounds.height.max(1.0)))
                    .unwrap_or((style.width, style.height))
            }
        } else {
            (style.width, style.height)
        };
        if let Some(n) = host {
            scene.set_bounds(n, Rect::new(ox, oy, w, h));
            scene.set_visible(n, style.background.is_some());
        }
        let pad = style.padding;
        let gap = style.gap;
        let inner_w = (w - pad * 2.0).max(0.0);
        let col_w = if columns == 0 {
            inner_w
        } else {
            (inner_w - gap * (columns.saturating_sub(1) as f32)).max(0.0) / columns as f32
        };
        let measures: Vec<_> = children.iter().map(|c| self.measure_widget(*c)).collect();
        let mut row_heights: Vec<f32> = Vec::new();
        for chunk in measures.chunks(columns) {
            let rh = chunk.iter().map(|m| m.height).fold(0.0f32, f32::max);
            row_heights.push(rh.max(1.0));
        }
        let mut cy = oy + pad;
        for (row_i, chunk) in children.chunks(columns).enumerate() {
            let rh = row_heights.get(row_i).copied().unwrap_or(1.0);
            for (col, &child_id) in chunk.iter().enumerate() {
                let cx = ox + pad + col as f32 * (col_w + gap);
                let m = measures
                    .get(row_i * columns + col)
                    .copied()
                    .unwrap_or(ChildMeasure::new(col_w, rh));
                let rect = Rect::new(cx, cy, col_w.min(m.width.max(col_w)), rh.max(m.height));
                // Prefer cell box so nested flex gets the column width.
                let cell = Rect::new(cx, cy, col_w, rh);
                self.place_child(scene, child_id, cell);
                let _ = rect;
            }
            cy += rh + gap;
        }
    }

    fn pointer_timeline(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let Some(Widget {
            kind: Kind::Timeline { items, style, .. },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return false;
        };
        let Some(host) = *node else { return false };
        let bounds = match scene.get(host) {
            Some(n) => n.bounds,
            None => return false,
        };
        let pad = style.padding;
        let ih = style.item_height;
        let rel_y = y - (bounds.y + pad);
        if rel_y < 0.0 || x < bounds.x || x > bounds.x + bounds.width {
            return false;
        }
        let idx = (rel_y / ih).floor() as isize;
        if idx < 0 || idx as usize >= items.len() {
            return false;
        }
        let idx = idx as usize;
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Timeline { selected, .. } = &mut w.kind {
                *selected = Some(idx);
            }
        }
        self.pending_timeline_select = Some((id, idx));
        true
    }

    fn pointer_calendar(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let Some(Widget {
            kind:
                Kind::Calendar {
                    year,
                    month,
                    style,
                    ..
                },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return false;
        };
        let year = *year;
        let month = *month;
        let style = *style;
        let Some(host) = *node else { return false };
        let bounds = match scene.get(host) {
            Some(n) => n.bounds,
            None => return false,
        };
        let pad = style.padding;
        let header_h = 28.0;
        let weekday_h = 18.0;
        // Prev / next hit zones in header
        let prev = Rect::new(bounds.x + pad, bounds.y + pad, 28.0, header_h - 4.0);
        let next = Rect::new(
            bounds.x + bounds.width - pad - 28.0,
            bounds.y + pad,
            28.0,
            header_h - 4.0,
        );
        if prev.contains(x, y) {
            return self.calendar_shift_month(id, -1);
        }
        if next.contains(x, y) {
            return self.calendar_shift_month(id, 1);
        }
        let grid_top = bounds.y + pad + header_h + weekday_h;
        let cell = style.cell;
        let dim = DateTimeValue::days_in_month(year, month);
        let start = CalendarDate::weekday_sun0(year, month, 1) as usize;
        for d in 1..=dim {
            let slot = start + (d as usize - 1);
            let col = (slot % 7) as f32;
            let row = (slot / 7) as f32;
            let r = Rect::new(
                bounds.x + pad + col * cell,
                grid_top + row * cell,
                cell,
                cell,
            );
            if r.contains(x, y) {
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Calendar { selected_day, .. } = &mut w.kind {
                        *selected_day = d;
                    }
                }
                let date = CalendarDate::new(year, month, d);
                self.pending_calendar_change = Some((id, date));
                return true;
            }
        }
        false
    }

    fn calendar_shift_month(&mut self, id: WidgetId, delta: i32) -> bool {
        let Some(Widget {
            kind: Kind::Calendar {
                year,
                month,
                selected_day,
                ..
            },
            ..
        }) = self.widgets.get(&id)
        else {
            return false;
        };
        let mut y = *year;
        let mut m = *month as i32 + delta;
        while m < 1 {
            m += 12;
            y -= 1;
        }
        while m > 12 {
            m -= 12;
            y += 1;
        }
        let day = *selected_day;
        let date = CalendarDate::new(y, m as u8, day);
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Calendar {
                year,
                month,
                selected_day,
                ..
            } = &mut w.kind
            {
                *year = date.year;
                *month = date.month;
                *selected_day = date.day;
            }
        }
        self.pending_calendar_change = Some((id, date));
        true
    }

    fn key_calendar(&mut self, id: WidgetId, key: UiKey) -> bool {
        let Some(Widget {
            kind: Kind::Calendar {
                year,
                month,
                selected_day,
                ..
            },
            ..
        }) = self.widgets.get(&id)
        else {
            return false;
        };
        let mut date = CalendarDate::new(*year, *month, *selected_day);
        let dim = DateTimeValue::days_in_month(date.year, date.month) as i32;
        match key {
            UiKey::Left => {
                if date.day > 1 {
                    date.day -= 1;
                } else {
                    return self.calendar_shift_month(id, -1);
                }
            }
            UiKey::Right => {
                if (date.day as i32) < dim {
                    date.day += 1;
                } else {
                    return self.calendar_shift_month(id, 1);
                }
            }
            UiKey::Up => {
                let nd = date.day as i32 - 7;
                if nd >= 1 {
                    date.day = nd as u8;
                } else {
                    return self.calendar_shift_month(id, -1);
                }
            }
            UiKey::Down => {
                let nd = date.day as i32 + 7;
                if nd <= dim {
                    date.day = nd as u8;
                } else {
                    return self.calendar_shift_month(id, 1);
                }
            }
            _ => return false,
        }
        date = date.clamp();
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Calendar {
                year,
                month,
                selected_day,
                ..
            } = &mut w.kind
            {
                *year = date.year;
                *month = date.month;
                *selected_day = date.day;
            }
        }
        self.pending_calendar_change = Some((id, date));
        true
    }

    fn paint_timeline(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Timeline {
                items,
                selected,
                style,
            },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let items = items.clone();
        let selected = *selected;
        let style = *style;
        let Some(host) = *node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        self.apply_corner_radius(scene, host, true);
        // aux: per item [dot, time_lab, content_lab] + optional rail segments
        let n = items.len();
        self.ensure_aux_nodes(scene, host, id, n * 3 + 1);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        let pad = style.padding;
        let rail_x = bounds.x + pad + style.time_width + 6.0;
        if let Some(&rail) = aux.first() {
            let rh = (n as f32 * style.item_height).max(4.0);
            scene.set_bounds(rail, Rect::new(rail_x, bounds.y + pad, 2.0, rh));
            scene.set_fill(rail, style.rail);
            scene.set_mask(rail, None);
            scene.set_visible(rail, n > 0);
        }
        for (i, item) in items.iter().enumerate() {
            let y = bounds.y + pad + i as f32 * style.item_height;
            let dot_c = item
                .color_rgb
                .map(|rgb| Color::rgb(((rgb >> 16) & 0xff) as u8, ((rgb >> 8) & 0xff) as u8, (rgb & 0xff) as u8))
                .unwrap_or(style.rail);
            if let Some(&dot) = aux.get(1 + i * 3) {
                scene.set_bounds(dot, Rect::new(rail_x - 3.0, y + style.item_height * 0.35, 8.0, 8.0));
                scene.set_fill(dot, if selected == Some(i) {
                    self.config.accent.unwrap_or(dot_c)
                } else {
                    dot_c
                });
                scene.set_mask(dot, None);
                scene.set_visible(dot, true);
                scene.set_corner_radius(dot, 4.0);
            }
            let (_, tm) = self.font.rasterize_text(&item.time, style.size, None);
            if let Some(&lab) = aux.get(2 + i * 3) {
                scene.set_bounds(
                    lab,
                    Rect::new(
                        bounds.x + pad,
                        y + ((style.item_height - tm.height as f32) * 0.5).max(0.0),
                        tm.width as f32,
                        tm.height as f32,
                    ),
                );
                scene.set_fill(lab, style.time_fg);
                scene.set_mask(lab, Some(AlphaMask::new(tm.width, tm.height, tm.pixels)));
                scene.set_visible(lab, true);
            }
            let (_, cm) = self.font.rasterize_text(&item.content, style.size, None);
            if let Some(&lab) = aux.get(3 + i * 3) {
                scene.set_bounds(
                    lab,
                    Rect::new(
                        rail_x + 12.0,
                        y + ((style.item_height - cm.height as f32) * 0.5).max(0.0),
                        cm.width as f32,
                        cm.height as f32,
                    ),
                );
                scene.set_fill(lab, style.content_fg);
                scene.set_mask(lab, Some(AlphaMask::new(cm.width, cm.height, cm.pixels)));
                scene.set_visible(lab, true);
            }
        }
    }

    fn paint_calendar(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Calendar {
                    year,
                    month,
                    selected_day,
                    style,
                },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let year = *year;
        let month = *month;
        let selected_day = *selected_day;
        let mut style = *style;
        if let Some(a) = self.config.accent {
            style.accent = a;
        }
        let Some(host) = *node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        self.apply_corner_radius(scene, host, true);
        // aux: header_bg, prev, title, next, 7 weekday labels, 42 day cells (bg+lab pairs = 84) — keep lighter:
        // 0 header_bg, 1 prev, 2 title, 3 next, 4..=10 weekday, 11..=52 day labels (42)
        let needed = 4 + 7 + 42;
        self.ensure_aux_nodes(scene, host, id, needed);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        let pad = style.padding;
        let header_h = 28.0;
        let weekday_h = 18.0;
        if let Some(&hb) = aux.first() {
            scene.set_bounds(
                hb,
                Rect::new(bounds.x + pad, bounds.y + pad, bounds.width - pad * 2.0, header_h),
            );
            scene.set_fill(hb, style.header_bg);
            scene.set_mask(hb, None);
            scene.set_visible(hb, true);
            self.apply_corner_radius(scene, hb, false);
        }
        let prev_s = "‹";
        let (_, pm) = self.font.rasterize_text(prev_s, style.size + 2.0, None);
        if let Some(&n) = aux.get(1) {
            scene.set_bounds(
                n,
                Rect::new(
                    bounds.x + pad + 6.0,
                    bounds.y + pad + ((header_h - pm.height as f32) * 0.5).max(0.0),
                    pm.width as f32,
                    pm.height as f32,
                ),
            );
            scene.set_fill(n, style.fg);
            scene.set_mask(n, Some(AlphaMask::new(pm.width, pm.height, pm.pixels)));
            scene.set_visible(n, true);
        }
        let title = format!("{:04}-{:02}", year, month);
        let (_, tm) = self.font.rasterize_text(&title, style.size, None);
        if let Some(&n) = aux.get(2) {
            scene.set_bounds(
                n,
                Rect::new(
                    bounds.x + ((bounds.width - tm.width as f32) * 0.5).max(0.0),
                    bounds.y + pad + ((header_h - tm.height as f32) * 0.5).max(0.0),
                    tm.width as f32,
                    tm.height as f32,
                ),
            );
            scene.set_fill(n, style.fg);
            scene.set_mask(n, Some(AlphaMask::new(tm.width, tm.height, tm.pixels)));
            scene.set_visible(n, true);
        }
        let next_s = "›";
        let (_, nm) = self.font.rasterize_text(next_s, style.size + 2.0, None);
        if let Some(&n) = aux.get(3) {
            scene.set_bounds(
                n,
                Rect::new(
                    bounds.x + bounds.width - pad - 6.0 - nm.width as f32,
                    bounds.y + pad + ((header_h - nm.height as f32) * 0.5).max(0.0),
                    nm.width as f32,
                    nm.height as f32,
                ),
            );
            scene.set_fill(n, style.fg);
            scene.set_mask(n, Some(AlphaMask::new(nm.width, nm.height, nm.pixels)));
            scene.set_visible(n, true);
        }
        let weekdays = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
        let wd_top = bounds.y + pad + header_h;
        for (i, wlab) in weekdays.iter().enumerate() {
            let (_, wm) = self.font.rasterize_text(wlab, style.size - 1.0, None);
            if let Some(&n) = aux.get(4 + i) {
                let cx = bounds.x + pad + i as f32 * style.cell;
                scene.set_bounds(
                    n,
                    Rect::new(
                        cx + ((style.cell - wm.width as f32) * 0.5).max(0.0),
                        wd_top + ((weekday_h - wm.height as f32) * 0.5).max(0.0),
                        wm.width as f32,
                        wm.height as f32,
                    ),
                );
                scene.set_fill(n, Color::rgb(0x90, 0x98, 0xa4));
                scene.set_mask(n, Some(AlphaMask::new(wm.width, wm.height, wm.pixels)));
                scene.set_visible(n, true);
            }
        }
        let grid_top = wd_top + weekday_h;
        let dim = DateTimeValue::days_in_month(year, month) as usize;
        let start = CalendarDate::weekday_sun0(year, month, 1) as usize;
        for slot in 0..42 {
            let aux_i = 11 + slot;
            let Some(&n) = aux.get(aux_i) else { continue };
            if slot < start || slot >= start + dim {
                scene.set_visible(n, false);
                continue;
            }
            let d = (slot - start + 1) as u8;
            let col = (slot % 7) as f32;
            let row = (slot / 7) as f32;
            let label = format!("{d}");
            let (_, dm) = self.font.rasterize_text(&label, style.size, None);
            let cx = bounds.x + pad + col * style.cell;
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
                if d == selected_day {
                    style.accent
                } else {
                    style.fg
                },
            );
            scene.set_mask(n, Some(AlphaMask::new(dm.width, dm.height, dm.pixels)));
            scene.set_visible(n, true);
        }
    }

    fn paint_image(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Image {
                    px_w,
                    px_h,
                    pixels,
                    caption,
                    style,
                    view,
                },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let view = *view;
        let px_w = *px_w;
        let px_h = *px_h;
        let pixels = pixels.clone();
        let caption = caption.clone();
        let style = *style;
        let Some(host) = *node else { return };
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        self.apply_corner_radius(scene, host, true);
        let cap_h = if caption.is_empty() {
            0.0
        } else {
            18.0f32.min(bounds.height.max(0.0))
        };
        let image_h = (bounds.height - cap_h).max(0.0);
        self.ensure_aux_nodes(scene, host, id, 2);
        let aux = self
            .widgets
            .get(&id)
            .map(|w| w.aux.clone())
            .unwrap_or_default();
        let image_node = aux[0];
        scene.set_visible(image_node, false);
        scene.set_image(image_node, None);
        if let Some(image) = pixels.and_then(|px| RgbaImage::new(px_w, px_h, px)) {
            if bounds.width > 0.0 && image_h > 0.0 {
                let scale = view.scale.unwrap_or_else(|| (bounds.width / px_w as f32).min(image_h / px_h as f32));
                let w = px_w as f32 * scale;
                let h = px_h as f32 * scale;
                scene.set_bounds(
                    image_node,
                    Rect::new(
                        bounds.x + (bounds.width - w) * 0.5 - view.offset_x.clamp(-((w - bounds.width) * 0.5).max(0.0), ((w - bounds.width) * 0.5).max(0.0)),
                        bounds.y + (image_h - h) * 0.5 - view.offset_y.clamp(-((h - image_h) * 0.5).max(0.0), ((h - image_h) * 0.5).max(0.0)),
                        w,
                        h,
                    ),
                );
                scene.set_clip(
                    image_node,
                    Some(Rect::new(bounds.x, bounds.y, bounds.width, image_h)),
                );
                scene.set_image(image_node, Some(image));
                scene.set_visible(image_node, true);
            }
        }
        let cap_node = aux[1];
        if !caption.is_empty() && cap_h > 0.0 {
            let (_, cm) = self.font.rasterize_text(&caption, style.caption_size, None);
            scene.set_bounds(
                cap_node,
                Rect::new(
                    bounds.x + 4.0,
                    bounds.y + image_h + ((cap_h - cm.height as f32) * 0.5).max(0.0),
                    cm.width as f32,
                    cm.height as f32,
                ),
            );
            scene.set_clip(
                cap_node,
                Some(Rect::new(bounds.x, bounds.y + image_h, bounds.width, cap_h)),
            );
            scene.set_fill(cap_node, style.caption_fg);
            scene.set_mask(
                cap_node,
                Some(AlphaMask::new(cm.width, cm.height, cm.pixels)),
            );
            scene.set_visible(cap_node, true);
        } else {
            scene.set_visible(cap_node, false);
        }
    }


    fn paint_upload(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Upload {
                    path,
                    placeholder,
                    browse_label,
                    clear_label,
                    style,
                },
            node,
            label_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let path = path.clone();
        let placeholder = placeholder.clone();
        let browse_label = browse_label.clone();
        let clear_label = clear_label.clone();
        let style = *style;
        let Some(host) = *node else { return };
        let label_node = *label_node;
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, style.bg);
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        self.apply_corner_radius(scene, host, false);

        let (browse, clear) = Self::upload_button_rects(bounds, style, path.is_some());
        let path_right = clear
            .map(|c| c.x - style.gap)
            .unwrap_or(browse.x - style.gap);
        let path_w = (path_right - bounds.x - style.padding).max(1.0);

        let (text, fg) = match &path {
            Some(p) if !p.is_empty() => (p.clone(), style.fg),
            _ => (placeholder, style.placeholder_fg),
        };
        let (_, mask) = self.font.rasterize_text(&text, style.size, None);
        if let Some(lid) = label_node {
            let lx = bounds.x + style.padding;
            let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
            scene.set_bounds(
                lid,
                Rect::new(lx, ly, mask.width as f32, mask.height as f32),
            );
            scene.set_fill(lid, fg);
            scene.set_mask(lid, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(lid, true);
        }
        let _ = path_w;

        // aux: 0=browse bg, 1=browse label, 2=clear bg, 3=clear label
        self.ensure_aux_nodes(scene, host, id, 4);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        if let Some(&bb) = aux.first() {
            scene.set_bounds(bb, browse);
            scene.set_fill(bb, style.browse_bg);
            scene.set_mask(bb, None);
            scene.set_visible(bb, true);
            self.apply_corner_radius(scene, bb, false);
            self.node_to_widget.insert(bb, id);
        }
        let (_, bm) = self.font.rasterize_text(&browse_label, style.size, None);
        if let Some(&bl) = aux.get(1) {
            scene.set_bounds(
                bl,
                Rect::new(
                    browse.x + ((browse.width - bm.width as f32) * 0.5).max(0.0),
                    browse.y + ((browse.height - bm.height as f32) * 0.5).max(0.0),
                    bm.width as f32,
                    bm.height as f32,
                ),
            );
            scene.set_fill(bl, style.browse_fg);
            scene.set_mask(bl, Some(AlphaMask::new(bm.width, bm.height, bm.pixels)));
            scene.set_visible(bl, true);
        }
        if let Some(cr) = clear {
            if let Some(&cb) = aux.get(2) {
                scene.set_bounds(cb, cr);
                scene.set_fill(cb, style.clear_bg);
                scene.set_mask(cb, None);
                scene.set_visible(cb, true);
                self.apply_corner_radius(scene, cb, false);
                self.node_to_widget.insert(cb, id);
            }
            let (_, cm) = self.font.rasterize_text(&clear_label, style.size, None);
            if let Some(&cl) = aux.get(3) {
                scene.set_bounds(
                    cl,
                    Rect::new(
                        cr.x + ((cr.width - cm.width as f32) * 0.5).max(0.0),
                        cr.y + ((cr.height - cm.height as f32) * 0.5).max(0.0),
                        cm.width as f32,
                        cm.height as f32,
                    ),
                );
                scene.set_fill(cl, style.clear_fg);
                scene.set_mask(cl, Some(AlphaMask::new(cm.width, cm.height, cm.pixels)));
                scene.set_visible(cl, true);
            }
        } else {
            for i in 2..4 {
                if let Some(&n) = aux.get(i) {
                    scene.set_visible(n, false);
                }
            }
        }
    }

    fn paint_color_picker(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::ColorPicker {
                    value: _,
                    hex,
                    selected_swatch,
                    presets,
                    style,
                },
            node,
            label_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let hex = hex.clone();
        let selected_swatch = *selected_swatch;
        let presets = presets.clone();
        let style = *style;
        let Some(host) = *node else { return };
        let label_node = *label_node;
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        if style.bg.a > 0 {
            scene.set_fill(host, style.bg);
            scene.set_mask(host, None);
            scene.set_visible(host, true);
            self.apply_corner_radius(scene, host, false);
        } else {
            scene.set_fill(host, Color::rgba(0, 0, 0, 0));
            scene.set_mask(host, Some(AlphaMask::empty()));
            scene.set_visible(host, true);
        }

        // aux: one node per swatch + hex bg
        let needed = presets.len() + 1;
        self.ensure_aux_nodes(scene, host, id, needed);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        let mut sx = bounds.x + style.padding;
        let sy = bounds.y + ((bounds.height - style.swatch_size) * 0.5).max(0.0);
        for (i, c) in presets.iter().enumerate() {
            if let Some(&n) = aux.get(i) {
                let r = Rect::new(sx, sy, style.swatch_size, style.swatch_size);
                scene.set_bounds(n, r);
                scene.set_fill(n, *c);
                scene.set_mask(n, None);
                scene.set_visible(n, true);
                self.apply_corner_radius(scene, n, false);
                self.node_to_widget.insert(n, id);
                if selected_swatch == Some(i) {
                    // draw selection as slightly inset darker — Soft-friendly: raise outline via smaller inset fill is hard;
                    // use host chrome: leave as selected fill (border simulated by padding gap already).
                    let _ = style.selected_border;
                }
            }
            sx += style.swatch_size + style.swatch_gap;
        }
        let hex_x = sx + style.swatch_gap;
        let hex_r = Rect::new(
            hex_x,
            bounds.y + 2.0,
            style.hex_width.min((bounds.x + bounds.width - hex_x - style.padding).max(40.0)),
            (bounds.height - 4.0).max(1.0),
        );
        if let Some(&hb) = aux.get(presets.len()) {
            scene.set_bounds(hb, hex_r);
            scene.set_fill(hb, style.hex_bg);
            scene.set_mask(hb, None);
            scene.set_visible(hb, true);
            self.apply_corner_radius(scene, hb, false);
            self.node_to_widget.insert(hb, id);
        }
        let display = if hex.is_empty() { "#------".to_string() } else { hex };
        let (_, mask) = self.font.rasterize_text(&display, style.size, None);
        if let Some(lid) = label_node {
            scene.set_bounds(
                lid,
                Rect::new(
                    hex_r.x + 6.0,
                    hex_r.y + ((hex_r.height - mask.height as f32) * 0.5).max(0.0),
                    mask.width as f32,
                    mask.height as f32,
                ),
            );
            scene.set_fill(lid, style.fg);
            scene.set_mask(lid, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(lid, true);
        }
    }

    fn paint_autocomplete_field(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Autocomplete {
                    value,
                    caret,
                    placeholder,
                    open,
                    style,
                    ..
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
        let caret = *caret;
        let placeholder = placeholder.clone();
        let open = *open;
        let style = *style;
        let Some(host) = *node else { return };
        let label_node = *label_node;
        let chrome_node = *chrome_node;
        let bounds = scene.get(host).map(|n| n.bounds).unwrap_or_else(Rect::empty);
        scene.set_fill(host, if open { style.selected_bg } else { style.bg });
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        self.apply_corner_radius(scene, host, false);
        let focused = self.focus == Some(id);
        let (text, fg) = if value.is_empty() {
            (placeholder, style.placeholder_fg)
        } else {
            (value.clone(), style.fg)
        };
        let (_, mask) = self.font.rasterize_text(&text, style.size, None);
        let lx = bounds.x + style.padding;
        let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
        if let Some(lid) = label_node {
            scene.set_bounds(lid, Rect::new(lx, ly, mask.width as f32, mask.height as f32));
            scene.set_fill(lid, fg);
            scene.set_mask(lid, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(lid, true);
        }
        if let Some(cid) = chrome_node {
            if focused && !value.is_empty() {
                let prefix: String = value.chars().take(caret).collect();
                let (pw, _) = self.font.measure_line(&prefix, style.size);
                scene.set_bounds(
                    cid,
                    Rect::new(lx + pw, ly, 2.0, mask.height as f32),
                );
                scene.set_fill(cid, style.focus_border);
                scene.set_mask(cid, None);
                scene.set_visible(cid, true);
            } else if focused && value.is_empty() {
                scene.set_bounds(cid, Rect::new(lx, ly, 2.0, style.size.max(12.0)));
                scene.set_fill(cid, style.focus_border);
                scene.set_mask(cid, None);
                scene.set_visible(cid, true);
            } else {
                scene.set_visible(cid, false);
            }
        }
    }

    fn paint_autocomplete_popup(&mut self, scene: &mut Scene, id: WidgetId) {
        let (filtered, highlight, style, host_bounds) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::Autocomplete {
                        open: true,
                        highlight,
                        style,
                        ..
                    },
                node,
                ..
            }) => {
                let b = node
                    .and_then(|n| scene.get(n).map(|n| n.bounds))
                    .unwrap_or_else(Rect::empty);
                let filtered = self.autocomplete_filtered(id);
                (filtered, *highlight, *style, b)
            }
            _ => return,
        };
        if filtered.is_empty() {
            return;
        }
        let n = filtered.len();
        let popup_h = (style.item_height * n as f32).min(style.popup_max_height);
        let panel = Rect::new(
            host_bounds.x,
            host_bounds.y + host_bounds.height,
            host_bounds.width.max(style.width),
            popup_h,
        );
        self.ensure_overlay_aux(scene, id, 1 + n * 2);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        if let Some(&bg) = aux.first() {
            scene.set_bounds(bg, panel);
            scene.set_fill(bg, style.popup_bg);
            scene.set_mask(bg, None);
            scene.set_visible(bg, true);
            self.apply_corner_radius(scene, bg, true);
            self.node_to_widget.insert(bg, id);
        }
        for (i, (_, label)) in filtered.iter().enumerate() {
            let y = panel.y + i as f32 * style.item_height;
            let row = Rect::new(panel.x, y, panel.width, style.item_height);
            let hi = highlight == Some(i);
            if let Some(&rbg) = aux.get(1 + i * 2) {
                scene.set_bounds(rbg, row);
                scene.set_fill(rbg, if hi { style.selected_bg } else { style.popup_bg });
                scene.set_mask(rbg, None);
                scene.set_visible(rbg, true);
                if hi {
                    self.apply_corner_radius(scene, rbg, false);
                }
                self.node_to_widget.insert(rbg, id);
            }
            let (_, mask) = self.font.rasterize_text(label, style.size, None);
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
        self.apply_corner_radius(scene, aux[0], false);
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
        self.apply_corner_radius(scene, aux[2], false);
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
        self.apply_corner_radius(scene, host, true);
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
                self.apply_corner_radius(scene, hbg, false);
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
        self.apply_corner_radius(scene, host, true);
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
                self.apply_corner_radius(scene, chrome, true);
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
        self.apply_corner_radius(scene, node, false);
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
            self.apply_corner_radius(scene, p, true);
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
        self.apply_corner_radius(scene, host, true);

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
                    self.apply_corner_radius(scene, bg, false);
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
                    self.apply_corner_radius(scene, bg, false);
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
        self.apply_corner_radius(scene, host, true);

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
                    self.apply_corner_radius(scene, bg, false);
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
        self.apply_corner_radius(scene, host, true);

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
                    self.apply_corner_radius(scene, bg, false);
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
                        | Some(Kind::Popover { open: false, .. })
                        | Some(Kind::Popconfirm { open: false, .. })
                        | Some(Kind::Autocomplete { open: false, .. })
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
            Some(Kind::Popover { .. }) => self.paint_popover_popup(scene, id),
            Some(Kind::Popconfirm { .. }) => self.paint_popconfirm_popup(scene, id),
            Some(Kind::Autocomplete { .. }) => self.paint_autocomplete_popup(scene, id),
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
        let mut label_w = 0.0f32;
        for it in &items {
            let (tw, _) = self.font.measure_line(&it.label, style.size);
            label_w = label_w.max(tw);
        }
        let popup_w = style
            .popup_width
            .max(host_bounds.width)
            .max(label_w + 28.0)
            .max(style.min_width);
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
            self.apply_corner_radius(scene, bg, true);
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
                if hi {
                    self.apply_corner_radius(scene, rbg, false);
                }
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
        self.apply_corner_radius(scene, node, false);
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
                self.apply_corner_radius(scene, cid, false);
            }
        } else {
            scene.set_bounds(node, Rect::new(bx, by, bw, bh));
            scene.set_fill(node, style.bg);
            scene.set_mask(node, None);
            scene.set_visible(node, true);
            self.apply_corner_radius(scene, node, false);
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
        self.apply_corner_radius(scene, node, false);
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
            self.apply_corner_radius(scene, cid, false);
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
        self.apply_corner_radius(scene, node, false);
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
        self.apply_corner_radius(scene, bg, false);
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
        let (title, body, primary, secondary, tertiary, action_focus, style) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::Modal {
                        title,
                        body,
                        primary,
                        secondary,
                        tertiary,
                        action_focus,
                        open: true,
                        style,
                    },
                ..
            }) => (
                title.clone(),
                body.clone(),
                primary.clone(),
                secondary.clone(),
                tertiary.clone(),
                *action_focus,
                *style,
            ),
            _ => return,
        };
        let (vw, vh) = self.viewport;
        let panel_w = style.width.min(vw - 40.0).max(200.0);
        let action_h = if primary.is_some() || secondary.is_some() || tertiary.is_some() {
            style.action_height + style.padding
        } else {
            0.0
        };
        let (_, body_mask) = self.font.rasterize_text(&body, style.body_size, Some((panel_w - 2.0 * style.padding).max(1.0)));
        let body_th = body_mask.height as f32;
        let panel_h = (style.title_height
            + style.padding * 2.0
            + body_th.max(40.0)
            + action_h)
            .max(style.height)
            .min(vh - 40.0);
        let px = ((vw - panel_w) * 0.5).max(0.0);
        let py = ((vh - panel_h) * 0.5).max(0.0);
        let panel = Rect::new(px, py, panel_w, panel_h);

        // Stable action slots preserve existing primary/secondary hit geometry.
        self.ensure_overlay_aux(scene, id, 12);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();
        if aux.len() < 12 {
            return;
        }
        let scrim = aux[0];
        let panel_n = aux[1];
        let title_bg = aux[2];
        let title_l = aux[3];
        let close_n = aux[4];
        let body_n = aux[5];


        scene.set_bounds(scrim, Rect::new(0.0, 0.0, vw, vh));
        scene.set_fill(scrim, style.scrim);
        scene.set_mask(scrim, None);
        scene.set_visible(scrim, true);

        scene.set_bounds(panel_n, panel);
        scene.set_fill(panel_n, style.panel_bg);
        scene.set_mask(panel_n, None);
        scene.set_visible(panel_n, true);
        self.apply_corner_radius(scene, panel_n, true);

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

        let bm = body_mask;
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

        let mut action_index = 0;
        for (slot, label) in [primary, secondary, tertiary].iter().enumerate() {
            let button = aux[6 + slot * 2];
            let text = aux[7 + slot * 2];
            let Some(label) = label else {
                scene.set_visible(button, false);
                scene.set_visible(text, false);
                continue;
            };
            let (tw, _) = self.font.measure_line(label, style.body_size);
            let bw = (tw + 24.0).max(72.0);
            let bx = cursor_x - bw;
            scene.set_bounds(button, Rect::new(bx, btn_y, bw, style.action_height));
            scene.set_fill(button, if action_index == action_focus { style.primary_bg } else { style.secondary_bg });
            scene.set_mask(button, None);
            scene.set_visible(button, true);
            self.apply_corner_radius(scene, button, true);
            let (_, lm) = self.font.rasterize_text(label, style.body_size, None);
            scene.set_bounds(text, Rect::new(
                bx + ((bw - lm.width as f32) * 0.5).max(0.0),
                btn_y + ((style.action_height - lm.height as f32) * 0.5).max(0.0),
                lm.width as f32, lm.height as f32,
            ));
            scene.set_fill(text, style.action_fg);
            scene.set_mask(text, Some(AlphaMask::new(lm.width, lm.height, lm.pixels)));
            scene.set_visible(text, true);
            cursor_x = bx - 8.0;
            action_index += 1;
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
            self.apply_corner_radius(scene, bg, true);
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
        self.apply_corner_radius(scene, node, false);
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
            self.apply_corner_radius(scene, bg, true);
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
                if hi {
                    self.apply_corner_radius(scene, rbg, false);
                }
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
        self.apply_corner_radius(scene, host, true);

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
                self.apply_corner_radius(scene, chrome, false);
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
                if flash {
                    self.apply_corner_radius(scene, bg, false);
                }
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
                        e.severity.color_for_theme(&self.theme)
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
        self.apply_corner_radius(scene, node, true);

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
        scene.set_clip(node, Some(bounds));
        scene.set_visible(node, true);
        self.apply_corner_radius(scene, node, true);

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
                        body.x + 4.0,
                        body.y + 6.0,
                        (body.width - 8.0).max(4.0),
                        (body.height - 12.0).max(4.0),
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
    /// Borrow the active [`Theme`] (for Style `from_theme` / DSL builders).
    pub fn theme(&self) -> &Theme {
        self.ui.theme()
    }

    /// Add a text label; returns its [`WidgetId`].
    ///
    /// Uses the active [`Theme`] text color and body size (density-scaled).
    pub fn text(&mut self, content: impl Into<String>) -> WidgetId {
        let theme = self.ui.theme;
        let size = (theme.typography.body * self.ui.config.density.font_scale())
            .round()
            .max(1.0);
        self.text_styled(content, theme.colors.text_primary, size)
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
    ///
    /// Uses the active [`Theme`] button colors and density-scaled sizes.
    pub fn button(&mut self, label: impl Into<String>) -> WidgetId {
        let theme = self.ui.theme;
        self.button_styled(label, theme.colors.button_bg, theme.colors.button_fg)
    }

    /// Button with custom colors (sizes still follow the active theme + density).
    pub fn button_styled(
        &mut self,
        label: impl Into<String>,
        bg: Color,
        fg: Color,
    ) -> WidgetId {
        let theme = self.ui.theme;
        let s = self.ui.config.density.spacing_scale();
        let f = self.ui.config.density.font_scale();
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Button {
                    label: label.into(),
                    enabled: true,
                    bg,
                    fg,
                    size: (theme.typography.button * f).round().max(1.0),
                    min_width: (88.0 * s).round().max(1.0),
                    height: (36.0 * s).round().max(1.0),
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

    /// Compact toolbar button (smaller height; theme colors + density).
    pub fn tool_button(&mut self, label: impl Into<String>) -> WidgetId {
        let theme = self.ui.theme;
        self.tool_button_styled(label, theme.colors.button_bg, theme.colors.button_fg)
    }

    /// Compact toolbar button with semantic colors; sizes follow the active theme.
    pub fn tool_button_styled(&mut self, label: impl Into<String>, bg: Color, fg: Color) -> WidgetId {
        let theme = self.ui.theme;
        let s = self.ui.config.density.spacing_scale();
        let f = self.ui.config.density.font_scale();
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Button {
                    label: label.into(),
                    enabled: true,
                    bg,
                    fg,
                    size: (theme.typography.button_sm * f).round().max(1.0),
                    min_width: (64.0 * s).round().max(1.0),
                    height: (28.0 * s).round().max(1.0),
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

    /// Built-in icon glyph (theme secondary text color).
    pub fn icon(&mut self, name: IconName) -> WidgetId {
        let theme = self.ui.theme;
        let size = (theme.typography.button * self.ui.config.density.font_scale())
            .round()
            .max(1.0);
        self.icon_styled(name, size, theme.colors.text_secondary)
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

    /// A clipped row whose overflowing controls remain reachable by horizontal wheel or drag.
    pub fn scroll_row<F>(&mut self, style: RowStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let viewport = ScrollAreaStyle {
            width: style.width,
            height: style.height,
            padding: 0.0,
            gap: 0.0,
            background: style.background,
            ..ScrollAreaStyle::default()
        };
        self.scroll_area(viewport, |area| {
            area.row(style, build);
        })
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

    /// Dialog chrome: titled [`Panel`] with body slot + OK/Cancel row.
    /// Prefer this over [`Ui::modal`] when the dialog needs custom child widgets
    /// (`Modal` only hosts title+body strings).
    ///
    /// Returns `(panel, ok_button, cancel_button)`.
    pub fn dialog_panel<F>(
        &mut self,
        title: impl Into<String>,
        style: PanelStyle,
        primary: impl Into<String>,
        secondary: impl Into<String>,
        build: F,
    ) -> (WidgetId, WidgetId, WidgetId)
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let mut ok_id = WidgetId(0);
        let mut cancel_id = WidgetId(0);
        let primary = primary.into();
        let secondary = secondary.into();
        let panel = self.panel(title, style, |p| {
            build(p);
            let mut row = RowStyle::from_theme(p.theme());
            row.height = 36.0;
            row.gap = 8.0;
            row.padding = 4.0;
            p.row(row, |r| {
                ok_id = r.tool_button(primary.clone());
                cancel_id = r.tool_button(secondary.clone());
            });
        });
        (panel, ok_id, cancel_id)
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
    ///
    /// Style comes from the active [`Theme`] + density.
    pub fn text_input(&mut self, placeholder: impl Into<String>) -> WidgetId {
        self.text_input_styled("", placeholder, self.ui.text_input_style())
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
                    secret: false,
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
        self.checkbox_styled(label, checked, self.ui.checkbox_style())
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
        self.radio_styled(group, label, selected, self.ui.radio_style())
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
        self.switch_styled(label, on, self.ui.switch_style())
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

    /// Basic text table with row selection and draggable column widths.
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
                    selected_col: None,
                    selected_range: None,
                    selected_col_range: None,
                    scroll_y: 0.0,
                    scroll_x: 0.0,
                    frozen_cols: 0,
                    cell_bg: Vec::new(),
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

    /// A table with standard text inputs aligned to its columns above the header.
    /// Inputs follow column resizing, horizontal scrolling and frozen columns;
    /// keyboard focus reveals hidden fields. Wheel over the filters scrolls the
    /// same table. `style.height` includes the filter row. Returns table + inputs.
    pub fn filtered_table(
        &mut self,
        columns: Vec<TableColumn>,
        rows: Vec<Vec<String>>,
        values: &[String],
        mut style: TableStyle,
        input_style: TextInputStyle,
    ) -> (WidgetId, Vec<WidgetId>) {
        let height = input_style.height + 4.0;
        let mut container = ColumnStyle::from_theme(self.theme());
        container.width = style.width;
        container.height = style.height;
        container.padding = 0.0;
        container.gap = 0.0;
        container.background = None;
        let mut filter_style = RowStyle::from_theme(self.theme());
        filter_style.width = style.width;
        filter_style.height = height;
        filter_style.padding = 2.0;
        filter_style.gap = 4.0;
        filter_style.background = Some(style.header_bg);
        let mut table = WidgetId(0);
        let mut fields = Vec::new();
        let mut filter_row = WidgetId(0);
        style.height = (style.height - height).max(style.header_height);
        self.column(container, |column| {
            filter_row = column.scroll_row(filter_style, |row| {
                for (index, definition) in columns.iter().enumerate() {
                    let mut field_style = input_style;
                    field_style.width = (definition.width - 4.0).max(1.0);
                    fields.push(row.text_input_styled(
                        values.get(index).map(String::as_str).unwrap_or(""),
                        &definition.title,
                        field_style,
                    ));
                }
            });
            table = column.table(columns, rows, style);
        });
        self.ui.table_filters.insert(
            filter_row,
            TableFilterRow {
                table,
                fields: fields.clone(),
                last_focus: None,
            },
        );
        (table, fields)
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
        self.tag_styled(label, false, self.ui.tag_style())
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
        self.badge_styled(BadgeKind::Count(count), self.ui.badge_style())
    }

    /// Standalone Badge dot.
    pub fn badge_dot(&mut self) -> WidgetId {
        self.badge_styled(BadgeKind::Dot, self.ui.badge_style())
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
        self.tooltip_styled(text, self.ui.tooltip_style(), build)
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
        self.progress_styled(value, false, self.ui.progress_style())
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
        self.alert_styled(tone, message, true, self.ui.alert_style())
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
                | Kind::Spin { children, .. }
                | Kind::Grid { children, .. } => {
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
                    selected_col: None,
                    selected_range: None,
                    selected_col_range: None,
                    scroll_y: 0.0,
                    scroll_x: 0.0,
                    frozen_cols: 0,
                    cell_bg: Vec::new(),
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
                    read_only: false,
                    placeholder: placeholder.into(),
                    caret,
                    preedit: String::new(),
                    preedit_cursor: None,
                    scroll_y: 0.0,
                    edit: TextAreaEdit::default(),
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


    /// Exclusive button group (Office P2a). One Tab stop; Left/Right changes selection.
    pub fn button_group(
        &mut self,
        labels: impl IntoIterator<Item = impl Into<String>>,
        selected: Option<usize>,
        style: ButtonGroupStyle,
    ) -> WidgetId {
        let labels: Vec<String> = labels.into_iter().map(|s| s.into()).collect();
        let selected = match selected {
            Some(i) if !labels.is_empty() => Some(i.min(labels.len() - 1)),
            Some(_) => None,
            None => None,
        };
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::ButtonGroup {
                    labels,
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

    /// Text link (Office P2a) with theme link color + underline.
    pub fn link(&mut self, label: impl Into<String>) -> WidgetId {
        self.link_styled(label, self.ui.link_style())
    }

    /// Text link with custom style.
    pub fn link_styled(&mut self, label: impl Into<String>, style: LinkStyle) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Link {
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

    /// Spacing container (Office P2a): sugar over [`Self::row`] / [`Self::column`].
    ///
    /// No separate retained `Kind` — see `docs/office-p2-design.md`.
    pub fn space<F>(&mut self, style: SpaceStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        match style.direction {
            SpaceDirection::Horizontal => {
                let row = RowStyle {
                    x: style.x,
                    y: style.y,
                    width: style.width,
                    height: style.height,
                    padding: style.padding,
                    gap: style.gap,
                    cross_align: style.cross_align,
                    background: style.background,
                };
                self.row(row, build)
            }
            SpaceDirection::Vertical => {
                let col = ColumnStyle {
                    x: style.x,
                    y: style.y,
                    width: style.width,
                    height: style.height,
                    padding: style.padding,
                    gap: style.gap,
                    cross_align: style.cross_align,
                    background: style.background,
                };
                self.column(col, build)
            }
        }
    }

    /// Step progress list (Office P2b).
    ///
    /// `items` are [`StepItem`] values (title + optional description). `current` is
    /// 0-based; clamped to the last item when non-empty.
    pub fn steps(
        &mut self,
        items: impl IntoIterator<Item = StepItem>,
        current: usize,
        style: StepsStyle,
    ) -> WidgetId {
        let items: Vec<StepItem> = items.into_iter().collect();
        let current = if items.is_empty() {
            0
        } else {
            current.min(items.len() - 1)
        };
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Steps {
                    items,
                    current,
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

    /// KPI statistic block (Office P2b); display-only (not in Tab order).
    pub fn statistic(
        &mut self,
        title: impl Into<String>,
        value: impl Into<String>,
        style: StatisticStyle,
    ) -> WidgetId {
        self.statistic_full(title, value, None::<String>, None::<String>, None::<String>, style)
    }

    /// Statistic with optional prefix / suffix / hint.
    pub fn statistic_full(
        &mut self,
        title: impl Into<String>,
        value: impl Into<String>,
        prefix: Option<impl Into<String>>,
        suffix: Option<impl Into<String>>,
        hint: Option<impl Into<String>>,
        style: StatisticStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Statistic {
                    title: title.into(),
                    value: value.into(),
                    prefix: prefix.map(|s| s.into()),
                    suffix: suffix.map(|s| s.into()),
                    hint: hint.map(|s| s.into()),
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

    /// Status result page block (Office P2b).
    ///
    /// Focusable only when at least one action label is set; Space/Enter activates
    /// Primary (else Secondary). Pointer hits on action rects drain
    /// [`Ui::take_result_action`].
    pub fn result(
        &mut self,
        status: ResultStatus,
        title: impl Into<String>,
        subtitle: Option<impl Into<String>>,
        primary_action: Option<impl Into<String>>,
        secondary_action: Option<impl Into<String>>,
        style: ResultStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Result {
                    status,
                    title: title.into(),
                    subtitle: subtitle.map(|s| s.into()),
                    primary_action: primary_action.map(|s| s.into()),
                    secondary_action: secondary_action.map(|s| s.into()),
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


    /// Popover: click-toggle bubble below the trigger (Office P2c).
    ///
    /// Placement defaults to below the host. Escape / outside click close via
    /// [`Ui`] `open_popup`. Prefer over Tooltip when content should stay until
    /// dismissed (Tooltip is hover-only).
    pub fn popover(
        &mut self,
        trigger: impl Into<String>,
        body: impl Into<String>,
        style: PopoverStyle,
    ) -> WidgetId {
        self.popover_titled(trigger, None::<String>, body, style)
    }

    /// Popover with optional title line above the body.
    pub fn popover_titled(
        &mut self,
        trigger: impl Into<String>,
        title: Option<impl Into<String>>,
        body: impl Into<String>,
        style: PopoverStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Popover {
                    trigger: trigger.into(),
                    title: title.map(|s| s.into()),
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
        self.push_child(id);
        id
    }

    /// Popconfirm: confirm bubble with OK / Cancel (Office P2c).
    ///
    /// Default labels are `"OK"` / `"Cancel"`. Confirm →
    /// [`Ui::take_popconfirm_action`](`Confirm`); Cancel button → `Cancel`;
    /// Escape / outside / host toggle-close → `Dismiss`.
    pub fn popconfirm(
        &mut self,
        trigger: impl Into<String>,
        message: impl Into<String>,
        style: PopconfirmStyle,
    ) -> WidgetId {
        self.popconfirm_labeled(trigger, message, "OK", "Cancel", style)
    }

    /// Popconfirm with custom OK / Cancel labels.
    pub fn popconfirm_labeled(
        &mut self,
        trigger: impl Into<String>,
        message: impl Into<String>,
        ok_label: impl Into<String>,
        cancel_label: impl Into<String>,
        style: PopconfirmStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Popconfirm {
                    trigger: trigger.into(),
                    message: message.into(),
                    ok_label: ok_label.into(),
                    cancel_label: cancel_label.into(),
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

    /// Upload path field + Browse (Office P2d).
    ///
    /// Browse emits [`Ui::take_upload_browse`]; the app opens an OS dialog (or
    /// stubs a path with [`Ui::set_upload_path`]). Clear emits
    /// [`Ui::take_upload_clear`] when `UploadStyle::show_clear` and a path is set.
    pub fn upload(
        &mut self,
        placeholder: impl Into<String>,
        style: UploadStyle,
    ) -> WidgetId {
        self.upload_labeled(placeholder, "Browse", "Clear", None::<String>, style)
    }

    /// Upload with custom Browse / Clear labels and optional initial path.
    pub fn upload_labeled(
        &mut self,
        placeholder: impl Into<String>,
        browse_label: impl Into<String>,
        clear_label: impl Into<String>,
        path: Option<impl Into<String>>,
        style: UploadStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Upload {
                    path: path.map(|s| s.into()),
                    placeholder: placeholder.into(),
                    browse_label: browse_label.into(),
                    clear_label: clear_label.into(),
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

    /// Color picker: preset swatches + hex field (Office P2d).
    pub fn color_picker(&mut self, value: Color, style: ColorPickerStyle) -> WidgetId {
        self.color_picker_with_presets(value, default_color_presets(), style)
    }

    /// Color picker with custom presets.
    pub fn color_picker_with_presets(
        &mut self,
        value: Color,
        presets: impl IntoIterator<Item = Color>,
        style: ColorPickerStyle,
    ) -> WidgetId {
        let presets: Vec<Color> = presets.into_iter().collect();
        let selected_swatch = presets.iter().position(|c| {
            c.r == value.r && c.g == value.g && c.b == value.b
        });
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::ColorPicker {
                    value,
                    hex: color_to_hex(value),
                    selected_swatch,
                    presets,
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

    /// Autocomplete text field + filtered suggestions (Office P2d).
    pub fn autocomplete(
        &mut self,
        options: impl IntoIterator<Item = impl Into<String>>,
        value: impl Into<String>,
        style: AutocompleteStyle,
    ) -> WidgetId {
        self.autocomplete_placeholder(options, value, "", style)
    }

    /// Autocomplete with placeholder.
    pub fn autocomplete_placeholder(
        &mut self,
        options: impl IntoIterator<Item = impl Into<String>>,
        value: impl Into<String>,
        placeholder: impl Into<String>,
        style: AutocompleteStyle,
    ) -> WidgetId {
        let options: Vec<String> = options.into_iter().map(|s| s.into()).collect();
        let value = value.into();
        let caret = value.chars().count();
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Autocomplete {
                    options,
                    value,
                    caret,
                    open: false,
                    highlight: None,
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


    /// Fixed-column grid (Office P2e). Children left→right, wrap by `columns`.
    pub fn grid<F>(&mut self, columns: usize, style: GridStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let columns = columns.max(1);
        let id = self.ui.alloc_container(Kind::Grid {
            columns,
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

    /// Vertical timeline (Office P2e).
    pub fn timeline(
        &mut self,
        items: impl IntoIterator<Item = TimelineItem>,
        style: TimelineStyle,
    ) -> WidgetId {
        let items: Vec<TimelineItem> = items.into_iter().collect();
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Timeline {
                    items,
                    selected: None,
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

    /// Month calendar (Office P2e); week starts Sunday.
    pub fn calendar(
        &mut self,
        year: i32,
        month: u8,
        selected_day: u8,
        style: CalendarStyle,
    ) -> WidgetId {
        let date = CalendarDate::new(year, month, selected_day);
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Calendar {
                    year: date.year,
                    month: date.month,
                    selected_day: date.day,
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

    /// RGBA8 image fitted to the host with preserved aspect ratio.
    /// Both renderers use nearest-neighbor scaling and straight-alpha blending.
    pub fn image_rgba(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u8],
        style: ImageStyle,
    ) -> WidgetId {
        self.image_rgba_caption(width, height, pixels, "", style)
    }

    /// RGBA8 image with caption.
    pub fn image_rgba_caption(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u8],
        caption: impl Into<String>,
        style: ImageStyle,
    ) -> WidgetId {
        let pixels = image_pixels_rgba8(width, height, pixels);
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Image {
                    px_w: width,
                    px_h: height,
                    pixels: Some(pixels),
                    view: ImageView::default(),
                    caption: caption.into(),
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

    /// Solid placeholder image with caption (no pixel buffer).
    pub fn image_placeholder(
        &mut self,
        caption: impl Into<String>,
        style: ImageStyle,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Image {
                    px_w: 0,
                    px_h: 0,
                    pixels: None,
                    view: ImageView::default(),
                    caption: caption.into(),
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
                // Opaque page fill so inactive siblings cannot show through gaps.
                background: Some(style.content_bg),
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


fn composed_text(value: &str, caret: usize, preedit: &str, cursor: Option<(usize, usize)>) -> (String, usize) {
    let caret = caret.min(value.chars().count());
    let mut display: String = value.chars().take(caret).collect();
    display.push_str(preedit);
    display.extend(value.chars().skip(caret));
    let offset = cursor.map(|(start, _)| preedit.char_indices().take_while(|(i, _)| *i < start).count()).unwrap_or_else(|| preedit.chars().count());
    (display, caret + offset)
}

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
    fn secret_input_masks_value_and_excludes_copy_and_accessible_name() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut input = WidgetId(0);
        ui.column(ColumnStyle::default(), |col| {
            input = col.text_input("Password");
        });
        ui.set_text_input(input, "中文secret");
        ui.set_text_input_secret(input, true);
        ui.set_focus(Some(input));
        ui.sync(&mut scene);
        assert_eq!(ui.text_input_value(input), Some("中文secret"));
        assert_eq!(ui.focused_text_for_copy(), None);
        assert_eq!(ui.accessible_name(input).as_deref(), Some("Password"));
        let widget = ui.widgets.get(&input).unwrap();
        let Kind::TextInput { style, .. } = &widget.kind else {
            panic!()
        };
        let (_, expected) = ui.font.rasterize_text("********", style.size, None);
        let actual = scene
            .get(widget.label_node.unwrap())
            .unwrap()
            .mask
            .as_ref()
            .unwrap();
        assert_eq!(actual.pixels, expected.pixels);
    }

    #[test]
    fn text_area_ime_previews_commits_and_cancels_without_changing_draft() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut area = WidgetId(0);
        ui.column(ColumnStyle::default(), |c| {
            area = c.text_area("SELECT '\n';", "SQL", TextAreaStyle::default());
        });
        ui.set_focus(Some(area));
        ui.key_input(UiKey::Left);
        assert!(ui.text_input_wants_ime());
        assert!(ui.ime_preedit("中文", Some((3, 3))));
        ui.sync(&mut scene);
        assert_eq!(ui.text_area_value(area), Some("SELECT '\n';"));
        assert!(ui.ime_cursor_area(&scene).is_some());
        let widget = ui.widgets.get(&area).unwrap();
        let mask = scene.get(widget.label_node.unwrap()).unwrap().mask.as_ref().unwrap().pixels.clone();
        ui.ime_clear_preedit();
        ui.sync(&mut scene);
        let widget = ui.widgets.get(&area).unwrap();
        assert_ne!(mask, scene.get(widget.label_node.unwrap()).unwrap().mask.as_ref().unwrap().pixels);
        assert!(ui.ime_commit("中文"));
        assert_eq!(ui.text_area_value(area), Some("SELECT '\n'中文;"));
        ui.ime_preedit("取消", None);
        ui.set_text_area_value(area, "replaced");
        let Kind::TextArea { preedit, .. } = &ui.widgets.get(&area).unwrap().kind else { panic!() };
        assert!(preedit.is_empty());
    }

    #[test]
    fn inactive_tab_descendants_stay_hidden_after_repeated_sync() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut tabs = WidgetId(0);
        let mut fields = Vec::new();
        ui.column(ColumnStyle::default(), |c| {
            tabs = c.tabs(TabsStyle::default(), |t| {
                for label in ["first", "second"] {
                    t.page(label, |p| {
                        p.column(ColumnStyle::default(), |c| {
                            fields.push(c.text_input(label));
                        });
                    });
                }
            });
        });
        for selected in [0, 1, 0, 1] {
            ui.set_tabs_selected(tabs, selected);
            ui.sync(&mut scene);
            ui.sync(&mut scene);
            for (i, id) in fields.iter().enumerate() {
                let w = ui.widgets.get(id).unwrap();
                for node in [w.node, w.label_node, w.chrome_node].into_iter().flatten() {
                    if i != selected {
                        assert!(!scene.get(node).unwrap().visible);
                    }
                }
                assert_eq!(scene.get(w.node.unwrap()).unwrap().visible, i == selected);
            }
        }
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
    fn text_widgets_accept_named_space_keys() {
        let mut ui = Ui::new();
        let mut input = WidgetId(0);
        let mut area = WidgetId(0);
        ui.column(ColumnStyle::default(), |c| {
            input = c.text_input("value");
            area = c.text_area("SELECT", "SQL", TextAreaStyle::default());
        });
        ui.set_focus(Some(input));
        assert!(ui.key_input(UiKey::Space));
        assert_eq!(ui.text_input_value(input), Some(" "));
        ui.set_focus(Some(area));
        assert!(ui.key_input(UiKey::Space));
        assert!(ui.key_input(UiKey::Char('1')));
        assert_eq!(ui.text_area_value(area), Some("SELECT 1"));
    }

    #[test]
    fn table_scroll_freeze_resize_and_clipping_share_geometry() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut table = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 400.0,
                ..ColumnStyle::default()
            },
            |c| {
                table = c.table(
                    (0..5)
                        .map(|i| TableColumn::new(format!("long header {i}"), 100.0))
                        .collect(),
                    vec![vec!["long cell text".into(); 5]; 20],
                    TableStyle {
                        width: 320.0,
                        height: 160.0,
                        padding: 0.0,
                        ..TableStyle::default()
                    },
                );
            },
        );
        ui.set_table_frozen_cols(table, 1);
        ui.set_table_scroll_offset(table, 150.0, 13.0);
        ui.sync(&mut scene);
        let w = ui.widgets.get(&table).unwrap();
        let bounds = scene.get(w.node.unwrap()).unwrap().bounds;
        let header_height = match &w.kind {
            Kind::Table { style, .. } => style.header_height,
            _ => unreachable!(),
        };
        // The second column is hidden; the visible third column begins at x + 100.
        ui.pointer_pressed(&mut scene, bounds.x + 120.0, bounds.y + 10.0);
        assert_eq!(ui.take_table_header_click(), Some((table, 2)));
        ui.pointer_pressed(&mut scene, bounds.x + 120.0, bounds.y + header_height + 5.0);
        assert_eq!(ui.table_selected_col(table), Some(2));
        ui.pointer_released();
        ui.pointer_pressed(&mut scene, bounds.x + 50.0, bounds.y + 10.0);
        assert_eq!(ui.take_table_header_click(), Some((table, 0)));
        // Resize the visible third column without sorting it.
        ui.pointer_pressed(&mut scene, bounds.x + 150.0, bounds.y + 10.0);
        assert!(ui.is_dragging());
        assert_eq!(ui.take_table_header_click(), None);
        assert!(ui.pointer_moved(bounds.x + 190.0, bounds.y + 10.0));
        ui.pointer_released();
        assert_eq!(ui.table_column_widths(table).unwrap()[2], 140.0);
        assert!(!ui.set_table_column_width(table, 2, f32::NAN));
        assert!(!ui.set_table_scroll_offset(table, f32::INFINITY, 0.0));
        ui.sync(&mut scene);
        let w = ui.widgets.get(&table).unwrap();
        let hidden_header = scene.get(w.aux[1]).unwrap();
        assert!(!hidden_header.visible);
        let header_clip = scene.get(w.aux[2]).unwrap().clip.unwrap();
        assert_eq!(header_clip.x, bounds.x + 100.0);
        assert_eq!(header_clip.width, 90.0);
        let body_clip = scene.get(w.aux[5 + 1 + 5 + 2]).unwrap().clip.unwrap();
        assert_eq!(body_clip.x, bounds.x + 100.0);
        assert_eq!(body_clip.y, bounds.y + header_height);
        // Encoder must crop the glyph masks, not only their layout bounds.
        scene.mark_full_dirty(500.0, 600.0);
        let dirty = scene.take_dirty();
        let cmds = next_gui_render::encode_scene(&scene, &dirty);
        for cmd in cmds {
            if let next_gui_render::DrawCommand::BlitAlpha { x, width, .. } = cmd {
                let edge = (bounds.x + 100.0) as i32;
                assert!(x >= edge || x + width as i32 <= edge);
            }
        }
        ui.set_table_column_width(table, 2, -1.0);
        assert_eq!(ui.table_column_widths(table).unwrap()[2], 24.0);
    }

    #[test]
    fn table_filter_inputs_follow_resize_scroll_freeze_and_keyboard_focus() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut table = WidgetId(0);
        let mut fields = Vec::new();
        ui.column(
            ColumnStyle {
                width: 360.0,
                padding: 0.0,
                gap: 0.0,
                ..ColumnStyle::default()
            },
            |c| {
                (table, fields) = c.filtered_table(
                    (0..5)
                        .map(|i| TableColumn::new(format!("Column {i}"), 100.0))
                        .collect(),
                    vec![vec!["value".into(); 5]; 30],
                    &["keep".into()],
                    TableStyle {
                        width: 320.0,
                        height: 200.0,
                        padding: 0.0,
                        ..TableStyle::default()
                    },
                    TextInputStyle::default(),
                );
            },
        );
        ui.set_table_frozen_cols(table, 1);
        ui.set_table_scroll_offset(table, 150.0, 0.0);
        ui.sync(&mut scene);
        let bounds = scene.get(ui.widgets[&table].node.unwrap()).unwrap().bounds;
        let field_bounds = |scene: &Scene, ui: &Ui, index: usize| {
            scene
                .get(ui.widgets[&fields[index]].node.unwrap())
                .unwrap()
                .clone()
        };
        assert_eq!(field_bounds(&scene, &ui, 0).bounds.x, bounds.x + 2.0);
        assert!(field_bounds(&scene, &ui, 1).clip.unwrap().is_empty());
        assert_eq!(
            field_bounds(&scene, &ui, 2).clip.unwrap().x,
            bounds.x + 100.0
        );
        assert_eq!(ui.text_input_value(fields[0]), Some("keep"));
        let visible = field_bounds(&scene, &ui, 2).clip.unwrap();
        ui.pointer_pressed(&mut scene, visible.x + 4.0, visible.y + 8.0);
        assert_eq!(ui.focused(), Some(fields[2]));
        assert!(ui.key_input(UiKey::Char('中')));
        ui.sync(&mut scene);
        assert!(ui.text_input_value(fields[2]).unwrap().contains('中'));
        ui.set_table_column_width(table, 0, 140.0);
        ui.sync(&mut scene);
        assert_eq!(field_bounds(&scene, &ui, 0).bounds.width, 136.0);
        assert!(field_bounds(&scene, &ui, 2).clip.unwrap().x >= bounds.x + 140.0);

        ui.set_focus(Some(fields[4]));
        ui.sync(&mut scene);
        let last = field_bounds(&scene, &ui, 4);
        assert!(!last.clip.unwrap().is_empty());
        assert!(last.clip.unwrap().x + last.clip.unwrap().width <= bounds.x + bounds.width);
        let offset = ui.table_scroll_offset(table).unwrap().0;
        let frozen = field_bounds(&scene, &ui, 0);
        assert!(ui.pointer_scroll(
            &scene,
            frozen.bounds.x + 4.0,
            frozen.bounds.y + 8.0,
            -50.0,
            32.0
        ));
        assert!(ui.table_scroll_offset(table).unwrap().0 < offset);
        assert!(ui.table_scroll_offset(table).unwrap().1 > 0.0);
        ui.sync(&mut scene);
        assert!(
            ui.table_scroll_offset(table).unwrap().0 < offset,
            "Wheel must not snap back to the focused column"
        );
        ui.set_focus(Some(fields[0]));
        assert_eq!(ui.focus_next(), Some(fields[1]));
        ui.sync(&mut scene);
        assert!(!field_bounds(&scene, &ui, 1).clip.unwrap().is_empty());
    }

    #[test]
    fn table_paints_all_body_rows_with_cell_bg_aux_stride() {
        // Regression: cell_bg doubled aux stride; old paint used 1+ncols and only
        // the last row stayed visible when nodes overlapped.
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut table = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 400.0,
                height: 220.0,
                ..ColumnStyle::default()
            },
            |col| {
                table = col.table(
                    vec![
                        TableColumn::new("id", 60.0),
                        TableColumn::new("name", 120.0),
                    ],
                    vec![
                        vec!["1".into(), "Alice".into()],
                        vec!["2".into(), "Bob".into()],
                        vec!["3".into(), "Carol".into()],
                    ],
                    TableStyle {
                        width: 360.0,
                        height: 160.0,
                        header_height: 28.0,
                        row_height: 26.0,
                        ..TableStyle::default()
                    },
                );
            },
        );
        ui.set_table_cell_bg(
            table,
            vec![
                vec![None, None],
                vec![None, Some(Color::rgb(0xe6, 0x7e, 0x22))],
                vec![None, None],
            ],
        );
        ui.sync(&mut scene);
        let w = ui.widgets.get(&table).expect("table");
        let ncols = 2usize;
        let n_rows = 3usize;
        // header labels + per-row (bg + cell_bg*ncols + text*ncols)
        let expected_aux = ncols + n_rows * (1 + 2 * ncols);
        assert!(
            w.aux.len() >= expected_aux,
            "aux len {} < expected {}",
            w.aux.len(),
            expected_aux
        );
        let host = w.node.expect("host");
        let bounds = scene.get(host).expect("bounds").bounds;
        let mut visible_textish = 0usize;
        for &n in &w.aux {
            if let Some(node) = scene.get(n) {
                if node.visible && node.mask.is_some() {
                    // body text sits below header
                    if node.bounds.y >= bounds.y + 28.0 - 1.0 {
                        visible_textish += 1;
                    }
                }
            }
        }
        // 3 rows × 2 cols = 6 body text labels should be visible
        assert!(
            visible_textish >= 6,
            "expected >=6 visible body text masks, got {visible_textish}"
        );
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
    fn modal_three_actions_wrap_body_and_support_pointer_and_keyboard() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        ui.set_viewport(640.0, 480.0);
        let modal = ui.modal("Save changes?", "A long explanation that must wrap within the modal instead of extending beyond its edge. ".repeat(3), "Save", Some("Discard".into()), ModalStyle::default());
        assert!(ui.set_modal_tertiary(modal, Some("Cancel".into())));
        for (slot, expected) in [(6, ModalAction::Primary), (8, ModalAction::Secondary), (10, ModalAction::Tertiary)] {
            ui.set_modal_open(modal, true);
            ui.sync(&mut scene);
            let aux = &ui.widgets[&modal].aux;
            let panel = scene.get(aux[1]).unwrap().bounds;
            let body = scene.get(aux[5]).unwrap().bounds;
            let bounds = scene.get(aux[slot]).unwrap().bounds;
            assert!(body.x + body.width <= panel.x + panel.width);
            assert!(body.y + body.height <= bounds.y);
            assert!(ui.pointer_press(&mut scene, bounds.x + 8.0, bounds.y + 8.0, false));
            assert_eq!(ui.take_modal_action(), Some((modal, expected)));
        }
        ui.set_modal_open(modal, true);
        assert!(ui.key_input(UiKey::Tab));
        assert!(ui.key_input(UiKey::Enter));
        assert_eq!(ui.take_modal_action(), Some((modal, ModalAction::Secondary)));
        ui.set_modal_open(modal, true);
        assert!(ui.key_input(UiKey::BackTab));
        assert!(ui.key_input(UiKey::Space));
        assert_eq!(ui.take_modal_action(), Some((modal, ModalAction::Tertiary)));
        ui.set_modal_open(modal, true);
        assert!(ui.key_input(UiKey::Escape));
        assert_eq!(ui.take_modal_action(), Some((modal, ModalAction::Dismiss)));
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
        assert_eq!(ui.text_area_caret(area), Some(11));
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

    #[test]
    fn office_p2a_button_group_select_and_keyboard() {
        let mut ui = Ui::new();
        let mut bg = WidgetId(0);
        ui.column(ColumnStyle::default(), |col| {
            bg = col.button_group(["A", "B", "C"], Some(0), ButtonGroupStyle::default());
        });
        assert_eq!(ui.button_group_selected(bg), Some(Some(0)));
        assert!(ui.focusable_widgets().contains(&bg));
        ui.set_focus(Some(bg));
        assert!(ui.take_button_group_change().is_none());
        assert!(ui.key_input(UiKey::Right));
        assert_eq!(ui.button_group_selected(bg), Some(Some(1)));
        assert_eq!(ui.take_button_group_change(), Some((bg, Some(1))));
        assert!(ui.take_button_group_change().is_none(), "drain once");
        assert!(ui.key_input(UiKey::Left));
        assert_eq!(ui.button_group_selected(bg), Some(Some(0)));
        assert!(ui.set_button_group_selected(bg, None));
        assert_eq!(ui.button_group_selected(bg), Some(None));
        assert_eq!(ui.take_button_group_change(), Some((bg, None)));
        assert!(!ui.set_button_group_selected(bg, Some(9)), "oob");
    }

    #[test]
    fn office_p2a_link_focus_and_activate() {
        let mut ui = Ui::new();
        let mut link = WidgetId(0);
        let mut btn = WidgetId(0);
        ui.column(ColumnStyle::default(), |col| {
            link = col.link("Docs");
            btn = col.button("Go");
        });
        let order = ui.focusable_widgets();
        assert!(order.contains(&link), "link focusable");
        assert_eq!(ui.accessible_name(link).as_deref(), Some("Docs"));
        ui.set_focus(Some(link));
        assert!(ui.take_link_activate().is_none());
        assert!(ui.key_input(UiKey::Enter));
        assert_eq!(ui.take_link_activate(), Some(link));
        assert!(ui.take_link_activate().is_none());
        assert!(ui.key_input(UiKey::Space));
        assert_eq!(ui.take_link_activate(), Some(link));
        let _ = btn;
    }

    #[test]
    fn office_p2a_space_sugar_row_column() {
        let mut ui = Ui::new();
        let mut inner_btn = WidgetId(0);
        let space_id = ui.space(
            SpaceStyle {
                direction: SpaceDirection::Horizontal,
                gap: 16.0,
                width: 300.0,
                height: 40.0,
                ..SpaceStyle::default()
            },
            |row| {
                inner_btn = row.button("X");
                row.link("Y");
            },
        );
        // Space is sugar → Kind::Row
        assert!(matches!(
            ui.widgets.get(&space_id).map(|w| &w.kind),
            Some(Kind::Row { .. })
        ));
        assert!(ui.focusable_widgets().contains(&inner_btn));

        let mut vert_child = WidgetId(0);
        let col_id = ui.space(
            SpaceStyle {
                direction: SpaceDirection::Vertical,
                gap: 8.0,
                ..SpaceStyle::default()
            },
            |col| {
                vert_child = col.link("Z");
            },
        );
        assert!(matches!(
            ui.widgets.get(&col_id).map(|w| &w.kind),
            Some(Kind::Column { .. })
        ));
        assert!(ui.focusable_widgets().contains(&vert_child));
    }

    #[test]
    fn office_p2a_button_group_pointer_selects() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut bg = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 320.0,
                height: 80.0,
                padding: 8.0,
                gap: 8.0,
                ..ColumnStyle::default()
            },
            |col| {
                bg = col.button_group(
                    ["One", "Two", "Three"],
                    Some(0),
                    ButtonGroupStyle {
                        width: 300.0,
                        height: 32.0,
                        ..ButtonGroupStyle::default()
                    },
                );
            },
        );
        ui.sync(&mut scene);
        let node = ui.widgets.get(&bg).unwrap().node.unwrap();
        let b = scene.get(node).unwrap().bounds;
        // Click toward the right third → index 2
        let x = b.x + b.width * 0.85;
        let y = b.y + b.height * 0.5;
        assert!(ui.pointer_pressed(&mut scene, x, y));
        assert_eq!(ui.button_group_selected(bg), Some(Some(2)));
        assert_eq!(ui.focused(), Some(bg));
        assert_eq!(ui.take_button_group_change(), Some((bg, Some(2))));
    }

    #[test]
    fn office_p2b_steps_change_keyboard_and_pointer() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut steps = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 520.0,
                height: 100.0,
                padding: 8.0,
                ..ColumnStyle::default()
            },
            |col| {
                steps = col.steps(
                    [
                        StepItem::new("One"),
                        StepItem::with_description("Two", "detail"),
                        StepItem::new("Three"),
                    ],
                    0,
                    StepsStyle {
                        width: 480.0,
                        height: 72.0,
                        clickable: true,
                        ..StepsStyle::default()
                    },
                );
            },
        );
        assert_eq!(ui.steps_current(steps), Some(0));
        assert!(ui.focusable_widgets().contains(&steps));
        assert_eq!(ui.accessible_name(steps).as_deref(), Some("One"));
        ui.set_focus(Some(steps));
        assert!(ui.key_input(UiKey::Right));
        assert_eq!(ui.steps_current(steps), Some(1));
        assert_eq!(ui.take_steps_change(), Some((steps, 1)));
        assert!(ui.take_steps_change().is_none());
        assert!(ui.key_input(UiKey::Left));
        assert_eq!(ui.steps_current(steps), Some(0));
        ui.sync(&mut scene);
        let node = ui.widgets.get(&steps).unwrap().node.unwrap();
        let b = scene.get(node).unwrap().bounds;
        let x = b.x + b.width * 0.85;
        let y = b.y + b.height * 0.5;
        assert!(ui.pointer_pressed(&mut scene, x, y));
        assert_eq!(ui.steps_current(steps), Some(2));
        assert_eq!(ui.take_steps_change(), Some((steps, 2)));
        assert!(!ui.set_steps_current(steps, 9), "oob");
    }

    #[test]
    fn office_p2b_statistic_mount_not_focusable() {
        let mut ui = Ui::new();
        let mut stat = WidgetId(0);
        ui.column(ColumnStyle::default(), |col| {
            stat = col.statistic_full(
                "Users",
                "1,280",
                Some("+"),
                Some("k"),
                Some("+12% MoM"),
                StatisticStyle::default(),
            );
        });
        assert!(!ui.focusable_widgets().contains(&stat));
        assert_eq!(
            ui.accessible_name(stat).as_deref(),
            Some("Users: 1,280")
        );
        assert!(matches!(
            ui.widgets.get(&stat).map(|w| &w.kind),
            Some(Kind::Statistic { .. })
        ));
        let mut scene = Scene::new();
        ui.sync(&mut scene);
        assert!(ui.widgets.get(&stat).unwrap().node.is_some());
    }

    #[test]
    fn office_p2b_result_action_keyboard_and_pointer() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut res = WidgetId(0);
        let mut bare = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 400.0,
                height: 280.0,
                padding: 8.0,
                gap: 8.0,
                ..ColumnStyle::default()
            },
            |col| {
                res = col.result(
                    ResultStatus::Success,
                    "Submitted",
                    Some("Ticket #42"),
                    Some("Back home"),
                    Some("View detail"),
                    ResultStyle {
                        width: 360.0,
                        height: 200.0,
                        ..ResultStyle::default()
                    },
                );
                bare = col.result(
                    ResultStatus::Info,
                    "No actions",
                    None::<&str>,
                    None::<&str>,
                    None::<&str>,
                    ResultStyle {
                        width: 360.0,
                        height: 120.0,
                        ..ResultStyle::default()
                    },
                );
            },
        );
        assert!(ui.focusable_widgets().contains(&res));
        assert!(!ui.focusable_widgets().contains(&bare));
        assert_eq!(ui.accessible_name(res).as_deref(), Some("Submitted"));
        ui.set_focus(Some(res));
        assert!(ui.key_input(UiKey::Enter));
        assert_eq!(
            ui.take_result_action(),
            Some((res, ResultAction::Primary))
        );
        assert!(ui.take_result_action().is_none());
        ui.sync(&mut scene);
        let node = ui.widgets.get(&res).unwrap().node.unwrap();
        let b = scene.get(node).unwrap().bounds;
        // Primary is rightmost — click near right edge of action row
        let x = b.x + b.width - 40.0;
        let y = b.y + b.height - 20.0;
        assert!(ui.pointer_pressed(&mut scene, x, y));
        assert_eq!(
            ui.take_result_action(),
            Some((res, ResultAction::Primary))
        );
        // Secondary is left of primary
        let x2 = b.x + b.width - 160.0;
        assert!(ui.pointer_pressed(&mut scene, x2, y));
        assert_eq!(
            ui.take_result_action(),
            Some((res, ResultAction::Secondary))
        );
    }

    #[test]
    fn office_p2c_popover_toggle_escape_and_outside() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        ui.set_viewport(800.0, 600.0);
        let mut pop = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 400.0,
                height: 200.0,
                padding: 8.0,
                ..ColumnStyle::default()
            },
            |col| {
                pop = col.popover_titled(
                    "More",
                    Some("Tips"),
                    "Rich-ish body text",
                    PopoverStyle::default(),
                );
            },
        );
        assert!(ui.focusable_widgets().contains(&pop));
        assert_eq!(ui.popover_is_open(pop), Some(false));
        assert_eq!(ui.accessible_name(pop).as_deref(), Some("Tips"));
        ui.sync(&mut scene);
        let node = ui.widgets.get(&pop).unwrap().node.unwrap();
        let b = scene.get(node).unwrap().bounds;
        let cx = b.x + b.width * 0.5;
        let cy = b.y + b.height * 0.5;
        assert!(ui.pointer_pressed(&mut scene, cx, cy));
        assert_eq!(ui.popover_is_open(pop), Some(true));
        assert_eq!(ui.take_popover_open_change(), Some((pop, true)));
        assert!(ui.take_popover_open_change().is_none());
        ui.sync(&mut scene);
        // Escape closes
        assert!(ui.key_input(UiKey::Escape));
        assert_eq!(ui.popover_is_open(pop), Some(false));
        assert_eq!(ui.take_popover_open_change(), Some((pop, false)));
        // Re-open via API
        assert!(ui.set_popover_open(pop, true));
        assert_eq!(ui.popover_is_open(pop), Some(true));
        let _ = ui.take_popover_open_change();
        ui.sync(&mut scene);
        // Outside click
        assert!(ui.pointer_pressed(&mut scene, 10.0, 500.0));
        assert_eq!(ui.popover_is_open(pop), Some(false));
        assert_eq!(ui.take_popover_open_change(), Some((pop, false)));
        // Keyboard toggle while focused
        ui.set_focus(Some(pop));
        assert!(ui.key_input(UiKey::Enter));
        assert_eq!(ui.popover_is_open(pop), Some(true));
    }

    #[test]
    fn office_p2c_popconfirm_confirm_cancel_dismiss() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        ui.set_viewport(800.0, 600.0);
        let mut pc = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 400.0,
                height: 240.0,
                padding: 8.0,
                ..ColumnStyle::default()
            },
            |col| {
                pc = col.popconfirm_labeled(
                    "Delete",
                    "Remove this item?",
                    "OK",
                    "Cancel",
                    PopconfirmStyle::default(),
                );
            },
        );
        assert!(ui.focusable_widgets().contains(&pc));
        assert_eq!(ui.popconfirm_is_open(pc), Some(false));
        ui.sync(&mut scene);
        let node = ui.widgets.get(&pc).unwrap().node.unwrap();
        let b = scene.get(node).unwrap().bounds;
        let cx = b.x + b.width * 0.5;
        let cy = b.y + b.height * 0.5;
        assert!(ui.pointer_pressed(&mut scene, cx, cy));
        assert_eq!(ui.popconfirm_is_open(pc), Some(true));
        ui.sync(&mut scene);
        // Hit OK button (right side of popup panel)
        let aux = ui.widgets.get(&pc).unwrap().aux.clone();
        assert!(aux.len() >= 6);
        let ok_bounds = scene.get(aux[2]).unwrap().bounds;
        assert!(ui.pointer_pressed(
            &mut scene,
            ok_bounds.x + ok_bounds.width * 0.5,
            ok_bounds.y + ok_bounds.height * 0.5,
        ));
        assert_eq!(
            ui.take_popconfirm_action(),
            Some((pc, PopconfirmAction::Confirm))
        );
        assert_eq!(ui.popconfirm_is_open(pc), Some(false));
        assert!(ui.take_popconfirm_action().is_none());

        // Re-open, Cancel
        assert!(ui.set_popconfirm_open(pc, true));
        ui.sync(&mut scene);
        let aux = ui.widgets.get(&pc).unwrap().aux.clone();
        let cancel_bounds = scene.get(aux[4]).unwrap().bounds;
        assert!(ui.pointer_pressed(
            &mut scene,
            cancel_bounds.x + cancel_bounds.width * 0.5,
            cancel_bounds.y + cancel_bounds.height * 0.5,
        ));
        assert_eq!(
            ui.take_popconfirm_action(),
            Some((pc, PopconfirmAction::Cancel))
        );

        // Re-open, Escape → Dismiss
        assert!(ui.set_popconfirm_open(pc, true));
        assert!(ui.key_input(UiKey::Escape));
        assert_eq!(
            ui.take_popconfirm_action(),
            Some((pc, PopconfirmAction::Dismiss))
        );
        assert_eq!(ui.popconfirm_is_open(pc), Some(false));

        // Re-open, outside → Dismiss
        assert!(ui.set_popconfirm_open(pc, true));
        ui.sync(&mut scene);
        assert!(ui.pointer_pressed(&mut scene, 10.0, 500.0));
        assert_eq!(
            ui.take_popconfirm_action(),
            Some((pc, PopconfirmAction::Dismiss))
        );
    }

    #[test]
    fn office_p2d_upload_browse_clear_and_keyboard() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        ui.set_viewport(800.0, 600.0);
        let mut up = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 480.0,
                height: 80.0,
                padding: 8.0,
                ..ColumnStyle::default()
            },
            |col| {
                up = col.upload("Choose a file…", UploadStyle::default());
            },
        );
        assert!(ui.focusable_widgets().contains(&up));
        assert_eq!(ui.upload_path(up), Some(None));
        assert_eq!(ui.accessible_name(up).as_deref(), Some("Choose a file…"));
        ui.sync(&mut scene);
        let node = ui.widgets.get(&up).unwrap().node.unwrap();
        let b = scene.get(node).unwrap().bounds;
        // Browse is on the right
        let browse_x = b.x + b.width - UploadStyle::default().padding - 4.0;
        let cy = b.y + b.height * 0.5;
        assert!(ui.pointer_pressed(&mut scene, browse_x, cy));
        assert_eq!(ui.take_upload_browse(), Some(up));
        assert!(ui.take_upload_browse().is_none());
        assert!(ui.set_upload_path(up, Some("/tmp/demo.csv")));
        assert_eq!(ui.upload_path(up), Some(Some("/tmp/demo.csv")));
        ui.sync(&mut scene);
        // Clear button left of Browse
        let clear_x = browse_x - UploadStyle::default().browse_width - 20.0;
        assert!(ui.pointer_pressed(&mut scene, clear_x, cy));
        assert_eq!(ui.take_upload_clear(), Some(up));
        assert_eq!(ui.upload_path(up), Some(None));
        ui.set_focus(Some(up));
        assert!(ui.key_input(UiKey::Enter));
        assert_eq!(ui.take_upload_browse(), Some(up));
    }

    #[test]
    fn office_p2d_color_picker_swatch_and_hex() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        ui.set_viewport(800.0, 600.0);
        let mut cp = WidgetId(0);
        let initial = Color::rgb(0xe0, 0x60, 0x60);
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 480.0,
                height: 80.0,
                padding: 8.0,
                ..ColumnStyle::default()
            },
            |col| {
                cp = col.color_picker(initial, ColorPickerStyle::default());
            },
        );
        assert!(ui.focusable_widgets().contains(&cp));
        assert_eq!(ui.color_picker_value(cp), Some(initial));
        assert_eq!(ui.accessible_name(cp).as_deref(), Some("#E06060"));
        ui.sync(&mut scene);
        ui.set_focus(Some(cp));
        assert!(ui.key_input(UiKey::Right));
        let changed = ui.take_color_change().expect("color change");
        assert_eq!(changed.0, cp);
        assert_ne!(changed.1, initial);
        // Hex commit
        assert!(ui.set_color_picker_value(cp, Color::rgb(0x10, 0x20, 0x30)));
        assert_eq!(ui.color_picker_value(cp).unwrap().r, 0x10);
        // Type hex via keys after clearing through set
        ui.set_focus(Some(cp));
        // Replace hex by typing digits after backspaces — use set + Enter path
        if let Some(w) = ui.widgets.get_mut(&cp) {
            if let Kind::ColorPicker { hex, .. } = &mut w.kind {
                *hex = "#AABBCC".into();
            }
        }
        assert!(ui.key_input(UiKey::Enter));
        assert_eq!(
            ui.take_color_change(),
            Some((cp, Color::rgb(0xaa, 0xbb, 0xcc)))
        );
        // Pointer on second swatch
        ui.sync(&mut scene);
        let aux = ui.widgets.get(&cp).unwrap().aux.clone();
        assert!(aux.len() >= 2);
        let sw = scene.get(aux[1]).unwrap().bounds;
        assert!(ui.pointer_pressed(
            &mut scene,
            sw.x + sw.width * 0.5,
            sw.y + sw.height * 0.5,
        ));
        let hit = ui.take_color_change().expect("swatch");
        assert_eq!(hit.0, cp);
    }

    #[test]
    fn office_p2d_autocomplete_filter_pick_and_escape() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        ui.set_viewport(800.0, 600.0);
        let mut ac = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 400.0,
                height: 200.0,
                padding: 8.0,
                ..ColumnStyle::default()
            },
            |col| {
                ac = col.autocomplete_placeholder(
                    ["Apple", "Apricot", "Banana", "Cherry"],
                    "",
                    "Fruit…",
                    AutocompleteStyle::default(),
                );
            },
        );
        assert!(ui.focusable_widgets().contains(&ac));
        assert_eq!(ui.autocomplete_value(ac), Some(""));
        assert_eq!(ui.accessible_name(ac).as_deref(), Some("Fruit…"));
        ui.sync(&mut scene);
        ui.set_focus(Some(ac));
        assert!(ui.key_input(UiKey::Char('a')));
        assert_eq!(ui.autocomplete_value(ac), Some("a"));
        // popup should be open with Apple/Apricot
        assert!(matches!(
            ui.widgets.get(&ac).map(|w| &w.kind),
            Some(Kind::Autocomplete { open: true, .. })
        ));
        assert!(ui.key_input(UiKey::Down));
        assert!(ui.key_input(UiKey::Enter));
        let (id, val) = ui.take_autocomplete_change().expect("pick");
        assert_eq!(id, ac);
        assert!(val == "Apple" || val == "Apricot");
        assert!(matches!(
            ui.widgets.get(&ac).map(|w| &w.kind),
            Some(Kind::Autocomplete { open: false, .. })
        ));
        // Type again, Escape closes
        ui.set_focus(Some(ac));
        assert!(ui.set_autocomplete_value(ac, ""));
        assert!(ui.key_input(UiKey::Char('b')));
        assert!(ui.key_input(UiKey::Escape));
        assert!(matches!(
            ui.widgets.get(&ac).map(|w| &w.kind),
            Some(Kind::Autocomplete { open: false, .. })
        ));
    }

    #[test]
    fn office_p2e_grid_lays_out_children() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        ui.set_viewport(800.0, 600.0);
        let mut ids = Vec::new();
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 400.0,
                height: 200.0,
                padding: 0.0,
                ..ColumnStyle::default()
            },
            |col| {
                col.grid(
                    2,
                    GridStyle {
                        width: 380.0,
                        height: 120.0,
                        padding: 4.0,
                        gap: 8.0,
                        background: Some(Color::rgb(0x20, 0x20, 0x24)),
                        ..GridStyle::default()
                    },
                    |g| {
                        ids.push(g.text_styled("A", Color::rgb(255, 255, 255), 14.0));
                        ids.push(g.text_styled("B", Color::rgb(255, 255, 255), 14.0));
                        ids.push(g.text_styled("C", Color::rgb(255, 255, 255), 14.0));
                    },
                );
            },
        );
        ui.sync(&mut scene);
        assert_eq!(ids.len(), 3);
        let ba = scene
            .get(ui.widgets.get(&ids[0]).unwrap().node.unwrap())
            .unwrap()
            .bounds;
        let bb = scene
            .get(ui.widgets.get(&ids[1]).unwrap().node.unwrap())
            .unwrap()
            .bounds;
        let bc = scene
            .get(ui.widgets.get(&ids[2]).unwrap().node.unwrap())
            .unwrap()
            .bounds;
        assert!(bb.x > ba.x, "B should be right of A");
        assert!(bc.y > ba.y, "C should wrap to next row");
    }

    #[test]
    fn office_p2e_timeline_select() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut tl = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 400.0,
                height: 200.0,
                padding: 8.0,
                ..ColumnStyle::default()
            },
            |col| {
                tl = col.timeline(
                    [
                        TimelineItem::new("09:00", "Boot"),
                        TimelineItem::new("09:05", "Ready").with_color(0x6ac08a),
                    ],
                    TimelineStyle::default(),
                );
            },
        );
        assert!(ui.focusable_widgets().contains(&tl));
        ui.sync(&mut scene);
        let node = ui.widgets.get(&tl).unwrap().node.unwrap();
        let b = scene.get(node).unwrap().bounds;
        let x = b.x + b.width * 0.5;
        let y = b.y + TimelineStyle::default().padding + TimelineStyle::default().item_height * 1.4;
        assert!(ui.pointer_pressed(&mut scene, x, y));
        assert_eq!(ui.take_timeline_select(), Some((tl, 1)));
        assert_eq!(ui.timeline_selected(tl), Some(Some(1)));
    }

    #[test]
    fn office_p2e_calendar_day_and_arrows() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        ui.set_config(UiConfig {
            locale: "zh-CN".into(),
            density: UiDensity::Compact,
            accent: Some(Color::rgb(0xe0, 0x60, 0x60)),
        });
        assert_eq!(ui.config().locale, "zh-CN");
        let mut cal = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 320.0,
                height: 320.0,
                padding: 8.0,
                ..ColumnStyle::default()
            },
            |col| {
                cal = col.calendar(2026, 10, 15, CalendarStyle::default());
            },
        );
        assert!(ui.focusable_widgets().contains(&cal));
        assert_eq!(
            ui.calendar_date(cal),
            Some(CalendarDate::new(2026, 10, 15))
        );
        assert_eq!(ui.accessible_name(cal).as_deref(), Some("2026-10-15"));
        ui.sync(&mut scene);
        ui.set_focus(Some(cal));
        assert!(ui.key_input(UiKey::Right));
        assert_eq!(
            ui.take_calendar_change(),
            Some((cal, CalendarDate::new(2026, 10, 16)))
        );
        ui.sync(&mut scene);
        let aux = ui.widgets.get(&cal).unwrap().aux.clone();
        let start = CalendarDate::weekday_sun0(2026, 10, 1) as usize;
        let day1 = aux.get(11 + start).copied().expect("day1 aux");
        let db = scene.get(day1).unwrap().bounds;
        assert!(ui.pointer_pressed(
            &mut scene,
            db.x + db.width * 0.5,
            db.y + db.height * 0.5,
        ));
        assert_eq!(
            ui.take_calendar_change(),
            Some((cal, CalendarDate::new(2026, 10, 1)))
        );
    }

    #[test]
    fn scroll_rows_route_to_nearest_viewport_and_bubble_at_limits() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut row = WidgetId(0);
        let mut button = WidgetId(0);
        let outer = ui.scroll_area(
            ScrollAreaStyle {
                width: 180.0,
                height: 60.0,
                padding: 0.0,
                ..ScrollAreaStyle::default()
            },
            |area| {
                row = area.scroll_row(
                    RowStyle {
                        width: 180.0,
                        height: 32.0,
                        padding: 0.0,
                        ..RowStyle::default()
                    },
                    |r| {
                        button = r.tool_button("first long action");
                        for _ in 0..8 {
                            r.tool_button("more long actions");
                        }
                    },
                );
                for _ in 0..12 {
                    area.text("vertical content");
                }
            },
        );
        ui.sync(&mut scene);
        let b = scene.get(ui.widgets[&button].node.unwrap()).unwrap().bounds;
        assert!(ui.pointer_scroll(&scene, b.x + 2.0, b.y + 2.0, 100.0, 0.0));
        assert_eq!(ui.scroll_offset(row), Some((100.0, 0.0)));
        assert_eq!(ui.scroll_offset(outer), Some((0.0, 0.0)));
        assert!(ui.pointer_scroll(&scene, b.x + 2.0, b.y + 2.0, 0.0, 20.0));
        assert_eq!(ui.scroll_offset(outer), Some((0.0, 20.0)));
        assert!(!ui.set_scroll_offset(outer, f32::NAN, 0.0));
        assert!(ui.set_scroll_offset(outer, 0.0, 0.0));
    }

    #[test]
    fn image_view_preserves_scale_clips_pans_and_rejects_invalid_geometry() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut image = WidgetId(0);
        ui.column(ColumnStyle::default(), |col| {
            image = col.image_rgba(
                400,
                300,
                &vec![255; 400 * 300 * 4],
                ImageStyle {
                    width: 100.0,
                    height: 80.0,
                    ..ImageStyle::default()
                },
            );
        });
        ui.sync(&mut scene);
        let node = ui.widgets[&image].aux[0];
        assert_eq!(scene.get(node).unwrap().bounds.width, 100.0);
        assert!(ui.set_image_view(
            image,
            ImageView {
                scale: Some(1.0),
                ..ImageView::default()
            }
        ));
        ui.sync(&mut scene);
        assert_eq!(scene.get(node).unwrap().bounds.width, 400.0);
        let host = scene.get(ui.widgets[&image].node.unwrap()).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, host.x + 50.0, host.y + 40.0));
        assert!(ui.pointer_moved(host.x + 30.0, host.y + 20.0));
        ui.pointer_released();
        assert_eq!(ui.image_view(image).unwrap().offset_x, 20.0);
        ui.sync(&mut scene);
        let bounds = scene.get(node).unwrap().bounds;
        assert!(scene.get(node).unwrap().clip.is_some());
        assert!(!ui.set_image_view(
            image,
            ImageView {
                scale: Some(f32::NAN),
                ..ImageView::default()
            }
        ));
        ui.sync(&mut scene);
        assert_eq!(scene.get(node).unwrap().bounds, bounds);
        assert!(ui.set_image_view(image, ImageView::default()));
        ui.sync(&mut scene);
        assert_eq!(scene.get(node).unwrap().bounds.width, 100.0);
    }

    #[test]
    fn office_p2e_image_rgba_and_placeholder() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut img = WidgetId(0);
        let mut ph = WidgetId(0);
        let px = [
            255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 400.0,
                height: 240.0,
                padding: 8.0,
                ..ColumnStyle::default()
            },
            |col| {
                img = col.image_rgba_caption(
                    2,
                    2,
                    &px,
                    "checker",
                    ImageStyle {
                        width: 80.0,
                        height: 60.0,
                        mosaic_max: 2,
                        ..ImageStyle::default()
                    },
                );
                ph = col.image_placeholder(
                    "no pixels",
                    ImageStyle {
                        width: 80.0,
                        height: 40.0,
                        ..ImageStyle::default()
                    },
                );
            },
        );
        assert!(!ui.focusable_widgets().contains(&img));
        assert!(!ui.focusable_widgets().contains(&ph));
        assert_eq!(ui.accessible_name(img).as_deref(), Some("checker"));
        assert_eq!(ui.accessible_name(ph).as_deref(), Some("no pixels"));
        ui.sync(&mut scene);
        assert!(ui.widgets.get(&img).unwrap().node.is_some());
        assert!(!ui.widgets.get(&ph).unwrap().aux.is_empty());
        let aux = &ui.widgets.get(&img).unwrap().aux;
        assert_eq!(aux.len(), 2);
        let image_node = scene.get(aux[0]).unwrap();
        assert_eq!(image_node.bounds.width, image_node.bounds.height);
        assert_eq!(image_node.image.as_ref().unwrap().pixels().as_ref(), &px);
        assert!(scene
            .get(ui.widgets.get(&ph).unwrap().aux[0])
            .unwrap()
            .image
            .is_none());
        assert!(image_pixels_rgba8(u32::MAX, u32::MAX, &[]).is_empty());
        assert!(image_pixels_rgba8(2, 2, &[0; 15]).is_empty());
    }

    #[test]
    fn office_p2e_config_provider_bag() {
        let mut ui = Ui::new();
        assert_eq!(ui.config().density, UiDensity::Default);
        ui.set_config(UiConfig {
            locale: "en-US".into(),
            density: UiDensity::Comfortable,
            accent: Some(Color::rgb(0x2f, 0x5b, 0xa8)),
        });
        assert_eq!(ui.config().locale, "en-US");
        assert_eq!(ui.config().density, UiDensity::Comfortable);
        assert_eq!(ui.config().accent.unwrap().r, 0x2f);
    }

    #[test]
    fn theme_defaults_follow_set_theme() {
        let mut ui = Ui::new();
        assert_eq!(ui.theme().mode, ThemeMode::Dark);
        assert_eq!(
            ui.column_style().background,
            Some(Theme::dark().colors.surface)
        );

        ui.set_theme(Theme::light());
        assert_eq!(ui.theme().mode, ThemeMode::Light);
        let col = ui.column_style();
        assert_eq!(col.background, Some(Theme::light().colors.surface));
        let panel = ui.panel_style();
        assert_eq!(panel.title_fg, Theme::light().colors.text_primary);
        assert_eq!(panel.background, Some(Theme::light().colors.panel));

        // Density from UiConfig scales padding.
        ui.set_config(UiConfig {
            locale: "en".into(),
            density: UiDensity::Compact,
            accent: None,
        });
        let compact = ui.column_style();
        assert!(compact.padding < ColumnStyle::from_theme(&Theme::light()).padding);

        // Accent: config overrides theme.
        assert_eq!(ui.resolve_accent(), Theme::light().colors.accent);
        ui.set_config(UiConfig {
            locale: "en".into(),
            density: UiDensity::Default,
            accent: Some(Color::rgb(0xe0, 0x60, 0x60)),
        });
        assert_eq!(ui.resolve_accent().r, 0xe0);
    }

    #[test]
    fn tree_table_hosts_get_theme_panel_radius() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let panel_r = ui.theme().radii.panel;
        let control_r = ui.theme().radii.control;
        let mut tree = WidgetId(0);
        let mut table = WidgetId(0);
        let mut empty = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 480.0,
                height: 420.0,
                ..ColumnStyle::default()
            },
            |col| {
                tree = col.tree(
                    vec![TreeNode::leaf(1, "Root")],
                    TreeStyle {
                        width: 200.0,
                        height: 120.0,
                        ..TreeStyle::default()
                    },
                );
                table = col.table(
                    vec![TableColumn::new("A", 80.0)],
                    vec![vec!["1".into()]],
                    TableStyle {
                        width: 200.0,
                        height: 100.0,
                        ..TableStyle::default()
                    },
                );
                empty = col.empty("nothing", EmptyStyle::default());
            },
        );
        ui.sync(&mut scene);
        // Select a tree row so chrome gets control radius.
        let tnode = ui.widgets.get(&tree).unwrap().node.unwrap();
        let tb = scene.get(tnode).unwrap().bounds;
        assert!(ui.pointer_pressed(&mut scene, tb.x + 12.0, tb.y + 8.0));
        ui.sync(&mut scene);

        let tree_host = ui.widgets.get(&tree).unwrap().node.unwrap();
        assert!(
            (scene.get(tree_host).unwrap().corner_radius - panel_r).abs() < f32::EPSILON,
            "tree host should use panel radius"
        );
        if let Some(chrome) = ui.widgets.get(&tree).unwrap().chrome_node {
            assert!(
                (scene.get(chrome).unwrap().corner_radius - control_r).abs() < f32::EPSILON,
                "tree selection chrome should use control radius"
            );
        } else {
            panic!("expected tree chrome_node");
        }

        let table_host = ui.widgets.get(&table).unwrap().node.unwrap();
        assert!(
            (scene.get(table_host).unwrap().corner_radius - panel_r).abs() < f32::EPSILON,
            "table host should use panel radius"
        );
        let empty_host = ui.widgets.get(&empty).unwrap().node.unwrap();
        assert!(
            (scene.get(empty_host).unwrap().corner_radius - panel_r).abs() < f32::EPSILON,
            "empty host should use panel radius"
        );
    }

    #[test]
    fn unstyled_text_and_button_use_theme() {
        let mut ui = Ui::new();
        ui.set_theme(Theme::light());
        let mut text_id = WidgetId(0);
        let mut btn_id = WidgetId(0);
        let style = ui.column_style();
        ui.column(style, |col| {
            text_id = col.text("hello");
            btn_id = col.button("Go");
        });
        match &ui.widgets.get(&text_id).unwrap().kind {
            Kind::Text { color, size, .. } => {
                assert_eq!(*color, Theme::light().colors.text_primary);
                assert!((*size - Theme::light().typography.body).abs() < 0.01);
            }
            other => panic!("expected Text, got {other:?}"),
        }
        match &ui.widgets.get(&btn_id).unwrap().kind {
            Kind::Button { bg, fg, .. } => {
                assert_eq!(*bg, Theme::light().colors.button_bg);
                assert_eq!(*fg, Theme::light().colors.button_fg);
            }
            other => panic!("expected Button, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod table_selection_tests;

#[cfg(test)]
mod button_tests;

#[cfg(test)]
mod container_render_tests;
