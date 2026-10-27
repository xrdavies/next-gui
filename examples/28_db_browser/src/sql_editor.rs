//! SQL line edits use Unicode scalar offsets, matching the shared TextArea.

use std::ops::Range;

pub fn toggle_line_comment(sql: &str, selection: Range<usize>) -> (Range<usize>, String) {
    // shortcut: CR-only lines follow TextArea's LF layout, extend both together.
    let byte_offset = |index| sql.char_indices().nth(index).map_or(sql.len(), |(i, _)| i);
    let from = byte_offset(selection.start.min(selection.end));
    let to = byte_offset(selection.start.max(selection.end));
    let start = sql[..from].rfind('\n').map_or(0, |i| i + 1);
    // A selection ending at the next line's start excludes that line.
    let end = if from != to && sql[..to].ends_with('\n') {
        to
    } else {
        sql[to..].find('\n').map_or(sql.len(), |i| to + i + 1)
    };
    let block = &sql[start..end];
    let uncomment = block.trim_start_matches([' ', '\t']).starts_with("--");
    let mut text = String::new();
    for line in block.split_inclusive('\n') {
        if uncomment {
            let trimmed = line.trim_start_matches([' ', '\t']);
            if let Some(rest) = trimmed.strip_prefix("--") {
                text.push_str(&line[..line.len() - trimmed.len()]);
                text.push_str(rest.strip_prefix(' ').unwrap_or(rest));
            } else {
                text.push_str(line);
            }
        } else {
            text.push_str("-- ");
            text.push_str(line);
        }
    }
    if block.is_empty() {
        text.push_str("-- ");
    }
    (
        sql[..start].chars().count()..sql[..end].chars().count(),
        text,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toggle(sql: &str, selection: Range<usize>) -> String {
        let (range, text) = toggle_line_comment(sql, selection);
        let start = sql
            .char_indices()
            .nth(range.start)
            .map_or(sql.len(), |(i, _)| i);
        let end = sql
            .char_indices()
            .nth(range.end)
            .map_or(sql.len(), |(i, _)| i);
        let mut result = sql.to_owned();
        result.replace_range(start..end, &text);
        result
    }

    #[test]
    fn current_line_and_selected_lines_keep_unicode_and_line_endings() {
        for eol in ["\n", "\r\n"] {
            let first = format!("SELECT '中文🙂';{eol}");
            let second = format!("  SELECT 2;{eol}");
            let sql = format!("{first}{second}SELECT 3;");
            let start = first.chars().count();
            let end = start + second.chars().count();
            let expected = format!("{first}-- {second}SELECT 3;");
            assert_eq!(toggle(&sql, start + 2..start + 2), expected);
            assert_eq!(toggle(&sql, start + 2..end), expected);
            assert_eq!(toggle(&sql, start..end - eol.chars().count()), expected);
            assert_eq!(
                toggle(&sql, end..sql.chars().count()),
                format!("{first}{second}-- SELECT 3;")
            );
        }
    }

    #[test]
    fn first_selected_line_decides_mode_and_only_two_hyphens_are_removed() {
        let sql = "\t--  中文\r\n---1\nSELECT 2;\n  --\tSELECT 3;";
        assert_eq!(
            toggle(sql, 1..sql.chars().count()),
            "\t 中文\r\n-1\nSELECT 2;\n  \tSELECT 3;"
        );
        let sql = "SELECT 1;\n-- SELECT 2;\n\n\t";
        assert_eq!(
            toggle(sql, 0..sql.chars().count()),
            "-- SELECT 1;\n-- -- SELECT 2;\n-- \n-- \t"
        );
        assert_eq!(toggle("\u{a0}-- keep", 0..0), "-- \u{a0}-- keep");
    }

    #[test]
    fn empty_and_trailing_lines_and_document_bounds_are_supported() {
        for (sql, position, expected) in [
            ("", 0, "-- "),
            ("SELECT 1;\n", 10, "SELECT 1;\n-- "),
            ("\n", 0, "-- \n"),
            ("--", 2, ""),
            ("---", 3, "-"),
        ] {
            assert_eq!(toggle(sql, position..position), expected);
        }
        assert_eq!(toggle("SELECT 1;", usize::MAX..usize::MAX), "-- SELECT 1;");
    }
}
