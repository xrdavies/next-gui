//! Explicit, source-bound view editing through a user-selected unique column.
use next_gui_ui::{ListStyle, ModalStyle, TextAreaStyle, Theme, Ui, WidgetId};

use crate::{db, state::AppState, Demo};

pub const QA_SQL: &str = "CREATE TABLE shipments(id INTEGER PRIMARY KEY,region TEXT,note TEXT,weight REAL CHECK(weight>=0));
    WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<18)
    INSERT INTO shipments SELECT i,CASE WHEN i%2=0 THEN 'Tokyo' ELSE 'Paris' END,'Dispatch '||i,i*1.25 FROM n;
    CREATE VIEW shipment_view AS SELECT id AS \"Shipment ID\",region AS Destination,note AS \"Dispatch note\",weight AS Weight FROM shipments;
    CREATE TRIGGER shipment_update INSTEAD OF UPDATE ON shipment_view BEGIN
      UPDATE shipments SET id=new.\"Shipment ID\",region=new.Destination,note=upper(new.\"Dispatch note\"),weight=new.Weight WHERE id=old.\"Shipment ID\"; END;
    CREATE TRIGGER shipment_delete INSTEAD OF DELETE ON shipment_view BEGIN DELETE FROM shipments WHERE id=old.\"Shipment ID\"; END;
    CREATE TRIGGER shipment_insert INSTEAD OF INSERT ON shipment_view BEGIN
      INSERT INTO shipments(id,region,note,weight) VALUES(new.\"Shipment ID\",new.Destination,upper(new.\"Dispatch note\"),new.Weight); END;";

#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    path: String,
    browse: usize,
    schema: String,
    table: String,
    columns: Vec<String>,
    key: Option<String>,
}

impl Source {
    pub fn new(state: &AppState) -> Result<Self, String> {
        let tab = state.browse();
        let table = tab.table.as_deref().ok_or("Select a table or view")?;
        let conn = state.conn.as_ref().ok_or("No database open")?;
        Ok(Self {
            path: state.path.clone(),
            browse: state.browse_tab_idx,
            schema: tab.schema.clone(),
            table: table.into(),
            columns: db::table_column_names_in(conn, &tab.schema, table)?,
            key: tab.unlock_view_pk.clone(),
        })
    }

    pub fn matches(&self, state: &AppState) -> bool {
        Self::new(state).is_ok_and(|source| source == *self)
    }
}

#[derive(Debug, Clone)]
pub struct Draft {
    source: Source,
    revision: u64,
    pub selected: Option<usize>,
    scroll: f32,
    reveal_selection: bool,
    body_scroll: (f32, f32),
    focus: usize,
    pub error: Option<String>,
}

impl Draft {
    pub fn new(state: &mut AppState) -> Result<Self, String> {
        state.guard_writable()?;
        if state.has_cell_draft() {
            return Err("Apply or discard the cell draft first".into());
        }
        let source = Source::new(state)?;
        if !db::source_is_view_in(state.conn.as_ref().unwrap(), &source.schema, &source.table)? {
            return Err("Select a view to unlock editing".into());
        }
        let selected = source
            .key
            .as_ref()
            .and_then(|key| source.columns.iter().position(|name| name == key))
            .or_else(|| (!source.columns.is_empty()).then_some(0));
        Ok(Self {
            source,
            revision: state.edit_revision,
            selected,
            scroll: 0.0,
            reveal_selection: true,
            body_scroll: (0.0, 0.0),
            focus: 0,
            error: None,
        })
    }
}

impl AppState {
    pub fn browse_is_view(&self) -> bool {
        let tab = self.browse();
        self.schema.iter().any(|object| {
            object.schema == tab.schema
                && Some(object.name.as_str()) == tab.table.as_deref()
                && object.kind == db::ObjectKind::View
        })
    }

    pub fn browse_editing_unlocked(&self) -> bool {
        self.browse().table.is_some()
            && (!self.browse_is_view() || self.browse().unlock_view_pk.is_some())
    }

    pub fn guard_browse_editable(&self) -> Result<(), String> {
        let tab = self.browse();
        let table = tab.table.as_deref().ok_or("Select a table or view")?;
        let conn = self.conn.as_ref().ok_or("No database open")?;
        if db::source_is_view_in(conn, &tab.schema, table)? {
            let key = tab
                .unlock_view_pk
                .as_ref()
                .ok_or("Unlock view editing and choose a unique column first")?;
            if !db::table_column_names_in(conn, &tab.schema, table)?
                .iter()
                .any(|column| column.eq_ignore_ascii_case(key))
            {
                return Err(
                    "The view key column no longer exists; choose a unique column again".into(),
                );
            }
        }
        Ok(())
    }

