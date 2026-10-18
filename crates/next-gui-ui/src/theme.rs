//! Theme tokens: colors, typography, spacing, radii.
//!
//! Profile (Field/Office) stays performance/capability. Appearance lives here and in
//! [`crate::UiConfig`] (locale / density / accent override).
//!
//! MVP: light + dark presets; `XxxStyle::from_theme*` for a representative set;
//! [`crate::Ui::set_theme`] + convenience style helpers. Explicit Style args still win.
//! Remaining styles keep hard-coded [`Default`] (≈ dark) until migrated — see
//! `docs/theme-tokens.md`.

use next_gui_layout::CrossAlign;
use next_gui_scene::Color;

use crate::{
    CalendarStyle, ColumnStyle, LinkStyle, PanelStyle, RowStyle, StatusBarStyle, TextInputStyle,
    ToolbarStyle, UiDensity,
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

/// Semantic color tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeColors {
    /// Page / column surface.
    pub surface: Color,
    /// Elevated strip (row / toolbar chrome).
    pub surface_elevated: Color,
    /// Panel body fill.
    pub panel: Color,
    /// Panel title bar.
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
    /// Borders and chrome lines.
    pub border: Color,
    /// Focus ring.
    pub focus: Color,
    /// Input field background.
    pub input_bg: Color,
    /// Placeholder text.
    pub placeholder: Color,
    /// Link color (often a lighter accent).
    pub link: Color,
    /// Status / muted bar fill.
    pub muted: Color,
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

/// Corner radii (px). Stored for future chrome; few styles consume them in MVP.
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
                border: Color::rgb(0x4a, 0x4a, 0x54),
                focus: Color::rgb(0x3b, 0x82, 0xf6),
                input_bg: Color::rgb(0x1e, 0x1e, 0x22),
                placeholder: Color::rgb(0x78, 0x80, 0x8c),
                link: Color::rgb(0x6a, 0xb0, 0xf0),
                muted: Color::rgb(0x2c, 0x2c, 0x30),
            },
            typography: ThemeTypography {
                body: 22.0,
                title: 14.0,
                button: 18.0,
                button_sm: 14.0,
                caption: 12.0,
                input: 14.0,
            },
            spacing: ThemeSpacing {
                padding_lg: 16.0,
                padding_md: 10.0,
                padding_sm: 8.0,
                gap_lg: 12.0,
                gap_md: 8.0,
                gap_sm: 6.0,
            },
            radii: ThemeRadii {
                control: 4.0,
                panel: 6.0,
            },
        }
    }

    /// Light preset (inverse surfaces; shared accent/button blues).
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
                border: Color::rgb(0xc4, 0xc4, 0xcc),
                focus: Color::rgb(0x3b, 0x82, 0xf6),
                input_bg: Color::rgb(0xff, 0xff, 0xff),
                placeholder: Color::rgb(0x98, 0x98, 0xa4),
                link: Color::rgb(0x1a, 0x6a, 0xc8),
                muted: Color::rgb(0xec, 0xec, 0xf0),
            },
            typography: ThemeTypography {
                body: 22.0,
                title: 14.0,
                button: 18.0,
                button_sm: 14.0,
                caption: 12.0,
                input: 14.0,
            },
            spacing: ThemeSpacing {
                padding_lg: 16.0,
                padding_md: 10.0,
                padding_sm: 8.0,
                gap_lg: 12.0,
                gap_md: 8.0,
                gap_sm: 6.0,
            },
            radii: ThemeRadii {
                control: 4.0,
                panel: 6.0,
            },
        }
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

impl ColumnStyle {
    /// Defaults from theme with [`UiDensity::Default`] spacing.
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density-scaled padding/gap.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let s = density.spacing_scale();
        Self {
            x: 24.0,
            y: 24.0,
            width: 400.0,
            height: 560.0,
            padding: scale(theme.spacing.padding_lg, s),
            gap: scale(theme.spacing.gap_lg, s),
            cross_align: CrossAlign::Start,
            background: Some(theme.colors.surface),
        }
    }
}

