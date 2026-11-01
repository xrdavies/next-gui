//! The result grid and copyable execution feedback keep independent viewports.
use crate::state::AppState;
use next_gui_layout::Axis;
use next_gui_ui::{
    FlexBuilder, SplitterStyle, TableColumn, TableStyle, TextAreaStyle, Theme, Ui, WidgetId,
};

#[derive(Clone, Copy)]
pub struct Controls {
    pub splitter: WidgetId,
    pub area: WidgetId,
    pub toggle: WidgetId,
    pub error: WidgetId,
    ratio: Option<f32>,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            splitter: WidgetId(0),
            area: WidgetId(0),
            toggle: WidgetId(0),
            error: WidgetId(0),
            ratio: None,
        }
    }
}

impl Controls {
    pub fn snapshot(&mut self, ui: &Ui, state: &mut AppState) {
        if let Some(ratio) = ui.splitter_ratio(self.splitter) {
            if Some(ratio) != self.ratio {
                state.sql_messages_ratio = ratio;
            }
            self.ratio = Some(ratio);
        }
        if let Some(view) = ui.text_area_state(self.area) {
            state.sql_messages_view = Some(view);
        }
    }

    pub fn configure(&mut self, ui: &mut Ui, state: &AppState) {
        ui.set_accessible_name(self.area, "SQL execution messages");
        ui.set_text_area_read_only(self.area, true);
        ui.set_accessible_name(self.splitter, "Resize query results and execution messages");
        ui.set_splitter_min_sizes(self.splitter, 96.0, 64.0);
        ui.set_flex_fill(self.splitter, true);
        self.ratio = ui.splitter_ratio(self.splitter);
        let restored = state
            .sql_messages_view
            .as_ref()
            .is_some_and(|view| ui.restore_text_area_state(self.area, view));
        if !restored {
            ui.select_text_area_range(self.area, 0..0);
        }
    }
}

