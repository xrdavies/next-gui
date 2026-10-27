//! Visual SQL tokens; execution and validation remain SQLite's responsibility.

use std::ops::Range;

use next_gui_scene::Color;
use next_gui_ui::{Theme, ThemeMode, Ui, WidgetId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Keyword,
    Literal,
    Identifier,
    Parameter,
    Function,
    Comment,
}

fn identifier(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' || ch as u32 >= 0x80
}

fn quoted(chars: &[char], mut i: usize, quote: char, doubled: bool) -> usize {
    while i < chars.len() {
        if chars[i] == quote {
            i += 1;
            if doubled && chars.get(i) == Some(&quote) {
                i += 1;
            } else {
                return i;
            }
        } else {
            i += 1;
        }
    }
    i
}

fn digits(chars: &[char], mut i: usize, hex: bool) -> usize {
    let digit = |ch: char| {
        if hex {
            ch.is_ascii_hexdigit()
        } else {
            ch.is_ascii_digit()
        }
    };
    while chars.get(i).is_some_and(|ch| digit(*ch)) {
        i += 1;
        if chars.get(i) == Some(&'_') && chars.get(i + 1).is_some_and(|ch| digit(*ch)) {
            i += 1;
        }
    }
    i
}

fn tokens(sql: &str) -> Vec<(Range<usize>, Kind)> {
    let chars: Vec<_> = sql.chars().collect();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        let ch = chars[i];
        let next = chars.get(i + 1).copied();
        let kind = if ch == '-' && next == Some('-') {
            i += 2;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            if chars.get(i) == Some(&'\n') {
                i += 1;
            }
            Some(Kind::Comment)
        } else if ch == '/' && next == Some('*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            Some(Kind::Comment)
        } else if matches!(ch, 'x' | 'X') && next == Some('\'') {
            i = quoted(&chars, i + 2, '\'', true);
            Some(Kind::Literal)
        } else if matches!(ch, '\'' | '"' | '\x60' | '[') {
            i = quoted(&chars, i + 1, if ch == '[' { ']' } else { ch }, ch != '[');
            Some(if ch == '\'' {
                Kind::Literal
            } else {
                Kind::Identifier
            })
        } else if ch.is_ascii_digit() || (ch == '.' && next.is_some_and(|ch| ch.is_ascii_digit())) {
            if ch == '0'
                && matches!(next, Some('x' | 'X'))
                && chars.get(i + 2).is_some_and(char::is_ascii_hexdigit)
            {
                i = digits(&chars, i + 2, true);
            } else {
                i = digits(&chars, i, false);
                if chars.get(i) == Some(&'.') {
                    i = digits(&chars, i + 1, false);
                }
                if matches!(chars.get(i), Some('e' | 'E')) {
                    let mut end = i + 1;
                    if matches!(chars.get(end), Some('+' | '-')) {
                        end += 1;
                    }
                    if chars.get(end).is_some_and(char::is_ascii_digit) {
                        i = digits(&chars, end, false);
                    }
                }
            }
            Some(Kind::Literal)
        } else if ch == '?' {
            i += 1;
            while chars.get(i).is_some_and(char::is_ascii_digit) {
                i += 1;
            }
            Some(Kind::Parameter)
        } else if matches!(ch, ':' | '@' | '$') && next.is_some_and(identifier) {
            i += 1;
            while i < chars.len() {
                if identifier(chars[i]) {
                    i += 1;
                } else if ch == '$' && chars[i] == ':' && chars.get(i + 1) == Some(&':') {
                    i += 2;
                } else {
                    break;
                }
            }
            if ch == '$' && chars.get(i) == Some(&'(') {
                i += 1;
                while i < chars.len() && chars[i] != ')' && !chars[i].is_whitespace() {
                    i += 1;
                }
                if chars.get(i) == Some(&')') {
                    i += 1;
                }
            }
            Some(Kind::Parameter)
        } else if identifier(ch) {
            i += 1;
            while chars.get(i).is_some_and(|ch| identifier(*ch)) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let keyword = word.is_ascii()
                && i32::try_from(word.len()).is_ok_and(|len| {
                    // SQLite reads exactly len bytes from this live UTF-8 slice.
                    unsafe { rusqlite::ffi::sqlite3_keyword_check(word.as_ptr().cast(), len) != 0 }
                });
            if keyword {
                Some(Kind::Keyword)
            } else if chars[i..].iter().find(|ch| !ch.is_whitespace()) == Some(&'(') {
                Some(Kind::Function)
            } else {
                None
            }
        } else {
            i += 1;
            None
        };
        if let Some(kind) = kind {
            spans.push((start..i, kind));
        }
    }
    spans
}

