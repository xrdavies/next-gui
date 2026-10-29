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
    let old_rowids = demo.state.browse().row_keys.clone();
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
    assert_eq!(demo.state.browse().row_keys, old_rowids);
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
    assert!(demo.state.browse().row_keys.is_empty());
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

#[test]
fn page_shortcuts_cross_cache_and_command_home_end_reach_the_complete_result() {
    let mut demo = demo();
    go(&mut demo, "499");
    let plain = Modifiers::default();
    let command = Modifiers {
        ctrl: true,
        ..plain
    };
    assert!(demo.handle_browse_navigation_key(KeyCode::PageDown, plain));
    demo.do_rebuild();
    assert_eq!(demo.state.browse().offset, 500);
    assert!(demo.handle_browse_navigation_key(KeyCode::PageUp, plain));
    demo.do_rebuild();
    assert_eq!(demo.state.browse().goto_record, "499");
    assert!(demo.handle_browse_navigation_key(KeyCode::Home, command));
    demo.do_rebuild();
    assert_eq!(selected(&demo), (0, 0, "1205"));
    assert!(demo.handle_browse_navigation_key(KeyCode::End, command));
    demo.do_rebuild();
    assert_eq!(selected(&demo), (500, 102, "1"));
    assert!(demo.handle_browse_navigation_key(KeyCode::PageDown, plain));
    assert!(!demo.rebuild, "disabled end action does not rebuild");
    for modifiers in [
        Modifiers {
            shift: true,
            ..plain
        },
        Modifiers { alt: true, ..plain },
    ] {
        assert!(!demo.handle_browse_navigation_key(KeyCode::PageUp, modifiers));
    }
    assert_eq!(map_key(KeyCode::PageUp, plain), Some(UiKey::PageUp));
    assert_eq!(
        map_key(
            KeyCode::PageDown,
            Modifiers {
                shift: true,
                ..plain
            }
        ),
        Some(UiKey::SelectPageDown)
    );
    demo.ui.set_focus(Some(demo.ids.record_number));
    assert!(!demo.handle_browse_navigation_key(KeyCode::PageUp, plain));
    demo.ui.set_focus(Some(demo.ids.browse_table));
    demo.state.confirm_open = true;
    assert!(!demo.handle_browse_navigation_key(KeyCode::PageUp, plain));
    demo.state.confirm_open = false;
    demo.state.overlay = Some(OverlayKind::ImportCsv);
    assert!(!demo.handle_browse_navigation_key(KeyCode::PageUp, plain));
    demo.state.overlay = None;
    demo.ui
        .set_text_area_value(demo.ids.edit_value, "retain shortcut draft");
    assert!(demo.handle_browse_navigation_key(KeyCode::PageUp, plain));
    assert_eq!(selected(&demo), (500, 102, "1"));
    assert_eq!(demo.state.edit_value, "retain shortcut draft");
    assert!(demo.state.error_msg.as_ref().unwrap().contains("draft"));
}

