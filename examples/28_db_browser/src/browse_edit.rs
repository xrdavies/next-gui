//! Browse cell editors share the typed cell draft and its existing write/rollback guards.
use crate::{
    db,
    state::{AppState, EditCellMode, MainTab},
    Demo, UiKey,
};
use next_gui_ui::{
    AutocompleteState, AutocompleteStyle, FlexBuilder, TableCellEditEvent, TextInputState,
    TextInputStyle, WidgetId,
};
use std::collections::HashSet;

pub(crate) const QA_SQL: &str = "CREATE TABLE cells(id INTEGER PRIMARY KEY, n, amount NUMERIC CHECK(typeof(amount) IN ('integer','real') AND amount>0), note TEXT, binary BLOB, computed AS(amount*2));
    INSERT INTO cells VALUES(9007199254740993,NULL,1.25,'保留\nline',x'00FF');
    INSERT INTO cells VALUES(9007199254740994,9007199254740993,2.5,'second',x'6869');";
pub(crate) const COMPLETION_QA_SQL: &str = "CREATE TABLE choices(id INTEGER PRIMARY KEY, word TEXT);
    INSERT INTO choices VALUES(1,'seed'),(2,'Éclair'),(3,'école'),(4,'xÉclair'),(5,'Éclair'),(6,'');";

#[derive(Debug, Clone)]
pub struct Draft {
    pub row: usize,
    pub column: usize,
    revision: u64,
    original: String,
    storage: usize,
    input: Option<TextInputState>,
    suggestions: Vec<String>,
    autocomplete: Option<AutocompleteState>,
    pub(crate) foreign_key: Option<crate::fk_values::Draft>,
}

impl Draft {
    pub(crate) fn foreign_key(state: &AppState, row: usize, column: usize, choice: crate::fk_values::Draft) -> Self {
        Self { row, column, revision: state.edit_revision, original: state.edit_value.clone(), storage: state.edit_type,
            input: None, suggestions: Vec::new(), autocomplete: None, foreign_key: Some(choice) }
    }
}

pub fn editable_columns(state: &AppState) -> Vec<usize> {
    if state.tab != MainTab::Browse
        || state.read_only
        || state.sql_busy
        || state.guard_background_idle().is_err()
        || state.browse().row_keys.is_empty()
    {
        return Vec::new();
    }
    let tab = state.browse();
    let Some(result) = &tab.result else {
        return Vec::new();
    };
    let mut columns: Vec<_> = result
        .columns
        .iter()
        .map(|name| db::ColDef::new(name, ""))
        .collect();
    let Some(object) = state
        .schema
        .iter()
        .find(|object| object.schema == tab.schema && Some(&object.name) == tab.table.as_ref())
    else {
        return Vec::new();
    };
    if object.kind == db::ObjectKind::View && tab.unlock_view_pk.is_none() {
        return Vec::new();
    }
    if let Some(sql) = object
        .sql
        .as_deref()
        .filter(|_| object.kind == db::ObjectKind::Table)
    {
        // The optional leading rowid is not a declared field; virtual tables have no generated fields.
        if !crate::table_sql::is_virtual_table(sql)
            && crate::table_sql::read_column_constraints(
                sql,
                &mut columns[usize::from(tab.show_row_id)..],
            )
            .is_err()
        {
            return Vec::new();
        }
    }
    columns
        .iter()
        .enumerate()
        .filter_map(|(index, column)| column.generated.is_none().then_some(index))
        .collect()
}

pub fn mount(
    builder: &mut FlexBuilder<'_>,
    table: WidgetId,
    state: &AppState,
    style: TextInputStyle,
    theme: &next_gui_ui::Theme,
) {
    if let Some(draft) = &state.browse_cell_edit {
        builder.table_cell_editor(table, draft.row, draft.column, |cell| {
            if let Some(choice) = &draft.foreign_key {
                let mut select = next_gui_ui::SelectStyle::from_theme(theme);
                select.width = style.width; select.height = style.height; select.size = style.size;
                select.item_height = select.item_height.max(style.size + 10.0);
                cell.data_select(choice.choices.clone(), Some(choice.selected), "Choose a referenced value", select)
            } else if draft.suggestions.is_empty() {
                cell.text_input_styled(&state.edit_value, "", style)
            } else {
                let mut completion = AutocompleteStyle::from_theme(theme);
                completion.width = style.width;
                completion.height = style.height;
                completion.size = style.size;
                completion.item_height = completion.item_height.max(style.size + 10.0);
                cell.autocomplete(draft.suggestions.clone(), &state.edit_value, completion)
            }
        });
    }
}

