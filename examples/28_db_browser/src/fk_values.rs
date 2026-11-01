//! Typed foreign-key choices loaded on the existing database connection.
use crate::{cell_snapshot::{CellSnapshot, SnapshotWriter}, db::{self, CellValue}, export_jobs::{Job, Progress},
    state::{AppState, MainTab}};
use next_gui_ui::{SelectDataSource, SelectState, WidgetId};
use std::sync::Arc;

const CANCELLED: &str = "Foreign key loading cancelled";
pub(crate) const QA_SQL: &str = "CREATE TABLE parent_values(k INTEGER PRIMARY KEY);
    WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1203) INSERT INTO parent_values SELECT x FROM n;
    CREATE TABLE child(id INTEGER PRIMARY KEY, ref REFERENCES parent_values(k), note TEXT);
    INSERT INTO child VALUES(1,1,'unchanged');";

pub(crate) fn prepare_qa(state: &mut AppState, narrow: bool) {
    state.new_memory_db().unwrap(); state.conn.as_ref().unwrap().execute_batch(QA_SQL).unwrap();
    state.refresh_schema().unwrap(); state.switch_browse_table_in("main", "child").unwrap();
    state.tab = MainTab::Browse; state.prefs.font_size = if narrow { 18.0 } else { 13.0 };
    state.dock_plot = false; state.write_changes().unwrap(); state.begin_browse_cell_edit(0,1).unwrap();
}

#[derive(Debug)]
pub(crate) struct Choices {
    values: CellSnapshot,
    original: CellValue,
    missing: bool,
    selected: usize,
}
impl Choices {
    fn value(&self, index: usize) -> Result<CellValue, String> {
        if self.missing && index == self.values.len() { Ok(self.original.clone()) }
        else { self.values.cell(index, 0) }
    }
}
impl SelectDataSource for Choices {
    fn len(&self) -> usize { self.values.len() + usize::from(self.missing) }
    fn label(&self, index: usize) -> Result<String, String> {
        let mut label = value_label(&self.value(index)?);
        if self.missing && index == self.values.len() { label.push_str(" (current value)"); }
        Ok(label)
    }
}

fn value_label(value: &CellValue) -> String {
    let bytes = |kind: &str, value: &[u8]| {
        let prefix = value.iter().take(32).map(|byte| format!("{byte:02X}")).collect::<String>();
        format!("{kind} X'{prefix}{}' · {} bytes", if value.len() > 32 { "…" } else { "" }, value.len())
    };
    match value {
        CellValue::Null => "NULL".into(),
        CellValue::Blob(value) => bytes("BLOB", value),
        CellValue::Text(value) => match std::str::from_utf8(value) {
            Ok(text) => {
                let preview = text.chars().take(160).collect::<String>();
                format!("{}{}", serde_json::to_string(&preview).unwrap(), if text.chars().nth(160).is_some() { "…" } else { "" })
            }
            Err(_) => bytes("TEXT", value),
        },
        _ => db::cell_to_string(value.as_ref()),
    }
}

fn load(conn: &rusqlite::Connection, schema: &str, parent: &str, column: &str, nullable: bool,
    original: CellValue, progress: &Progress) -> Result<Choices, String> {
    if progress.cancelled() { return Err(CANCELLED.into()); }
    let versions = db::data_versions(conn)?;
    let column = db::quote_ident(column);
    let sql = format!("SELECT DISTINCT {column} FROM {} WHERE {column} IS NOT NULL{}",
        db::qualified_name(schema, parent), if nullable { " UNION SELECT NULL" } else { "" });
    let mut statement = conn.prepare(&sql).map_err(|error| error.to_string())?;
    if !statement.readonly() { return Err("Foreign key choices require a read-only query".into()); }
    let mut query = statement.query([]).map_err(|error| error.to_string())?;
    let mut writer = SnapshotWriter::new(1)?;
    let mut selected = None;
    let mut count = 0;
    while let Some(row) = query.next().map_err(|error| error.to_string())? {
        if progress.cancelled() { return Err(CANCELLED.into()); }
        let value = row.get_ref(0).map(CellValue::from).map_err(|error| error.to_string())?;
        if value == original { selected = Some(count); }
        writer.push(&[value])?; count += 1; progress.row();
    }
    drop(query); drop(statement);
    if versions != db::data_versions(conn)? { return Err("Database changed while loading foreign keys; refresh and retry".into()); }
    let values = writer.finish()?;
    Ok(Choices { values, original, missing: selected.is_none(), selected: selected.unwrap_or(count) })
}

