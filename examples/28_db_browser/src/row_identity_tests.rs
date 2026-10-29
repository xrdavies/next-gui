use super::*;
use crate::{Demo, PresentBackend, Theme, UiKey};

fn ledger() -> AppState {
    let mut state = AppState::empty();
    state.new_memory_db().unwrap();
    state
        .conn
        .as_ref()
        .unwrap()
        .execute_batch(crate::ROW_KEY_QA_SQL)
        .unwrap();
    state.write_changes().unwrap();
    state.refresh_schema().unwrap();
    state.tab = MainTab::Browse;
    state.switch_browse_table("warehouse_ledger").unwrap();
    state.prefs.auto_switch_mode = false;
    select(&mut state, 0, 3);
    state
}
fn select(state: &mut AppState, row: usize, col: usize) {
    let tab = state.browse_mut();
    tab.sel_row = Some(row);
    tab.sel_col = Some(col);
    tab.sel_row_start = None;
    tab.sel_row_end = None;
    tab.sel_col_start = None;
    tab.sel_col_end = None;
    state.sync_edit_cell_from_selection();
}

#[test]
fn without_rowid_apply_tracks_changed_primary_keys_external_sources_and_foreign_keys() {
    let mut state = ledger();
    let original = state.browse().row_keys[0].clone();
    state.edit_value = "saved note 中文".into();
    state.apply_cell_draft().unwrap();
    assert_eq!(state.browse().sel_row, Some(0));
    assert_eq!(state.edit_value, "saved note 中文");
    assert_eq!(state.browse().row_keys[0], original);
    assert!(state.dirty);
    assert!(!state.has_cell_draft());
    select(&mut state, 0, 1);
    state.edit_value = "00042".into();
    state.apply_cell_draft().unwrap();
    assert_eq!(state.browse().sel_col, Some(1));
    assert_eq!(state.edit_value, "42");
    let key = state.browse().row_keys[state.browse().sel_row.unwrap()].clone();
    assert_eq!(key.values[1], db::CellValue::Integer(42));
    assert_eq!(state.edit_saved, rusqlite::types::Value::Integer(42));
    let row = state.browse().sel_row.unwrap();
    select(&mut state, row, 3);
    let path = state.begin_external_cell().unwrap();
    std::fs::write(&path, "from external").unwrap();
    let mut stale = key.clone();
    stale.columns = vec!["different".into(), "record".into(), "raw key".into()].into();
    state.browse_mut().row_keys[row] = stale;
    assert!(state.load_external_cell().unwrap_err().contains("changed"));
    assert!(path.is_file());
    state.browse_mut().row_keys[row] = key;
    state.load_external_cell().unwrap();
    assert!(!path.exists());
    state.apply_cell_draft().unwrap();
    assert_eq!(state.edit_value, "from external");
    let row = state.browse().sel_row.unwrap();
    select(&mut state, row, 0);
    state.navigate_fk().unwrap();
    assert_eq!(state.browse().table.as_deref(), Some("regions"));
    assert_eq!(state.browse().result.as_ref().unwrap().rows, [vec!["APAC"]]);
    assert!(!state.browse().row_keys.is_empty());
    state.switch_browse_tab(0).unwrap();
    select(&mut state, 0, 3);
    state.read_only = true;
    state.edit_value = "denied".into();
    assert!(state.apply_cell_draft().unwrap_err().contains("read-only"));
    assert_eq!(state.edit_value, "denied");
}

#[test]
fn without_rowid_rectangles_keep_types_and_rollback_stale_targets_and_constraints() {
    let mut state = ledger();
    state.browse_mut().sel_row_start = Some(1);
    let copy = state.copy_browse_cells(false, false).unwrap();
    let before = state
        .browse()
        .result
        .as_ref()
        .unwrap()
        .values
        .rectangle(0..=1, 3..=3)
        .unwrap();
    state.cut_browse_cells().unwrap();
    assert_eq!(
        state
            .browse()
            .result
            .as_ref()
            .unwrap()
            .values
            .rectangle(0..=1, 3..=3)
            .unwrap(),
        vec![vec![db::CellValue::Null]; 2]
    );
    select(&mut state, 0, 3);
    state
        .paste_browse_cells(copy.internal_cells(&copy.html).unwrap())
        .unwrap();
    assert_eq!(
        state
            .browse()
            .result
            .as_ref()
            .unwrap()
            .values
            .rectangle(0..=1, 3..=3)
            .unwrap(),
        before
    );
    select(&mut state, 0, 1);
    let original = state.browse().result.as_ref().unwrap().rows.clone();
    assert!(state.paste_browse_tsv("42\n42").is_err());
    assert_eq!(
        db::browse_table(
            state.conn.as_ref().unwrap(),
            "warehouse_ledger",
            &[],
            None,
            500,
            0
        )
        .unwrap()
        .rows,
        original
    );
    let second = state.browse().row_keys[1].clone();
    db::delete_row_in(
        state.conn.as_ref().unwrap(),
        "main",
        "warehouse_ledger",
        &second,
    )
    .unwrap();
    select(&mut state, 0, 3);
    assert!(state
        .paste_browse_tsv("rolled back\nstale")
        .unwrap_err()
        .contains("no longer exists"));
    let first = &state.browse().row_keys[0];
    assert_eq!(
        db::read_keyed_cell_in(
            state.conn.as_ref().unwrap(),
            "main",
            "warehouse_ledger",
            first,
            "note"
        )
        .unwrap(),
        before[0][0]
    );
    assert!(!state.conn.as_ref().unwrap().is_autocommit());
    state.discard_cell_draft();
    state.revert_changes().unwrap();
    assert_eq!(
        db::count_rows(state.conn.as_ref().unwrap(), "warehouse_ledger").unwrap(),
        18
    );
    assert_eq!(state.browse().row_keys.len(), 18);
}

