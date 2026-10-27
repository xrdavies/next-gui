//! Extract editable fragments from SQLite's stored CREATE INDEX, not arbitrary SQL validation.

use crate::sql_syntax::{self, Kind};
use std::ops::Range;

/// Character ranges keep row actions aligned with the editable SQL and its undo history.
pub fn key_ranges(sql: &str) -> Result<Vec<Range<usize>>, String> {
    if sql.trim().is_empty() {
        return Ok(Vec::new());
    }
    let chars: Vec<_> = sql.chars().collect();
    let mut code = chars.clone();
    for (range, _) in sql_syntax::tokens(sql) {
        code[range].fill(' ');
    }
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    for (i, ch) in code.iter().enumerate() {
        match *ch {
            '(' => depth += 1,
            ')' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or("Unmatched closing parenthesis in index keys")?
            }
            ';' => return Err("Index keys cannot contain a statement separator".into()),
            ',' if depth == 0 => {
                ranges.push(start..i);
                start = i + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err("Complete the index expression before changing key rows".into());
    }
    ranges.push(start..chars.len());
    for range in &mut ranges {
        while range.start < range.end && chars[range.start].is_whitespace() {
            range.start += 1;
        }
        while range.end > range.start && chars[range.end - 1].is_whitespace() {
            range.end -= 1;
        }
        if range.start == range.end {
            return Err("Index key cannot be empty".into());
        }
    }
    Ok(ranges)
}

pub fn keys(sql: &str) -> Result<Vec<String>, String> {
    let chars: Vec<_> = sql.chars().collect();
    Ok(key_ranges(sql)?
        .into_iter()
        .map(|range| chars[range].iter().collect())
        .collect())
}

pub fn definition(sql: &str) -> Result<(String, String), String> {
    let chars: Vec<_> = sql.chars().collect();
    let mut punctuation = chars.clone();
    for (range, _) in sql_syntax::tokens(sql) {
        punctuation[range].fill(' ');
    }
    let open = punctuation
        .iter()
        .position(|ch| *ch == '(')
        .ok_or("Index key list missing")?;
    let mut depth = 0;
    let close = punctuation
        .iter()
        .enumerate()
        .skip(open)
        .find_map(|(index, ch)| {
            if *ch == '(' {
                depth += 1;
            }
            if *ch == ')' {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            None
        })
        .ok_or("Index key list is incomplete")?;
    let end = punctuation
        .iter()
        .enumerate()
        .skip(close + 1)
        .find_map(|(i, ch)| (*ch == ';').then_some(i))
        .unwrap_or(chars.len());
    let keys: String = chars[open + 1..close].iter().collect();
    let tail: String = chars[close + 1..end].iter().collect();
    let tail_chars: Vec<_> = tail.chars().collect();
    let mut tail_code = tail_chars.clone();
    for (range, kind) in sql_syntax::tokens(&tail) {
        if kind == Kind::Comment {
            tail_code[range].fill(' ');
        }
    }
    let predicate = if let Some(start) = tail_code.iter().position(|ch| !ch.is_whitespace()) {
        let end = start
            + tail_code[start..]
                .iter()
                .take_while(|ch| sql_syntax::identifier(**ch))
                .count();
        let word: String = tail_chars[start..end].iter().collect();
        if !word.eq_ignore_ascii_case("WHERE") {
            return Err("Unexpected text after index keys".into());
        }
        tail_chars[end..]
            .iter()
            .collect::<String>()
            .trim()
            .to_string()
    } else {
        String::new()
    };
    Ok((keys.trim().into(), predicate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_rows_preserve_nested_quoted_and_commented_sql() {
        let sql = "coalesce(\"列,名\", ',;)') DESC,\n`a``b` -- , )\n, [other,column] /* , ( */";
        let expected = [
            "coalesce(\"列,名\", ',;)') DESC",
            "`a``b` -- , )",
            "[other,column] /* , ( */",
        ];
        assert_eq!(keys(sql).unwrap(), expected);
        assert_eq!(keys(&expected.join("\n,\n")).unwrap(), expected);
        assert_eq!(key_ranges("  中文 , x ").unwrap(), [2..4, 7..8]);
        assert!(keys(" \n").unwrap().is_empty());
        for sql in ["x,", ",x", "x,,y", "f(x", "x)", "x; DELETE FROM t"] {
            assert!(keys(sql).is_err(), "{sql}");
        }
    }

    #[test]
    fn stored_index_fragments_keep_nested_expressions_quotes_and_comments() {
        for (sql, keys, predicate) in [
            ("CREATE INDEX ix ON t(v)", "v", ""),
            ("CREATE UNIQUE INDEX \"i(\" ON [t)] /* ( */ (coalesce(\"列,名\", ') WHERE;') COLLATE NOCASE DESC, length(v)) /* WHERE ) */ WHERE v != ';)' AND length(v)>0; -- tail", "coalesce(\"列,名\", ') WHERE;') COLLATE NOCASE DESC, length(v)", "v != ';)' AND length(v)>0"),
            ("CREATE INDEX `i``(` ON t(v -- ) WHERE\n) -- (\nWHERE v IS NOT NULL", "v -- ) WHERE", "v IS NOT NULL"),
        ] {
            assert_eq!(definition(sql).unwrap(), (keys.into(), predicate.into()));
        }
        for sql in [
            "",
            "CREATE INDEX ix ON t",
            "CREATE INDEX ix ON t((v)",
            "CREATE INDEX ix ON t(v) unexpected",
        ] {
            assert!(definition(sql).is_err(), "{sql}");
        }
    }
}
