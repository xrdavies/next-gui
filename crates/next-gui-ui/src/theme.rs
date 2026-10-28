//! Theme tokens: colors, typography, spacing, radii.
//!
//! Profile (Field/Office) stays performance/capability. Appearance lives here and in
//! [`crate::UiConfig`] (locale / density / accent override).
//!
//! Light + dark presets; every widget `XxxStyle` has `from_theme` /
//! `from_theme_with_density`. [`crate::Ui::set_theme`] + `Ui::*_style()` helpers;
//! unstyled constructors read the active theme. Explicit Style args still win.
//! [`Default`] stays historical hardcodes (≈ dark) for compat — see `docs/theme-tokens.md`.

use next_gui_layout::{Axis, CrossAlign};
use next_gui_scene::Color;

use crate::{
    AlarmListStyle, AlertStyle, AutocompleteStyle, BackdropStyle, BadgeStyle, BreadcrumbStyle,
    ButtonGroupStyle, CalendarStyle, CardStyle, ChartStyle, CheckboxStyle, CollapseStyle,
    ColorPickerStyle, ColumnStyle, ContextMenuStyle, DateTimeStyle, DescriptionsStyle,
    DrawerPlacement, DrawerStyle, EmptyStyle, FormStyle, GaugeStyle, GridStyle, ImageStyle,
    InputNumberStyle, LinkStyle, ListStyle, MenuStyle, ModalStyle, NotificationStyle,
    PaginationStyle, PanelStyle, PopconfirmStyle, PopoverStyle, ProgressStyle, PropertyGridStyle,
    RadioStyle, ResultStyle, RowStyle, ScrollAreaStyle, SegmentedStyle, SelectStyle, SliderStyle,
    SpaceDirection, SpaceStyle, SpinStyle, SplitterStyle, StatisticStyle, StatusBarStyle,
    StepsDirection, StepsStyle, SwitchStyle, TableStyle, TabsStyle, TagStyle, TextAreaStyle,
    TextInputStyle, TimelineStyle, ToolbarStyle, TooltipStyle, TransferStyle, TreeSelectStyle,
    TreeStyle, TreeTableStyle, TrendStyle, UiDensity, UploadStyle,
};

/// Light vs dark appearance mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThemeMode {
    /// Light surfaces, dark text.
    Light,
    /// Dark surfaces, light text (matches historical widget [`Default`]s).
    #[default]
    Dark,
}

/// Semantic color tokens used by style `from_theme` helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeColors {
    /// Page / column surface.
    pub surface: Color,
    /// Elevated strip (row / toolbar chrome).
    pub surface_elevated: Color,
    /// Panel body fill.
    pub panel: Color,
    /// Panel / card title bar.
    pub title_bar: Color,
    /// Primary text / labels.
    pub text_primary: Color,
    /// Secondary / muted text.
    pub text_secondary: Color,
    /// Default button fill.
    pub button_bg: Color,
    /// Default button label.
    pub button_fg: Color,
    /// Brand / selection accent.
    pub accent: Color,
    /// Text painted on accent fills.
    pub on_accent: Color,
    /// Borders and chrome lines.
    pub border: Color,
    /// Focus ring.
    pub focus: Color,
    /// Input field background.
    pub input_bg: Color,
    /// Placeholder text.
    pub placeholder: Color,
    /// Link color.
    pub link: Color,
    /// Status / muted bar fill.
    pub muted: Color,
    /// Disabled / separator text.
    pub disabled: Color,
    /// Popup / menu / tooltip panel.
    pub popup: Color,
    /// List / tree / table body.
    pub list_bg: Color,
    /// Table / transfer header.
    pub header_bg: Color,
    /// Active chrome (tab active, secondary buttons).
    pub chrome: Color,
    /// Selected row / item background.
    pub selected_bg: Color,
    /// Hover / open menu background.
    pub hover_bg: Color,
    /// Slider / progress track.
    pub track: Color,
    /// Switch on track.
    pub track_on: Color,
    /// Slider / switch thumb.
    pub thumb: Color,
    /// Table grid lines.
    pub grid: Color,
    /// Splitter sash.
    pub sash: Color,
    /// Modal / drawer scrim.
    pub scrim: Color,
    /// Danger / badge fill.
    pub danger: Color,
    /// Soft danger (popconfirm trigger).
    pub danger_soft: Color,
    /// Warning accent (Result / Alarm).
    pub warning: Color,
    /// Info accent (Result / Alarm).
    pub info: Color,
    /// Success accent (gauge).
    pub success: Color,
    /// Soft success fill (steps done).
    pub success_soft: Color,
    /// Alert info background.
    pub info_bg: Color,
    /// Alert success background.
    pub success_bg: Color,
    /// Alert warning background.
    pub warning_bg: Color,
    /// Alert error background.
    pub error_bg: Color,
    /// Alarm flash background.
    pub flash_bg: Color,
    /// Spin overlay.
    pub overlay: Color,
    /// Gauge dial.
    pub gauge_dial: Color,
    /// Trend line.
    pub trend_line: Color,
    /// Trend fill under line.
    pub trend_fill: Color,
    /// Positive hint (statistic).
    pub hint_positive: Color,
    /// Tabs content pane.
    pub content_bg: Color,
    /// Tabs strip.
    pub strip_bg: Color,
    /// Radio outer ring.
    pub ring: Color,
    /// Scroll viewport fill.
    pub scroll_bg: Color,
}

/// Font size scale (px). Density may scale these at apply time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTypography {
    /// Body / default label size ([`crate::FlexBuilder::text`]).
    pub body: f32,
    /// Panel title size.
    pub title: f32,
    /// Primary button label.
    pub button: f32,
    /// Compact / toolbar button label.
    pub button_sm: f32,
    /// Caption / calendar day text.
    pub caption: f32,
    /// Text input value size.
    pub input: f32,
}

/// Spacing tokens (px) before density scaling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeSpacing {
    /// Large padding (columns).
    pub padding_lg: f32,
    /// Medium padding (panels).
    pub padding_md: f32,
    /// Small padding (rows / inputs).
    pub padding_sm: f32,
    /// Large gap (columns).
    pub gap_lg: f32,
    /// Medium gap.
    pub gap_md: f32,
    /// Small gap (toolbars).
    pub gap_sm: f32,
}

