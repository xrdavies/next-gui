use next_gui_scene::Color;
use next_gui_ui::{ColumnStyle, ListStyle, ModalStyle, RowStyle, Theme, Ui, WidgetId};

use crate::{
    db,
    state::{AppState, OverlayKind},
    table_sql, Demo,
};

#[derive(Debug, Clone)]
pub struct Draft {
    pub page: usize,
    pub row: usize,
    pub available: Vec<(String, String)>,
    pub selected: Vec<(String, String)>,
    original: Vec<(String, String)>,
    pub selection: [Option<usize>; 2],
    scroll: [f32; 2],
    body_scroll: (f32, f32),
    focus: Option<usize>,
    pub error: Option<String>,
}

impl Draft {
    pub fn new(state: &AppState, page: usize, row: usize) -> Result<Self, String> {
        if state.overlay != Some(OverlayKind::EditTable) || !state.table_wizard_names_valid() {
            return Err("Finish the table and column names before choosing columns".into());
        }
        let selected = match page {
            0 => table_sql::index_constraint_columns(
                state
                    .wizard_index_constraints
                    .get(row)
                    .ok_or("Select an index constraint")?,
            )?,
            1 => table_sql::foreign_key_columns(
                state
                    .wizard_foreign_keys
                    .get(row)
                    .ok_or("Select a foreign key")?,
            )?,
            _ => return Err("This constraint has no column list".into()),
        };
        if selected.iter().any(|(name, _)| {
            !state
                .wizard_cols
                .iter()
                .any(|column| column.name.eq_ignore_ascii_case(name))
        }) {
            return Err("Correct unknown constraint columns before choosing columns".into());
        }
        let available: Vec<_> = state
            .wizard_cols
            .iter()
            .filter(|column| {
                !selected
                    .iter()
                    .any(|(name, _)| column.name.eq_ignore_ascii_case(name))
            })
            .map(|column| (column.name.clone(), db::quote_ident(&column.name)))
            .collect();
        Ok(Self {
            page,
            row,
            selection: [
                (!available.is_empty()).then_some(0),
                (!selected.is_empty()).then_some(0),
            ],
            available,
            original: selected.clone(),
            selected,
            scroll: [0.0; 2],
            body_scroll: (0.0, 0.0),
            focus: None,
            error: None,
        })
    }

    fn transfer(&mut self, from: usize) {
        let Some(index) = self.selection[from] else {
            return;
        };
        let (source, target) = if from == 0 {
            (&mut self.available, &mut self.selected)
        } else {
            (&mut self.selected, &mut self.available)
        };
        if index >= source.len() {
            return;
        }
        target.push(source.remove(index));
        self.selection[from] =
            (!source.is_empty()).then_some(index.min(source.len().saturating_sub(1)));
        self.selection[1 - from] = target.len().checked_sub(1);
        self.error = None;
    }

    fn move_selected(&mut self, down: bool) {
        let Some(index) = self.selection[1] else {
            return;
        };
        let target = if down {
            index.checked_add(1)
        } else {
            index.checked_sub(1)
        };
        if let Some(target) = target.filter(|&target| target < self.selected.len()) {
            self.selected.swap(index, target);
            self.selection[1] = Some(target);
            self.error = None;
        }
    }
}

#[derive(Clone, Copy)]
pub struct Controls {
    pub dialog: WidgetId,
    pub lists: [WidgetId; 2],
    pub add: WidgetId,
    pub remove: WidgetId,
    pub up: WidgetId,
    pub down: WidgetId,
    pub apply: WidgetId,
    pub cancel: WidgetId,
    pub error: WidgetId,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            dialog: WidgetId(0),
            lists: [WidgetId(0); 2],
            add: WidgetId(0),
            remove: WidgetId(0),
            up: WidgetId(0),
            down: WidgetId(0),
            apply: WidgetId(0),
            cancel: WidgetId(0),
            error: WidgetId(0),
        }
    }
}

impl Controls {
    fn focus_order(self) -> [WidgetId; 9] {
        [
            self.lists[0],
            self.add,
            self.remove,
            self.lists[1],
            self.up,
            self.down,
            self.apply,
            self.cancel,
            self.error,
        ]
    }
}

