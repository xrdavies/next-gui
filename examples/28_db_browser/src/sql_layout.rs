#[cfg(test)]
mod tests {
    use crate::{
        browse_filter::tests::{capture, fixture},
        state::MainTab,
        UiKey,
    };
    use next_gui_ui::{Theme, WidgetId};
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    fn bounds(demo: &crate::Demo, id: WidgetId) -> next_gui_scene::Rect {
        demo.scene
            .iter_paint_order()
            .into_iter()
            .find(|node| demo.ui.widget_from_node(node.id) == Some(id))
            .unwrap()
            .bounds
    }

    #[test]
    fn desktop_sql_split_drag_retains_editor_history_and_complete_results() {
        for (name, theme, width, height) in [
            ("dark", Theme::dark(), 1600, 900),
            ("light", Theme::light(), 1920, 927),
        ] {
            let mut demo = fixture(width, height, theme);
            demo.state.write_changes().unwrap();
            crate::desktop_workflow::prepare(&mut demo.state, "desktop-sql").unwrap();
            let deadline = Instant::now() + Duration::from_secs(20);
            while demo.state.sql_busy {
                assert!(Instant::now() < deadline);
                demo.state.poll_sql_job();
                std::thread::sleep(Duration::from_millis(1));
            }
            let query = "SELECT id,note FROM orders ORDER BY id;";
            demo.state.sql_tabs[0] = query.into();
            demo.state.start_sql_job().unwrap();
            while demo.state.sql_busy {
                assert!(Instant::now() < deadline);
                demo.state.poll_sql_job();
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
            demo.do_rebuild();
            let values = demo
                .state
                .sql_result
                .as_ref()
                .unwrap()
                .all_values
                .clone()
                .unwrap();
            assert_eq!(values.len(), 100000);
            demo.ui.set_table_selected_cells(
                demo.ids.sql_table,
                Some((99997, 99999)),
                Some((0, 1)),
            );
            demo.ui.reveal_table_selection(demo.ids.sql_table);
            demo.ui.set_table_column_width(demo.ids.sql_table, 1, 250.0);
            demo.ui.set_focus(Some(demo.ids.sql_area));
            demo.ui.select_text_area_range(demo.ids.sql_area, 0..0);
            demo.ui
                .replace_text_area_selection(demo.ids.sql_area, "-- report draft\n");
            demo.ui.select_text_area_range(demo.ids.sql_area, 3..9);
            demo.ui.sync(&mut demo.scene);
            let original = bounds(&demo, demo.ids.sql_area);
            let table = bounds(&demo, demo.ids.sql_table);
            let end = table.y + table.height;
            for (stage, delta) in [("results", -150.0), ("editor", 180.0)] {
                let editor = bounds(&demo, demo.ids.sql_area);
                let x = editor.x + editor.width * 0.5;
                let y = editor.y + editor.height + 3.0;
                assert_eq!(
                    demo.ui.hit_widget(&demo.scene, x, y),
                    Some(demo.ids.sql_splitter)
                );
                assert!(demo.ui.pointer_pressed(&mut demo.scene, x, y));
                assert!(demo.ui.pointer_moved(x, y + delta));
                demo.ui.pointer_released();
                demo.ui.sync(&mut demo.scene);
                let resized = bounds(&demo, demo.ids.sql_table);
                assert!((resized.y + resized.height - end).abs() < 1.0);
                assert!(resized.height >= 96.0);
                if stage == "results" {
                    assert!(resized.height > table.height + 100.0);
                }
                let ratio = demo.ui.splitter_ratio(demo.ids.sql_splitter).unwrap();
                demo.snapshot_inputs();
                demo.do_rebuild();
                assert!(
                    (demo.ui.splitter_ratio(demo.ids.sql_splitter).unwrap() - ratio).abs() < 0.001
                );
                assert_eq!(
                    demo.ui
                        .text_area_selected_text(demo.ids.sql_area)
                        .as_deref(),
                    Some("report")
                );
                assert_eq!(demo.ui.table_selected_row(demo.ids.sql_table), Some(99999));
                assert_eq!(
                    demo.ui.table_selection_anchor(demo.ids.sql_table),
                    Some((99997, Some(0)))
                );
                assert_eq!(
                    demo.ui.table_column_widths(demo.ids.sql_table).unwrap()[1],
                    250.0
                );
                assert!(Arc::ptr_eq(
                    &values,
                    demo.state
                        .sql_result
                        .as_ref()
                        .unwrap()
                        .all_values
                        .as_ref()
                        .unwrap()
                ));
                capture(
                    &mut demo,
                    &format!("desktop-sql-split-{stage}-{name}-{width}"),
                );
            }
            assert!(bounds(&demo, demo.ids.sql_area).height > original.height);
            let preferred = demo.state.sql_ratio;
            demo.state.tab = MainTab::Browse;
            demo.do_rebuild();
            demo.state.tab = MainTab::ExecuteSql;
            demo.do_rebuild();
            assert!(
                (demo.ui.splitter_ratio(demo.ids.sql_splitter).unwrap() - preferred).abs() < 0.001
            );
            demo.ui.set_focus(Some(demo.ids.sql_area));
            assert!(demo.ui.key_input(UiKey::Undo));
            assert_eq!(demo.ui.text_area_value(demo.ids.sql_area), Some(query));
            let project = crate::project::ProjectFile::from_state(&demo.state);
            let dir = tempfile::tempdir().unwrap();
            for extension in ["json", "sqbpro"] {
                let path = dir.path().join(format!("desktop.{extension}"));
                project.save(&path).unwrap();
                let loaded = crate::project::ProjectFile::load(&path).unwrap();
                assert!((loaded.sql_ratio - preferred).abs() < 0.001);
            }
            let mut project = project;
            project.sql_ratio = f32::NAN;
            project.apply_to(&mut demo.state);
            assert_eq!(demo.state.sql_ratio, 0.5);
        }
    }
}
