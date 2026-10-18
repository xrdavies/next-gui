//! Office P2 styles: P2a (`button_group` / `link` / `space`), P2b
//! (`steps` / `statistic` / `result`), and P2c (`popover` / `popconfirm`).
//!
//! See `docs/office-p2-design.md`. Cascader and P2d+ widgets stay out of this module.

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

// ── P2b: steps / statistic / result ──────────────────────────────────────────

/// One step in [`crate::FlexBuilder::steps`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepItem {
    /// Short title shown beside the marker.
    pub title: String,
    /// Optional secondary line under the title.
    pub description: Option<String>,
}

impl StepItem {
    /// Title-only step.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: None,
        }
    }

    /// Title + description.
    pub fn with_description(title: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: Some(description.into()),
        }
    }
}

/// Layout axis for [`StepsStyle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StepsDirection {
    /// Left-to-right markers + titles (default).
    #[default]
    Horizontal,
    /// Top-to-bottom stack.
    Vertical,
}

/// Style for [`crate::FlexBuilder::steps`].
#[derive(Debug, Clone, Copy)]
pub struct StepsStyle {
    /// Total width budget.
    pub width: f32,
    /// Total height budget.
    pub height: f32,
    /// Horizontal or vertical layout.
    pub direction: StepsDirection,
    /// When true, pointer on a step title/marker changes `current`.
    pub clickable: bool,
    /// Title font size.
    pub size: f32,
    /// Description font size.
    pub desc_size: f32,
    /// Marker diameter.
    pub marker: f32,
    /// Wait (future) text color.
    pub wait_fg: Color,
    /// Current (process) text color.
    pub process_fg: Color,
    /// Done text color.
    pub done_fg: Color,
    /// Wait marker fill.
    pub wait_bg: Color,
    /// Process marker fill.
    pub process_bg: Color,
    /// Done marker fill.
    pub done_bg: Color,
    /// Connector line between markers.
    pub connector: Color,
    /// Panel background.
    pub bg: Color,
    /// Inner padding.
    pub padding: f32,
    /// Gap between steps (vertical) or extra slack (horizontal).
    pub gap: f32,
}

impl Default for StepsStyle {
    fn default() -> Self {
        Self {
            width: 480.0,
            height: 72.0,
            direction: StepsDirection::Horizontal,
            clickable: true,
            size: 12.0,
            desc_size: 11.0,
            marker: 22.0,
            wait_fg: Color::rgb(0x90, 0x98, 0xa4),
            process_fg: Color::rgb(0xe8, 0xec, 0xf0),
            done_fg: Color::rgb(0xb0, 0xb8, 0xc4),
            wait_bg: Color::rgb(0x3a, 0x3a, 0x44),
            process_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            done_bg: Color::rgb(0x3d, 0x8b, 0x5a),
            connector: Color::rgb(0x4a, 0x4a, 0x54),
            bg: Color::rgba(0, 0, 0, 0),
            padding: 8.0,
            gap: 12.0,
        }
    }
}

/// Style for [`crate::FlexBuilder::statistic`] (KPI block).
#[derive(Debug, Clone, Copy)]
pub struct StatisticStyle {
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
    /// Title font size.
    pub title_size: f32,
    /// Value font size.
    pub value_size: f32,
    /// Hint / delta font size.
    pub hint_size: f32,
    /// Title color.
    pub title_fg: Color,
    /// Value color.
    pub value_fg: Color,
    /// Hint color.
    pub hint_fg: Color,
    /// Background.
    pub bg: Color,
    /// Inner padding.
    pub padding: f32,
}

impl Default for StatisticStyle {
    fn default() -> Self {
        Self {
            width: 160.0,
            height: 88.0,
            title_size: 12.0,
            value_size: 28.0,
            hint_size: 11.0,
            title_fg: Color::rgb(0x90, 0x98, 0xa4),
            value_fg: Color::rgb(0xe8, 0xec, 0xf0),
            hint_fg: Color::rgb(0x6a, 0xc0, 0x8a),
            bg: Color::rgb(0x22, 0x22, 0x28),
            padding: 12.0,
        }
    }
}

/// Status glyph / tone for [`crate::FlexBuilder::result`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResultStatus {
    /// Neutral / informational.
    #[default]
    Info,
    /// Success / OK.
    Success,
    /// Warning.
    Warning,
    /// Error / fault.
    Error,
}

/// Which action button fired on a Result block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultAction {
    /// Primary CTA.
    Primary,
    /// Secondary CTA.
    Secondary,
}

/// Style for [`crate::FlexBuilder::result`].
#[derive(Debug, Clone, Copy)]
pub struct ResultStyle {
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
    /// Title font size.
    pub title_size: f32,
    /// Subtitle font size.
    pub subtitle_size: f32,
    /// Status label font size (glyph substitute).
    pub status_size: f32,
    /// Action button font size.
    pub action_size: f32,
    /// Action button height.
    pub action_height: f32,
    /// Title color.
    pub title_fg: Color,
    /// Subtitle color.
    pub subtitle_fg: Color,
    /// Background.
    pub bg: Color,
    /// Primary button fill.
    pub primary_bg: Color,
    /// Primary button text.
    pub primary_fg: Color,
    /// Secondary button fill.
    pub secondary_bg: Color,
    /// Secondary button text.
    pub secondary_fg: Color,
    /// Inner padding.
    pub padding: f32,
    /// Gap between action buttons.
    pub gap: f32,
}