pub fn build(ui: &mut Ui, state: &AppState, theme: &Theme, width: f32) -> Controls {
    let Some(draft) = &state.wizard_column_picker else {
        return Controls::default();
    };
    let mut controls = Controls::default();
    let mut modal = ModalStyle::from_theme(theme);
    modal.width = 537.0;
    modal.height = 290.0;
    modal.padding = 12.0;
    modal.title_height = 36.0;
    modal.action_height = 30.0;
    modal.close_on_backdrop = true;
    modal.scrim = Color::rgba(
        0,
        0,
        0,
        if theme.mode == next_gui_ui::ThemeMode::Dark {
            110
        } else {
            64
        },
    );
    let body_width = modal.width.min((width - 40.0).max(width * 0.75)) - 2.0 * modal.padding;
    let density = ui.config().density;
    let button_size = (theme.typography.button_sm * density.font_scale())
        .round()
        .max(1.0);
    let button_width = |label| ui.font().measure_line(label, button_size).0.max(0.0) + 32.0;
    let minimum = (64.0 * density.spacing_scale()).round().max(1.0);
    let transfer_width = button_width("Add →")
        .max(button_width("← Remove"))
        .max(minimum);
    let order_width = button_width("↑ Up")
        .max(button_width("↓ Down"))
        .max(minimum);
    let list_width = ((body_width - transfer_width - order_width - 3.0 * 8.0) * 0.5).max(40.0);
    let mut row = RowStyle::from_theme(theme);
    row.width = body_width;
    row.height = if draft.error.is_some() { 112.0 } else { 154.0 };
    row.padding = 0.0;
    row.gap = 8.0;
    row.background = None;
    let mut column = ColumnStyle::from_theme(theme);
    column.width = list_width;
    column.height = row.height;
    column.padding = 0.0;
    column.gap = 6.0;
    column.background = None;
    let mut buttons = column;
    buttons.width = transfer_width;
    let mut order_buttons = buttons;
    order_buttons.width = order_width;
    let mut list = ListStyle::from_theme(theme);
    list.width = list_width;
    list.height = if draft.error.is_some() { 80.0 } else { 128.0 };
    list.item_height = 26.0;
    controls.dialog = ui.dialog(
        if draft.page == 0 {
            "Index columns"
        } else {
            "Foreign key columns"
        },
        modal,
        |body| {
            body.row(row, |r| {
                r.column(column, |c| {
                    c.text("Available");
                    controls.lists[0] = c.list(
                        draft
                            .available
                            .iter()
                            .map(|item| item.0.replace(['\r', '\n', '\t'], " ")),
                        list,
                    );
                });
                r.column(buttons, |c| {
                    c.text("");
                    controls.add = c.tool_button("Add →");
                    controls.remove = c.tool_button("← Remove");
                });
                r.column(column, |c| {
                    c.text("Selected");
                    controls.lists[1] = c.list(
                        draft
                            .selected
                            .iter()
                            .map(|item| item.0.replace(['\r', '\n', '\t'], " ")),
                        list,
                    );
                });
                r.column(order_buttons, |c| {
                    c.text("");
                    controls.up = c.tool_button("↑ Up");
                    controls.down = c.tool_button("↓ Down");
                });
            });
            if let Some(error) = &draft.error {
                let mut style = next_gui_ui::TextAreaStyle::from_theme(theme);
                style.width = body_width;
                style.height = 64.0;
                controls.error = body.text_area(error, "", style);
            }
        },
        |footer| {
            controls.apply =
                footer.tool_button_styled("Apply", theme.colors.accent, theme.colors.on_accent);
            controls.cancel = footer.tool_button("Cancel");
        },
    );
    ui.set_text_area_read_only(controls.error, true);
    ui.set_accessible_name(controls.error, "Column selection validation error");
    for (index, label) in [
        "Available constraint columns",
        "Selected constraint columns in order",
    ]
    .into_iter()
    .enumerate()
    {
        ui.set_accessible_name(controls.lists[index], label);
        ui.set_list_selected(controls.lists[index], draft.selection[index]);
        ui.set_list_activatable(controls.lists[index], true);
    }
    ui.set_button_enabled(controls.add, draft.selection[0].is_some());
    ui.set_button_enabled(controls.remove, draft.selection[1].is_some());
    ui.set_button_enabled(
        controls.up,
        draft.selection[1].is_some_and(|index| index > 0),
    );
    ui.set_button_enabled(
        controls.down,
        draft.selection[1].is_some_and(|index| index + 1 < draft.selected.len()),
    );
    ui.set_button_enabled(controls.apply, !draft.selected.is_empty());
    ui.set_dialog_open(controls.dialog, true);
    controls
}

