use next_gui_ui::{
    AutocompleteState, AutocompleteStyle, FlexBuilder, RowStyle, SelectStyle, TextInputState,
    TextInputStyle, Theme, Ui, WidgetId,
};

use crate::{
    db,
    state::AppState,
    table_sql::{self, ForeignKeyReference},
};

#[derive(Debug, Clone)]
pub struct Draft {
    pub value: ForeignKeyReference,
    initial: ForeignKeyReference,
    raw: String,
    tables: Vec<(String, Vec<String>)>,
    columns: Option<AutocompleteState>,
    clauses: Option<TextInputState>,
    focus: Option<usize>,
}

impl Draft {
    pub fn new(state: &AppState, raw: &str) -> Result<Self, String> {
        let value = table_sql::foreign_key_reference(raw)?;
        let mut tables: Vec<(String, Vec<String>)> = vec![(
            state.wizard_table_name.clone(),
            state
                .wizard_cols
                .iter()
                .map(|column| db::quote_ident(&column.name))
                .collect(),
        )];
        for key in &state.wizard_index_constraints {
            if let Ok(columns) = table_sql::index_constraint_columns(key) {
                let names = columns
                    .into_iter()
                    .map(|(name, _)| db::quote_ident(&name))
                    .collect::<Vec<_>>()
                    .join(", ");
                if !tables[0].1.contains(&names) {
                    tables[0].1.push(names);
                }
            }
        }
        if let Some(conn) = &state.conn {
            for object in db::list_schema_in(conn, &state.wizard_schema)? {
                if object.kind != db::ObjectKind::Table
                    || object.name.eq_ignore_ascii_case(&state.wizard_table_name)
                    || state
                        .wizard_table_original
                        .as_ref()
                        .is_some_and(|original| original.name.eq_ignore_ascii_case(&object.name))
                {
                    continue;
                }
                let mut choices =
                    db::table_visible_columns_in(conn, &state.wizard_schema, &object.name)?
                        .iter()
                        .map(|column| db::quote_ident(&column.name))
                        .collect::<Vec<_>>();
                if let Some(sql) = object.sql {
                    for key in table_sql::table_index_constraints(&sql).unwrap_or_default() {
                        let columns = table_sql::index_constraint_columns(&key)?
                            .into_iter()
                            .map(|(name, _)| db::quote_ident(&name))
                            .collect::<Vec<_>>()
                            .join(", ");
                        if !choices.contains(&columns) {
                            choices.push(columns);
                        }
                    }
                }
                tables.push((object.name, choices));
            }
        }
        if !value.table.is_empty() {
            if let Some((name, _)) = tables
                .iter_mut()
                .find(|(name, _)| name.eq_ignore_ascii_case(&value.table))
            {
                *name = value.table.clone();
            } else {
                tables.push((value.table.clone(), Vec::new()));
            }
        }
        Ok(Self {
            initial: value.clone(),
            value,
            raw: raw.into(),
            tables,
            columns: None,
            clauses: None,
            focus: None,
        })
    }

    pub fn sql(&self) -> String {
        if self.value == self.initial {
            self.raw.clone()
        } else {
            self.value.sql()
        }
    }

    pub fn snapshot(&mut self, ui: &mut Ui, controls: Controls) -> bool {
        let table = ui
            .select_selected(controls.parent)
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| self.tables.get(index))
            .map(|(name, _)| name.clone())
            .unwrap_or_default();
        let changed = table != self.value.table;
        let had_columns = !self.value.columns.is_empty();
        self.value.columns = ui
            .autocomplete_value(controls.columns)
            .unwrap_or(&self.value.columns)
            .into();
        self.value.suffix = ui
            .text_input_value(controls.clauses)
            .unwrap_or(&self.value.suffix)
            .into();
        if had_columns && self.value.columns.is_empty() && ui.focused() == Some(controls.columns) {
            ui.close_popup();
        }
        self.columns = ui.autocomplete_state(controls.columns);
        self.clauses = ui.text_input_state(controls.clauses);
        self.focus = controls
            .order()
            .iter()
            .position(|id| Some(*id) == ui.focused());
        if changed {
            self.value.table = table;
            self.value.columns.clear();
            self.columns = None;
        }
        changed
    }

    pub fn reset(&mut self) {
        self.value = ForeignKeyReference::default();
        self.columns = None;
        self.clauses = None;
        self.focus = Some(0);
    }
}

