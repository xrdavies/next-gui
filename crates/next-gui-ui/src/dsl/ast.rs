//! AST for `.ngui` documents.

use next_gui_scene::Color;

use crate::ThemeMode;

/// Top-level document: optional theme + root Column + optional Ui overlays.
#[derive(Debug)]
pub struct Document {
    /// Optional `theme: light|dark` directive (document-level or root Column prop).
    pub theme: Option<ThemeMode>,
    pub root: AstWidget,
    pub overlays: Vec<AstWidget>,
}

/// One widget invocation.
#[derive(Debug, Clone)]
pub struct AstWidget {
    pub kind: String,
    pub name: Option<String>,
    /// Positional args inside `(...)`.
    pub args: Vec<AstValue>,
    /// `key: value` properties (style / seed state).
    pub props: Vec<(String, AstValue)>,
    /// Nested child widgets inside `{ ... }`.
    pub children: Vec<AstWidget>,
    pub line: usize,
    pub column: usize,
}

/// Literal / structured value.
#[derive(Debug, Clone)]
pub enum AstValue {
    String(String),
    Number(f32),
    Bool(bool),
    Color(Color),
    Ident(String),
    List(Vec<AstValue>),
    Object(Vec<(String, AstValue)>),
}
