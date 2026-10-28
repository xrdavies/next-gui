use super::*;

fn bounds(demo: &Demo, id: WidgetId) -> next_gui_scene::Rect {
    demo.scene
        .iter_paint_order()
        .into_iter()
        .find(|node| demo.ui.widget_from_node(node.id) == Some(id))
        .unwrap()
        .bounds
}

#[test]
fn independent_cell_and_plot_docks_keep_drafts_scrolls_ratios_and_project_layout() {
    let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
    demo.state.new_memory_db().unwrap();
    db::seed_demo(demo.state.conn.as_ref().unwrap()).unwrap();
    demo.state.refresh_schema().unwrap();
    demo.state
        .switch_browse_table_in("main", "customers")
        .unwrap();
    demo.state.tab = MainTab::Browse;
    demo.state.browse_mut().sel_row = Some(0);
    demo.state.browse_mut().sel_col = Some(1);
    demo.state.sync_edit_cell_from_selection();
    demo.state.dock_focus = DockKind::Plot;
    demo.state.sync_current_plot().unwrap();
    for (row, axis) in [(0, 1), (1, 2)] {
        demo.state.plot.check(row, axis, true);
    }
    demo.do_rebuild();
    assert!(demo.ui.is_splitter(demo.ids.dock_splitter));
    let editor = bounds(&demo, demo.ids.edit_value);
    let chart = bounds(&demo, demo.ids.chart);
    assert!(
        editor.y + editor.height < chart.y,
        "The cell editor and Plot must both be visible"
    );
    assert_eq!(
        demo.ui
            .hit_widget(&demo.scene, editor.x + 10.0, editor.y + 10.0),
        Some(demo.ids.edit_value)
    );
    assert_eq!(
        demo.ui
            .hit_widget(&demo.scene, chart.x + 10.0, chart.y + 10.0),
        Some(demo.ids.chart)
    );
    demo.ui.set_splitter_ratio(demo.ids.dock_splitter, 0.46);
    demo.ui.sync(&mut demo.scene);
    demo.snapshot_inputs();
    assert!((demo.state.dock_ratio - 0.46).abs() < 0.001);
    demo.ui.set_splitter_ratio(demo.ids.plot_splitter, 0.51);
    demo.ui.sync(&mut demo.scene);
    demo.snapshot_inputs();
    assert!((demo.state.plot_ratio - 0.51).abs() < 0.001);
    demo.win_w = 800;
    demo.win_h = 600;
    demo.do_rebuild();
    demo.snapshot_inputs();
    assert!(
        (demo.state.dock_ratio - 0.46).abs() < 0.001,
        "Adaptive constraints must retain the preferred split"
    );
    assert!((demo.state.plot_ratio - 0.51).abs() < 0.001);
    for dock in [DockKind::EditCell, DockKind::Plot] {
        let id = demo.ids.dock_scrolls[dock as usize];
        assert_ne!(id.0, 0);
        demo.ui.set_scroll_offset(id, 0.0, 60.0);
        let offset = demo.ui.scroll_offset(id).unwrap();
        assert!(offset.1 > 0.0);
        demo.snapshot_inputs();
        assert_eq!(demo.state.dock_scroll[dock as usize], offset);
    }
    let scrolls = demo.state.dock_scroll;
    demo.do_rebuild();
    for dock in [DockKind::EditCell, DockKind::Plot] {
        assert_eq!(
            demo.ui.scroll_offset(demo.ids.dock_scrolls[dock as usize]),
            Some(scrolls[dock as usize])
        );
    }
    demo.ui.set_focus(Some(demo.ids.dock_tabs));
    assert!(demo.ui.key_input(UiKey::Home));
    demo.snapshot_inputs();
    demo.do_rebuild();
    demo.snapshot_inputs();
    assert_eq!(
        demo.state.dock_scroll[DockKind::Plot as usize],
        scrolls[DockKind::Plot as usize],
        "Hidden pages must not overwrite retained scrolls with unlaid-out zero offsets"
    );
    assert!(demo.ui.key_input(UiKey::Right));
    demo.snapshot_inputs();
    demo.do_rebuild();
    assert_eq!(
        demo.ui
            .scroll_offset(demo.ids.dock_scrolls[DockKind::Plot as usize]),
        Some(scrolls[DockKind::Plot as usize])
    );
    demo.ui
        .set_text_area_value(demo.ids.edit_value, "Kept cell draft");
    demo.snapshot_inputs();
    assert!(demo.state.has_cell_draft());
    demo.handle_button(demo.ids.btn_dock_edit_close);
    demo.do_rebuild();
    assert!(!demo.state.dock_edit && demo.state.dock_focus == DockKind::Plot);
    assert_eq!(demo.state.edit_value, "Kept cell draft");
    demo.handle_menu(mid::VIEW_DOCK_EDIT);
    demo.do_rebuild();
    assert!(demo.state.dock_edit);
    assert_eq!(
        demo.ui.text_area_value(demo.ids.edit_value),
        Some("Kept cell draft")
    );
    demo.handle_button(demo.ids.btn_dock_close);
    demo.do_rebuild();
    assert!(!demo.state.dock_plot && demo.state.dock_edit);
    assert!(demo.state.has_cell_draft());
    demo.handle_menu(mid::VIEW_DOCK_PLOT);
    demo.do_rebuild();
    assert!(demo.state.dock_plot);
    demo.state.dock_remote = false;
    let project = project::ProjectFile::from_state(&demo.state);
    let json: project::ProjectFile =
        serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
    let xml =
        project_xml::encode(&project, std::path::Path::new("/tmp/next-gui-docks.sqbpro")).unwrap();
    for saved in [json, project_xml::decode(&xml).unwrap()] {
        let mut state = AppState::empty();
        saved.apply_to(&mut state);
        assert!(
            (state.dock_ratio - 0.46).abs() < 0.001
                && (state.plot_ratio - 0.51).abs() < 0.001
                && state.dock_focus == DockKind::Plot
                && !state.dock_remote
        );
    }
    let old: project::ProjectFile = serde_json::from_str("{}").unwrap();
    assert_eq!(old.dock_ratio, 0.38);
    demo.state.sync_edit_cell_from_selection();
    let folder = std::env::var("NEXT_GUI_DOCK_QA_OUT")
        .ok()
        .map(std::path::PathBuf::from);
    for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
        demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
        for (width, height) in [(1400, 1000), (800, 600)] {
            demo.win_w = width;
            demo.win_h = height;
            demo.state.dock_scroll = [(0.0, 0.0); 5];
            demo.do_rebuild();
            assert_ne!(demo.ids.edit_value.0, 0);
            assert_ne!(demo.ids.chart.0, 0);
            let chart = bounds(&demo, demo.ids.chart);
            assert!(
                chart.height >= 150.0,
                "Chart axes need readable spacing at the minimum window size"
            );
            let tabs = bounds(&demo, demo.ids.dock_tabs);
            assert!(chart.y + chart.height <= tabs.y + tabs.height - 32.0);
            for id in [demo.ids.plot.copy, demo.ids.btn_edit_cell] {
                demo.ui.set_focus(Some(id));
                demo.ui.sync(&mut demo.scene);
                let control = bounds(&demo, id);
                assert_eq!(
                    demo.ui
                        .hit_widget(&demo.scene, control.x + 10.0, control.y + 10.0),
                    Some(id)
                );
                assert_eq!(
                    bounds(&demo, demo.ids.chart),
                    chart,
                    "Options and editor scrolling must leave the chart visible"
                );
                assert_eq!(
                    demo.ui.hit_widget(
                        &demo.scene,
                        chart.x + chart.width * 0.5,
                        chart.y + chart.height * 0.5
                    ),
                    Some(demo.ids.chart)
                );
            }
            demo.ui.set_focus(None);
            demo.ui
                .set_scroll_offset(demo.ids.dock_scrolls[DockKind::EditCell as usize], 0.0, 0.0);
            demo.ui
                .set_scroll_offset(demo.ids.dock_scrolls[DockKind::Plot as usize], 0.0, 0.0);
            demo.ui.sync(&mut demo.scene);
            if let Some(folder) = &folder {
                std::fs::create_dir_all(folder).unwrap();
                crate::plot_export::capture(&mut demo, folder, &format!("docks-{name}-{width}"));
            }
        }
    }
    demo.state.sql_busy = true;
    demo.handle_menu(mid::VIEW_DOCK_EDIT);
    assert!(!demo.state.dock_edit);
    demo.handle_menu(mid::VIEW_DOCK_EDIT);
    assert!(demo.state.dock_edit);
    demo.state.sql_busy = false;
    for _ in 0..demo.state.visible_dock_tabs().len() {
        let before = demo.state.visible_dock_tabs().len();
        demo.handle_button(demo.ids.btn_dock_close);
        demo.do_rebuild();
        assert_eq!(demo.state.visible_dock_tabs().len(), before - 1);
    }
    assert_eq!(demo.ids.dock_splitter.0, 0);
    assert_ne!(demo.ids.edit_value.0, 0);
    demo.handle_button(demo.ids.btn_dock_edit_close);
    demo.do_rebuild();
    assert_eq!(demo.ids.body_splitter.0, 0);
    assert_ne!(demo.ids.browse_table.0, 0);
}