/// Corner radii (px). Soft: `control` for buttons/inputs/tags/…; `panel` for Panel/Card/Modal/Drawer/list hosts/popups.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeRadii {
    /// Controls / buttons.
    pub control: f32,
    /// Panels / cards.
    pub panel: f32,
}

/// Full theme: mode + token bags.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// Light or dark.
    pub mode: ThemeMode,
    /// Color tokens.
    pub colors: ThemeColors,
    /// Type scale.
    pub typography: ThemeTypography,
    /// Spacing scale.
    pub spacing: ThemeSpacing,
    /// Radii (forward-looking).
    pub radii: ThemeRadii,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

fn shared_typography() -> ThemeTypography {
    ThemeTypography {
        body: 22.0,
        title: 14.0,
        button: 18.0,
        button_sm: 14.0,
        caption: 12.0,
        input: 14.0,
    }
}

fn shared_spacing() -> ThemeSpacing {
    ThemeSpacing {
        padding_lg: 16.0,
        padding_md: 10.0,
        padding_sm: 8.0,
        gap_lg: 12.0,
        gap_md: 8.0,
        gap_sm: 6.0,
    }
}

fn shared_radii() -> ThemeRadii {
    ThemeRadii {
        control: 4.0,
        panel: 6.0,
    }
}

