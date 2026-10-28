//! Compact `.ngui` file DSL.
//!
//! Parses PascalCase widget markup into the same retained [`Ui`] as the Rust
//! FlexBuilder API. See `docs/ngui-widgets.md` for the keyword list and limits.
//!
//! **Exception:** raw `ImageRgba` pixel buffers are Rust-only; use
//! `ImagePlaceholder` in `.ngui`.
//!
//! Optional `theme: light|dark` (document-level before root `Column`, or as a
//! root Column prop) selects [`crate::Theme`]; widget Style defaults use
//! `from_theme` (explicit color props still win). Color props also accept
//! unquoted [`crate::ThemeColors`] token names (e.g. `background: surface`).

mod ast;
mod build;
mod lex;
mod parse;
mod style;

use std::collections::HashMap;
use std::path::Path;

use crate::{Ui, WidgetId};

#[allow(unused_imports)]
pub use ast::{AstValue, AstWidget, Document};

/// Result of loading a `.ngui` document into a [`Ui`].
#[derive(Debug)]
pub struct LoadedUi {
    /// Retained widget tree ready for [`Ui::sync`].
    pub ui: Ui,
    /// `@name` → widget id.
    pub names: HashMap<String, WidgetId>,
}

impl LoadedUi {
    /// Look up a named widget (`@label` → `"label"`).
    pub fn widget(&self, name: &str) -> Option<WidgetId> {
        self.names.get(name).copied()
    }

    /// Require a named widget or return an error string.
    pub fn require(&self, name: &str) -> Result<WidgetId, String> {
        self.widget(name)
            .ok_or_else(|| format!(".ngui: missing named widget @{name}"))
    }
}

/// Parse error with 1-based line/column when available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DslError {
    /// Human-readable message.
    pub message: String,
    /// 1-based line (0 if unknown).
    pub line: usize,
    /// 1-based column (0 if unknown).
    pub column: usize,
}

