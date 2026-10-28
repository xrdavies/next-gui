//! Original SQL export selection/options and their modal workflow.
use crate::db::{self, ExportSqlOptions, ObjectKind};
use next_gui_ui::{Ui, WidgetId};

pub struct Draft {
    pub options: ExportSqlOptions,
    pub objects: Vec<(String, ObjectKind, bool)>,
    pub selected: Option<usize>,
    pub scroll: (f32, f32),
}
impl Draft {
    pub fn new(
        conn: &rusqlite::Connection,
        options: &ExportSqlOptions,
        selection: Option<&str>,
    ) -> Result<Self, String> {
        let mut objects: Vec<_> = db::list_schema(conn)?
            .into_iter()
            .filter(|object| matches!(object.kind, ObjectKind::Table | ObjectKind::View))
            .map(|object| (object.name, object.kind, false))
            .collect();
        objects.sort_by_key(|entry| entry.0.to_lowercase());
        let selection = selection.filter(|name| objects.iter().any(|entry| entry.0 == *name));
        for (name, _, selected) in &mut objects {
            *selected = selection.is_none_or(|current| current == name);
        }
        Ok(Self {
            options: options.clone(),
            objects,
            selected: Some(0),
            scroll: (0.0, 0.0),
        })
    }
    fn filename(&self, database: &str) -> String {
        let selected: Vec<_> = self.objects.iter().filter(|entry| entry.2).collect();
        let name = if selected.len() == 1 {
            selected[0].0.as_str()
        } else if database == ":memory:" {
            "database"
        } else {
            std::path::Path::new(database)
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("database")
        };
        let csv = crate::data_export::filename("main", name, false);
        format!("{}.sql", csv.strip_suffix(".csv").unwrap())
    }
    pub fn options(&self) -> Result<ExportSqlOptions, String> {
        let selected: Vec<_> = self
            .objects
            .iter()
            .filter(|entry| entry.2)
            .map(|entry| entry.0.clone())
            .collect();
        if selected.is_empty() {
            return Err("Select at least one table or view".into());
        }
        Ok(ExportSqlOptions {
            selected: Some(selected),
            ..self.options.clone()
        })
    }
}

#[derive(Clone, Copy)]
pub struct Controls {
    pub dialog: WidgetId,
    pub table: WidgetId,
    all: WidgetId,
    none: WidgetId,
    what: WidgetId,
    existing: WidgetId,
    names: WidgetId,
    multiple: WidgetId,
    original: WidgetId,
    foreign_keys: WidgetId,
    export: WidgetId,
    cancel: WidgetId,
    progress_dialog: WidgetId,
    progress_text: WidgetId,
    progress_bar: WidgetId,
    stop: WidgetId,
}
impl Default for Controls {
    fn default() -> Self {
        let zero = WidgetId(0);
        Self {
            dialog: zero,
            table: zero,
            all: zero,
            none: zero,
            what: zero,
            existing: zero,
            names: zero,
            multiple: zero,
            original: zero,
            foreign_keys: zero,
            export: zero,
            cancel: zero,
            progress_dialog: zero,
            progress_text: zero,
            progress_bar: zero,
            stop: zero,
        }
    }
}
pub struct Session {
    focus: usize,
    select: Option<next_gui_ui::SelectState>,
}
impl Controls {
    pub fn progress_ids(&self) -> (WidgetId, WidgetId, WidgetId) {
        (self.progress_text, self.progress_bar, self.stop)
    }
    fn focus_ids(&self) -> [WidgetId; 11] {
        [
            self.table,
            self.all,
            self.none,
            self.what,
            self.existing,
            self.names,
            self.multiple,
            self.original,
            self.foreign_keys,
            self.export,
            self.cancel,
        ]
    }
    pub fn session(&self, ui: &Ui) -> Option<Session> {
        if ui.active_dialog() != Some(self.dialog) {
            return None;
        }
        self.focus_ids()
            .iter()
            .enumerate()
            .find(|(_, id)| id.0 != 0 && ui.focused() == Some(**id))
            .map(|(focus, id)| Session {
                focus,
                select: ui.select_state(*id),
            })
    }
    pub fn restore(&self, ui: &mut Ui, session: &Session) {
        if ui.active_dialog() != Some(self.dialog) {
            return;
        }
        let id = self.focus_ids()[session.focus];
        if id.0 == 0 {
            return;
        }
        if let Some(select) = &session.select {
            ui.restore_select_state(id, select);
        }
        ui.set_focus(Some(id));
    }
}

