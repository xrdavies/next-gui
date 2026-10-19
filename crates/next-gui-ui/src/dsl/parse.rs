//! Parser: tokens → [`Document`].

use super::ast::{AstValue, AstWidget, Document};
use super::lex::{Lexer, Tok};
use super::DslError;

pub fn parse_document(src: &str) -> Result<Document, DslError> {
    let mut p = Parser::new(src);
    let mut theme = None;
    // Optional document-level `theme: light|dark` before the root Column.
    if matches!(p.lex.peek_tok()?, Tok::Ident(ref s) if s == "theme") {
        p.lex.next_tok()?; // theme
        match p.lex.next_tok()? {
            Tok::Colon => {}
            _ => return Err(p.lex.err("expected `:` after `theme`")),
        }
        let mode = match p.lex.next_tok()? {
            Tok::Ident(s) | Tok::String(s) => parse_theme_mode_str(&s).map_err(|m| p.lex.err(m))?,
            other => {
                return Err(p.lex.err(format!(
                    "theme: expected light|dark, got {other:?}"
                )));
            }
        };
        theme = Some(mode);
    }
    let root = p.parse_widget()?;
    if root.kind != "Column" {
        return Err(DslError {
            message: format!("document root must be Column, got {}", root.kind),
            line: root.line,
            column: root.column,
        });
    }
    let mut overlays = Vec::new();
    loop {
        match p.lex.peek_tok()? {
            Tok::Eof => break,
            Tok::Ident(ref s) if s == "Modal" || s == "Drawer" || s == "Notify" => {
                overlays.push(p.parse_widget()?);
            }
            other => {
                return Err(p.lex.err(format!(
                    "expected Modal/Drawer/Notify overlay or EOF, got {other:?}"
                )));
            }
        }
    }
    Ok(Document {
        theme,
        root,
        overlays,
    })
}

fn parse_theme_mode_str(s: &str) -> Result<crate::ThemeMode, String> {
    match s {
        "light" => Ok(crate::ThemeMode::Light),
        "dark" => Ok(crate::ThemeMode::Dark),
        other => Err(format!(
            "unknown theme `{other}` (expected light|dark)"
        )),
    }
}

