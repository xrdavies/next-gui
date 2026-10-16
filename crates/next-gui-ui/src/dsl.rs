//! Compact `.ngui` file DSL (P1).
//!
//! Parses a small Column / Text / Button markup and builds a retained [`Ui`]
//! via the same builders as the Rust API. No bindings, no GPU, no CJK.

use std::collections::HashMap;
use std::path::Path;

use next_gui_scene::Color;

use crate::{ColumnStyle, Ui, WidgetId};

/// Result of loading a `.ngui` document into a [`Ui`].
#[derive(Debug)]
pub struct LoadedUi {
    /// Retained widget tree ready for [`Ui::sync`].
    pub ui: Ui,
    /// `@name` → widget id (Text / Button / nested Column).
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
    let mut parser = Parser::new(source);
    let ast = parser.parse_document()?;
    ast_to_ui(ast)
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

// --- AST ---

#[derive(Debug)]
struct AstColumn {
    name: Option<String>,
    style: ColumnStyle,
    children: Vec<AstChild>,
}

#[derive(Debug)]
enum AstChild {
    Text {
        name: Option<String>,
        content: String,
    },
    Button {
        name: Option<String>,
        label: String,
        bg: Option<Color>,
        fg: Option<Color>,
    },
    Column(AstColumn),
}

fn ast_to_ui(root: AstColumn) -> Result<LoadedUi, DslError> {
    let mut ui = Ui::new();
    let mut names = HashMap::new();
    let AstColumn {
        name,
        style,
        children,
    } = root;
    let root_id = ui.column(style, |builder| {
        build_children(builder, &mut names, children);
    });
    if let Some(n) = name {
        names.insert(n, root_id);
    }
    Ok(LoadedUi { ui, names })
}

fn build_children(
    builder: &mut crate::ColumnBuilder<'_>,
    names: &mut HashMap<String, WidgetId>,
    children: Vec<AstChild>,
) {
    for child in children {
        match child {
            AstChild::Text { name, content } => {
                let id = builder.text(content);
                if let Some(n) = name {
                    names.insert(n, id);
                }
            }
            AstChild::Button {
                name,
                label,
                bg,
                fg,
            } => {
                let id = match (bg, fg) {
                    (Some(bg), Some(fg)) => builder.button_styled(label, bg, fg),
                    (Some(bg), None) => {
                        builder.button_styled(label, bg, Color::rgb(0xff, 0xff, 0xff))
                    }
                    (None, Some(fg)) => {
                        builder.button_styled(label, Color::rgb(0x3b, 0x82, 0xf6), fg)
                    }
                    (None, None) => builder.button(label),
                };
                if let Some(n) = name {
                    names.insert(n, id);
                }
            }
            AstChild::Column(nested) => {
                let AstColumn {
                    name,
                    style,
                    children,
                } = nested;
                let id = builder.column(style, |inner| {
                    build_children(inner, names, children);
                });
                if let Some(n) = name {
                    names.insert(n, id);
                }
            }
        }
    }
}

// --- Lexer / Parser ---

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    String(String),
    Number(f32),
    Color(Color),
    At,       // @
    Colon,    // :
    LBrace,   // {
    RBrace,   // }
    LParen,   // (
    RParen,   // )
    Eof,
}

