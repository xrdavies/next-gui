//! Lexical function/argument context; never prepares or executes editor SQL.

use crate::sql_syntax::{self, Kind};

#[derive(Debug, PartialEq, Eq)]
pub struct Context {
    pub name: String,
    pub anchor: usize,
    pub argument: usize,
}

// shortcut: synchronous lexical scans; use an incremental worker for large editor documents.
pub fn context(sql: &str, caret: usize, known: impl Fn(&str) -> bool) -> Option<Context> {
    let chars: Vec<_> = sql.chars().collect();
    if caret > chars.len() {
        return None;
    }
    let prefix: String = chars[..caret].iter().collect();
    let tokens = sql_syntax::tokens(&prefix);
    let mut tokens = tokens.iter().peekable();
    let mut stack: Vec<(Option<String>, usize, usize)> = Vec::new();
    let mut name = None;
    let mut i = 0;
    while i < caret {
        while tokens.peek().is_some_and(|(range, _)| range.start < i) {
            tokens.next();
        }
        if let Some((range, kind)) = tokens.peek().filter(|(range, _)| range.start == i) {
            match kind {
                Kind::Comment => {
                    i = range.end;
                    continue;
                }
                Kind::Literal | Kind::Parameter => {
                    name = None;
                    i = range.end;
                    continue;
                }
                Kind::Identifier => {
                    let quote = chars[i];
                    let close = if quote == '[' { ']' } else { quote };
                    name = (range.end > i + 1 && chars[range.end - 1] == close).then(|| {
                        chars[i + 1..range.end - 1]
                            .iter()
                            .collect::<String>()
                            .replace(&format!("{close}{close}"), &close.to_string())
                            .to_ascii_lowercase()
                    });
                    i = range.end;
                    continue;
                }
                Kind::Keyword | Kind::Function => {}
            }
        }
        let ch = chars[i];
        if sql_syntax::identifier(ch) {
            let start = i;
            i += 1;
            while i < caret && sql_syntax::identifier(chars[i]) {
                i += 1;
            }
            name = Some(
                chars[start..i]
                    .iter()
                    .collect::<String>()
                    .to_ascii_lowercase(),
            );
            continue;
        }
        match ch {
            '(' => {
                stack.push((name.take().filter(|s| known(s)), i, 0));
            }
            ')' => {
                stack.pop();
                name = None;
            }
            ',' => {
                if let Some((_, _, argument)) = stack.last_mut() {
                    *argument += 1;
                }
                name = None;
            }
            ';' => {
                stack.clear();
                name = None;
            }
            _ if ch.is_whitespace() => {}
            _ => name = None,
        }
        i += 1;
    }
    stack
        .into_iter()
        .rev()
        .find_map(|(name, anchor, argument)| {
            name.map(|name| Context {
                name,
                anchor,
                argument,
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_end(sql: &str) -> Option<Context> {
        context(sql, sql.chars().count(), |name| {
            ["coalesce", "substr", "lower"].contains(&name)
        })
    }

    #[test]
    fn nested_calls_groups_quotes_and_comments_count_only_current_arguments() {
        for (sql, name, argument) in [
            ("SELECT coalesce(substr('中,文)', 1,", "substr", 2),
            ("SELECT coalesce(substr('中,文)', 1, 2),", "coalesce", 1),
            ("SELECT coalesce((1, 2),", "coalesce", 1),
            ("SELECT coalesce(unknown(1, 2),", "coalesce", 1),
            ("SELECT coalesce(/* (, */ 'it''s,)', -- ,)\n", "coalesce", 1),
            ("SELECT LOWER /* comment */ (", "lower", 0),
            ("SELECT [substr](`a,b`, \"c)d\",", "substr", 2),
            ("SELECT coalesce($name::x(a,b),", "coalesce", 1),
            ("SELECT coalesce(X'ABCD',", "coalesce", 1),
        ] {
            let result = at_end(sql).unwrap_or_else(|| panic!("{sql}"));
            assert_eq!((&*result.name, result.argument), (name, argument), "{sql}");
            assert_eq!(sql.chars().nth(result.anchor), Some('('));
        }
        for sql in [
            "'lower('",
            "-- lower(",
            "/* lower( */",
            "unknown(",
            "lower('x')",
            "lower(; SELECT 1",
        ] {
            assert_eq!(at_end(sql), None, "{sql}");
        }
        let sql = "SELECT '中文', substr('x', 1, 2)";
        let caret = sql.chars().position(|c| c == '1').unwrap();
        let result = context(sql, caret, |name| name == "substr").unwrap();
        assert_eq!(result.argument, 1);
        assert_eq!(sql.chars().nth(result.anchor), Some('('));
        assert!(context(sql, usize::MAX, |_| true).is_none());
    }
}