#[derive(Debug, Clone)]
pub(crate) struct Draft {
    pub choices: Arc<Choices>,
    pub selected: usize,
    pub value: CellValue,
    pub session: Option<SelectState>,
    target: Target,
}
impl Draft {
    pub fn changed(&self) -> bool { self.value != self.choices.original }
    pub fn select(&mut self, index: usize) -> Result<(), String> {
        let value = self.choices.value(index)?;
        self.selected = index; self.value = value; Ok(())
    }
}

#[derive(Debug, Clone)]
struct Target {
    path: String, tab: usize, schema: String, table: String, column: String,
    row: usize, col: usize, key: db::RowKey, revision: u64, values: Arc<()>, original: CellValue,
}
impl Target {
    fn matches(&self, state: &AppState) -> bool {
        let tab = state.browse();
        state.tab == MainTab::Browse && state.path == self.path && state.browse_tab_idx == self.tab
            && state.edit_revision == self.revision && tab.schema == self.schema && tab.table.as_deref() == Some(&self.table)
            && tab.sel_row == Some(self.row) && tab.sel_col == Some(self.col) && tab.row_keys.get(self.row) == Some(&self.key)
            && tab.result.as_ref().is_some_and(|result| Arc::ptr_eq(&result.values.identity(), &self.values))
    }
}

pub struct Work {
    job: Option<Job<Choices>>,
    target: Target,
    pub progress: Arc<Progress>,
}
impl Drop for Work {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() { self.progress.cancel(); let _ = job.join(); }
    }
}

impl AppState {
    pub(crate) fn start_fk_values(&mut self, row: usize, col: usize) -> Result<bool, String> {
        self.guard_background_idle()?;
        let tab = self.browse();
        let result = tab.result.as_ref().ok_or("No Browse data")?;
        let table = tab.table.as_deref().ok_or("No Browse table")?;
        let column = result.columns.get(col).ok_or("No Browse column")?;
        let conn = self.conn.as_ref().ok_or("No database")?;
        let keys = db::list_foreign_keys_in(conn, &tab.schema, table)?;
        let Some(key) = keys.into_iter().find(|key| key.from_col.eq_ignore_ascii_case(column)) else { return Ok(false); };
        let info = db::table_columns_in(conn, &tab.schema, table)?;
        let nullable = info.iter().find(|info| info.name.eq_ignore_ascii_case(column)).is_some_and(|info| !info.notnull);
        let target = Target { path: self.path.clone(), tab: self.browse_tab_idx, schema: tab.schema.clone(), table: table.into(),
            column: column.clone(), row, col, key: tab.row_keys.get(row).cloned().ok_or("No editable row key")?,
            revision: self.edit_revision, values: result.values.identity(), original: result.values.cell(row, col)? };
        let schema = target.schema.clone(); let original = target.original.clone();
        let conn = self.conn.take().unwrap();
        let progress = Arc::new(Progress::default()); let worker = progress.clone();
        let job = Job::start_task(conn, move |conn| {
            let interrupted = worker.clone(); conn.progress_handler(500, Some(move || interrupted.cancelled()));
            let result = load(conn, &schema, &key.to_table, &key.to_col, nullable, original, &worker);
            conn.progress_handler(0, None::<fn() -> bool>);
            if worker.cancelled() { Err(CANCELLED.into()) } else { result }
        });
        self.fk_work = Some(Work { job: Some(job), target, progress });
        Ok(true)
    }