struct Parser<'a> {
    src: &'a str,
    /// byte offset
    i: usize,
    line: usize,
    col: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            i: 0,
            line: 1,
            col: 1,
        }
    }

    fn err(&self, message: impl Into<String>) -> DslError {
        DslError {
            message: message.into(),
            line: self.line,
            column: self.col,
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.src[self.i..].chars().next()
    }

    fn bump_char(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.i += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn skip_ws_and_comments(&mut self) {
        loop {
            while let Some(c) = self.peek_char() {
                if c.is_whitespace() {
                    self.bump_char();
                } else {
                    break;
                }
            }
            // `#` is reserved for #RRGGBB colors; line comments use // only.
            if self.src[self.i..].starts_with("//") {
                self.bump_char();
                self.bump_char();
                while let Some(c) = self.peek_char() {
                    self.bump_char();
                    if c == '\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    fn next_tok(&mut self) -> Result<Tok, DslError> {
        self.skip_ws_and_comments();
        let start_line = self.line;
        let start_col = self.col;
        let Some(c) = self.peek_char() else {
            return Ok(Tok::Eof);
        };
        match c {
            '{' => {
                self.bump_char();
                Ok(Tok::LBrace)
            }
            '}' => {
                self.bump_char();
                Ok(Tok::RBrace)
            }
            '(' => {
                self.bump_char();
                Ok(Tok::LParen)
            }
            ')' => {
                self.bump_char();
                Ok(Tok::RParen)
            }
            ':' => {
                self.bump_char();
                Ok(Tok::Colon)
            }
            '@' => {
                self.bump_char();
                Ok(Tok::At)
            }
            '"' => self.lex_string(start_line, start_col),
            '#' => self.lex_color(start_line, start_col),
            c if c.is_ascii_digit() || c == '-' || c == '+' => self.lex_number(),
            c if is_ident_start(c) => self.lex_ident(),
            _ => Err(DslError {
                message: format!("unexpected character {c:?}"),
                line: start_line,
                column: start_col,
            }),
        }
    }

    fn lex_string(&mut self, line: usize, column: usize) -> Result<Tok, DslError> {
        self.bump_char(); // opening "
        let mut out = String::new();
        loop {
            match self.bump_char() {
                None => {
                    return Err(DslError {
                        message: "unterminated string".into(),
                        line,
                        column,
                    });
                }
                Some('"') => return Ok(Tok::String(out)),
                Some('\\') => match self.bump_char() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some(other) => out.push(other),
                    None => {
                        return Err(DslError {
                            message: "unterminated string escape".into(),
                            line,
                            column,
                        });
                    }
                },
                Some(c) => out.push(c),
            }
        }
    }

    fn lex_color(&mut self, line: usize, column: usize) -> Result<Tok, DslError> {
        self.bump_char(); // #
        let mut hex = String::new();
        while let Some(c) = self.peek_char() {
            if c.is_ascii_hexdigit() {
                hex.push(c);
                self.bump_char();
            } else {
                break;
            }
        }
        if hex.len() != 6 {
            return Err(DslError {
                message: format!("color must be #RRGGBB, got #{hex}"),
                line,
                column,
            });
        }
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap();
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap();
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap();
        Ok(Tok::Color(Color::rgb(r, g, b)))
    }

    fn lex_number(&mut self) -> Result<Tok, DslError> {
        let start = self.i;
        if matches!(self.peek_char(), Some('+') | Some('-')) {
            self.bump_char();
        }
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() || c == '.' {
                self.bump_char();
            } else {
                break;
            }
        }
        let s = &self.src[start..self.i];
        let n: f32 = s.parse().map_err(|_| self.err(format!("invalid number {s}")))?;
        Ok(Tok::Number(n))
    }

    fn lex_ident(&mut self) -> Result<Tok, DslError> {
        let mut s = String::new();
        while let Some(c) = self.peek_char() {
            if is_ident_continue(c) {
                s.push(c);
                self.bump_char();
            } else {
                break;
            }
        }
        Ok(Tok::Ident(s))
    }

    fn peek_tok(&mut self) -> Result<Tok, DslError> {
        let save = (self.i, self.line, self.col);
        let t = self.next_tok()?;
        self.i = save.0;
        self.line = save.1;
        self.col = save.2;
        Ok(t)
    }

    fn expect_ident(&mut self, want: &str) -> Result<(), DslError> {
        match self.next_tok()? {
            Tok::Ident(s) if s == want => Ok(()),
            other => Err(self.err(format!("expected `{want}`, got {other:?}"))),
        }
    }

    fn parse_document(&mut self) -> Result<AstColumn, DslError> {
        let col = self.parse_column()?;
        match self.next_tok()? {
            Tok::Eof => Ok(col),
            other => Err(self.err(format!("unexpected trailing token {other:?}"))),
        }
    }

    fn parse_name_opt(&mut self) -> Result<Option<String>, DslError> {
        if matches!(self.peek_tok()?, Tok::At) {
            self.next_tok()?; // @
            match self.next_tok()? {
                Tok::Ident(s) => Ok(Some(s)),
                _ => Err(self.err("expected identifier after `@`")),
            }
        } else {
            Ok(None)
        }
    }

    fn parse_column(&mut self) -> Result<AstColumn, DslError> {
        self.expect_ident("Column")?;
        let name = self.parse_name_opt()?;
        match self.next_tok()? {
            Tok::LBrace => {}
            other => return Err(self.err(format!("expected `{{` after Column, got {other:?}"))),
        }

        let mut style = ColumnStyle::default();
        let mut children = Vec::new();

        loop {
            match self.peek_tok()? {
                Tok::RBrace => {
                    self.next_tok()?;
                    break;
                }
                Tok::Eof => return Err(self.err("unclosed Column `{`")),
                Tok::Ident(ref id) if id == "Text" || id == "Button" || id == "Column" => {
                    children.push(self.parse_child()?);
                }
                Tok::Ident(_) => {
                    self.parse_column_prop(&mut style)?;
                }
                other => {
                    return Err(self.err(format!("unexpected token in Column: {other:?}")));
                }
            }
        }

        Ok(AstColumn {
            name,
            style,
            children,
        })
    }

    fn parse_column_prop(&mut self, style: &mut ColumnStyle) -> Result<(), DslError> {
        let key = match self.next_tok()? {
            Tok::Ident(s) => s,
            _ => return Err(self.err("expected property name")),
        };
        match self.next_tok()? {
            Tok::Colon => {}
            _ => return Err(self.err(format!("expected `:` after property `{key}`"))),
        }
        match key.as_str() {
            "x" => style.x = self.expect_number()?,
            "y" => style.y = self.expect_number()?,
            "width" => style.width = self.expect_number()?,
            "height" => style.height = self.expect_number()?,
            "padding" => style.padding = self.expect_number()?,
            "gap" => style.gap = self.expect_number()?,
            "background" => {
                style.background = Some(self.expect_color()?);
            }
            other => {
                return Err(self.err(format!("unknown Column property `{other}`")));
            }
        }
        Ok(())
    }

    fn expect_number(&mut self) -> Result<f32, DslError> {
        match self.next_tok()? {
            Tok::Number(n) => Ok(n),
            other => Err(self.err(format!("expected number, got {other:?}"))),
        }
    }

    fn expect_color(&mut self) -> Result<Color, DslError> {
        match self.next_tok()? {
            Tok::Color(c) => Ok(c),
            other => Err(self.err(format!("expected #RRGGBB color, got {other:?}"))),
        }
    }

    fn expect_string(&mut self) -> Result<String, DslError> {
        match self.next_tok()? {
            Tok::String(s) => Ok(s),
            other => Err(self.err(format!("expected string, got {other:?}"))),
        }
    }

    fn parse_child(&mut self) -> Result<AstChild, DslError> {
        match self.peek_tok()? {
            Tok::Ident(ref s) if s == "Text" => self.parse_text(),
            Tok::Ident(ref s) if s == "Button" => self.parse_button(),
            Tok::Ident(ref s) if s == "Column" => Ok(AstChild::Column(self.parse_column()?)),
            other => Err(self.err(format!("expected Text, Button, or Column, got {other:?}"))),
        }
    }

    fn parse_text(&mut self) -> Result<AstChild, DslError> {
        self.expect_ident("Text")?;
        match self.next_tok()? {
            Tok::LParen => {}
            _ => return Err(self.err("expected `(` after Text")),
        }
        let content = self.expect_string()?;
        match self.next_tok()? {
            Tok::RParen => {}
            _ => return Err(self.err("expected `)` after Text string")),
        }
        let name = self.parse_name_opt()?;
        Ok(AstChild::Text { name, content })
    }

    fn parse_button(&mut self) -> Result<AstChild, DslError> {
        self.expect_ident("Button")?;
        match self.next_tok()? {
            Tok::LParen => {}
            _ => return Err(self.err("expected `(` after Button")),
        }
        let label = self.expect_string()?;
        match self.next_tok()? {
            Tok::RParen => {}
            _ => return Err(self.err("expected `)` after Button string")),
        }
        let name = self.parse_name_opt()?;
        let mut bg = None;
        let mut fg = None;
        if matches!(self.peek_tok()?, Tok::LBrace) {
            self.next_tok()?;
            loop {
                match self.peek_tok()? {
                    Tok::RBrace => {
                        self.next_tok()?;
                        break;
                    }
                    Tok::Ident(_) => {
                        let key = match self.next_tok()? {
                            Tok::Ident(s) => s,
                            _ => unreachable!(),
                        };
                        match self.next_tok()? {
                            Tok::Colon => {}
                            _ => {
                                return Err(self.err(format!("expected `:` after `{key}`")));
                            }
                        }
                        match key.as_str() {
                            "bg" => bg = Some(self.expect_color()?),
                            "fg" => fg = Some(self.expect_color()?),
                            other => {
                                return Err(self.err(format!("unknown Button property `{other}`")));
                            }
                        }
                    }
                    other => {
                        return Err(self.err(format!("unexpected token in Button props: {other:?}")));
                    }
                }
            }
        }
        Ok(AstChild::Button {
            name,
            label,
            bg,
            fg,
        })
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_counter_ngui() {
        let src = r#"
// counter demo
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
}
