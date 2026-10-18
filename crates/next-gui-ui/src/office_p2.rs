//! Office P2a styles: `button_group`, `link`, `space`.
//!
//! See `docs/office-p2-design.md`. Cascader and P2b+ widgets stay out of this module.

use next_gui_layout::CrossAlign;
use next_gui_scene::Color;

/// Axis for [`SpaceStyle`] (sugar over [`crate::RowStyle`] / [`crate::ColumnStyle`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpaceDirection {
    /// Horizontal main axis → nested [`crate::Ui::row`] / [`crate::FlexBuilder::row`].
    #[default]
    Horizontal,
    /// Vertical main axis → nested column.
    Vertical,
}

/// Spacing container style (Ant Space / MUI Stack).
///
/// `space(...)` maps this onto [`crate::RowStyle`] or [`crate::ColumnStyle`]; there is
/// no separate retained `Kind`. Prefer explicit `row` / `column` when the axis is fixed
/// in call-site code.
#[derive(Debug, Clone, Copy)]
pub struct SpaceStyle {
    /// Outer x (top-level only; nested placement ignores).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Max width.
    pub width: f32,
    /// Max height.
    pub height: f32,
    /// Inner padding.
    pub padding: f32,
    /// Gap between children.
    pub gap: f32,
    /// Main axis.
    pub direction: SpaceDirection,
    /// Cross-axis alignment.
    pub cross_align: CrossAlign,
    /// Optional fill.
    pub background: Option<Color>,
}

impl Default for SpaceStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 48.0,
            padding: 0.0,
            gap: 12.0,
            direction: SpaceDirection::Horizontal,
            cross_align: CrossAlign::Center,
            background: None,
        }
    }
}

/// Style for exclusive [`crate::FlexBuilder::button_group`].
#[derive(Debug, Clone, Copy)]
pub struct ButtonGroupStyle {
    /// Total width budget.
    pub width: f32,
    /// Height of the strip.
    pub height: f32,
    /// Label font size.
    pub size: f32,
    /// Unselected text.
    pub fg: Color,
    /// Selected text.
    pub selected_fg: Color,
    /// Strip / unselected button background.
    pub bg: Color,
    /// Selected button background.
    pub selected_bg: Color,
    /// Outline between buttons.
    pub border: Color,
    /// Gap between discrete buttons.
    pub gap: f32,
    /// Inner padding of the strip.
    pub padding: f32,
}

impl Default for ButtonGroupStyle {
    fn default() -> Self {
        Self {
            width: 280.0,
            height: 32.0,
            size: 12.0,
            fg: Color::rgb(0xb0, 0xb8, 0xc4),
            selected_fg: Color::rgb(0xff, 0xff, 0xff),
            bg: Color::rgb(0x2a, 0x2a, 0x32),
            selected_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            border: Color::rgb(0x3a, 0x3a, 0x44),
            gap: 4.0,
            padding: 2.0,
        }
    }
}

/// Style for [`crate::FlexBuilder::link`].
#[derive(Debug, Clone, Copy)]
pub struct LinkStyle {
    /// Font size.
    pub size: f32,
    /// Link color (accent).
    pub fg: Color,
    /// Draw a 1px underline under the label.
    pub underline: bool,
}

impl Default for LinkStyle {
    fn default() -> Self {
        Self {
            size: 14.0,
            fg: Color::rgb(0x6a, 0xb0, 0xf0),
            underline: true,
        }
    }
}