#[derive(Clone, Copy)]
pub struct Controls {
    pub root: WidgetId,
    pub parent: WidgetId,
    pub columns: WidgetId,
    pub clauses: WidgetId,
    pub reset: WidgetId,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            root: WidgetId(0),
            parent: WidgetId(0),
            columns: WidgetId(0),
            clauses: WidgetId(0),
            reset: WidgetId(0),
        }
    }
}
impl Controls {
    pub fn order(self) -> [WidgetId; 4] {
        [self.parent, self.columns, self.clauses, self.reset]
    }
    pub fn focus(self, ui: &mut Ui, index: usize) {
        if let Some(id) = self.order().get(index) {
            ui.set_focus(Some(*id));
        }
    }
}

pub fn build(
    c: &mut FlexBuilder<'_>,
    table: WidgetId,
    (row, column): (usize, usize),
    width: f32,
    reset_width: f32,
    draft: &Draft,
    theme: &Theme,
) -> Controls {
    let mut controls = Controls::default();
    let width = (width - 2.0).max(1.0);
    let reset = reset_width.min(width);
    let remaining = width - reset;
    let parent = remaining * 0.30;
    let target = remaining * 0.30;
    let clauses = (remaining - parent - target).max(1.0);
    let choices = draft
        .tables
        .iter()
        .find(|(name, _)| name == &draft.value.table)
        .map(|(_, choices)| choices.clone())
        .unwrap_or_default();
    controls.root = c
        .table_cell_editor(table, row, column, |cell| {
            cell.row(
                RowStyle {
                    width,
                    height: 24.0,
                    padding: 0.0,
                    gap: 0.0,
                    background: None,
                    ..Default::default()
                },
                |r| {
                    controls.parent = r.select(
                        std::iter::once(String::new())
                            .chain(draft.tables.iter().map(|(name, _)| name.clone())),
                        Some(
                            draft
                                .tables
                                .iter()
                                .position(|(name, _)| name == &draft.value.table)
                                .map_or(0, |index| index + 1),
                        ),
                        "Parent table",
                        SelectStyle {
                            width: parent,
                            ..SelectStyle::from_theme(theme)
                        },
                    );
                    controls.columns = r.autocomplete_placeholder(
                        choices,
                        &draft.value.columns,
                        "Primary key",
                        AutocompleteStyle {
                            width: target,
                            ..AutocompleteStyle::from_theme(theme)
                        },
                    );
                    controls.clauses = r.text_input_styled(
                        &draft.value.suffix,
                        "ON UPDATE / DELETE",
                        TextInputStyle {
                            width: clauses,
                            ..TextInputStyle::from_theme(theme)
                        },
                    );
                    controls.reset = r.tool_button("Reset");
                },
            )
        })
        .unwrap_or(WidgetId(0));
    controls
}

pub fn restore(ui: &mut Ui, controls: Controls, draft: &Draft) {
    if let Some(session) = &draft.columns {
        ui.set_autocomplete_state(controls.columns, session);
    }
    if let Some(session) = &draft.clauses {
        ui.restore_text_input_state(controls.clauses, session);
    }
    ui.set_accessible_name(controls.parent, "Foreign key parent table");
    ui.set_accessible_name(
        controls.columns,
        "Referenced columns, in order; empty uses the parent primary key",
    );
    ui.set_accessible_name(
        controls.clauses,
        "Foreign key ON UPDATE, ON DELETE and deferred clauses",
    );
    ui.set_accessible_name(controls.reset, "Reset this foreign key");
    controls.focus(ui, draft.focus.unwrap_or(0));
}

#[cfg(test)]
mod tests {
    use crate::{ui_build, Demo, PresentBackend, Theme, UiKey};

    fn finish(demo: &mut Demo) {
        demo.handle_table_cell_edit();
        if demo.rebuild {
            demo.do_rebuild();
        }
        demo.snapshot_inputs();
        if demo.rebuild {
            demo.do_rebuild();
        }
        demo.ui.sync(&mut demo.scene);
    }

