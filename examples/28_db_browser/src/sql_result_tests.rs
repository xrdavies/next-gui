use super::*;
use std::sync::Arc;

#[test]
fn complete_sql_grid_navigates_copies_links_plots_and_retains_tab_selections() {
    let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap();
    demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE t(x INTEGER,y INTEGER); WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<3000) INSERT INTO t SELECT x,0 FROM n").unwrap();
    demo.state.tab = MainTab::ExecuteSql;
    *demo.state.sql_editor_mut() = "UPDATE t SET y=y+1 RETURNING x,y".into();
    demo.state.run_sql_sync().unwrap();
    let values = demo.state.sql_result.as_ref().unwrap().all_values.clone().unwrap();
    assert_eq!(values.len(), 3000);
    assert_eq!(demo.state.sql_result.as_ref().unwrap().rows.len(), 500);
    assert!(demo.state.sql_message.contains("3000"));
    demo.state.dock_focus = DockKind::Plot;
    demo.state.dock_plot = true;
    demo.state.plot.settings.manual = false;
    demo.state.sync_current_plot().unwrap();
    demo.state.plot.check(1, 1, true); demo.state.plot.check(2, 2, true);
    demo.do_rebuild();
    demo.ui.set_focus(Some(demo.ids.sql_table));
    assert!(demo.ui.key_input(UiKey::End)); demo.ui.sync(&mut demo.scene);
    assert_eq!(demo.ui.table_selected_row(demo.ids.sql_table), Some(2999));
    assert_eq!(demo.ui.focused_text_for_copy().as_deref(), Some("3000\t1"));
    assert!(demo.ui.set_table_selected_cells(demo.ids.sql_table, Some((2999, 2997)), Some((1, 0))));
    demo.snapshot_inputs();
    let scroll = demo.state.sql_scroll;
    demo.do_rebuild();
    assert_eq!(demo.ui.focused(), Some(demo.ids.sql_table));
    assert_eq!(demo.ui.table_selection_anchor(demo.ids.sql_table), Some((2999, Some(1))));
    assert_eq!(demo.ui.table_selected_row(demo.ids.sql_table), Some(2997));
    assert_eq!(demo.ui.table_scroll_offset(demo.ids.sql_table), Some(scroll));
    assert!(demo.ui.key_input(UiKey::SelectUp));
    assert_eq!(demo.ui.table_selected_rows(demo.ids.sql_table), Some((2996, 2999)));
    demo.snapshot_inputs();
    demo.state.sql_tabs.push("SELECT 42".into()); demo.state.switch_sql_tab(1);
    demo.state.run_sql_sync().unwrap(); demo.do_rebuild();
    assert_eq!(demo.ui.table_selected_row(demo.ids.sql_table), None);
    demo.state.switch_sql_tab(0); demo.do_rebuild();
    assert!(Arc::ptr_eq(&values, demo.state.sql_result.as_ref().unwrap().all_values.as_ref().unwrap()));
    assert_eq!(demo.ui.table_selection_anchor(demo.ids.sql_table), Some((2999, Some(1))));
    assert_eq!(demo.ui.table_selected_row(demo.ids.sql_table), Some(2996));
    demo.state.start_plot_load().unwrap();
    let deadline = Instant::now() + std::time::Duration::from_secs(10);
    while demo.state.plot_work.is_some() {
        assert!(Instant::now() < deadline); demo.state.poll_plot_load().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    demo.do_rebuild();
    assert!(demo.ui.chart_plot_mut(demo.ids.chart, 0).unwrap().select_row(2501, false));
    demo.snapshot_inputs(); demo.poll_selection_changes(); demo.ui.sync(&mut demo.scene);
    assert_eq!(demo.ui.table_selected_row(demo.ids.sql_table), Some(2501));
    assert_eq!(demo.ui.table_selected_text(demo.ids.sql_table).as_deref(), Some("2502\t1"));
    demo.do_rebuild();
    assert_eq!(demo.ui.table_selected_row(demo.ids.sql_table), Some(2501));
    assert!(demo.ui.chart_plot_mut(demo.ids.chart, 0).unwrap().select_row(2504, true));
    demo.snapshot_inputs(); demo.poll_selection_changes(); demo.ui.sync(&mut demo.scene);
    assert_eq!(demo.ui.table_selected_rows(demo.ids.sql_table), Some((2501, 2504)));
    assert_eq!(demo.ui.table_selected_text(demo.ids.sql_table).as_deref(), Some("2502\t1\n2503\t1\n2504\t1\n2505\t1"));
    let sum: i64 = demo.state.conn.as_ref().unwrap().query_row("SELECT sum(y) FROM t", [], |row| row.get(0)).unwrap();
    assert_eq!(sum, 3000, "Table and Plot reads must never repeat UPDATE RETURNING");
    capture(&mut demo, "NEXT_GUI_SQL_GRID_QA_OUT");
    assert!(demo.state.run_sql_text_sync("SELECT missing_column FROM t").is_err()); demo.do_rebuild();
    assert!(demo.state.sql_result.is_none());
    demo.state.run_sql_text_sync("SELECT 7 AS value").unwrap(); demo.do_rebuild();
    assert_eq!(demo.ui.table_selected_row(demo.ids.sql_table), None);
    assert_eq!(demo.state.sql_scroll, (0.0, 0.0));
}