#[test]
fn selecting_an_unqueryable_view_restores_the_previous_table_result_and_editor() {
    let mut demo = demo();
    demo.state
        .conn
        .as_ref()
        .unwrap()
        .execute_batch("CREATE VIEW broken AS SELECT n,label FROM t WHERE missing_function()")
        .unwrap();
    demo.state.refresh_schema().unwrap();
    go(&mut demo, "501");
    let names: Vec<_> = demo.state.browse_tables().iter().map(|(_, name)| name.to_string()).collect();
    let index = names.iter().position(|name| name == "broken").unwrap();
    demo.ui
        .set_select_selected(demo.ids.browse_select, Some(index));
    demo.snapshot_inputs();
    demo.poll_selection_changes();
    demo.do_rebuild();
    assert_eq!(demo.state.browse().table.as_deref(), Some("t"));
    assert_eq!(selected(&demo), (500, 0, "205"));
    assert_eq!(demo.state.edit_value, "row 205");
    assert!(demo.state.error_msg.is_some());
    assert_eq!(
        demo.ui.select_selected(demo.ids.browse_select),
        names.iter().position(|name| name == "t")
    );
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
fn command_page_shortcuts_cycle_tables_and_views_in_both_directions() {
    let mut demo = demo();
    demo.state
        .conn
        .as_ref()
        .unwrap()
        .execute_batch(
            "CREATE TABLE u(other TEXT); INSERT INTO u VALUES('u');
         CREATE VIEW v AS SELECT * FROM t WHERE n=1",
        )
        .unwrap();
    demo.state.refresh_schema().unwrap();
    demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.browse_table));
    let names: Vec<_> = demo.state.browse_tables().iter().map(|(_, name)| name.to_string()).collect();
    assert_eq!(names.len(), 3);
    let start = names.iter().position(|name| name == "t").unwrap();
    let command = Modifiers {
        ctrl: true,
        ..Modifiers::default()
    };
    for next in [true, false] {
        for step in 1..=names.len() {
            let key = if next {
                KeyCode::PageDown
            } else {
                KeyCode::PageUp
            };
            assert!(demo.handle_browse_navigation_key(key, command));
            demo.do_rebuild();
            let index = if next {
                (start + step) % names.len()
            } else {
                (start + names.len() - step) % names.len()
            };
            assert_eq!(
                demo.state.browse().table.as_deref(),
                Some(names[index].as_str())
            );
            assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
            assert_eq!(demo.state.browse().sel_row, None);
            assert!(demo.state.edit_col.is_empty());
            if names[index] == "t" {
                assert_eq!(demo.state.browse().ordering()[0].column, "n");
            } else {
                assert!(demo.state.browse().ordering().is_empty());
            }
            assert_eq!(demo.state.browse().offset, 0);
            assert_eq!(demo.ui.focused(), Some(demo.ids.browse_table));
        }
    }
}

#[test]
fn table_switch_apply_guards_drafts_and_busy_queries_without_mutating_the_tab() {
    let mut demo = demo();
    demo.state
        .conn
        .as_ref()
        .unwrap()
        .execute_batch("CREATE TABLE u(n,label)")
        .unwrap();
    demo.state.refresh_schema().unwrap();
    go(&mut demo, "501");
    let names: Vec<_> = demo.state.browse_tables().iter().map(|(_, name)| name.to_string()).collect();
    let target = names.iter().position(|name| name == "u").unwrap();
    demo.ui
        .set_text_area_value(demo.ids.edit_value, "keep table draft");
    demo.ui
        .set_select_selected(demo.ids.browse_select, Some(target));
    demo.handle_button(demo.ids.btn_browse_go);
    demo.do_rebuild();
    assert_eq!(demo.state.browse().table.as_deref(), Some("t"));
    assert_eq!(selected(&demo), (500, 0, "205"));
    assert_eq!(demo.state.edit_value, "keep table draft");
    assert!(demo.state.error_msg.as_ref().unwrap().contains("draft"));
    demo.handle_button(demo.ids.btn_edit_discard);
    demo.do_rebuild();
    demo.state.sql_busy = true;
    assert!(demo
        .state
        .switch_browse_table("u")
        .unwrap_err()
        .contains("Wait"));
    demo.state.sql_busy = false;
    assert!(demo.state.switch_browse_table("missing").is_err());
    assert_eq!(selected(&demo), (500, 0, "205"));
    assert_eq!(demo.state.edit_value, "row 205");
    assert!(!demo.state.dirty);
}