impl Theme {
    /// Dark preset (aligned with historical widget defaults).
    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            colors: ThemeColors {
                surface: Color::rgb(0x2a, 0x2a, 0x2e),
                surface_elevated: Color::rgb(0x36, 0x36, 0x3a),
                panel: Color::rgb(0x28, 0x28, 0x2c),
                title_bar: Color::rgb(0x3a, 0x3a, 0x42),
                text_primary: Color::rgb(0xe5, 0xe5, 0xe5),
                text_secondary: Color::rgb(0xa0, 0xa8, 0xb4),
                button_bg: Color::rgb(0x3b, 0x82, 0xf6),
                button_fg: Color::rgb(0xff, 0xff, 0xff),
                accent: Color::rgb(0x2f, 0x5b, 0xa8),
                on_accent: Color::rgb(0xff, 0xff, 0xff),
                border: Color::rgb(0x4a, 0x4a, 0x54),
                focus: Color::rgb(0x3b, 0x82, 0xf6),
                input_bg: Color::rgb(0x1e, 0x1e, 0x22),
                placeholder: Color::rgb(0x78, 0x80, 0x8c),
                link: Color::rgb(0x6a, 0xb0, 0xf0),
                muted: Color::rgb(0x2c, 0x2c, 0x30),
                disabled: Color::rgb(0x70, 0x74, 0x80),
                popup: Color::rgb(0x2a, 0x2a, 0x32),
                list_bg: Color::rgb(0x22, 0x22, 0x28),
                header_bg: Color::rgb(0x32, 0x32, 0x3a),
                chrome: Color::rgb(0x3a, 0x3a, 0x48),
                selected_bg: Color::rgb(0x2f, 0x5b, 0xa8),
                hover_bg: Color::rgb(0x2f, 0x5b, 0xa8),
                track: Color::rgb(0x3a, 0x3a, 0x44),
                track_on: Color::rgb(0x22, 0xc5, 0x5e),
                thumb: Color::rgb(0xf0, 0xf0, 0xf4),
                grid: Color::rgb(0x3a, 0x3a, 0x44),
                sash: Color::rgb(0x4a, 0x4a, 0x52),
                scrim: Color::rgba(0x0c, 0x0c, 0x10, 160),
                danger: Color::rgb(0xe0, 0x4f, 0x5f),
                danger_soft: Color::rgb(0xa0, 0x40, 0x40),
                warning: Color::rgb(0xe0, 0xb0, 0x40),
                info: Color::rgb(0x6a, 0xb0, 0xf0),
                success: Color::rgb(0x3b, 0xc4, 0x8c),
                success_soft: Color::rgb(0x3d, 0x8b, 0x5a),
                info_bg: Color::rgb(0x2a, 0x3a, 0x55),
                success_bg: Color::rgb(0x1e, 0x3d, 0x2a),
                warning_bg: Color::rgb(0x4a, 0x3a, 0x18),
                error_bg: Color::rgb(0x4a, 0x22, 0x28),
                flash_bg: Color::rgb(0x5a, 0x22, 0x28),
                overlay: Color::rgb(0x14, 0x14, 0x18),
                gauge_dial: Color::rgb(0x3b, 0xc4, 0x8c),
                trend_line: Color::rgb(0x4a, 0xc0, 0xf0),
                trend_fill: Color::rgb(0x1a, 0x40, 0x55),
                hint_positive: Color::rgb(0x6a, 0xc0, 0x8a),
                content_bg: Color::rgb(0x24, 0x24, 0x2a),
                strip_bg: Color::rgb(0x2c, 0x2c, 0x34),
                ring: Color::rgb(0x6a, 0x70, 0x7c),
                scroll_bg: Color::rgb(0x24, 0x24, 0x28),
            },
            typography: shared_typography(),
            spacing: shared_spacing(),
            radii: shared_radii(),
        }
    }

    /// Light preset (inverse surfaces; shared accent / brand blues).
    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            colors: ThemeColors {
                surface: Color::rgb(0xf4, 0xf4, 0xf6),
                surface_elevated: Color::rgb(0xff, 0xff, 0xff),
                panel: Color::rgb(0xff, 0xff, 0xff),
                title_bar: Color::rgb(0xe8, 0xe8, 0xec),
                text_primary: Color::rgb(0x1a, 0x1a, 0x1e),
                text_secondary: Color::rgb(0x5a, 0x5a, 0x64),
                button_bg: Color::rgb(0x3b, 0x82, 0xf6),
                button_fg: Color::rgb(0xff, 0xff, 0xff),
                accent: Color::rgb(0x2f, 0x5b, 0xa8),
                on_accent: Color::rgb(0xff, 0xff, 0xff),
                border: Color::rgb(0xc4, 0xc4, 0xcc),
                focus: Color::rgb(0x3b, 0x82, 0xf6),
                input_bg: Color::rgb(0xff, 0xff, 0xff),
                placeholder: Color::rgb(0x98, 0x98, 0xa4),
                link: Color::rgb(0x1a, 0x6a, 0xc8),
                muted: Color::rgb(0xec, 0xec, 0xf0),
                disabled: Color::rgb(0x98, 0x98, 0xa0),
                popup: Color::rgb(0xff, 0xff, 0xff),
                list_bg: Color::rgb(0xff, 0xff, 0xff),
                header_bg: Color::rgb(0xee, 0xee, 0xf2),
                chrome: Color::rgb(0xe0, 0xe0, 0xe8),
                selected_bg: Color::rgb(0x2f, 0x5b, 0xa8),
                hover_bg: Color::rgb(0x2f, 0x5b, 0xa8),
                track: Color::rgb(0xd0, 0xd0, 0xd8),
                track_on: Color::rgb(0x22, 0xc5, 0x5e),
                thumb: Color::rgb(0xff, 0xff, 0xff),
                grid: Color::rgb(0xd8, 0xd8, 0xe0),
                sash: Color::rgb(0xc0, 0xc0, 0xc8),
                scrim: Color::rgba(0x20, 0x24, 0x30, 96),
                danger: Color::rgb(0xe0, 0x4f, 0x5f),
                danger_soft: Color::rgb(0xc0, 0x50, 0x50),
                warning: Color::rgb(0xc0, 0x80, 0x20),
                info: Color::rgb(0x1a, 0x6a, 0xc8),
                success: Color::rgb(0x2a, 0xa0, 0x6a),
                success_soft: Color::rgb(0x3d, 0x8b, 0x5a),
                info_bg: Color::rgb(0xd8, 0xe4, 0xf4),
                success_bg: Color::rgb(0xd4, 0xec, 0xda),
                warning_bg: Color::rgb(0xf4, 0xe8, 0xc8),
                error_bg: Color::rgb(0xf4, 0xd8, 0xdc),
                flash_bg: Color::rgb(0xf0, 0xd0, 0xd4),
                overlay: Color::rgb(0xf0, 0xf0, 0xf4),
                gauge_dial: Color::rgb(0x2a, 0xa0, 0x6a),
                trend_line: Color::rgb(0x2a, 0x90, 0xc8),
                trend_fill: Color::rgb(0xc8, 0xe4, 0xf0),
                hint_positive: Color::rgb(0x2a, 0xa0, 0x6a),
                content_bg: Color::rgb(0xf8, 0xf8, 0xfa),
                strip_bg: Color::rgb(0xee, 0xee, 0xf2),
                ring: Color::rgb(0x90, 0x94, 0xa0),
                scroll_bg: Color::rgb(0xf0, 0xf0, 0xf4),
            },
            typography: shared_typography(),
            spacing: shared_spacing(),
            radii: shared_radii(),
        }
    }

    /// Resolve a `.ngui` color token name (`surface`, `text_primary`, …) against this theme.
    ///
    /// Names match [`ThemeColors`] field names. Returns `None` for unknown tokens.
    pub fn color_by_name(&self, name: &str) -> Option<Color> {
        let c = &self.colors;
        Some(match name {
            "surface" => c.surface,
            "surface_elevated" => c.surface_elevated,
            "panel" => c.panel,
            "title_bar" => c.title_bar,
            "text_primary" => c.text_primary,
            "text_secondary" => c.text_secondary,
            "button_bg" => c.button_bg,
            "button_fg" => c.button_fg,
            "accent" => c.accent,
            "on_accent" => c.on_accent,
            "border" => c.border,
            "focus" => c.focus,
            "input_bg" => c.input_bg,
            "placeholder" => c.placeholder,
            "link" => c.link,
            "muted" => c.muted,
            "disabled" => c.disabled,
            "popup" => c.popup,
            "list_bg" => c.list_bg,
            "header_bg" => c.header_bg,
            "chrome" => c.chrome,
            "selected_bg" => c.selected_bg,
            "hover_bg" => c.hover_bg,
            "track" => c.track,
            "track_on" => c.track_on,
            "thumb" => c.thumb,
            "grid" => c.grid,
            "sash" => c.sash,
            "scrim" => c.scrim,
            "danger" => c.danger,
            "danger_soft" => c.danger_soft,
            "warning" => c.warning,
            "info" => c.info,
            "success" => c.success,
            "success_soft" => c.success_soft,
            "info_bg" => c.info_bg,
            "success_bg" => c.success_bg,
            "warning_bg" => c.warning_bg,
            "error_bg" => c.error_bg,
            "flash_bg" => c.flash_bg,
            "overlay" => c.overlay,
            "gauge_dial" => c.gauge_dial,
            "trend_line" => c.trend_line,
            "trend_fill" => c.trend_fill,
            "hint_positive" => c.hint_positive,
            "content_bg" => c.content_bg,
            "strip_bg" => c.strip_bg,
            "ring" => c.ring,
            "scroll_bg" => c.scroll_bg,
            _ => return None,
        })
    }
}

impl UiDensity {
    /// Multiplier for spacing tokens (`padding` / `gap`).
    pub fn spacing_scale(self) -> f32 {
        match self {
            Self::Compact => 0.75,
            Self::Default => 1.0,
            Self::Comfortable => 1.25,
        }
    }

    /// Multiplier for font sizes (milder than spacing).
    pub fn font_scale(self) -> f32 {
        match self {
            Self::Compact => 0.92,
            Self::Default => 1.0,
            Self::Comfortable => 1.08,
        }
    }
}

fn scale(v: f32, s: f32) -> f32 {
    (v * s).round().max(1.0)
}

fn transparent() -> Color {
    Color::rgba(0, 0, 0, 0)
}

impl ColumnStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            x: 24.0,
            y: 24.0,
            width: 400.0,
            height: 560.0,
            padding: scale(theme.spacing.padding_lg, s),
            gap: scale(theme.spacing.gap_lg, s),
            cross_align: CrossAlign::Start,
            background: Some(c.surface),
        }
    }
}


impl RowStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            x: 0.0,
            y: 0.0,
            width: 600.0,
            height: scale(48.0, s),
            padding: scale(theme.spacing.padding_sm, s),
            gap: scale(theme.spacing.gap_md, s),
            cross_align: CrossAlign::Center,
            background: Some(c.surface_elevated),
        }
    }
}