#[test]
fn sql_cell_dock_follows_complete_selection_and_keeps_read_only_modes_and_browse_identity() {
    let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap();
    demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE t(id INTEGER,n INTEGER,raw BLOB); WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<3000) INSERT INTO t SELECT x,0,X'00FF' FROM n").unwrap();
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(2, 2, image::Rgba([20, 40, 60, 255])))
        .write_to(&mut png, image::ImageFormat::Png).unwrap();
    demo.state.conn.as_ref().unwrap().execute("UPDATE t SET raw=?1 WHERE id=3000", [png.into_inner()]).unwrap();
    demo.state.refresh_schema().unwrap(); demo.state.switch_browse_table_in("main", "t").unwrap();
    demo.state.goto_browse_record(1).unwrap(); demo.state.browse_mut().sel_col = Some(1); demo.state.sync_edit_cell_from_selection();
    demo.state.tab = MainTab::ExecuteSql;
    *demo.state.sql_editor_mut() = "UPDATE t SET n=n+1\nRETURNING id,n,raw,'{\"ready\":true}' AS json;".into();
    demo.state.run_sql_sync().unwrap();
    let snapshot = demo.state.sql_result.as_ref().unwrap().all_values.clone().unwrap();
    demo.state.dock_focus = DockKind::Plot; demo.do_rebuild();
    demo.ui.set_table_selected_cells(demo.ids.sql_table, Some((2999, 2999)), Some((1, 1)));
    demo.ui.set_focus(Some(demo.ids.sql_table)); demo.snapshot_inputs(); demo.poll_selection_changes(); demo.do_rebuild();
    assert_eq!(demo.state.sql_cell_position(), Some((2999, 1))); assert_eq!(demo.state.edit_value, "1");
    assert_eq!(demo.ui.focused(), Some(demo.ids.sql_table));
    assert_eq!(demo.ui.button_enabled(demo.ids.btn_edit_cell), Some(false));
    assert_eq!(demo.ui.button_enabled(demo.ids.btn_edit_discard), Some(false));
    assert_eq!(demo.ui.text_input_enabled(demo.ids.edit_col), Some(false)); assert_eq!(demo.ids.edit_type, WidgetId(0));
    demo.ui.set_focus(Some(demo.ids.edit_value)); demo.ui.key_input(UiKey::SelectAll);
    assert_eq!(demo.ui.focused_text_for_copy().as_deref(), Some("1"));
    for key in [UiKey::Char('9'), UiKey::Backspace, UiKey::Delete] { demo.ui.key_input(key); }
    demo.ui.paste_into_focused_text("unsafe"); demo.ui.ime_commit("替换");
    assert_eq!(demo.ui.text_area_value(demo.ids.edit_value), Some("1")); demo.snapshot_inputs();
    assert!(!demo.state.has_cell_draft()); assert!(demo.state.apply_cell_draft().is_err());
    capture(&mut demo, "NEXT_GUI_SQL_CELL_QA_OUT");
    demo.state.prefs.indent_compact=true;
    for (column, mode) in [(3, crate::state::EditCellMode::Json), (2, crate::state::EditCellMode::Image)] {
        demo.ui.set_table_selected_cells(demo.ids.sql_table, Some((2999, 2999)), Some((column, column)));
        demo.snapshot_inputs(); demo.poll_selection_changes(); demo.do_rebuild();
        demo.ui.set_select_selected(demo.ids.edit_mode, Some(mode as usize));
        demo.snapshot_inputs(); demo.poll_selection_changes(); demo.do_rebuild();
        assert_eq!(demo.state.edit_mode, mode); assert!(!demo.state.has_cell_draft());
        if column == 3 { assert!(demo.state.edit_value.contains("\n")); }
        else { assert_eq!(demo.state.edit_image_rgba.as_ref().map(|(w,h,_)| (*w,*h)), Some((2,2))); }
        demo.do_rebuild(); assert_eq!(demo.state.edit_mode, mode);
    }
    assert!(Arc::ptr_eq(&snapshot, demo.state.sql_result.as_ref().unwrap().all_values.as_ref().unwrap()));
    assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT sum(n) FROM t", [], |r| r.get::<_,i64>(0)).unwrap(), 3000);
    demo.state.dock_edit = false; demo.do_rebuild();
    demo.ui.set_table_selected_cells(demo.ids.sql_table, Some((2500, 2500)), Some((0, 0)));
    demo.snapshot_inputs(); demo.poll_selection_changes();
    assert_eq!(demo.state.sql_cell_position(), Some((2999, 2)));
    demo.state.dock_edit = true; demo.do_rebuild();
    assert_eq!(demo.state.sql_cell_position(), Some((2500, 0))); assert_eq!(demo.state.edit_value, "2501");
    demo.state.tab = MainTab::Browse; demo.state.goto_browse_record(1).unwrap();
    demo.state.browse_mut().sel_col = Some(1); demo.do_rebuild();
    assert_eq!(demo.state.sql_cell_position(), None); assert_eq!(demo.state.edit_value, "1");
    demo.state.sql_tabs.push("SELECT 42".into()); demo.state.switch_sql_tab(1);
    demo.state.tab = MainTab::ExecuteSql; demo.do_rebuild();
    demo.state.switch_sql_tab(0); demo.do_rebuild();
    assert_eq!(demo.state.sql_cell_position(), Some((2500, 0)));
    demo.state.run_sql_text_sync("SELECT 99 AS fresh").unwrap(); demo.do_rebuild();
    assert_eq!(demo.state.sql_cell_position(), None); assert!(demo.state.edit_value.is_empty());
}

