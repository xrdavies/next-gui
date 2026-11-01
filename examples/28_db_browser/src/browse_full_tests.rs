use crate::{
    browse_filter::tests::{capture, fixture},
    db,
    state::{AppState, MainTab},
    UiKey,
};
use next_gui_ui::Theme;

#[test]
fn complete_browse_keeps_typed_keys_across_the_old_page_boundary_and_reopens_saved_edits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("orders.sqlite");
    let mut state = AppState::empty();
    crate::desktop_workflow::prepare(&mut state, "desktop-browse").unwrap();
    state.save_as(path.to_str().unwrap()).unwrap();
    assert_eq!(state.browse().result.as_ref().unwrap().rows.len(), 500);
    assert_eq!(state.browse().result.as_ref().unwrap().values.len(), 100000);
    assert_eq!(state.browse().row_keys.len(), 100000);
    for record in [499, 500, 501, 75001, 100000] {
        state.goto_browse_record(record).unwrap();
        assert_eq!(state.browse().offset, 0);
        assert_eq!(state.browse().sel_row, Some(record - 1));
        assert_eq!(
            state.browse().row_keys[record - 1].values,
            [db::CellValue::Integer(record as i64)]
        );
    }
    state.begin_browse_cell_edit(75000, 9).unwrap();
    state.edit_value = "whole-table saved".into();
    state.apply_cell_draft().unwrap();
    state.write_changes().unwrap();
    let reader = db::open_readonly(path.to_str().unwrap()).unwrap();
    assert_eq!(
        reader
            .query_row("SELECT note FROM orders WHERE id=75001", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "whole-table saved"
    );
    state.browse_mut().filter = "status='paid'".into();
    state.refresh_browse().unwrap();
    assert_eq!(state.browse().result.as_ref().unwrap().values.len(), 25000);
    state.goto_browse_record(usize::MAX).unwrap();
    assert_eq!(state.browse().sel_row, Some(24999));
}

#[test]
fn desktop_complete_grid_reaches_last_row_and_preserves_global_rectangles_and_editing() {
    for (name, theme, width, height) in [
        ("dark", Theme::dark(), 1600, 900),
        ("light", Theme::light(), 1920, 927),
    ] {
        let mut demo = fixture(width, height, theme);
        demo.state.write_changes().unwrap();
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(id INTEGER PRIMARY KEY,name TEXT NOT NULL);WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<100000) INSERT INTO records SELECT x,printf('Record %06d',x) FROM n;").unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state
            .switch_browse_table_in("main", "records")
            .unwrap();
        demo.state.tab = MainTab::Browse;
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.browse_table));
        demo.ui.key_input(UiKey::End);
        demo.snapshot_inputs();
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!(demo.state.browse().sel_row, Some(99999));
        assert_eq!(
            demo.ui.table_selected_row(demo.ids.browse_table),
            Some(99999)
        );
        assert!(
            demo.ui
                .table_scroll_offset(demo.ids.browse_table)
                .unwrap()
                .1
                > 2000000.0
        );
        capture(&mut demo, &format!("browse-complete-end-{name}-{width}"));
        demo.ui
            .set_table_selected_cells(demo.ids.browse_table, Some((501, 499)), Some((1, 0)));
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!(
            demo.ui.table_selected_rows(demo.ids.browse_table),
            Some((499, 501))
        );
        assert_eq!(demo.ui.table_selected_row(demo.ids.browse_table), Some(499));
        assert!(demo
            .ui
            .table_selected_text(demo.ids.browse_table)
            .unwrap()
            .contains("502\tRecord 000502"));
        demo.state.goto_browse_record(75001).unwrap();
        demo.reveal_browse_selection = true;
        demo.do_rebuild();
        assert_eq!(
            demo.ui.table_selected_row(demo.ids.browse_table),
            Some(75000)
        );
        capture(&mut demo, &format!("browse-complete-goto-{name}-{width}"));
    }
}