// shortcut: retokenizes SQL synchronously; use an incremental worker for large scripts.
pub fn apply(ui: &mut Ui, id: WidgetId, theme: &Theme) {
    let Some(sql) = ui.text_area_value(id) else {
        return;
    };
    let dark = theme.mode == ThemeMode::Dark;
    let spans = tokens(sql)
        .into_iter()
        .map(|(range, kind)| {
            let color = match (dark, kind) {
                (true, Kind::Keyword) => Color::rgb(173, 155, 255),
                (false, Kind::Keyword) => Color::rgb(104, 48, 156),
                (true, Kind::Literal) => Color::rgb(156, 211, 161),
                (false, Kind::Literal) => Color::rgb(35, 112, 60),
                (true, Kind::Identifier | Kind::Function) => Color::rgb(113, 197, 221),
                (false, Kind::Identifier | Kind::Function) => Color::rgb(0, 102, 127),
                (true, Kind::Parameter) => Color::rgb(239, 190, 123),
                (false, Kind::Parameter) => Color::rgb(143, 82, 14),
                (_, Kind::Comment) => theme.colors.text_secondary,
            };
            (range, color)
        })
        .collect();
    ui.set_text_area_foregrounds(id, spans);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn styled(sql: &str) -> Vec<(String, Kind)> {
        let mut previous = 0;
        tokens(sql)
            .into_iter()
            .map(|(range, kind)| {
                assert!(range.start >= previous && range.end <= sql.chars().count());
                previous = range.end;
                (
                    sql.chars().skip(range.start).take(range.len()).collect(),
                    kind,
                )
            })
            .collect()
    }

    #[test]
    fn sqlite_tokens_distinguish_literals_identifiers_parameters_and_comments() {
        let sql = "select \"select\", [from], `where`, 'it''s SELECT', X'ABCD', 0x1_f, 1_234.5e-2, .5, :中文, ?12, @arg, $ns::var(sfx), lower(abc) -- SELECT\r\n/* 多行\n SELECT */ RETURNING";
        let expected = [
            ("select", Kind::Keyword),
            ("\"select\"", Kind::Identifier),
            ("[from]", Kind::Identifier),
            ("`where`", Kind::Identifier),
            ("'it''s SELECT'", Kind::Literal),
            ("X'ABCD'", Kind::Literal),
            ("0x1_f", Kind::Literal),
            ("1_234.5e-2", Kind::Literal),
            (".5", Kind::Literal),
            (":中文", Kind::Parameter),
            ("?12", Kind::Parameter),
            ("@arg", Kind::Parameter),
            ("$ns::var(sfx)", Kind::Parameter),
            ("lower", Kind::Function),
            ("-- SELECT\r\n", Kind::Comment),
            ("/* 多行\n SELECT */", Kind::Comment),
            ("RETURNING", Kind::Keyword),
        ];
        assert_eq!(
            styled(sql),
            expected
                .into_iter()
                .map(|(s, k)| (s.into(), k))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn unfinished_tokens_and_escaped_quotes_do_not_leak_keywords() {
        for (sql, kind) in [
            ("'中文 SELECT", Kind::Literal),
            ("/* SELECT", Kind::Comment),
            ("\"a\"\"SELECT", Kind::Identifier),
            ("`a``SELECT", Kind::Identifier),
            ("[SELECT", Kind::Identifier),
        ] {
            assert_eq!(styled(sql), [(sql.into(), kind)]);
        }
        assert_eq!(
            styled("表名.SELECT 1e+ bad 0xZ"),
            [
                ("SELECT".into(), Kind::Keyword),
                ("1".into(), Kind::Literal),
                ("0".into(), Kind::Literal)
            ]
        );
        assert_eq!(
            styled("-- a\rSELECT"),
            [("-- a\rSELECT".into(), Kind::Comment)]
        );
        assert!(styled("").is_empty());
    }
}