pub fn build(
    ui: &mut Ui,
    state: &crate::state::AppState,
    theme: &next_gui_ui::Theme,
    width: f32,
    height: f32,
) -> Controls {
    use next_gui_ui::{
        ModalStyle, RowStyle, SelectStyle, SwitchStyle, TableColumn, TableStyle, TextAreaStyle,
    };
    let mut ids = Controls::default();
    let Some(draft) = state
        .sql_export
        .as_ref()
        .filter(|_| state.overlay == Some(crate::state::OverlayKind::ExportSql))
    else {
        return ids;
    };
    let mut style = ModalStyle::from_theme(theme);
    style.width = (width - 48.0).min(740.0);
    style.height = (height - 48.0).min(720.0);
    style.close_on_backdrop = false;
    let inner = style.width - 2.0 * style.padding;
    let mut error = WidgetId(0);
    ids.dialog = ui.dialog(
        "Export database as SQL",
        style,
        |body| {
            if let Some(message) = &state.error_msg {
                let mut text = TextAreaStyle::from_theme(theme);
                text.width = inner;
                text.height = 64.0;
                error = body.text_area(message, "", text);
            }
            body.text("Tables and views");
            let mut grid = TableStyle::from_theme(theme);
            grid.width = inner - 12.0;
            grid.height = (style.height * 0.3).clamp(132.0, 216.0);
            ids.table = body.table(
                vec![
                    TableColumn::new("Use", 44.0),
                    TableColumn::new("Name", grid.width - 2.0 * grid.padding - 140.0),
                    TableColumn::new("Type", 96.0),
                ],
                draft
                    .objects
                    .iter()
                    .map(|(name, kind, _)| {
                        vec![String::new(), name.clone(), kind.sql_type().into()]
                    })
                    .collect(),
                grid,
            );
            let mut row = RowStyle::from_theme(theme);
            row.width = inner;
            row.height = 34.0;
            row.padding = 0.0;
            row.background = None;
            body.row(row, |row| {
                ids.all = row.tool_button("Select All");
                ids.none = row.tool_button("Deselect All");
            });
            let mut select = SelectStyle::from_theme(theme);
            select.width = inner;
            select.bg = theme.colors.surface;
            body.text("What to export");
            ids.what = body.select(
                ["Everything", "Structure only", "Data only"],
                Some(if !draft.options.structure {
                    2
                } else if !draft.options.data {
                    1
                } else {
                    0
                }),
                "Export contents",
                select,
            );
            if draft.options.structure {
                body.text("Existing schema");
                ids.existing = body.select(
                    [
                        "Keep · CREATE IF NOT EXISTS",
                        "Replace · DROP before CREATE",
                    ],
                    Some(usize::from(draft.options.drop_if_exists)),
                    "Existing schema",
                    select,
                );
            }
            let sw = SwitchStyle::from_theme(theme);
            ids.names = body.switch_styled(
                "Column names in INSERT statements",
                draft.options.insert_column_names,
                sw,
            );
            ids.multiple = body.switch_styled(
                "Multiple rows in each INSERT statement",
                draft.options.multiple_rows,
                sw,
            );
            ids.original = body.switch_styled(
                "Keep original CREATE statements",
                draft.options.keep_original_create,
                sw,
            );
            ids.foreign_keys = body.switch_styled(
                "Suspend foreign keys during restore",
                draft.options.keep_fk,
                sw,
            );
        },
        |footer| {
            ids.cancel = footer.tool_button("Cancel");
            ids.export =
                footer.tool_button_styled("Export…", theme.colors.accent, theme.colors.on_accent);
        },
    );
    ui.set_text_area_read_only(error, true);
    ui.set_table_check_cells(
        ids.table,
        draft
            .objects
            .iter()
            .map(|entry| vec![Some(entry.2), None, None])
            .collect(),
    );
    ui.set_table_selected_col(ids.table, Some(0));
    ui.set_table_selected_row(ids.table, draft.selected);
    ui.set_table_scroll_offset(ids.table, draft.scroll.0, draft.scroll.1);
    ui.set_accessible_name(
        ids.table,
        "SQL tables and views to export; Space toggles inclusion",
    );
    ui.set_accessible_name(ids.what, "SQL export contents");
    ui.set_accessible_name(ids.existing, "SQL export existing schema policy");
    ui.set_button_enabled(ids.export, draft.objects.iter().any(|entry| entry.2));
    ui.set_dialog_open(ids.dialog, true);
    ui.set_focus(Some(ids.cancel));
    if let Some(work) = &state.export_work {
        (
            ids.progress_dialog,
            ids.progress_text,
            ids.progress_bar,
            ids.stop,
        ) = crate::export_jobs::build_progress(ui, work, theme, width, height);
    }
    ids
}