impl AppState {
    fn browse_cell_suggestions(&self, column: usize) -> Result<Vec<String>, String> {
        let limit = self.prefs.browse_completion_threshold;
        let tab = self.browse();
        let Some(total) = tab.total_rows.filter(|&total| limit > 0 && total <= limit) else {
            return Ok(Vec::new());
        };
        let result = tab
            .result
            .as_ref()
            .ok_or("Refresh Browse before completing values")?;
        if let (Some(loaded), Some(conn)) = (&tab.loaded, &self.conn) {
            if !loaded.matches(conn, tab, &self.prefs.filter_escape)? { return Ok(Vec::new()); }
        }
        let name = result.columns.get(column).ok_or("No completion column")?;
        let queried;
        let (values, column) =
            if tab.offset == 0 && !result.truncated && result.values.len() == total {
                (&result.values, column)
            } else {
                let filter = Self::composed_where(tab, &result.columns, &self.prefs.filter_escape)?;
                queried = db::browse_column_values_in(
                    self.conn.as_ref().ok_or("No database")?,
                    &tab.schema,
                    tab.table.as_deref().ok_or("No table")?,
                    name,
                    &tab.ordering(),
                    filter.as_deref(),
                    limit,
                )?;
                // A stale count must never expose a partial page as the complete candidate set.
                if queried.truncated || queried.values.len() != total {
                    return Ok(Vec::new());
                }
                (&queried.values, 0)
            };
        let mut seen = HashSet::new();
        let mut suggestions = Vec::new();
        for row in values.rows()? {
            let row = row?;
            let text = match row.get(column) {
                Some(db::CellValue::Text(bytes)) => std::str::from_utf8(bytes)
                    .ok()
                    .filter(|_| !bytes.contains(&0))
                    .map(str::to_owned),
                Some(value @ (db::CellValue::Integer(_) | db::CellValue::Real(_))) => {
                    Some(db::cell_to_string(value.as_ref()))
                }
                _ => None,
            };
            if let Some(text) = text.filter(|text| !text.is_empty() && seen.insert(text.clone())) {
                suggestions.push(text);
            }
        }
        Ok(suggestions)
    }

    pub(crate) fn begin_browse_cell_edit(
        &mut self,
        row: usize,
        column: usize,
    ) -> Result<(), String> {
        self.guard_writable()?;
        self.guard_browse_editable()?;
        if self.sql_busy || self.has_cell_draft() {
            return Err("Apply or discard the cell draft first".into());
        }
        if !editable_columns(self).contains(&column) || self.browse().row_keys.get(row).is_none() {
            return Err("This cell is read-only".into());
        }
        let tab = self.browse_mut();
        tab.clear_selection();
        tab.sel_row = Some(row);
        tab.sel_col = Some(column);
        self.sync_edit_cell_from_selection();
        if self.edit_cell_read_only() || self.edit_col.is_empty() {
            return Err("This cell is read-only".into());
        }
        if self.start_fk_values(row, column)? { return Ok(()); }
        if matches!(&self.edit_saved, rusqlite::types::Value::Blob(bytes) if std::str::from_utf8(bytes).is_err() || bytes.contains(&0))
        {
            self.dock_edit = true;
            return Err("Use the Edit Cell Hex editor for binary data".into());
        }
        self.set_edit_mode(EditCellMode::Text)?;
        self.status_msg = format!(
            "Editing {} · Enter/Tab to apply · Esc to cancel",
            self.edit_col
        );
        // Suggestions are optional; a failed auxiliary read must not prevent normal editing.
        let suggestions = self.browse_cell_suggestions(column).unwrap_or_default();
        self.browse_cell_edit = Some(Draft {
            row,
            column,
            revision: self.edit_revision,
            original: self.edit_value.clone(),
            storage: self.edit_type,
            input: None,
            suggestions,
            autocomplete: None,
            foreign_key: None,
        });
        Ok(())
    }
}

impl Demo {
    pub(crate) fn snapshot_browse_cell_editor(&mut self) {
        let Some(editor) = self.ui.table_cell_editor(self.ids.browse_table) else {
            return;
        };
        let Some(draft) = &mut self.state.browse_cell_edit else {
            return;
        };
        if draft.revision != self.state.edit_revision || draft.revision != self.last_edit_revision {
            return;
        }
        if let Some(choice) = &mut draft.foreign_key {
            choice.session = self.ui.select_state(editor);
            if let Some(index) = self.ui.select_selected(editor).filter(|index| *index != choice.selected) {
                if let Err(error) = choice.select(index) { self.set_error(error); self.rebuild = true; }
            }
            if let Some((_, error)) = self.ui.take_select_data_error() { self.set_error(error); }
            return;
        }
        draft.input = self.ui.text_input_state(editor);
        draft.autocomplete = self.ui.autocomplete_state(editor);
        if let Some(value) = self
            .ui
            .text_input_value(editor)
            .or_else(|| self.ui.autocomplete_value(editor))
            .map(str::to_owned)
        {
            // Unchanged NULL/BLOB/numeric values keep their original storage, including Undo.
            let storage = if value == draft.original {
                draft.storage
            } else {
                3
            };
            let changed = self.state.edit_value != value || self.state.edit_type != storage;
            self.state.edit_type = storage;
            self.state.edit_value = value.clone();
            self.state.edit_view = None;
            self.ui.set_text_area_value(self.ids.edit_value, value);
            self.ui
                .set_select_selected(self.ids.edit_type, Some(self.state.edit_type));
            if changed {
                self.state.invalidate_cell_format();
                self.state.refresh_cell_format_issue();
                crate::ui_build::update_cell_caption(
                    &mut self.ui,
                    &self.state,
                    self.ids.edit_meta,
                    self.ids.edit_meta_label,
                );
            }
        }
    }

    fn finish_browse_cell_editor(&mut self, cancel: bool) -> bool {
        self.snapshot_browse_cell_editor();
        if self.state.browse_cell_edit.is_none() {
            return true;
        }
        if cancel {
            self.state.discard_cell_draft();
            self.state.status_msg = "Cell draft discarded".into();
        } else if let Err(error) = self.state.apply_cell_draft() {
            self.set_error(error);
            self.rebuild = true;
            return false;
        }
        self.state.browse_cell_edit = None;
        self.clear_error();
        self.rebuild = true;
        true
    }