impl ToolbarStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: scale(40.0, s),
            padding: scale(6.0, s),
            gap: scale(theme.spacing.gap_sm, s),
            cross_align: CrossAlign::Center,
            background: Some(c.surface_elevated),
        }
    }
}


impl PanelStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            x: 0.0,
            y: 0.0,
            width: 320.0,
            height: 400.0,
            padding: scale(theme.spacing.padding_md, s),
            gap: scale(theme.spacing.gap_md, s),
            cross_align: CrossAlign::Start,
            background: Some(c.panel),
            title_height: scale(28.0, s),
            title_bg: c.title_bar,
            title_fg: c.text_primary,
            title_size: scale(theme.typography.title, f),
        }
    }
}


impl StatusBarStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: scale(26.0, s),
            padding: scale(6.0, s),
            gap: scale(theme.spacing.gap_lg, s),
            cross_align: CrossAlign::Center,
            background: Some(c.muted),
        }
    }
}


impl SplitterStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 480.0,
            axis: Axis::Vertical,
            ratio: 0.3,
            sash_size: scale(5.0, s),
            sash_color: c.sash,
            background: None,
        }
    }
}


impl ScrollAreaStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            x: 0.0,
            y: 0.0,
            width: 280.0,
            height: 200.0,
            padding: scale(theme.spacing.padding_sm, s),
            gap: scale(theme.spacing.gap_sm, s),
            cross_align: CrossAlign::Start,
            background: Some(c.scroll_bg),
        }
    }
}


impl TextInputStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 220.0,
            height: scale(32.0, s),
            size: scale(theme.typography.input, f),
            fg: c.text_primary,
            placeholder_fg: c.placeholder,
            bg: c.input_bg,
            border: c.border,
            focus_border: c.focus,
            padding_x: scale(theme.spacing.padding_sm, s),
        }
    }
}


impl CheckboxStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            box_size: scale(18.0, s),
            size: scale(theme.typography.input, f),
            fg: c.text_primary,
            box_bg: c.panel,
            checked_bg: c.focus,
            check_fg: c.on_accent,
            gap: scale(theme.spacing.gap_md, s),
        }
    }
}


impl RadioStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            diameter: scale(18.0, s),
            size: scale(theme.typography.input, f),
            fg: c.text_primary,
            ring: c.ring,
            dot: c.focus,
            gap: scale(theme.spacing.gap_md, s),
        }
    }
}


impl SwitchStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: scale(40.0, s),
            height: scale(22.0, s),
            size: scale(theme.typography.input, f),
            fg: c.text_primary,
            track_off: c.track,
            track_on: c.track_on,
            thumb: c.thumb,
            gap: scale(theme.spacing.gap_md, s),
        }
    }
}


impl ListStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            x: 0.0,
            y: 0.0,
            width: 220.0,
            height: 200.0,
            item_height: scale(26.0, s),
            padding: scale(4.0, s),
            size: scale(13.0, f),
            fg: c.text_primary,
            selected_fg: c.on_accent,
            bg: c.list_bg,
            selected_bg: c.selected_bg,
            focus_border: c.focus,
        }
    }
}


impl TreeStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            x: 0.0,
            y: 0.0,
            width: 240.0,
            height: 260.0,
            row_height: scale(26.0, s),
            indent: scale(14.0, s),
            padding: scale(4.0, s),
            size: scale(13.0, f),
            fg: c.text_primary,
            selected_fg: c.on_accent,
            bg: c.list_bg,
            selected_bg: c.selected_bg,
            focus_border: c.focus,
        }
    }
}


impl TableStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            x: 0.0,
            y: 0.0,
            width: 420.0,
            height: 240.0,
            header_height: scale(28.0, s),
            row_height: scale(26.0, s),
            padding: scale(4.0, s),
            size: scale(13.0, f),
            fg: c.text_primary,
            header_fg: c.text_secondary,
            selected_fg: c.on_accent,
            bg: c.list_bg,
            header_bg: c.header_bg,
            selected_bg: c.selected_bg,
            drag_select: false,
            grid: c.grid,
            focus_border: c.focus,
        }
    }
}


impl MenuStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            min_width: 96.0,
            item_height: scale(28.0, s),
            popup_width: 260.0,
            size: scale(13.0, f),
            fg: c.text_primary,
            disabled_fg: c.disabled,
            title_bg: c.surface_elevated,
            open_bg: c.hover_bg,
            popup_bg: c.popup,
            hover_bg: c.hover_bg,
            hover_fg: c.on_accent,
        }
    }
}


impl ContextMenuStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 280.0,
            height: 120.0,
            popup_width: 180.0,
            item_height: scale(28.0, s),
            size: scale(13.0, f),
            host_bg: c.muted,
            hint_fg: c.text_secondary,
            popup_bg: c.popup,
            fg: c.text_primary,
            disabled_fg: c.disabled,
            hover_bg: c.hover_bg,
            hover_fg: c.on_accent,
        }
    }
}


impl TabsStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 400.0,
            height: 240.0,
            strip_height: scale(32.0, s),
            scrollable: false,
            strip_at_bottom: false,
            padding: scale(theme.spacing.padding_md, s),
            gap: scale(theme.spacing.gap_md, s),
            cross_align: CrossAlign::Start,
            size: scale(13.0, f),
            fg: c.text_secondary,
            active_fg: c.text_primary,
            strip_bg: c.strip_bg,
            active_bg: c.chrome,
            content_bg: c.content_bg,
        }
    }
}


impl SelectStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 200.0,
            height: scale(30.0, s),
            popup_max_height: 160.0,
            item_height: scale(26.0, s),
            padding: scale(theme.spacing.padding_sm, s),
            size: scale(13.0, f),
            fg: c.text_primary,
            placeholder_fg: c.placeholder,
            bg: c.popup,
            popup_bg: c.popup,
            selected_bg: c.selected_bg,
            selected_fg: c.on_accent,
            focus_border: c.focus,
        }
    }
}


impl SliderStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 220.0,
            height: scale(28.0, s),
            track_thickness: scale(6.0, s),
            thumb_size: scale(16.0, s),
            track: c.track,
            fill: c.focus,
            thumb: c.thumb,
            focus_border: c.focus,
        }
    }
}