    pub fn set_view_key(&mut self, key: Option<String>) -> Result<(), String> {
        self.guard_writable()?;
        if self.has_cell_draft() {
            return Err("Apply or discard the cell draft first".into());
        }
        let source = Source::new(self)?;
        if !db::source_is_view_in(self.conn.as_ref().unwrap(), &source.schema, &source.table)? {
            return Err("Select a view to unlock editing".into());
        }
        let key = key
            .map(|key| {
                source
                    .columns
                    .iter()
                    .find(|name| name.eq_ignore_ascii_case(&key))
                    .cloned()
                    .ok_or("The view key column no longer exists")
            })
            .transpose()?;
        let previous = self.browse().clone();
        self.browse_mut().unlock_view_pk = key.clone();
        if let Err(error) = self.refresh_browse() {
            *self.browse_mut() = previous;
            return Err(error);
        }
        self.status_msg = key
            .map(|key| {
                format!("View editing unlocked using {key}; INSTEAD OF triggers are required")
            })
            .unwrap_or_else(|| "View editing locked".into());
        Ok(())
    }

    fn apply_view_key(&mut self, draft: &Draft) -> Result<(), String> {
        if !draft.source.matches(self) || draft.revision != self.edit_revision {
            return Err("The view source changed; cancel and reopen Unlock View Editing".into());
        }
        let key = draft
            .selected
            .and_then(|index| draft.source.columns.get(index))
            .ok_or("Select a unique view column")?;
        self.set_view_key(Some(key.clone()))
    }
}

#[derive(Clone, Copy)]
pub struct Controls {
    pub dialog: WidgetId,
    pub list: WidgetId,
    pub apply: WidgetId,
    pub cancel: WidgetId,
    pub error: WidgetId,
    help: WidgetId,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            dialog: WidgetId(0),
            list: WidgetId(0),
            apply: WidgetId(0),
            cancel: WidgetId(0),
            error: WidgetId(0),
            help: WidgetId(0),
        }
    }
}

impl Controls {
    fn focus_order(self) -> [WidgetId; 3] {
        [self.list, self.apply, self.cancel]
    }
}

pub fn build(ui: &mut Ui, state: &AppState, theme: &Theme, width: f32) -> Controls {
    let Some(draft) = &state.view_key_dialog else {
        return Controls::default();
    };
    let mut controls = Controls::default();
    let mut modal = ModalStyle::from_theme(theme);
    modal.width = 560.0;
    modal.height = if draft.error.is_some() { 370.0 } else { 314.0 };
    modal.padding = 12.0;
    modal.title_height = 36.0;
    modal.action_height = 32.0;
    let content_width = modal.width.min((width - 40.0).max(width * 0.75)) - 2.0 * modal.padding;
    let mut text = TextAreaStyle::from_theme(theme);
    text.width = content_width;
    text.height = 78.0;
    let mut list = ListStyle::from_theme(theme);
    list.width = content_width;
    list.height = 112.0;
    list.item_height = theme.typography.body.max(18.0) + 10.0;
    let mut description = WidgetId(0);
    controls.dialog = ui.dialog("Unlock View Editing", modal, |body| {
        description = body.text_area("Choose a unique column to identify records in this view. Editing requires suitable INSTEAD OF triggers. Each write checks that the selected key identifies exactly one record.", "", text);
        controls.help = body.tooltip("", |host| {
            controls.list = host.list(draft.source.columns.iter().map(|name| name.replace(['\r','\n','\t'], " ")), list);
        });
        if let Some(error) = &draft.error { text.height = 48.0; controls.error = body.text_area(error, "", text); }
    }, |footer| {
        controls.apply = footer.tool_button_styled("Unlock", theme.colors.accent, theme.colors.on_accent);
        controls.cancel = footer.tool_button("Cancel");
    });
    ui.set_text_area_read_only(description, true);
    ui.set_accessible_name(description, "View editing requirements");
    ui.set_text_area_read_only(controls.error, true);
    ui.set_accessible_name(controls.error, "View key validation error");
    let help = draft
        .selected
        .and_then(|index| draft.source.columns.get(index))
        .map(|name| format!("Selected unique view key column: {name}"))
        .unwrap_or_else(|| "Select a unique view key column".into());
    ui.set_accessible_name(controls.list, &help);
    ui.set_tooltip_text(controls.help, help);
    ui.set_list_selected(controls.list, draft.selected);
    ui.set_list_activatable(controls.list, true);
    ui.set_button_enabled(controls.apply, draft.selected.is_some());
    ui.set_dialog_open(controls.dialog, true);
    controls
}