#[test]
fn full_browse_edits_composite_raw_keys_and_unlocked_view_rows_past_the_preview() {
    let mut state = AppState::empty();
    state.new_memory_db().unwrap();
    state.conn.as_ref().unwrap().execute_batch("ATTACH ':memory:' AS \"aux.名\"; CREATE TABLE \"aux.名\".\"raw.table\"(k,n INTEGER,payload,derived AS(n*2),PRIMARY KEY(k,n)) WITHOUT ROWID; WITH RECURSIVE seq(n) AS(VALUES(1) UNION ALL SELECT n+1 FROM seq WHERE n<605) INSERT INTO \"aux.名\".\"raw.table\"(k,n,payload) SELECT CASE n%3 WHEN 0 THEN CAST(X'FF00' AS TEXT) WHEN 1 THEN X'00FF' ELSE 1 END,n,NULL FROM seq; CREATE VIEW \"aux.名\".visible AS SELECT n,payload FROM \"raw.table\"; CREATE TRIGGER \"aux.名\".edit_visible INSTEAD OF UPDATE ON visible BEGIN UPDATE \"raw.table\" SET payload=NEW.payload WHERE n=OLD.n; END;").unwrap();
    state.refresh_schema().unwrap();
    state.switch_browse_table_in("aux.名", "raw.table").unwrap();
    state
        .set_browse_ordering(vec![db::OrderBy {
            column: "n".into(),
            descending: false,
        }])
        .unwrap();
    let identity = state.browse().result.as_ref().unwrap().values.identity();
    state.goto_browse_record(600).unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &identity,
        &state.browse().result.as_ref().unwrap().values.identity()
    ));
    assert_eq!(
        state.browse().row_keys[599].values,
        [
            db::CellValue::Text(vec![255, 0]),
            db::CellValue::Integer(600)
        ]
    );
    state.browse_mut().sel_col = Some(2);
    state
        .paste_browse_cells(vec![vec![db::CellValue::Blob(vec![0, 255, 7])]])
        .unwrap();
    assert_eq!(
        state
            .conn
            .as_ref()
            .unwrap()
            .query_row(
                "SELECT payload FROM \"aux.名\".\"raw.table\" WHERE n=600",
                [],
                |row| row.get::<_, Vec<u8>>(0)
            )
            .unwrap(),
        [0, 255, 7]
    );
    assert!(state
        .begin_browse_cell_edit(599, 3)
        .unwrap_err()
        .contains("read-only"));
    state.discard_cell_draft();
    state.switch_browse_table_in("aux.名", "visible").unwrap();
    state.browse_mut().unlock_view_pk = Some("n".into());
    state
        .set_browse_ordering(vec![db::OrderBy {
            column: "n".into(),
            descending: true,
        }])
        .unwrap();
    state.goto_browse_record(601).unwrap();
    assert_eq!(
        state.browse().row_keys[600].values,
        [db::CellValue::Integer(5)]
    );
    state.browse_mut().sel_col = Some(1);
    state
        .paste_browse_cells(vec![vec![db::CellValue::Text(vec![255, 0, 128])]])
        .unwrap();
    assert_eq!(
        state
            .conn
            .as_ref()
            .unwrap()
            .query_row(
                "SELECT hex(payload) FROM \"aux.名\".\"raw.table\" WHERE n=5",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "FF0080"
    );
    state.goto_browse_record(600).unwrap();
    state.browse_mut().sel_col = Some(1);
    state
        .paste_browse_cells(vec![vec![db::CellValue::Null]])
        .unwrap();
    assert!(state
        .conn
        .as_ref()
        .unwrap()
        .query_row(
            "SELECT payload IS NULL FROM \"aux.名\".\"raw.table\" WHERE n=6",
            [],
            |row| row.get::<_, bool>(0)
        )
        .unwrap());
}

