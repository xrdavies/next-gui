//! Office P1 rest styles and shared types (Drawer, virtualization helpers, entry/feedback/structure).

use next_gui_scene::Color;

/// Drawer slide side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrawerPlacement {
    /// Slide from the left edge.
    Left,
    /// Slide from the right edge.
    #[default]
    Right,
}

/// Shared scrim / backdrop look used by Modal and Drawer.
#[derive(Debug, Clone, Copy)]
pub struct BackdropStyle {
    /// Opaque-ish dim fill (Soft MVP has no true alpha blend budget).
    pub scrim: Color,
    /// Close owner when scrim is clicked.
    pub close_on_click: bool,
}

impl Default for BackdropStyle {
    fn default() -> Self {
        Self {
            scrim: Color::rgb(0x0c, 0x0c, 0x10),
            close_on_click: true,
        }
    }
}

/// Style for a side Drawer panel.
#[derive(Debug, Clone, Copy)]
pub struct DrawerStyle {
    /// Panel width.
    pub width: f32,
    /// Title bar height.
    pub title_height: f32,
    /// Content padding.
    pub padding: f32,
    /// Title font size.
    pub title_size: f32,
    /// Body font size.
    pub body_size: f32,
    /// Title text color.
    pub title_fg: Color,
    /// Body text color.
    pub body_fg: Color,
    /// Panel background.
    pub panel_bg: Color,
    /// Title bar background.
    pub title_bg: Color,
    /// Backdrop / scrim.
    pub backdrop: BackdropStyle,
    /// Which edge the drawer anchors to.
    pub placement: DrawerPlacement,
}

impl Default for DrawerStyle {
    fn default() -> Self {
        Self {
            width: 320.0,
            title_height: 40.0,
            padding: 16.0,
            title_size: 15.0,
            body_size: 13.0,
            title_fg: Color::rgb(0xf0, 0xf2, 0xf6),
            body_fg: Color::rgb(0xc0, 0xc8, 0xd0),
            panel_bg: Color::rgb(0x2a, 0x2a, 0x32),
            title_bg: Color::rgb(0x32, 0x32, 0x3c),
            backdrop: BackdropStyle::default(),
            placement: DrawerPlacement::Right,
        }
    }
}

/// Result of a Drawer dismiss; drained by `Ui::take_drawer_action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawerAction {
    /// Escape, title close, or backdrop.
    Dismiss,
}

/// Style for multi-line TextArea.
#[derive(Debug, Clone, Copy)]
pub struct TextAreaStyle {
    /// Field width.
    pub width: f32,
    /// Field height.
    pub height: f32,
    /// Font size.
    pub size: f32,
    /// Line height (px).
    pub line_height: f32,
    /// Text color.
    pub fg: Color,
    /// Placeholder color.
    pub placeholder_fg: Color,
    /// Background.
    pub bg: Color,
    /// Border.
    pub border: Color,
    /// Focused border.
    pub focus_border: Color,
    /// Horizontal padding.
    pub padding_x: f32,
    /// Vertical padding.
    pub padding_y: f32,
}

impl Default for TextAreaStyle {
    fn default() -> Self {
        Self {
            width: 280.0,
            height: 96.0,
            size: 13.0,
            line_height: 18.0,
            fg: Color::rgb(0xe8, 0xec, 0xf0),
            placeholder_fg: Color::rgb(0x78, 0x80, 0x8c),
            bg: Color::rgb(0x1e, 0x1e, 0x22),
            border: Color::rgb(0x4a, 0x4a, 0x54),
            focus_border: Color::rgb(0x3b, 0x82, 0xf6),
            padding_x: 8.0,
            padding_y: 6.0,
        }
    }
}

/// Style for InputNumber stepper.
#[derive(Debug, Clone, Copy)]
pub struct InputNumberStyle {
    /// Total width (field + steppers).
    pub width: f32,
    /// Height.
    pub height: f32,
    /// Font size.
    pub size: f32,
    /// Text color.
    pub fg: Color,
    /// Background.
    pub bg: Color,
    /// Border.
    pub border: Color,
    /// Focus border.
    pub focus_border: Color,
    /// Stepper button background.
    pub step_bg: Color,
    /// Stepper label color.
    pub step_fg: Color,
    /// Horizontal padding inside value field.
    pub padding_x: f32,
}

impl Default for InputNumberStyle {
    fn default() -> Self {
        Self {
            width: 140.0,
            height: 30.0,
            size: 13.0,
            fg: Color::rgb(0xe8, 0xec, 0xf0),
            bg: Color::rgb(0x1e, 0x1e, 0x22),
            border: Color::rgb(0x4a, 0x4a, 0x54),
            focus_border: Color::rgb(0x3b, 0x82, 0xf6),
            step_bg: Color::rgb(0x3a, 0x3a, 0x48),
            step_fg: Color::rgb(0xf0, 0xf2, 0xf6),
            padding_x: 8.0,
        }
    }
}

/// Notification / toast tone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationTone {
    /// Neutral info.
    #[default]
    Info,
    /// Success.
    Success,
    /// Warning.
    Warning,
    /// Error.
    Error,
}