impl TagStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            height: scale(24.0, s),
            padding_x: scale(theme.spacing.padding_sm, s),
            size: scale(theme.typography.caption, f),
            fg: c.text_primary,
            bg: c.chrome,
            close_fg: c.text_secondary,
        }
    }
}


impl BadgeStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            dot_size: scale(8.0, s),
            height: scale(18.0, s),
            padding_x: scale(6.0, s),
            size: scale(11.0, f),
            fg: c.on_accent,
            bg: c.danger,
            offset: scale(4.0, s),
        }
    }
}


impl TooltipStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            size: scale(theme.typography.caption, f),
            padding: scale(6.0, s),
            max_width: 280.0,
            fg: c.text_primary,
            bg: c.popup,
            gap: scale(theme.spacing.gap_sm, s),
        }
    }
}


impl ModalStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 420.0,
            height: 200.0,
            title_height: scale(36.0, s),
            padding: scale(theme.spacing.padding_lg, s),
            title_size: scale(15.0, f),
            body_size: scale(13.0, f),
            action_height: scale(32.0, s),
            title_fg: c.text_primary,
            body_fg: c.text_secondary,
            panel_bg: c.popup,
            title_bg: c.title_bar,
            scrim: c.scrim,
            primary_bg: c.accent,
            secondary_bg: c.chrome,
            action_fg: c.on_accent,
            secondary_fg: c.text_primary,
            close_on_backdrop: true,
        }
    }
}


impl ProgressStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 240.0,
            height: scale(10.0, s),
            track: c.track,
            fill: c.focus,
        }
    }
}


impl AlertStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 480.0,
            height: scale(36.0, s),
            padding: scale(theme.spacing.padding_md, s),
            size: scale(13.0, f),
            fg: c.text_primary,
            info_bg: c.info_bg,
            success_bg: c.success_bg,
            warning_bg: c.warning_bg,
            error_bg: c.error_bg,
            close_fg: c.text_secondary,
        }
    }
}


impl FormStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            x: 0.0,
            y: 0.0,
            width: 320.0,
            height: 280.0,
            padding: scale(theme.spacing.padding_sm, s),
            gap: scale(theme.spacing.gap_sm, s),
            cross_align: CrossAlign::Start,
            background: Some(c.panel),
            label_size: scale(theme.typography.caption, f),
            label_fg: c.text_secondary,
            error_size: scale(11.0, f),
            error_fg: c.danger,
        }
    }
}


impl TreeSelectStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 220.0,
            height: scale(30.0, s),
            popup_max_height: 200.0,
            row_height: scale(26.0, s),
            indent: scale(14.0, s),
            padding: scale(theme.spacing.padding_sm, s),
            size: scale(13.0, f),
            fg: c.text_primary,
            placeholder_fg: c.placeholder,
            bg: c.popup,
            popup_bg: c.popup,
            selected_bg: c.selected_bg,
            focus_border: c.focus,
        }
    }
}


impl AlarmListStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 420.0,
            height: 200.0,
            row_height: scale(28.0, s),
            padding: scale(4.0, s),
            size: scale(theme.typography.caption, f),
            fg: c.text_primary,
            bg: c.input_bg,
            selected_bg: c.selected_bg,
            flash_bg: c.flash_bg,
            focus_border: c.focus,
        }
    }
}


impl GaugeStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            size: scale(120.0, s),
            dial_fg: c.gauge_dial,
            text_fg: c.text_primary,
            value_size: scale(16.0, f),
            label_size: scale(11.0, f),
        }
    }
}


impl TrendStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 220.0,
            height: scale(64.0, s),
            line: c.trend_line,
            fill: c.trend_fill,
            bg: c.list_bg,
        }
    }
}


impl ChartStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 640.0,
            height: 320.0,
            columns: 2,
            show_plot_coordinates: false,
            gap: scale(10.0, s),
            padding: scale(theme.spacing.padding_md, s),
            bg: c.list_bg,
            title_fg: c.text_secondary,
            title_size: scale(theme.typography.caption, f),
        }
    }
}


impl BackdropStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            scrim: c.scrim,
            close_on_click: true,
        }
    }
}


impl DrawerStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 320.0,
            title_height: scale(40.0, s),
            padding: scale(theme.spacing.padding_lg, s),
            title_size: scale(15.0, f),
            body_size: scale(13.0, f),
            title_fg: c.text_primary,
            body_fg: c.text_secondary,
            panel_bg: c.popup,
            title_bg: c.title_bar,
            backdrop: BackdropStyle::from_theme_with_density(theme, density),
            placement: DrawerPlacement::Right,
        }
    }
}


impl TextAreaStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 280.0,
            height: scale(96.0, s),
            size: scale(13.0, f),
            line_height: scale(18.0, s),
            fg: c.text_primary,
            placeholder_fg: c.placeholder,
            bg: c.input_bg,
            border: c.border,
            focus_border: c.focus,
            padding_x: scale(theme.spacing.padding_sm, s),
            padding_y: scale(6.0, s),
        }
    }
}


impl InputNumberStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 140.0,
            height: scale(30.0, s),
            size: scale(13.0, f),
            fg: c.text_primary,
            bg: c.input_bg,
            border: c.border,
            focus_border: c.focus,
            step_bg: c.chrome,
            step_fg: c.text_primary,
            padding_x: scale(theme.spacing.padding_sm, s),
        }
    }
}


impl NotificationStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 280.0,
            height: scale(44.0, s),
            size: scale(13.0, f),
            fg: c.text_primary,
            bg: c.popup,
            gap: scale(theme.spacing.gap_md, s),
            margin: scale(theme.spacing.padding_lg, s),
            max_visible: 4,
        }
    }
}


impl SpinStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 200.0,
            height: 120.0,
            overlay: c.overlay,
            fg: c.text_primary,
            size: scale(13.0, f),
            tip: "Loading…",
        }
    }
}


impl EmptyStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 240.0,
            height: 100.0,
            size: scale(13.0, f),
            fg: c.text_secondary,
            bg: c.list_bg,
        }
    }
}


