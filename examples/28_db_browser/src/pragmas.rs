//! Original Edit Pragmas fields, shared by validation and form controls.
pub const FIELDS: &[(&str, &str)] = &[
    ("auto_vacuum", "Auto Vacuum"),
    ("automatic_index", "Automatic Index"),
    ("checkpoint_fullfsync", "Checkpoint Full FSYNC"),
    ("foreign_keys", "Foreign Keys"),
    ("fullfsync", "Full FSYNC"),
    ("ignore_check_constraints", "Ignore Check Constraints"),
    ("journal_mode", "Journal Mode"),
    ("journal_size_limit", "Journal Size Limit"),
    ("locking_mode", "Locking Mode"),
    ("max_page_count", "Max Page Count"),
    ("page_size", "Page Size"),
    ("recursive_triggers", "Recursive Triggers"),
    ("secure_delete", "Secure Delete"),
    ("synchronous", "Synchronous"),
    ("temp_store", "Temp Store"),
    ("user_version", "User Version"),
    ("wal_autocheckpoint", "WAL Auto Checkpoint"),
    ("case_sensitive_like", "Case Sensitive LIKE"),
    ("cache_size", "Cache Size"),
    ("busy_timeout", "Busy Timeout (ms)"),
];

pub fn is_boolean(key: &str) -> bool {
    matches!(
        key,
        "automatic_index"
            | "checkpoint_fullfsync"
            | "foreign_keys"
            | "fullfsync"
            | "ignore_check_constraints"
            | "recursive_triggers"
            | "case_sensitive_like"
    )
}

pub fn choices(key: &str) -> &'static [(&'static str, &'static str)] {
    match key {
        "auto_vacuum" => &[("0", "None"), ("1", "Full"), ("2", "Incremental")],
        "journal_mode" => &[
            ("delete", "Delete"),
            ("truncate", "Truncate"),
            ("persist", "Persist"),
            ("memory", "Memory"),
            ("wal", "WAL"),
            ("off", "Off"),
        ],
        "locking_mode" => &[("normal", "Normal"), ("exclusive", "Exclusive")],
        "page_size" => &[
            ("512", "512"),
            ("1024", "1024"),
            ("2048", "2048"),
            ("4096", "4096"),
            ("8192", "8192"),
            ("16384", "16384"),
            ("32768", "32768"),
            ("65536", "65536"),
        ],
        "secure_delete" => &[("0", "Off"), ("1", "On"), ("2", "Fast")],
        "synchronous" => &[("0", "Off"), ("1", "Normal"), ("2", "Full"), ("3", "Extra")],
        "temp_store" => &[("0", "Default"), ("1", "File"), ("2", "Memory")],
        _ => &[],
    }
}