impl NotificationTone {
    /// Accent fill for the toast bar.
    pub fn accent(self) -> Color {
        match self {
            NotificationTone::Info => Color::rgb(0x3b, 0x82, 0xf6),
            NotificationTone::Success => Color::rgb(0x22, 0xc5, 0x5e),
            NotificationTone::Warning => Color::rgb(0xe0, 0xa0, 0x30),
            NotificationTone::Error => Color::rgb(0xe0, 0x4f, 0x5f),
        }
    }
}

/// One queued notification toast.
#[derive(Debug, Clone)]
pub struct NotificationItem {
    /// Stable id for dismiss.
    pub id: u64,
    /// Message text.
    pub message: String,
    /// Tone / accent.
    pub tone: NotificationTone,
}

/// Style for notification toasts.
#[derive(Debug, Clone, Copy)]
pub struct NotificationStyle {
    /// Toast width.
    pub width: f32,
    /// Min height.
    pub height: f32,
    /// Font size.
    pub size: f32,
    /// Text color.
    pub fg: Color,
    /// Panel background.
    pub bg: Color,
    /// Gap between stacked toasts.
    pub gap: f32,
    /// Margin from viewport top-right.
    pub margin: f32,
    /// Max visible toasts.
    pub max_visible: usize,
}

impl Default for NotificationStyle {
    fn default() -> Self {
        Self {
            width: 280.0,
            height: 44.0,
            size: 13.0,
            fg: Color::rgb(0xf0, 0xf2, 0xf6),
            bg: Color::rgb(0x2a, 0x2a, 0x34),
            gap: 8.0,
            margin: 16.0,
            max_visible: 4,
        }
    }
}

/// Style for Spin / loading overlay host.
#[derive(Debug, Clone, Copy)]
pub struct SpinStyle {
    /// Host width.
    pub width: f32,
    /// Host height.
    pub height: f32,
    /// Overlay dim color.
    pub overlay: Color,
    /// Spinner / tip text color.
    pub fg: Color,
    /// Font size for tip.
    pub size: f32,
    /// Tip text when spinning.
    pub tip: &'static str,
}

impl Default for SpinStyle {
    fn default() -> Self {
        Self {
            width: 200.0,
            height: 120.0,
            overlay: Color::rgb(0x14, 0x14, 0x18),
            fg: Color::rgb(0xd0, 0xd4, 0xdc),
            size: 13.0,
            tip: "Loading…",
        }
    }
}

/// Style for Empty state placeholder.
#[derive(Debug, Clone, Copy)]
pub struct EmptyStyle {
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
    /// Description font size.
    pub size: f32,
    /// Description color.
    pub fg: Color,
    /// Background (optional panel feel).
    pub bg: Color,
}

impl Default for EmptyStyle {
    fn default() -> Self {
        Self {
            width: 240.0,
            height: 100.0,
            size: 13.0,
            fg: Color::rgb(0x90, 0x98, 0xa4),
            bg: Color::rgb(0x22, 0x22, 0x28),
        }
    }
}

/// Style for Collapse accordion.
#[derive(Debug, Clone, Copy)]
pub struct CollapseStyle {
    /// Width.
    pub width: f32,
    /// Header row height.
    pub header_height: f32,
    /// Body padding.
    pub padding: f32,
    /// Font size.
    pub size: f32,
    /// Header / body text.
    pub fg: Color,
    /// Header background.
    pub header_bg: Color,
    /// Body background.
    pub body_bg: Color,
    /// Border / separator.
    pub border: Color,
}

impl Default for CollapseStyle {
    fn default() -> Self {
        Self {
            width: 320.0,
            header_height: 32.0,
            padding: 10.0,
            size: 13.0,
            fg: Color::rgb(0xe0, 0xe4, 0xec),
            header_bg: Color::rgb(0x32, 0x32, 0x3c),
            body_bg: Color::rgb(0x28, 0x28, 0x30),
            border: Color::rgb(0x3a, 0x3a, 0x44),
        }
    }
}

/// One Collapse panel section.
#[derive(Debug, Clone)]
pub struct CollapsePanel {
    /// Header title.
    pub title: String,
    /// Body text (MVP; nested children later).
    pub body: String,
    /// Whether expanded.
    pub open: bool,
}

impl CollapsePanel {
    /// Create a panel.
    pub fn new(title: impl Into<String>, body: impl Into<String>, open: bool) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            open,
        }
    }
}

/// Style for Descriptions key-value list.
#[derive(Debug, Clone, Copy)]
pub struct DescriptionsStyle {
    /// Width.
    pub width: f32,
    /// Row height.
    pub row_height: f32,
    /// Label column width.
    pub label_width: f32,
    /// Font size.
    pub size: f32,
    /// Label color.
    pub label_fg: Color,
    /// Value color.
    pub value_fg: Color,
    /// Background.
    pub bg: Color,
    /// Padding.
    pub padding: f32,
}