    fn begin(demo: &mut Demo, row: usize) {
        demo.ui
            .set_table_selected_row(demo.ids.ov_fks_table, Some(row));
        demo.ui
            .set_table_selected_col(demo.ids.ov_fks_table, Some(2));
        demo.ui.set_focus(Some(demo.ids.ov_fks_table));
        demo.snapshot_inputs();
        if demo.rebuild {
            demo.do_rebuild();
        }
        demo.ui.set_focus(Some(demo.ids.ov_fks_table));
        assert!(demo.ui.key_input(UiKey::Edit));
        finish(demo);
        assert_eq!(demo.ui.focused(), Some(demo.ids.reference_editor.parent));
    }

    fn paint(demo: &mut Demo, theme: &str, name: &str) {
        let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_REFERENCE_QA_OUT") else {
            return;
        };
        use next_gui_render::Renderer;
        let (w, h) = (demo.win_w, demo.win_h);
        demo.scene.mark_full_dirty(w as f32, h as f32);
        let dirty = demo.scene.take_dirty();
        let commands = crate::encode_scene(&demo.scene, &dirty);
        let clear = crate::clear_color(&demo.theme);
        let mut soft = crate::SoftRenderer::new(w, h, clear);
        soft.render(&commands, &dirty);
        crate::write_ppm(
            &format!("{prefix}-{theme}-{name}.ppm"),
            soft.frame_pixels().unwrap().0,
            w,
            h,
        )
        .unwrap();
        if let Ok(mut gpu) = crate::GpuRenderer::try_new(w, h, clear) {
            gpu.render(&commands, &dirty);
            let delta = soft
                .frame_pixels()
                .unwrap()
                .0
                .iter()
                .zip(gpu.frame_pixels().unwrap().0)
                .flat_map(|(a, b)| {
                    [0, 8, 16].map(|shift| {
                        (((a >> shift) & 255) as i32 - ((b >> shift) & 255) as i32).abs()
                    })
                })
                .max()
                .unwrap();
            assert!(delta <= 2, "{theme} {name}: {delta}");
            eprintln!("Reference editor {theme} {name}: Soft/Metal delta {delta}");
        } else {
            eprintln!("Reference editor GPU comparison not_run: adapter unavailable");
        }
    }

    #[test]
    fn reference_reset_removes_only_its_constraint_and_keeps_named_deferrals_on_cancel() {
        let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("PRAGMA foreign_keys=ON;
            CREATE TABLE parents(id INTEGER PRIMARY KEY);INSERT INTO parents VALUES(1);
            CREATE TABLE records(id INTEGER PRIMARY KEY,a INTEGER CONSTRAINT first REFERENCES PARENTS(id)
                CONSTRAINT later DEFERRABLE INITIALLY DEFERRED CHECK(a>0) CONSTRAINT second REFERENCES parents);
            INSERT INTO records VALUES(7,1);").unwrap();
        demo.state.refresh_schema().unwrap();
        let index = demo
            .state
            .schema
            .iter()
            .position(|object| object.name == "records")
            .unwrap();
        demo.state.select_schema_node(10_000 + index as u64);
        demo.state.open_modify_table_wizard().unwrap();
        demo.state.wizard_table_page = 2;
        demo.do_rebuild();
        let original = demo.state.wizard_foreign_keys.clone();
        assert_eq!(original.len(), 2);
        begin(&mut demo, 0);
        demo.ui.key_input(UiKey::Enter);
        finish(&mut demo);
        assert_eq!(demo.state.wizard_foreign_keys, original);
        begin(&mut demo, 0);
        demo.handle_button(demo.ids.reference_editor.reset);
        finish(&mut demo);
        demo.ui.key_input(UiKey::Escape);
        finish(&mut demo);
        assert_eq!(demo.state.wizard_foreign_keys, original);
        begin(&mut demo, 0);
        demo.ui.set_focus(Some(demo.ids.reference_editor.clauses));
        demo.ui.key_input(UiKey::SelectAll);
        demo.ui.paste_into_focused_text(
            "ON UPDATE CASCADE CONSTRAINT later DEFERRABLE INITIALLY DEFERRED",
        );
        finish(&mut demo);
        assert!(demo.state.wizard_foreign_keys[0]
            .clause
            .contains("CONSTRAINT later"));
        demo.ui.key_input(UiKey::Enter);
        finish(&mut demo);
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
        demo.state
            .conn
            .as_ref()
            .unwrap()
            .execute("UPDATE parents SET id=2", [])
            .unwrap();
        assert_eq!(
            demo.state
                .conn
                .as_ref()
                .unwrap()
                .query_row("SELECT a FROM records", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            2
        );
        demo.state.open_modify_table_wizard().unwrap();
        demo.state.wizard_table_page = 2;
        demo.do_rebuild();
        let second = demo.state.wizard_foreign_keys[1].clone();
        begin(&mut demo, 0);
        demo.handle_button(demo.ids.reference_editor.reset);
        finish(&mut demo);
        demo.ids.reference_editor.focus(&mut demo.ui, 3);
        demo.ui.key_input(UiKey::Tab);
        finish(&mut demo);
        assert!(demo.state.wizard_column_picker.is_some());
        assert_eq!(demo.state.wizard_column_picker.as_ref().unwrap().row, 0);
        demo.handle_column_picker_button(demo.ids.column_picker.cancel);
        demo.do_rebuild();
        assert_eq!(demo.state.wizard_foreign_keys, vec![second.clone()]);
        assert_eq!(demo.state.wizard_cols[1].foreign_key_clause, second.clause);
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
        let conn = demo.state.conn.as_ref().unwrap();
        let sql = conn
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE name='records'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap();
        assert_eq!(
            crate::table_sql::inline_foreign_keys(&sql).unwrap(),
            vec![second]
        );
        assert!(sql.contains("CHECK"));
        assert!(conn.execute("UPDATE records SET a=99", []).is_err());
    }

    #[test]
    fn reference_cell_invalid_sql_keeps_the_database_and_draft_until_repaired() {
        let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("PRAGMA foreign_keys=ON;
            CREATE TABLE parents(id INTEGER PRIMARY KEY); INSERT INTO parents VALUES(1);
            CREATE TABLE records(id INTEGER PRIMARY KEY,a INTEGER REFERENCES parents(id)); INSERT INTO records VALUES(7,1);").unwrap();
        demo.state.refresh_schema().unwrap();
        let index = demo
            .state
            .schema
            .iter()
            .position(|object| object.name == "records")
            .unwrap();
        demo.state.select_schema_node(10_000 + index as u64);
        demo.state.open_modify_table_wizard().unwrap();
        demo.state.wizard_table_page = 2;
        demo.do_rebuild();
        begin(&mut demo, 0);
        demo.ui.set_focus(Some(demo.ids.reference_editor.clauses));
        demo.ui
            .paste_into_focused_text("ON DELETE CASCADE); DROP TABLE parents;--");
        finish(&mut demo);
        assert!(demo
            .ui
            .text_area_value(demo.ids.ov_table_preview)
            .unwrap()
            .starts_with("--"));
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.is_some());
        assert_eq!(demo.state.overlay, Some(crate::OverlayKind::EditTable));
        assert!(demo.state.wizard_foreign_keys[0]
            .clause
            .contains("DROP TABLE"));
        let conn = demo.state.conn.as_ref().unwrap();
        assert_eq!(
            conn.query_row("SELECT a FROM records WHERE id=7", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM parents", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        if demo.rebuild {
            demo.do_rebuild();
        }
        demo.ui.key_input(UiKey::Undo);
        finish(&mut demo);
        assert!(!demo
            .ui
            .text_area_value(demo.ids.ov_table_preview)
            .unwrap()
            .starts_with("--"));
        demo.ui.key_input(UiKey::Enter);
        finish(&mut demo);
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
    }

    #[test]
    fn reference_reset_then_pointer_row_change_keeps_the_clicked_constraint_identity() {
        let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
        demo.state.new_memory_db().unwrap();
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE parents(id INTEGER PRIMARY KEY);
            CREATE TABLE records(a INTEGER,CONSTRAINT first FOREIGN KEY(a) REFERENCES parents,
                CONSTRAINT second FOREIGN KEY(a) REFERENCES parents,CONSTRAINT third FOREIGN KEY(a) REFERENCES parents);").unwrap();
        demo.state.refresh_schema().unwrap();
        let index = demo
            .state
            .schema
            .iter()
            .position(|object| object.name == "records")
            .unwrap();
        demo.state.select_schema_node(10_000 + index as u64);
        demo.state.open_modify_table_wizard().unwrap();
        demo.state.wizard_table_page = 2;
        demo.do_rebuild();
        begin(&mut demo, 0);
        demo.handle_button(demo.ids.reference_editor.reset);
        finish(&mut demo);
        let table = demo
            .scene
            .iter_paint_order()
            .into_iter()
            .find(|node| demo.ui.widget_from_node(node.id) == Some(demo.ids.ov_fks_table))
            .unwrap()
            .bounds;
        let style = next_gui_ui::TableStyle::from_theme(&demo.theme);
        assert!(demo.ui.pointer_pressed(
            &mut demo.scene,
            table.x + 270.0,
            table.y + style.header_height + style.row_height * 1.5
        ));
        demo.ui.pointer_released();
        demo.snapshot_inputs();
        finish(&mut demo);
        assert_eq!(
            demo.state
                .wizard_foreign_keys
                .iter()
                .map(|key| key.name.as_deref().unwrap())
                .collect::<Vec<_>>(),
            ["second", "third"]
        );
        assert_eq!(demo.state.wizard_foreign_key_sel, 0);
        assert_eq!(
            demo.ui.text_input_value(demo.ids.ov_fk_name),
            Some("second")
        );
        demo.handle_button(demo.ids.ov_btn_ok);
        assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
    }

    #[test]
    fn reference_cells_keep_schema_identity_composites_reset_cancel_and_real_cascades() {
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
            demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
            demo.state.new_memory_db().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("PRAGMA foreign_keys=ON;
                CREATE TABLE parents(wrong_main_column);CREATE TEMP TABLE parents(wrong_temp_column);
                ATTACH ':memory:' AS analytics;
                CREATE TABLE analytics.parents(p TEXT,q INTEGER,PRIMARY KEY(p,q)) WITHOUT ROWID;
                CREATE TABLE analytics.other(p TEXT,q INTEGER,UNIQUE(q,p));
                INSERT INTO analytics.parents VALUES('west',1);
                CREATE TABLE analytics.records(id INTEGER PRIMARY KEY,a TEXT,b INTEGER,
                    CONSTRAINT pair FOREIGN KEY(a,b) REFERENCES parents(p,q) ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED);
                INSERT INTO analytics.records VALUES(7,'west',1);
                CREATE VIEW analytics.visible AS SELECT id,a,b FROM records;").unwrap();
            demo.state.refresh_schema().unwrap();
            let index = demo
                .state
                .schema
                .iter()
                .position(|object| object.schema == "analytics" && object.name == "records")
                .unwrap();
            demo.state.select_schema_node(10_000 + index as u64);
            demo.state.open_modify_table_wizard().unwrap();
            demo.state.wizard_table_page = 2;
            demo.do_rebuild();
            let original = demo.state.wizard_foreign_keys[0].clone();
            begin(&mut demo, 0);
            let draft = demo
                .state
                .wizard_constraint_edit
                .as_ref()
                .unwrap()
                .reference
                .as_ref()
                .unwrap()
                .clone();
            assert!(draft
                .tables
                .iter()
                .find(|(name, _)| name == "parents")
                .unwrap()
                .1
                .iter()
                .all(|name| !name.contains("wrong")));
            assert_eq!(draft.value.columns, "p, q");
            paint(&mut demo, theme_name, "reference");
            demo.ui.key_input(UiKey::Space);
            demo.ui.sync(&mut demo.scene);
            assert!(demo.ui.has_open_popup());
            paint(&mut demo, theme_name, "tables");
            demo.ui.key_input(UiKey::Escape);
            finish(&mut demo);
            assert!(draft
                .tables
                .iter()
                .find(|(name, _)| name == "parents")
                .unwrap()
                .1
                .contains(&"\"p\", \"q\"".into()));
            demo.ui.key_input(UiKey::Enter);
            finish(&mut demo);
            assert_eq!(demo.state.wizard_foreign_keys[0], original);

            begin(&mut demo, 0);
            demo.handle_button(demo.ids.reference_editor.reset);
            finish(&mut demo);
            assert_eq!(demo.state.wizard_foreign_keys[0].clause, "");
            demo.ui.key_input(UiKey::Escape);
            finish(&mut demo);
            assert_eq!(demo.state.wizard_foreign_keys[0], original);
            begin(&mut demo, 0);
            let other = demo
                .state
                .wizard_constraint_edit
                .as_ref()
                .unwrap()
                .reference
                .as_ref()
                .unwrap()
                .tables
                .iter()
                .position(|(name, _)| name == "other")
                .unwrap()
                + 1;
            demo.ui
                .set_select_selected(demo.ids.reference_editor.parent, Some(other));
            finish(&mut demo);
            assert_eq!(
                demo.ui
                    .autocomplete_value(demo.ids.reference_editor.columns),
                Some("")
            );
            assert!(demo.state.wizard_foreign_keys[0]
                .clause
                .contains("ON DELETE SET NULL"));
            demo.ui.key_input(UiKey::Escape);
            finish(&mut demo);
            assert_eq!(demo.state.wizard_foreign_keys[0], original);

            begin(&mut demo, 0);
            demo.ui.key_input(UiKey::Tab);
            demo.ui.key_input(UiKey::Tab);
            demo.ui.key_input(UiKey::SelectAll);
            demo.ui.paste_into_focused_text(
                "ON UPDATE CASCADE ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED",
            );
            finish(&mut demo);
            demo.win_w = 800;
            demo.win_h = 600;
            demo.do_rebuild();
            assert_eq!(demo.ui.focused(), Some(demo.ids.reference_editor.clauses));
            paint(&mut demo, theme_name, "small");
            let reset = demo
                .scene
                .iter_paint_order()
                .into_iter()
                .find(|node| {
                    demo.ui.widget_from_node(node.id) == Some(demo.ids.reference_editor.reset)
                })
                .unwrap()
                .bounds;
            assert_eq!(
                demo.ui.hit_widget(
                    &demo.scene,
                    reset.x + reset.width / 2.0,
                    reset.y + reset.height / 2.0
                ),
                Some(demo.ids.reference_editor.reset)
            );
            demo.ui.key_input(UiKey::Undo);
            finish(&mut demo);
            assert_eq!(demo.state.wizard_foreign_keys[0], original);
            demo.ui.key_input(UiKey::Redo);
            finish(&mut demo);
            demo.ui.set_focus(Some(demo.ids.reference_editor.columns));
            demo.ui.key_input(UiKey::SelectAll);
            demo.ui.key_input(UiKey::Delete);
            finish(&mut demo);
            assert!(demo
                .state
                .wizard_constraint_edit
                .as_ref()
                .unwrap()
                .reference
                .as_ref()
                .unwrap()
                .value
                .columns
                .is_empty());
            demo.ui.key_input(UiKey::Enter);
            finish(&mut demo);
            demo.handle_button(demo.ids.ov_btn_ok);
            assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
            let conn = demo.state.conn.as_ref().unwrap();
            assert_eq!(
                conn.query_row("SELECT id FROM analytics.visible", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                7
            );
            conn.execute("UPDATE analytics.parents SET p='east',q=2", [])
                .unwrap();
            assert_eq!(
                conn.query_row("SELECT a,b FROM analytics.records", [], |row| Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?
                )))
                .unwrap(),
                ("east".into(), 2)
            );
            conn.execute("DELETE FROM analytics.parents", []).unwrap();
            assert!(conn
                .query_row(
                    "SELECT a IS NULL AND b IS NULL FROM analytics.records",
                    [],
                    |row| row.get::<_, bool>(0)
                )
                .unwrap());
        }
    }
}
