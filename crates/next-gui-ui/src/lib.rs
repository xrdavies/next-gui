//! Compact declarative UI API for next-gui.
//!
//! Retained widget tree built with a short builder or loaded from a compact `.ngui` file,
//! synced into [`next_gui_scene::Scene`]. Text uses [`next_gui_text`] (font metrics + CJK).
//!
//! Office P0 shell + input + data + nav + feedback widgets: Icon/Toolbar/Panel/StatusBar/Splitter,
//! ScrollArea, TextInput, Checkbox, Radio, Switch, List, Tree, Table,
//! Menu, ContextMenu, Tabs, Select, Slider,
//! Tag, Badge, Tooltip, Modal, Progress, Alert.

#![deny(missing_docs)]

mod dsl;
mod icon;

pub use dsl::{load_ngui, load_ngui_path, DslError, LoadedUi};
pub use icon::IconName;

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
    /// Window viewport for Modal scrim (set via [`Ui::set_viewport`]).
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
    /// Printable character (ASCII / Latin). IME/CJK composition is not handled yet.
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
    /// Tab.
    Tab,
    /// Space (toggle focused checkbox/switch/radio).
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
            overlay_host: None,
            tooltip_hover: None,
            pending_modal_action: None,
            pending_tag_close: None,
            pending_alert_dismiss: None,
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
        self.widgets.insert(
            id,
            Widget {
                kind,
                node: None,
                label_node: None,
                chrome_node: None,
            aux: Vec::new(),
            },
        );
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
                return true;
            }
            Some(Kind::Checkbox { .. }) => {
                self.focus = Some(id);
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Checkbox { checked, .. } = &mut w.kind {
                        *checked = !*checked;
                    }
                }
                return true;
            }
            Some(Kind::Radio { .. }) => {
                self.focus = Some(id);
                self.select_radio(id);
                return true;
            }
            Some(Kind::Switch { .. }) => {
                self.focus = Some(id);
                if let Some(w) = self.widgets.get_mut(&id) {
                    if let Kind::Switch { on, .. } = &mut w.kind {
                        *on = !*on;
                    }
                }
                return true;
            }
            Some(Kind::List { .. }) => {
                self.focus = Some(id);
                return self.pointer_list(scene, id, x, y);
            }
            Some(Kind::Tree { .. }) => {
                self.focus = Some(id);
                return self.pointer_tree(scene, id, x, y);
            }
            Some(Kind::Table { .. }) => {
                self.focus = Some(id);
                return self.pointer_table(scene, id, x, y);
            }
            Some(Kind::Menu { .. }) => {
                self.focus = Some(id);
                return self.pointer_menu_title(scene, id);
            }
            Some(Kind::ContextMenu { .. }) => {
                // Left click on host: no menu; children handle via hit.
                self.focus = None;
                return false;
            }
            Some(Kind::Tabs { .. }) => {
                self.focus = Some(id);
                return self.pointer_tabs(scene, id, x, y);
            }
            Some(Kind::Select { .. }) => {
                self.focus = Some(id);
                return self.pointer_select_toggle(scene, id);
            }
            Some(Kind::Slider { .. }) => {
                self.focus = Some(id);
                return self.pointer_slider(scene, id, x, y);
            }
            Some(Kind::Tag { closable: true, .. }) => {
                self.focus = None;
                return self.pointer_tag_close(scene, id, x, y);
            }
            Some(Kind::Alert {
                dismissible: true,
                dismissed: false,
                ..
            }) => {
                self.focus = None;
                return self.pointer_alert_dismiss(scene, id, x, y);
            }
            Some(Kind::Button { .. }) => {
                self.focus = None;
                return false; // let app handle button clicks via hit_widget
            }
            _ => {
                self.focus = None;
                return false;
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
                    | Kind::Tooltip { children, .. } => children.as_slice(),
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
                ..
            } = &mut w.kind
            {
                *v = value.into();
                *caret = v.chars().count();
                *select_all = false;
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
            Some(Kind::List { .. } | Kind::Tree { .. } | Kind::Table { .. } | Kind::ScrollArea { .. })
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
                | Kind::Table { scroll_y, .. } => {
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
                if matches!(
                    self.widgets.get(&id).map(|w| &w.kind),
                    Some(Kind::Modal { .. })
                ) {
                    self.pending_modal_action = Some((id, ModalAction::Dismiss));
                }
            }
            return self.close_popup_internal();
        }
        let Some(id) = self.focus else {
            return false;
        };
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::TextInput { .. }) => self.key_text_input(id, key),
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
            Some(Kind::Slider { .. }) => self.key_slider(id, key),
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
            ..
        } = &mut w.kind
        else {
            return false;
        };
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
            UiKey::Enter | UiKey::Tab | UiKey::Space | UiKey::Up | UiKey::Down | UiKey::Escape => false,
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
        let mut changed = false;
        if idx >= 0 && (idx as usize) < n_items {
            if let Some(w) = self.widgets.get_mut(&id) {
                if let Kind::List { selected, .. } = &mut w.kind {
                    let new = Some(idx as usize);
                    if *selected != new {
                        *selected = new;
                        changed = true;
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
        changed || true
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
                | Kind::Modal { open, .. } => {
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
                | Kind::Tooltip { children, .. } => children.as_slice(),
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
        if matches!(w.kind, Kind::Modal { open: true, .. }) {
            // Modal owns the whole viewport while open.
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
        // Click on title/host while open → toggle close (Menu/Select).
        if let Some(w) = self.widgets.get(&id) {
            if let Some(node) = w.node {
                if let Some(n) = scene.get(node) {
                    if n.bounds.contains(x, y) {
                        match &w.kind {
                            Kind::Menu { .. } => return self.pointer_menu_title(scene, id),
                            Kind::Select { .. } => return self.pointer_select_toggle(scene, id),
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
        let changed = self.apply_slider_x(id, track, x);
        self.drag = Some(DragState::Slider { slider: id, track });
        changed || true
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
                | Kind::Tooltip { children, .. } => children.as_slice(),
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
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                        w.chrome_node = Some(caret);
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
                    | Kind::Tooltip { children, .. },
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
            None => ChildMeasure::new(0.0, 0.0),
        }
    }

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
                _ => {}
            }
        }
    }

    fn paint_widget(&mut self, scene: &mut Scene, id: WidgetId) {
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
                | Kind::Tooltip { children, .. },
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
                style,
            } => {
                let value = value.clone();
                let placeholder = placeholder.clone();
                let caret = *caret;
                let select_all = *select_all;
                let style = *style;
                let focused = self.focus == Some(id);
                let border = if focused {
                    style.focus_border
                } else {
                    style.border
                };
                scene.set_fill(node, style.bg);
                scene.set_mask(node, None);
                // Focus ring: chrome caret node also reused; draw border as slightly larger? keep bg only.
                let _ = border;
                if focused {
                    scene.set_fill(node, style.bg);
                }
                let show = if value.is_empty() {
                    (placeholder.as_str(), style.placeholder_fg)
                } else {
                    (value.as_str(), style.fg)
                };
                let (_, mask) = self.font.rasterize_text(show.0, style.size, None);
                if let Some(lid) = label_node {
                    let lx = bounds.x + style.padding_x;
                    let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
                    scene.set_bounds(
                        lid,
                        Rect::new(lx, ly, mask.width as f32, mask.height as f32),
                    );
                    scene.set_fill(lid, show.1);
                    if select_all && !value.is_empty() {
                        // Selection: tint label bg via host already; use brighter fg.
                        scene.set_fill(lid, Color::rgb(0xff, 0xff, 0xff));
                    }
                    scene.set_mask(
                        lid,
                        Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                    );
                    scene.set_visible(lid, true);
                }
                if let Some(cid) = self.widgets.get(&id).and_then(|w| w.chrome_node) {
                    if focused {
                        let prefix: String = value.chars().take(caret).collect();
                        let (cw, _) = self.font.measure_line(&prefix, style.size);
                        let cx = bounds.x + style.padding_x + cw;
                        let cy = bounds.y + 4.0;
                        let ch = (bounds.height - 8.0).max(8.0);
                        scene.set_bounds(cid, Rect::new(cx, cy, 2.0, ch));
                        scene.set_fill(cid, style.focus_border);
                        scene.set_mask(cid, None);
                        scene.set_visible(cid, true);
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
            | Kind::Alert { .. } => {}
        }
    }


    fn paint_list(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::List {
                    items,
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
        let items = items.clone();
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

        let focused = self.focus == Some(id);
        let _ = (focused, style.focus_border);

        let needed = items.len() * 2;
        self.ensure_aux_nodes(scene, host, id, needed);
        let aux = self.widgets.get(&id).map(|w| w.aux.clone()).unwrap_or_default();

        // selection chrome
        if let Some(chrome) = chrome {
            if let Some(sel) = selected {
                let y = bounds.y + style.padding + sel as f32 * style.item_height - scroll_y;
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
        }

        for (i, text) in items.iter().enumerate() {
            let y = bounds.y + style.padding + i as f32 * style.item_height - scroll_y;
            let row = Rect::new(
                bounds.x + style.padding,
                y,
                (bounds.width - style.padding * 2.0).max(1.0),
                style.item_height,
            );
            // skip far offscreen
            if row.y + row.height < bounds.y - style.item_height
                || row.y > bounds.y + bounds.height + style.item_height
            {
                if let Some(&bg) = aux.get(i * 2) {
                    scene.set_visible(bg, false);
                }
                if let Some(&lab) = aux.get(i * 2 + 1) {
                    scene.set_visible(lab, false);
                }
                continue;
            }
            let is_sel = selected == Some(i);
            if let Some(&bg) = aux.get(i * 2) {
                // row bg only when not using chrome selection (keep transparent)
                scene.set_visible(bg, false);
                let _ = bg;
            }
            let (_, mask) = self.font.rasterize_text(text, style.size, None);
            if let Some(&lab) = aux.get(i * 2 + 1) {
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
        // aux layout: [header texts x ncols] then per row: [row_bg + cell texts x ncols]
        let needed = ncols + rows.len() * (1 + ncols);
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
        for (ri, row) in rows.iter().enumerate() {
            let base = ncols + ri * (1 + ncols);
            let y = body_top + ri as f32 * style.row_height - scroll_y;
            if y + style.row_height < body_top - style.row_height
                || y > bounds.y + bounds.height + style.row_height
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

    fn paint_open_popups(&mut self, scene: &mut Scene) {
        let Some(id) = self.open_popup else {
            // Hide any leftover popup aux on closed widgets — walk menus/selects/ctx/modal
            for wid in self.widgets.keys().copied().collect::<Vec<_>>() {
                if matches!(
                    self.widgets.get(&wid).map(|w| &w.kind),
                    Some(Kind::Menu { open: false, .. })
                        | Some(Kind::ContextMenu { open: false, .. })
                        | Some(Kind::Select { open: false, .. })
                        | Some(Kind::Modal { open: false, .. })
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
            Some(Kind::Modal { .. }) => self.paint_modal_popup(scene, id),
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
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
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
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
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
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
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
                | Kind::Tooltip { children, .. } => {
                    children.push(child);
                }
                _ => {}
            }
        }
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
}