impl Default for DescriptionsStyle {
    fn default() -> Self {
        Self {
            width: 320.0,
            row_height: 26.0,
            label_width: 100.0,
            size: 12.0,
            label_fg: Color::rgb(0x90, 0x98, 0xa4),
            value_fg: Color::rgb(0xe0, 0xe4, 0xec),
            bg: Color::rgb(0x28, 0x28, 0x30),
            padding: 8.0,
        }
    }
}

/// One Descriptions row.
#[derive(Debug, Clone)]
pub struct DescriptionItem {
    /// Label.
    pub label: String,
    /// Value.
    pub value: String,
}

impl DescriptionItem {
    /// Create an item.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

/// Style for Card surface.
#[derive(Debug, Clone, Copy)]
pub struct CardStyle {
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
    /// Title bar height.
    pub title_height: f32,
    /// Padding.
    pub padding: f32,
    /// Title size.
    pub title_size: f32,
    /// Body size.
    pub body_size: f32,
    /// Title color.
    pub title_fg: Color,
    /// Body color.
    pub body_fg: Color,
    /// Card background.
    pub bg: Color,
    /// Title bar background.
    pub title_bg: Color,
}

impl Default for CardStyle {
    fn default() -> Self {
        Self {
            width: 260.0,
            height: 140.0,
            title_height: 34.0,
            padding: 12.0,
            title_size: 14.0,
            body_size: 12.0,
            title_fg: Color::rgb(0xf0, 0xf2, 0xf6),
            body_fg: Color::rgb(0xb0, 0xb8, 0xc4),
            bg: Color::rgb(0x2a, 0x2a, 0x32),
            title_bg: Color::rgb(0x32, 0x32, 0x3c),
        }
    }
}

/// Style for Segmented control.
#[derive(Debug, Clone, Copy)]
pub struct SegmentedStyle {
    /// Total width.
    pub width: f32,
    /// Height.
    pub height: f32,
    /// Font size.
    pub size: f32,
    /// Unselected text.
    pub fg: Color,
    /// Selected text.
    pub selected_fg: Color,
    /// Track background.
    pub bg: Color,
    /// Selected segment background.
    pub selected_bg: Color,
    /// Padding.
    pub padding: f32,
}

impl Default for SegmentedStyle {
    fn default() -> Self {
        Self {
            width: 240.0,
            height: 30.0,
            size: 12.0,
            fg: Color::rgb(0xb0, 0xb8, 0xc4),
            selected_fg: Color::rgb(0xff, 0xff, 0xff),
            bg: Color::rgb(0x22, 0x22, 0x28),
            selected_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            padding: 2.0,
        }
    }
}

/// Style for Breadcrumb.
#[derive(Debug, Clone, Copy)]
pub struct BreadcrumbStyle {
    /// Width budget.
    pub width: f32,
    /// Height.
    pub height: f32,
    /// Font size.
    pub size: f32,
    /// Link / crumb color.
    pub fg: Color,
    /// Current (last) crumb color.
    pub current_fg: Color,
    /// Separator color.
    pub sep_fg: Color,
}

impl Default for BreadcrumbStyle {
    fn default() -> Self {
        Self {
            width: 400.0,
            height: 28.0,
            size: 12.0,
            fg: Color::rgb(0x6a, 0xb0, 0xf0),
            current_fg: Color::rgb(0xe0, 0xe4, 0xec),
            sep_fg: Color::rgb(0x70, 0x74, 0x80),
        }
    }
}

/// Style for Pagination.
#[derive(Debug, Clone, Copy)]
pub struct PaginationStyle {
    /// Total width.
    pub width: f32,
    /// Height.
    pub height: f32,
    /// Font size.
    pub size: f32,
    /// Text color.
    pub fg: Color,
    /// Button background.
    pub btn_bg: Color,
    /// Active page background.
    pub active_bg: Color,
    /// Active page text.
    pub active_fg: Color,
    /// Gap between controls.
    pub gap: f32,
}

impl Default for PaginationStyle {
    fn default() -> Self {
        Self {
            width: 320.0,
            height: 30.0,
            size: 12.0,
            fg: Color::rgb(0xd0, 0xd4, 0xdc),
            btn_bg: Color::rgb(0x3a, 0x3a, 0x48),
            active_bg: Color::rgb(0x2f, 0x5b, 0xa8),
            active_fg: Color::rgb(0xff, 0xff, 0xff),
            gap: 6.0,
        }
    }
}

/// Visible window helper for virtualized lists/tables.
pub(crate) fn virtual_window(
    scroll_y: f32,
    viewport_h: f32,
    row_h: f32,
    total: usize,
    overscan: usize,
) -> (usize, usize) {
    if total == 0 || row_h <= 0.0 {
        return (0, 0);
    }
    let first = ((scroll_y / row_h).floor() as isize).max(0) as usize;
    let first = first.saturating_sub(overscan).min(total);
    let visible = ((viewport_h / row_h).ceil() as usize).saturating_add(overscan * 2 + 1);
    let count = visible.min(total.saturating_sub(first));
    (first, count)
}
