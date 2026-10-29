//! DB4S conditional expressions run against the live SQLite connection.
use crate::{
    db::{self, CellValue},
    state::{AppState, CondFormatRule},
};
use next_gui_scene::Color;
use rusqlite::{types::Value, Connection, Statement};
use std::{borrow::Cow, sync::Arc};

pub(crate) struct Cache {
    schema: String,
    table: Option<String>,
    values: Arc<()>,
    columns: Vec<String>,
    rules: Vec<CondFormatRule>,
    escape: String,
    revision: u64,
    matches: Vec<Vec<Option<usize>>>,
}

fn expression_sql(filter: &str, escape: &str) -> String {
    let condition = crate::filters::condition(filter, escape).unwrap_or_default();
    format!("SELECT CAST((?1 {condition}) AS TEXT)")
}

pub(crate) fn validate(conn: &Connection, filter: &str, escape: &str) -> Result<(), String> {
    crate::filters::validate_escape(escape)?;
    crate::filters::validate(filter)?;
    conn.prepare(&expression_sql(filter, escape))
        .map_err(|error| error.to_string())?;
    Ok(())
}

// Match the original model's UTF-8 quick test, including an incomplete last code point.
fn text(bytes: &[u8]) -> Option<Cow<'_, str>> {
    if let Some(bytes) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return Some(String::from_utf8_lossy(bytes));
    }
    if bytes.len() > 512 {
        if bytes.contains(&0) {
            return None;
        }
        if std::str::from_utf8(&bytes[..512]).is_err_and(|error| error.error_len().is_some()) {
            return None;
        }
        return Some(String::from_utf8_lossy(bytes));
    }
    let text = std::str::from_utf8(bytes).ok()?;
    text.chars()
        .all(|ch| {
            (!ch.is_control() || ch.is_whitespace())
                && !matches!(ch as u32, 0xFDD0..=0xFDEF)
                && ch as u32 & 0xFFFE != 0xFFFE
        })
        .then_some(Cow::Borrowed(text))
}

fn cell_text(value: &CellValue) -> Option<Cow<'_, str>> {
    match value {
        CellValue::Null => None,
        CellValue::Integer(value) => Some(Cow::Owned(value.to_string())),
        CellValue::Real(value) => Some(Cow::Owned(db::sqlite_real_text(*value))),
        CellValue::Text(bytes) | CellValue::Blob(bytes) => text(bytes),
    }
}

fn numeric_operand(text: &str) -> Result<Option<Value>, ()> {
    let text = text.trim();
    let Ok(number) = text.parse::<f64>() else {
        return Ok(None);
    };
    if !number.is_finite() {
        return Err(());
    }
    if !text
        .bytes()
        .all(|ch| ch.is_ascii_digit() || b"+-.eE".contains(&ch))
    {
        return Ok(None);
    }
    Ok(Some(if !text.contains(['.', 'e', 'E']) {
        text.parse::<i64>()
            .map(Value::Integer)
            .unwrap_or(Value::Real(number))
    } else {
        Value::Real(number)
    }))
}

struct Expression<'conn> {
    statement: Statement<'conn>,
    numeric: bool,
}
impl Expression<'_> {
    fn matches(&mut self, value: &str) -> bool {
        let number = if self.numeric {
            match numeric_operand(value) {
                Ok(number) => number,
                Err(()) => return false,
            }
        } else {
            None
        };
        let result = if let Some(number) = number {
            self.statement
                .query_row([number], |row| row.get::<_, Option<String>>(0))
        } else {
            self.statement
                .query_row([value], |row| row.get::<_, Option<String>>(0))
        };
        result.is_ok_and(|result| result.as_deref() == Some("1"))
    }
}

fn matches(state: &AppState) -> Vec<Vec<Option<usize>>> {
    let result = state.browse().result.as_ref().unwrap();
    let rules = &state.cond_rules;
    let mut expressions: Vec<_> = rules
        .iter()
        .map(|rule| {
            if rule.op != 5
                || rule.value.is_empty()
                || rule.schema != state.browse().schema
                || rule.table != state.browse().table
            {
                return None;
            }
            let condition = crate::filters::condition(&rule.value, &state.prefs.filter_escape)
                .unwrap_or_default();
            let statement = state
                .conn
                .as_ref()?
                .prepare(&expression_sql(&rule.value, &state.prefs.filter_escape))
                .ok()?;
            Some(Expression {
                statement,
                numeric: !condition.contains('\''),
            })
        })
        .collect();
    let columns: Vec<Vec<usize>> = result
        .columns
        .iter()
        .map(|column| {
            rules
                .iter()
                .enumerate()
                .filter(|(_, rule)| {
                    rule.schema == state.browse().schema
                        && rule.table == state.browse().table
                        && &rule.column == column
                })
                .map(|(index, _)| index)
                .collect()
        })
        .collect();
    (0..result.rows.len())
        .map(|row| {
            let cells = result.values.row(row).ok();
            columns
                .iter()
                .enumerate()
                .map(|(column, indices)| {
                    let value = cell_text(cells.as_ref()?.get(column)?)?;
                    indices.iter().copied().find(|index| {
                        let rule = &rules[*index];
                        let legacy = result.rows[row]
                            .get(column)
                            .map(String::as_str)
                            .unwrap_or_default();
                        match rule.op {
                            0 => legacy == rule.value,
                            1 => legacy != rule.value,
                            2 => legacy.contains(&rule.value),
                            3 | 4 => legacy
                                .parse::<f64>()
                                .ok()
                                .zip(rule.value.parse::<f64>().ok())
                                .is_some_and(|(a, b)| if rule.op == 3 { a > b } else { a < b }),
                            5 => {
                                rule.value.is_empty()
                                    || expressions[*index]
                                        .as_mut()
                                        .is_some_and(|expression| expression.matches(&value))
                            }
                            _ => false,
                        }
                    })
                })
                .collect()
        })
        .collect()
}