#[test]
fn typed_key_ui_apply_delete_and_readonly_geometry() {
    let folder = std::env::var("NEXT_GUI_ROW_KEYS_QA_OUT")
        .ok()
        .map(PathBuf::from);
    if let Some(folder) = &folder {
        std::fs::create_dir_all(folder).unwrap();
    }
    for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
        for (width, height, size) in [(1400, 860, 13.0), (800, 600, 18.0)] {
            let mut demo = Demo::new(width, height, PresentBackend::Softbuffer);
            demo.state = ledger();
            demo.state.prefs.font_size = size;
            demo.theme = crate::ui_build::browser_theme(theme, size);
            demo.state.dock_focus = DockKind::Plot;
            demo.state.body_ratio = 0.45;
            demo.state.dock_ratio = 0.62;
            demo.state.sync_current_plot().unwrap();
            demo.state.plot.check(4, 1, true);
            demo.do_rebuild();
            let capture = |demo: &mut Demo, stage: &str| {
                if let Some(folder) = &folder {
                    crate::plot_export::capture(
                        demo,
                        folder,
                        &format!("row-keys-{stage}-{name}-{width}"),
                    );
                }
            };
            demo.ui.set_focus(Some(demo.ids.edit_value));
            demo.ui.key_input(UiKey::SelectAll);
            demo.ui.paste_into_focused_text("UI composite edit 中文");
            demo.snapshot_inputs();
            demo.do_rebuild();
            assert!(demo.state.has_cell_draft());
            capture(&mut demo, "draft");
            for id in [
                demo.ids.btn_edit_cell,
                demo.ids.btn_edit_discard,
                demo.ids.btn_cell_import,
                demo.ids.btn_cell_export,
                demo.ids.btn_cell_null,
            ] {
                demo.ui.set_focus(Some(id));
                demo.do_rebuild();
                assert_eq!(demo.ui.focused(), Some(id));
            }
            demo.ui.set_focus(Some(demo.ids.btn_edit_cell));
            demo.do_rebuild();
            assert_eq!(demo.ui.focused(), Some(demo.ids.btn_edit_cell));
            assert!(demo.ui.key_input(UiKey::Enter));
            assert_eq!(demo.ui.take_button_activate(), Some(demo.ids.btn_edit_cell));
            demo.snapshot_inputs();
            demo.handle_button(demo.ids.btn_edit_cell);
            demo.do_rebuild();
            assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
            assert_eq!(demo.state.edit_value, "UI composite edit 中文");
            assert_eq!(demo.state.browse().sel_col, Some(3));
            select(&mut demo.state, 0, 1);
            demo.state.edit_value = "42".into();
            demo.do_rebuild();
            demo.handle_button(demo.ids.btn_edit_cell);
            demo.do_rebuild();
            assert_eq!(demo.state.edit_value, "42");
            assert_eq!(
                demo.state.browse().row_keys[0].values[1],
                db::CellValue::Integer(42)
            );
            capture(&mut demo, "key-applied");
            demo.state.confirm_kind = ConfirmKind::DeleteRow;
            demo.state.confirm_open = true;
            demo.do_rebuild();
            capture(&mut demo, "delete");
            assert!(demo.ui.key_input(UiKey::Enter));
            demo.handle_modal();
            demo.do_rebuild();
            assert_eq!(
                db::count_rows(demo.state.conn.as_ref().unwrap(), "warehouse_ledger").unwrap(),
                17
            );
            assert!(demo.state.error_msg.is_none());
            demo.state.read_only = true;
            select(&mut demo.state, 0, 3);
            demo.do_rebuild();
            capture(&mut demo, "readonly");
            let key = demo.state.browse().row_keys[0].clone();
            demo.state.edit_value = "denied".into();
            demo.handle_button(demo.ids.btn_edit_cell);
            demo.do_rebuild();
            assert_eq!(
                db::read_keyed_cell_in(
                    demo.state.conn.as_ref().unwrap(),
                    "main",
                    "warehouse_ledger",
                    &key,
                    "note"
                )
                .unwrap(),
                db::CellValue::Text(b"Shipment 3".to_vec())
            );
        }
    }
}