    pub(crate) fn poll_fk_values(&mut self) -> Result<bool, String> {
        if !self.fk_work.as_ref().is_some_and(|work| work.job.as_ref().is_some_and(Job::is_finished)) { return Ok(false); }
        let mut work = self.fk_work.take().unwrap();
        let (conn, result) = work.job.take().unwrap().join();
        self.conn = conn;
        if self.conn.is_none() { self.close_db(); }
        self.recover_ended_transaction()?;
        if work.progress.cancelled() { self.status_msg = CANCELLED.into(); return Ok(true); }
        let choices = Arc::new(result?);
        if !work.target.matches(self) { return Err("The foreign key edit source changed; select the cell again".into()); }
        let current = db::read_keyed_cell_in(self.conn.as_ref().ok_or("No database")?, &work.target.schema, &work.target.table, &work.target.key, &work.target.column)?;
        if current != work.target.original { return Err("The cell changed while loading foreign keys; refresh and retry".into()); }
        self.browse_cell_edit = Some(crate::browse_edit::Draft::foreign_key(self, work.target.row, work.target.col,
            Draft { selected: choices.selected, value: choices.original.clone(), choices, session: None, target: work.target.clone() }));
        self.status_msg = "Choose a referenced value · Enter/Tab to apply · Esc to cancel".into();
        Ok(true)
    }