#[test]
fn per_table_filters_sort_and_widths_restore_in_each_tab_and_project_roundtrip() {
    let mut demo = demo();
    demo.state
        .conn
        .as_ref()
        .unwrap()
        .execute_batch("CREATE TABLE u(other TEXT); INSERT INTO u VALUES('u')")
        .unwrap();
    demo.state.refresh_schema().unwrap();
    let options = state::BrowseTableOptions {
        column_names: vec!["n".into(), "label".into()],
        filter: "n%2=1".into(),
        global_filters: vec!["row".into()],
        col_filters: vec!["<=999".into(), String::new()],
        ordering: vec![
            db::OrderBy {
                column: "n".into(),
                descending: true,
            },
            db::OrderBy {
                column: "label".into(),
                descending: false,
            },
        ],
        column_widths: vec![147.0, 231.0],
        frozen_cols: Some(0),
        unlock_view_pk: None,
        show_row_id: false,
        rowid_filter: String::new(),
    };
    demo.state.browse_mut().set_options(options.clone());
    demo.state.refresh_browse().unwrap();
    demo.state.switch_browse_table("u").unwrap();
    assert!(demo.state.browse().filter.is_empty());
    assert!(demo.state.browse().global_filters.is_empty());
    assert!(demo.state.browse().ordering().is_empty());
    assert!(demo.state.browse().column_widths.is_empty());
    assert_eq!(demo.state.browse().table_options["t"], options);
    demo.state.browse_mut().filter = "other='u'".into();
    demo.state.refresh_browse().unwrap();
    let project = project::ProjectFile::from_state(&demo.state);
    let json = serde_json::to_string(&project).unwrap();
    let xml = project_xml::encode(&project, std::path::Path::new("options.sqbpro")).unwrap();
    for restored in [
        serde_json::from_str::<project::ProjectFile>(&json).unwrap(),
        project_xml::decode(&xml).unwrap(),
    ] {
        restored.apply_to(&mut demo.state);
        demo.state.switch_browse_table("t").unwrap();
        assert_eq!(demo.state.browse().options(), options);
        assert_eq!(
            demo.state.browse().result.as_ref().unwrap().rows[0][0],
            "999"
        );
        assert_eq!(demo.state.browse().sel_row, None);
        assert!(demo.state.edit_col.is_empty());
        demo.state.switch_browse_table("u").unwrap();
        assert_eq!(demo.state.browse().filter, "other='u'");
        assert_eq!(demo.state.browse().result.as_ref().unwrap().rows[0][0], "u");
    }
    demo.state.add_browse_tab().unwrap();
    demo.state.switch_browse_table("t").unwrap();
    assert!(
        demo.state.browse().filter.is_empty(),
        "a new browse tab has independent options"
    );
    demo.state.switch_browse_tab(0).unwrap();
    demo.state.switch_browse_table("t").unwrap();
    assert_eq!(demo.state.browse().options(), options);
    if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
        demo.state.browse_mut().sel_row = Some(0);
        demo.state.browse_mut().sel_col = Some(1);
        demo.state.sync_edit_cell_from_selection();
        demo.do_rebuild();
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
fn invalid_saved_table_options_do_not_replace_the_current_result() {
    let mut demo = demo();
    demo.state
        .conn
        .as_ref()
        .unwrap()
        .execute_batch("CREATE TABLE u(other)")
        .unwrap();
    demo.state.refresh_schema().unwrap();
    demo.state.goto_browse_record(501).unwrap();
    for options in [
        state::BrowseTableOptions {
            column_widths: vec![-1.0],
            ..Default::default()
        },
        state::BrowseTableOptions {
            filter: "missing_function()".into(),
            ..Default::default()
        },
    ] {
        demo.state
            .browse_mut()
            .table_options
            .insert("u".into(), options.clone());
        assert!(demo.state.switch_browse_table("u").is_err());
        assert_eq!(demo.state.browse().table.as_deref(), Some("t"));
        assert_eq!(selected(&demo), (500, 0, "205"));
        assert_eq!(demo.state.edit_value, "row 205");
        assert_eq!(demo.state.browse().table_options["u"], options);
    }
}

#[test]
fn native_xml_restores_and_writes_options_for_tables_that_are_not_active() {
    let mut demo = demo();
    demo.state
        .conn
        .as_ref()
        .unwrap()
        .execute_batch("CREATE TABLE u(other); INSERT INTO u VALUES('u')")
        .unwrap();
    demo.state.refresh_schema().unwrap();
    let source = "<sqlb_project><db path=':memory:'/>
        <tab_browse><table table='4,1:mainu'/><browse_table_settings>
        <table name='t' schema='main' freeze_columns='2'><sort><column index='0' mode='1'/></sort>
        <column_widths><column index='1' value='177'/><column index='2' value='233'/></column_widths>
        <filter_values><column index='0' value='&lt;=9'/></filter_values>
        <global_filter><filter value='row'/></global_filter></table>
        <table name='u' schema='main'/></browse_table_settings></tab_browse></sqlb_project>";
    let mut project = project_xml::decode(source).unwrap();
    project_xml::restore_browse(&mut project, demo.state.conn.as_ref().unwrap()).unwrap();
    assert_eq!(
        project.frozen_cols, 0,
        "inactive freeze setting does not leak into the active table"
    );
    project.apply_to(&mut demo.state);
    demo.state.switch_browse_table("t").unwrap();
    assert_eq!(demo.state.browse_frozen_cols(), 1);
    assert_eq!(demo.state.browse().column_widths, [177.0, 233.0]);
    assert_eq!(demo.state.browse().global_filters, ["row"]);
    assert_eq!(demo.state.browse().col_filters[0], "<=9");
    assert_eq!(demo.state.browse().result.as_ref().unwrap().rows[0][0], "9");
    if let Ok(path) = std::env::var("NEXT_GUI_DB_BROWSER_QA_OUT") {
        demo.state.tab = MainTab::Browse;
        demo.do_rebuild();
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
    demo.state.browse_mut().col_filters[0] = "<7".into();
    demo.state.refresh_browse().unwrap();
    demo.state.switch_browse_table("u").unwrap();
    assert_eq!(demo.state.browse_frozen_cols(), 0);
    let mut project = project::ProjectFile::from_state(&demo.state);
    project_xml::capture_connection(&mut project, demo.state.conn.as_ref().unwrap()).unwrap();
    let mut xml = project_xml::encode(&project, std::path::Path::new("native.sqbpro")).unwrap();
    let start = xml.find("<next_gui").unwrap();
    let end = xml.find("</next_gui>").unwrap() + "</next_gui>".len();
    xml.replace_range(start..end, "");
    let mut native = project_xml::decode(&xml).unwrap();
    project_xml::restore_browse(&mut native, demo.state.conn.as_ref().unwrap()).unwrap();
    assert_eq!(native.browse_tabs[0].table.as_deref(), Some("u"));
    let stored = &native.browse_tabs[0].table_options["t"];
    assert_eq!(stored.col_filters[0], "<7");
    assert_eq!(stored.column_widths, [177.0, 233.0]);
    assert_eq!(stored.ordering[0].column, "n");
    assert!(stored.ordering[0].descending);
    assert_eq!(stored.global_filters, ["row"]);
    assert_eq!(stored.frozen_cols, Some(1));
    native.apply_to(&mut demo.state);
    demo.state.switch_browse_table("t").unwrap();
    assert_eq!(
        demo.state.browse_frozen_cols(),
        1,
        "inactive freeze metadata survives saving other options"
    );
}

#[test]
fn opening_a_project_preflights_saved_tables_before_replacing_the_database() {
    let mut demo = demo();
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("other.db");
    let conn = rusqlite::Connection::open(&database).unwrap();
    conn.execute_batch("CREATE TABLE t(n,label); CREATE TABLE u(other)")
        .unwrap();
    drop(conn);
    demo.state.checkpoint_project();
    for active in [Some("t".to_string()), None] {
        let mut tab = state::BrowseTabState::new(active);
        tab.table_options.insert(
            "u".into(),
            state::BrowseTableOptions {
                filter: "missing_function()".into(),
                ..Default::default()
            },
        );
        let project = project::ProjectFile {
            db_path: database.to_string_lossy().into_owned(),
            browse_tabs: vec![tab],
            ..Default::default()
        };
        let path = dir.path().join("bad.json");
        project.save(&path).unwrap();
        let error = demo.state.open_project(&path).unwrap_err();
        assert!(error.contains("Saved table u"), "{error}");
        assert!(error.contains("missing_function"), "{error}");
        assert_eq!(demo.state.path, ":memory:");
        assert_eq!(selected(&demo), (0, 0, "1205"));
        assert_eq!(demo.state.edit_value, "row 1205");
        assert_eq!(
            db::count_rows(demo.state.conn.as_ref().unwrap(), "t").unwrap(),
            1205
        );
    }
}

#[test]
fn freeze_uses_selected_column_and_keeps_frozen_cells_and_filters_stationary() {
    let mut demo = demo();
    demo.state.frozen_cols = 0;
    demo.state
        .conn
        .as_ref()
        .unwrap()
        .execute_batch(
            "ALTER TABLE t ADD COLUMN c; ALTER TABLE t ADD COLUMN d; CREATE TABLE u(other)",
        )
        .unwrap();
    demo.state.refresh_schema().unwrap();
    demo.state.refresh_browse().unwrap();
    demo.state.browse_mut().column_widths = vec![240.0; 4];
    demo.state.browse_mut().sel_col = Some(1);
    demo.win_w = 1000;
    demo.do_rebuild();
    demo.handle_button(demo.ids.btn_freeze);
    demo.do_rebuild();
    assert_eq!(demo.state.browse_frozen_cols(), 2);
    demo.ui
        .set_table_scroll_offset(demo.ids.browse_table, 150.0, 0.0);
    demo.ui.sync(&mut demo.scene);
    let table = demo
        .scene
        .iter_paint_order()
        .into_iter()
        .find(|node| demo.ui.widget_from_node(node.id) == Some(demo.ids.browse_table))
        .unwrap()
        .bounds;
    let field = demo.ids.browse_col_filters[0];
    let filter = demo
        .scene
        .iter_paint_order()
        .into_iter()
        .find(|node| demo.ui.widget_from_node(node.id) == Some(field))
        .unwrap()
        .bounds;
    let style = next_gui_ui::TableStyle::from_theme(&demo.theme);
    assert!((filter.x - table.x - style.padding - 2.0).abs() < 0.01);
    assert!(demo.ui.pointer_pressed(
        &mut demo.scene,
        table.x + 150.0,
        table.y + style.header_height + 12.0
    ));
    demo.ui.pointer_released();
    assert_eq!(demo.ui.table_selected_col(demo.ids.browse_table), Some(0));
    demo.snapshot_inputs();
    demo.state.switch_browse_table("u").unwrap();
    assert_eq!(demo.state.browse_frozen_cols(), 0);
    demo.state.switch_browse_table("t").unwrap();
    assert_eq!(demo.state.browse_frozen_cols(), 2);
    demo.do_rebuild();
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
    demo.handle_button(demo.ids.btn_freeze);
    demo.do_rebuild();
    assert_eq!(demo.state.browse().frozen_cols, Some(0));
    demo.state.switch_browse_table("u").unwrap();
    demo.state.switch_browse_table("t").unwrap();
    assert_eq!(demo.state.browse_frozen_cols(), 0);
    let mut empty = Demo::new(1000, 700, PresentBackend::Softbuffer);
    empty.state.close_db();
    empty.state.tab = MainTab::Browse;
    empty.do_rebuild();
    assert_eq!(empty.ui.button_enabled(empty.ids.btn_freeze), Some(false));
}

#[test]
fn frozen_overrides_roundtrip_per_tab_and_legacy_projects_keep_the_global_count() {
    let mut demo = demo();
    demo.state.frozen_cols = 0;
    demo.state.browse_mut().frozen_cols = Some(2);
    demo.state.browse_mut().column_widths = vec![147.0, 231.0];
    demo.state.add_browse_tab().unwrap();
    demo.state.switch_browse_table("t").unwrap();
    assert_eq!(demo.state.browse_frozen_cols(), 0);
    demo.state.browse_mut().filter = "n<10".into();
    demo.state.refresh_browse().unwrap();
    demo.state.browse_mut().column_widths = vec![188.0, 288.0];
    demo.state.switch_browse_tab(0).unwrap();
    let mut project = project::ProjectFile::from_state(&demo.state);
    project_xml::capture_connection(&mut project, demo.state.conn.as_ref().unwrap()).unwrap();
    let json = serde_json::to_string(&project).unwrap();
    let xml = project_xml::encode(&project, std::path::Path::new("freeze.sqbpro")).unwrap();
    for mut restored in [
        serde_json::from_str::<project::ProjectFile>(&json).unwrap(),
        project_xml::decode(&xml).unwrap(),
    ] {
        project_xml::restore_browse(&mut restored, demo.state.conn.as_ref().unwrap()).unwrap();
        restored.apply_to(&mut demo.state);
        demo.state.refresh_browse().unwrap();
        assert_eq!(demo.state.browse_frozen_cols(), 2);
        assert_eq!(demo.state.browse().column_widths, [147.0, 231.0]);
        demo.state.switch_browse_tab(1).unwrap();
        assert_eq!(demo.state.browse_frozen_cols(), 0);
        assert_eq!(demo.state.browse().column_widths, [188.0, 288.0]);
        assert_eq!(demo.state.browse().filter, "n<10");
    }
    let mut legacy = serde_json::to_value(&project).unwrap();
    legacy["frozen_cols"] = serde_json::json!(1);
    for tab in legacy["browse_tabs"].as_array_mut().unwrap() {
        tab.as_object_mut().unwrap().remove("frozen_cols");
    }
    let legacy: project::ProjectFile = serde_json::from_value(legacy).unwrap();
    legacy.apply_to(&mut demo.state);
    demo.state.refresh_browse().unwrap();
    assert_eq!(demo.state.browse_frozen_cols(), 1);
    demo.state.switch_browse_tab(1).unwrap();
    assert_eq!(demo.state.browse_frozen_cols(), 1);
    demo.state.browse_mut().frozen_cols = Some(usize::MAX);
    assert_eq!(demo.state.browse_frozen_cols(), 2);
}

#[test]
fn native_frozen_counts_are_validated_and_clamped_to_the_table_columns() {
    let demo = demo();
    for (value, expected) in [
        ("0", Some(0)),
        ("1", Some(0)),
        ("2", Some(1)),
        ("3", Some(2)),
        ("999", Some(2)),
        ("-1", None),
        ("invalid", None),
    ] {
        let xml = format!("<sqlb_project><db path=':memory:'/><tab_browse><table table='4,1:maint'/><browse_table_settings><table name='t' schema='main' freeze_columns='{value}'/></browse_table_settings></tab_browse></sqlb_project>");
        let mut project = project_xml::decode(&xml).unwrap();
        let restored = project_xml::restore_browse(&mut project, demo.state.conn.as_ref().unwrap());
        if let Some(expected) = expected {
            restored.unwrap();
            assert_eq!(project.browse_tabs[0].frozen_cols, Some(expected));
        } else {
            assert!(restored.unwrap_err().contains("frozen column count"));
        }
    }
}

#[test]
fn header_context_uses_the_clicked_column_and_supports_keyboard_selection() {
    let mut demo = demo();
    demo.state.frozen_cols = 0;
    demo.state.conn.as_ref().unwrap().execute_batch(
        "ALTER TABLE t RENAME COLUMN label TO '姓 名'; ALTER TABLE t ADD COLUMN c; ALTER TABLE t ADD COLUMN d"
    ).unwrap();
    demo.state.refresh_browse().unwrap();
    demo.state.browse_mut().sel_row = Some(0);
    demo.state.browse_mut().sel_col = Some(0);
    demo.state.browse_mut().column_widths = vec![240.0; 4];
    demo.state.sync_edit_cell_from_selection();
    demo.win_w = 1000;
    demo.do_rebuild();
    demo.ui
        .set_table_scroll_offset(demo.ids.browse_table, 150.0, 0.0);
    demo.ui.sync(&mut demo.scene);
    let bounds = demo
        .scene
        .iter_paint_order()
        .into_iter()
        .find(|node| demo.ui.widget_from_node(node.id) == Some(demo.ids.browse_table))
        .unwrap()
        .bounds;
    assert!(!demo.open_browse_header_menu(bounds.x + 270.0, bounds.y + 60.0));
    assert!(demo.open_browse_header_menu(bounds.x + 270.0, bounds.y + 10.0));
    assert_eq!(demo.browse_header_name(), Some("姓 名"));
    assert_eq!(demo.state.browse().sel_col, Some(0));
    assert_eq!(demo.state.edit_value, "1205");
    demo.ui.sync(&mut demo.scene);
    assert!(!demo.handle_browse_navigation_key(KeyCode::PageDown, Modifiers::default()));
    demo.ui.key_input(UiKey::Home);
    demo.ui.key_input(UiKey::Enter);
    let (_, action) = demo.ui.take_menu_action().unwrap();
    assert_eq!(action, mid::BROWSE_FREEZE);
    demo.handle_menu(action);
    demo.do_rebuild();
    assert_eq!(demo.state.browse_frozen_cols(), 2);
    assert_eq!(demo.state.browse().sel_col, Some(0));
    assert!(demo.open_browse_header_menu(bounds.x + 270.0, bounds.y + 10.0));
    demo.ui.key_input(UiKey::Home);
    demo.ui.key_input(UiKey::Down);
    demo.ui.key_input(UiKey::Enter);
    let (_, action) = demo.ui.take_menu_action().unwrap();
    assert_eq!(action, mid::BROWSE_SELECT_COLUMN);
    demo.handle_menu(action);
    demo.do_rebuild();
    assert_eq!(
        demo.ui.table_selected_cols(demo.ids.browse_table),
        Some((1, 1))
    );
    assert_eq!(
        demo.ui.table_selected_rows(demo.ids.browse_table),
        Some((0, 499))
    );
    assert_eq!(demo.state.edit_value, "row 1205");
    assert_eq!(demo.ui.table_selected_row(demo.ids.browse_table), Some(0));
    demo.poll_selection_changes();
    assert_eq!(demo.state.browse().sel_row, Some(0));
    assert_eq!(demo.state.browse().sel_row_end, Some(499));
    assert!(demo.state.status_msg.contains("500 records"));
    assert!(!demo.state.dirty);
    assert!(demo.open_browse_header_menu(bounds.x + 270.0, bounds.y + 10.0));
    demo.ui.key_input(UiKey::End);
    demo.ui.key_input(UiKey::Up);
    demo.ui.sync(&mut demo.scene);
    assert_eq!(demo.browse_header_name(), Some("姓 名"));
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
    demo.ui.key_input(UiKey::Enter);
    assert_eq!(
        demo.ui.take_menu_action(),
        Some((demo.ids.context_menu, mid::BROWSE_COPY_COLUMN))
    );
}

#[test]
fn header_context_keeps_drafts_and_refuses_obscured_busy_or_stale_targets() {
    let mut demo = demo();
    let bounds = demo
        .scene
        .iter_paint_order()
        .into_iter()
        .find(|node| demo.ui.widget_from_node(node.id) == Some(demo.ids.browse_table))
        .unwrap()
        .bounds;
    let point = (bounds.x + 30.0, bounds.y + 10.0);
    demo.state.overlay = Some(OverlayKind::Preferences);
    assert!(!demo.open_browse_header_menu(point.0, point.1));
    demo.state.overlay = None;
    demo.state.confirm_open = true;
    assert!(!demo.open_browse_header_menu(point.0, point.1));
    demo.state.confirm_open = false;
    demo.state.sql_busy = true;
    assert!(!demo.open_browse_header_menu(point.0, point.1));
    demo.state.sql_busy = false;
    demo.ui
        .set_text_area_value(demo.ids.edit_value, "keep draft");
    assert!(demo.open_browse_header_menu(point.0, point.1));
    demo.ui.key_input(UiKey::Home);
    demo.ui.key_input(UiKey::Down);
    demo.ui.key_input(UiKey::Enter);
    assert_eq!(
        demo.ui.take_menu_action(),
        Some((demo.ids.context_menu, mid::BROWSE_COPY_COLUMN))
    );
    assert_eq!(demo.state.edit_value, "keep draft");
    assert_eq!(selected(&demo), (0, 0, "1205"));
    demo.handle_menu(mid::BROWSE_SELECT_COLUMN);
    assert!(demo.state.error_msg.as_ref().unwrap().contains("draft"));
    assert_eq!(demo.state.edit_value, "keep draft");
    demo.do_rebuild();
    let count = demo.state.browse_frozen_cols();
    demo.handle_menu(mid::BROWSE_FREEZE);
    assert_eq!(demo.state.browse_frozen_cols(), count);
    assert!(demo.state.has_cell_draft());
    assert!(demo.open_browse_header_menu(point.0, point.1));
    demo.handle_menu(mid::BROWSE_FREEZE);
    demo.do_rebuild();
    assert_eq!(
        demo.state.browse_frozen_cols(),
        if count == 0 { 1 } else { 0 }
    );
    assert_eq!(demo.state.edit_value, "keep draft");
}

#[test]
fn selected_and_highlighted_labels_have_readable_contrast_in_both_themes() {
    fn luminance(color: Color) -> f64 {
        let linear = |channel: u8| {
            let channel = f64::from(channel) / 255.0;
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
    }
    for base in [Theme::light(), Theme::dark()] {
        let theme = ui_build::browser_theme(base, 13.0);
        let table = next_gui_ui::TableStyle::from_theme(&theme);
        let menu = next_gui_ui::MenuStyle::from_theme(&theme);
        let context = next_gui_ui::ContextMenuStyle::from_theme(&theme);
        for (foreground, background) in [
            (table.selected_fg, table.selected_bg),
            (menu.hover_fg, menu.hover_bg),
            (context.hover_fg, context.hover_bg),
        ] {
            let (a, b) = (luminance(foreground), luminance(background));
            let ratio = (a.max(b) + 0.05) / (a.min(b) + 0.05);
            assert!(ratio >= 4.5, "{:?}: contrast {ratio}", theme.mode);
        }
    }
}

#[test]
fn revert_modal_cancel_and_accept_reconcile_sources_and_rebuild() {
    let folder=std::env::var("NEXT_GUI_REVERT_QA_OUT").ok().map(std::path::PathBuf::from);
    if let Some(folder)=&folder {std::fs::create_dir_all(folder).unwrap();}
    for (name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
        for (width,height,font) in [(1400,860,13.0),(800,600,18.0)] {
            for missing in [false,true] {
                let mut demo=Demo::new(width,height,PresentBackend::Softbuffer);
                demo.state.new_memory_db().unwrap();demo.state.conn.as_ref().unwrap().execute_batch(crate::browse_rowid::QA_SQL).unwrap();demo.state.write_changes().unwrap();demo.state.refresh_schema().unwrap();
                demo.state.tab=MainTab::Browse;demo.state.switch_browse_table_in("main","measurements").unwrap();demo.state.set_show_rowid(true).unwrap();demo.state.add_browse_tab().unwrap();
                demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE pending_measurements AS SELECT region,reading,note FROM measurements LIMIT 4; UPDATE measurements SET reading=reading+100;").unwrap();demo.state.refresh_schema().unwrap();
                if missing {demo.state.switch_browse_table_in("main","pending_measurements").unwrap();} else {demo.state.refresh_browse().unwrap();}
                demo.state.set_show_rowid(true).unwrap();demo.state.goto_browse_record(1).unwrap();demo.state.browse_mut().sel_col=Some(2);demo.state.prefs.auto_switch_mode=false;demo.state.sync_edit_cell_from_selection();
                demo.state.dirty=true;demo.state.dock_focus=DockKind::Plot;demo.state.body_ratio=0.45;demo.state.dock_ratio=0.62;demo.state.prefs.font_size=font;demo.theme=ui_build::browser_theme(theme,font);
                demo.state.sync_current_plot().unwrap();assert!(demo.state.plot.check(0,1,true));assert!(demo.state.plot.check(3,2,true));demo.do_rebuild();
                let kind=if missing {"missing"} else {"existing"};
                let capture=|demo:&mut Demo,stage:&str| {if let Some(folder)=&folder {demo.ui.sync(&mut demo.scene);crate::plot_export::capture(demo,folder,&format!("revert-{kind}-{stage}-{name}-{width}"));}};
                demo.handle_button(demo.ids.btn_revert);demo.do_rebuild();assert!(demo.state.confirm_open);assert_eq!(demo.state.confirm_kind,ConfirmKind::Revert);
                demo.ui.set_focus(Some(demo.ids.modal));capture(&mut demo,"confirm");
                assert!(demo.ui.key_input(UiKey::Escape));demo.handle_modal();demo.do_rebuild();
                assert!(!demo.state.confirm_open);assert!(demo.state.dirty);assert!(demo.state.error_msg.is_none());assert!(demo.state.browse().result.is_some());
                assert_eq!(dbmod::count_rows(demo.state.conn.as_ref().unwrap(),"pending_measurements").unwrap(),4);capture(&mut demo,"cancelled");
                demo.handle_button(demo.ids.btn_revert);demo.do_rebuild();demo.ui.set_focus(Some(demo.ids.modal));assert!(demo.ui.key_input(UiKey::Enter));demo.handle_modal();demo.do_rebuild();
                assert!(!demo.state.confirm_open);assert!(!demo.state.dirty);assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);assert!(demo.state.status_msg.starts_with("Changes reverted"));
                assert!(demo.state.browse().row_keys.is_empty() || !missing);assert_eq!(demo.state.browse().sel_row,None);assert!(demo.state.edit_col.is_empty());
                if missing {assert!(demo.state.browse().table.is_none());assert!(demo.state.browse().result.is_none());}
                else {assert_eq!(demo.state.browse().result.as_ref().unwrap().rows[0][2],"1.25");assert_eq!(demo.state.browse().row_keys.len(),18);}
                assert!(dbmod::table_column_names_in(demo.state.conn.as_ref().unwrap(),"main","pending_measurements").is_err());capture(&mut demo,"reverted");
                demo.state.switch_browse_tab(0).unwrap();demo.do_rebuild();assert_eq!(demo.state.browse().result.as_ref().unwrap().rows[0][2],"1.25");assert_eq!(demo.state.browse().row_keys.len(),18);
            }
        }
    }
}
