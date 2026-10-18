//! Compact `.ngui` file DSL.
//!
//! Parses PascalCase widget markup into the same retained [`Ui`] as the Rust
//! FlexBuilder API. See `docs/ngui-widgets.md` for the keyword list and limits.
//!
//! **Exception:** raw `ImageRgba` pixel buffers are Rust-only; use
//! `ImagePlaceholder` in `.ngui`.

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
}
