use super::*;

fn demo() -> Demo {
    let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap();
    demo.state
        .conn
        .as_ref()
        .unwrap()
        .execute_batch(
            "CREATE TABLE t(n INTEGER, label TEXT);
         WITH RECURSIVE seq(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM seq WHERE n<1205)
         INSERT INTO t SELECT n, 'row '||n FROM seq;",
        )
        .unwrap();
    demo.state.refresh_schema().unwrap();
    let tab = demo.state.browse_mut();
    tab.table = Some("t".into());
    tab.filter = "n%2=1".into();
    tab.order_col = Some("n".into());
    tab.order_desc = true;
    demo.state.refresh_browse().unwrap();
    demo.state.write_changes().unwrap();
    demo.state.browse_mut().sel_row = Some(0);
    demo.state.browse_mut().sel_col = Some(1);
    demo.state.sync_edit_cell_from_selection();
    demo.state.tab = MainTab::Browse;
    demo.do_rebuild();
    demo
}

fn selected(demo: &Demo) -> (usize, usize, &str) {
    let tab = demo.state.browse();
    let row = tab.sel_row.unwrap();
    (tab.offset, row, &tab.result.as_ref().unwrap().rows[row][0])
}

fn go(demo: &mut Demo, value: &str) {
    demo.ui.set_text_input(demo.ids.record_number, value);
    demo.handle_button(demo.ids.btn_record_go);
    demo.do_rebuild();
}

#[test]
fn jump_uses_filtered_sorted_record_numbers_and_preserves_the_column() {
    let mut demo = demo();
    assert_eq!(demo.state.browse().total_rows, Some(603));
    for (record, expected) in [
        (501, (500, 0, "205")),
        (603, (500, 102, "1")),
        (usize::MAX, (500, 102, "1")),
        (0, (0, 0, "1205")),
    ] {
        demo.state.goto_browse_record(record).unwrap();
        assert_eq!(selected(&demo), expected);
        assert_eq!(demo.state.browse().sel_col, Some(1));
        assert_eq!(demo.state.edit_value, format!("row {}", expected.2));
        assert!(!demo.state.has_cell_draft());
        assert!(!demo.state.dirty);
    }
    assert_eq!(demo.state.browse().goto_record, "1");
    // The navigation input is transient; legacy projects need no new field.
    let json = serde_json::to_value(demo.state.browse()).unwrap();
    assert!(json.get("goto_record").is_none());
    let restored: state::BrowseTabState = serde_json::from_value(json).unwrap();
    assert!(restored.goto_record.is_empty());
}

#[test]
fn jump_guards_drafts_busy_queries_and_restores_selection_on_query_failure() {
    let mut demo = demo();
    demo.state.goto_browse_record(510).unwrap();
    let old_rows = demo.state.browse().result.as_ref().unwrap().rows.clone();
    let old_rowids = demo.state.browse().rowids.clone();
    demo.state.edit_value = "keep this draft".into();
    assert!(demo
        .state
        .goto_browse_record(1)
        .unwrap_err()
        .contains("draft"));
    assert_eq!(selected(&demo), (500, 9, "187"));
    assert_eq!(demo.state.edit_value, "keep this draft");
    demo.state.discard_cell_draft();
    let old_editor = demo.state.edit_value.clone();
    demo.state.sql_busy = true;
    assert!(demo
        .state
        .goto_browse_record(1)
        .unwrap_err()
        .contains("Wait"));
    demo.state.sql_busy = false;
    demo.state.browse_mut().filter = "missing_function()".into();
    assert!(demo.state.goto_browse_record(1).is_err());
    assert_eq!(selected(&demo), (500, 9, "187"));
    assert_eq!(demo.state.browse().result.as_ref().unwrap().rows, old_rows);
    assert_eq!(demo.state.browse().rowids, old_rowids);
    assert_eq!(demo.state.edit_value, old_editor);
    assert_eq!(demo.state.browse().goto_record, "510");
    assert!(!demo.state.dirty);
}