fn capture(demo: &mut Demo, variable: &str) {
    let Ok(prefix) = std::env::var(variable) else { return; };
    for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
        demo.theme = crate::ui_build::browser_theme(theme, demo.state.prefs.font_size);
        for (width, height) in [(1400, 1000), (800, 600)] {
            demo.win_w = width; demo.win_h = height; demo.do_rebuild();
            demo.ui.reveal_table_selection(demo.ids.sql_table); demo.ui.sync(&mut demo.scene);
            demo.scene.mark_full_dirty(width as f32, height as f32);
            let dirty = demo.scene.take_dirty(); let commands = encode_scene(&demo.scene, &dirty);
            let clear = clear_color(&demo.theme);
            let mut soft = SoftRenderer::new(width, height, clear); soft.render(&commands, &dirty);
            let pixels = soft.frame_pixels().unwrap().0;
            write_ppm(&format!("{prefix}-{name}-{width}.ppm"), pixels, width, height).unwrap();
            if let Ok(mut gpu) = GpuRenderer::try_new(width, height, clear) {
                gpu.render(&commands, &dirty); let rendered = gpu.frame_pixels().unwrap().0;
                write_ppm(&format!("{prefix}-{name}-{width}-metal.ppm"), rendered, width, height).unwrap();
                let bounds = demo.scene.iter_paint_order().into_iter().find(|n| demo.ui.widget_from_node(n.id) == Some(demo.ids.sql_table)).unwrap().bounds;
                let mut delta = 0; let mut checked = 0;
                for y in (bounds.y.ceil().max(0.0) as usize + 2)..((bounds.y + bounds.height).floor().min(height as f32) as usize).saturating_sub(2) {
                    for x in (bounds.x.ceil().max(0.0) as usize + 2)..((bounds.x + bounds.width).floor().min(width as f32) as usize).saturating_sub(2) {
                        for shift in [0, 8, 16] { delta = delta.max((((pixels[y * width as usize + x] >> shift) & 255) as i32 - ((rendered[y * width as usize + x] >> shift) & 255) as i32).abs()); }
                        checked += 1;
                    }
                }
                assert!(checked > 1000 && delta <= 2, "SQL grid {name}/{width}: delta={delta}, pixels={checked}");
                eprintln!("SQL grid {name}/{width}: delta={delta}, pixels={checked}");
            }
        }
    }
}