impl crate::Demo {
    pub(crate) fn action_export_sql(&mut self) {
        if let Err(error) = self.state.guard_background_idle() {
            self.set_error(error);
            return;
        }
        let result = self
            .state
            .conn
            .as_ref()
            .ok_or("Open an idle database before exporting".into())
            .and_then(|conn| {
                if self.state.sql_busy {
                    return Err("Wait for SQL to finish before exporting".into());
                }
                let selection = if self.state.tab == crate::state::MainTab::Browse
                    && self.state.browse().schema == "main"
                {
                    self.state.browse().table.as_deref()
                } else {
                    self.state
                        .selected
                        .as_ref()
                        .filter(|object| object.schema == "main")
                        .map(|object| object.name.as_str())
                };
                Draft::new(conn, &self.state.prefs.sql_export, selection)
            });
        match result {
            Ok(draft) => {
                self.state.sql_export = Some(draft);
                self.state.overlay = Some(crate::state::OverlayKind::ExportSql);
                self.clear_error();
                self.rebuild = true;
            }
            Err(error) => self.set_error(error),
        }
    }
    pub(crate) fn snapshot_sql_export(&mut self) {
        let ids = self.ids.sql_export;
        if self.state.export_work.is_some() {
            return;
        }
        if ids.dialog.0 == 0 || self.state.overlay != Some(crate::state::OverlayKind::ExportSql) {
            return;
        }
        let Some(draft) = &mut self.state.sql_export else {
            return;
        };
        let before = draft.options.structure;
        if let Some(what) = self.ui.select_selected(ids.what) {
            draft.options.structure = what != 2;
            draft.options.data = what != 1;
        }
        if let Some(existing) = self.ui.select_selected(ids.existing) {
            draft.options.drop_if_exists = existing == 1;
        }
        for (id, value) in [
            (ids.names, &mut draft.options.insert_column_names),
            (ids.multiple, &mut draft.options.multiple_rows),
            (ids.original, &mut draft.options.keep_original_create),
            (ids.foreign_keys, &mut draft.options.keep_fk),
        ] {
            if let Some(checked) = self.ui.is_checked(id) {
                *value = checked;
            }
        }
        draft.selected = self.ui.table_selected_row(ids.table);
        if let Some(scroll) = self.ui.table_scroll_offset(ids.table) {
            draft.scroll = scroll;
        }
        if before != draft.options.structure {
            self.rebuild = true;
        }
    }
    pub(crate) fn handle_sql_export_dismiss(&mut self) {
        if let Some(id) = self.ui.take_dialog_dismiss() {
            if self.state.export_work.is_some()
                && (id == self.ids.sql_export.dialog || id == self.ids.sql_export.progress_dialog)
            {
                self.cancel_data_export();
            } else if id == self.ids.sql_export.dialog {
                self.close_sql_export();
            }
        }
    }