pub fn restore(ui: &mut Ui, controls: Controls, draft: Option<&Draft>) {
    let Some(draft) = draft else {
        return;
    };
    for (index, list) in controls.lists.into_iter().enumerate() {
        ui.set_list_scroll_offset(list, draft.scroll[index]);
    }
    if let Some(body) = ui.dialog_body(controls.dialog) {
        ui.set_scroll_offset(body, draft.body_scroll.0, draft.body_scroll.1);
    }
    if let Some(id) = draft
        .focus
        .and_then(|index| controls.focus_order().get(index).copied())
    {
        ui.set_focus(Some(id));
    }
}

impl Demo {
    pub(crate) fn release_column_picker_pointer(&mut self) -> bool {
        if self.state.wizard_column_picker.is_none() || !self.ui.is_dragging() {
            return false;
        }
        self.ui.pointer_released();
        self.snapshot_inputs();
        if self.rebuild {
            self.do_rebuild();
        } else {
            self.ui.sync(&mut self.scene);
        }
        true
    }
    pub(crate) fn handle_column_list_activate(&mut self) {
        if let Some((id, _)) = self.ui.take_list_activate() {
            let controls = self.ids.column_picker;
            if self.state.wizard_column_picker.is_some() {
                if id == controls.lists[0] {
                    self.handle_column_picker_button(controls.add);
                } else if id == controls.lists[1] {
                    self.handle_column_picker_button(controls.remove);
                }
            }
        }
    }
    pub(crate) fn snapshot_column_picker(&mut self) {
        if self.rebuild {
            return;
        }
        let Some(draft) = &mut self.state.wizard_column_picker else {
            return;
        };
        let controls = self.ids.column_picker;
        if controls.dialog.0 == 0 {
            return;
        }
        for (index, list) in controls.lists.into_iter().enumerate() {
            draft.selection[index] = self.ui.list_selected(list);
            draft.scroll[index] = self.ui.list_scroll_offset(list).unwrap_or_default();
        }
        if let Some(body) = self.ui.dialog_body(controls.dialog) {
            draft.body_scroll = self.ui.scroll_offset(body).unwrap_or_default();
        }
        draft.focus = controls
            .focus_order()
            .iter()
            .position(|&id| Some(id) == self.ui.focused());
        self.ui
            .set_button_enabled(controls.add, draft.selection[0].is_some());
        self.ui
            .set_button_enabled(controls.remove, draft.selection[1].is_some());
        self.ui.set_button_enabled(
            controls.up,
            draft.selection[1].is_some_and(|index| index > 0),
        );
        self.ui.set_button_enabled(
            controls.down,
            draft.selection[1].is_some_and(|index| index + 1 < draft.selected.len()),
        );
    }

    pub(crate) fn open_column_picker(&mut self, page: usize, row: usize) {
        match Draft::new(&self.state, page, row) {
            Ok(draft) => {
                self.state.wizard_constraint_edit = None;
                self.state.wizard_table_page = page + 1;
                self.state.wizard_column_picker = Some(draft);
                self.rebuild = true;
            }
            Err(error) => {
                self.set_error(error);
                self.rebuild = true;
            }
        }
    }

    pub(crate) fn handle_column_picker_dismiss(&mut self) {
        if self.ui.take_dialog_dismiss() == Some(self.ids.column_picker.dialog)
            && self.state.wizard_column_picker.is_some()
        {
            self.close_column_picker();
        }
    }

    fn close_column_picker(&mut self) {
        let Some(draft) = self.state.wizard_column_picker.take() else {
            return;
        };
        self.do_rebuild();
        self.ui.set_focus(Some(
            [self.ids.ov_keys_table, self.ids.ov_fks_table][draft.page],
        ));
        self.ui.sync(&mut self.scene);
    }

    pub(crate) fn handle_column_picker_button(&mut self, id: WidgetId) {
        self.snapshot_column_picker();
        let controls = self.ids.column_picker;
        if id == controls.cancel {
            self.close_column_picker();
            return;
        }
        if id == controls.apply {
            let draft = self.state.wizard_column_picker.as_ref().unwrap().clone();
            match self.state.apply_column_picker(&draft) {
                Ok(()) => {
                    self.clear_error();
                    self.close_column_picker();
                }
                Err(error) => {
                    self.state.wizard_column_picker.as_mut().unwrap().error = Some(error);
                    self.rebuild = true;
                }
            }
            return;
        }
        let Some(draft) = &mut self.state.wizard_column_picker else {
            return;
        };
        if id == controls.add {
            draft.transfer(0);
        } else if id == controls.remove {
            draft.transfer(1);
        } else if id == controls.up {
            draft.move_selected(false);
        } else if id == controls.down {
            draft.move_selected(true);
        } else {
            return;
        }
        self.rebuild = true;
        self.do_rebuild();
        for list in self.ids.column_picker.lists {
            self.ui.reveal_list_selection(list);
        }
        self.ui.sync(&mut self.scene);
    }
}