pub fn restore(ui: &mut Ui, controls: Controls, draft: Option<&Draft>) {
    let Some(draft) = draft else {
        return;
    };
    ui.set_list_scroll_offset(controls.list, draft.scroll);
    if draft.reveal_selection {
        ui.reveal_list_selection(controls.list);
    }
    if let Some(body) = ui.dialog_body(controls.dialog) {
        ui.set_scroll_offset(body, draft.body_scroll.0, draft.body_scroll.1);
    }
    ui.set_focus(controls.focus_order().get(draft.focus).copied());
}

impl Demo {
    pub(crate) fn open_view_key_dialog(&mut self) {
        self.snapshot_inputs();
        match Draft::new(&mut self.state) {
            Ok(draft) => {
                self.state.view_key_dialog = Some(draft);
                self.clear_error();
            }
            Err(error) => self.set_error(error),
        }
        self.rebuild = true;
    }

    pub(crate) fn snapshot_view_key_dialog(&mut self) {
        if self.rebuild {
            return;
        }
        let Some(draft) = &mut self.state.view_key_dialog else {
            return;
        };
        let controls = self.ids.view_keys;
        if controls.dialog.0 == 0 {
            return;
        }
        draft.selected = self.ui.list_selected(controls.list);
        draft.reveal_selection = false;
        let help = draft
            .selected
            .and_then(|index| draft.source.columns.get(index))
            .map(|name| format!("Selected unique view key column: {name}"))
            .unwrap_or_else(|| "Select a unique view key column".into());
        self.ui.set_accessible_name(controls.list, &help);
        self.ui.set_tooltip_text(controls.help, help);
        draft.scroll = self
            .ui
            .list_scroll_offset(controls.list)
            .unwrap_or_default();
        if let Some(body) = self.ui.dialog_body(controls.dialog) {
            draft.body_scroll = self.ui.scroll_offset(body).unwrap_or_default();
        }
        if let Some(index) = controls
            .focus_order()
            .iter()
            .position(|id| self.ui.focused() == Some(*id))
        {
            draft.focus = index;
        }
        self.ui
            .set_button_enabled(controls.apply, draft.selected.is_some());
    }

    pub(crate) fn handle_view_key_button(&mut self, id: WidgetId) {
        let controls = self.ids.view_keys;
        if id == controls.cancel {
            self.close_view_key_dialog();
            return;
        }
        if id != controls.apply {
            return;
        }
        self.snapshot_view_key_dialog();
        let Some(draft) = self.state.view_key_dialog.clone() else {
            return;
        };
        match self.state.apply_view_key(&draft) {
            Ok(()) => {
                self.clear_error();
                self.close_view_key_dialog();
            }
            Err(error) => {
                self.state.view_key_dialog.as_mut().unwrap().error = Some(error);
                self.rebuild = true;
            }
        }
    }

    pub(crate) fn handle_view_key_dismiss(&mut self) {
        if self.ui.take_dialog_dismiss() == Some(self.ids.view_keys.dialog) {
            self.close_view_key_dialog();
        }
    }