    pub(crate) fn apply_fk_value(&mut self, value: CellValue) -> Result<(), String> {
        let draft = self.browse_cell_edit.as_ref().and_then(|draft| draft.foreign_key.as_ref()).ok_or("No foreign key edit")?;
        if !draft.target.matches(self) { return Err("The foreign key edit source changed; select the cell again".into()); }
        let original = draft.choices.original.clone();
        let (schema, table, key, column) = self.cell_apply_target()?;
        let conn = self.conn.as_ref().ok_or("No database")?;
        let (changed, updated) = db::with_savepoint(conn, || {
            let current = db::read_keyed_cell_in(conn, &schema, &table, &key, &column)?;
            if current != original { return Err("The cell changed since editing began; refresh and retry".into()); }
            if current == value { return Ok((false, Some(key.clone()))); }
            let updated = db::update_cells_in(conn, &schema, &table, std::slice::from_ref(&column), &[(key.clone(), vec![value])])?.remove(0);
            Ok((true, updated))
        })?;
        if changed { self.log_sql(format!("UPDATE {} SET {}=… -- foreign key choice", db::qualified_name(&schema, &table), db::quote_ident(&column))); self.mark_dirty(); }
        self.clear_browse_editor(); self.refresh_browse()?;
        if let Some(row) = updated.and_then(|key| self.browse().row_keys.iter().position(|candidate| *candidate == key)) {
            let col = self.browse().result.as_ref().and_then(|result| result.columns.iter().position(|name| name == &column));
            let tab = self.browse_mut(); tab.clear_selection(); tab.sel_row = Some(row); tab.sel_col = col;
            tab.goto_record = (tab.offset + row + 1).to_string(); self.sync_edit_cell_from_selection(); self.reveal_applied_cell = col.is_some();
        }
        self.status_msg = if changed { "Foreign key updated (unwritten)" } else { "Cell unchanged" }.into();
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct Controls { pub dialog: WidgetId, text: WidgetId, cancel: WidgetId }
impl Default for Controls {
    fn default() -> Self { Self { dialog: WidgetId(0), text: WidgetId(0), cancel: WidgetId(0) } }
}
pub fn build(ui: &mut next_gui_ui::Ui, state: &AppState, theme: &next_gui_ui::Theme, width: f32, height: f32) -> Controls {
    let Some(work) = &state.fk_work else { return Controls::default(); };
    let mut style = next_gui_ui::ModalStyle::from_theme(theme); style.width = (width - 80.0).min(620.0); style.height = (height - 48.0).min(230.0); style.close_on_backdrop = false;
    let mut text = next_gui_ui::TextAreaStyle::from_theme(theme); text.width = style.width - 48.0; text.height = 100.0;
    let mut progress = next_gui_ui::ProgressStyle::from_theme(theme); progress.width = text.width;
    let mut ids = Controls::default();
    ids.dialog = ui.dialog("Load referenced values", style, |body| {
        ids.text = body.text_area(format!("{} values read\nCancel keeps the current cell and pending changes.", work.progress.counts().0), "", text);
        body.progress_styled(0.0, true, progress);
    }, |footer| { ids.cancel = footer.tool_button("Cancel"); });
    ui.set_text_area_read_only(ids.text, true); ui.set_accessible_name(ids.text, "Foreign key loading progress");
    ui.set_button_enabled(ids.cancel, !work.progress.cancelled()); ui.set_dialog_open(ids.dialog, true); ui.set_focus(Some(ids.cancel)); ids
}
impl crate::Demo {
    pub(crate) fn cancel_fk_values(&mut self) { if let Some(work) = &self.state.fk_work { work.progress.cancel(); self.rebuild = true; } }
    pub(crate) fn handle_fk_values_button(&mut self, id: WidgetId) { if id == self.ids.fk_values.cancel { self.cancel_fk_values(); } }
    pub(crate) fn handle_fk_values_dismiss(&mut self) { if self.ui.take_dialog_dismiss() == Some(self.ids.fk_values.dialog) { self.cancel_fk_values(); } }
    pub(crate) fn poll_fk_values(&mut self) {
        match self.state.poll_fk_values() {
            Ok(false) => {
                if let Some(work) = &self.state.fk_work {
                    self.ui.set_text_area_value(self.ids.fk_values.text, format!("{} values read\nCancel keeps the current cell and pending changes.", work.progress.counts().0));
                    self.ui.set_button_enabled(self.ids.fk_values.cancel, !work.progress.cancelled()); self.ui.sync(&mut self.scene); self.needs_paint = true;
                }
                return;
            }
            Ok(true) => self.clear_error(), Err(error) => self.set_error(error),
        }
        self.rebuild = true; self.do_rebuild();
        if let Some(editor) = self.ui.table_cell_editor(self.ids.browse_table) { self.ui.set_focus(Some(editor)); self.ui.key_input(crate::UiKey::Space); }
        else { self.ui.set_focus(Some(self.ids.browse_table)); }
        self.ui.sync(&mut self.scene);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn state(sql: &str, schema: &str, table: &str) -> AppState {
        let mut state = AppState::empty(); state.new_memory_db().unwrap();
        state.conn.as_ref().unwrap().execute_batch("COMMIT; PRAGMA foreign_keys=ON; BEGIN;").unwrap();
        state.conn.as_ref().unwrap().execute_batch(sql).unwrap();
        state.refresh_schema().unwrap(); state.switch_browse_table_in(schema, table).unwrap();
        state.tab = MainTab::Browse; state.write_changes().unwrap(); state
    }
    fn settle(state: &mut AppState) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while state.fk_work.is_some() {
            assert!(Instant::now() < deadline, "Foreign key worker did not finish");
            state.poll_fk_values()?; std::thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }
    fn choose(state: &mut AppState, value: &CellValue) {
        let choice = state.browse_cell_edit.as_mut().unwrap().foreign_key.as_mut().unwrap();
        let index = (0..choice.choices.len()).find(|&index| choice.choices.value(index).unwrap() == *value).unwrap();
        choice.select(index).unwrap();
    }

    fn settle_demo(demo: &mut crate::Demo) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while demo.state.fk_work.is_some() { assert!(Instant::now() < deadline); demo.poll_fk_values(); std::thread::sleep(Duration::from_millis(1)); }
        assert!(demo.state.browse_cell_edit.as_ref().is_some_and(|draft| draft.foreign_key.is_some()));
    }
    fn dispatch_edit(demo: &mut crate::Demo) {
        let (table, event) = demo.ui.take_table_cell_edit().expect("Table editor event");
        assert_eq!(table, demo.ids.browse_table); demo.handle_browse_cell_edit(event);
    }

    #[test]
    fn foreign_key_editor_keeps_popup_rebuild_apply_cancel_tab_and_progress_focus() {
        use crate::browse_filter::tests::{fixture, capture};
        use next_gui_ui::{Theme, UiKey};
        for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400,860),(800,600)] {
                let mut demo = fixture(width,height,theme);
                demo.state = state(QA_SQL,"main","child");
                demo.do_rebuild(); demo.state.begin_browse_cell_edit(0,1).unwrap(); demo.do_rebuild();
                assert_eq!(demo.ui.active_dialog(),Some(demo.ids.fk_values.dialog));
                capture(&mut demo,&format!("fk-progress-{name}-{width}")); settle_demo(&mut demo);
                let editor = demo.ui.table_cell_editor(demo.ids.browse_table).unwrap();
                assert_eq!(demo.ui.focused(),Some(editor)); assert_eq!(demo.ui.select_value(editor),Some("1")); assert!(demo.ui.has_open_popup());
                capture(&mut demo,&format!("fk-options-{name}-{width}"));
                demo.ui.key_input(UiKey::End); demo.snapshot_browse_cell_editor(); assert!(demo.state.has_cell_draft());
                assert_eq!(demo.ui.select_value(editor),Some("1203"));
                demo.do_rebuild(); let editor = demo.ui.table_cell_editor(demo.ids.browse_table).unwrap();
                assert_eq!(demo.ui.focused(),Some(editor)); assert!(demo.ui.has_open_popup()); assert_eq!(demo.ui.select_value(editor),Some("1203"));
                capture(&mut demo,&format!("fk-last-{name}-{width}"));
                demo.ui.key_input(UiKey::Enter); assert!(demo.ui.take_table_cell_edit().is_none());
                demo.ui.key_input(UiKey::Enter); dispatch_edit(&mut demo);
                assert!(demo.state.dirty); assert!(demo.state.browse_cell_edit.is_none()); assert_eq!(demo.ui.focused(),Some(demo.ids.browse_table));
                assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT ref FROM child",[],|row|row.get::<_,i64>(0)).unwrap(),1203);
                capture(&mut demo,&format!("fk-applied-{name}-{width}"));
                demo.ui.key_input(UiKey::Edit); dispatch_edit(&mut demo); settle_demo(&mut demo);
                demo.ui.key_input(UiKey::Home); demo.snapshot_browse_cell_editor(); assert!(demo.state.has_cell_draft());
                demo.ui.key_input(UiKey::Escape); assert!(demo.ui.take_table_cell_edit().is_none());
                demo.ui.key_input(UiKey::Escape); dispatch_edit(&mut demo);
                assert!(!demo.state.has_cell_draft()); assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT ref FROM child",[],|row|row.get::<_,i64>(0)).unwrap(),1203);
                demo.ui.key_input(UiKey::Edit); dispatch_edit(&mut demo); settle_demo(&mut demo);
                demo.ui.key_input(UiKey::Home); demo.ui.key_input(UiKey::Tab); dispatch_edit(&mut demo);
                assert!(demo.state.conn.as_ref().unwrap().query_row("SELECT ref IS NULL FROM child",[],|row|row.get::<_,bool>(0)).unwrap());
                assert_eq!(demo.state.browse_cell_edit.as_ref().unwrap().column,2);
                demo.ui.key_input(UiKey::Escape); dispatch_edit(&mut demo); demo.state.revert_changes().unwrap(); demo.do_rebuild();
                demo.state.begin_browse_cell_edit(0,1).unwrap(); demo.do_rebuild();
                demo.handle_button(demo.ids.fk_values.cancel);
                let deadline = Instant::now() + Duration::from_secs(10);
                while demo.state.fk_work.is_some() { assert!(Instant::now() < deadline); demo.poll_fk_values(); std::thread::sleep(Duration::from_millis(1)); }
                assert!(demo.state.browse_cell_edit.is_none()); assert!(demo.state.conn.is_some()); assert!(!demo.state.dirty);
                assert_eq!(demo.ui.focused(),Some(demo.ids.browse_table));
            }
        }
    }

    #[test]
    fn foreign_key_choices_preserve_namespace_and_every_sqlite_storage_type() {
        let mut state = state("CREATE TABLE parent(k PRIMARY KEY); INSERT INTO parent VALUES('wrong main');
            CREATE TEMP TABLE parent(k PRIMARY KEY); INSERT INTO temp.parent VALUES('wrong temp');
            ATTACH ':memory:' AS \"aux.名前\";
            CREATE TABLE \"aux.名前\".parent(k PRIMARY KEY);
            INSERT INTO \"aux.名前\".parent VALUES('NULL'),(''),(9223372036854775807),(1.25),(x'00FF'),(CAST(x'FF' AS TEXT));
            CREATE TABLE \"aux.名前\".child(id INTEGER PRIMARY KEY, ref REFERENCES parent);
            INSERT INTO \"aux.名前\".child VALUES(1,9223372036854775807);", "aux.名前", "child");
        for value in [CellValue::Text(b"NULL".to_vec()), CellValue::Text(Vec::new()), CellValue::Integer(i64::MAX),
            CellValue::Real(1.25), CellValue::Blob(vec![0,255]), CellValue::Text(vec![255]), CellValue::Null] {
            state.begin_browse_cell_edit(0,1).unwrap();
            assert!(state.conn.is_none()); assert!(state.guard_background_idle().is_err());
            assert!(state.refresh_browse().is_err()); settle(&mut state).unwrap();
            let choices = &state.browse_cell_edit.as_ref().unwrap().foreign_key.as_ref().unwrap().choices;
            assert_eq!(choices.len(), 7); assert!(!choices.missing);
            assert!((0..choices.len()).all(|index| choices.value(index).unwrap() != CellValue::Text(b"wrong main".to_vec())));
            choose(&mut state, &value); assert_eq!(state.has_cell_draft(), value != CellValue::Integer(i64::MAX));
            state.edit_mode = crate::state::EditCellMode::Json;
            assert!(!state.prepare_cell_format_confirmation().unwrap(), "typed choices must bypass presentation formatting");
            state.apply_cell_draft().unwrap();
            let key = &state.browse().row_keys[0];
            assert_eq!(db::read_keyed_cell_in(state.conn.as_ref().unwrap(), "aux.名前", "child", key, "ref").unwrap(), value);
            assert_eq!(state.dirty, value != CellValue::Integer(i64::MAX));
            state.revert_changes().unwrap();
        }
        assert_ne!(value_label(&CellValue::Null), value_label(&CellValue::Text(b"NULL".to_vec())));
        assert_ne!(value_label(&CellValue::Text(vec![255])), value_label(&CellValue::Blob(vec![255])));
        assert!(value_label(&CellValue::Text("🙂".repeat(10000).into_bytes())).len() < 1000);
    }

    #[test]
    fn implicit_composite_references_keep_other_fields_and_retain_failed_drafts() {
        let mut state = state("CREATE TABLE parent(a,b,PRIMARY KEY(a,b)) WITHOUT ROWID;
            INSERT INTO parent VALUES('A',1),('A',2),('B',3);
            CREATE TABLE child(id INTEGER PRIMARY KEY,x NOT NULL,y NOT NULL,FOREIGN KEY(x,y) REFERENCES parent);
            INSERT INTO child VALUES(1,'A',1);", "main", "child");
        state.begin_browse_cell_edit(0,2).unwrap(); settle(&mut state).unwrap();
        let choices = &state.browse_cell_edit.as_ref().unwrap().foreign_key.as_ref().unwrap().choices;
        assert_eq!(choices.len(),3); assert!((0..choices.len()).all(|index| choices.value(index).unwrap() != CellValue::Null));
        choose(&mut state,&CellValue::Integer(3));
        assert!(state.apply_fk_value(CellValue::Integer(3)).unwrap_err().contains("FOREIGN KEY"));
        assert!(state.has_cell_draft()); assert!(!state.dirty);
        assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT y FROM child",[],|row|row.get::<_,i64>(0)).unwrap(),1);
        choose(&mut state,&CellValue::Integer(2)); state.apply_fk_value(CellValue::Integer(2)).unwrap();
        assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT x,y FROM child",[],|row|Ok((row.get::<_,String>(0)?,row.get::<_,i64>(1)?))).unwrap(),("A".into(),2));
        assert!(state.dirty); state.revert_changes().unwrap();
    }

    #[test]
    fn foreign_key_load_cancel_failures_and_stale_edits_restore_the_connection() {
        let mut state = state("CREATE TABLE parent(k PRIMARY KEY); INSERT INTO parent VALUES(1),(2);
            CREATE TABLE child(id INTEGER PRIMARY KEY, ref REFERENCES parent(k)); INSERT INTO child VALUES(1,1);", "main", "child");
        state.conn.as_ref().unwrap().execute("INSERT INTO parent VALUES(9)",[]).unwrap(); state.mark_dirty();
        state.begin_browse_cell_edit(0,1).unwrap(); state.fk_work.as_ref().unwrap().progress.cancel(); settle(&mut state).unwrap();
        assert!(state.conn.is_some()); assert!(state.browse_cell_edit.is_none()); assert!(state.dirty);
        assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT count(*) FROM parent",[],|row|row.get::<_,i64>(0)).unwrap(),3);
        state.begin_browse_cell_edit(0,1).unwrap(); settle(&mut state).unwrap(); choose(&mut state,&CellValue::Integer(9));
        state.apply_cell_draft().unwrap();
        assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT ref FROM child",[],|row|row.get::<_,i64>(0)).unwrap(),9);
        state.revert_changes().unwrap();
        state.begin_browse_cell_edit(0,1).unwrap(); state.path.push_str("changed");
        assert!(settle(&mut state).unwrap_err().contains("source changed")); assert!(state.conn.is_some()); state.path = ":memory:".into();
        state.begin_browse_cell_edit(0,1).unwrap(); settle(&mut state).unwrap(); choose(&mut state,&CellValue::Integer(2));
        state.conn.as_ref().unwrap().execute("UPDATE child SET ref=2",[]).unwrap();
        assert!(state.apply_fk_value(CellValue::Integer(2)).unwrap_err().contains("cell changed")); assert!(state.has_cell_draft());
        state.discard_cell_draft(); state.revert_changes().unwrap();
        state.begin_browse_cell_edit(0,1).unwrap(); settle(&mut state).unwrap(); choose(&mut state,&CellValue::Integer(2));
        state.browse_mut().schema = "temp".into(); assert!(state.apply_fk_value(CellValue::Integer(2)).unwrap_err().contains("source changed"));
        state.browse_mut().schema = "main".into(); state.discard_cell_draft();
        state.read_only = true; assert!(state.begin_browse_cell_edit(0,1).is_err()); state.read_only = false;
        state.conn.as_ref().unwrap().execute_batch("COMMIT; PRAGMA foreign_keys=OFF; BEGIN; DROP TABLE child; CREATE TABLE child(id INTEGER PRIMARY KEY,ref REFERENCES absent(k)); INSERT INTO child(id) VALUES(1);").unwrap();
        state.refresh_schema().unwrap(); state.refresh_browse().unwrap(); state.begin_browse_cell_edit(0,1).unwrap();
        assert!(settle(&mut state).unwrap_err().contains("absent")); assert!(state.conn.is_some()); assert!(state.browse_cell_edit.is_none());
    }

    #[test]
    fn foreign_key_loading_reads_every_parent_row_and_preserves_orphans() {
        let mut state = state("CREATE TABLE parent(k PRIMARY KEY); WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1203) INSERT INTO parent SELECT x FROM n;
            CREATE TABLE child(id INTEGER PRIMARY KEY,ref REFERENCES parent(k)); INSERT INTO child VALUES(1,1203);", "main", "child");
        state.begin_browse_cell_edit(0,1).unwrap(); settle(&mut state).unwrap();
        let choices = &state.browse_cell_edit.as_ref().unwrap().foreign_key.as_ref().unwrap().choices;
        assert_eq!(choices.len(),1204); assert_eq!(choices.value(choices.selected).unwrap(),CellValue::Integer(1203));
        choose(&mut state,&CellValue::Integer(1202)); state.apply_fk_value(CellValue::Integer(1202)).unwrap();
        state.revert_changes().unwrap(); state.conn.as_ref().unwrap().execute_batch("COMMIT; PRAGMA foreign_keys=OFF; BEGIN; UPDATE child SET ref=9999;").unwrap(); state.refresh_browse().unwrap();
        state.begin_browse_cell_edit(0,1).unwrap(); settle(&mut state).unwrap();
        let choices = &state.browse_cell_edit.as_ref().unwrap().foreign_key.as_ref().unwrap().choices;
        assert!(choices.missing); assert_eq!(choices.len(),1205); assert!(choices.label(choices.selected).unwrap().contains("current value"));
        assert!(!state.has_cell_draft()); state.apply_fk_value(CellValue::Integer(9999)).unwrap(); assert!(!state.dirty);
    }

    #[test]
    fn save_all_applies_the_selected_raw_value_without_formatting_or_text_roundtrips() {
        let mut state = state("CREATE TABLE parent(k PRIMARY KEY); INSERT INTO parent VALUES(1),(CAST(x'FF' AS TEXT));
            CREATE TABLE child(id INTEGER PRIMARY KEY,ref REFERENCES parent(k)); INSERT INTO child VALUES(1,1);", "main", "child");
        state.checkpoint_project(); state.begin_browse_cell_edit(0,1).unwrap(); settle(&mut state).unwrap();
        choose(&mut state,&CellValue::Text(vec![255])); state.edit_mode = crate::state::EditCellMode::Json;
        assert!(!state.prepare_cell_format_confirmation().unwrap());
        let folder = tempfile::tempdir().unwrap(); let output = folder.path().join("saved.sqlite");
        state.save_documents(Some(&output),None).unwrap();
        assert!(!state.dirty); assert!(state.browse_cell_edit.is_none());
        let saved = db::open_readonly(output.to_str().unwrap()).unwrap();
        assert_eq!(db::execute_sql(&saved,"SELECT ref FROM child").unwrap().values.cell(0,0).unwrap(),CellValue::Text(vec![255]));
    }

    #[test]
    fn foreign_key_primary_key_edits_restore_the_moved_record() {
        let mut state = state("CREATE TABLE parent(k INTEGER PRIMARY KEY); INSERT INTO parent VALUES(1),(2);
            CREATE TABLE child(k INTEGER PRIMARY KEY REFERENCES parent(k)); INSERT INTO child VALUES(1);", "main", "child");
        state.begin_browse_cell_edit(0,0).unwrap(); settle(&mut state).unwrap(); choose(&mut state,&CellValue::Integer(2));
        state.apply_cell_draft().unwrap();
        assert_eq!(state.browse().sel_row,Some(0)); assert_eq!(state.browse().sel_col,Some(0)); assert_eq!(state.edit_col,"k");
        assert_eq!(state.edit_saved,rusqlite::types::Value::Integer(2)); assert_eq!(state.browse().row_keys[0].values,vec![CellValue::Integer(2)]);
        assert!(state.dirty); state.revert_changes().unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().values.cell(0,0).unwrap(),CellValue::Integer(1));
    }
}