impl AppState {
    fn apply_column_picker(&mut self, draft: &Draft) -> Result<(), String> {
        if draft.selected == draft.original {
            return Ok(());
        }
        if draft.selected.is_empty() {
            return Err("Select at least one column".into());
        }
        let columns = draft
            .selected
            .iter()
            .map(|item| item.1.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let mut indexes = self.wizard_index_constraints.clone();
        let mut foreign = self.wizard_foreign_keys.clone();
        let mut options = self.wizard_table_options.clone();
        let mut fields = self.wizard_cols.clone();
        if draft.page == 0 {
            indexes
                .get_mut(draft.row)
                .ok_or("The selected index constraint no longer exists")?
                .columns = columns;
            fields = table_sql::columns_with_index_constraints(&fields, &indexes)?;
            options.index_constraints = Some(indexes.clone());
            options.primary_key_conflict = indexes
                .iter()
                .find(|key| key.primary)
                .and_then(|key| key.conflict);
        } else {
            let key = foreign
                .get_mut(draft.row)
                .ok_or("The selected foreign key no longer exists")?;
            key.columns = columns;
            table_sql::normalize_foreign_key_scope(key);
            table_sql::foreign_key_sql(key)?;
            options.foreign_keys =
                Some(foreign.iter().filter(|key| !key.inline).cloned().collect());
            options.inline_foreign_keys =
                Some(foreign.iter().filter(|key| key.inline).cloned().collect());
        }
        // Validate the complete proposed SQL before retaining any changed draft fields.
        let old_indexes = std::mem::replace(&mut self.wizard_index_constraints, indexes);
        let old_foreign = std::mem::replace(&mut self.wizard_foreign_keys, foreign);
        let old_options = std::mem::replace(&mut self.wizard_table_options, options);
        let old_fields = std::mem::replace(&mut self.wizard_cols, fields);
        if let Err(error) = self.table_wizard_sql() {
            self.wizard_index_constraints = old_indexes;
            self.wizard_foreign_keys = old_foreign;
            self.wizard_table_options = old_options;
            self.wizard_cols = old_fields;
            return Err(error);
        }
        self.sync_wizard_foreign_key_fields();
        self.wizard_key_states
            .resize_with(self.wizard_index_constraints.len(), || None);
        self.wizard_foreign_key_states
            .resize_with(self.wizard_foreign_keys.len(), || [None, None]);
        if draft.page == 0 {
            self.wizard_key_states[draft.row] = None;
        } else {
            self.wizard_foreign_key_states[draft.row][0] = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::ObjectKind, state::MainTab, ui_build, UiKey};
    use next_gui_platform::PresentBackend;
    use next_gui_render::Renderer;

    fn fixture(sql: &str) -> AppState {
        let mut state = AppState::new_with_demo();
        state.new_memory_db().unwrap();
        state.conn.as_ref().unwrap().execute_batch(sql).unwrap();
        state.refresh_schema().unwrap();
        let row = state
            .schema
            .iter()
            .position(|object| object.name == "records" && object.kind == ObjectKind::Table)
            .unwrap();
        state.select_schema_node(10_000 + row as u64);
        state.open_modify_table_wizard().unwrap();
        state
    }

    #[test]
    fn ordered_columns_keep_duplicate_operands_collations_comments_and_real_data() {
        let mut state = fixture("CREATE TABLE records(id INTEGER PRIMARY KEY AUTOINCREMENT,a TEXT,b INTEGER,\"x,y\" TEXT,\"中文\" TEXT,\"line\nname\" TEXT,CONSTRAINT pair UNIQUE(a COLLATE NOCASE ASC,b DESC,a COLLATE RTRIM DESC) ON CONFLICT FAIL); INSERT INTO records(id,a,b) VALUES(7,'West',1),(11,'East',2); CREATE VIEW visible AS SELECT id,a,b FROM records; CREATE TABLE audit(value); CREATE TRIGGER edited AFTER UPDATE OF a ON records BEGIN INSERT INTO audit VALUES(new.a); END;");
        let row = state
            .wizard_index_constraints
            .iter()
            .position(|key| key.name.as_deref() == Some("pair"))
            .unwrap();
        state.wizard_index_constraints[row].columns =
            "a /* keep this comment */ COLLATE NOCASE ASC,b DESC,a COLLATE RTRIM DESC".into();
        state.wizard_table_options.index_constraints = Some(state.wizard_index_constraints.clone());
        let raw = state.wizard_index_constraints[row].columns.clone();
        let mut draft = Draft::new(&state, 0, row).unwrap();
        assert_eq!(
            draft
                .selected
                .iter()
                .map(|item| item.0.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "a"]
        );
        state.apply_column_picker(&draft).unwrap();
        assert_eq!(state.wizard_index_constraints[row].columns, raw);
        let original = draft.selected.clone();
        draft.selection[1] = Some(2);
        draft.transfer(1);
        assert_eq!(draft.available.last(), original.last());
        draft.selection[0] = draft.available.len().checked_sub(1);
        draft.transfer(0);
        assert_eq!(draft.selected, original);
        draft.move_selected(false);
        assert_eq!(draft.selected[1], original[2]);
        assert_eq!(draft.selected[2], original[1]);
        for name in ["x,y", "中文", "line\nname"] {
            draft.selection[0] = draft.available.iter().position(|item| item.0 == name);
            draft.transfer(0);
        }
        state.apply_column_picker(&draft).unwrap();
        let key = &state.wizard_index_constraints[row];
        assert_eq!(key.conflict, Some(sqlite3_parser::ast::ResolveType::Fail));
        assert_eq!(
            table_sql::index_constraint_columns(key).unwrap(),
            draft.selected
        );
        state.apply_table_wizard().unwrap();
        let conn = state.conn.as_ref().unwrap();
        assert_eq!(
            conn.query_row("SELECT sum(rowid+id+b) FROM records", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            39
        );
        conn.execute("UPDATE records SET a='changed' WHERE id=7", [])
            .unwrap();
        assert_eq!(
            conn.query_row("SELECT value FROM audit", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "changed"
        );
        assert_eq!(
            conn.query_row("SELECT a FROM visible WHERE id=7", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            "changed"
        );
        conn.execute("INSERT INTO records(a,b) VALUES('next',3)", [])
            .unwrap();
        assert_eq!(conn.last_insert_rowid(), 12);
    }

    #[test]
    fn invalid_key_and_foreign_key_changes_preserve_all_drafts_and_database() {
        let mut state = fixture("CREATE TABLE parent(a,b,PRIMARY KEY(a,b)); INSERT INTO parent VALUES(1,2); CREATE TABLE records(id INTEGER PRIMARY KEY AUTOINCREMENT,a,b,CONSTRAINT pair FOREIGN KEY(a,b) REFERENCES parent(a,b) ON UPDATE CASCADE DEFERRABLE INITIALLY DEFERRED); INSERT INTO records VALUES(7,1,2);");
        let primary = state
            .wizard_index_constraints
            .iter()
            .position(|key| key.primary)
            .unwrap();
        let indexes = state.wizard_index_constraints.clone();
        let foreign = state.wizard_foreign_keys.clone();
        let sql = state.table_wizard_sql().unwrap();
        let mut draft = Draft::new(&state, 0, primary).unwrap();
        draft.transfer(0);
        assert!(state.apply_column_picker(&draft).is_err());
        assert_eq!(state.wizard_index_constraints, indexes);
        assert_eq!(state.table_wizard_sql().unwrap(), sql);
        let mut draft = Draft::new(&state, 1, 0).unwrap();
        draft.transfer(1);
        assert!(state
            .apply_column_picker(&draft)
            .unwrap_err()
            .contains("counts must match"));
        assert_eq!(state.wizard_foreign_keys, foreign);
        assert_eq!(state.table_wizard_sql().unwrap(), sql);
        state.wizard_foreign_keys[0].clause.clear();
        assert_eq!(Draft::new(&state, 1, 0).unwrap().selected.len(), 2);
        state.wizard_foreign_keys = foreign.clone();
        state.wizard_foreign_keys[0].columns = "a".into();
        state.store_wizard_foreign_keys();
        let mut repair = Draft::new(&state, 1, 0).unwrap();
        repair.selection[0] = repair.available.iter().position(|item| item.0 == "b");
        repair.transfer(0);
        state.apply_column_picker(&repair).unwrap();
        assert_eq!(
            table_sql::foreign_key_columns(&state.wizard_foreign_keys[0])
                .unwrap()
                .len(),
            2
        );
        assert!(state.wizard_cols[0].auto_increment);
        assert_eq!(
            state
                .conn
                .as_ref()
                .unwrap()
                .query_row("SELECT sum(id+a+b) FROM records", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            10
        );

        let mut state = fixture("CREATE TABLE parent(id PRIMARY KEY); CREATE TABLE records(id INTEGER PRIMARY KEY,p REFERENCES parent CONSTRAINT deadline DEFERRABLE INITIALLY DEFERRED,q);");
        let mut draft = Draft::new(&state, 1, 0).unwrap();
        draft.transfer(1);
        draft.selection[0] = draft.available.iter().position(|item| item.0 == "q");
        draft.transfer(0);
        state.apply_column_picker(&draft).unwrap();
        assert_eq!(state.wizard_foreign_keys[0].columns, "\"q\"");
        assert!(state.wizard_foreign_keys[0].clause.contains("deadline"));
        let mut draft = Draft::new(&state, 1, 0).unwrap();
        draft.transfer(0);
        assert!(state.apply_column_picker(&draft).is_err());
        assert!(state.wizard_foreign_keys[0].inline);
        assert!(state.table_wizard_sql().unwrap().contains("deadline"));
    }

    #[test]
    fn reordered_foreign_columns_keep_actions_deferral_and_enforce_the_complete_key() {
        let mut state = fixture("CREATE TABLE parent(a,b,PRIMARY KEY(a,b)); INSERT INTO parent VALUES(1,2),(2,1); CREATE TABLE records(id INTEGER PRIMARY KEY AUTOINCREMENT,a,b,CONSTRAINT pair FOREIGN KEY(a,b) REFERENCES parent(a,b) ON UPDATE CASCADE ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED); INSERT INTO records VALUES(7,1,2);");
        let clause = state.wizard_foreign_keys[0].clause.clone();
        let mut draft = Draft::new(&state, 1, 0).unwrap();
        draft.move_selected(true);
        state.apply_column_picker(&draft).unwrap();
        assert_eq!(state.wizard_foreign_keys[0].clause, clause);
        state.apply_table_wizard().unwrap();
        let conn = state.conn.as_ref().unwrap();
        assert_eq!(
            conn.query_row("SELECT rowid+id+a+b FROM records", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            17
        );
        conn.execute("UPDATE parent SET a=3 WHERE a=2 AND b=1", [])
            .unwrap();
        assert_eq!(
            conn.query_row("SELECT a*10+b FROM records", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            13
        );
        conn.execute("INSERT INTO records(a,b) VALUES(8,9)", [])
            .unwrap();
        assert!(conn.execute_batch("COMMIT").is_err());
        conn.execute("INSERT INTO parent VALUES(9,8)", []).unwrap();
        conn.execute_batch("COMMIT; BEGIN").unwrap();
        conn.execute("DELETE FROM parent WHERE a=3 AND b=1", [])
            .unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT a IS NULL AND b IS NULL FROM records WHERE id=7",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn resizing_inline_primary_key_desc_keeps_non_alias_rowids_and_name() {
        let mut state = fixture("CREATE TABLE records(id INTEGER CONSTRAINT descending PRIMARY KEY DESC,a INTEGER); INSERT INTO records(rowid,id,a) VALUES(7,42,5);");
        let mut draft = Draft::new(&state, 0, 0).unwrap();
        draft.selection[0] = draft.available.iter().position(|item| item.0 == "a");
        draft.transfer(0);
        state.apply_column_picker(&draft).unwrap();
        let mut draft = Draft::new(&state, 0, 0).unwrap();
        draft.selection[1] = Some(1);
        draft.transfer(1);
        state.apply_column_picker(&draft).unwrap();
        assert!(state.wizard_index_constraints[0].inline);
        state.wizard_cols[1].type_name = "REAL".into();
        state.apply_table_wizard().unwrap();
        let conn = state.conn.as_ref().unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT rowid+id+CAST(a AS INTEGER) FROM records",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            54
        );
        conn.execute("INSERT INTO records(a) VALUES(43)", [])
            .unwrap();
        assert_eq!(
            conn.query_row("SELECT id IS NULL FROM records WHERE a=43", [], |row| row
                .get::<_, i64>(
                0
            ))
            .unwrap(),
            1
        );
        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE name='records'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(sql.contains("descending") && sql.contains("DESC"));
    }

    fn capture(demo: &mut Demo, label: &str) {
        let Ok(prefix) = std::env::var("NEXT_GUI_DB_BROWSER_COLUMN_PICKER_QA_OUT") else {
            return;
        };
        let (width, height) = (demo.win_w, demo.win_h);
        demo.ui.sync(&mut demo.scene);
        demo.scene.mark_full_dirty(width as f32, height as f32);
        let dirty = demo.scene.take_dirty();
        let commands = crate::encode_scene(&demo.scene, &dirty);
        let clear = crate::clear_color(&demo.theme);
        let mut soft = crate::SoftRenderer::new(width, height, clear);
        soft.render(&commands, &dirty);
        crate::write_ppm(
            &format!("{prefix}-{label}-{width}.ppm"),
            soft.frame_pixels().unwrap().0,
            width,
            height,
        )
        .unwrap();
        if let Ok(mut gpu) = crate::GpuRenderer::try_new(width, height, clear) {
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
            assert!(delta <= 2, "{label}/{width}: {delta}");
            eprintln!("Column picker {label}/{width}: Soft/Metal delta {delta}");
        } else {
            eprintln!("Column picker GPU comparison not_run: unavailable");
        }
    }

    fn finish(demo: &mut Demo) {
        demo.handle_column_picker_dismiss();
        demo.handle_column_list_activate();
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

    #[test]
    fn column_dialog_grid_buttons_cancel_resize_scroll_and_real_save() {
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            let mut demo = Demo::new(1400, 1000, PresentBackend::Softbuffer);
            demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
            demo.state = fixture("CREATE TABLE parent(a,b,PRIMARY KEY(a,b)); INSERT INTO parent VALUES(1,2); CREATE TABLE records(id INTEGER PRIMARY KEY AUTOINCREMENT,a,b,CONSTRAINT pair UNIQUE(a COLLATE NOCASE ASC,b DESC),FOREIGN KEY(a,b) REFERENCES parent(a,b) DEFERRABLE INITIALLY DEFERRED); INSERT INTO records VALUES(7,1,2);");
            demo.state.tab = MainTab::Structure;
            for index in 0..20 {
                let mut column = demo.state.wizard_cols[1].clone();
                column.name = format!("available_{index}");
                column.pk = false;
                column.unique = false;
                demo.state.wizard_cols.push(column);
                demo.state.wizard_col_sources.push(None);
                demo.state
                    .wizard_constraint_columns
                    .push(format!("available_{index}"));
            }
            demo.state.wizard_table_page = 1;
            let row = demo
                .state
                .wizard_index_constraints
                .iter()
                .position(|key| key.name.as_deref() == Some("pair"))
                .unwrap();
            demo.state.wizard_key_sel = row;
            demo.do_rebuild();
            let table = demo.ids.ov_keys_table;
            demo.ui.set_table_selected_col(table, Some(0));
            demo.ui.set_focus(Some(table));
            assert!(demo.ui.key_input(UiKey::Edit));
            finish(&mut demo);
            assert_eq!(demo.ui.active_dialog(), Some(demo.ids.column_picker.dialog));
            assert!(demo.state.wizard_constraint_edit.is_none());
            let original = demo.state.wizard_index_constraints.clone();
            demo.handle_button(demo.ids.ov_btn_ok);
            assert_eq!(demo.state.overlay, Some(OverlayKind::EditTable));
            demo.ui.set_focus(Some(demo.ids.ov_name));
            assert_ne!(demo.ui.focused(), Some(demo.ids.ov_name));
            demo.ui.set_focus(Some(demo.ids.column_picker.lists[0]));
            demo.ui.key_input(UiKey::End);
            finish(&mut demo);
            assert!(
                demo.ui
                    .list_scroll_offset(demo.ids.column_picker.lists[0])
                    .unwrap()
                    > 0.0
            );
            demo.win_w = 800;
            demo.win_h = 600;
            demo.do_rebuild();
            assert_eq!(
                demo.ui.list_selected(demo.ids.column_picker.lists[0]),
                Some(20)
            );
            assert!(
                demo.ui
                    .list_scroll_offset(demo.ids.column_picker.lists[0])
                    .unwrap()
                    > 0.0
            );
            assert!(demo.ui.key_input(UiKey::Enter));
            finish(&mut demo);
            assert_eq!(
                demo.state
                    .wizard_column_picker
                    .as_ref()
                    .unwrap()
                    .selected
                    .last()
                    .unwrap()
                    .0,
                "available_19"
            );
            demo.ui.set_focus(Some(demo.ids.column_picker.lists[1]));
            assert!(demo.ui.key_input(UiKey::Enter));
            finish(&mut demo);
            let list = demo.ids.column_picker.lists[0];
            demo.ui.set_focus(Some(list));
            demo.ui.key_input(UiKey::End);
            finish(&mut demo);
            let bounds = demo
                .scene
                .iter_paint_order()
                .into_iter()
                .find(|node| demo.ui.widget_from_node(node.id) == Some(list))
                .unwrap()
                .bounds;
            let y = bounds.y
                + ListStyle::from_theme(&demo.theme).padding
                + (demo.ui.list_selected(list).unwrap() as f32 + 0.5) * 26.0
                - demo.ui.list_scroll_offset(list).unwrap();
            assert!(demo.ui.pointer_pressed(&mut demo.scene, bounds.x + 20.0, y));
            assert!(demo.release_column_picker_pointer());
            assert_eq!(demo.ids.column_picker.lists[0], list);
            assert!(demo.ui.pointer_pressed(&mut demo.scene, bounds.x + 20.0, y));
            finish(&mut demo);
            assert_eq!(
                demo.state
                    .wizard_column_picker
                    .as_ref()
                    .unwrap()
                    .selected
                    .last()
                    .unwrap()
                    .0,
                "available_19"
            );
            demo.handle_button(demo.ids.column_picker.up);
            finish(&mut demo);
            assert_eq!(
                demo.ui.list_selected(demo.ids.column_picker.lists[1]),
                Some(1)
            );
            demo.ui.key_input(UiKey::Escape);
            finish(&mut demo);
            assert!(demo.state.wizard_column_picker.is_none());
            assert_eq!(demo.state.wizard_index_constraints, original);
            assert_eq!(demo.ui.focused(), Some(demo.ids.ov_keys_table));

            demo.open_column_picker(1, 0);
            demo.do_rebuild();
            capture(&mut demo, &format!("{theme_name}-foreign"));
            demo.handle_button(demo.ids.column_picker.remove);
            finish(&mut demo);
            demo.handle_button(demo.ids.column_picker.apply);
            finish(&mut demo);
            assert!(demo
                .state
                .wizard_column_picker
                .as_ref()
                .unwrap()
                .error
                .as_deref()
                .unwrap()
                .contains("counts must match"));
            assert!(demo.ui.active_dialog().is_some());
            assert!(demo
                .ui
                .text_area_value(demo.ids.column_picker.error)
                .unwrap()
                .contains("counts must match"));
            let area = demo
                .scene
                .iter_paint_order()
                .into_iter()
                .find(|node| {
                    demo.ui.widget_from_node(node.id) == Some(demo.ids.column_picker.error)
                })
                .unwrap();
            let body = demo.ui.dialog_body(demo.ids.column_picker.dialog).unwrap();
            let viewport = demo
                .scene
                .iter_paint_order()
                .into_iter()
                .find(|node| demo.ui.widget_from_node(node.id) == Some(body))
                .unwrap()
                .bounds;
            assert!(
                area.bounds.y >= viewport.y
                    && area.bounds.y + area.bounds.height <= viewport.y + viewport.height
            );
            capture(&mut demo, &format!("{theme_name}-error"));
            demo.handle_button(demo.ids.column_picker.cancel);
            finish(&mut demo);

            demo.open_column_picker(0, row);
            demo.do_rebuild();
            demo.handle_button(demo.ids.column_picker.down);
            finish(&mut demo);
            let changed = demo
                .state
                .wizard_column_picker
                .as_ref()
                .unwrap()
                .selected
                .clone();
            for (width, height) in [(800, 600), (1400, 1000)] {
                demo.win_w = width;
                demo.win_h = height;
                demo.do_rebuild();
                assert_eq!(demo.ui.active_dialog(), Some(demo.ids.column_picker.dialog));
                let body = demo.ui.dialog_body(demo.ids.column_picker.dialog).unwrap();
                demo.ui.set_scroll_offset(body, 10000.0, 0.0);
                assert!(demo.ui.scroll_offset(body).unwrap().0 < 0.01);
                capture(&mut demo, &format!("{theme_name}-index"));
            }
            demo.handle_button(demo.ids.column_picker.apply);
            finish(&mut demo);
            assert!(
                demo.state.wizard_column_picker.is_none(),
                "{:?}",
                demo.state
                    .wizard_column_picker
                    .as_ref()
                    .and_then(|draft| draft.error.as_deref())
            );
            assert_eq!(
                table_sql::index_constraint_columns(&demo.state.wizard_index_constraints[row])
                    .unwrap(),
                changed
            );
            demo.handle_button(demo.ids.ov_btn_ok);
            finish(&mut demo);
            assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
            assert!(demo.state.overlay.is_none());
            let conn = demo.state.conn.as_ref().unwrap();
            assert_eq!(
                conn.query_row("SELECT rowid+id+a+b FROM records", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                17
            );
            assert!(conn
                .execute("INSERT INTO records(a,b) VALUES(1,2)", [])
                .is_err());
            assert!(demo
                .state
                .conn
                .as_ref()
                .unwrap()
                .execute_batch("COMMIT")
                .is_ok());
            demo.state.open_create_table_wizard();
            assert!(demo.state.wizard_column_picker.is_none());
        }
    }
}