    pub(crate) fn finish_browse_cell_pointer(&mut self, x: f32, y: f32) -> bool {
        if self.state.browse_cell_edit.is_none() {
            return true;
        }
        if self.ui.hit_widget(&self.scene, x, y).is_some_and(|hit| {
            self.ui
                .table_cell_editor_contains(self.ids.browse_table, hit)
        }) {
            return true;
        }
        let cancel = self.ui.hit_widget(&self.scene, x, y) == Some(self.ids.btn_edit_discard);
        if !self.finish_browse_cell_editor(cancel) {
            self.do_rebuild();
            if let Some(editor) = self.ui.table_cell_editor(self.ids.browse_table) {
                self.ui.set_focus(Some(editor));
                self.ui.sync(&mut self.scene);
            }
            return false;
        }
        self.do_rebuild();
        true
    }

    pub(crate) fn handle_browse_cell_edit(&mut self, event: TableCellEditEvent) {
        match event {
            TableCellEditEvent::End { cancel } => {
                let finished = self.finish_browse_cell_editor(cancel);
                self.do_rebuild();
                self.ui.set_focus(Some(if finished {
                    self.ids.browse_table
                } else {
                    self.ui
                        .table_cell_editor(self.ids.browse_table)
                        .unwrap_or(self.ids.browse_table)
                }));
            }
            TableCellEditEvent::Begin { row, column, key } => {
                let same_record = self
                    .state
                    .browse_cell_edit
                    .as_ref()
                    .is_some_and(|draft| draft.row == row);
                let target = self.state.browse().row_keys.get(row).cloned();
                if self.state.browse_cell_edit.is_some() && !self.finish_browse_cell_editor(false) {
                    self.do_rebuild();
                    if let Some(editor) = self.ui.table_cell_editor(self.ids.browse_table) {
                        self.ui.set_focus(Some(editor));
                    }
                } else {
                    let row = if same_record {
                        self.state.browse().sel_row
                    } else {
                        target.and_then(|key| {
                            self.state
                                .browse()
                                .row_keys
                                .iter()
                                .position(|candidate| *candidate == key)
                        })
                    };
                    let result = row
                        .ok_or_else(|| "The target record is no longer in this page".to_string())
                        .and_then(|row| self.state.begin_browse_cell_edit(row, column));
                    if let Err(error) = result {
                        self.set_error(error);
                        self.do_rebuild();
                    } else {
                        self.clear_error();
                        self.do_rebuild();
                        if let Some(editor) = self.ui.table_cell_editor(self.ids.browse_table) {
                            self.ui.set_focus(Some(editor));
                            self.ui.key_input(UiKey::SelectAll);
                            if let Some(key) = key {
                                self.ui.key_input(key);
                            }
                            self.snapshot_browse_cell_editor();
                        }
                    }
                }
            }
        }
        self.ui.sync(&mut self.scene);
    }

