use super::*;

#[test]
fn cell_editor_sessions_survive_rebuilds_but_never_follow_another_cell_or_replacement() {
    let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap();
    let original = (0..40).map(|line| format!("第{line:02}行 alpha 中文🙂\n")).collect::<String>();
    let conn = demo.state.conn.as_ref().unwrap();
    conn.execute_batch("CREATE TABLE cells(id INTEGER PRIMARY KEY, value TEXT); CREATE TABLE other(id INTEGER PRIMARY KEY, value TEXT);").unwrap();
    for table in ["cells", "other"] {
        for id in [1, 2] {
            conn.execute(&format!("INSERT INTO {table} VALUES(?1,?2)"), rusqlite::params![id, original]).unwrap();
        }
    }
    demo.state.refresh_schema().unwrap(); demo.state.tab = MainTab::Browse;
    demo.state.switch_browse_table_in("main", "cells").unwrap();
    demo.state.goto_browse_record(1).unwrap(); demo.state.browse_mut().sel_col = Some(1);
    demo.state.sync_edit_cell_from_selection(); demo.state.dock_focus = DockKind::Plot;
    demo.state.dock_ratio = 0.55;
    let folder = std::env::var("NEXT_GUI_CELL_SESSION_QA_OUT").ok().map(PathBuf::from);
    if let Some(folder) = &folder { std::fs::create_dir_all(folder).unwrap(); }
    for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
        demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.edit_value));
        demo.ui.select_text_area_range(demo.ids.edit_value, 2..6);
        demo.ui.paste_into_focused_text("编辑"); demo.snapshot_inputs();
        let changed = demo.state.edit_value.clone();
        assert_ne!(changed, original); assert!(demo.state.has_cell_draft());
        for (width, height, size) in [(1400, 1000, 13.0), (800, 600, 18.0)] {
            demo.win_w = width; demo.win_h = height; demo.state.prefs.font_size = size;
            demo.do_rebuild();
            assert_eq!(demo.ui.focused(), Some(demo.ids.edit_value));
            assert_eq!(demo.ui.text_area_caret(demo.ids.edit_value), Some(4));
            assert_eq!(demo.ui.text_area_value(demo.ids.edit_value), Some(changed.as_str()));
            assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs();
            demo.state.browse_mut().sel_row_start = None;
            demo.state.browse_mut().sel_col_start = None;
            let revision = demo.state.edit_revision;
            demo.poll_selection_changes();
            assert_eq!(demo.state.edit_revision, revision, "Range anchors must not reload the current cell");
            demo.do_rebuild();
            assert_eq!(demo.state.edit_value, original);
            assert_eq!(demo.ui.text_area_selection(demo.ids.edit_value), Some(2..6));
            assert!(demo.ui.key_input(UiKey::Redo)); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.state.edit_value, changed);
            let end = changed.chars().count(); demo.ui.select_text_area_range(demo.ids.edit_value, end..end); demo.ui.sync(&mut demo.scene); demo.snapshot_inputs();
            let scroll = demo.ui.text_area_scroll(demo.ids.edit_value).unwrap(); assert!(scroll > 0.0);
            demo.do_rebuild(); assert_eq!(demo.ui.text_area_scroll(demo.ids.edit_value), Some(scroll));
            demo.ui.select_text_area_range(demo.ids.edit_value, std::ops::Range { start: 4, end: 2 });
            demo.ui.ime_preedit("東京", Some((3, 6))); demo.ui.sync(&mut demo.scene);
            let candidate = demo.ui.ime_cursor_area(&demo.scene); assert!(candidate.is_some());
            demo.snapshot_inputs();
            demo.do_rebuild();
            assert_eq!(demo.ui.ime_cursor_area(&demo.scene), candidate);
            assert_eq!(demo.ui.text_area_caret(demo.ids.edit_value), Some(2));
            assert_eq!(demo.ui.focused_text_for_copy().as_deref(), Some("编辑"));
            if let Some(folder) = &folder { crate::plot_export::capture(&mut demo, folder, &format!("cell-session-{name}-{width}")); }
            assert!(demo.ui.ime_commit("東京")); demo.snapshot_inputs(); demo.do_rebuild();
            assert!(demo.state.edit_value.contains("東京"));
            assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs(); demo.do_rebuild();
            assert_eq!(demo.state.edit_value, changed);
            demo.ui.select_text_area_range(demo.ids.edit_value, 2..4); demo.snapshot_inputs();
        }
        demo.ui.set_focus(Some(demo.ids.edit_col)); demo.ui.key_input(UiKey::SelectAll);
        demo.ui.paste_into_focused_text("temporary"); demo.snapshot_inputs(); demo.do_rebuild();
        assert_eq!(demo.ui.focused(), Some(demo.ids.edit_col));
        assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs(); demo.do_rebuild();
        assert_eq!(demo.state.edit_col, "value");
        demo.ui.set_focus(Some(demo.ids.edit_type)); demo.ui.key_input(UiKey::Space); demo.do_rebuild();
        assert_eq!(demo.ui.popup_owner(), Some(demo.ids.edit_type)); demo.ui.key_input(UiKey::Escape);
        demo.ui.set_focus(Some(demo.ids.edit_value));
        // Closing removes the widgets; their same-cell session must outlive the panel.
        demo.snapshot_inputs(); demo.handle_menu(mid::VIEW_DOCK_EDIT); demo.do_rebuild();
        assert_eq!(demo.ids.edit_value, WidgetId(0));
        demo.handle_menu(mid::VIEW_DOCK_EDIT); demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.edit_value));
        assert_eq!(demo.ui.text_area_selection(demo.ids.edit_value), Some(2..4));
        assert!(demo.ui.key_input(UiKey::Undo)); demo.snapshot_inputs(); demo.do_rebuild();
        assert_eq!(demo.state.edit_value, original);
        assert!(demo.ui.key_input(UiKey::Redo)); demo.snapshot_inputs();
        assert_eq!(demo.state.edit_value, changed);
        demo.state.discard_cell_draft(); demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.key_input(UiKey::Undo);
        demo.snapshot_inputs(); assert_eq!(demo.state.edit_value, original);
        demo.win_w = 1400; demo.win_h = 1000; demo.state.prefs.font_size = 13.0;
    }
    // Leave redo and column-name history whose current text equals the next cell.
    demo.do_rebuild(); demo.ui.set_focus(Some(demo.ids.edit_value));
    demo.ui.paste_into_focused_text("old cell only"); demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs();
    demo.ui.set_focus(Some(demo.ids.edit_col)); demo.ui.key_input(UiKey::SelectAll);
    demo.ui.paste_into_focused_text("temporary"); demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs();
    assert_eq!(demo.state.edit_col, "value"); assert_eq!(demo.state.edit_value, original);
    demo.state.goto_browse_record(2).unwrap();
    demo.snapshot_inputs(); demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.edit_col)); demo.ui.key_input(UiKey::Redo);
    assert_eq!(demo.ui.text_input_value(demo.ids.edit_col), Some("value"));
    demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.key_input(UiKey::Redo);
    demo.snapshot_inputs(); assert_eq!(demo.state.edit_value, original);
    // File/NULL/mode replacement also invalidates history before a stale UI snapshot.
    let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("value.txt");
    std::fs::write(&path, "replacement").unwrap();
    demo.ui.paste_into_focused_text("stale draft"); demo.snapshot_inputs();
    demo.state.import_cell_file(&path).unwrap(); demo.snapshot_inputs(); demo.do_rebuild();
    assert_eq!(demo.state.edit_value, "replacement"); demo.ui.set_focus(Some(demo.ids.edit_value));
    demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs(); assert_eq!(demo.state.edit_value, "replacement");
    demo.state.set_cell_null().unwrap(); demo.do_rebuild(); demo.ui.set_focus(Some(demo.ids.edit_value));
    demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs(); assert_eq!(demo.state.edit_type, 0);
    assert_eq!(demo.state.edit_value, ""); demo.state.discard_cell_draft(); demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.paste_into_focused_text("not for other table");
    demo.ui.key_input(UiKey::Undo); demo.snapshot_inputs();
    demo.state.switch_browse_table_in("main", "other").unwrap(); demo.state.goto_browse_record(1).unwrap();
    demo.state.browse_mut().sel_col = Some(1); demo.state.sync_edit_cell_from_selection(); demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.key_input(UiKey::Redo); demo.snapshot_inputs();
    assert_eq!(demo.state.edit_value, original); assert!(!demo.state.has_cell_draft());
    demo.state.tab = MainTab::ExecuteSql;
    demo.state.run_sql_text_sync("SELECT value FROM cells ORDER BY id").unwrap();
    demo.state.sql_selection.rows = Some((0,0)); demo.state.sync_edit_cell_from_sql_selection().unwrap(); demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.select_text_area_range(demo.ids.edit_value, 2..6);
    demo.snapshot_inputs(); demo.do_rebuild();
    assert_eq!(demo.ui.focused(), Some(demo.ids.edit_value));
    assert_eq!(demo.ui.focused_text_for_copy().as_deref(), Some("0行 a"));
    assert!(!demo.ui.ime_preedit("unsafe", None)); demo.ui.key_input(UiKey::Redo);
    demo.snapshot_inputs(); assert_eq!(demo.state.edit_value, original); assert!(!demo.state.has_cell_draft());
    demo.state.run_sql_text_sync("SELECT value FROM cells ORDER BY id").unwrap();
    demo.state.sql_selection.rows = Some((0,0)); assert!(demo.state.sync_edit_cell_from_sql_selection().unwrap());
    demo.snapshot_inputs(); demo.do_rebuild();
    let end = original.chars().count();
    assert_eq!(demo.ui.text_area_selection(demo.ids.edit_value), Some(end..end));
    assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT value FROM cells WHERE id=1", [], |row| row.get::<_,String>(0)).unwrap(), original);
}
