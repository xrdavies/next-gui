//! Office P2 styles: P2a–P2e (`button_group` … `config_provider` types).
//!
//! See `docs/office-p2-design.md`. Cascader stays optional / out of this module.
//! [`UiConfig`] is a Ui-level bag (not a visual `Kind`).

use std::sync::Arc;

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

// ── P2d: upload / color_picker / autocomplete ────────────────────────────────

/// Style for [`crate::FlexBuilder::upload`] (path + Browse; OS dialog is app-owned).
#[derive(Debug, Clone, Copy)]
pub struct UploadStyle {
    /// Total width.
    pub width: f32,
    /// Height of the control.
    pub height: f32,
    /// Path / button font size.
    pub size: f32,
    /// Path text color.
    pub fg: Color,
    /// Placeholder when no path.
    pub placeholder_fg: Color,
    /// Field background.
    pub bg: Color,
    /// Browse button fill.
    pub browse_bg: Color,
    /// Browse button text.
    pub browse_fg: Color,
    /// Browse button width.
    pub browse_width: f32,
    /// Clear button fill.
    pub clear_bg: Color,
    /// Clear button text.
    pub clear_fg: Color,
    /// Clear button width.
    pub clear_width: f32,
    /// Inner padding of the path area.
    pub padding: f32,
    /// Gap between path and buttons.
    pub gap: f32,
    /// Show Clear when a path is set.
    pub show_clear: bool,
}

impl Default for UploadStyle {
    fn default() -> Self {
        Self {
            width: 360.0,
            height: 32.0,
            size: 13.0,
            fg: Color::rgb(0xe0, 0xe4, 0xec),
            placeholder_fg: Color::rgb(0x70, 0x74, 0x80),
            bg: Color::rgb(0x2a, 0x2a, 0x32),
            browse_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            browse_fg: Color::rgb(0xff, 0xff, 0xff),
            browse_width: 72.0,
            clear_bg: Color::rgb(0x3a, 0x3a, 0x44),
            clear_fg: Color::rgb(0xd0, 0xd4, 0xdc),
            clear_width: 56.0,
            padding: 8.0,
            gap: 6.0,
            show_clear: true,
        }
    }
}

/// Style for [`crate::FlexBuilder::color_picker`] (preset swatches + hex field).
#[derive(Debug, Clone, Copy)]
pub struct ColorPickerStyle {
    /// Total width budget.
    pub width: f32,
    /// Row height.
    pub height: f32,
    /// Swatch square size.
    pub swatch_size: f32,
    /// Gap between swatches.
    pub swatch_gap: f32,
    /// Hex text field width.
    pub hex_width: f32,
    /// Hex font size.
    pub size: f32,
    /// Hex text color.
    pub fg: Color,
    /// Hex field background.
    pub hex_bg: Color,
    /// Panel / host background.
    pub bg: Color,
    /// Selected swatch outline.
    pub selected_border: Color,
    /// Inner padding.
    pub padding: f32,
}

impl Default for ColorPickerStyle {
    fn default() -> Self {
        Self {
            width: 360.0,
            height: 36.0,
            swatch_size: 22.0,
            swatch_gap: 4.0,
            hex_width: 88.0,
            size: 12.0,
            fg: Color::rgb(0xe0, 0xe4, 0xec),
            hex_bg: Color::rgb(0x1e, 0x1e, 0x22),
            bg: Color::rgba(0, 0, 0, 0),
            selected_border: Color::rgb(0xf0, 0xf2, 0xf6),
            padding: 4.0,
        }
    }
}

/// Default Soft-friendly preset palette for [`crate::FlexBuilder::color_picker`].
pub fn default_color_presets() -> Vec<Color> {
    vec![
        Color::rgb(0xe0, 0x60, 0x60),
        Color::rgb(0xe0, 0xa0, 0x40),
        Color::rgb(0xe0, 0xd0, 0x40),
        Color::rgb(0x6a, 0xc0, 0x8a),
        Color::rgb(0x40, 0xc0, 0xc0),
        Color::rgb(0x6a, 0xb0, 0xf0),
        Color::rgb(0x2f, 0x5b, 0xa8),
        Color::rgb(0xa0, 0x70, 0xd0),
        Color::rgb(0xf0, 0xf0, 0xf4),
        Color::rgb(0x90, 0x98, 0xa4),
        Color::rgb(0x2a, 0x2a, 0x32),
        Color::rgb(0x10, 0x10, 0x14),
    ]
}

/// Format `Color` as `#RRGGBB` (alpha ignored).
pub fn color_to_hex(c: Color) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