impl Default for ResultStyle {
    fn default() -> Self {
        Self {
            width: 360.0,
            height: 200.0,
            title_size: 18.0,
            subtitle_size: 13.0,
            status_size: 14.0,
            action_size: 13.0,
            action_height: 32.0,
            title_fg: Color::rgb(0xe8, 0xec, 0xf0),
            subtitle_fg: Color::rgb(0x90, 0x98, 0xa4),
            bg: Color::rgb(0x22, 0x22, 0x28),
            primary_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            primary_fg: Color::rgb(0xff, 0xff, 0xff),
            secondary_bg: Color::rgb(0x3a, 0x3a, 0x44),
            secondary_fg: Color::rgb(0xd0, 0xd4, 0xdc),
            padding: 20.0,
            gap: 12.0,
        }
    }
}

impl ResultStatus {
    /// Short Soft-friendly label drawn above the title.
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Success => "SUCCESS",
            Self::Warning => "WARNING",
            Self::Error => "ERROR",
        }
    }

    /// Accent color for the status label.
    pub fn color(self) -> Color {
        match self {
            Self::Info => Color::rgb(0x6a, 0xb0, 0xf0),
            Self::Success => Color::rgb(0x6a, 0xc0, 0x8a),
            Self::Warning => Color::rgb(0xe0, 0xb0, 0x40),
            Self::Error => Color::rgb(0xe0, 0x60, 0x60),
        }
    }
}

// ── P2c: popover / popconfirm ────────────────────────────────────────────────

/// Style for [`crate::FlexBuilder::popover`] (click-toggle bubble below host).
#[derive(Debug, Clone, Copy)]
pub struct PopoverStyle {
    /// Trigger button width.
    pub width: f32,
    /// Trigger button height.
    pub height: f32,
    /// Trigger label font size.
    pub size: f32,
    /// Popup title font size.
    pub title_size: f32,
    /// Popup body font size.
    pub body_size: f32,
    /// Max popup width (text not wrapped in MVP).
    pub max_width: f32,
    /// Inner padding of the popup panel.
    pub padding: f32,
    /// Gap between host bottom and popup top.
    pub gap: f32,
    /// Trigger text color.
    pub trigger_fg: Color,
    /// Trigger fill.
    pub trigger_bg: Color,
    /// Popup panel fill.
    pub popup_bg: Color,
    /// Title text color.
    pub title_fg: Color,
    /// Body text color.
    pub body_fg: Color,
}

impl Default for PopoverStyle {
    fn default() -> Self {
        Self {
            width: 120.0,
            height: 32.0,
            size: 13.0,
            title_size: 13.0,
            body_size: 12.0,
            max_width: 280.0,
            padding: 10.0,
            gap: 6.0,
            trigger_fg: Color::rgb(0xe8, 0xec, 0xf0),
            trigger_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            popup_bg: Color::rgb(0x2a, 0x2a, 0x34),
            title_fg: Color::rgb(0xf0, 0xf2, 0xf6),
            body_fg: Color::rgb(0xc0, 0xc4, 0xcc),
        }
    }
}

/// Which action closed a [`crate::FlexBuilder::popconfirm`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopconfirmAction {
    /// OK / Confirm button.
    Confirm,
    /// Cancel button.
    Cancel,
    /// Escape, outside click, or host toggle-close while open.
    Dismiss,
}

/// Style for [`crate::FlexBuilder::popconfirm`] (confirm bubble below host).
#[derive(Debug, Clone, Copy)]
pub struct PopconfirmStyle {
    /// Trigger button width.
    pub width: f32,
    /// Trigger button height.
    pub height: f32,
    /// Trigger / message / button font size.
    pub size: f32,
    /// Max popup width.
    pub max_width: f32,
    /// Inner padding of the popup panel.
    pub padding: f32,
    /// Gap between host bottom and popup top.
    pub gap: f32,
    /// Action button height.
    pub action_height: f32,
    /// Gap between OK and Cancel.
    pub action_gap: f32,
    /// Trigger text color.
    pub trigger_fg: Color,
    /// Trigger fill.
    pub trigger_bg: Color,
    /// Popup panel fill.
    pub popup_bg: Color,
    /// Message text color.
    pub message_fg: Color,
    /// Confirm button fill.
    pub ok_bg: Color,
    /// Confirm button text.
    pub ok_fg: Color,
    /// Cancel button fill.
    pub cancel_bg: Color,
    /// Cancel button text.
    pub cancel_fg: Color,
}

impl Default for PopconfirmStyle {
    fn default() -> Self {
        Self {
            width: 120.0,
            height: 32.0,
            size: 13.0,
            max_width: 280.0,
            padding: 12.0,
            gap: 6.0,
            action_height: 28.0,
            action_gap: 8.0,
            trigger_fg: Color::rgb(0xe8, 0xec, 0xf0),
            trigger_bg: Color::rgb(0xa0, 0x40, 0x40),
            popup_bg: Color::rgb(0x2a, 0x2a, 0x34),
            message_fg: Color::rgb(0xe0, 0xe4, 0xec),
            ok_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            ok_fg: Color::rgb(0xff, 0xff, 0xff),
            cancel_bg: Color::rgb(0x3a, 0x3a, 0x44),
            cancel_fg: Color::rgb(0xd0, 0xd4, 0xdc),
        }
    }
}