impl std::fmt::Display for DslError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line > 0 {
            write!(
                f,
                "{} at line {}, column {}",
                self.message, self.line, self.column
            )
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for DslError {}

/// Load `.ngui` source into a [`LoadedUi`].
pub fn load_ngui(source: &str) -> Result<LoadedUi, DslError> {
    let doc = parse::parse_document(source)?;
    build::document_to_ui(doc)
}

/// Read a `.ngui` file from disk and load it.
pub fn load_ngui_path(path: impl AsRef<Path>) -> Result<LoadedUi, DslError> {
    let path = path.as_ref();
    let source = std::fs::read_to_string(path).map_err(|e| DslError {
        message: format!("failed to read {}: {e}", path.display()),
        line: 0,
        column: 0,
    })?;
    load_ngui(&source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_counter_ngui() {
        let src = r#"
Column {
  x: 40
  y: 40
  width: 360
  height: 520
  padding: 20
  gap: 14
  background: #2a2a2e

  Text("Count: 0") @label
  Button("Increment") @inc
  Button("Reset") @reset {
    bg: #6b7280
    fg: #ffffff
  }
}
"#;
        let loaded = load_ngui(src).expect("parse");
        assert!(loaded.widget("label").is_some());
        assert!(loaded.widget("inc").is_some());
        assert!(loaded.widget("reset").is_some());
        assert!(loaded.ui.is_button(loaded.widget("inc").unwrap()));
        assert_eq!(
            loaded.ui.text_content(loaded.widget("label").unwrap()),
            Some("Count: 0")
        );
    }

    #[test]
    fn reject_unknown_prop() {
        let err = load_ngui("Column { foo: 1 }").unwrap_err();
        assert!(err.message.contains("unknown Column property"));
    }

    #[test]
    fn parse_row_toolbar() {
        let src = r#"
Column {
  width: 640
  height: 400
  Row {
    width: 600
    height: 44
    gap: 8
    cross_align: center
    Button("Open") @open
    Spacer { width: 12 height: 1 }
    Separator { vertical: true length: 24 }
    Button("Save") @save
  }
  Text("body") @body
}
"#;
        let loaded = load_ngui(src).expect("parse");
        assert!(loaded.widget("open").is_some());
        assert!(loaded.widget("save").is_some());
        assert!(loaded.ui.is_button(loaded.widget("open").unwrap()));
    }

    #[test]
    fn text_area_line_numbers_property_uses_boolean_validation() {
        for (flag, expected) in [("true", true), ("false", false)] {
            let loaded = load_ngui(&format!("Column {{ TextArea @sql {{ line_numbers: {flag} }} }}")).unwrap();
            let id = loaded.widget("sql").unwrap();
            let crate::Kind::TextArea { line_numbers, .. } = loaded.ui.widgets[&id].kind else { panic!() };
            assert_eq!(line_numbers, expected);
        }
        let error = load_ngui("Column { TextArea { line_numbers: \"yes\" } }").unwrap_err();
        assert!(error.message.contains("line_numbers: expected bool"));
    }

    #[test]
    fn tabs_strip_properties_use_boolean_validation() {
        for (flag, expected) in [("true", true), ("false", false)] {
            let src = format!("Column {{ Tabs @tabs {{ scrollable: {flag} strip_at_bottom: {flag} Tab(\"One\") {{ Text(\"p1\") }} }} }}");
            let loaded = load_ngui(&src).unwrap();
            let id = loaded.widget("tabs").unwrap();
            let crate::Kind::Tabs { style, .. } = &loaded.ui.widgets[&id].kind else { panic!("expected Tabs"); };
            assert_eq!(style.scrollable, expected);
            assert_eq!(style.strip_at_bottom, expected);
        }
        let error = load_ngui("Column { Tabs { scrollable: \"yes\" } }").unwrap_err();
        assert!(error.message.contains("scrollable: expected bool"));
        let error = load_ngui("Column { Tabs { strip_at_bottom: \"yes\" } }").unwrap_err();
        assert!(error.message.contains("strip_at_bottom: expected bool"));
    }

    #[test]
    fn chart_coordinate_property_uses_boolean_validation() {
        for (flag, expected) in [("true", true), ("false", false)] {
            let loaded = load_ngui(&format!("Column {{ Chart @chart {{ show_plot_coordinates: {flag} }} }}")).unwrap();
            let id = loaded.widget("chart").unwrap();
            let crate::Kind::Chart { style, .. } = &loaded.ui.widgets[&id].kind else { panic!() };
            assert_eq!(style.show_plot_coordinates, expected);
        }
        let error = load_ngui("Column { Chart { show_plot_coordinates: \"yes\" } }").unwrap_err();
        assert!(error.message.contains("show_plot_coordinates: expected bool"));
    }

    #[test]
    fn button_icon_keeps_label_and_validates_named_glyphs() {
        let loaded = load_ngui("Column { Button(\"Save database\") @save { icon: save } }").unwrap();
        let id = loaded.widget("save").unwrap();
        let crate::Kind::Button { icon, .. } = loaded.ui.widgets[&id].kind else { panic!() };
        assert_eq!(icon, Some(crate::IconName::Save));
        assert_eq!(loaded.ui.accessible_name(id).as_deref(), Some("Save database"));
        for value in ["missing", "blank", "123"] {
            assert!(load_ngui(&format!("Column {{ Button(\"Save\") {{ icon: {value} }} }}")).is_err());
        }
    }

    #[test]
    fn load_all_widgets_fixture() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/all_widgets.ngui"
        );
        let loaded = load_ngui_path(path).expect("all_widgets.ngui");
        // Spot-check a spread of named widgets from the fixture.
        for name in [
            "ti", "cb", "rd", "sw", "lst", "tr", "tbl", "menu", "tabs", "sel", "sld",
            "tag", "badge", "prog", "al", "gauge", "trend", "chart", "ta", "num",
            "card", "link", "steps", "cal", "img", "modal", "drawer",
        ] {
            assert!(
                loaded.widget(name).is_some(),
                "missing @{name} in all_widgets fixture"
            );
        }
        assert!(loaded.ui.is_text_input(loaded.widget("ti").unwrap()));
        assert!(loaded.ui.is_list(loaded.widget("lst").unwrap()));
        assert!(loaded.ui.is_table(loaded.widget("tbl").unwrap()));
        assert!(loaded.ui.is_tabs(loaded.widget("tabs").unwrap()));
        assert!(loaded.ui.is_modal(loaded.widget("modal").unwrap()));
        assert!(loaded.ui.is_form(loaded.widget("form").unwrap()));
        assert!(loaded.ui.is_chart(loaded.widget("chart").unwrap()));
    }