/// Parse `#RRGGBB` / `RRGGBB` (case-insensitive). Returns `None` on failure.
pub fn parse_hex_color(s: &str) -> Option<Color> {
    let t = s.trim();
    let hex = t.strip_prefix('#').unwrap_or(t);
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::rgb(r, g, b))
}

/// Style for [`crate::FlexBuilder::autocomplete`] (text field + filtered dropdown).
#[derive(Debug, Clone, Copy)]
pub struct AutocompleteStyle {
    /// Closed field width.
    pub width: f32,
    /// Closed field height.
    pub height: f32,
    /// Dropdown max height.
    pub popup_max_height: f32,
    /// Suggestion row height.
    pub item_height: f32,
    /// Inner padding.
    pub padding: f32,
    /// Font size.
    pub size: f32,
    /// Value text.
    pub fg: Color,
    /// Placeholder text.
    pub placeholder_fg: Color,
    /// Field background.
    pub bg: Color,
    /// Popup background.
    pub popup_bg: Color,
    /// Highlighted suggestion row.
    pub selected_bg: Color,
    /// Focus border hint.
    pub focus_border: Color,
}

impl Default for AutocompleteStyle {
    fn default() -> Self {
        Self {
            width: 240.0,
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

// ── P2e: grid / timeline / calendar / image / config ─────────────────────────

/// Style for [`crate::FlexBuilder::grid`] (fixed-column CSS-grid-lite).
#[derive(Debug, Clone, Copy)]
pub struct GridStyle {
    /// Outer x (top-level only).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Max width of the grid host.
    pub width: f32,
    /// Max height of the grid host.
    pub height: f32,
    /// Inner padding.
    pub padding: f32,
    /// Gap between cells (row and column).
    pub gap: f32,
    /// Optional fill.
    pub background: Option<Color>,
}

impl Default for GridStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 200.0,
            padding: 8.0,
            gap: 8.0,
            background: None,
        }
    }
}

/// One entry in [`crate::FlexBuilder::timeline`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineItem {
    /// Time / label column (left).
    pub time: String,
    /// Content string (right of the rail).
    pub content: String,
    /// Optional rail/dot color override as packed RGB (`0xRRGGBB`); `None` = style default.
    pub color_rgb: Option<u32>,
}

impl TimelineItem {
    /// Time + content.
    pub fn new(time: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            time: time.into(),
            content: content.into(),
            color_rgb: None,
        }
    }

    /// With optional RGB accent for the dot.
    pub fn with_color(mut self, rgb: u32) -> Self {
        self.color_rgb = Some(rgb);
        self
    }
}

/// Style for [`crate::FlexBuilder::timeline`].
#[derive(Debug, Clone, Copy)]
pub struct TimelineStyle {
    /// Total width.
    pub width: f32,
    /// Total height budget.
    pub height: f32,
    /// Row height per item.
    pub item_height: f32,
    /// Time column width.
    pub time_width: f32,
    /// Font size.
    pub size: f32,
    /// Time text color.
    pub time_fg: Color,
    /// Content text color.
    pub content_fg: Color,
    /// Default rail / dot color.
    pub rail: Color,
    /// Panel background.
    pub bg: Color,
    /// Inner padding.
    pub padding: f32,
    /// When true, pointer on a row selects it (`take_timeline_select`).
    pub clickable: bool,
}

impl Default for TimelineStyle {
    fn default() -> Self {
        Self {
            width: 360.0,
            height: 160.0,
            item_height: 36.0,
            time_width: 72.0,
            size: 12.0,
            time_fg: Color::rgb(0x90, 0x98, 0xa4),
            content_fg: Color::rgb(0xe0, 0xe4, 0xec),
            rail: Color::rgb(0x2f, 0x5b, 0xa8),
            bg: Color::rgb(0x2a, 0x2a, 0x32),
            padding: 8.0,
            clickable: true,
        }
    }
}

/// Civil date for [`crate::FlexBuilder::calendar`] (no time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarDate {
    /// Year (e.g. 2026).
    pub year: i32,
    /// Month 1..=12.
    pub month: u8,
    /// Day 1..=31.
    pub day: u8,
}

impl CalendarDate {
    /// Construct (clamps month/day roughly; day clamped via [`Self::clamp`]).
    pub fn new(year: i32, month: u8, day: u8) -> Self {
        Self {
            year,
            month: month.clamp(1, 12),
            day: day.clamp(1, 31),
        }
        .clamp()
    }