fn settle_browse(demo: &mut crate::Demo) {
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + Duration::from_secs(20);
    while demo.state.browse_work.is_some() {
        assert!(Instant::now() < deadline);
        demo.advance_browse_filter(Instant::now());
        if demo.rebuild {
            demo.do_rebuild();
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
}

#[test]
fn desktop_open_sort_and_filter_load_complete_rows_on_the_existing_worker() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("orders.sqlite");
    let mut writer = AppState::empty();
    crate::desktop_workflow::prepare(&mut writer, "desktop-browse").unwrap();
    writer.save_as(path.to_str().unwrap()).unwrap();
    let mut demo = fixture(1600, 900, Theme::dark());
    demo.state.write_changes().unwrap();
    demo.state.browse_background = true;
    demo.state.open_path(path.to_str().unwrap()).unwrap();
    assert!(demo.state.browse().result.is_none());
    assert!(demo.state.browse_work.is_none());
    demo.state.tab = MainTab::Browse;
    demo.do_rebuild();
    demo.advance_browse_filter(std::time::Instant::now());
    assert!(demo.state.browse_work.is_some());
    assert!(demo.state.conn.is_none());
    settle_browse(&mut demo);
    demo.state.switch_browse_table_in("main", "orders").unwrap();
    assert!(demo.state.browse_work.is_some());
    settle_browse(&mut demo);
    let identity = demo
        .state
        .browse()
        .result
        .as_ref()
        .unwrap()
        .values
        .identity();
    demo.state.sort_browse_column("id".into(), false).unwrap();
    assert!(demo.state.browse_work.is_some());
    assert!(std::sync::Arc::ptr_eq(
        &identity,
        &demo
            .state
            .browse()
            .result
            .as_ref()
            .unwrap()
            .values
            .identity()
    ));
    settle_browse(&mut demo);
    assert_eq!(demo.state.browse().sel_row, Some(0));
    assert_eq!(demo.state.browse().sel_col, Some(0));
    demo.state.sort_browse_column("id".into(), false).unwrap();
    settle_browse(&mut demo);
    assert_eq!(
        demo.state.browse().row_keys[0].values,
        [db::CellValue::Integer(100000)]
    );
    assert_eq!(
        demo.state.browse().result.as_ref().unwrap().values.len(),
        100000
    );
    demo.ui
        .set_text_input(demo.ids.browse_filter, "status='paid'");
    demo.handle_button(demo.ids.btn_browse_go);
    assert!(demo.state.browse_work.is_some());
    settle_browse(&mut demo);
    assert_eq!(
        demo.state.browse().result.as_ref().unwrap().values.len(),
        25000
    );
    demo.state.goto_browse_record(usize::MAX).unwrap();
    assert_eq!(
        demo.state.browse().row_keys[24999].values,
        [db::CellValue::Integer(1)]
    );
    demo.state
        .set_browse_ordering(vec![db::OrderBy {
            column: "missing".into(),
            descending: false,
        }])
        .unwrap();
    while demo.state.browse_work.is_some() {
        demo.advance_browse_filter(std::time::Instant::now());
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(demo.state.error_msg.is_some());
    assert_eq!(
        demo.state.browse().row_keys[24999].values,
        [db::CellValue::Integer(1)]
    );
    assert_eq!(demo.state.browse().ordering()[0].column, "id");
    demo.state.error_msg = None;
    demo.state.tab = MainTab::Structure;
    let index = demo
        .state
        .schema
        .iter()
        .position(|object| object.schema == "main" && object.name == "products")
        .unwrap();
    demo.state.select_schema_node(10000 + index as u64);
    assert!(
        demo.state.browse_work.is_none(),
        "Structure navigation must not load the entire table"
    );
    assert_eq!(demo.state.browse().table.as_deref(), Some("products"));
    assert!(demo.state.browse().result.is_none());
    demo.state.tab = MainTab::Browse;
    demo.do_rebuild();
    demo.advance_browse_filter(std::time::Instant::now());
    assert!(demo.state.browse_work.is_some());
    settle_browse(&mut demo);
    assert_eq!(
        demo.state.browse().result.as_ref().unwrap().values.len(),
        1000
    );
}

#[test]
fn cancelling_desktop_table_load_keeps_the_previous_grid_and_unwritten_database_changes() {
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };
    let mut demo = fixture(1600, 900, Theme::dark());
    demo.state.write_changes().unwrap();
    crate::desktop_workflow::prepare(&mut demo.state, "desktop-browse").unwrap();
    demo.state.browse_background = true;
    let (entered, waiting) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let mut first = true;
    demo.state
        .conn
        .as_ref()
        .unwrap()
        .create_scalar_function(
            "browse_gate",
            1,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            move |context| {
                if first {
                    first = false;
                    entered.send(()).unwrap();
                    resume.recv_timeout(Duration::from_secs(5)).unwrap();
                }
                context.get::<i64>(0)
            },
        )
        .unwrap();
    demo.state.conn.as_ref().unwrap().execute_batch("CREATE VIEW gated AS SELECT id,browse_gate(id) AS gated FROM orders; UPDATE orders SET note='pending before load' WHERE id=100000;").unwrap();
    demo.state.dirty = true;
    demo.state.refresh_schema().unwrap();
    let identity = demo
        .state
        .browse()
        .result
        .as_ref()
        .unwrap()
        .values
        .identity();
    demo.do_rebuild();
    demo.state.switch_browse_table_in("main", "gated").unwrap();
    waiting.recv_timeout(Duration::from_secs(5)).unwrap();
    demo.do_rebuild();
    assert_eq!(demo.state.browse().table.as_deref(), Some("orders"));
    assert_eq!(
        demo.ui.button_enabled(demo.ids.btn_browse_cancel),
        Some(true)
    );
    release.send(()).unwrap();
    demo.handle_button(demo.ids.btn_browse_cancel);
    assert!(demo.state.browse_work.is_none());
    assert!(std::sync::Arc::ptr_eq(
        &identity,
        &demo
            .state
            .browse()
            .result
            .as_ref()
            .unwrap()
            .values
            .identity()
    ));
    assert!(demo.state.dirty);
    assert!(!demo.state.conn.as_ref().unwrap().is_autocommit());
    assert_eq!(
        demo.state
            .conn
            .as_ref()
            .unwrap()
            .query_row("SELECT note FROM orders WHERE id=100000", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        "pending before load"
    );
    demo.do_rebuild();
    demo.advance_browse_filter(Instant::now() + Duration::from_secs(2));
    assert!(
        demo.state.browse_work.is_none(),
        "Cancel must not restart the cancelled load"
    );
}