impl RowStyle {
    /// Defaults from theme with [`UiDensity::Default`] spacing.
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density-scaled padding/gap.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let s = density.spacing_scale();
        Self {
            x: 0.0,
            y: 0.0,
            width: 600.0,
            height: scale(48.0, s),
            padding: scale(theme.spacing.padding_sm, s),
            gap: scale(theme.spacing.gap_md, s),
            cross_align: CrossAlign::Center,
            background: Some(theme.colors.surface_elevated),
        }
    }
}

impl ToolbarStyle {
    /// Defaults from theme with [`UiDensity::Default`] spacing.
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density-scaled padding/gap/height.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let s = density.spacing_scale();
        Self {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: scale(40.0, s),
            padding: scale(6.0, s),
            gap: scale(theme.spacing.gap_sm, s),
            cross_align: CrossAlign::Center,
            background: Some(theme.colors.surface_elevated),
        }
    }
}

impl PanelStyle {
    /// Defaults from theme with [`UiDensity::Default`] spacing.
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density-scaled padding/gap/title.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let s = density.spacing_scale();
        let f = density.font_scale();
        Self {
            x: 0.0,
            y: 0.0,
            width: 320.0,
            height: 400.0,
            padding: scale(theme.spacing.padding_md, s),
            gap: scale(theme.spacing.gap_md, s),
            cross_align: CrossAlign::Start,
            background: Some(theme.colors.panel),
            title_height: scale(28.0, s),
            title_bg: theme.colors.title_bar,
            title_fg: theme.colors.text_primary,
            title_size: scale(theme.typography.title, f),
        }
    }
}

impl StatusBarStyle {
    /// Defaults from theme with [`UiDensity::Default`] spacing.
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density-scaled padding/gap/height.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let s = density.spacing_scale();
        Self {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: scale(26.0, s),
            padding: scale(6.0, s),
            gap: scale(theme.spacing.gap_lg, s),
            cross_align: CrossAlign::Center,
            background: Some(theme.colors.muted),
        }
    }
}

impl TextInputStyle {
    /// Defaults from theme with [`UiDensity::Default`] spacing.
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme colors/sizes with density-scaled height/padding/font.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let s = density.spacing_scale();
        let f = density.font_scale();
        Self {
            width: 220.0,
            height: scale(32.0, s),
            size: scale(theme.typography.input, f),
            fg: theme.colors.text_primary,
            placeholder_fg: theme.colors.placeholder,
            bg: theme.colors.input_bg,
            border: theme.colors.border,
            focus_border: theme.colors.focus,
            padding_x: scale(theme.spacing.padding_sm, s),
        }
    }
}

impl LinkStyle {
    /// Defaults from theme (link color + caption-ish size).
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme link color with density-scaled font.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let f = density.font_scale();
        Self {
            size: scale(theme.typography.input, f),
            fg: theme.colors.link,
            underline: true,
        }
    }
}

impl CalendarStyle {
    /// Defaults from theme (accent from theme; [`crate::UiConfig::accent`] still overrides at paint).
    pub fn from_theme(theme: &Theme) -> Self {
        Self::from_theme_with_density(theme, UiDensity::Default)
    }

    /// Theme calendar chrome with density-scaled cell/padding/font.
    pub fn from_theme_with_density(theme: &Theme, density: UiDensity) -> Self {
        let s = density.spacing_scale();
        let f = density.font_scale();
        Self {
            width: 280.0,
            height: 260.0,
            cell: scale(32.0, s),
            size: scale(theme.typography.caption, f),
            fg: theme.colors.text_primary,
            accent: theme.colors.accent,
            bg: theme.colors.surface_elevated,
            header_bg: theme.colors.surface,
            padding: scale(theme.spacing.padding_sm, s),
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
        // Shared brand blues for continuity.
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
}