struct Parser<'a> {
    lex: Lexer<'a>,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            lex: Lexer::new(src),
        }
    }

    fn parse_name_opt(&mut self) -> Result<Option<String>, DslError> {
        if matches!(self.lex.peek_tok()?, Tok::At) {
            self.lex.next_tok()?;
            match self.lex.next_tok()? {
                Tok::Ident(s) => Ok(Some(s)),
                _ => Err(self.lex.err("expected identifier after `@`")),
            }
        } else {
            Ok(None)
        }
    }

    fn parse_widget(&mut self) -> Result<AstWidget, DslError> {
        let (line, column) = (self.lex.line, self.lex.col);
        let kind = match self.lex.next_tok()? {
            Tok::Ident(s) => s,
            other => {
                return Err(self.lex.err(format!("expected widget name, got {other:?}")));
            }
        };
        // Optional args: Widget(...) or Widget
        let mut args = Vec::new();
        if matches!(self.lex.peek_tok()?, Tok::LParen) {
            self.lex.next_tok()?;
            args = self.parse_arg_list(Tok::RParen)?;
        }
        let name = self.parse_name_opt()?;
        let mut props = Vec::new();
        let mut children = Vec::new();
        if matches!(self.lex.peek_tok()?, Tok::LBrace) {
            self.lex.next_tok()?;
            loop {
                match self.lex.peek_tok()? {
                    Tok::RBrace => {
                        self.lex.next_tok()?;
                        break;
                    }
                    Tok::Eof => {
                        return Err(self.lex.err(format!("unclosed `{{` in {kind}")));
                    }
                    Tok::Ident(ref id) if is_widget_name(id) => {
                        children.push(self.parse_widget()?);
                    }
                    Tok::Ident(_) => {
                        // property: key: value  (value may be list/object/widget-free)
                        let key = match self.lex.next_tok()? {
                            Tok::Ident(s) => s,
                            _ => unreachable!(),
                        };
                        match self.lex.next_tok()? {
                            Tok::Colon => {}
                            _ => {
                                return Err(self
                                    .lex
                                    .err(format!("expected `:` after property `{key}`")));
                            }
                        }
                        let val = self.parse_value()?;
                        props.push((key, val));
                    }
                    other => {
                        return Err(self
                            .lex
                            .err(format!("unexpected token in {kind}: {other:?}")));
                    }
                }
            }
        }
        Ok(AstWidget {
            kind,
            name,
            args,
            props,
            children,
            line,
            column,
        })
    }

    fn parse_arg_list(&mut self, end: Tok) -> Result<Vec<AstValue>, DslError> {
        let mut args = Vec::new();
        loop {
            match self.lex.peek_tok()? {
                ref t if token_eq(t, &end) => {
                    self.lex.next_tok()?;
                    break;
                }
                Tok::Eof => return Err(self.lex.err("unterminated argument list")),
                _ => {
                    args.push(self.parse_value()?);
                    match self.lex.peek_tok()? {
                        Tok::Comma => {
                            self.lex.next_tok()?;
                        }
                        ref t if token_eq(t, &end) => {}
                        other => {
                            return Err(self.lex.err(format!(
                                "expected `,` or closer in arg list, got {other:?}"
                            )));
                        }
                    }
                }
            }
        }
        Ok(args)
    }

    fn parse_value(&mut self) -> Result<AstValue, DslError> {
        match self.lex.peek_tok()? {
            Tok::String(_) => match self.lex.next_tok()? {
                Tok::String(s) => Ok(AstValue::String(s)),
                _ => unreachable!(),
            },
            Tok::Number(_) => match self.lex.next_tok()? {
                Tok::Number(n) => Ok(AstValue::Number(n)),
                _ => unreachable!(),
            },
            Tok::Bool(_) => match self.lex.next_tok()? {
                Tok::Bool(b) => Ok(AstValue::Bool(b)),
                _ => unreachable!(),
            },
            Tok::Color(_) => match self.lex.next_tok()? {
                Tok::Color(c) => Ok(AstValue::Color(c)),
                _ => unreachable!(),
            },
            Tok::LBracket => {
                self.lex.next_tok()?;
                let items = self.parse_arg_list(Tok::RBracket)?;
                Ok(AstValue::List(items))
            }
            Tok::LBrace => {
                self.lex.next_tok()?;
                let mut fields = Vec::new();
                loop {
                    match self.lex.peek_tok()? {
                        Tok::RBrace => {
                            self.lex.next_tok()?;
                            break;
                        }
                        Tok::Ident(_) => {
                            let key = match self.lex.next_tok()? {
                                Tok::Ident(s) => s,
                                _ => unreachable!(),
                            };
                            match self.lex.next_tok()? {
                                Tok::Colon => {}
                                _ => {
                                    return Err(self.lex.err(format!(
                                        "expected `:` after object key `{key}`"
                                    )));
                                }
                            }
                            let val = self.parse_value()?;
                            fields.push((key, val));
                            if matches!(self.lex.peek_tok()?, Tok::Comma) {
                                self.lex.next_tok()?;
                            }
                        }
                        other => {
                            return Err(self
                                .lex
                                .err(format!("unexpected token in object: {other:?}")));
                        }
                    }
                }
                Ok(AstValue::Object(fields))
            }
            Tok::Ident(_) => match self.lex.next_tok()? {
                Tok::Ident(s) => Ok(AstValue::Ident(s)),
                _ => unreachable!(),
            },
            other => Err(self.lex.err(format!("expected value, got {other:?}"))),
        }
    }
}

fn token_eq(a: &Tok, b: &Tok) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// PascalCase (or known) widget names vs lowercase style props.
fn is_widget_name(id: &str) -> bool {
    let first = id.chars().next().unwrap_or('\0');
    if first.is_ascii_uppercase() {
        return true;
    }
    // Allow a few aliases if ever needed — primary rule is PascalCase.
    false
}