    fn close_view_key_dialog(&mut self) {
        self.state.view_key_dialog = None;
        self.do_rebuild();
        self.ui.set_focus(Some(self.ids.browse_table));
        self.ui.sync(&mut self.scene);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{BrowseTabState, MainTab, OverlayKind};
    use rusqlite::{types::Value, Connection};

    fn fixture() -> AppState {
        let mut state = AppState::empty();
        state.new_memory_db().unwrap();
        state.conn.as_ref().unwrap().execute_batch(QA_SQL).unwrap();
        state.write_changes().unwrap();
        state.refresh_schema().unwrap();
        state.tab = MainTab::Browse;
        state.switch_browse_table("shipment_view").unwrap();
        state.prefs.auto_switch_mode = false;
        state
    }

    fn select(state: &mut AppState, row: usize, column: usize) {
        let tab = state.browse_mut();
        tab.sel_row = Some(row);
        tab.sel_col = Some(column);
        tab.sel_row_start = None;
        tab.sel_row_end = None;
        tab.sel_col_start = None;
        tab.sel_col_end = None;
        state.sync_edit_cell_from_selection();
    }

    #[test]
    fn view_trigger_keys_use_exact_values_qualified_sources_and_atomic_writes() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("ATTACH ':memory:' AS \"aux.\"\"db\"; CREATE TABLE keeper(v); INSERT INTO keeper VALUES('saved'); BEGIN; INSERT INTO keeper VALUES('pending');").unwrap();
        for schema in ["main", "temp", "aux.\"db"] {
            let q = db::quote_ident(schema);
            conn.execute_batch(&format!("CREATE TABLE {q}.records(id INTEGER PRIMARY KEY,note,weight CHECK(weight>=0)); INSERT INTO {q}.records VALUES(9007199254740993,'before',1),(9,'second',2);
                CREATE TABLE {q}.audit(v);
                CREATE VIEW {q}.\"view.\"\"name\" AS SELECT id AS \"key.\"\"name\",note,weight FROM records;
                CREATE TRIGGER {q}.update_view INSTEAD OF UPDATE ON \"view.\"\"name\" BEGIN INSERT INTO audit VALUES(new.note); UPDATE records SET id=new.\"key.\"\"name\",note=upper(new.note),weight=new.weight WHERE id=old.\"key.\"\"name\"; END;
                CREATE TRIGGER {q}.delete_view INSTEAD OF DELETE ON \"view.\"\"name\" BEGIN DELETE FROM records WHERE id=old.\"key.\"\"name\"; END;")).unwrap();
            let (result, keys) = db::browse_view_with_key_in(
                &conn,
                schema,
                "view.\"name",
                "key.\"name",
                &[],
                None,
                (500, 0),
            )
            .unwrap();
            assert_eq!(result.columns, ["key.\"name", "note", "weight"]);
            let key = keys
                .iter()
                .find(|key| key.values == [db::CellValue::Integer(9007199254740993)])
                .unwrap();
            let before = conn.total_changes();
            let saved = db::update_cell_in(
                &conn,
                schema,
                "view.\"name",
                key,
                "note",
                &Value::Text("saved 中文".into()),
            )
            .unwrap()
            .unwrap();
            assert_eq!(conn.changes(), 0);
            assert!(conn.total_changes() > before);
            assert_eq!(
                db::read_keyed_cell_in(&conn, schema, "view.\"name", &saved, "note").unwrap(),
                db::CellValue::Text("SAVED 中文".as_bytes().to_vec())
            );
            let moved = db::update_cell_in(
                &conn,
                schema,
                "view.\"name",
                &saved,
                "key.\"name",
                &Value::Text("00042".into()),
            )
            .unwrap()
            .unwrap();
            assert_eq!(moved.values, [db::CellValue::Integer(42)]);
            let other = keys
                .iter()
                .find(|key| key.values == [db::CellValue::Integer(9)])
                .unwrap();
            let audit = db::count_browse_rows_in(&conn, schema, "audit", None).unwrap();
            assert!(db::update_cells_in(
                &conn,
                schema,
                "view.\"name",
                &["weight".into()],
                &[
                    (moved.clone(), vec![db::CellValue::Integer(8)]),
                    (other.clone(), vec![db::CellValue::Integer(-1)])
                ]
            )
            .is_err());
            assert_eq!(
                db::read_keyed_cell_in(&conn, schema, "view.\"name", &moved, "weight").unwrap(),
                db::CellValue::Integer(1)
            );
            assert_eq!(
                db::count_browse_rows_in(&conn, schema, "audit", None).unwrap(),
                audit
            );
            assert_eq!(
                db::delete_row_in(&conn, schema, "view.\"name", &moved).unwrap(),
                1
            );
            assert_eq!(conn.changes(), 0);
            assert_eq!(
                db::count_browse_rows_in(&conn, schema, "records", None).unwrap(),
                1
            );
            assert!(db::browse_view_with_key_in(
                &conn,
                schema,
                "view.\"name",
                "missing",
                &[],
                None,
                (500, 0)
            )
            .unwrap_err()
            .contains("column"));
            assert!(db::browse_view_with_key_in(
                &conn,
                schema,
                "records",
                "id",
                &[],
                None,
                (500, 0)
            )
            .unwrap_err()
            .contains("views"));
        }
        assert!(!conn.is_autocommit());
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(db::count_rows(&conn, "keeper").unwrap(), 1);
    }