impl CollapseStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 320.0,
            header_height: scale(32.0, s),
            padding: scale(theme.spacing.padding_md, s),
            size: scale(13.0, f),
            fg: c.text_primary,
            header_bg: c.title_bar,
            body_bg: c.panel,
            border: c.grid,
        }
    }
}


impl DescriptionsStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 320.0,
            row_height: scale(26.0, s),
            label_width: 100.0,
            size: scale(theme.typography.caption, f),
            label_fg: c.text_secondary,
            value_fg: c.text_primary,
            bg: c.panel,
            padding: scale(theme.spacing.padding_sm, s),
        }
    }
}


impl CardStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 260.0,
            height: 140.0,
            title_height: scale(34.0, s),
            padding: scale(12.0, s),
            title_size: scale(theme.typography.title, f),
            body_size: scale(theme.typography.caption, f),
            title_fg: c.text_primary,
            body_fg: c.text_secondary,
            bg: c.popup,
            title_bg: c.title_bar,
        }
    }
}


impl SegmentedStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 240.0,
            height: scale(30.0, s),
            size: scale(theme.typography.caption, f),
            fg: c.text_secondary,
            selected_fg: c.on_accent,
            bg: c.list_bg,
            selected_bg: c.selected_bg,
            padding: scale(2.0, s),
        }
    }
}


impl BreadcrumbStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 400.0,
            height: scale(28.0, s),
            size: scale(theme.typography.caption, f),
            fg: c.link,
            current_fg: c.text_primary,
            sep_fg: c.disabled,
        }
    }
}


impl PaginationStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 320.0,
            height: scale(30.0, s),
            size: scale(theme.typography.caption, f),
            fg: c.text_primary,
            btn_bg: c.chrome,
            active_bg: c.selected_bg,
            active_fg: c.on_accent,
            gap: scale(theme.spacing.gap_sm, s),
        }
    }
}


impl DateTimeStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 200.0,
            height: scale(30.0, s),
            size: scale(13.0, f),
            fg: c.text_primary,
            placeholder_fg: c.placeholder,
            bg: c.input_bg,
            border: c.border,
            focus_border: c.focus,
            popup_bg: c.popup,
            accent: c.accent,
            popup_width: 280.0,
            cell: scale(32.0, s),
            padding: scale(theme.spacing.padding_sm, s),
        }
    }
}


impl TransferStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 520.0,
            height: 260.0,
            list_width: 200.0,
            row_height: scale(26.0, s),
            size: scale(theme.typography.caption, f),
            fg: c.text_primary,
            selected_fg: c.on_accent,
            bg: c.list_bg,
            selected_bg: c.selected_bg,
            header_bg: c.title_bar,
            btn_bg: c.chrome,
            btn_fg: c.text_primary,
            gap: scale(10.0, s),
            padding: scale(6.0, s),
        }
    }
}


impl PropertyGridStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 320.0,
            height: 220.0,
            row_height: scale(28.0, s),
            key_width: 120.0,
            size: scale(theme.typography.caption, f),
            key_fg: c.text_secondary,
            value_fg: c.text_primary,
            bg: c.panel,
            selected_bg: c.selected_bg,
            selected_fg: c.on_accent,
            header_bg: c.title_bar,
            padding: scale(4.0, s),
        }
    }
}


impl TreeTableStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            x: 0.0,
            y: 0.0,
            width: 480.0,
            height: 280.0,
            header_height: scale(28.0, s),
            row_height: scale(26.0, s),
            indent: scale(14.0, s),
            tree_col_width: 160.0,
            size: scale(theme.typography.caption, f),
            fg: c.text_primary,
            selected_fg: c.on_accent,
            header_fg: c.text_secondary,
            bg: c.list_bg,
            header_bg: c.title_bar,
            selected_bg: c.selected_bg,
            padding: scale(4.0, s),
        }
    }
}


impl SpaceStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: scale(48.0, s),
            padding: 0.0,
            gap: scale(theme.spacing.gap_lg, s),
            direction: SpaceDirection::Horizontal,
            cross_align: CrossAlign::Center,
            background: None,
        }
    }
}


impl ButtonGroupStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 280.0,
            height: scale(32.0, s),
            size: scale(theme.typography.caption, f),
            fg: c.text_secondary,
            selected_fg: c.on_accent,
            bg: c.popup,
            selected_bg: c.selected_bg,
            border: c.grid,
            gap: scale(4.0, s),
            padding: scale(2.0, s),
        }
    }
}


impl LinkStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            size: scale(theme.typography.input, f),
            fg: c.link,
            underline: true,
        }
    }
}


impl StepsStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 480.0,
            height: scale(72.0, s),
            direction: StepsDirection::Horizontal,
            clickable: true,
            size: scale(theme.typography.caption, f),
            desc_size: scale(11.0, f),
            marker: scale(22.0, s),
            wait_fg: c.text_secondary,
            process_fg: c.text_primary,
            done_fg: c.text_secondary,
            wait_bg: c.track,
            process_bg: c.selected_bg,
            done_bg: c.success_soft,
            connector: c.border,
            bg: transparent(),
            padding: scale(theme.spacing.padding_sm, s),
            gap: scale(theme.spacing.gap_lg, s),
        }
    }
}


impl StatisticStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 160.0,
            height: scale(88.0, s),
            title_size: scale(theme.typography.caption, f),
            value_size: scale(28.0, f),
            hint_size: scale(11.0, f),
            title_fg: c.text_secondary,
            value_fg: c.text_primary,
            hint_fg: c.hint_positive,
            bg: c.list_bg,
            padding: scale(12.0, s),
        }
    }
}


impl ResultStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 360.0,
            height: 200.0,
            title_size: scale(18.0, f),
            subtitle_size: scale(13.0, f),
            status_size: scale(theme.typography.title, f),
            action_size: scale(13.0, f),
            action_height: scale(32.0, s),
            title_fg: c.text_primary,
            subtitle_fg: c.text_secondary,
            bg: c.list_bg,
            primary_bg: c.selected_bg,
            primary_fg: c.on_accent,
            secondary_bg: c.track,
            secondary_fg: c.text_primary,
            padding: scale(20.0, s),
            gap: scale(theme.spacing.gap_lg, s),
        }
    }
}