    fn close_sql_export(&mut self) {
        self.state.sql_export = None;
        self.state.overlay = None;
        self.rebuild = true;
    }
    pub(crate) fn handle_sql_export_button(&mut self, id: WidgetId) {
        if self.state.export_work.is_some() {
            if id == self.ids.sql_export.stop {
                self.cancel_data_export();
            }
            return;
        }
        self.snapshot_sql_export();
        let ids = self.ids.sql_export;
        if id == ids.cancel {
            self.close_sql_export();
        } else if id == ids.export {
            self.action_export_sql_do();
        } else if id == ids.all || id == ids.none {
            if let Some(draft) = &mut self.state.sql_export {
                for entry in &mut draft.objects {
                    entry.2 = id == ids.all;
                }
            }
            self.rebuild = true;
        }
    }
    pub(crate) fn action_export_sql_do(&mut self) {
        if let Err(error) = self
            .state
            .sql_export
            .as_ref()
            .ok_or("Open SQL export options")
            .and_then(|draft| {
                draft
                    .options()
                    .map(|_| ())
                    .map_err(|_| "Select at least one table or view")
            })
        {
            self.set_error(error);
            return;
        }
        let filename = self
            .state
            .sql_export
            .as_ref()
            .unwrap()
            .filename(&self.state.path);
        let Some(path) = crate::file_dlg::pick_save_sql(Some(&filename)) else {
            return;
        };
        self.perform_sql_export(path);
    }
    fn perform_sql_export(&mut self, path: std::path::PathBuf) {
        if let Err(error) = self.state.guard_background_idle() {
            self.set_error(error);
            return;
        }
        let options = match self
            .state
            .sql_export
            .as_ref()
            .ok_or("Open SQL export options".into())
            .and_then(Draft::options)
        {
            Ok(options) => options,
            Err(error) => {
                self.set_error(error);
                return;
            }
        };
        let Some(conn) = self.state.conn.take() else {
            self.set_error("No idle database available");
            return;
        };
        self.state.io_path = path.display().to_string();
        self.state
            .log_sql(format!("-- export SQL to {}", path.display()));
        let progress = std::sync::Arc::new(crate::export_jobs::Progress::default());
        self.state.export_work = Some(crate::export_jobs::Work {
            job: crate::export_jobs::Job::start_sql(conn, path, options.clone(), progress.clone()),
            progress,
            options: crate::export_jobs::Options::Sql(options),
            count: 1,
        });
        self.clear_error();
        self.rebuild = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    #[test]
    fn merging_autoincrement_keeps_both_source_and_destination_high_water_marks() {
        let source = Connection::open_in_memory().unwrap();
        source.execute_batch("CREATE TABLE t(id INTEGER PRIMARY KEY AUTOINCREMENT,v);INSERT INTO t VALUES(2001,'deleted');DELETE FROM t;INSERT INTO t VALUES(10,'source')").unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("merge.sql");
        db::export_sql_dump_opts(&source, &path, &ExportSqlOptions::default()).unwrap();
        for (previous, expected) in [(9000, 9001), (20, 2002)] {
            let target = Connection::open_in_memory().unwrap();
            target
                .execute_batch("CREATE TABLE t(id INTEGER PRIMARY KEY AUTOINCREMENT,v)")
                .unwrap();
            target
                .execute("INSERT INTO t(id,v) VALUES(?1,'deleted')", [previous])
                .unwrap();
            target.execute("DELETE FROM t", []).unwrap();
            target
                .execute_batch(&std::fs::read_to_string(&path).unwrap())
                .unwrap();
            target
                .execute("INSERT INTO t(v) VALUES('next')", [])
                .unwrap();
            assert_eq!(target.last_insert_rowid(), expected);
        }
    }
    #[test]
    fn create_options_keep_quoted_names_comments_and_deferred_constraints() {
        let source = Connection::open_in_memory().unwrap();
        source.execute_batch("PRAGMA foreign_keys=ON;CREATE TABLE \"IF\"(id INTEGER PRIMARY KEY);CREATE TABLE /* source */ \"表\"(v INTEGER REFERENCES \"IF\"(id) CONSTRAINT \"later\" DEFERRABLE INITIALLY DEFERRED);CREATE INDEX \"table\" ON \"表\"(v) WHERE v>0;CREATE VIEW \"view\" AS SELECT v FROM \"表\";CREATE TRIGGER \"trigger\" AFTER INSERT ON \"表\" BEGIN SELECT 1; END").unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("schema.sql");
        for original in [true, false] {
            let options = ExportSqlOptions {
                keep_original_create: original,
                ..Default::default()
            };
            db::export_sql_dump_opts(&source, &path, &options).unwrap();
            let target = Connection::open_in_memory().unwrap();
            let sql = std::fs::read_to_string(&path).unwrap();
            target.execute_batch(&sql).unwrap();
            target.execute_batch(&sql).unwrap();
            assert_eq!(db::list_schema(&target).unwrap().len(), 5);
            target
                .execute_batch("BEGIN;INSERT INTO \"表\" VALUES(9)")
                .unwrap();
            assert!(target.execute_batch("COMMIT").is_err());
            target
                .execute_batch("INSERT INTO \"IF\" VALUES(9);COMMIT")
                .unwrap();
        }
        let raw = "CREATE /* retain */ TABLE /* before name */ \"IF\"(v)";
        assert_eq!(
            crate::table_sql::export_create(raw, true, true).unwrap(),
            "CREATE /* retain */ TABLE IF NOT EXISTS /* before name */ \"IF\"(v)"
        );
        let existing = "CREATE TABLE /* keep */ IF NOT EXISTS \"IF\"(v)";
        assert_eq!(
            crate::table_sql::export_create(existing, true, true).unwrap(),
            existing
        );
    }
    #[test]
    fn sql_export_progress_cancel_preserves_destination_and_pending_edits_then_retries() {
        use crate::{csv_files::interaction_tests::capture, ui_build, Demo, PresentBackend};
        use next_gui_ui::{Theme, UiKey};
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400, 1000), (800, 600)] {
                let mut demo = Demo::new(width, height, PresentBackend::Softbuffer);
                demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
                demo.state.new_memory_db().unwrap();
                demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE t(v);WITH RECURSIVE n(v) AS (VALUES(1) UNION ALL SELECT v+1 FROM n WHERE v<1000) INSERT INTO t SELECT v FROM n").unwrap();
                demo.state.dirty = true;
                demo.state.refresh_schema().unwrap();
                demo.state.selected = None;
                demo.action_export_sql();
                demo.do_rebuild();
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("kept.sql");
                std::fs::write(&path, "previous").unwrap();
                let (entered, waiting) = std::sync::mpsc::channel();
                let (release, resume) = std::sync::mpsc::channel();
                // Pause at SQLite's next VM step after the first row has been staged.
                let progress = std::sync::Arc::new(crate::export_jobs::Progress::default());
                let observing = progress.clone();
                let mut paused = false;
                demo.state.conn.as_ref().unwrap().progress_handler(
                    1,
                    Some(move || {
                        if !paused && observing.counts().0 >= 1 {
                            paused = true;
                            entered.send(()).unwrap();
                            let _ = resume.recv();
                        }
                        false
                    }),
                );
                let options = demo.state.sql_export.as_ref().unwrap().options().unwrap();
                demo.state.export_work = Some(crate::export_jobs::Work {
                    job: crate::export_jobs::Job::start_sql(
                        demo.state.conn.take().unwrap(),
                        path.clone(),
                        options.clone(),
                        progress.clone(),
                    ),
                    progress,
                    options: crate::export_jobs::Options::Sql(options),
                    count: 1,
                });
                waiting
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap();
                demo.do_rebuild();
                assert_eq!(
                    demo.ui.active_dialog(),
                    Some(demo.ids.sql_export.progress_dialog)
                );
                assert_eq!(demo.ui.focused(), Some(demo.ids.sql_export.stop));
                assert!(demo.state.guard_background_idle().is_err());
                capture(&mut demo, theme_name, width, "sql-export-progress");
                demo.ui.key_input(UiKey::Escape);
                demo.handle_sql_export_dismiss();
                demo.do_rebuild();
                assert!(demo
                    .state
                    .export_work
                    .as_ref()
                    .unwrap()
                    .progress
                    .cancelled());
                assert!(demo.state.sql_export.is_some());
                capture(&mut demo, theme_name, width, "sql-export-canceling");
                release.send(()).unwrap();
                demo.settle_export_test();
                demo.do_rebuild();
                assert!(demo.state.error_msg.is_none());
                assert!(demo.state.sql_export.is_some());
                assert_eq!(std::fs::read_to_string(&path).unwrap(), "previous");
                assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
                assert!(demo.state.dirty && !demo.state.conn.as_ref().unwrap().is_autocommit());
                demo.state
                    .conn
                    .as_ref()
                    .unwrap()
                    .progress_handler(0, None::<fn() -> bool>);
                demo.perform_sql_export(path.clone());
                demo.do_rebuild();
                demo.settle_export_test();
                demo.do_rebuild();
                assert!(demo.state.sql_export.is_none() && demo.state.overlay.is_none());
                let restored = Connection::open_in_memory().unwrap();
                restored
                    .execute_batch(&std::fs::read_to_string(path).unwrap())
                    .unwrap();
                assert_eq!(db::count_rows(&restored, "t").unwrap(), 1000);
                demo.state
                    .conn
                    .as_ref()
                    .unwrap()
                    .execute_batch("ROLLBACK")
                    .unwrap();
                assert!(db::list_schema(demo.state.conn.as_ref().unwrap())
                    .unwrap()
                    .is_empty());
            }
        }
    }
    #[test]
    fn sql_export_dialog_retains_selection_options_keyboard_focus_and_cancel() {
        use crate::{csv_files::interaction_tests::capture, ui_build, Demo, PresentBackend};
        use next_gui_ui::{Theme, UiKey};
        for (theme_name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400, 1000), (800, 600)] {
                let mut demo = Demo::new(width, height, PresentBackend::Softbuffer);
                demo.theme = ui_build::browser_theme(theme, demo.state.prefs.font_size);
                demo.state.new_memory_db().unwrap();
                demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE first(v);INSERT INTO first VALUES('日本');CREATE TABLE second(v);CREATE VIEW viewed AS SELECT * FROM first").unwrap();
                demo.state.refresh_schema().unwrap();
                demo.state.selected = None;
                demo.action_export_sql();
                demo.do_rebuild();
                assert_eq!(demo.ui.active_dialog(), Some(demo.ids.sql_export.dialog));
                assert_ne!(demo.ids.tree.0, 0);
                demo.ui
                    .set_table_scroll_offset(demo.ids.sql_export.table, 1000.0, 0.0);
                assert_eq!(
                    demo.ui
                        .table_scroll_offset(demo.ids.sql_export.table)
                        .unwrap()
                        .0,
                    0.0
                );
                assert_eq!(demo.ui.focused(), Some(demo.ids.sql_export.cancel));
                assert_eq!(demo.state.sql_export.as_ref().unwrap().objects.len(), 3);
                demo.handle_button(demo.ids.sql_export.none);
                demo.do_rebuild();
                assert_eq!(
                    demo.ui.button_enabled(demo.ids.sql_export.export),
                    Some(false)
                );
                demo.ui.set_focus(Some(demo.ids.sql_export.table));
                demo.ui
                    .set_table_selected_row(demo.ids.sql_export.table, Some(1));
                assert!(demo.ui.key_input(UiKey::Space));
                demo.handle_field_check_change();
                demo.do_rebuild();
                assert!(demo.state.sql_export.as_ref().unwrap().objects[1].2);
                assert_eq!(demo.ui.focused(), Some(demo.ids.sql_export.table));
                demo.handle_button(demo.ids.sql_export.all);
                demo.do_rebuild();
                for id in [
                    demo.ids.sql_export.names,
                    demo.ids.sql_export.multiple,
                    demo.ids.sql_export.original,
                ] {
                    demo.ui.set_focus(Some(id));
                    assert!(demo.ui.key_input(UiKey::Space));
                    demo.snapshot_inputs();
                }
                demo.ui
                    .set_select_selected(demo.ids.sql_export.existing, Some(1));
                demo.snapshot_inputs();
                let options = &demo.state.sql_export.as_ref().unwrap().options;
                assert!(
                    !options.insert_column_names
                        && options.multiple_rows
                        && options.keep_original_create
                        && options.drop_if_exists
                );
                demo.win_w = if width == 800 { 1400 } else { 800 };
                demo.win_h = if height == 600 { 1000 } else { 600 };
                demo.do_rebuild();
                assert_eq!(demo.ui.focused(), Some(demo.ids.sql_export.original));
                demo.win_w = width;
                demo.win_h = height;
                demo.do_rebuild();
                capture(&mut demo, theme_name, width, "sql-export-options");
                demo.ui
                    .set_select_selected(demo.ids.sql_export.what, Some(2));
                demo.snapshot_inputs();
                demo.do_rebuild();
                assert_eq!(demo.ids.sql_export.existing.0, 0);
                assert!(!demo.state.sql_export.as_ref().unwrap().options.structure);
                capture(&mut demo, theme_name, width, "sql-export-data");
                demo.ui
                    .set_select_selected(demo.ids.sql_export.what, Some(0));
                demo.snapshot_inputs();
                demo.do_rebuild();
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("out.sql");
                demo.perform_sql_export(path.clone());
                demo.settle_export_test();
                demo.do_rebuild();
                assert!(demo.state.sql_export.is_none() && demo.state.overlay.is_none());
                assert!(demo.state.error_msg.is_none());
                let target = Connection::open_in_memory().unwrap();
                target
                    .execute_batch(&std::fs::read_to_string(path).unwrap())
                    .unwrap();
                assert_eq!(db::count_rows(&target, "first").unwrap(), 1);
                assert!(demo.state.prefs.sql_export.multiple_rows);
                demo.action_export_sql();
                demo.do_rebuild();
                demo.ui.key_input(UiKey::Escape);
                demo.handle_sql_export_dismiss();
                demo.do_rebuild();
                assert!(demo.state.sql_export.is_none() && demo.state.overlay.is_none());
                demo.state.tab = crate::state::MainTab::Browse;
                demo.state.browse_mut().table = Some("first".into());
                demo.action_export_sql();
                demo.do_rebuild();
                assert_eq!(
                    demo.state
                        .sql_export
                        .as_ref()
                        .unwrap()
                        .options()
                        .unwrap()
                        .selected,
                    Some(vec!["first".into()])
                );
                demo.handle_button(demo.ids.sql_export.cancel);
                demo.do_rebuild();
                assert!(demo.state.sql_export.is_none());
            }
        }
    }
    #[test]
    fn selected_dump_restores_batches_generated_values_and_only_selected_sequences() {
        let source = Connection::open_in_memory().unwrap();
        source.execute_batch("CREATE TABLE t(id INTEGER PRIMARY KEY AUTOINCREMENT,v TEXT,n INT GENERATED ALWAYS AS (length(v)) STORED);CREATE TABLE untouched(id INTEGER PRIMARY KEY AUTOINCREMENT);CREATE INDEX tv ON t(v);CREATE INDEX other ON untouched(id);CREATE VIEW chosen AS SELECT * FROM t;CREATE VIEW other_view AS SELECT * FROM untouched;CREATE TRIGGER t_trigger AFTER UPDATE ON t BEGIN SELECT 1;END;INSERT INTO t(id,v) VALUES(1000,'removed');DELETE FROM t;INSERT INTO untouched(id) VALUES(777)").unwrap();
        for i in 0..1001 {
            source
                .execute("INSERT INTO t(v) VALUES(?1)", [format!("quote'中文 {i}")])
                .unwrap();
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("selected.sql");
        for names in [true, false] {
            let options = ExportSqlOptions {
                selected: Some(vec!["t".into(), "chosen".into()]),
                insert_column_names: names,
                multiple_rows: true,
                ..Default::default()
            };
            db::export_sql_dump_opts(&source, &path, &options).unwrap();
            let sql = std::fs::read_to_string(&path).unwrap();
            assert_eq!(sql.matches("INSERT INTO \"t\"").count(), 3);
            assert!(sql.contains("IF NOT EXISTS"));
            assert!(
                !sql.contains("other_view")
                    && !sql.contains("untouched")
                    && !sql.contains("CREATE INDEX other")
            );
            let target = Connection::open_in_memory().unwrap();
            target.execute_batch("CREATE TABLE untouched(id INTEGER PRIMARY KEY AUTOINCREMENT);INSERT INTO untouched(id) VALUES(9999)").unwrap();
            target.execute_batch(&sql).unwrap();
            assert_eq!(db::count_rows(&target, "t").unwrap(), 1001);
            assert_eq!(db::count_rows(&target, "chosen").unwrap(), 1001);
            assert_eq!(
                target
                    .query_row(
                        "SELECT seq FROM sqlite_sequence WHERE name='untouched'",
                        [],
                        |r| r.get::<_, i64>(0)
                    )
                    .unwrap(),
                9999
            );
            assert_eq!(
                target
                    .query_row("SELECT seq FROM sqlite_sequence WHERE name='t'", [], |r| {
                        r.get::<_, i64>(0)
                    })
                    .unwrap(),
                2001
            );
            assert!(target
                .query_row("SELECT sum(n)=sum(length(v)) FROM t", [], |r| r
                    .get::<_, bool>(0))
                .unwrap());
        }
    }
    #[test]
    fn dump_modes_preserve_original_create_and_merge_or_replace_selected_schema() {
        let source = Connection::open_in_memory().unwrap();
        source.execute_batch("CREATE TABLE t( v TEXT /* retained source comment */);INSERT INTO t VALUES('new');CREATE INDEX idx ON t(v);CREATE VIEW chosen AS SELECT v FROM t").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.sql");
        for original in [true, false] {
            for drop in [true, false] {
                let options = ExportSqlOptions {
                    selected: Some(vec!["t".into(), "chosen".into()]),
                    keep_original_create: original,
                    drop_if_exists: drop,
                    ..Default::default()
                };
                db::export_sql_dump_opts(&source, &path, &options).unwrap();
                let sql = std::fs::read_to_string(&path).unwrap();
                assert_eq!(sql.contains("retained source comment"), original);
                let target = Connection::open_in_memory().unwrap();
                target.execute_batch("CREATE TABLE t(v);INSERT INTO t(rowid,v) VALUES(9000,'old');CREATE INDEX idx ON t(v);CREATE VIEW chosen AS SELECT v FROM t;CREATE TABLE safe(v);INSERT INTO safe VALUES(1)").unwrap();
                target.execute_batch(&sql).unwrap();
                assert_eq!(
                    db::count_rows(&target, "t").unwrap(),
                    if drop { 1 } else { 2 }
                );
                assert_eq!(db::count_rows(&target, "safe").unwrap(), 1);
            }
        }
        let options = ExportSqlOptions {
            structure: false,
            insert_column_names: false,
            selected: Some(vec!["t".into()]),
            ..Default::default()
        };
        db::export_sql_dump_opts(&source, &path, &options).unwrap();
        let sql = std::fs::read_to_string(&path).unwrap();
        assert!(!sql.contains("CREATE "));
        let target = Connection::open_in_memory().unwrap();
        target.execute_batch("CREATE TABLE t(v)").unwrap();
        target.execute_batch(&sql).unwrap();
        assert_eq!(db::count_rows(&target, "t").unwrap(), 1);
        let options = ExportSqlOptions {
            data: false,
            ..Default::default()
        };
        db::export_sql_dump_opts(&source, &path, &options).unwrap();
        let sql = std::fs::read_to_string(&path).unwrap();
        assert!(!sql.contains("INSERT INTO"));
    }
    #[test]
    fn invalid_selection_and_late_query_errors_preserve_destinations_and_pending_edits() {
        let source = Connection::open_in_memory().unwrap();
        source
            .execute_batch("CREATE TABLE safe(v);BEGIN;INSERT INTO safe VALUES('pending')")
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kept.sql");
        std::fs::write(&path, "previous").unwrap();
        for selected in [
            vec![],
            vec!["missing".into()],
            vec!["safe;DROP TABLE safe".into()],
        ] {
            let options = ExportSqlOptions {
                selected: Some(selected),
                ..Default::default()
            };
            assert!(db::export_sql_dump_opts(&source, &path, &options).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "previous");
            assert!(!source.is_autocommit());
        }
        source.authorizer(Some(|ctx: rusqlite::hooks::AuthContext<'_>| {
            if matches!(
                ctx.action,
                rusqlite::hooks::AuthAction::Read {
                    table_name: "safe",
                    ..
                }
            ) {
                rusqlite::hooks::Authorization::Deny
            } else {
                rusqlite::hooks::Authorization::Allow
            }
        }));
        assert!(db::export_sql_dump_opts(&source, &path, &ExportSqlOptions::default()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "previous");
        assert!(!source.is_autocommit());
        source.authorizer(
            None::<fn(rusqlite::hooks::AuthContext<'_>) -> rusqlite::hooks::Authorization>,
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        source.execute_batch("ROLLBACK").unwrap();
        assert_eq!(db::count_rows(&source, "safe").unwrap(), 0);
        let draft = Draft::new(&source, &ExportSqlOptions::default(), Some("safe")).unwrap();
        assert_eq!(draft.options().unwrap().selected, Some(vec!["safe".into()]));
        let json = serde_json::to_string(&draft.options().unwrap()).unwrap();
        assert!(!json.contains("selected"));
        assert_eq!(draft.filename(":memory:"), "safe.sql");
        let mut draft = draft;
        draft
            .objects
            .push(("other".into(), ObjectKind::Table, true));
        assert_eq!(draft.filename(":memory:"), "database.sql");
        assert_eq!(draft.filename("/tmp/archive.db"), "archive.sql");
        draft.objects[0].2 = false;
        draft.objects[1].0 = "../CON".into();
        assert_eq!(draft.filename(":memory:"), "%2E%2E%2FCON.sql");
    }
}