pub(crate) fn colors(state: &AppState) -> Vec<Vec<Option<Color>>> {
    let Some(result) = &state.browse().result else {
        return Vec::new();
    };
    if !state.cond_rules.iter().any(|rule| {
        rule.schema == state.browse().schema
            && rule.table == state.browse().table
            && result.columns.contains(&rule.column)
    }) {
        return vec![vec![None; result.columns.len()]; result.rows.len()];
    }
    let values = result.values.identity();
    let mut cache = state.cond_match.borrow_mut();
    let current = cache.as_ref().is_some_and(|cache| {
        cache.schema == state.browse().schema
            && cache.table == state.browse().table
            && Arc::ptr_eq(&cache.values, &values)
            && cache.columns == result.columns
            && cache.rules == state.cond_rules
            && cache.escape == state.prefs.filter_escape
            && (state.conn.is_none() || cache.revision == state.sql_log.revision())
    });
    let found = if current {
        cache.as_ref().unwrap().matches.clone()
    } else {
        // shortcut: scan the loaded page once; move matching to the worker when Browse loads more than500 rows.
        let found = matches(state);
        if state.conn.is_some() {
            *cache = Some(Cache {
                schema: state.browse().schema.clone(),
                table: state.browse().table.clone(),
                values,
                columns: result.columns.clone(),
                rules: state.cond_rules.clone(),
                escape: state.prefs.filter_escape.clone(),
                revision: state.sql_log.revision(),
                matches: found.clone(),
            });
        }
        found
    };
    found
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|index| {
                    index.map(|index| {
                        let rgb = state.cond_rules[index].color_rgb;
                        Color::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
                    })
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> AppState {
        let mut state = AppState::empty();
        state.new_memory_db().unwrap();
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE cells(v); INSERT INTO cells VALUES('alpha'),('ALPHA'),('A_B'),('AXB'),(''),(NULL),(X'00FF'),(X'616C706861'),(CAST(X'80' AS TEXT)),(1),(1.0),('1'),('1.0'),('01'),('1e0'),(9223372036854775807),('9223372036854775807'),(9223372036854775806),(1.2345678901234567),(CAST(X'610062' AS TEXT));").unwrap();
        state.refresh_schema().unwrap();
        state.switch_browse_table_in("main", "cells").unwrap();
        state.cond_rules = vec![CondFormatRule {
            schema: "main".into(),
            table: Some("cells".into()),
            column: "v".into(),
            op: 5,
            value: String::new(),
            color_rgb: 0xABCDEF,
        }];
        state
    }
    fn hits(state: &mut AppState, filter: &str) -> Vec<usize> {
        state.cond_rules[0].value = filter.into();
        colors(state)
            .iter()
            .enumerate()
            .filter(|(_, row)| row[0].is_some())
            .map(|(row, _)| row)
            .collect()
    }

    #[test]
    fn original_expressions_use_live_sqlite_semantics_exact_numbers_and_typed_bytes() {
        let mut state = source();
        assert_eq!(hits(&mut state, "alp"), [0, 1, 7]);
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("PRAGMA case_sensitive_like=ON")
            .unwrap();
        assert_eq!(hits(&mut state, "alp"), [0, 7]);
        state.prefs.filter_escape = "!".into();
        assert_eq!(hits(&mut state, "A!_B"), [2]);
        assert_eq!(hits(&mut state, "/^a(?=lp)/"), [0, 7]);
        assert!(hits(&mut state, "=NULL").is_empty());
        assert_eq!(hits(&mut state, "=''"), [4]);
        assert_eq!(hits(&mut state, "=1"), [9, 11]);
        assert_eq!(hits(&mut state, "1~1.1"), [9, 10, 11, 12, 13, 14]);
        assert_eq!(
            hits(&mut state, ">=9223372036854775807"),
            [0, 1, 2, 3, 4, 7, 15, 16]
        );
        assert_eq!(hits(&mut state, "=1.23456789012346"), [18]);
        // Upstream ignores an empty SQL condition only when its filter is actually empty.
        assert_eq!(hits(&mut state, "%"), [9, 11, 13]);
        assert_eq!(
            hits(&mut state, ""),
            (0..20)
                .filter(|row| ![5, 6, 8, 19].contains(row))
                .collect::<Vec<_>>()
        );
        assert!(hits(&mut state, "='; DELETE FROM cells; --").is_empty());
        assert_eq!(
            state
                .conn
                .as_ref()
                .unwrap()
                .query_row("SELECT count(*) FROM cells", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            20
        );
        assert!(!state.conn.as_ref().unwrap().is_autocommit());
    }

    #[test]
    fn cache_avoids_repeated_queries_retains_busy_results_and_invalidates_changes() {
        let mut state = source();
        state.cond_rules[0].value = "alp".into();
        let expected = colors(&state);
        let revision = state.sql_log.revision();
        assert_eq!(colors(&state), expected);
        assert_eq!(state.sql_log.revision(), revision);
        let conn = state.conn.take();
        state
            .sql_log
            .push(crate::sql_log::Source::User, "SELECT busy");
        assert_eq!(colors(&state), expected);
        state.conn = conn;
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("PRAGMA case_sensitive_like=ON")
            .unwrap();
        assert!(colors(&state)[1][0].is_none());
        state.cond_rules[0].color_rgb = 0x123456;
        assert_eq!(colors(&state)[0][0], Some(Color::rgb(0x12, 0x34, 0x56)));
        let mut fallback = state.cond_rules[0].clone();
        fallback.value.clear();
        fallback.color_rgb = 0x112233;
        state.cond_rules.insert(0, fallback);
        assert_eq!(colors(&state)[0][0], Some(Color::rgb(0x11, 0x22, 0x33)));
        state.cond_rules.swap(0, 1);
        assert_eq!(colors(&state)[0][0], Some(Color::rgb(0x12, 0x34, 0x56)));
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("UPDATE cells SET v='changed' WHERE rowid=1")
            .unwrap();
        state.refresh_browse().unwrap();
        assert_eq!(colors(&state)[0][0], Some(Color::rgb(0x11, 0x22, 0x33)));
        state.browse_mut().schema = "temp".into();
        assert!(colors(&state).iter().flatten().all(Option::is_none));
        state.close_db();
        assert!(state.cond_match.borrow().is_none());
    }

    #[test]
    fn live_filter_worker_keeps_old_formats_then_matches_the_new_snapshot() {
        use std::time::{Duration, Instant};
        let mut state = source();
        state.cond_rules[0].value = "alp".into();
        let before = colors(&state);
        let identity = state.browse().result.as_ref().unwrap().values.identity();
        state.browse_mut().global_filters = vec!["alp".into()];
        state.start_browse_filter().unwrap();
        assert!(state.conn.is_none());
        assert_eq!(colors(&state), before);
        let deadline = Instant::now() + Duration::from_secs(10);
        while state.browse_work.is_some() {
            assert!(Instant::now() < deadline);
            state.poll_browse_filter().unwrap();
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(state.conn.is_some());
        assert!(!state.conn.as_ref().unwrap().is_autocommit());
        assert!(!Arc::ptr_eq(
            &identity,
            &state.browse().result.as_ref().unwrap().values.identity()
        ));
        let after = colors(&state);
        assert!(!after.is_empty());
        assert!(after.len() < before.len());
        assert!(after.iter().flatten().all(Option::is_some));
    }

    #[test]
    fn binary_quick_test_and_validation_follow_original_content_classification() {
        assert!(text(b"printable\t\n").is_some());
        assert!(text(b"zero\0").is_none());
        assert!(text(&[0xFF]).is_none());
        assert!(text("\u{FDD0}".as_bytes()).is_none());
        assert!(text("\u{1FFFE}".as_bytes()).is_none());
        assert!(text(&[0xFF, 0xFE, b'x', 0]).is_none());
        assert_eq!(text(b"\xEF\xBB\xBFvalue").as_deref(), Some("value"));
        let mut boundary = vec![b'x'; 511];
        boundary.extend_from_slice("界".as_bytes());
        assert!(text(&boundary).is_some());
        boundary.push(0);
        assert!(text(&boundary).is_none());
        let mut late_invalid = vec![b'x'; 513];
        late_invalid.push(0xFF);
        assert!(text(&late_invalid).is_some());
        let state = source();
        let conn = state.conn.as_ref().unwrap();
        for filter in [
            "",
            "1~2",
            "/^a(?=l)/",
            "O'Reilly",
            "='; DELETE FROM cells; --",
        ] {
            validate(conn, filter, "\\").unwrap();
        }
        assert!(validate(conn, "/[/", "\\").is_err());
        assert!(validate(conn, "alpha", "two").is_err());
        assert!(validate(conn, "a\0b", "\\").is_err());
        assert_eq!(
            numeric_operand("9223372036854775807").unwrap(),
            Some(Value::Integer(i64::MAX))
        );
        assert_eq!(numeric_operand("1.0").unwrap(), Some(Value::Real(1.0)));
        assert!(numeric_operand("Inf").is_err());
        assert_eq!(numeric_operand("1; DROP TABLE cells").unwrap(), None);
    }
}