impl PopoverStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 120.0,
            height: scale(32.0, s),
            size: scale(13.0, f),
            title_size: scale(13.0, f),
            body_size: scale(theme.typography.caption, f),
            max_width: 280.0,
            padding: scale(theme.spacing.padding_md, s),
            gap: scale(theme.spacing.gap_sm, s),
            trigger_fg: c.text_primary,
            trigger_bg: c.selected_bg,
            popup_bg: c.popup,
            title_fg: c.text_primary,
            body_fg: c.text_secondary,
        }
    }
}


impl PopconfirmStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 120.0,
            height: scale(32.0, s),
            size: scale(13.0, f),
            max_width: 280.0,
            padding: scale(12.0, s),
            gap: scale(theme.spacing.gap_sm, s),
            action_height: scale(28.0, s),
            action_gap: scale(theme.spacing.gap_md, s),
            trigger_fg: c.text_primary,
            trigger_bg: c.danger_soft,
            popup_bg: c.popup,
            message_fg: c.text_primary,
            ok_bg: c.selected_bg,
            ok_fg: c.on_accent,
            cancel_bg: c.track,
            cancel_fg: c.text_primary,
        }
    }
}


impl UploadStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 360.0,
            height: scale(32.0, s),
            size: scale(13.0, f),
            fg: c.text_primary,
            placeholder_fg: c.placeholder,
            bg: c.popup,
            browse_bg: c.selected_bg,
            browse_fg: c.on_accent,
            browse_width: 72.0,
            clear_bg: c.track,
            clear_fg: c.text_primary,
            clear_width: 56.0,
            padding: scale(theme.spacing.padding_sm, s),
            gap: scale(theme.spacing.gap_sm, s),
            show_clear: true,
        }
    }
}


impl ColorPickerStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 360.0,
            height: scale(36.0, s),
            swatch_size: scale(22.0, s),
            swatch_gap: scale(4.0, s),
            hex_width: 88.0,
            size: scale(theme.typography.caption, f),
            fg: c.text_primary,
            hex_bg: c.input_bg,
            bg: transparent(),
            selected_border: c.text_primary,
            padding: scale(4.0, s),
        }
    }
}


impl AutocompleteStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 240.0,
            height: scale(30.0, s),
            popup_max_height: 160.0,
            item_height: scale(26.0, s),
            padding: scale(theme.spacing.padding_sm, s),
            size: scale(13.0, f),
            fg: c.text_primary,
            placeholder_fg: c.placeholder,
            bg: c.popup,
            popup_bg: c.popup,
            selected_bg: c.selected_bg,
            selected_fg: c.on_accent,
            focus_border: c.focus,
        }
    }
}


impl GridStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 200.0,
            padding: scale(theme.spacing.padding_sm, s),
            gap: scale(theme.spacing.gap_md, s),
            background: None,
        }
    }
}


impl TimelineStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 360.0,
            height: 160.0,
            item_height: scale(36.0, s),
            time_width: 72.0,
            size: scale(theme.typography.caption, f),
            time_fg: c.text_secondary,
            content_fg: c.text_primary,
            rail: c.accent,
            bg: c.popup,
            padding: scale(theme.spacing.padding_sm, s),
            clickable: true,
        }
    }
}


impl CalendarStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 280.0,
            height: 260.0,
            cell: scale(32.0, s),
            size: scale(theme.typography.caption, f),
            fg: c.text_primary,
            accent: c.accent,
            bg: c.surface_elevated,
            header_bg: c.surface,
            padding: scale(theme.spacing.padding_sm, s),
        }
    }
}


