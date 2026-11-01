//! SQL line edits use Unicode scalar offsets, matching the shared TextArea.

use std::ops::Range;

#[derive(Clone)]
pub struct ErrorTarget {
    pub document: String,
    pub selection: Range<usize>,
}

impl ErrorTarget {
    pub fn new(document: String, executed: Range<usize>, error: Range<usize>) -> Option<Self> {
        if executed.start > executed.end || executed.end > document.chars().count() { return None; }
        let script: String = document.chars().skip(executed.start).take(executed.len()).collect();
        if error.start > error.end || error.end > script.len() || !script.is_char_boundary(error.start) || !script.is_char_boundary(error.end) { return None; }
        let start = executed.start + script[..error.start].chars().count();
        let end = executed.start + script[..error.end].chars().count();
        Some(Self { document, selection: start..end })
    }
}

impl crate::Demo {
    pub(crate) fn goto_sql_error(&mut self) {
        let area = self.ids.sql_area;
        let Some(target) = &self.state.sql_error else { return; };
        if self.ui.text_area_value(area) != Some(target.document.as_str()) {
            self.set_error("SQL text has changed since execution; run it again to locate the error");
        } else {
            self.ui.select_text_area_range(area, target.selection.clone());
            self.ui.set_focus(Some(area));
            self.snapshot_inputs();
            self.clear_error();
        }
        self.rebuild = true;
    }
}

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

    #[test]
    fn desktop_sql_error_navigation_tracks_selected_sources_and_rejects_stale_text() {
        use crate::{browse_filter::tests::{capture,fixture}, state::MainTab, UiKey};
        use next_gui_ui::Theme;
        for (name,width,height,theme) in [("dark",1600,900,Theme::dark()),("light",1920,927,Theme::light())] {
            let mut demo = fixture(width,height,theme); demo.state.write_changes().unwrap();
            let prefix = "-- 中文🙂\nSELECT 'same';\n";
            let selected = "SELECT 'same';\nSELECT 你好 FROM names;";
            let document = format!("{prefix}{selected}\nSELECT 99;");
            demo.state.tab = MainTab::ExecuteSql; demo.state.sql_tabs[0] = document.clone(); demo.do_rebuild();
            let start = prefix.chars().count(); let end = start+selected.chars().count();
            demo.ui.select_text_area_range(demo.ids.sql_area,start..end); demo.execute_sql_selection();
            let deadline = std::time::Instant::now()+std::time::Duration::from_secs(10);
            while demo.state.sql_busy { assert!(std::time::Instant::now()<deadline); demo.state.poll_sql_job(); std::thread::sleep(std::time::Duration::from_millis(1)); }
            assert!(demo.state.error_msg.is_some()); demo.do_rebuild();
            let target = demo.state.sql_error.as_ref().unwrap();
            let expected = document[..document.find("你好").unwrap()].chars().count();
            assert_eq!(target.selection,expected..expected+2);
            assert_eq!(demo.ui.button_enabled(demo.ids.sql_messages.error),Some(true));
            demo.handle_button(demo.ids.sql_messages.error); demo.do_rebuild();
            assert_eq!(demo.ui.text_area_selected_text(demo.ids.sql_area).as_deref(),Some("你好"));
            assert_eq!(demo.ui.focused(),Some(demo.ids.sql_area));
            capture(&mut demo,&format!("desktop-sql-error-source-{name}"));
            demo.snapshot_inputs(); demo.state.sql_tabs.push("SELECT 1;".into()); demo.state.switch_sql_tab(1); demo.do_rebuild();
            assert!(demo.state.sql_error.is_none()); demo.state.switch_sql_tab(0); demo.do_rebuild();
            assert_eq!(demo.state.sql_error.as_ref().unwrap().selection,expected..expected+2);
            demo.ui.set_focus(Some(demo.ids.sql_area)); demo.ui.key_input(UiKey::End); demo.ui.paste_into_focused_text(" ");
            let selection = demo.ui.text_area_selection(demo.ids.sql_area);
            demo.goto_sql_error();
            assert!(demo.state.error_msg.as_ref().unwrap().contains("changed since execution"));
            assert_eq!(demo.ui.text_area_selection(demo.ids.sql_area),selection);
            demo.state.sql_tabs[0] = document.clone(); demo.state.execute_current_line(expected).unwrap();
            while demo.state.sql_busy { assert!(std::time::Instant::now()<deadline); demo.state.poll_sql_job(); std::thread::sleep(std::time::Duration::from_millis(1)); }
            assert_eq!(demo.state.sql_error.as_ref().unwrap().selection,expected..expected+2);
        }
    }

    #[test]
    fn sql_execution_reports_runtime_statement_without_replaying_prior_writes() {
        let conn = crate::db::open_path(":memory:").unwrap();
        conn.execute_batch("CREATE TABLE t(v UNIQUE);BEGIN;").unwrap();
        let script = "INSERT INTO t VALUES('中文;');\nINSERT INTO t VALUES('中文;'); SELECT 99;";
        let report = crate::db::execute_sql_report(&conn,script);
        assert!(report.result.is_err());
        assert_eq!(script[report.error_range.unwrap()].trim(),"INSERT INTO t VALUES('中文;');");
        assert_eq!(conn.query_row("SELECT count(*) FROM t",[],|r|r.get::<_,usize>(0)).unwrap(),1);
        assert!(!conn.is_autocommit());
    }

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
