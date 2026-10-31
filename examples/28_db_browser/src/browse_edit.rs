//! Browse cell editors share the typed cell draft and its existing write/rollback guards.
use crate::{
    db,
    state::{AppState, EditCellMode, MainTab},
    Demo, UiKey,
};
use next_gui_ui::{FlexBuilder, TableCellEditEvent, TextInputState, TextInputStyle, WidgetId};

pub(crate) const QA_SQL: &str = "CREATE TABLE cells(id INTEGER PRIMARY KEY, n, amount NUMERIC CHECK(typeof(amount) IN ('integer','real') AND amount>0), note TEXT, binary BLOB, computed AS(amount*2));
    INSERT INTO cells VALUES(9007199254740993,NULL,1.25,'保留\nline',x'00FF');
    INSERT INTO cells VALUES(9007199254740994,9007199254740993,2.5,'second',x'6869');";

#[derive(Debug, Clone)]
pub struct Draft {
    pub row: usize,
    pub column: usize,
    revision: u64,
    original: String,
    storage: usize,
    input: Option<TextInputState>,
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
) {
    if let Some(draft) = &state.browse_cell_edit {
        builder.table_cell_editor(table, draft.row, draft.column, |cell| {
            cell.text_input_styled(&state.edit_value, "", style)
        });
    }
}

impl AppState {
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
        self.browse_cell_edit = Some(Draft {
            row,
            column,
            revision: self.edit_revision,
            original: self.edit_value.clone(),
            storage: self.edit_type,
            input: None,
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
        draft.input = self.ui.text_input_state(editor);
        if let Some(value) = self.ui.text_input_value(editor).map(str::to_owned) {
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
            bounds.y + style.header_height + style.row_height * 0.5,
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