#[test]
fn controls_normalize_numbers_report_invalid_input_and_reveal_the_target_cell() {
    let mut demo = demo();
    assert_eq!(
        demo.ui.button_enabled(demo.ids.btn_record_first),
        Some(false)
    );
    demo.handle_button(demo.ids.btn_record_last);
    demo.do_rebuild();
    assert_eq!(selected(&demo), (500, 102, "1"));
    assert_eq!(
        demo.ui.text_input_value(demo.ids.record_number),
        Some("603")
    );
    assert_eq!(
        demo.ui.button_enabled(demo.ids.btn_record_last),
        Some(false)
    );
    assert_eq!(demo.ui.focused(), Some(demo.ids.browse_table));
    assert!(
        demo.ui
            .table_scroll_offset(demo.ids.browse_table)
            .unwrap()
            .1
            > 0.0
    );
    demo.handle_button(demo.ids.btn_record_first);
    demo.do_rebuild();
    assert_eq!(selected(&demo), (0, 0, "1205"));
    assert_eq!(
        demo.ui
            .table_scroll_offset(demo.ids.browse_table)
            .unwrap()
            .1,
        0.0
    );
    go(&mut demo, " 150 ");
    assert_eq!(selected(&demo), (0, 149, "907"));
    assert_eq!(
        demo.ui.text_input_value(demo.ids.record_number),
        Some("150")
    );
    let scroll = demo.ui.table_scroll_offset(demo.ids.browse_table).unwrap();
    assert!(scroll.1 > 0.0);
    for invalid in ["1.5", "", "a"] {
        go(&mut demo, invalid);
        assert_eq!(
            demo.ui.text_input_value(demo.ids.record_number),
            Some(invalid)
        );
        assert_eq!(selected(&demo), (0, 149, "907"));
        assert!(demo
            .state
            .error_msg
            .as_ref()
            .unwrap()
            .contains("whole record"));
        assert_eq!(
            demo.ui.table_scroll_offset(demo.ids.browse_table),
            Some(scroll)
        );
    }
    go(&mut demo, "-4");
    assert_eq!(selected(&demo), (0, 0, "1205"));
    go(&mut demo, "999999999999999999999999999999");
    assert_eq!(selected(&demo), (500, 102, "1"));
    assert!(demo.state.error_msg.is_none());
    if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
        demo.scene
            .mark_full_dirty(demo.win_w as f32, demo.win_h as f32);
        let dirty = demo.scene.take_dirty();
        let mut renderer = SoftRenderer::new(demo.win_w, demo.win_h, clear_color(&demo.theme));
        renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
        write_ppm(
            &path,
            renderer.frame_pixels().unwrap().0,
            demo.win_w,
            demo.win_h,
        )
        .unwrap();
    }
}

#[test]
fn shrinking_results_and_empty_views_clamp_navigation_without_a_stale_editor() {
    let mut demo = demo();
    demo.state.goto_browse_record(501).unwrap();
    demo.state
        .conn
        .as_ref()
        .unwrap()
        .execute("DELETE FROM t WHERE n>5", [])
        .unwrap();
    demo.state.goto_browse_record(usize::MAX).unwrap();
    assert_eq!(demo.state.browse().total_rows, Some(3));
    assert_eq!(selected(&demo), (0, 2, "1"));
    demo.state
        .conn
        .as_ref()
        .unwrap()
        .execute_batch("CREATE VIEW v AS SELECT * FROM t")
        .unwrap();
    demo.state.refresh_schema().unwrap();
    demo.state.browse_mut().table = Some("v".into());
    demo.state.read_only = true;
    demo.state.goto_browse_record(2).unwrap();
    assert_eq!(selected(&demo), (0, 1, "3"));
    assert!(demo.state.browse().rowids.is_empty());
    demo.state
        .conn
        .as_ref()
        .unwrap()
        .execute("DELETE FROM t", [])
        .unwrap();
    demo.state.goto_browse_record(1).unwrap();
    assert_eq!(demo.state.browse().total_rows, Some(0));
    assert_eq!(demo.state.browse().sel_row, None);
    assert_eq!(demo.state.browse().sel_col, None);
    assert_eq!(demo.state.browse().goto_record, "0");
    assert!(demo.state.edit_col.is_empty());
    assert!(demo.state.edit_value.is_empty());
    demo.do_rebuild();
    for id in [
        demo.ids.btn_record_first,
        demo.ids.btn_record_last,
        demo.ids.btn_record_go,
    ] {
        assert_eq!(demo.ui.button_enabled(id), Some(false));
    }
}