impl ImageStyle {
    /// Defaults from theme with [`UiDensity::Default`].
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density scaling.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let c = &theme.colors;
        let s = density.spacing_scale();
        let f = density.font_scale();
        let _ = (c, s, f);
        Self {
            width: 160.0,
            height: 100.0,
            bg: c.input_bg,
            caption_fg: c.text_secondary,
            caption_size: scale(11.0, f),
            mosaic_max: 24,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_and_light_presets_differ() {
        let d = Theme::dark();
        let l = Theme::light();
        assert_eq!(d.mode, ThemeMode::Dark);
        assert_eq!(l.mode, ThemeMode::Light);
        assert_ne!(d.colors.surface, l.colors.surface);
        assert_ne!(d.colors.text_primary, l.colors.text_primary);
        assert_eq!(d.colors.button_bg, l.colors.button_bg);
        assert_eq!(d.colors.accent, l.colors.accent);
    }

    #[test]
    fn column_style_tracks_theme_surface() {
        let dark = ColumnStyle::from_theme(&Theme::dark());
        let light = ColumnStyle::from_theme(&Theme::light());
        assert_eq!(dark.background, Some(Theme::dark().colors.surface));
        assert_eq!(light.background, Some(Theme::light().colors.surface));
        assert_ne!(dark.background, light.background);
    }

    #[test]
    fn density_scales_padding() {
        let theme = Theme::dark();
        let def = ColumnStyle::from_theme_with_density(&theme, UiDensity::Default);
        let compact = ColumnStyle::from_theme_with_density(&theme, UiDensity::Compact);
        let cozy = ColumnStyle::from_theme_with_density(&theme, UiDensity::Comfortable);
        assert!(compact.padding < def.padding);
        assert!(cozy.padding > def.padding);
        assert!(compact.gap < def.gap);
    }

    #[test]
    fn panel_and_input_from_theme() {
        let light = Theme::light();
        let panel = PanelStyle::from_theme(&light);
        assert_eq!(panel.background, Some(light.colors.panel));
        assert_eq!(panel.title_bg, light.colors.title_bar);
        assert_eq!(panel.title_fg, light.colors.text_primary);

        let input = TextInputStyle::from_theme(&light);
        assert_eq!(input.bg, light.colors.input_bg);
        assert_eq!(input.fg, light.colors.text_primary);
        assert_eq!(input.focus_border, light.colors.focus);
    }

    #[test]
    fn calendar_and_link_from_theme() {
        let dark = Theme::dark();
        let cal = CalendarStyle::from_theme(&dark);
        assert_eq!(cal.accent, dark.colors.accent);
        assert_eq!(cal.fg, dark.colors.text_primary);

        let link = LinkStyle::from_theme(&Theme::light());
        assert_eq!(link.fg, Theme::light().colors.link);
        assert!(link.underline);
    }

    #[test]
    fn default_theme_is_dark() {
        assert_eq!(Theme::default().mode, ThemeMode::Dark);
    }

    #[test]
    fn all_styles_from_theme_smoke() {
        let theme = Theme::light();
        let d = UiDensity::Compact;
        let _ = ColumnStyle::from_theme_with_density(&theme, d);
        let _ = RowStyle::from_theme_with_density(&theme, d);
        let _ = ToolbarStyle::from_theme_with_density(&theme, d);
        let _ = PanelStyle::from_theme_with_density(&theme, d);
        let _ = StatusBarStyle::from_theme_with_density(&theme, d);
        let _ = SplitterStyle::from_theme_with_density(&theme, d);
        let _ = ScrollAreaStyle::from_theme_with_density(&theme, d);
        let _ = TextInputStyle::from_theme_with_density(&theme, d);
        let _ = CheckboxStyle::from_theme_with_density(&theme, d);
        let _ = RadioStyle::from_theme_with_density(&theme, d);
        let _ = SwitchStyle::from_theme_with_density(&theme, d);
        let _ = ListStyle::from_theme_with_density(&theme, d);
        let _ = TreeStyle::from_theme_with_density(&theme, d);
        let _ = TableStyle::from_theme_with_density(&theme, d);
        let _ = MenuStyle::from_theme_with_density(&theme, d);
        let _ = ContextMenuStyle::from_theme_with_density(&theme, d);
        let _ = TabsStyle::from_theme_with_density(&theme, d);
        let _ = SelectStyle::from_theme_with_density(&theme, d);
        let _ = SliderStyle::from_theme_with_density(&theme, d);
        let _ = TagStyle::from_theme_with_density(&theme, d);
        let _ = BadgeStyle::from_theme_with_density(&theme, d);
        let _ = TooltipStyle::from_theme_with_density(&theme, d);
        let _ = ModalStyle::from_theme_with_density(&theme, d);
        let _ = ProgressStyle::from_theme_with_density(&theme, d);
        let _ = AlertStyle::from_theme_with_density(&theme, d);
        let _ = FormStyle::from_theme_with_density(&theme, d);
        let _ = TreeSelectStyle::from_theme_with_density(&theme, d);
        let _ = AlarmListStyle::from_theme_with_density(&theme, d);
        let _ = GaugeStyle::from_theme_with_density(&theme, d);
        let _ = TrendStyle::from_theme_with_density(&theme, d);
        let _ = ChartStyle::from_theme_with_density(&theme, d);
        let _ = BackdropStyle::from_theme_with_density(&theme, d);
        let _ = DrawerStyle::from_theme_with_density(&theme, d);
        let _ = TextAreaStyle::from_theme_with_density(&theme, d);
        let _ = InputNumberStyle::from_theme_with_density(&theme, d);
        let _ = NotificationStyle::from_theme_with_density(&theme, d);
        let _ = SpinStyle::from_theme_with_density(&theme, d);
        let _ = EmptyStyle::from_theme_with_density(&theme, d);
        let _ = CollapseStyle::from_theme_with_density(&theme, d);
        let _ = DescriptionsStyle::from_theme_with_density(&theme, d);
        let _ = CardStyle::from_theme_with_density(&theme, d);
        let _ = SegmentedStyle::from_theme_with_density(&theme, d);
        let _ = BreadcrumbStyle::from_theme_with_density(&theme, d);
        let _ = PaginationStyle::from_theme_with_density(&theme, d);
        let _ = DateTimeStyle::from_theme_with_density(&theme, d);
        let _ = TransferStyle::from_theme_with_density(&theme, d);
        let _ = PropertyGridStyle::from_theme_with_density(&theme, d);
        let _ = TreeTableStyle::from_theme_with_density(&theme, d);
        let _ = SpaceStyle::from_theme_with_density(&theme, d);
        let _ = ButtonGroupStyle::from_theme_with_density(&theme, d);
        let _ = LinkStyle::from_theme_with_density(&theme, d);
        let _ = StepsStyle::from_theme_with_density(&theme, d);
        let _ = StatisticStyle::from_theme_with_density(&theme, d);
        let _ = ResultStyle::from_theme_with_density(&theme, d);
        let _ = PopoverStyle::from_theme_with_density(&theme, d);
        let _ = PopconfirmStyle::from_theme_with_density(&theme, d);
        let _ = UploadStyle::from_theme_with_density(&theme, d);
        let _ = ColorPickerStyle::from_theme_with_density(&theme, d);
        let _ = AutocompleteStyle::from_theme_with_density(&theme, d);
        let _ = GridStyle::from_theme_with_density(&theme, d);
        let _ = TimelineStyle::from_theme_with_density(&theme, d);
        let _ = CalendarStyle::from_theme_with_density(&theme, d);
        let _ = ImageStyle::from_theme_with_density(&theme, d);
    }

    #[test]
    fn result_status_colors_follow_theme() {
        use crate::ResultStatus;
        let dark = Theme::dark();
        let light = Theme::light();
        assert_eq!(
            ResultStatus::Error.color_for_theme(&dark),
            dark.colors.danger
        );
        assert_eq!(
            ResultStatus::Success.color_for_theme(&light),
            light.colors.success
        );
        assert_ne!(
            ResultStatus::Info.color_for_theme(&dark),
            ResultStatus::Info.color_for_theme(&light)
        );
        assert_ne!(
            ResultStatus::Warning.color_for_theme(&dark),
            ResultStatus::Warning.color_for_theme(&light)
        );
        assert_eq!(ResultStatus::Error.color(), Theme::dark().colors.danger);
    }

    #[test]
    fn alarm_severity_maps_theme_tokens() {
        use crate::AlarmSeverity;
        let light = Theme::light();
        assert_eq!(
            AlarmSeverity::Critical.color_for_theme(&light),
            light.colors.danger
        );
        assert_eq!(
            AlarmSeverity::Info.color_for_theme(&light),
            light.colors.text_secondary
        );
        assert_ne!(
            AlarmSeverity::Minor.color_for_theme(&Theme::dark()),
            AlarmSeverity::Minor.color_for_theme(&light)
        );
    }
}