    #[test]
    fn theme_document_directive_light_vs_dark() {
        let dark_src = r#"
theme: dark
Column @root {
  width: 320
  height: 200
  Text("hi") @label
  Button("Go") @btn
  TextInput("") @ti
}
"#;
        let light_src = r#"
theme: light
Column @root {
  width: 320
  height: 200
  Text("hi") @label
  Button("Go") @btn
  TextInput("") @ti
}
"#;
        let dark = load_ngui(dark_src).expect("dark");
        let light = load_ngui(light_src).expect("light");
        assert_eq!(dark.ui.theme().mode, crate::ThemeMode::Dark);
        assert_eq!(light.ui.theme().mode, crate::ThemeMode::Light);
        assert_ne!(
            dark.ui.theme().colors.surface,
            light.ui.theme().colors.surface
        );
        // Root column background follows theme surface (no explicit background prop).
        let dark_root = dark.require("root").unwrap();
        let light_root = light.require("root").unwrap();
        match (
            &dark.ui.widgets.get(&dark_root).unwrap().kind,
            &light.ui.widgets.get(&light_root).unwrap().kind,
        ) {
            (
                crate::Kind::Column { style: ds, .. },
                crate::Kind::Column { style: ls, .. },
            ) => {
                assert_eq!(ds.background, Some(crate::Theme::dark().colors.surface));
                assert_eq!(ls.background, Some(crate::Theme::light().colors.surface));
                assert_ne!(ds.background, ls.background);
            }
            other => panic!("expected Column roots, got {other:?}"),
        }
        // TextInput bg differs light vs dark.
        let dark_ti = dark.require("ti").unwrap();
        let light_ti = light.require("ti").unwrap();
        match (
            &dark.ui.widgets.get(&dark_ti).unwrap().kind,
            &light.ui.widgets.get(&light_ti).unwrap().kind,
        ) {
            (
                crate::Kind::TextInput { style: ds, .. },
                crate::Kind::TextInput { style: ls, .. },
            ) => {
                assert_eq!(ds.bg, crate::Theme::dark().colors.input_bg);
                assert_eq!(ls.bg, crate::Theme::light().colors.input_bg);
                assert_ne!(ds.bg, ls.bg);
            }
            other => panic!("expected TextInput, got {other:?}"),
        }
    }

    #[test]
    fn theme_root_column_prop_overrides_directive() {
        let src = r#"
theme: dark
Column {
  theme: light
  width: 200
  height: 100
}
"#;
        let loaded = load_ngui(src).expect("parse");
        assert_eq!(loaded.ui.theme().mode, crate::ThemeMode::Light);
    }

    #[test]
    fn reject_unknown_theme_name() {
        let err = load_ngui("theme: neon\nColumn { width: 1 height: 1 }").unwrap_err();
        assert!(
            err.message.contains("unknown theme") || err.message.contains("neon"),
            "msg={}",
            err.message
        );
    }

    #[test]
    fn explicit_background_overrides_theme_surface() {
        let src = r#"
theme: light
Column @root {
  width: 200
  height: 100
  background: #112233
}
"#;
        let loaded = load_ngui(src).expect("parse");
        let root = loaded.require("root").unwrap();
        match &loaded.ui.widgets.get(&root).unwrap().kind {
            crate::Kind::Column { style, .. } => {
                assert_eq!(style.background, Some(next_gui_scene::Color::rgb(0x11, 0x22, 0x33)));
            }
            other => panic!("expected Column, got {other:?}"),
        }
    }

    #[test]
    fn theme_color_token_name_surface_light_vs_dark() {
        let dark_src = r#"
theme: dark
Column @root {
  width: 200
  height: 100
  background: surface
}
"#;
        let light_src = r#"
theme: light
Column @root {
  width: 200
  height: 100
  background: surface
}
"#;
        let dark = load_ngui(dark_src).expect("dark");
        let light = load_ngui(light_src).expect("light");
        let dark_root = dark.require("root").unwrap();
        let light_root = light.require("root").unwrap();
        match (
            &dark.ui.widgets.get(&dark_root).unwrap().kind,
            &light.ui.widgets.get(&light_root).unwrap().kind,
        ) {
            (
                crate::Kind::Column { style: ds, .. },
                crate::Kind::Column { style: ls, .. },
            ) => {
                assert_eq!(ds.background, Some(crate::Theme::dark().colors.surface));
                assert_eq!(ls.background, Some(crate::Theme::light().colors.surface));
                assert_ne!(ds.background, ls.background);
            }
            other => panic!("expected Column roots, got {other:?}"),
        }
    }

    #[test]
    fn theme_color_token_button_bg_accent() {
        let src = r#"
theme: light
Column {
  width: 200
  height: 100
  Button("Go") @btn { bg: accent fg: on_accent }
}
"#;
        let loaded = load_ngui(src).expect("parse");
        let btn = loaded.require("btn").unwrap();
        match &loaded.ui.widgets.get(&btn).unwrap().kind {
            crate::Kind::Button { bg, fg, .. } => {
                assert_eq!(*bg, crate::Theme::light().colors.accent);
                assert_eq!(*fg, crate::Theme::light().colors.on_accent);
            }
            other => panic!("expected Button, got {other:?}"),
        }
    }

    #[test]
    fn reject_unknown_color_token() {
        let src = r#"
theme: dark
Column { width: 1 height: 1 background: neon_glow }
"#;
        let err = load_ngui(src).unwrap_err();
        assert!(
            err.message.contains("unknown color token") || err.message.contains("neon_glow"),
            "msg={}",
            err.message
        );
    }
}