#[test]
fn narrow_window_reveals_the_navigation_input_when_focused() {
    let mut demo = demo();
    demo.win_w = 800;
    demo.win_h = 600;
    demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.record_number));
    demo.ui.sync(&mut demo.scene);
    let bounds = demo
        .scene
        .iter_paint_order()
        .into_iter()
        .find(|node| demo.ui.widget_from_node(node.id) == Some(demo.ids.record_number))
        .unwrap()
        .bounds;
    assert!(
        bounds.x >= 0.0 && bounds.x + bounds.width <= 800.0,
        "{bounds:?}"
    );
    assert!(
        bounds.y >= 0.0 && bounds.y + bounds.height <= 600.0,
        "{bounds:?}"
    );
    let x = bounds.x + bounds.width / 2.0;
    let y = bounds.y + bounds.height / 2.0;
    assert_eq!(
        demo.ui.hit_widget(&demo.scene, x, y),
        Some(demo.ids.record_number)
    );
    for id in [
        demo.ids.btn_record_go,
        demo.ids.btn_record_next,
        demo.ids.btn_record_last,
    ] {
        demo.ui.set_focus(Some(id));
        demo.ui.sync(&mut demo.scene);
        let bounds = demo
            .scene
            .iter_paint_order()
            .into_iter()
            .find(|node| demo.ui.widget_from_node(node.id) == Some(id))
            .unwrap()
            .bounds;
        assert!(
            bounds.x >= 0.0 && bounds.x + bounds.width <= 800.0,
            "{bounds:?}"
        );
        assert!(
            bounds.y >= 0.0 && bounds.y + bounds.height <= 600.0,
            "{bounds:?}"
        );
        assert_eq!(
            demo.ui
                .hit_widget(&demo.scene, bounds.x + 5.0, bounds.y + 5.0),
            Some(id)
        );
    }
    demo.ui.set_focus(Some(demo.ids.record_number));
    demo.ui.sync(&mut demo.scene);
    if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
        demo.scene.mark_full_dirty(800.0, 600.0);
        let dirty = demo.scene.take_dirty();
        let mut renderer = SoftRenderer::new(800, 600, clear_color(&demo.theme));
        renderer.render(&encode_scene(&demo.scene, &dirty), &dirty);
        write_ppm(
            &PathBuf::from(path)
                .with_extension("narrow.ppm")
                .to_string_lossy(),
            renderer.frame_pixels().unwrap().0,
            800,
            600,
        )
        .unwrap();
    }
}

#[test]
fn previous_next_step_by_the_live_viewport_and_cross_cache_boundaries() {
    let mut demo = demo();
    go(&mut demo, "499");
    let step = demo.ui.table_visible_rows(demo.ids.browse_table).unwrap() - 1;
    assert!(step > 2 && step < 500);
    demo.handle_button(demo.ids.btn_record_next);
    demo.do_rebuild();
    assert_eq!(demo.state.browse().goto_record, (499 + step).to_string());
    assert_eq!(demo.state.browse().offset, 500);
    assert_eq!(demo.state.browse().sel_col, Some(1));
    demo.handle_button(demo.ids.btn_record_prev);
    demo.do_rebuild();
    assert_eq!(demo.state.browse().goto_record, "499");
    assert_eq!(demo.state.browse().offset, 0);
    demo.win_h = 680;
    demo.do_rebuild();
    let smaller_step = demo.ui.table_visible_rows(demo.ids.browse_table).unwrap() - 1;
    assert!(smaller_step < step);
    demo.handle_button(demo.ids.btn_record_prev);
    demo.do_rebuild();
    assert_eq!(
        demo.state.browse().goto_record,
        (499 - smaller_step).to_string()
    );
    go(&mut demo, "2");
    demo.handle_button(demo.ids.btn_record_prev);
    demo.do_rebuild();
    assert_eq!(selected(&demo), (0, 0, "1205"));
    assert_eq!(
        demo.ui.button_enabled(demo.ids.btn_record_prev),
        Some(false)
    );
    demo.state.toggle_browse_view();
    demo.do_rebuild();
    demo.handle_button(demo.ids.btn_record_next);
    demo.do_rebuild();
    assert_eq!(selected(&demo), (0, 1, "1203"));
    demo.handle_button(demo.ids.btn_record_prev);
    demo.do_rebuild();
    assert_eq!(selected(&demo), (0, 0, "1205"));
}

#[test]
fn previous_next_keep_drafts_and_failed_query_selection_intact() {
    let mut demo = demo();
    demo.ui
        .set_text_area_value(demo.ids.edit_value, "keep draft");
    demo.handle_button(demo.ids.btn_record_next);
    demo.do_rebuild();
    assert_eq!(selected(&demo), (0, 0, "1205"));
    assert_eq!(demo.state.edit_value, "keep draft");
    assert!(demo.state.error_msg.as_ref().unwrap().contains("draft"));
    demo.handle_button(demo.ids.btn_edit_discard);
    demo.do_rebuild();
    demo.ui
        .set_text_input(demo.ids.browse_filter, "missing_function()");
    demo.handle_button(demo.ids.btn_record_next);
    demo.do_rebuild();
    assert_eq!(selected(&demo), (0, 0, "1205"));
    assert_eq!(demo.state.edit_value, "row 1205");
    assert!(demo.state.error_msg.is_some());
    assert!(!demo.state.dirty);
    demo.ui.set_text_input(demo.ids.browse_filter, "n%2=1");
    demo.handle_button(demo.ids.btn_browse_go);
    demo.do_rebuild();
    assert_eq!(demo.state.browse().sel_row, None);
    demo.handle_button(demo.ids.btn_record_next);
    demo.do_rebuild();
    assert_eq!(selected(&demo), (0, 0, "1205"));
    assert!(demo.state.error_msg.is_none());
}