    /// Clamp day to month length (Gregorian).
    pub fn clamp(self) -> Self {
        let max = match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                let leap = (self.year % 4 == 0 && self.year % 100 != 0) || (self.year % 400 == 0);
                if leap { 29 } else { 28 }
            }
            _ => 30,
        };
        Self {
            day: self.day.clamp(1, max),
            ..self
        }
    }

    /// Weekday with **Sunday = 0** … Saturday = 6 (Sakamoto).
    pub fn weekday_sun0(year: i32, month: u8, day: u8) -> u8 {
        let mut y = year;
        let t = [0i32, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        if month < 3 {
            y -= 1;
        }
        let dow = (y + y / 4 - y / 100 + y / 400 + t[month as usize - 1] + day as i32).rem_euclid(7);
        dow as u8
    }
}

/// Style for [`crate::FlexBuilder::calendar`].
///
/// Week rows start on **Sunday** (Sun–Sat).
#[derive(Debug, Clone, Copy)]
pub struct CalendarStyle {
    /// Total width.
    pub width: f32,
    /// Total height.
    pub height: f32,
    /// Day cell size.
    pub cell: f32,
    /// Header / day font size.
    pub size: f32,
    /// Text color.
    pub fg: Color,
    /// Selected day accent (overridden by [`crate::UiConfig::accent`] when set).
    pub accent: Color,
    /// Panel background.
    pub bg: Color,
    /// Header / chrome background.
    pub header_bg: Color,
    /// Inner padding.
    pub padding: f32,
}

impl Default for CalendarStyle {
    fn default() -> Self {
        Self {
            width: 280.0,
            height: 260.0,
            cell: 32.0,
            size: 12.0,
            fg: Color::rgb(0xe0, 0xe4, 0xec),
            accent: Color::rgb(0x2f, 0x5b, 0xa8),
            bg: Color::rgb(0x2a, 0x2a, 0x32),
            header_bg: Color::rgb(0x22, 0x22, 0x28),
            padding: 8.0,
        }
    }
}

/// Style for [`crate::FlexBuilder::image_rgba`] / [`crate::FlexBuilder::image_placeholder`].
#[derive(Debug, Clone, Copy)]
pub struct ImageStyle {
    /// Display width of the host.
    pub width: f32,
    /// Display height of the host (caption sits inside when present).
    pub height: f32,
    /// Placeholder / letterbox fill.
    pub bg: Color,
    /// Caption text color.
    pub caption_fg: Color,
    /// Caption font size.
    pub caption_size: f32,
    /// Max mosaic cells on the long edge when painting RGBA via Soft fill rects
    /// (no scene RGBA texture yet).
    pub mosaic_max: u32,
}

impl Default for ImageStyle {
    fn default() -> Self {
        Self {
            width: 160.0,
            height: 100.0,
            bg: Color::rgb(0x1e, 0x1e, 0x24),
            caption_fg: Color::rgb(0x90, 0x98, 0xa4),
            caption_size: 11.0,
            mosaic_max: 24,
        }
    }
}

/// Owned RGBA8 pixels for [`crate::FlexBuilder::image_rgba`] (`width * height * 4` bytes).
pub type ImagePixels = Arc<[u8]>;

/// Copy RGBA8 bytes into an [`ImagePixels`] arc (truncates to `w*h*4` if longer).
pub fn image_pixels_rgba8(width: u32, height: u32, pixels: &[u8]) -> ImagePixels {
    let need = (width as usize).saturating_mul(height as usize).saturating_mul(4);
    let slice = if pixels.len() >= need {
        &pixels[..need]
    } else {
        pixels
    };
    let mut buf = vec![0u8; need];
    let n = slice.len().min(need);
    buf[..n].copy_from_slice(&slice[..n]);
    Arc::from(buf)
}

/// Density hint for [`UiConfig`] (theme entry point; widgets may ignore in MVP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiDensity {
    /// Tighter spacing.
    Compact,
    /// Default spacing.
    #[default]
    Default,
    /// Looser spacing.
    Comfortable,
}

/// Ui-level defaults bag (Office P2e `config_provider` — **not** a visual `Kind`).
///
/// Set via [`crate::Ui::set_config`] / read via [`crate::Ui::config`].
/// Appearance colors/type/spacing live in [`crate::Theme`] ([`crate::Ui::set_theme`]);
/// this bag holds locale, density (scales theme spacing/fonts), and optional accent override.
#[derive(Debug, Clone, PartialEq)]
pub struct UiConfig {
    /// Locale tag (BCP 47-ish string; display / formatting hint).
    pub locale: String,
    /// Density preference.
    pub density: UiDensity,
    /// Optional accent color (e.g. calendar selection).
    pub accent: Option<Color>,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            locale: "en".into(),
            density: UiDensity::Default,
            accent: None,
        }
    }
}