    pub(crate) fn restore_browse_cell_editor(&mut self, focused: bool) {
        if let (Some(editor), Some(draft)) = (
            self.ui.table_cell_editor(self.ids.browse_table),
            &self.state.browse_cell_edit,
        ) {
            if focused {
                self.ui.set_focus(Some(editor));
            }
            if let Some(input) = &draft.input {
                self.ui.restore_text_input_state(editor, input);
            }
            self.ui.set_autocomplete_prefix_matching(editor, true);
            if let Some(session) = &draft.autocomplete {
                self.ui.set_autocomplete_state(editor, session);
            }
            if let Some(session) = draft.foreign_key.as_ref().and_then(|choice| choice.session.as_ref()) {
                self.ui.restore_select_state(editor, session);
            }
            self.ui.set_accessible_name(
                editor,
                format!(
                    "Edit {} · Record {} · Enter to apply, Escape to cancel",
                    self.state.edit_col,
                    self.state.browse().offset + draft.row + 1
                ),
            );
            self.ui.sync(&mut self.scene);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PresentBackend, Theme};
    use rusqlite::types::Value;

    fn completion_fixture(theme: Theme) -> Demo {
        let mut demo = fixture(theme);
        demo.state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch(COMPLETION_QA_SQL)
            .unwrap();
        demo.state.write_changes().unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.switch_browse_table("choices").unwrap();
        select(&mut demo, 0, 1);
        demo
    }

    #[test]
    fn completion_uses_unique_typed_values_and_cached_complete_results_without_queries() {
        let mut demo = fixture(Theme::dark());
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE choices(value);
            INSERT INTO choices VALUES(NULL),(''),('NULL'),('Éclair'),('Éclair'),('éclair'),(9007199254740993),(1.25),(x'6869'),(CAST(x'FF' AS TEXT)),(CAST(x'610062' AS TEXT));").unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.switch_browse_table("choices").unwrap();
        let parked = demo.state.conn.take();
        assert_eq!(
            demo.state.browse_cell_suggestions(0).unwrap(),
            ["NULL", "Éclair", "éclair", "9007199254740993", "1.25"]
        );
        demo.state.prefs.browse_completion_threshold = 0;
        assert!(demo.state.browse_cell_suggestions(0).unwrap().is_empty());
        demo.state.prefs.browse_completion_threshold = 10;
        assert!(demo.state.browse_cell_suggestions(0).unwrap().is_empty());
        demo.state.prefs.browse_completion_threshold = 11;
        assert_eq!(demo.state.browse_cell_suggestions(0).unwrap().len(), 5);
        demo.state.browse_mut().total_rows = None;
        assert!(demo.state.browse_cell_suggestions(0).unwrap().is_empty());
        demo.state.conn = parked;
    }

    #[test]
    fn completion_reads_beyond_the_page_with_qualified_filters_order_and_stale_count_guards() {
        let mut demo = fixture(Theme::dark());
        let conn = demo.state.conn.as_ref().unwrap();
        conn.execute_batch("ATTACH ':memory:' AS aux;
            CREATE TABLE aux.choices(id INTEGER PRIMARY KEY, \"word.名\" TEXT, payload BLOB);
            WITH RECURSIVE seq(n) AS(VALUES(1) UNION ALL SELECT n+1 FROM seq WHERE n<602)
            INSERT INTO aux.choices SELECT n,printf('Value %03d',n),NULL FROM seq;
            UPDATE aux.choices SET payload=zeroblob(1048576) WHERE id=602;
            CREATE TABLE choices(id,\"word.名\");INSERT INTO choices VALUES(1,'main');
            CREATE TEMP TABLE choices(id,\"word.名\");INSERT INTO temp.choices VALUES(1,'temporary');").unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.switch_browse_table_in("aux", "choices").unwrap();
        assert_eq!(demo.state.browse().result.as_ref().unwrap().rows.len(), 500);
        let values = demo.state.browse_cell_suggestions(1).unwrap();
        assert_eq!(values.len(), 602);
        assert!(values.contains(&"Value 602".into()));
        demo.state.browse_mut().total_rows = Some(601);
        assert!(demo.state.browse_cell_suggestions(1).unwrap().is_empty());
        demo.state.browse_mut().total_rows = Some(602);
        demo.state.prefs.browse_completion_threshold = 601;
        assert!(demo.state.browse_cell_suggestions(1).unwrap().is_empty());
        demo.state.prefs.browse_completion_threshold = 1000;
        demo.state.browse_mut().col_filters = vec![">1".into()];
        demo.state.refresh_browse().unwrap();
        demo.state.browse_page(true).unwrap();
        assert_eq!(demo.state.browse().offset, 0);
        assert_eq!(demo.state.browse().sel_row, Some(500));
        let values = demo.state.browse_cell_suggestions(1).unwrap();
        assert_eq!(values.len(), 601);
        assert!(!values.contains(&"Value 001".into()));
        demo.state
            .set_browse_ordering(vec![db::OrderBy {
                column: "id".into(),
                descending: true,
            }])
            .unwrap();
        let values = demo.state.browse_cell_suggestions(1).unwrap();
        assert_eq!(values.first().map(String::as_str), Some("Value 602"));
        let conn = demo.state.conn.as_ref().unwrap();
        assert!(
            db::browse_column_values_in(conn, "aux", "choices", "missing", &[], None, 1000)
                .is_err()
        );
        assert!(db::browse_column_values_in(
            conn,
            "aux",
            "choices",
            "word.名",
            &[],
            Some("1; DELETE FROM aux.choices"),
            1000
        )
        .is_err());
        assert_eq!(
            db::count_browse_rows_in(conn, "aux", "choices", None).unwrap(),
            602
        );
        conn.execute_batch(
            "WITH RECURSIVE seq(n) AS(VALUES(603) UNION ALL SELECT n+1 FROM seq WHERE n<1003)
            INSERT INTO aux.choices(id,\"word.名\") SELECT n,'new' FROM seq;",
        )
        .unwrap();
        assert!(demo.state.browse_cell_suggestions(1).unwrap().is_empty());
    }

    #[test]
    fn completion_popup_retains_selection_history_and_cell_commit_cancel_tab_semantics() {
        let folder = std::env::var_os("NEXT_GUI_COMPLETION_QA_OUT").map(std::path::PathBuf::from);
        if let Some(folder) = &folder {
            std::fs::create_dir_all(folder).unwrap();
        }
        for (name, theme, width, height, size) in [
            ("dark-wide", Theme::dark(), 1400, 860, 13.0),
            ("light-wide", Theme::light(), 1400, 860, 18.0),
            ("dark-narrow", Theme::dark(), 800, 600, 24.0),
            ("light-narrow", Theme::light(), 800, 600, 18.0),
        ] {
            let mut demo = completion_fixture(theme);
            demo.win_w = width;
            demo.win_h = height;
            demo.state.prefs.font_size = size;
            key(&mut demo, UiKey::Edit);
            demo.ui.paste_into_focused_text("é");
            demo.snapshot_inputs();
            assert!(demo.ui.has_open_popup());
            key(&mut demo, UiKey::Down);
            demo.do_rebuild();
            assert!(demo.ui.has_open_popup());
            if let Some(folder) = &folder {
                capture(&mut demo, folder, &format!("{name}-popup"));
            }
            key(&mut demo, UiKey::Enter);
            assert_eq!(demo.state.edit_value, "école");
            assert!(demo.state.browse_cell_edit.is_some());
            assert_eq!(
                demo.state
                    .conn
                    .as_ref()
                    .unwrap()
                    .query_row("SELECT word FROM choices WHERE id=1", [], |r| r
                        .get::<_, String>(0))
                    .unwrap(),
                "seed"
            );
            key(&mut demo, UiKey::Undo);
            assert_eq!(demo.state.edit_value, "é");
            key(&mut demo, UiKey::Redo);
            assert_eq!(demo.state.edit_value, "école");
            key(&mut demo, UiKey::Escape);
            assert!(demo.state.browse_cell_edit.is_some());
            key(&mut demo, UiKey::Enter);
            assert!(demo.state.browse_cell_edit.is_none());
            assert_eq!(
                demo.state
                    .conn
                    .as_ref()
                    .unwrap()
                    .query_row("SELECT word FROM choices WHERE id=1", [], |r| r
                        .get::<_, String>(0))
                    .unwrap(),
                "école"
            );
            if let Some(folder) = &folder {
                capture(&mut demo, folder, &format!("{name}-applied"));
            }
            key(&mut demo, UiKey::Edit);
            demo.ui.paste_into_focused_text("é");
            demo.snapshot_inputs();
            key(&mut demo, UiKey::Escape);
            assert!(demo.state.has_cell_draft());
            key(&mut demo, UiKey::Escape);
            assert!(demo.state.browse_cell_edit.is_none());
            assert_eq!(demo.state.edit_value, "école");
            key(&mut demo, UiKey::Edit);
            demo.ui.paste_into_focused_text("é");
            demo.snapshot_inputs();
            key(&mut demo, UiKey::Tab);
            assert_eq!(
                demo.state
                    .browse_cell_edit
                    .as_ref()
                    .map(|d| (d.row, d.column)),
                Some((1, 0))
            );
            assert_eq!(demo.state.edit_value, "2");
            assert_eq!(
                demo.state
                    .conn
                    .as_ref()
                    .unwrap()
                    .query_row("SELECT word FROM choices WHERE id=1", [], |r| r
                        .get::<_, String>(0))
                    .unwrap(),
                "é"
            );
            if let Some(folder) = &folder {
                capture(&mut demo, folder, &format!("{name}-tab"));
            }
            key(&mut demo, UiKey::Escape);
        }
    }

    #[test]
    fn completion_preferences_validate_before_mutation_and_preserve_drafts_on_rebuild_cancel_save()
    {
        use crate::state::OverlayKind;
        let folder = std::env::var_os("NEXT_GUI_COMPLETION_QA_OUT").map(std::path::PathBuf::from);
        for (name, theme, width, height) in [
            ("dark-wide", Theme::dark(), 1400, 860),
            ("light-narrow", Theme::light(), 800, 600),
        ] {
            let mut demo = completion_fixture(theme);
            demo.win_w = width;
            demo.win_h = height;
            let defaults: crate::prefs::Prefs = serde_json::from_str("{}").unwrap();
            assert_eq!(defaults.browse_completion_threshold, 1000);
            demo.state.sync_prefs_drafts();
            let original = serde_json::to_string(&demo.state.prefs).unwrap();
            demo.state.prefs_font_size = "21".into();
            for invalid in ["-1", "100000001", "bad", "1.5", ""] {
                demo.state.prefs_browse_completion_threshold = invalid.into();
                assert!(demo.state.apply_prefs_overlay().is_err());
                assert_eq!(serde_json::to_string(&demo.state.prefs).unwrap(), original);
            }
            demo.state.sync_prefs_drafts();
            demo.state.overlay = Some(OverlayKind::Preferences);
            demo.do_rebuild();
            demo.ui
                .set_text_input(demo.ids.ov_prefs_browse_completion, "0");
            demo.handle_button(demo.ids.ov_btn_cancel);
            demo.do_rebuild();
            assert_eq!(demo.state.prefs.browse_completion_threshold, 1000);
            demo.state.sync_prefs_drafts();
            demo.state.overlay = Some(OverlayKind::Preferences);
            demo.do_rebuild();
            let field = demo.ids.ov_prefs_browse_completion;
            demo.ui.set_focus(Some(field));
            demo.ui.key_input(UiKey::SelectAll);
            demo.ui.paste_into_focused_text("600");
            demo.snapshot_inputs();
            demo.do_rebuild();
            assert_eq!(demo.ui.focused(), Some(demo.ids.ov_prefs_browse_completion));
            assert_eq!(
                demo.ui
                    .text_input_value(demo.ids.ov_prefs_browse_completion),
                Some("600")
            );
            assert!(demo.ui.key_input(UiKey::Undo));
            assert_eq!(
                demo.ui
                    .text_input_value(demo.ids.ov_prefs_browse_completion),
                Some("1000")
            );
            demo.ui.key_input(UiKey::Redo);
            demo.snapshot_inputs();
            demo.ui.sync(&mut demo.scene);
            if let Some(folder) = &folder {
                capture(&mut demo, folder, &format!("{name}-preferences"));
            }
            demo.handle_button(demo.ids.ov_btn_ok);
            demo.do_rebuild();
            assert_eq!(demo.state.prefs.browse_completion_threshold, 600);
            let restored: crate::prefs::Prefs =
                serde_json::from_str(&serde_json::to_string(&demo.state.prefs).unwrap()).unwrap();
            assert_eq!(restored.browse_completion_threshold, 600);
            demo.state.sync_prefs_drafts();
            demo.state.prefs_browse_completion_threshold = "0".into();
            demo.state.apply_prefs_overlay().unwrap();
            assert!(demo.state.browse_cell_suggestions(1).unwrap().is_empty());
        }
    }

    fn capture(demo: &mut Demo, folder: &std::path::Path, name: &str) {
        use next_gui_render::{encode_scene, Renderer};
        demo.scene
            .mark_full_dirty(demo.win_w as f32, demo.win_h as f32);
        let dirty = demo.scene.take_dirty();
        let commands = encode_scene(&demo.scene, &dirty);
        let clear = crate::ui_build::clear_color(&demo.theme);
        let mut soft = next_gui_render_soft::SoftRenderer::new(demo.win_w, demo.win_h, clear);
        soft.render(&commands, &dirty);
        let pixels = soft.frame_pixels().unwrap().0;
        crate::write_ppm(
            folder.join(format!("{name}-soft.ppm")).to_str().unwrap(),
            pixels,
            demo.win_w,
            demo.win_h,
        )
        .unwrap();
        let mut gpu = next_gui_render_gpu::GpuRenderer::try_new(demo.win_w, demo.win_h, clear)
            .expect("Inline visual QA requires a GPU");
        gpu.render(&commands, &dirty);
        let rendered = gpu.frame_pixels().unwrap().0;
        crate::write_ppm(
            folder.join(format!("{name}-metal.ppm")).to_str().unwrap(),
            rendered,
            demo.win_w,
            demo.win_h,
        )
        .unwrap();
        let delta = pixels
            .iter()
            .zip(rendered)
            .flat_map(|(a, b)| {
                [0, 8, 16]
                    .map(|shift| (((a >> shift) & 255) as i32 - ((b >> shift) & 255) as i32).abs())
            })
            .max()
            .unwrap();
        assert!(delta <= 2, "{name}: full-frame delta={delta}");
        eprintln!("Inline QA {name}: full-frame delta={delta}");
    }

    #[test]
    fn inline_editors_and_errors_fit_narrow_font_scaled_and_frozen_tables() {
        let folder = std::env::var_os("NEXT_GUI_INLINE_QA_OUT").map(std::path::PathBuf::from);
        if let Some(folder) = &folder {
            std::fs::create_dir_all(folder).unwrap();
        }
        for (name, theme, width, height, size) in [
            ("dark-wide", Theme::dark(), 1400, 860, 13.0),
            ("light-wide", Theme::light(), 1400, 860, 18.0),
            ("dark-narrow", Theme::dark(), 800, 600, 24.0),
            ("light-narrow", Theme::light(), 800, 600, 18.0),
        ] {
            let mut demo = fixture(theme);
            demo.win_w = width;
            demo.win_h = height;
            demo.state.prefs.font_size = size;
            demo.state.browse_mut().frozen_cols = Some(1);
            select(&mut demo, 0, 2);
            key(&mut demo, UiKey::Edit);
            demo.ui.paste_into_focused_text("12.75");
            demo.snapshot_inputs();
            demo.ui.sync(&mut demo.scene);
            let editor = demo.ui.table_cell_editor(demo.ids.browse_table).unwrap();
            let node = demo
                .scene
                .iter_paint_order()
                .into_iter()
                .find(|node| demo.ui.widget_from_node(node.id) == Some(editor))
                .unwrap();
            let clip = node.clip.unwrap();
            assert!(clip.width > 30.0 && clip.height > 10.0, "{name}: {clip:?}");
            assert!(
                clip.x >= 0.0
                    && clip.y >= 0.0
                    && clip.x + clip.width <= width as f32
                    && clip.y + clip.height <= height as f32,
                "{name}: {clip:?}"
            );
            if let Some(folder) = &folder {
                capture(&mut demo, folder, &format!("{name}-edit"));
            }
            key(&mut demo, UiKey::Enter);
            assert_eq!(
                value(&demo, 9_007_199_254_740_993, "amount"),
                Value::Real(12.75)
            );
            if let Some(folder) = &folder {
                capture(&mut demo, folder, &format!("{name}-applied"));
            }
            key(&mut demo, UiKey::Char('x'));
            key(&mut demo, UiKey::Enter);
            assert_eq!(demo.state.edit_value, "x");
            if let Some(folder) = &folder {
                capture(&mut demo, folder, &format!("{name}-error"));
            }
            key(&mut demo, UiKey::Escape);
            if let Some(folder) = &folder {
                capture(&mut demo, folder, &format!("{name}-cancel"));
            }
        }
    }

    #[test]
    fn inline_qualified_composite_and_custom_view_keys_never_write_shadow_tables() {
        let mut demo = fixture(Theme::dark());
        demo.state.conn.as_ref().unwrap().execute_batch("ATTACH ':memory:' AS aux;
            CREATE TABLE aux.cells(tenant TEXT,k INTEGER,note TEXT,PRIMARY KEY(tenant,k)) WITHOUT ROWID;
            INSERT INTO aux.cells VALUES('中文',9007199254740993,'attached');
            CREATE TEMP TABLE cells(id INTEGER PRIMARY KEY,note TEXT);INSERT INTO temp.cells VALUES(1,'temporary');
            CREATE VIEW aux.v AS SELECT * FROM cells;
            CREATE TRIGGER aux.edit_v INSTEAD OF UPDATE ON v BEGIN UPDATE cells SET note=NEW.note WHERE tenant=OLD.tenant AND k=OLD.k; END;").unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.switch_browse_table_in("aux", "cells").unwrap();
        select(&mut demo, 0, 2);
        key(&mut demo, UiKey::Char('附'));
        key(&mut demo, UiKey::Enter);
        assert_eq!(
            demo.state
                .conn
                .as_ref()
                .unwrap()
                .query_row("SELECT note FROM aux.cells", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "附"
        );
        assert_eq!(
            demo.state
                .conn
                .as_ref()
                .unwrap()
                .query_row("SELECT note FROM temp.cells", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "temporary"
        );
        demo.state.switch_browse_table_in("aux", "v").unwrap();
        demo.do_rebuild();
        assert!(editable_columns(&demo.state).is_empty());
        demo.state.set_view_key(Some("k".into())).unwrap();
        select(&mut demo, 0, 2);
        key(&mut demo, UiKey::Char('视'));
        key(&mut demo, UiKey::Enter);
        assert_eq!(
            demo.state
                .conn
                .as_ref()
                .unwrap()
                .query_row("SELECT note FROM aux.cells", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "视"
        );
        select(&mut demo, 0, 2);
        key(&mut demo, UiKey::Char('失'));
        demo.state
            .conn
            .as_ref()
            .unwrap()
            .execute("DELETE FROM aux.cells", [])
            .unwrap();
        key(&mut demo, UiKey::Enter);
        assert!(demo.state.has_cell_draft());
        assert_eq!(demo.state.edit_value, "失");
    }

    #[test]
    fn inline_rowid_and_virtual_table_columns_keep_generated_fields_read_only() {
        let mut demo = fixture(Theme::dark());
        demo.state.set_show_rowid(true).unwrap();
        select(&mut demo, 0, 3);
        assert!(editable_columns(&demo.state).contains(&3));
        assert!(!editable_columns(&demo.state).contains(&6));
        key(&mut demo, UiKey::Char('7'));
        key(&mut demo, UiKey::Enter);
        assert_eq!(
            value(&demo, 9_007_199_254_740_993, "amount"),
            Value::Integer(7)
        );
        demo.state.conn.as_ref().unwrap().execute_batch("CREATE VIRTUAL TABLE words USING fts5(body);INSERT INTO words(rowid,body) VALUES(17,'original');").unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.switch_browse_table("words").unwrap();
        demo.state.set_show_rowid(true).unwrap();
        select(&mut demo, 0, 0);
        assert_eq!(editable_columns(&demo.state), vec![0, 1]);
        key(&mut demo, UiKey::Edit);
        demo.ui.paste_into_focused_text("42");
        key(&mut demo, UiKey::Tab);
        assert_eq!(
            demo.state
                .browse_cell_edit
                .as_ref()
                .map(|draft| draft.column),
            Some(1)
        );
        demo.ui.paste_into_focused_text("全文 中文");
        key(&mut demo, UiKey::Enter);
        assert_eq!(
            demo.state
                .conn
                .as_ref()
                .unwrap()
                .query_row("SELECT body FROM words WHERE rowid=42", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "全文 中文"
        );
    }

    #[test]
    fn inline_pointer_release_edits_but_drag_selects_and_discard_does_not_apply() {
        let mut demo = fixture(Theme::dark());
        let bounds = demo
            .scene
            .iter_paint_order()
            .into_iter()
            .find(|node| demo.ui.widget_from_node(node.id) == Some(demo.ids.browse_table))
            .unwrap()
            .bounds;
        let style = next_gui_ui::TableStyle::from_theme(&demo.theme);
        let widths = demo.ui.table_column_widths(demo.ids.browse_table).unwrap();
        let (x, y) = (
            bounds.x + widths[0] + 20.0,
            bounds.y + demo.ui.table_header_height(demo.ids.browse_table).unwrap() + style.row_height * 0.5,
        );
        assert!(demo.ui.pointer_pressed(&mut demo.scene, x, y));
        demo.snapshot_inputs();
        assert!(demo.state.browse_cell_edit.is_none());
        demo.ui.pointer_released();
        demo.snapshot_inputs();
        demo.handle_table_cell_edit();
        assert!(demo.state.browse_cell_edit.is_some());
        demo.ui.paste_into_focused_text("cancelled pointer draft");
        demo.snapshot_inputs();
        let discard = demo
            .scene
            .iter_paint_order()
            .into_iter()
            .find(|node| demo.ui.widget_from_node(node.id) == Some(demo.ids.btn_edit_discard))
            .unwrap()
            .bounds;
        assert!(demo.finish_browse_cell_pointer(discard.x + 10.0, discard.y + 10.0));
        assert!(demo.state.browse_cell_edit.is_none());
        assert_eq!(value(&demo, 9_007_199_254_740_993, "n"), Value::Null);
        assert!(!demo.state.dirty);
        assert!(demo.ui.pointer_pressed(&mut demo.scene, x, y));
        demo.ui.pointer_moved(x + 120.0, y + style.row_height);
        demo.ui.pointer_released();
        demo.snapshot_inputs();
        demo.handle_table_cell_edit();
        assert!(demo.state.browse_cell_edit.is_none());
        assert_eq!(
            demo.ui.table_selected_rows(demo.ids.browse_table),
            Some((0, 1))
        );
    }

    fn fixture(theme: Theme) -> Demo {
        let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
        demo.theme = theme;
        demo.state.new_memory_db().unwrap();
        demo.state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch(QA_SQL)
            .unwrap();
        demo.state.write_changes().unwrap();
        demo.state.refresh_schema().unwrap();
        demo.state.tab = MainTab::Browse;
        demo.state.switch_browse_table("cells").unwrap();
        demo.state.browse_mut().sel_row = Some(0);
        demo.state.browse_mut().sel_col = Some(1);
        demo.state.sync_edit_cell_from_selection();
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.browse_table));
        demo
    }

    fn key(demo: &mut Demo, key: UiKey) {
        assert!(demo.ui.key_input(key), "unhandled {key:?}");
        demo.snapshot_inputs();
        demo.handle_table_cell_edit();
        if !demo.rebuild {
            demo.poll_selection_changes();
        }
        if demo.rebuild {
            demo.do_rebuild();
        }
    }

    fn select(demo: &mut Demo, row: usize, column: usize) {
        demo.state.browse_mut().clear_selection();
        demo.state.browse_mut().sel_row = Some(row);
        demo.state.browse_mut().sel_col = Some(column);
        demo.state.sync_edit_cell_from_selection();
        demo.do_rebuild();
        demo.ui.set_focus(Some(demo.ids.browse_table));
    }

    fn value(demo: &Demo, id: i64, column: &str) -> Value {
        demo.state
            .conn
            .as_ref()
            .unwrap()
            .query_row(
                &format!("SELECT {} FROM cells WHERE id=?", db::quote_ident(column)),
                [id],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn inline_noops_preserve_null_large_integers_multiline_and_text_blobs() {
        for theme in [Theme::dark(), Theme::light()] {
            let mut demo = fixture(theme);
            for (row, column) in [(0, 0), (0, 1), (0, 3), (1, 1), (1, 4)] {
                select(&mut demo, row, column);
                let saved = demo.state.edit_saved.clone();
                key(&mut demo, UiKey::Edit);
                assert!(demo.state.browse_cell_edit.is_some());
                key(&mut demo, UiKey::Enter);
                assert_eq!(demo.state.edit_saved, saved);
                assert!(!demo.state.dirty);
                assert!(!demo.state.has_cell_draft());
            }
            select(&mut demo, 0, 4);
            assert!(demo.ui.key_input(UiKey::Edit));
            demo.handle_table_cell_edit();
            assert!(demo.state.browse_cell_edit.is_none());
            assert_eq!(
                value(&demo, 9_007_199_254_740_993, "binary"),
                Value::Blob(vec![0, 255])
            );
        }
    }

    #[test]
    fn inline_typing_undo_ime_rebuild_apply_and_cancel_share_the_cell_draft() {
        let mut demo = fixture(Theme::light());
        key(&mut demo, UiKey::Char('中'));
        assert_eq!(demo.state.edit_value, "中");
        assert!(demo.state.has_cell_draft());
        assert_eq!(
            demo.ui.text_content(demo.ids.edit_meta_label),
            Some(demo.state.cell_editor_caption().as_str())
        );
        let editor = demo.ui.table_cell_editor(demo.ids.browse_table).unwrap();
        assert!(demo.ui.ime_preedit("候選", Some((0, 2))));
        demo.do_rebuild();
        let current = demo.ui.table_cell_editor(demo.ids.browse_table).unwrap();
        assert_eq!(demo.ui.focused(), Some(current));
        assert!(demo.ui.key_input(UiKey::Escape));
        assert_eq!(demo.ui.take_table_cell_edit(), None);
        assert_eq!(demo.state.edit_value, "中");
        key(&mut demo, UiKey::Undo);
        assert_eq!(demo.state.edit_value, "");
        assert!(!demo.state.has_cell_draft());
        key(&mut demo, UiKey::Redo);
        assert_eq!(demo.state.edit_value, "中");
        key(&mut demo, UiKey::Enter);
        assert_eq!(
            value(&demo, 9_007_199_254_740_993, "n"),
            Value::Text("中".into())
        );
        key(&mut demo, UiKey::Char('x'));
        key(&mut demo, UiKey::Escape);
        assert_eq!(
            value(&demo, 9_007_199_254_740_993, "n"),
            Value::Text("中".into())
        );
        assert_eq!(demo.state.edit_value, "中");
        assert!(demo.ui.text_input_value(editor).is_none());
    }

    #[test]
    fn inline_tab_commits_affinity_and_tracks_an_edited_primary_key() {
        let mut demo = fixture(Theme::dark());
        select(&mut demo, 0, 0);
        key(&mut demo, UiKey::Edit);
        demo.ui.paste_into_focused_text("42");
        key(&mut demo, UiKey::Tab);
        assert_eq!(
            demo.state
                .browse_cell_edit
                .as_ref()
                .map(|draft| draft.column),
            Some(1)
        );
        assert_eq!(value(&demo, 42, "n"), Value::Null);
        assert_eq!(demo.state.edit_value, "");
        assert!(!demo.state.has_cell_draft());
        key(&mut demo, UiKey::Tab);
        assert_eq!(demo.state.edit_value, "1.25");
        assert_eq!(value(&demo, 42, "n"), Value::Null);
        demo.ui.paste_into_focused_text("12.75");
        key(&mut demo, UiKey::Tab);
        assert_eq!(value(&demo, 42, "amount"), Value::Real(12.75));
        assert_eq!(demo.state.edit_value, "保留\nline");
        assert!(!demo.state.has_cell_draft());
        key(&mut demo, UiKey::BackTab);
        assert_eq!(value(&demo, 42, "note"), Value::Text("保留\nline".into()));
        assert_eq!(
            demo.state
                .browse_cell_edit
                .as_ref()
                .map(|draft| draft.column),
            Some(2)
        );
        key(&mut demo, UiKey::Escape);
    }

    #[test]
    fn inline_failed_writes_retain_drafts_and_pending_changes_and_block_pointer_exit() {
        let mut demo = fixture(Theme::dark());
        demo.state
            .conn
            .as_ref()
            .unwrap()
            .execute(
                "UPDATE cells SET note='pending' WHERE id=9007199254740994",
                [],
            )
            .unwrap();
        demo.state.dirty = true;
        select(&mut demo, 0, 2);
        key(&mut demo, UiKey::Char('x'));
        key(&mut demo, UiKey::Enter);
        assert_eq!(demo.state.edit_value, "x");
        assert!(demo.state.has_cell_draft());
        assert!(demo.state.error_msg.as_ref().unwrap().contains("CHECK"));
        assert!(!demo.finish_browse_cell_pointer(10.0, 10.0));
        assert_eq!(
            demo.ui.focused(),
            demo.ui.table_cell_editor(demo.ids.browse_table)
        );
        assert_eq!(
            value(&demo, 9_007_199_254_740_994, "note"),
            Value::Text("pending".into())
        );
        key(&mut demo, UiKey::Escape);
        assert_eq!(
            value(&demo, 9_007_199_254_740_993, "amount"),
            Value::Real(1.25)
        );
        assert!(demo.state.dirty);
        assert!(!demo.state.has_cell_draft());
        demo.state.read_only = true;
        demo.do_rebuild();
        assert!(!demo.ui.key_input(UiKey::Edit));
        assert!(demo.state.begin_browse_cell_edit(0, 2).is_err());
        demo.state.read_only = false;
        demo.do_rebuild();
        select(&mut demo, 0, 5);
        assert!(!demo.ui.key_input(UiKey::Edit));
        assert!(demo.state.begin_browse_cell_edit(0, 5).is_err());
    }
}