pub fn build(
    p: &mut FlexBuilder<'_>,
    state: &AppState,
    theme: &Theme,
    columns: Vec<TableColumn>,
    rows: Vec<Vec<String>>,
    table_style: TableStyle,
) -> (Controls, WidgetId) {
    let mut controls = Controls::default();
    let mut table = WidgetId(0);
    let mut grid = |p: &mut FlexBuilder<'_>| {
        table = if let Some(values) = state
            .sql_result
            .as_ref()
            .and_then(|result| result.all_values.clone())
        {
            p.data_table(columns.clone(), values, table_style)
        } else {
            p.table(columns.clone(), rows.clone(), table_style)
        };
    };
    if state.sql_messages_visible {
        let mut split = SplitterStyle::from_theme(theme);
        split.width = table_style.width;
        split.height = table_style.height;
        split.axis = Axis::Horizontal;
        split.ratio = if state.sql_messages_ratio.is_finite() {
            state.sql_messages_ratio
        } else {
            0.65
        };
        let mut style = TextAreaStyle::from_theme(theme);
        style.width = table_style.width;
        style.height = 100.0;
        if state.error_msg.is_some()
            || state
                .sql_message
                .starts_with("Execution finished with errors.")
        {
            style.border = theme.colors.danger;
            style.fg = theme.colors.danger;
        }
        controls.splitter = p.splitter(split, &mut grid, |p| {
            controls.area = p.text_area(&state.sql_message, "Execution messages", style);
        });
    } else {
        grid(p);
    }
    (controls, table)
}

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
    fn finish(demo: &mut crate::Demo) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while demo.state.sql_busy {
            assert!(Instant::now() < deadline);
            demo.state.poll_sql_job();
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn desktop_sql_messages_resize_copy_and_keep_pending_edits_and_tab_sessions() {
        for (name, theme, width, height) in [
            ("dark", Theme::dark(), 1600, 900),
            ("light", Theme::light(), 1920, 927),
        ] {
            let mut demo = fixture(width, height, theme);
            demo.state.write_changes().unwrap();
            crate::desktop_workflow::prepare(&mut demo.state, "desktop-sql").unwrap();
            finish(&mut demo);
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("orders.sqlite");
            demo.state.save_as(path.to_str().unwrap()).unwrap();
            let sql = crate::desktop_workflow::DELIVERY_UPDATE;
            demo.state.sql_tabs[0] = sql.into();
            demo.state.start_sql_job().unwrap();
            finish(&mut demo);
            assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
            assert!(
                demo.state
                    .sql_message
                    .contains("4 rows returned; 4 rows affected"),
                "{}",
                demo.state.sql_message
            );
            assert!(demo.state.dirty);
            let reader = crate::db::open_readonly(path.to_str().unwrap()).unwrap();
            assert_eq!(
                reader
                    .query_row("SELECT shipping FROM orders WHERE id=100000", [], |r| r
                        .get::<_, f64>(0))
                    .unwrap(),
                4.95
            );
            let values = demo
                .state
                .sql_result
                .as_ref()
                .unwrap()
                .all_values
                .clone()
                .unwrap();
            demo.do_rebuild();
            let area = demo.ids.sql_messages.area;
            assert!(bounds(&demo, area).height >= 64.0);
            assert_eq!(
                demo.ui.text_area_scroll_offsets(area),
                Some((0.0, 0.0)),
                "new execution must show its outcome before the SQL source"
            );
            assert_eq!(demo.ui.text_area_selection(area), Some(0..0));
            demo.ui.set_focus(Some(area));
            demo.ui.key_input(UiKey::SelectAll);
            assert_eq!(
                demo.ui.focused_text_for_copy().as_deref(),
                Some(demo.state.sql_message.as_str())
            );
            for key in [UiKey::Char('x'), UiKey::Delete, UiKey::Backspace] {
                demo.ui.key_input(key);
            }
            demo.ui.paste_into_focused_text("unsafe");
            demo.ui.ime_commit("替换");
            assert_eq!(
                demo.ui.text_area_value(area),
                Some(demo.state.sql_message.as_str())
            );
            demo.snapshot_inputs();
            demo.do_rebuild();
            assert_eq!(demo.ui.focused(), Some(demo.ids.sql_messages.area));
            assert_eq!(
                demo.ui.focused_text_for_copy().as_deref(),
                Some(demo.state.sql_message.as_str())
            );
            let grid = bounds(&demo, demo.ids.sql_table);
            let x = grid.x + grid.width * 0.5;
            let y = grid.y + grid.height + 3.0;
            assert_eq!(
                demo.ui.hit_widget(&demo.scene, x, y),
                Some(demo.ids.sql_messages.splitter)
            );
            assert!(demo.ui.pointer_pressed(&mut demo.scene, x, y));
            assert!(demo.ui.pointer_moved(x, y - 55.0));
            demo.ui.pointer_released();
            demo.ui.sync(&mut demo.scene);
            assert!(bounds(&demo, demo.ids.sql_table).height < grid.height);
            demo.snapshot_inputs();
            let ratio = demo.state.sql_messages_ratio;
            demo.do_rebuild();
            assert!(
                (demo
                    .ui
                    .splitter_ratio(demo.ids.sql_messages.splitter)
                    .unwrap()
                    - ratio)
                    .abs()
                    < 0.001
            );
            capture(&mut demo, &format!("desktop-sql-messages-{name}-{width}"));
            let before = bounds(&demo, demo.ids.sql_table).height;
            demo.handle_button(demo.ids.sql_messages.toggle);
            demo.do_rebuild();
            assert!(!demo.state.sql_messages_visible);
            assert_eq!(demo.ids.sql_messages.area, WidgetId(0));
            assert!(bounds(&demo, demo.ids.sql_table).height > before + 60.0);
            demo.handle_button(demo.ids.sql_messages.toggle);
            demo.do_rebuild();
            assert!(demo.state.sql_messages_visible);
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
            demo.ui.set_focus(Some(demo.ids.sql_messages.area));
            assert_eq!(
                demo.ui.focused_text_for_copy().as_deref(),
                Some(demo.state.sql_message.as_str())
            );
            demo.state
                .sql_tabs
                .push("SELECT missing_column FROM orders;".into());
            demo.snapshot_inputs();
            demo.state.switch_sql_tab(1);
            demo.state.start_sql_job().unwrap();
            finish(&mut demo);
            demo.do_rebuild();
            assert!(demo
                .state
                .sql_message
                .starts_with("Execution finished with errors."));
            assert!(demo
                .state
                .sql_message
                .contains("Statement 1: preparation failed"));
            assert_eq!(
                demo.ui.text_area_scroll_offsets(demo.ids.sql_messages.area),
                Some((0.0, 0.0))
            );
            capture(&mut demo, &format!("desktop-sql-error-{name}-{width}"));
            demo.state.switch_sql_tab(0);
            demo.do_rebuild();
            assert!(demo.state.sql_message.contains("4 rows affected"));
            demo.ui.set_focus(Some(demo.ids.sql_messages.area));
            assert_eq!(
                demo.ui.focused_text_for_copy().as_deref(),
                Some(demo.state.sql_message.as_str())
            );
            demo.state.write_changes().unwrap();
            assert_eq!(
                reader
                    .query_row("SELECT shipping FROM orders WHERE id=100000", [], |r| r
                        .get::<_, f64>(0))
                    .unwrap(),
                5.95
            );
            assert_eq!(
                reader
                    .query_row(
                        "SELECT count(*) FROM orders WHERE note='Priority delivery revised'",
                        [],
                        |r| r.get::<_, usize>(0)
                    )
                    .unwrap(),
                4
            );
            let project = crate::project::ProjectFile::from_state(&demo.state);
            for extension in ["json", "sqbpro"] {
                let path = dir.path().join(format!("desktop.{extension}"));
                project.save(&path).unwrap();
                let loaded = crate::project::ProjectFile::load(&path).unwrap();
                assert!((loaded.sql_messages_ratio - ratio).abs() < 0.001);
                assert!(loaded.sql_messages_visible);
            }
            let mut project = project;
            project.sql_messages_ratio = f32::NAN;
            project.sql_messages_visible = false;
            project.apply_to(&mut demo.state);
            assert_eq!(demo.state.sql_messages_ratio, 0.65);
            assert!(!demo.state.sql_messages_visible);
            demo.state.tab = MainTab::Browse;
        }
    }
}