pub fn normalize(key: &str, value: &str) -> Result<String, String> {
    if !FIELDS.iter().any(|(name, _)| *name == key) {
        return Err("Unsupported pragma".into());
    }
    let value = value.trim();
    if is_boolean(key) {
        return match value.to_ascii_lowercase().as_str() {
            "1" | "on" | "true" | "yes" => Ok("1".into()),
            "0" | "off" | "false" | "no" => Ok("0".into()),
            _ => Err(format!("Invalid boolean PRAGMA {key}")),
        };
    }
    let options = choices(key);
    if !options.is_empty() {
        return options
            .iter()
            .find(|(raw, label)| {
                raw.eq_ignore_ascii_case(value) || label.eq_ignore_ascii_case(value)
            })
            .map(|(raw, _)| (*raw).into())
            .ok_or_else(|| format!("Invalid PRAGMA {key}: {value}"));
    }
    let number = value
        .parse::<i64>()
        .map_err(|_| format!("PRAGMA {key} needs an integer"))?;
    let (min, max) = match key {
        "journal_size_limit" => (-1, i64::MAX),
        "max_page_count" => (1, 4_294_967_294),
        "cache_size" => (i32::MIN as i64, i32::MAX as i64),
        _ => (0, i32::MAX as i64),
    };
    if !(min..=max).contains(&number) {
        return Err(format!("PRAGMA {key} must be between {min} and {max}"));
    }
    Ok(number.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pragma_save_confirmation_defaults_to_cancel_and_only_commits_after_accepting() {
        use crate::{csv_files::interaction_tests::capture, ui_build, Demo, PresentBackend};
        use next_gui_ui::{Theme, UiKey};
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400, 1000), (800, 600)] {
                let mut demo = Demo::new(width, height, PresentBackend::Softbuffer);
                demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
                demo.state.new_memory_db().unwrap();
                demo.state
                    .conn
                    .as_ref()
                    .unwrap()
                    .execute_batch("CREATE TABLE kept(v);INSERT INTO kept VALUES('pending')")
                    .unwrap();
                demo.state.dirty = true;
                demo.state.tab = crate::state::MainTab::Pragmas;
                demo.state.dock_edit = false;
                demo.do_rebuild();
                let version = demo
                    .state
                    .pragmas
                    .iter()
                    .position(|(key, _)| key == "user_version")
                    .unwrap();
                demo.ui.set_text_input(demo.ids.pragma_inputs[version], "7");
                demo.handle_button(demo.ids.btn_pragma_apply);
                demo.do_rebuild();
                assert_eq!(demo.state.confirm_kind, crate::state::ConfirmKind::Pragmas);
                assert!(demo.state.confirm_open);
                assert_eq!(demo.ui.focused(), Some(demo.ids.modal));
                capture(&mut demo, theme_name, width, "pragmas-confirm");
                demo.ui.key_input(UiKey::Enter);
                demo.handle_modal();
                assert!(!demo.state.confirm_open && demo.state.dirty);
                assert_eq!(
                    crate::db::read_pragma(demo.state.conn.as_ref().unwrap(), "user_version")
                        .unwrap(),
                    "0"
                );
                assert!(!demo.state.conn.as_ref().unwrap().is_autocommit());
                demo.do_rebuild();
                demo.handle_button(demo.ids.btn_pragma_apply);
                demo.do_rebuild();
                demo.ui.key_input(UiKey::Tab);
                demo.ui.key_input(UiKey::Enter);
                demo.handle_modal();
                assert!(
                    !demo.state.dirty && demo.state.error_msg.is_none(),
                    "{:?}",
                    demo.state.error_msg
                );
                assert_eq!(
                    crate::db::read_pragma(demo.state.conn.as_ref().unwrap(), "user_version")
                        .unwrap(),
                    "7"
                );
                demo.state.revert_changes().unwrap();
                assert_eq!(
                    crate::db::count_rows(demo.state.conn.as_ref().unwrap(), "kept").unwrap(),
                    1
                );
            }
        }
        let mut state = crate::state::AppState::empty();
        state.new_memory_db().unwrap();
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("CREATE TABLE pending(v);INSERT INTO pending VALUES(1)")
            .unwrap();
        state.dirty = true;
        let version = state
            .pragmas
            .iter()
            .position(|(key, _)| key == "user_version")
            .unwrap();
        state.pragma_drafts[version] = "invalid".into();
        assert!(state.apply_pragmas(true).is_err());
        assert!(state.dirty && !state.conn.as_ref().unwrap().is_autocommit());
        state.revert_changes().unwrap();
        assert!(!crate::db::list_schema(state.conn.as_ref().unwrap())
            .unwrap()
            .iter()
            .any(|object| object.name == "pending"));
    }
    #[test]
    fn saved_page_size_vacuum_and_extended_settings_survive_reopening() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("database.db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE t(v);INSERT INTO t VALUES('preserved')")
            .unwrap();
        drop(conn);
        let mut state = crate::state::AppState::empty();
        state.open_path(path.to_str().unwrap()).unwrap();
        for (key, value) in [
            ("page_size", "8192"),
            ("auto_vacuum", "2"),
            ("user_version", "42"),
            ("secure_delete", "FAST"),
            ("journal_size_limit", "65536"),
        ] {
            let index = state
                .pragmas
                .iter()
                .position(|(name, _)| name == key)
                .unwrap();
            state.pragma_drafts[index] = value.into();
        }
        state.save_pragmas().unwrap();
        assert!(!state.conn.as_ref().unwrap().is_autocommit());
        state.close_db();
        let conn = rusqlite::Connection::open(&path).unwrap();
        assert_eq!(crate::db::read_pragma(&conn, "page_size").unwrap(), "8192");
        assert_eq!(crate::db::read_pragma(&conn, "auto_vacuum").unwrap(), "2");
        assert_eq!(crate::db::read_pragma(&conn, "user_version").unwrap(), "42");
        assert_eq!(crate::db::count_rows(&conn, "t").unwrap(), 1);
    }

    #[test]
    fn validation_precedes_all_writes_and_unchanged_like_extension_is_retained() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let mut state = crate::state::AppState::empty();
        state.new_memory_db().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        state
            .conn
            .as_ref()
            .unwrap()
            .create_scalar_function(
                "like",
                2,
                rusqlite::functions::FunctionFlags::SQLITE_UTF8,
                move |_| {
                    count.fetch_add(1, Ordering::Relaxed);
                    Ok(0)
                },
            )
            .unwrap();
        state.refresh_pragmas().unwrap();
        let version = state
            .pragmas
            .iter()
            .position(|(key, _)| key == "user_version")
            .unwrap();
        let timeout = state
            .pragmas
            .iter()
            .position(|(key, _)| key == "busy_timeout")
            .unwrap();
        state.pragma_drafts[version] = "12".into();
        state.pragma_drafts[timeout] = "invalid".into();
        assert!(state.save_pragmas().is_err());
        assert_eq!(
            crate::db::read_pragma(state.conn.as_ref().unwrap(), "user_version").unwrap(),
            "0"
        );
        assert!(!state.conn.as_ref().unwrap().is_autocommit());
        state.pragma_drafts[timeout] = "5000".into();
        state.save_pragmas().unwrap();
        let before = calls.load(Ordering::Relaxed);
        assert_eq!(
            crate::db::read_pragma(state.conn.as_ref().unwrap(), "case_sensitive_like").unwrap(),
            "1"
        );
        assert!(calls.load(Ordering::Relaxed) > before);
        let like = state
            .pragmas
            .iter()
            .position(|(key, _)| key == "case_sensitive_like")
            .unwrap();
        state.pragma_drafts[like] = "0".into();
        state.save_pragmas().unwrap();
        assert_eq!(
            crate::db::read_pragma(state.conn.as_ref().unwrap(), "case_sensitive_like").unwrap(),
            "0"
        );
    }

    #[test]
    fn original_pragma_controls_keep_focus_custom_values_and_scroll_across_rebuilds() {
        use crate::{csv_files::interaction_tests::capture, ui_build, Demo, PresentBackend};
        use next_gui_ui::{Theme, UiKey};
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400, 1000), (800, 600)] {
                let mut demo = Demo::new(width, height, PresentBackend::Softbuffer);
                demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
                demo.state.new_memory_db().unwrap();
                demo.state.tab = crate::state::MainTab::Pragmas;
                demo.state.dock_edit = false;
                demo.do_rebuild();
                assert_eq!(demo.state.pragmas.len(), 20);
                for (index, (key, _)) in demo.state.pragmas.iter().enumerate() {
                    let id = if is_boolean(key) {
                        demo.ids.pragma_switches[index]
                    } else if choices(key).is_empty() {
                        demo.ids.pragma_inputs[index]
                    } else {
                        demo.ids.pragma_selects[index]
                    };
                    assert_ne!(id.0, 0, "{key}");
                    assert!(demo.ui.accessible_name(id).is_some());
                }
                capture(&mut demo, theme_name, width, "pragmas-top");
                let version = demo
                    .state
                    .pragmas
                    .iter()
                    .position(|(key, _)| key == "user_version")
                    .unwrap();
                demo.ui
                    .set_scroll_offset(demo.ids.pragma_scroll, 0.0, 10000.0);
                demo.ui.sync(&mut demo.scene);
                let scroll = demo.ui.scroll_offset(demo.ids.pragma_scroll).unwrap();
                assert!(scroll.1 > 0.0);
                demo.ui.set_focus(Some(demo.ids.pragma_inputs[version]));
                demo.ui.key_input(UiKey::SelectAll);
                demo.ui.paste_into_focused_text("42");
                demo.snapshot_inputs();
                demo.do_rebuild();
                assert_eq!(demo.ui.focused(), Some(demo.ids.pragma_inputs[version]));
                assert_eq!(demo.ui.scroll_offset(demo.ids.pragma_scroll), Some(scroll));
                demo.ui.key_input(UiKey::Undo);
                assert_eq!(
                    demo.ui.text_input_value(demo.ids.pragma_inputs[version]),
                    Some("0")
                );
                demo.ui.key_input(UiKey::Redo);
                demo.snapshot_inputs();
                demo.win_w = if width == 800 { 1400 } else { 800 };
                demo.do_rebuild();
                assert_eq!(demo.ui.focused(), Some(demo.ids.pragma_inputs[version]));
                assert_eq!(
                    demo.ui.text_input_value(demo.ids.pragma_inputs[version]),
                    Some("42")
                );
                demo.win_w = width;
                demo.do_rebuild();
                demo.ui
                    .set_scroll_offset(demo.ids.pragma_scroll, 0.0, 10000.0);
                demo.ui.sync(&mut demo.scene);
                capture(&mut demo, theme_name, width, "pragmas-bottom");
                demo.handle_button(demo.ids.btn_pragma_apply);
                assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
                assert_eq!(
                    crate::db::read_pragma(demo.state.conn.as_ref().unwrap(), "user_version")
                        .unwrap(),
                    "42"
                );
                demo.state.read_only = true;
                demo.do_rebuild();
                assert_eq!(
                    demo.ui.button_enabled(demo.ids.btn_pragma_apply),
                    Some(false)
                );
            }
        }
    }
    #[test]
    fn original_fields_validate_without_losing_fast_mode_or_numeric_limits() {
        assert_eq!(FIELDS.len(), 20);
        for (i, (key, _)) in FIELDS.iter().enumerate() {
            assert!(!FIELDS[..i].iter().any(|(old, _)| old == key));
        }
        for (key, value, expected) in [
            ("secure_delete", "FAST", "2"),
            ("synchronous", "Extra", "3"),
            ("page_size", "65536", "65536"),
            ("journal_size_limit", "-1", "-1"),
            ("max_page_count", "4294967294", "4294967294"),
            ("foreign_keys", "yes", "1"),
        ] {
            assert_eq!(normalize(key, value).unwrap(), expected);
        }
        for (key, value) in [
            ("page_size", "4000"),
            ("auto_vacuum", "3"),
            ("busy_timeout", "-1"),
            ("user_version", "2147483648"),
            ("max_page_count", "0"),
            ("journal_size_limit", "-2"),
            ("foreign_keys", "maybe"),
            ("cache_size", "3;DELETE FROM t"),
        ] {
            assert!(normalize(key, value).is_err(), "{key}={value}");
        }
    }
}