    #[test]
    fn view_keys_reject_ambiguous_ignored_and_missing_trigger_mutations() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE records(k,note); INSERT INTO records VALUES(NULL,'a'),(NULL,'b'),(9,'unique');
            CREATE VIEW duplicate_view AS SELECT * FROM records;
            CREATE TRIGGER dup_update INSTEAD OF UPDATE ON duplicate_view BEGIN UPDATE records SET note=new.note WHERE k IS old.k; END;
            CREATE TRIGGER dup_delete INSTEAD OF DELETE ON duplicate_view BEGIN DELETE FROM records WHERE k IS old.k; END;
            CREATE VIEW ignored_view AS SELECT * FROM records;
            CREATE TRIGGER ignore_update INSTEAD OF UPDATE ON ignored_view BEGIN SELECT RAISE(IGNORE); END;
            CREATE TRIGGER ignore_delete INSTEAD OF DELETE ON ignored_view BEGIN SELECT RAISE(IGNORE); END;
            CREATE VIEW silent_view AS SELECT * FROM records;
            CREATE TRIGGER silent_update INSTEAD OF UPDATE ON silent_view BEGIN SELECT 1; END;
            CREATE TRIGGER silent_delete INSTEAD OF DELETE ON silent_view BEGIN SELECT 1; END;
            CREATE VIEW no_trigger AS SELECT * FROM records;").unwrap();
        let (_, keys) =
            db::browse_view_with_key_in(&conn, "main", "duplicate_view", "k", &[], None, (500, 0))
                .unwrap();
        assert!(db::update_cells_in(
            &conn,
            "main",
            "duplicate_view",
            &["note".into()],
            &[
                (
                    keys[2].clone(),
                    vec![db::CellValue::Text(b"changed".to_vec())]
                ),
                (keys[0].clone(), vec![db::CellValue::Null])
            ]
        )
        .unwrap_err()
        .contains("not unique"));
        assert!(db::delete_row_in(&conn, "main", "duplicate_view", &keys[0])
            .unwrap_err()
            .contains("not unique"));
        assert_eq!(
            db::read_keyed_cell_in(&conn, "main", "duplicate_view", &keys[2], "note").unwrap(),
            db::CellValue::Text(b"unique".to_vec())
        );
        for view in ["ignored_view", "silent_view", "no_trigger"] {
            let (_, keys) =
                db::browse_view_with_key_in(&conn, "main", view, "k", &[], Some("k=9"), (500, 0))
                    .unwrap();
            assert!(
                db::update_cell_in(&conn, "main", view, &keys[0], "note", &Value::Null).is_err()
            );
            assert!(db::delete_row_in(&conn, "main", view, &keys[0]).is_err());
        }
        assert_eq!(db::count_rows(&conn, "records").unwrap(), 3);
    }

    #[test]
    fn view_unlock_apply_reload_insert_relock_and_stale_drafts() {
        let mut state = fixture();
        select(&mut state, 0, 2);
        assert!(state.edit_cell_read_only());
        assert!(state.open_insert_row().unwrap_err().contains("Unlock"));
        assert!(state.cut_browse_cells().is_err());
        state.set_view_key(Some("shipment id".into())).unwrap();
        assert!(!state.dirty);
        assert_eq!(
            state.browse().unlock_view_pk.as_deref(),
            Some("Shipment ID")
        );
        select(&mut state, 0, 2);
        state.edit_value = "saved dispatch 中文".into();
        assert!(state.set_view_key(None).is_err());
        state.apply_cell_draft().unwrap();
        assert_eq!(state.edit_value, "SAVED DISPATCH 中文");
        assert!(!state.has_cell_draft());
        select(&mut state, 0, 0);
        state.edit_value = "42".into();
        state.apply_cell_draft().unwrap();
        assert_eq!(state.edit_value, "42");
        assert_eq!(
            state.browse().row_keys[state.browse().sel_row.unwrap()].values,
            [db::CellValue::Integer(42)]
        );
        state.open_insert_row().unwrap();
        state.insert_modes = vec![2, 2, 2, 2];
        state.insert_values = vec![
            "100".into(),
            "Kyoto".into(),
            "new record".into(),
            "3.5".into(),
        ];
        state.apply_insert_row().unwrap();
        assert_eq!(
            state
                .conn
                .as_ref()
                .unwrap()
                .query_row("SELECT note FROM shipments WHERE id=100", [], |row| row
                    .get::<_, String>(
                    0
                ))
                .unwrap(),
            "NEW RECORD"
        );
        state.open_insert_row().unwrap();
        let retained = state.insert_values.clone();
        state.switch_browse_table("shipments").unwrap();
        assert!(state
            .apply_insert_row()
            .unwrap_err()
            .contains("source changed"));
        assert_eq!(state.insert_values, retained);
        state.overlay = None;
        state.switch_browse_table("shipment_view").unwrap();
        assert!(state.browse().unlock_view_pk.is_some());
        let draft = Draft::new(&mut state).unwrap();
        state
            .browse_tabs
            .push(BrowseTabState::new(Some("shipment_view".into())));
        state.browse_tab_idx = 1;
        assert!(state
            .apply_view_key(&draft)
            .unwrap_err()
            .contains("source changed"));
        state.browse_tab_idx = 0;
        state.set_view_key(None).unwrap();
        assert!(state.browse().row_keys.is_empty());
        assert!(state.open_insert_row().is_err());
        state.read_only = true;
        assert!(Draft::new(&mut state).is_err());
        state.read_only = false;
        state.revert_changes().unwrap();
        assert_eq!(
            db::count_rows(state.conn.as_ref().unwrap(), "shipments").unwrap(),
            18
        );
    }

    #[test]
    fn view_insert_ignored_or_failed_retains_values_and_pending_transaction() {
        let mut state = fixture();
        state.set_view_key(Some("Shipment ID".into())).unwrap();
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE pending(v); INSERT INTO pending VALUES('keep'); DROP TRIGGER shipment_insert; CREATE TRIGGER shipment_insert INSTEAD OF INSERT ON shipment_view BEGIN SELECT RAISE(IGNORE); END;").unwrap();
        state.open_insert_row().unwrap();
        state.insert_modes = vec![2, 2, 2, 2];
        state.insert_values = vec!["100".into(), "Kyoto".into(), "kept".into(), "4".into()];
        let values = state.insert_values.clone();
        assert!(state.apply_insert_row().unwrap_err().contains("ignored"));
        assert_eq!(state.insert_values, values);
        assert_eq!(state.overlay, Some(OverlayKind::InsertRow));
        assert_eq!(
            db::count_rows(state.conn.as_ref().unwrap(), "pending").unwrap(),
            1
        );
        assert!(!state.conn.as_ref().unwrap().is_autocommit());
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("DROP TRIGGER shipment_insert;")
            .unwrap();
        assert!(state.apply_insert_row().is_err());
        assert_eq!(state.insert_values, values);
    }

    fn finish(demo: &mut Demo) {
        demo.handle_column_list_activate();
        demo.snapshot_inputs();
        if demo.rebuild {
            demo.do_rebuild();
        }
        demo.ui.sync(&mut demo.scene);
    }

    #[test]
    fn view_key_dialog_context_keyboard_cancel_relock_and_trigger_apply() {
        use crate::{state::DockKind, PresentBackend, Theme, UiKey};
        let folder = std::env::var("NEXT_GUI_VIEW_KEYS_QA_OUT")
            .ok()
            .map(std::path::PathBuf::from);
        if let Some(folder) = &folder {
            std::fs::create_dir_all(folder).unwrap();
        }
        for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height, font) in [(1400, 860, 13.0), (800, 600, 18.0)] {
                let mut demo = Demo::new(width, height, PresentBackend::Softbuffer);
                demo.state = fixture();
                demo.state.prefs.font_size = font;
                demo.state.dock_focus = DockKind::Plot;
                demo.state.body_ratio = 0.45;
                demo.state.dock_ratio = 0.62;
                demo.theme = crate::ui_build::browser_theme(theme, font);
                demo.state.sync_current_plot().unwrap();
                demo.state.plot.check(3, 1, true);
                select(&mut demo.state, 0, 2);
                demo.do_rebuild();
                let capture = |demo: &mut Demo, stage: &str| {
                    if let Some(folder) = &folder {
                        crate::plot_export::capture(
                            demo,
                            folder,
                            &format!("view-keys-{stage}-{name}-{width}"),
                        );
                    }
                };
                assert!(!demo.ui.button_enabled(demo.ids.btn_add_row).unwrap());
                assert!(!demo.ui.button_enabled(demo.ids.btn_del_row).unwrap());
                capture(&mut demo, "locked");
                let bounds = demo
                    .scene
                    .iter_paint_order()
                    .into_iter()
                    .find(|node| demo.ui.widget_from_node(node.id) == Some(demo.ids.browse_table))
                    .unwrap()
                    .bounds;
                assert!(demo.open_browse_header_menu(bounds.x + 30.0, bounds.y + 10.0));
                demo.ui.sync(&mut demo.scene);
                capture(&mut demo, "menu");
                assert!(demo.ui.key_input(UiKey::End));
                assert!(demo.ui.key_input(UiKey::Enter));
                assert_eq!(
                    demo.ui.take_menu_action(),
                    Some((demo.ids.context_menu, crate::mid::BROWSE_UNLOCK_VIEW))
                );
                demo.handle_menu(crate::mid::BROWSE_UNLOCK_VIEW);
                finish(&mut demo);
                assert_eq!(demo.ui.active_dialog(), Some(demo.ids.view_keys.dialog));
                assert_eq!(demo.ui.focused(), Some(demo.ids.view_keys.list));
                let options = demo.state.browse().options();
                demo.handle_button(demo.ids.btn_write);
                demo.handle_menu(crate::mid::FILE_NEW_MEM);
                assert!(demo.state.view_key_dialog.is_some());
                assert_eq!(demo.state.browse().options(), options);
                capture(&mut demo, "dialog");
                demo.ui.key_input(UiKey::Escape);
                demo.handle_view_key_dismiss();
                finish(&mut demo);
                assert!(demo.state.view_key_dialog.is_none());
                assert_eq!(demo.state.browse().options(), options);
                assert_eq!(demo.ui.focused(), Some(demo.ids.browse_table));
                demo.open_view_key_dialog();
                finish(&mut demo);
                assert!(demo.ui.key_input(UiKey::Enter));
                finish(&mut demo);
                assert!(
                    demo.state.view_key_dialog.is_none(),
                    "{:?}",
                    demo.state
                        .view_key_dialog
                        .as_ref()
                        .and_then(|draft| draft.error.as_ref())
                );
                select(&mut demo.state, 0, 2);
                demo.do_rebuild();
                assert!(demo.ui.button_enabled(demo.ids.btn_add_row).unwrap());
                assert!(demo.ui.button_enabled(demo.ids.btn_del_row).unwrap());
                capture(&mut demo, "unlocked");
                demo.ui.set_focus(Some(demo.ids.edit_value));
                demo.ui.key_input(UiKey::SelectAll);
                demo.ui.paste_into_focused_text("trigger saved 中文");
                demo.snapshot_inputs();
                demo.ui.set_focus(Some(demo.ids.btn_edit_cell));
                demo.do_rebuild();
                assert!(demo.ui.key_input(UiKey::Enter));
                let id = demo.ui.take_button_activate().unwrap();
                demo.handle_button(id);
                finish(&mut demo);
                assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
                assert_eq!(demo.state.edit_value, "TRIGGER SAVED 中文");
                assert!(!demo.state.has_cell_draft());
                capture(&mut demo, "applied");
                demo.state.open_insert_row().unwrap();
                demo.state.insert_modes = vec![2, 2, 2, 2];
                demo.state.insert_values = vec![
                    "100".into(),
                    "Kyoto".into(),
                    "new dispatch".into(),
                    "4.5".into(),
                ];
                demo.do_rebuild();
                let value = demo.ids.insert_values[2];
                demo.ui.set_focus(Some(value));
                demo.ui.key_input(UiKey::SelectAll);
                demo.ui.paste_into_focused_text("retained insertion");
                demo.snapshot_inputs();
                demo.do_rebuild();
                assert_eq!(demo.ui.focused(), Some(value));
                assert!(demo.ui.key_input(UiKey::Undo));
                assert_eq!(demo.ui.text_input_value(value), Some("new dispatch"));
                assert!(demo.ui.key_input(UiKey::Redo));
                assert_eq!(demo.ui.text_input_value(value), Some("retained insertion"));
                let mode = demo.ids.insert_modes[2];
                demo.ui.set_focus(Some(mode));
                demo.ui.key_input(UiKey::Space);
                demo.snapshot_inputs();
                let selected = demo.ui.select_selected(mode);
                assert_eq!(demo.ui.popup_owner(), Some(mode));
                demo.do_rebuild();
                assert_eq!(demo.ui.focused(), Some(mode));
                assert_eq!(demo.ui.select_selected(mode), selected);
                assert_eq!(demo.ui.popup_owner(), Some(mode));
                demo.ui.key_input(UiKey::Escape);
                for id in [demo.ids.ov_btn_ok, demo.ids.ov_btn_cancel] {
                    demo.ui.set_focus(Some(id));
                    demo.do_rebuild();
                    assert_eq!(demo.ui.focused(), Some(id));
                }
                demo.ui.set_focus(Some(demo.ids.ov_btn_ok));
                demo.do_rebuild();
                demo.ui.key_input(UiKey::Enter);
                let id = demo.ui.take_button_activate().unwrap();
                demo.handle_button(id);
                finish(&mut demo);
                assert!(demo.state.overlay.is_none(), "{:?}", demo.state.error_msg);
                assert_eq!(
                    demo.state
                        .conn
                        .as_ref()
                        .unwrap()
                        .query_row("SELECT note FROM shipments WHERE id=100", [], |row| row
                            .get::<_, String>(
                            0
                        ))
                        .unwrap(),
                    "RETAINED INSERTION"
                );
                demo.handle_menu(crate::mid::BROWSE_UNLOCK_VIEW);
                finish(&mut demo);
                assert!(demo.state.browse().unlock_view_pk.is_none());
                assert!(demo.state.browse().row_keys.is_empty());
                capture(&mut demo, "relocked");
            }
        }
    }

    #[test]
    fn view_key_dialog_long_columns_rebuild_resize_and_stale_source_error() {
        use crate::{PresentBackend, Theme, UiKey};
        for theme in [Theme::dark(), Theme::light()] {
            let mut demo = Demo::new(1400, 860, PresentBackend::Softbuffer);
            demo.state = fixture();
            demo.theme = crate::ui_build::browser_theme(theme, 18.0);
            let columns = (0..48)
                .map(|i| {
                    format!(
                        "id AS {}",
                        db::quote_ident(&format!("Unique shipment column {i} — long 中文 label"))
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            demo.state
                .conn
                .as_ref()
                .unwrap()
                .execute_batch(&format!(
                    "CREATE VIEW wide_view AS SELECT {columns} FROM shipments;"
                ))
                .unwrap();
            demo.state.refresh_schema().unwrap();
            demo.state.switch_browse_table("wide_view").unwrap();
            demo.do_rebuild();
            demo.open_view_key_dialog();
            finish(&mut demo);
            demo.ui.key_input(UiKey::End);
            demo.snapshot_inputs();
            assert_eq!(
                demo.state.view_key_dialog.as_ref().unwrap().selected,
                Some(47)
            );
            assert!(demo.ui.list_scroll_offset(demo.ids.view_keys.list).unwrap() > 0.0);
            demo.win_w = 800;
            demo.win_h = 600;
            demo.do_rebuild();
            assert_eq!(demo.ui.list_selected(demo.ids.view_keys.list), Some(47));
            assert!(demo
                .ui
                .accessible_name(demo.ids.view_keys.list)
                .unwrap()
                .contains("column 47 — long 中文 label"));
            assert!(demo.ui.list_scroll_offset(demo.ids.view_keys.list).unwrap() > 0.0);
            assert_eq!(demo.ui.focused(), Some(demo.ids.view_keys.list));
            let options = demo.state.browse().options();
            demo.state
                .browse_tabs
                .push(BrowseTabState::new(Some("shipment_view".into())));
            demo.state.browse_tab_idx = 1;
            demo.ui.key_input(UiKey::Enter);
            finish(&mut demo);
            assert!(demo
                .state
                .view_key_dialog
                .as_ref()
                .unwrap()
                .error
                .as_deref()
                .unwrap()
                .contains("source changed"));
            assert!(demo
                .ui
                .text_area_value(demo.ids.view_keys.error)
                .unwrap()
                .contains("source changed"));
            demo.handle_button(demo.ids.view_keys.cancel);
            finish(&mut demo);
            demo.state.browse_tab_idx = 0;
            assert_eq!(demo.state.browse().options(), options);
            demo.state
                .set_view_key(Some("Unique shipment column 47 — long 中文 label".into()))
                .unwrap();
            demo.open_view_key_dialog();
            finish(&mut demo);
            assert_eq!(demo.ui.list_selected(demo.ids.view_keys.list), Some(47));
            assert!(demo.ui.list_scroll_offset(demo.ids.view_keys.list).unwrap() > 0.0);
            demo.handle_button(demo.ids.view_keys.cancel);
            finish(&mut demo);
            demo.state.read_only = true;
            demo.open_view_key_dialog();
            finish(&mut demo);
            assert!(demo.state.view_key_dialog.is_none());
        }
    }
}
