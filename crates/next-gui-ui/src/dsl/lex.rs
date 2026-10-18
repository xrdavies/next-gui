//! Lexer for `.ngui` (strings, numbers, colors, lists, bools).

use next_gui_scene::Color;

use super::DslError;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    String(String),
    Number(f32),
    Color(Color),
    Bool(bool),
    At,
    Colon,
    Comma,
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Eof,
}

pub struct Lexer<'a> {
    src: &'a str,
    i: usize,
    pub line: usize,
    pub col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            i: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn err(&self, message: impl Into<String>) -> DslError {
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

    pub fn next_tok(&mut self) -> Result<Tok, DslError> {
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
            '[' => {
                self.bump_char();
                Ok(Tok::LBracket)
            }
            ']' => {
                self.bump_char();
                Ok(Tok::RBracket)
            }
            ':' => {
                self.bump_char();
                Ok(Tok::Colon)
            }
            ',' => {
                self.bump_char();
                Ok(Tok::Comma)
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

    pub fn peek_tok(&mut self) -> Result<Tok, DslError> {
        let save = (self.i, self.line, self.col);
        let t = self.next_tok()?;
        self.i = save.0;
        self.line = save.1;
        self.col = save.2;
        Ok(t)
    }

    fn lex_string(&mut self, line: usize, column: usize) -> Result<Tok, DslError> {
        self.bump_char();
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
        self.bump_char();
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
        let n: f32 = s
            .parse()
            .map_err(|_| self.err(format!("invalid number {s}")))?;
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
        match s.as_str() {
            "true" => Ok(Tok::Bool(true)),
            "false" => Ok(Tok::Bool(false)),
            _ => Ok(Tok::Ident(s)),
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}
