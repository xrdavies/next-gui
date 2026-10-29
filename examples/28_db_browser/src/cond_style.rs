//! Persisted typography and editable, validated conditional-format style fields.
use next_gui_scene::Color;
use next_gui_text::{load_system_font, system_font_families, Font};
use next_gui_ui::{TableCellStyle, TableTextAlign};

pub(crate) const ALIGNMENTS: [&str; 4] = ["Left", "Right", "Center", "Justify"];

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Style {
    pub foreground_rgb: Option<u32>,
    pub family: String,
    pub size: Option<u16>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub alignment: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Draft {
    pub foreground: String,
    pub family: String,
    pub size: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub alignment: usize,
}

pub(crate) fn rgb(value: u32) -> Color {
    Color::rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

impl From<&Style> for Draft {
    fn from(style: &Style) -> Self {
        Self {
            foreground: style
                .foreground_rgb
                .map(|value| format!("#{value:06X}"))
                .unwrap_or_default(),
            family: style.family.clone(),
            size: style
                .size
                .map(|value| value.to_string())
                .unwrap_or_default(),
            bold: style.bold,
            italic: style.italic,
            underline: style.underline,
            alignment: style.alignment,
        }
    }
}

impl Draft {
    pub(crate) fn validate(&self) -> Result<Style, String> {
        let foreground_rgb = if self.foreground.is_empty() {
            None
        } else {
            let text = self
                .foreground
                .strip_prefix('#')
                .unwrap_or(&self.foreground);
            if text.len() != 6 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err("foreground color must be #RRGGBB or blank for default".into());
            }
            Some(u32::from_str_radix(text, 16).unwrap())
        };
        let size = if self.size.is_empty() {
            None
        } else {
            Some(
                self.size
                    .parse::<u16>()
                    .ok()
                    .filter(|value| (1..=99).contains(value))
                    .ok_or("font size must be1–99 points or blank for default")?,
            )
        };
        if self.alignment >= ALIGNMENTS.len() {
            return Err("choose a supported alignment".into());
        }
        if self.family.contains(['\0', '\n', '\r']) {
            return Err("font family must be a single name".into());
        }
        Ok(Style {
            foreground_rgb,
            family: self.family.clone(),
            size,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            alignment: self.alignment,
        })
    }

    pub(crate) fn families(&self) -> Vec<String> {
        let mut families = vec!["Default".into()];
        if !self.family.is_empty() && !system_font_families().contains(&self.family) {
            families.push(self.family.clone());
        }
        families.extend_from_slice(system_font_families());
        families
    }
}

impl Style {
    pub(crate) fn cell(&self, background: u32, fallback: &Font) -> TableCellStyle {
        let font = if !self.family.is_empty() {
            Some(
                load_system_font(&self.family, self.bold, self.italic)
                    .map(|font| {
                        font.with_fallback(
                            fallback
                                .clone()
                                .with_synthetic_style(self.bold, self.italic),
                        )
                    })
                    .unwrap_or_else(|_| {
                        fallback
                            .clone()
                            .with_synthetic_style(self.bold, self.italic)
                    }),
            )
        } else if self.bold || self.italic {
            Some(
                fallback
                    .clone()
                    .with_synthetic_style(self.bold, self.italic),
            )
        } else {
            None
        };
        TableCellStyle {
            background: Some(rgb(background)),
            foreground: self.foreground_rgb.map(rgb),
            font,
            // Qt's base logical DPI is72 on macOS and96 on Windows/X11.
            size: self
                .size
                .filter(|size| (1..=99).contains(size))
                .map(|size| {
                    size as f32
                        * if cfg!(target_os = "macos") {
                            1.0
                        } else {
                            96.0 / 72.0
                        }
                }),
            underline: self.underline,
            alignment: match self.alignment {
                1 => TableTextAlign::Right,
                2 => TableTextAlign::Center,
                3 => TableTextAlign::Justify,
                _ => TableTextAlign::Left,
            },
        }
    }
}

pub(crate) fn prepare_qa(state: &mut crate::state::AppState) {
    crate::cond_format::prepare_qa(state);
    state
        .conn
        .as_ref()
        .unwrap()
        .execute("UPDATE names SET name='alpha 中文' WHERE amount=1", [])
        .unwrap();
    state.refresh_browse().unwrap();
    let family = ["Menlo", "DejaVu Sans Mono", "Consolas"]
        .into_iter()
        .find(|family| system_font_families().iter().any(|name| name == family))
        .unwrap_or_default();
    state.cond_rules[0].op = 5;
    state.cond_rules[0].value = "2~4".into();
    state.cond_rules[0].style = Style {
        foreground_rgb: Some(0xF5F0DC),
        family: family.into(),
        size: Some(18),
        bold: true,
        italic: true,
        underline: true,
        alignment: 1,
    };
    state.cond_rules[1].style = Style {
        foreground_rgb: Some(0xEAF4FF),
        size: Some(14),
        alignment: 2,
        ..Default::default()
    };
    state.cond_rules[2].style = Style {
        foreground_rgb: Some(0x102B22),
        family: if system_font_families()
            .iter()
            .any(|family| family == "Times New Roman")
        {
            "Times New Roman".into()
        } else {
            String::new()
        },
        italic: true,
        size: Some(16),
        ..Default::default()
    };
    state.begin_cond_formats().unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        browse_filter::tests::{capture, fixture},
        state::OverlayKind,
    };
    use next_gui_ui::{Theme, UiKey};

    #[test]
    fn styles_validate_atomically_and_round_trip_legacy_json_and_project_xml() {
        let legacy: crate::state::CondFormatRule = serde_json::from_str(
            r#"{"schema":"main","table":"t","column":"v","op":5,"value":"","color_rgb":1193046}"#,
        )
        .unwrap();
        assert_eq!(legacy.style, Style::default());
        let mut state = crate::state::AppState::empty();
        prepare_qa(&mut state);
        let original = state.cond_rules.clone();
        for foreground in ["#GG0000", "#12345", "#1234567", " red"] {
            state.cond_style.foreground = foreground.into();
            assert!(state
                .save_cond_formats()
                .unwrap_err()
                .contains("foreground"));
            assert_eq!(state.cond_rules, original);
        }
        state.cond_style.foreground = "#aAbBcC".into();
        for size in ["0", "100", "1.5", "NaN", " 18"] {
            state.cond_style.size = size.into();
            assert!(state.save_cond_formats().unwrap_err().contains("font size"));
            assert_eq!(state.cond_rules, original);
        }
        state.cond_style.size = "20".into();
        state.cond_style.alignment = 4;
        assert!(state.save_cond_formats().unwrap_err().contains("alignment"));
        state.cond_style.alignment = 3;
        state.cond_style.family = "font\nname".into();
        assert!(state.save_cond_formats().unwrap_err().contains("family"));
        state.cond_style.family = "Unavailable project font 中文".into();
        state.save_cond_formats().unwrap();
        assert_eq!(state.cond_rules[0].style.foreground_rgb, Some(0xAABBCC));
        assert_eq!(state.cond_rules[0].style.size, Some(20));
        let project = crate::project::ProjectFile::from_state(&state);
        let json = serde_json::to_string(&project).unwrap();
        let restored: crate::project::ProjectFile = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.cond_rules, state.cond_rules);
        let xml =
            crate::project_xml::encode(&project, std::path::Path::new("/tmp/style-project.sqbpro"))
                .unwrap();
        assert_eq!(
            crate::project_xml::decode(&xml).unwrap().cond_rules,
            state.cond_rules
        );
        let font = next_gui_text::load_default_cjk_font().unwrap();
        let styles = crate::cond_match::styles(&state, &font);
        let style = styles[1][1].as_ref().unwrap();
        assert_eq!(style.foreground, Some(rgb(0xAABBCC)));
        assert_eq!(style.alignment, TableTextAlign::Justify);
        assert!(style.font.as_ref().unwrap().has_glyph('中'));
        assert_eq!(
            state
                .conn
                .as_ref()
                .unwrap()
                .query_row("SELECT count(*) FROM names", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            5
        );
        assert!(!state.conn.as_ref().unwrap().is_autocommit());
    }

    #[test]
    fn conditional_style_controls_render_save_cancel_and_keep_editing_sessions() {
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400, 860), (800, 600)] {
                let mut demo = fixture(width, height, theme);
                prepare_qa(&mut demo.state);
                if width == 800 {
                    demo.state.prefs.font_size = 18.0;
                }
                demo.do_rebuild();
                let original = demo.state.cond_rules.clone();
                capture(&mut demo, &format!("styles-manager-{theme_name}-{width}"));
                let controls = demo.ids.cond_formats;
                demo.ui.set_focus(Some(controls.family));
                demo.ui.key_input(UiKey::Enter);
                demo.ui.sync(&mut demo.scene);
                capture(&mut demo, &format!("styles-fonts-{theme_name}-{width}"));
                demo.ui.key_input(UiKey::Escape);
                demo.ui.set_text_input(controls.foreground, "#F8D56B");
                demo.ui.set_text_input(controls.size, "20");
                demo.ui.set_checked(controls.bold, false);
                demo.ui.set_checked(controls.italic, false);
                demo.ui.set_checked(controls.underline, false);
                assert!(demo.ui.set_select_selected(controls.family, Some(0)));
                assert!(demo.ui.set_select_selected(controls.alignment, Some(2)));
                demo.ui.set_focus(Some(controls.foreground));
                demo.ui.select_text_input_range(controls.foreground, 1..3);
                demo.snapshot_inputs();
                demo.do_rebuild();
                assert_eq!(demo.ui.focused(), Some(demo.ids.cond_formats.foreground));
                assert_eq!(
                    demo.ui
                        .text_input_selection(demo.ids.cond_formats.foreground),
                    Some(1..3)
                );
                assert_eq!(demo.state.cond_rules, original);
                demo.state.overlay = None;
                demo.state.cond_formats = None;
                demo.state.begin_cond_formats().unwrap();
                demo.do_rebuild();
                assert_eq!(demo.state.cond_style.foreground, "#F5F0DC");
                demo.ui
                    .set_text_input(demo.ids.cond_formats.foreground, "#F8D56B");
                demo.ui.set_text_input(demo.ids.cond_formats.size, "20");
                demo.ui.set_checked(demo.ids.cond_formats.bold, false);
                demo.ui.set_checked(demo.ids.cond_formats.italic, false);
                demo.ui.set_checked(demo.ids.cond_formats.underline, false);
                demo.ui
                    .set_select_selected(demo.ids.cond_formats.family, Some(0));
                demo.ui
                    .set_select_selected(demo.ids.cond_formats.alignment, Some(2));
                demo.snapshot_inputs();
                demo.state.save_cond_formats().unwrap();
                demo.do_rebuild();
                assert!(demo.state.overlay.is_none());
                let style = &demo.state.cond_rules[0].style;
                assert_eq!(style.foreground_rgb, Some(0xF8D56B));
                assert_eq!(style.size, Some(20));
                assert!(
                    !style.bold && !style.italic && !style.underline && style.family.is_empty()
                );
                assert_eq!(style.alignment, 2);
                capture(&mut demo, &format!("styles-saved-{theme_name}-{width}"));
                demo.state.begin_cond_formats().unwrap();
                demo.do_rebuild();
                let align = demo.ids.cond_formats.alignment;
                demo.ui.set_focus(Some(align));
                demo.ui.sync(&mut demo.scene);
                assert!(demo
                    .ui
                    .accessible_name(align)
                    .unwrap()
                    .contains("alignment"));
                assert_eq!(demo.state.overlay, Some(OverlayKind::CondFormat));
                capture(&mut demo, &format!("styles-controls-{theme_name}-{width}"));
                demo.ui.set_focus(Some(demo.ids.ov_cond_val));
                let mut steps = 0;
                while demo.ui.focused() != Some(demo.ids.ov_btn_ok) && steps < 24 {
                    demo.ui.key_input(UiKey::Tab);
                    steps += 1;
                }
                assert!(steps < 24);
                eprintln!("Style native value-to-Save tab steps: {steps}");
            }
        }
    }
}
