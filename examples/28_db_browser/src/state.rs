//! Application state for the DB Browser example (DB4S chrome + Phase 2 wizards).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rusqlite::{Connection, InterruptHandle};
use crate::sql_log::{LoggedConnection, Source, SqlLog};

use crate::db::{
    self, ColDef, CsvImportOptions, ExportSqlOptions, ObjectKind, QueryResult, SchemaObject,
    COLUMN_TYPES,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainTab {
    Structure = 0,
    Browse = 1,
    Pragmas = 2,
    ExecuteSql = 3,
}

impl MainTab {
    pub fn from_index(i: usize) -> Self {
        match i {
            1 => MainTab::Browse,
            2 => MainTab::Pragmas,
            3 => MainTab::ExecuteSql,
            _ => MainTab::Structure,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn title(self) -> &'static str {
        match self {
            MainTab::Structure => "Database Structure",
            MainTab::Browse => "Browse Data",
            MainTab::Pragmas => "Edit Pragmas",
            MainTab::ExecuteSql => "Execute SQL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockKind {
    EditCell = 0,
    Schema = 1,
    Log = 2,
    Plot = 3,
    Remote = 4,
}

impl DockKind {
    #[allow(dead_code)]
    pub fn from_index(i: usize) -> Self {
        match i {
            1 => DockKind::Schema,
            2 => DockKind::Log,
            3 => DockKind::Plot,
            4 => DockKind::Remote,
            _ => DockKind::EditCell,
        }
    }

    #[allow(dead_code)]
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn title(self) -> &'static str {
        match self {
            DockKind::EditCell => "Edit Cell",
            DockKind::Schema => "DB Schema",
            DockKind::Log => "SQL Log",
            DockKind::Plot => "Plot",
            DockKind::Remote => "Remote",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowseViewMode {
    Table = 0,
    Form = 1,
}

/// Full-window overlay wizards (Modal cannot host custom controls).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    EditTable,
    InsertRow,
    EditIndex,
    ImportCsv,
    FindReplace,
    Attach,
    PasteCsv,
    Cipher,
    Preferences,
    ExportSql,
    CondFormat,
    LoadExtension,
    About,
}

#[derive(Debug, Clone)]
pub struct SelectedObject {
    pub kind: ObjectKind,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BrowseTabState {
    pub title: String,
    pub table: Option<String>,
    pub filter: String,
    #[serde(default)]
    pub global_filters: Vec<String>,
    /// Parallel to current result columns; empty = no filter.
    pub col_filters: Vec<String>,
    #[serde(default)]
    pub offset: usize,
    pub order_col: Option<String>,
    pub order_desc: bool,
    #[serde(default)]
    pub column_widths: Vec<f32>,
    #[serde(default)]
    pub scroll: (f32, f32),
    #[serde(skip)]
    pub result: Option<QueryResult>,
    #[serde(skip)]
    pub rowids: Vec<i64>,
    #[serde(skip)]
    pub sel_row: Option<usize>,
    #[serde(skip)]
    pub sel_row_start: Option<usize>,
    #[serde(skip)]
    pub sel_row_end: Option<usize>,
    #[serde(skip)]
    pub sel_col: Option<usize>,
    #[serde(skip)]
    pub sel_col_start: Option<usize>,
    #[serde(skip)]
    pub sel_col_end: Option<usize>,
    #[serde(skip)]
    pub total_rows: Option<usize>,
}

impl BrowseTabState {
    pub fn new(table: Option<String>) -> Self {
        let title = table.clone().unwrap_or_else(|| "Browse".into());
        Self {
            title,
            table,
            filter: String::new(),
            global_filters: Vec::new(),
            col_filters: Vec::new(),
            offset: 0,
            order_col: None,
            order_desc: false,
            column_widths: Vec::new(),
            scroll: (0.0, 0.0),
            result: None,
            rowids: Vec::new(),
            sel_row: None,
            sel_row_start: None,
            sel_row_end: None,
            sel_col: None,
            sel_col_start: None,
            sel_col_end: None,
            total_rows: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditCellMode {
    Text = 0,
    Hex = 1,
    Json = 2,
    Image = 3,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CondFormatRule {
    pub table: Option<String>,
    pub column: String,
    /// 0== 1!= 2contains 3> 4<
    pub op: usize,
    pub value: String,
    /// RGB packed 0xRRGGBB for cell tint
    pub color_rgb: u32,
}

pub struct AppState {
    pub conn: Option<LoggedConnection>,
    pub path: String,
    pub path_draft: String,
    pub read_only: bool,
    pub dirty: bool,
    pub schema: Vec<SchemaObject>,
    pub database_encoding: String,
    pub selected: Option<SelectedObject>,
    pub tab: MainTab,
    pub status_msg: String,
    pub error_msg: Option<String>,

    pub body_ratio: f32,
    pub structure_ratio: f32,
    pub shell_scroll: (f32, f32),
    pub page_scroll: [(f32,f32);4],
    pub dock_scroll: [(f32,f32);5],
    pub dock_edit: bool,
    pub dock_schema: bool,
    pub dock_log: bool,
    pub dock_plot: bool,
    pub dock_focus: DockKind,

    // Browse (multi-tab)
    pub browse_tabs: Vec<BrowseTabState>,
    pub browse_tab_idx: usize,
    pub edit_col: String,
    pub edit_value: String,
    pub insert_columns: Vec<db::ColumnInfo>,
    pub insert_values: Vec<String>,
    pub insert_modes: Vec<usize>,

    // Structure helpers / wizard drafts
    pub ddl_draft: String,
    pub overlay: Option<OverlayKind>,
    pub wizard_table_name: String,
    pub wizard_table_original: Option<String>,
    pub wizard_cols: Vec<ColDef>,
    pub wizard_col_sel: usize,
    pub wizard_index_name: String,
    pub wizard_index_table: String,
    pub wizard_index_cols: String,
    pub wizard_index_unique: bool,
    pub wizard_index_original: Option<String>,

    // CSV import wizard
    pub csv_path: String,
    pub csv_table: String,
    pub csv_has_header: bool,
    pub csv_delim_idx: usize, // 0=, 1=; 2=tab 3=|
    pub csv_quote_idx: usize, // 0=" 1=' 2=none
    pub csv_paste: String,

    // Attach
    pub attach_path: String,
    pub attach_schema: String,

    // Find/Replace
    pub find_text: String,
    pub replace_text: String,
    pub find_search: crate::sql_search::Search,
    pub find_message: String,

    // Execute SQL
    pub sql_tabs: Vec<String>,
    pub sql_files: Vec<Option<SqlFile>>,
    sql_saved: Vec<String>,
    pub sql_edit_states: Vec<Option<next_gui_ui::TextAreaState>>,
    pub sql_tab_idx: usize,
    pub sql_result: Option<QueryResult>,
    pub sql_outputs: Vec<SqlOutput>,
    pub sql_column_widths: Vec<f32>,
    pub sql_scroll: (f32, f32),
    pub sql_message: String,
    pub sql_view_name: String,
    pub sql_busy: bool,
    pub sql_cancel: Arc<AtomicBool>,
    pub sql_interrupt: Option<InterruptHandle>,
    pub sql_job: Option<SqlJob>,

    // Pragmas (key, value) + drafts mirrored for form widgets
    pub pragmas: Vec<(String, String)>,
    pub pragma_drafts: Vec<String>,

    // Plot
    pub chart_table: Option<String>,
    pub chart_column: String,
    pub chart_points: Vec<f32>,

    pub sql_log: SqlLog,
    pub sql_log_filter: usize,
    pub sql_log_view: Option<next_gui_ui::TextAreaState>,
    sql_sensitive: bool,
    pub io_path: String,
    pub recent: VecDeque<String>,

    pub confirm_kind: ConfirmKind,
    pub confirm_open: bool,
    pub pending_document: Option<DocumentAction>,

    // Phase 3
    pub project_path: Option<String>,
    pub project_xml: Option<String>,
    saved_project: serde_json::Value,
    pub edit_saved: rusqlite::types::Value,
    pub prefs: crate::prefs::Prefs,
    pub frozen_cols: usize,
    pub cond_rules: Vec<CondFormatRule>,
    pub edit_mode: EditCellMode,
    pub edit_original: rusqlite::types::Value,
    pub edit_type: usize,
    pub edit_binary_len: usize,
    pub edit_image_kind: String,
    pub toolbar_db: bool,
    pub toolbar_extra: bool,
    pub toolbar_project: bool,
    pub toolbar_sql: bool,
    pub cipher_passphrase: String,
    pub cipher_page_size: String,
    pub cipher_kdf_iter: String,
    pub cipher_active: bool,
    pub cipher_open_path: Option<(String, bool)>,
    pub export_structure: bool,
    pub export_data: bool,
    pub export_drop: bool,
    pub export_keep_fk: bool,
    pub prefs_font_size: String,
    pub prefs_frozen_cols: usize,
    pub prefs_theme_dark: bool,
    pub prefs_confirm_drops: bool,
    pub prefs_csv_delim: String,
    pub cond_col: String,
    pub cond_op_idx: usize,
    pub cond_value: String,
    pub cond_color: String,
    pub dock_remote: bool,
    pub browse_view: BrowseViewMode,
    #[allow(dead_code)]
    pub form_field_idx: usize,
    pub ext_path: String,
    pub ext_entry: String,
    pub remote_paths: Vec<String>,
    pub remote_sel: usize,
    pub remote_draft: String,
    pub edit_image_view: next_gui_ui::ImageView,
    pub edit_image_rgba: Option<(u32, u32, Vec<u8>)>,
    pub cipher_linked: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SqlFile {
    pub path: PathBuf,
    pub saved: String,
}

#[derive(Default)]
pub struct SqlOutput {
    result: Option<QueryResult>,
    widths: Vec<f32>,
    scroll: (f32, f32),
    message: String,
}

pub struct SqlJob {
    handle: std::thread::JoinHandle<(LoggedConnection, Result<QueryResult, String>, bool)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmKind {
    None,
    DropObject,
    DeleteRow,
    Documents,
    Revert,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentAction {
    NewDatabase(PathBuf),
    NewMemory,
    OpenDatabase(PathBuf, bool),
    CloseDatabase,
    OpenProject(PathBuf),
    CloseProject,
    CloseSqlTab(usize),
    Exit,
}

impl AppState {
    pub fn new_with_demo() -> Self {
        let demo = db::default_demo_path();
        let mut state = Self::empty();
        state.load_recent();
        state.sync_prefs_drafts();
        match open_or_create_demo(&demo) {
            Ok(mut conn) => {
                conn.bind_log(&state.sql_log);
                state.path = demo.display().to_string();
                state.path_draft = state.path.clone();
                state.conn = Some(conn);
                let _ = state.begin_txn();
                state.push_recent(&state.path.clone());
                state.status_msg = format!("Opened demo DB {}", state.path);
                let _ = state.refresh_schema();
                if let Some(t) = state
                    .schema
                    .iter()
                    .find(|o| o.kind == ObjectKind::Table)
                    .map(|o| o.name.clone())
                {
                    state.browse_tabs = vec![BrowseTabState::new(Some(t))];
                    let _ = state.refresh_browse();
                }
                state.pick_chart_source();
                let _ = state.refresh_chart();
                let _ = state.refresh_pragmas();
                state.probe_cipher_linked();
            }
            Err(e) => {
                state.status_msg = format!("Demo DB failed: {e}; using in-memory");
                if let Ok(mut conn) = open_shared_memory() {
                    conn.bind_log(&state.sql_log);
                    let _ = db::seed_demo(&conn);
                    state.conn = Some(conn);
                    state.path = ":memory:".into();
                    state.path_draft = state.path.clone();
                    let _ = state.begin_txn();
                    let _ = state.refresh_schema();
                    state.pick_chart_source();
                    let _ = state.refresh_chart();
                    let _ = state.refresh_pragmas();
                    state.probe_cipher_linked();
                }
            }
        }
        state.frozen_cols = state.prefs.frozen_cols_default;
        state.log_sql("-- session start");
        state.checkpoint_project();
        state
    }

    fn empty() -> Self {
        let mut state = Self {
            conn: None,
            path: String::new(),
            path_draft: String::new(),
            read_only: false,
            dirty: false,
            schema: Vec::new(),
            database_encoding: String::new(),
            selected: None,
            tab: MainTab::Structure,
            status_msg: "Ready".into(),
            error_msg: None,
            body_ratio: 0.72,
            structure_ratio: 0.34,
            shell_scroll: (0.0,0.0),
            page_scroll: [(0.0,0.0);4],
            dock_scroll: [(0.0,0.0);5],
            dock_edit: true,
            dock_schema: true,
            dock_log: true,
            dock_plot: true,
            dock_focus: DockKind::EditCell,
            browse_tabs: vec![BrowseTabState::new(None)],
            browse_tab_idx: 0,
            edit_col: String::new(),
            edit_value: String::new(),
            insert_columns: Vec::new(),
            insert_values: Vec::new(),
            insert_modes: Vec::new(),
            ddl_draft: String::new(),
            overlay: None,
            wizard_table_name: "new_table".into(),
            wizard_table_original: None,
            wizard_cols: vec![
                ColDef {
                    name: "id".into(),
                    type_name: "INTEGER".into(),
                    pk: true,
                    notnull: false,
                    unique: false,
                    default_value: String::new(),
                },
                ColDef::new("name", "TEXT"),
            ],
            wizard_col_sel: 0,
            wizard_index_name: "idx_name".into(),
            wizard_index_table: String::new(),
            wizard_index_cols: "name".into(),
            wizard_index_unique: false,
            wizard_index_original: None,
            csv_path: String::new(),
            csv_table: "imported".into(),
            csv_has_header: true,
            csv_delim_idx: 0,
            csv_quote_idx: 0,
            csv_paste: String::new(),
            attach_path: String::new(),
            attach_schema: "other".into(),
            find_text: String::new(),
            replace_text: String::new(),
            find_search: crate::sql_search::Search::default(),
            find_message: String::new(),
            sql_edit_states: Vec::new(),
            sql_files: Vec::new(),
            sql_saved: Vec::new(),
            sql_tabs: vec!["SELECT name, type FROM sqlite_master ORDER BY type, name;\n".into()],
            sql_tab_idx: 0,
            sql_result: None,
            sql_outputs: Vec::new(),
            sql_column_widths: Vec::new(),
            sql_scroll: (0.0, 0.0),
            sql_view_name: "e28_saved_view".into(),
            sql_message: "Ready — Execute SQL to see results.".into(),
            sql_busy: false,
            sql_cancel: Arc::new(AtomicBool::new(false)),
            sql_interrupt: None,
            sql_job: None,
            pragmas: Vec::new(),
            pragma_drafts: Vec::new(),
            chart_table: None,
            chart_column: "price".into(),
            chart_points: Vec::new(),
            sql_log: SqlLog::default(),
            sql_log_filter: 0,
            sql_log_view: None,
            sql_sensitive: false,
            io_path: String::new(),
            recent: VecDeque::new(),
            confirm_kind: ConfirmKind::None,
            confirm_open: false,
            pending_document: None,
            project_path: None,
            project_xml: None,
            saved_project: serde_json::Value::Null,
            edit_saved: rusqlite::types::Value::Null,
            prefs: crate::prefs::Prefs::load(),
            frozen_cols: 0,
            cond_rules: Vec::new(),
            edit_mode: EditCellMode::Text,
            edit_original: rusqlite::types::Value::Null,
            edit_type: 0,
            edit_binary_len: 0,
            edit_image_kind: String::new(),
            toolbar_db: true,
            toolbar_extra: true,
            toolbar_project: true,
            toolbar_sql: true,
            cipher_passphrase: String::new(),
            cipher_page_size: "4096".into(),
            cipher_kdf_iter: "256000".into(),
            cipher_active: false,
            cipher_open_path: None,
            export_structure: true,
            export_data: true,
            export_drop: false,
            export_keep_fk: true,
            prefs_frozen_cols: 0,
            prefs_font_size: "13".into(),
            prefs_theme_dark: true,
            prefs_confirm_drops: true,
            prefs_csv_delim: ",".into(),
            cond_col: String::new(),
            cond_op_idx: 0,
            cond_value: String::new(),
            cond_color: "E67E22".into(),
            dock_remote: true,
            browse_view: BrowseViewMode::Table,
            form_field_idx: 0,
            ext_path: String::new(),
            ext_entry: String::new(),
            remote_paths: Vec::new(),
            remote_sel: 0,
            remote_draft: String::new(),
            edit_image_view: next_gui_ui::ImageView::default(),
            edit_image_rgba: None,
            cipher_linked: false,
        };
        state.checkpoint_project();
        state
    }

    pub fn browse(&self) -> &BrowseTabState {
        self.browse_tabs
            .get(self.browse_tab_idx)
            .or_else(|| self.browse_tabs.first())
            .expect("browse tabs")
    }

    pub fn browse_mut(&mut self) -> &mut BrowseTabState {
        if self.browse_tabs.is_empty() {
            self.browse_tabs.push(BrowseTabState::new(None));
            self.browse_tab_idx = 0;
        }
        if self.browse_tab_idx >= self.browse_tabs.len() {
            self.browse_tab_idx = self.browse_tabs.len() - 1;
        }
        &mut self.browse_tabs[self.browse_tab_idx]
    }

    pub fn sql_editor(&self) -> &str {
        self.sql_tabs
            .get(self.sql_tab_idx)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    pub fn sql_editor_mut(&mut self) -> &mut String {
        if self.sql_tabs.is_empty() {
            self.sql_tabs.push(String::new());
            self.sql_tab_idx = 0;
        }
        if self.sql_tab_idx >= self.sql_tabs.len() {
            self.sql_tab_idx = self.sql_tabs.len() - 1;
        }
        &mut self.sql_tabs[self.sql_tab_idx]
    }

    pub fn switch_sql_tab(&mut self, index: usize) {
        if index >= self.sql_tabs.len() || index == self.sql_tab_idx || self.sql_busy {
            return;
        }
        self.sql_outputs
            .resize_with(self.sql_tabs.len(), SqlOutput::default);
        self.sql_outputs[self.sql_tab_idx] = SqlOutput {
            result: self.sql_result.take(),
            widths: std::mem::take(&mut self.sql_column_widths),
            scroll: self.sql_scroll,
            message: std::mem::take(&mut self.sql_message),
        };
        let output = std::mem::take(&mut self.sql_outputs[index]);
        self.sql_result = output.result;
        self.sql_column_widths = output.widths;
        self.sql_scroll = output.scroll;
        self.sql_message = output.message;
        self.error_msg = None;
        self.sql_tab_idx = index;
        self.find_search.reset(&self.sql_tabs[index], 0..0);
        self.find_message.clear();
    }

    fn project_snapshot(&self) -> serde_json::Value {
        let mut value = serde_json::to_value(crate::project::ProjectFile::from_state(self))
            .expect("project serialization");
        // Active pages are navigation, not unsaved content.
        for key in ["main_tab", "sql_tab_idx", "browse_tab_idx"] {
            value.as_object_mut().unwrap().remove(key);
        }
        value
    }

    pub fn checkpoint_project(&mut self) {
        self.saved_project = self.project_snapshot();
        self.sql_saved = self.sql_tabs.clone();
    }

    pub fn mark_sql_saved(&mut self) {
        self.mark_sql_tab_saved(self.sql_tab_idx);
    }

    fn mark_sql_tab_saved(&mut self, index: usize) {
        let sql = self.sql_tabs[index].clone();
        self.sql_saved.resize(self.sql_tabs.len(), String::new());
        self.sql_saved[index] = sql.clone();
        if let Some(Some(file)) = self.sql_files.get_mut(index) { file.saved = sql.clone(); }
        if let Some(tabs) = self.saved_project.get_mut("sql_tabs").and_then(|v| v.as_array_mut()) {
            while tabs.len() < self.sql_tabs.len() { tabs.push(serde_json::Value::String(String::new())); }
            tabs[index] = serde_json::Value::String(sql);
        }
        self.saved_project["sql_files"] = serde_json::to_value(&self.sql_files).unwrap();
    }

    pub fn sql_tab_label(&self, index: usize) -> String {
        let title = self.sql_files.get(index).and_then(|f| f.as_ref()).and_then(|f| f.path.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| format!("SQL {}", index + 1));
        let modified = if self.sql_tab_dirty(index) { " ●" } else { "" };
        if index == self.sql_tab_idx { format!("[{title}]{modified}") } else { format!("{title}{modified}") }
    }

    pub fn sql_tab_dirty(&self, index: usize) -> bool {
        let Some(text) = self.sql_tabs.get(index) else { return false; };
        if let Some(Some(file)) = self.sql_files.get(index) { return text != &file.saved; }
        self.sql_saved.get(index).map(String::as_str).unwrap_or("") != text
    }

    pub fn has_project_drafts(&self) -> bool {
        let mut current = self.project_snapshot();
        for (index, file) in self.sql_files.iter().enumerate() {
            if file.is_some() {
                if let Some(saved) = self.saved_project.get("sql_tabs").and_then(|v| v.get(index)) {
                    current["sql_tabs"][index] = saved.clone();
                }
            }
        }
        current != self.saved_project
    }

    pub fn open_sql_file(&mut self, path: &Path) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        self.sql_tabs.push(text.clone());
        self.sql_files.resize_with(self.sql_tabs.len(), || None);
        self.sql_files[self.sql_tabs.len() - 1] = Some(SqlFile { path: path.to_path_buf(), saved: text });
        self.switch_sql_tab(self.sql_tabs.len() - 1);
        self.mark_sql_saved();
        self.tab = MainTab::ExecuteSql;
        self.status_msg = format!("Opened SQL {}", path.display());
        Ok(())
    }

    pub fn save_sql_file(&mut self, index: usize, path: &Path) -> Result<(), String> {
        use std::io::Write;
        let text = self.sql_tabs.get(index).ok_or("SQL tab no longer exists")?;
        if let Some(conn) = &self.conn { db::guard_output_path(conn, path)?; }
        if let Some(Some(file)) = self.sql_files.get(index) {
            if file.path == path && path.exists() {
                let disk = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
                if disk != file.saved { return Err(format!("{} changed outside this session; use Save As", path.display())); }
            }
        }
        let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        temp.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        temp.persist(path).map_err(|e| e.to_string())?;
        self.sql_files.resize_with(self.sql_tabs.len(), || None);
        self.sql_files[index] = Some(SqlFile { path: path.to_path_buf(), saved: text.clone() });
        self.mark_sql_tab_saved(index);
        self.status_msg = format!("Saved SQL {}", path.display());
        Ok(())
    }

    pub fn move_sql_tab(&mut self, from: usize, to: usize) {
        if self.sql_busy || from >= self.sql_tabs.len() || to >= self.sql_tabs.len() { return; }
        self.sql_files.resize_with(self.sql_tabs.len(), || None);
        self.sql_saved.resize(self.sql_tabs.len(), String::new());
        self.sql_edit_states.resize(self.sql_tabs.len(), None);
        self.sql_outputs.resize_with(self.sql_tabs.len(), SqlOutput::default);
        self.sql_tabs.swap(from, to);
        self.sql_files.swap(from, to);
        self.sql_saved.swap(from, to);
        self.sql_edit_states.swap(from, to);
        self.sql_outputs.swap(from, to);
        if self.sql_tab_idx == from { self.sql_tab_idx = to; }
        else if self.sql_tab_idx == to { self.sql_tab_idx = from; }
    }

    pub fn close_sql_tab(&mut self, index: usize) {
        if self.sql_busy || index >= self.sql_tabs.len() { return; }
        self.sql_files.resize_with(self.sql_tabs.len(), || None);
        self.sql_saved.resize(self.sql_tabs.len(), String::new());
        self.sql_edit_states.resize(self.sql_tabs.len(), None);
        self.sql_outputs.resize_with(self.sql_tabs.len(), SqlOutput::default);
        if index == self.sql_tab_idx && self.sql_tabs.len() > 1 {
            self.switch_sql_tab(if index == 0 { 1 } else { index - 1 });
        }
        self.sql_tabs.remove(index);
        self.sql_files.remove(index);
        self.sql_saved.remove(index);
        self.sql_edit_states.remove(index);
        self.sql_outputs.remove(index);
        if self.sql_tabs.is_empty() {
            self.sql_tabs.push(String::new());
            self.sql_saved.push(String::new());
            self.sql_result = None;
            self.sql_message.clear();
            self.sql_column_widths.clear();
            self.sql_scroll = (0.0, 0.0);
            self.sql_tab_idx = 0;
            self.find_search.reset("", 0..0);
            self.find_message.clear();
        } else if self.sql_tab_idx > index { self.sql_tab_idx -= 1; }
    }

    pub fn has_cell_draft(&self) -> bool {
        if self.edit_col.is_empty() {
            return false;
        }
        if self.edit_mode == EditCellMode::Image {
            return self.edit_original != self.edit_saved;
        }
        !matches!(self.edited_cell_value(), Ok(value) if value == self.edit_saved)
    }

    pub fn has_unsaved_drafts(&self) -> bool {
        self.has_project_drafts()
            || self.sql_tabs.iter().enumerate().any(|(i, _)| self.sql_tab_dirty(i))
            || self.has_cell_draft()
            || self.has_pragma_drafts()
    }

    pub fn any_dock_visible(&self) -> bool {
        self.dock_edit || self.dock_schema || self.dock_log || self.dock_plot || self.dock_remote
    }

    pub fn visible_docks(&self) -> Vec<DockKind> {
        let mut v = Vec::new();
        if self.dock_edit {
            v.push(DockKind::EditCell);
        }
        if self.dock_schema {
            v.push(DockKind::Schema);
        }
        if self.dock_log {
            v.push(DockKind::Log);
        }
        if self.dock_plot {
            v.push(DockKind::Plot);
        }
        if self.dock_remote {
            v.push(DockKind::Remote);
        }
        v
    }

    pub fn mark_dirty(&mut self) {
        if !self.read_only {
            self.dirty = true;
        }
    }

    pub fn begin_txn(&mut self) -> Result<(), String> {
        let Some(conn) = self.conn.as_ref() else {
            return Ok(());
        };
        if self.read_only {
            return Ok(());
        }
        db::begin_deferred(conn)
    }

    pub fn write_changes(&mut self) -> Result<(), String> {
        if self.read_only {
            return Err("Database is open read-only".into());
        }
        let Some(conn) = self.conn.as_ref() else {
            return Err("No database open".into());
        };
        db::commit_and_begin(conn)?;
        self.dirty = false;
        self.log_sql("-- WRITE CHANGES (COMMIT)");
        self.status_msg = "Changes written".into();
        Ok(())
    }

    pub fn revert_changes(&mut self) -> Result<(), String> {
        if self.read_only {
            return Err("Database is open read-only".into());
        }
        let Some(conn) = self.conn.as_ref() else {
            return Err("No database open".into());
        };
        db::rollback_and_begin(conn)?;
        self.edit_col.clear();
        self.dirty = false;
        self.log_sql("-- REVERT CHANGES (ROLLBACK)");
        self.refresh_schema()?;
        self.refresh_browse()?;
        self.refresh_pragmas()?;
        self.refresh_chart()?;
        self.status_msg = "Changes reverted".into();
        Ok(())
    }

    pub fn log_sql(&mut self, stmt: impl Into<String>) {
        self.sql_log.push(Source::Application, stmt);
    }

    pub fn export_sql_log(&self, path: &Path) -> Result<(), String> {
        if self.sql_busy { return Err("Wait for SQL to finish before exporting the log".into()); }
        if let Some(conn) = &self.conn { db::guard_output_path(conn, path)?; }
        let mut file = tempfile::NamedTempFile::new_in(path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new(".")))
            .map_err(|e| e.to_string())?;
        use std::io::Write;
        file.write_all(self.sql_log.text(Source::from_filter(self.sql_log_filter)).as_bytes()).map_err(|e| e.to_string())?;
        file.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn refresh_schema(&mut self) -> Result<(), String> {
        let Some(conn) = self.conn.as_ref() else {
            self.schema.clear();
            self.database_encoding.clear();
            return Ok(());
        };
        let schema = db::list_schema(conn)?;
        let encoding = db::read_pragma(conn, "main.encoding")?;
        self.schema = schema;
        self.database_encoding = encoding;
        Ok(())
    }

    pub fn refresh_pragmas(&mut self) -> Result<(), String> {
        let Some(conn) = self.conn.as_ref() else {
            self.pragmas.clear();
            self.pragma_drafts.clear();
            return Ok(());
        };
        self.pragmas = db::read_all_pragmas(conn);
        self.pragma_drafts = self.pragmas.iter().map(|(_, v)| v.clone()).collect();
        Ok(())
    }

    fn composed_where(tab: &BrowseTabState, columns: &[String]) -> Result<Option<String>, String> {
        let mut parts = Vec::new();
        if !tab.filter.trim().is_empty() {
            parts.push(format!("({})", tab.filter.trim()));
        }
        for (col, filter) in columns.iter().zip(&tab.col_filters) {
            crate::filters::validate(filter)?;
            if let Some(condition) = crate::filters::condition(filter) {
                parts.push(format!("{} {condition}", db::quote_ident(col)));
            }
        }
        for filter in &tab.global_filters {
            crate::filters::validate(filter)?;
            if let Some(condition) = crate::filters::condition(filter) {
                let any_column = columns.iter().map(|col| format!("{} {condition}", db::quote_ident(col)))
                    .collect::<Vec<_>>().join(" OR ");
                if !any_column.is_empty() { parts.push(format!("({any_column})")); }
            }
        }
        Ok(if parts.is_empty() {
            None
        } else {
            Some(parts.join(" AND "))
        })
    }

    pub fn export_browse(&self, path: &Path, json: bool) -> Result<(), String> {
        let conn = self.conn.as_ref().ok_or("No database open")?;
        let tab = self.browse();
        let table = tab.table.as_ref().ok_or("Select a table to export")?;
        let columns = db::table_column_names(conn, table)?;
        db::export_table(
            conn,
            table,
            Self::composed_where(tab, &columns)?.as_deref(),
            path,
            json,
        )
    }

    pub fn refresh_browse(&mut self) -> Result<(), String> {
        if self.has_cell_draft() { return Err("Apply or discard the cell draft first".into()); }
        if self.conn.is_none() {
            if let Some(t) = self.browse_tabs.get_mut(self.browse_tab_idx) {
                t.result = None;
                t.rowids.clear();
                t.total_rows = None;
            }
            return Ok(());
        }
        let (table, order, desc, mut col_filters, mut prev_cols) = {
            let t = self.browse_mut();
            (
                t.table.clone(),
                t.order_col.clone(),
                t.order_desc,
                t.col_filters.clone(),
                t.result
                    .as_ref()
                    .map(|r| r.columns.clone())
                    .unwrap_or_default(),
            )
        };
        let Some(table) = table else {
            let t = self.browse_mut();
            t.result = None;
            t.rowids.clear();
            t.total_rows = None;
            return Ok(());
        };
        if prev_cols.is_empty() {
            prev_cols = db::table_column_names(self.conn.as_ref().unwrap(), &table)?;
        }
        let is_view = self
            .schema
            .iter()
            .any(|o| o.name == table && o.kind == ObjectKind::View);

        let where_sql = Self::composed_where(self.browse(), &prev_cols)?;

        let offset = self.browse().offset;
        let (total_rows, result, rowids) = {
            let conn = self.conn.as_ref().unwrap();
            let total_rows = db::count_browse_rows(conn, &table, where_sql.as_deref())?;
            if is_view {
                (
                    total_rows,
                    db::browse_table(
                        conn,
                        &table,
                        order.as_deref(),
                        desc,
                        where_sql.as_deref(),
                        db::DEFAULT_LIMIT,
                        offset,
                    )?,
                    Vec::new(),
                )
            } else {
                match db::browse_with_rowid(
                    conn,
                    &table,
                    order.as_deref(),
                    desc,
                    where_sql.as_deref(),
                    db::DEFAULT_LIMIT,
                    offset,
                ) {
                    Ok(mut q) => {
                        let mut rowids = Vec::new();
                        if let Some(idx) = q.columns.iter().position(|c| c == "__rowid__") {
                            for row in &q.rows {
                                let id = row.get(idx).and_then(|s| s.parse().ok()).unwrap_or(-1);
                                rowids.push(id);
                            }
                            q.columns.remove(idx);
                            for row in &mut q.rows {
                                if idx < row.len() {
                                    row.remove(idx);
                                }
                            }
                        }
                        (total_rows, q, rowids)
                    }
                    Err(_) => (
                        total_rows,
                        db::browse_table(
                            conn,
                            &table,
                            order.as_deref(),
                            desc,
                            where_sql.as_deref(),
                            db::DEFAULT_LIMIT,
                            offset,
                        )?,
                        Vec::new(),
                    ),
                }
            }
        };

        let n = result.columns.len();
        col_filters.resize(n, String::new());
        let nrows = result.rows.len();
        let truncated = result.truncated;
        let order_note = {
            let t = self.browse();
            match &t.order_col {
                Some(c) => format!(
                    " ORDER BY {} {}",
                    c,
                    if t.order_desc { "DESC" } else { "ASC" }
                ),
                None => String::new(),
            }
        };
        {
            let t = self.browse_mut();
            t.col_filters = col_filters;
            t.title = table.clone();
            t.rowids = rowids;
            t.total_rows = Some(total_rows);
            t.result = Some(result);
            t.sel_row = None;
            t.sel_col = None;
        }
        self.edit_col.clear();
        self.edit_value.clear();
        self.edit_type = 0;
        self.edit_original = rusqlite::types::Value::Null;
        self.edit_saved = rusqlite::types::Value::Null;
        self.edit_image_rgba = None;
        self.status_msg = format!(
            "Browse {} — {} rows {}–{} of {}{}{}",
            table,
            nrows,
            if nrows == 0 { 0 } else { offset.saturating_add(1) },
            offset.saturating_add(nrows),
            total_rows,
            if truncated { " (more pages)" } else { "" },
            order_note
        );
        Ok(())
    }

    pub fn browse_page(&mut self, next: bool) -> Result<(), String> {
        let tab = self.browse_mut();
        if next
            && tab
                .total_rows
                .is_some_and(|total| tab.offset.saturating_add(db::DEFAULT_LIMIT) >= total)
        {
            return Ok(());
        }
        let old = tab.offset;
        tab.offset = if next {
            old.saturating_add(db::DEFAULT_LIMIT)
        } else {
            old.saturating_sub(db::DEFAULT_LIMIT)
        };
        if let Err(e) = self.refresh_browse() {
            self.browse_mut().offset = old;
            return Err(e);
        }
        Ok(())
    }

    pub fn refresh_chart(&mut self) -> Result<(), String> {
        let Some(conn) = self.conn.as_ref() else {
            self.chart_points.clear();
            return Ok(());
        };
        let Some(table) = self.chart_table.clone() else {
            self.chart_points.clear();
            return Ok(());
        };
        if self.chart_column.trim().is_empty() {
            self.chart_points.clear();
            return Ok(());
        }
        self.chart_points.clear();
        self.chart_points = db::numeric_series(conn, &table, self.chart_column.trim(), 64)?;
        Ok(())
    }

    pub fn select_schema_node(&mut self, node_id: u64) {
        if node_id < 10_000 {
            return;
        }
        let idx = (node_id - 10_000) as usize;
        if let Some(obj) = self.schema.get(idx).cloned() {
            self.selected = Some(SelectedObject {
                kind: obj.kind,
                name: obj.name.clone(),
            });
            self.ddl_draft = obj.sql.clone().unwrap_or_default();
            if matches!(obj.kind, ObjectKind::Table | ObjectKind::View) {
                let t = self.browse_mut();
                t.table = Some(obj.name.clone());
                t.title = obj.name.clone();
                let _ = self.refresh_browse();
            }
        }
    }

    pub fn open_insert_row(&mut self) -> Result<(), String> {
        self.guard_writable()?;
        let table = self.browse().table.as_ref().ok_or("Select a table")?;
        self.insert_columns = db::table_columns(self.conn.as_ref().unwrap(), table)?;
        self.insert_values = vec![String::new(); self.insert_columns.len()];
        self.insert_modes = vec![0; self.insert_columns.len()];
        self.overlay = Some(OverlayKind::InsertRow);
        Ok(())
    }

    pub fn apply_insert_row(&mut self) -> Result<(), String> {
        self.guard_writable()?;
        let table = self.browse().table.as_ref().ok_or("Select a table")?;
        let mut columns = Vec::new();
        let mut values = Vec::new();
        for (i, column) in self.insert_columns.iter().enumerate() {
            match self.insert_modes[i] {
                1 => {
                    columns.push(db::quote_ident(&column.name));
                    values.push(rusqlite::types::Value::Null);
                }
                2 => {
                    columns.push(db::quote_ident(&column.name));
                    values.push(rusqlite::types::Value::Text(self.insert_values[i].clone()));
                }
                _ => {}
            }
        }
        let sql = if columns.is_empty() {
            format!("INSERT INTO {} DEFAULT VALUES", db::quote_ident(table))
        } else {
            format!(
                "INSERT INTO {} ({}) VALUES ({})",
                db::quote_ident(table),
                columns.join(","),
                vec!["?"; values.len()].join(",")
            )
        };
        self.conn
            .as_ref()
            .unwrap()
            .execute(&sql, rusqlite::params_from_iter(values))
            .map_err(|e| e.to_string())?;
        self.mark_dirty();
        self.refresh_browse()?;
        self.status_msg = "Record inserted (unwritten)".into();
        self.overlay = None;
        Ok(())
    }

    pub fn open_create_table_wizard(&mut self) {
        self.wizard_table_name = "new_table".into();
        self.wizard_table_original = None;
        self.wizard_cols = vec![
            ColDef {
                name: "id".into(),
                type_name: "INTEGER".into(),
                pk: true,
                notnull: false,
                unique: false,
                default_value: String::new(),
            },
            ColDef::new("name", "TEXT"),
        ];
        self.wizard_col_sel = 0;
        self.overlay = Some(OverlayKind::EditTable);
    }

    pub fn open_modify_table_wizard(&mut self) -> Result<(), String> {
        let sel = self
            .selected
            .clone()
            .ok_or_else(|| "Select a table to modify".to_string())?;
        if sel.kind != ObjectKind::Table {
            return Err("Modify Table only works on tables".into());
        }
        let conn = self.conn.as_ref().ok_or("No database open")?;
        let cols = db::table_columns(conn, &sel.name)?;
        self.wizard_table_name = sel.name.clone();
        self.wizard_table_original = Some(sel.name.clone());
        self.wizard_cols = cols
            .into_iter()
            .map(|c| ColDef {
                name: c.name,
                type_name: if c.type_name.is_empty() {
                    "TEXT".into()
                } else {
                    c.type_name
                },
                pk: c.pk,
                notnull: c.notnull,
                unique: false,
                default_value: c.default_value.unwrap_or_default(),
            })
            .collect();
        if self.wizard_cols.is_empty() {
            self.wizard_cols.push(ColDef::new("col1", "TEXT"));
        }
        self.wizard_col_sel = 0;
        self.overlay = Some(OverlayKind::EditTable);
        Ok(())
    }

    pub fn open_create_index_wizard(&mut self) {
        let table = self
            .browse()
            .table
            .clone()
            .or_else(|| {
                self.schema
                    .iter()
                    .find(|o| o.kind == ObjectKind::Table)
                    .map(|o| o.name.clone())
            })
            .unwrap_or_default();
        self.wizard_index_name = format!("idx_{table}");
        self.wizard_index_table = table;
        self.wizard_index_cols = String::new();
        self.wizard_index_unique = false;
        self.wizard_index_original = None;
        self.overlay = Some(OverlayKind::EditIndex);
    }

    pub fn open_modify_index_wizard(&mut self) -> Result<(), String> {
        let sel = self
            .selected
            .clone()
            .ok_or_else(|| "Select an index".to_string())?;
        if sel.kind != ObjectKind::Index {
            return Err("Select an index object".into());
        }
        let conn = self.conn.as_ref().ok_or("No database open")?;
        let table = self
            .schema
            .iter()
            .find(|o| o.name == sel.name && o.kind == ObjectKind::Index)
            .and_then(|o| o.tbl_name.clone())
            .ok_or("Index table missing")?;
        let (unique, partial): (bool, bool) = conn
            .query_row(
                "SELECT \"unique\", partial FROM pragma_index_list(?1) WHERE name=?2",
                rusqlite::params![table, sel.name],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        if partial {
            return Err(
                "Use Execute SQL to modify a partial index without losing its WHERE clause".into(),
            );
        }
        let mut stmt = conn
            .prepare(
                "SELECT name, desc, coll FROM pragma_index_xinfo(?1) WHERE key=1 ORDER BY seqno",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([&sel.name], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, bool>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut columns = Vec::new();
        for row in rows {
            let (name, desc, collation) = row.map_err(|e| e.to_string())?;
            let name = name.ok_or("Use Execute SQL to modify an expression index")?;
            columns.push(format!(
                "{} COLLATE {} {}",
                db::quote_ident(&name),
                db::quote_ident(&collation),
                if desc { "DESC" } else { "ASC" }
            ));
        }
        self.wizard_index_name = sel.name.clone();
        self.wizard_index_original = Some(sel.name);
        self.wizard_index_table = table;
        self.wizard_index_unique = unique;
        self.wizard_index_cols = columns.join(", ");
        self.overlay = Some(OverlayKind::EditIndex);
        Ok(())
    }

    pub fn apply_table_wizard(&mut self) -> Result<(), String> {
        self.guard_writable()?;
        let name = self.wizard_table_name.trim().to_string();
        if name.is_empty() {
            return Err("Table name required".into());
        }
        if self.wizard_cols.is_empty() {
            return Err("Add at least one column".into());
        }
        for c in &self.wizard_cols {
            if c.name.trim().is_empty() {
                return Err("Column name required".into());
            }
        }
        let conn = self.conn.as_ref().unwrap();
        if let Some(orig) = self.wizard_table_original.clone() {
            db::recreate_table(conn, &orig, &name, &self.wizard_cols)?;
            self.log_sql(format!("-- modify table {orig} → {name}"));
        } else {
            let ddl = db::create_table_ddl(&name, &self.wizard_cols);
            conn.execute_batch(&ddl).map_err(|e| e.to_string())?;
            self.log_sql(ddl);
        }
        self.mark_dirty();
        self.overlay = None;
        self.refresh_schema()?;
        {
            let t = self.browse_mut();
            t.table = Some(name.clone());
            t.title = name.clone();
        }
        self.refresh_browse()?;
        self.status_msg = format!("Table `{name}` saved (unwritten)");
        Ok(())
    }

    pub fn apply_index_wizard(&mut self) -> Result<(), String> {
        self.guard_writable()?;
        let name = self.wizard_index_name.trim().to_string();
        let table = self.wizard_index_table.trim().to_string();
        let cols = self.wizard_index_cols.trim().to_string();
        if name.is_empty() || table.is_empty() || cols.is_empty() {
            return Err("Index needs name, table, and columns".into());
        }
        let conn = self.conn.as_ref().unwrap();
        db::with_savepoint(conn, || {
            if let Some(orig) = self.wizard_index_original.as_ref() {
                db::drop_object(conn, ObjectKind::Index, orig)?;
            }
            db::create_index(conn, &name, &table, &cols, self.wizard_index_unique)
        })?;
        self.log_sql(format!(
            "CREATE {}INDEX {name} ON {table}({cols})",
            if self.wizard_index_unique {
                "UNIQUE "
            } else {
                ""
            }
        ));
        self.mark_dirty();
        self.overlay = None;
        self.refresh_schema()?;
        self.status_msg = format!("Index `{name}` saved (unwritten)");
        Ok(())
    }

    pub fn csv_options(&self) -> CsvImportOptions {
        let delimiter = match self.csv_delim_idx {
            1 => ';',
            2 => '\t',
            3 => '|',
            _ => ',',
        };
        let quote = match self.csv_quote_idx {
            1 => Some('\''),
            2 => None,
            _ => Some('"'),
        };
        CsvImportOptions {
            has_header: self.csv_has_header,
            delimiter,
            quote,
        }
    }

    pub fn apply_csv_import(&mut self) -> Result<(), String> {
        self.guard_writable()?;
        let table = self.csv_table.trim().to_string();
        if table.is_empty() {
            return Err("Table name required".into());
        }
        let opts = self.csv_options();
        let (cols, rows) = if self.overlay == Some(OverlayKind::PasteCsv) {
            db::import_csv_from_text(&self.csv_paste, &opts)?
        } else {
            db::import_csv_parse_opts(Path::new(&self.csv_path), &opts)?
        };
        let n = db::import_csv_into_table(self.conn.as_ref().unwrap(), &table, &cols, &rows, true)?;
        self.log_sql(format!("-- import csv → {table} ({n} rows)"));
        self.mark_dirty();
        self.overlay = None;
        self.refresh_schema()?;
        *self.browse_mut() = BrowseTabState::new(Some(table.clone()));
        self.refresh_browse()?;
        self.status_msg = format!("Imported {n} rows into `{table}` (unwritten)");
        Ok(())
    }

    pub fn pick_chart_source(&mut self) {
        self.chart_column = if self.chart_column.trim().is_empty() {
            "price".into()
        } else {
            self.chart_column.clone()
        };
        let names = self
            .schema
            .iter()
            .filter(|o| o.kind == ObjectKind::Table)
            .map(|o| o.name.clone())
            .collect::<Vec<_>>();
        let preferred = ["products", "order_items", "orders"];
        self.chart_table = preferred
            .iter()
            .copied()
            .chain(names.iter().map(String::as_str))
            .find(|name| {
                names.iter().any(|n| n == name)
                    && self.conn.as_ref().is_some_and(|conn| {
                        db::numeric_series(conn, name, self.chart_column.trim(), 8).is_ok()
                    })
            })
            .map(String::from);
    }

    pub fn table_names(&self) -> Vec<String> {
        self.schema
            .iter()
            .filter(|o| matches!(o.kind, ObjectKind::Table | ObjectKind::View))
            .map(|o| o.name.clone())
            .collect()
    }

    pub fn column_type_names() -> Vec<String> {
        COLUMN_TYPES.iter().map(|s| (*s).to_string()).collect()
    }

    fn adopt_conn(
        &mut self,
        mut conn: LoggedConnection,
        path: String,
        read_only: bool,
    ) -> Result<(), String> {
        self.guard_replace()?;
        db::list_schema(&conn)?;
        conn.bind_log(&self.sql_log);
        self.conn = Some(conn);
        self.cipher_active = false;
        self.cipher_open_path = None;
        self.selected = None;
        self.sql_result = None;
        self.sql_outputs.clear();
        self.path = path;
        self.path_draft = self.path.clone();
        self.read_only = read_only;
        self.dirty = false;
        self.begin_txn()?;
        self.refresh_schema()?;
        self.refresh_pragmas()?;
        let first = self
            .schema
            .iter()
            .find(|o| o.kind == ObjectKind::Table)
            .map(|o| o.name.clone());
        self.browse_tabs = vec![BrowseTabState::new(first)];
        self.browse_tab_idx = 0;
        self.pick_chart_source();
        self.refresh_browse()?;
        self.refresh_chart()?;
        self.error_msg = None;
        if self.path != ":memory:" && !self.path.is_empty() {
            self.push_recent(&self.path.clone());
        }
        self.probe_cipher_linked();
        self.project_path = None;
        self.project_xml = None;
        self.checkpoint_project();
        Ok(())
    }

    pub fn open_path(&mut self, path: &str) -> Result<(), String> {
        self.guard_replace()?;
        if self.request_cipher_open(path, false)? {
            return Ok(());
        }
        let conn = db::open_path(path)?;
        let p = if path.is_empty() {
            ":memory:".into()
        } else {
            path.to_string()
        };
        self.log_sql(format!("-- open {p}"));
        self.adopt_conn(conn, p.clone(), false)?;
        self.status_msg = format!("Opened {p}");
        Ok(())
    }

    pub fn open_readonly_path(&mut self, path: &str) -> Result<(), String> {
        self.guard_replace()?;
        if self.request_cipher_open(path, true)? {
            return Ok(());
        }
        let conn = db::open_readonly(path)?;
        self.log_sql(format!("-- open read-only {path}"));
        self.adopt_conn(conn, path.to_string(), true)?;
        self.probe_cipher_linked();
        self.status_msg = format!("Opened read-only {path}");
        Ok(())
    }

    pub fn create_new_db(&mut self, path: &str) -> Result<(), String> {
        self.guard_replace()?;
        let conn = db::create_new(path)?;
        self.log_sql(format!("-- create {path}"));
        self.schema.clear();
        self.sql_result = None;
        self.sql_outputs.clear();
        self.adopt_conn(conn, path.to_string(), false)?;
        self.status_msg = format!("Created {path}");
        Ok(())
    }

    pub fn new_memory_db(&mut self) -> Result<(), String> {
        self.guard_replace()?;
        let conn = open_shared_memory()?;
        self.log_sql("-- new in-memory");
        self.schema.clear();
        self.sql_result = None;
        self.sql_outputs.clear();
        self.adopt_conn(conn, ":memory:".into(), false)?;
        self.status_msg = "New in-memory database".into();
        Ok(())
    }

    #[allow(dead_code)]
    pub fn new_demo_db(&mut self) -> Result<(), String> {
        let path = db::default_demo_path();
        let conn = db::create_new(path.to_str().unwrap())?;
        db::seed_demo(&conn)?;
        self.log_sql("-- seed demo");
        self.adopt_conn(conn, path.display().to_string(), false)?;
        self.browse_tabs = vec![BrowseTabState::new(Some("products".into()))];
        self.chart_column = "price".into();
        self.pick_chart_source();
        self.refresh_browse()?;
        self.refresh_chart()?;
        self.status_msg = format!("Demo DB ready at {}", self.path);
        Ok(())
    }

    pub fn save_as(&mut self, dst: &str) -> Result<(), String> {
        if self.sql_busy {
            return Err("Wait for SQL to finish".into());
        }
        if Path::new(dst).exists() {
            return Err("Save As requires a new path".into());
        }
        if self.cipher_active {
            return Err("Use encrypted export to save an encrypted database".into());
        }
        if !self.read_only {
            self.write_changes()?;
        }
        let conn = self.conn.as_ref().ok_or("No database open")?;
        db::copy_database_file(conn, Path::new(dst))?;
        let mut next = db::open_path(dst)?;
        next.bind_log(&self.sql_log);
        self.conn = Some(next);
        self.path = dst.to_string();
        self.path_draft = self.path.clone();
        self.read_only = false;
        self.dirty = false;
        self.begin_txn()?;
        self.push_recent(dst);
        self.status_msg = format!("Saved As {dst}");
        Ok(())
    }

    fn guard_replace(&self) -> Result<(), String> {
        if self.sql_busy {
            return Err("Stop or finish SQL before changing databases".into());
        }
        if self.has_cell_draft() { return Err("Apply or discard the cell draft first".into()); }
        if self.dirty {
            return Err("Write Changes or Revert before changing databases".into());
        }
        Ok(())
    }

    pub fn close_db(&mut self) {
        if let Some(conn) = self.conn.take() {
            let _ = conn.execute_batch("ROLLBACK");
            drop(conn);
        }
        self.schema.clear();
        self.database_encoding.clear();
        self.selected = None;
        self.browse_tabs = vec![BrowseTabState::new(None)];
        self.browse_tab_idx = 0;
        self.sql_result = None;
        self.sql_outputs.clear();
        self.pragmas.clear();
        self.chart_points.clear();
        self.path.clear();
        self.read_only = false;
        self.dirty = false;
        self.cipher_active = false;
        self.cipher_open_path = None;
        self.cipher_passphrase.clear();
        self.log_sql("-- close");
        self.project_path = None;
        self.project_xml = None;
        self.checkpoint_project();
        self.status_msg = "Database closed".into();
    }

    pub fn guard_writable(&self) -> Result<(), String> {
        if self.conn.is_none() {
            return Err("No database open".into());
        }
        if self.read_only {
            return Err("Database is open read-only".into());
        }
        Ok(())
    }

    pub fn recent_path() -> PathBuf {
        db::default_demo_path()
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(".e28_recent")
    }

    pub fn load_recent(&mut self) {
        self.recent.clear();
        if let Ok(text) = std::fs::read_to_string(Self::recent_path()) {
            for line in text.lines().take(8) {
                let l = line.trim();
                if !l.is_empty() {
                    self.recent.push_back(l.to_string());
                }
            }
        }
    }

    pub fn push_recent(&mut self, path: &str) {
        self.recent.retain(|p| p != path);
        self.recent.push_front(path.to_string());
        while self.recent.len() > 5 {
            self.recent.pop_back();
        }
        let body = self.recent.iter().cloned().collect::<Vec<_>>().join("\n");
        if !cfg!(test) {
            let _ = std::fs::write(Self::recent_path(), body);
        }
    }

    pub fn add_browse_tab(&mut self) {
        let table = self.browse().table.clone().or_else(|| {
            self.schema
                .iter()
                .find(|o| o.kind == ObjectKind::Table)
                .map(|o| o.name.clone())
        });
        self.browse_tabs.push(BrowseTabState::new(table));
        self.browse_tab_idx = self.browse_tabs.len() - 1;
        let _ = self.refresh_browse();
    }

    pub fn apply_attach(&mut self) -> Result<(), String> {
        self.guard_writable()?;
        let path = self.attach_path.trim().to_string();
        let schema = self.attach_schema.trim().to_string();
        if path.is_empty() || schema.is_empty() {
            return Err("Attach needs path and schema name".into());
        }
        db::attach_database_mode(self.conn.as_ref().unwrap(), &path, &schema, self.read_only)?;
        self.log_sql(format!("ATTACH DATABASE '{path}' AS {schema}"));
        self.overlay = None;
        self.refresh_schema()?;
        self.status_msg = format!("Attached `{path}` as {schema}");
        Ok(())
    }

    pub fn save_pragmas(&mut self) -> Result<(), String> {
        self.guard_writable()?;
        self.guard_replace()?;
        let pairs: Vec<(String, String)> = self
            .pragmas
            .iter()
            .enumerate()
            .map(|(i, (k, _))| {
                let v = self.pragma_drafts.get(i).cloned().unwrap_or_default();
                (k.clone(), v)
            })
            .collect();
        let mut logs = Vec::new();
        {
            let conn = self.conn.as_ref().unwrap();
            conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
            let result = (|| {
                for (k, v) in &pairs {
                    let vv = v.trim();
                    if vv.is_empty() {
                        continue;
                    }
                    let out = match k.as_str() {
                        "foreign_keys" | "recursive_triggers" => {
                            match vv.to_ascii_lowercase().as_str() {
                                "1" | "on" | "true" => "ON",
                                "0" | "off" | "false" => "OFF",
                                _ => return Err(format!("Invalid boolean PRAGMA {k}")),
                            }
                        }
                        _ => vv,
                    };
                    if self.pragmas.iter().any(|(key, old)| key == k && old == v) {
                        continue;
                    }
                    db::write_pragma(conn, k, out)?;
                    let actual = db::read_pragma(conn, k)?;
                    let lower = out.to_ascii_lowercase();
                    let expected = match (k.as_str(), lower.as_str()) {
                        ("foreign_keys" | "recursive_triggers", "on" | "true") => "1",
                        ("foreign_keys" | "recursive_triggers", "off" | "false")
                        | ("synchronous", "off") => "0",
                        ("synchronous", "normal") => "1",
                        ("synchronous", "full") => "2",
                        ("synchronous", "extra") => "3",
                        ("temp_store", "default") | ("auto_vacuum", "none") => "0",
                        ("temp_store", "file") | ("auto_vacuum", "full") => "1",
                        ("temp_store", "memory") | ("auto_vacuum", "incremental") => "2",
                        _ => out,
                    };
                    let matches = actual.eq_ignore_ascii_case(expected)
                        || matches!(
                            (actual.parse::<i64>(), expected.parse::<i64>()),
                            (Ok(a), Ok(b)) if a == b
                        );
                    if !matches {
                        return Err(format!("PRAGMA {k} requested {out}, actual {actual}; earlier settings may have applied. auto_vacuum changes from NONE require VACUUM."));
                    }
                    logs.push(format!("PRAGMA {k} = {out}"));
                }
                Ok::<_, String>(())
            })();
            db::begin_deferred(conn)?;
            result?;
        }
        for l in logs {
            self.log_sql(l);
        }
        self.refresh_pragmas()?;
        self.status_msg = "Pragmas applied".into();
        Ok(())
    }

    pub fn run_sql_sync(&mut self) -> Result<(), String> {
        let sql = self.sql_editor().to_string();
        self.run_sql_text_sync(&sql)
    }

    pub fn run_sql_text_sync(&mut self, sql: &str) -> Result<(), String> {
        self.sql_sensitive = crate::sql_log::sensitive(sql);
        self.sql_log.push(Source::User, sql);
        let conn = self.conn.as_ref().ok_or("No database open")?;
        let result = conn.user_sql(|| db::execute_sql(conn, sql));
        self.dirty = !self.read_only && db::has_pending_changes(conn)?;
        if !self.read_only {
            db::begin_deferred(conn)?;
        }
        self.finish_sql_result(result)
    }

    /// Move the live connection to the worker so edits, keys and ATTACH share one transaction.
    pub fn start_sql_job(&mut self) -> Result<(), String> {
        self.start_sql_text(self.sql_editor().to_string())
    }

    pub fn start_sql_text(&mut self, sql: String) -> Result<(), String> {
        if self.sql_busy {
            return Err("SQL already running".into());
        }
        if sql.trim().is_empty() {
            return Err("SQL is empty".into());
        }
        let worker = self.conn.take().ok_or("No database open")?;
        let read_only = self.read_only;
        self.sql_cancel.store(false, Ordering::SeqCst);
        let cancel = self.sql_cancel.clone();
        self.sql_interrupt = Some(worker.get_interrupt_handle());
        self.sql_busy = true;
        self.sql_message = "Running…".into();
        self.status_msg = "SQL running…".into();
        self.sql_sensitive = crate::sql_log::sensitive(&sql);
        self.sql_log.push(Source::User, &sql);
        let handle = std::thread::spawn(move || {
            worker.progress_handler(500, Some(move || cancel.load(Ordering::SeqCst)));
            let mut result = worker.user_sql(|| db::execute_sql(&worker, &sql));
            worker.progress_handler(0, None::<fn() -> bool>);
            let changed = !read_only && db::has_pending_changes(&worker).unwrap_or(true);
            if result.is_err() && worker.is_autocommit() && !read_only {
                result = result.map_err(|e| {
                    format!(
                        "{e}; the transaction ended through explicit SQL or a SQLite rollback; check earlier pending edits"
                    )
                });
            }
            if !read_only {
                if let Err(e) = db::begin_deferred(&worker) {
                    result = Err(e);
                }
            }
            (worker, result, changed)
        });
        self.sql_job = Some(SqlJob { handle });
        Ok(())
    }

    pub fn request_sql_stop(&mut self) {
        self.sql_cancel.store(true, Ordering::SeqCst);
        if let Some(h) = self.sql_interrupt.as_ref() {
            h.interrupt();
        }
        self.status_msg = "Stop requested…".into();
    }

    pub fn poll_sql_job(&mut self) -> bool {
        if !self
            .sql_job
            .as_ref()
            .is_some_and(|j| j.handle.is_finished())
        {
            return false;
        }
        let job = self.sql_job.take().unwrap();
        self.sql_busy = false;
        self.sql_interrupt = None;
        let result = match job.handle.join() {
            Ok((conn, result, changed)) => {
                self.conn = Some(conn);
                self.dirty = changed;
                result
            }
            Err(_) => {
                Err("SQL worker panicked; connection closed and pending changes rolled back".into())
            }
        };
        let _ = self.finish_sql_result(result);
        true
    }

    fn finish_sql_result(&mut self, result: Result<QueryResult, String>) -> Result<(), String> {
        self.sql_column_widths.clear();
        self.sql_scroll = (0.0, 0.0);
        let _ = self.refresh_schema();
        let _ = self.refresh_browse();
        match result {
            Ok(res) => {
                self.sql_message = format!(
                    "OK — {} result row(s){}",
                    res.rows.len(),
                    if res.truncated {
                        " (preview limited to 500)"
                    } else {
                        ""
                    }
                );
                self.sql_result = Some(res);
                self.status_msg = self.sql_message.clone();
                self.error_msg = None;
                Ok(())
            }
            Err(e) => {
                self.sql_log.push(Source::Error, if self.sql_sensitive { "SQLCipher statement failed (details redacted)" } else { &e });
                self.sql_result = None;
                self.error_msg = Some(e.clone());
                self.sql_message = e.clone();
                self.status_msg = e.clone();
                Err(e)
            }
        }
    }
}

fn open_or_create_demo(path: &Path) -> Result<LoggedConnection, String> {
    if path.exists() {
        db::open_path(path.to_str().unwrap_or(":memory:"))
    } else {
        let conn = db::create_new(path.to_str().unwrap())?;
        db::seed_demo(&conn)?;
        Ok(conn)
    }
}

fn open_shared_memory() -> Result<LoggedConnection, String> {
    db::open_path(":memory:")
}

impl AppState {
    pub fn sync_prefs_drafts(&mut self) {
        self.prefs_font_size = format!("{}", self.prefs.font_size as i32);
        self.prefs_theme_dark = self.prefs.default_theme_dark;
        self.prefs_confirm_drops = self.prefs.confirm_drops;
        self.prefs_csv_delim = self.prefs.csv_delimiter.to_string();
        self.prefs_frozen_cols = self.frozen_cols;
    }

    pub fn apply_prefs_overlay(&mut self) -> Result<(), String> {
        let fs: f32 = self
            .prefs_font_size
            .trim()
            .parse()
            .map_err(|_| "font size must be a number".to_string())?;
        if !fs.is_finite() {
            return Err("font size must be finite".into());
        }
        self.prefs.font_size = fs.clamp(9.0, 28.0);
        self.prefs.default_theme_dark = self.prefs_theme_dark;
        self.prefs.confirm_drops = self.prefs_confirm_drops;
        if let Some(c) = self.prefs_csv_delim.chars().next() {
            self.prefs.csv_delimiter = if c == 't' { '\t' } else { c };
        }
        self.prefs.frozen_cols_default = self.prefs_frozen_cols;
        self.frozen_cols = self.prefs_frozen_cols;
        self.prefs.save()?;
        self.status_msg = format!(
            "Preferences saved (font {}, theme {})",
            self.prefs.font_size,
            if self.prefs.default_theme_dark {
                "dark"
            } else {
                "light"
            }
        );
        Ok(())
    }

    fn request_cipher_open(&mut self, path: &str, read_only: bool) -> Result<bool, String> {
        if Path::new(path).is_file()
            && std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 0
            && !db::looks_like_sqlite(Path::new(path))?
        {
            self.cipher_open_path = Some((path.into(), read_only));
            self.cipher_passphrase.clear();
            self.overlay = Some(OverlayKind::Cipher);
            self.status_msg = "Enter the database key; the current database remains open".into();
            return Ok(true);
        }
        Ok(false)
    }

    pub fn apply_cipher_overlay(&mut self) -> Result<(), String> {
        self.guard_replace()?;
        let (path, read_only) = self
            .cipher_open_path
            .clone()
            .ok_or("Select an encrypted database to open")?;
        let page = self
            .cipher_page_size
            .trim()
            .parse()
            .map_err(|_| "Invalid page size")?;
        let kdf = self
            .cipher_kdf_iter
            .trim()
            .parse()
            .map_err(|_| "Invalid KDF iterations")?;
        let conn = if read_only {
            db::open_readonly(&path)?
        } else {
            db::open_path(&path)?
        };
        db::apply_cipher_key(&conn, &self.cipher_passphrase, page, kdf)?;
        self.adopt_conn(conn, path, read_only)?;
        self.cipher_active = true;
        self.cipher_passphrase.clear();
        self.status_msg = "Encrypted database opened".into();
        Ok(())
    }

    pub fn export_sql_with_options(&mut self, path: &std::path::Path) -> Result<(), String> {
        let Some(conn) = self.conn.as_ref() else {
            return Err("No database open".into());
        };
        let opts = ExportSqlOptions {
            structure: self.export_structure,
            data: self.export_data,
            drop_if_exists: self.export_drop,
            keep_fk: self.export_keep_fk,
        };
        if !opts.structure && !opts.data {
            return Err("Select structure and/or data".into());
        }
        db::export_sql_dump_opts(conn, path, &opts)?;
        self.status_msg = format!("Exported SQL → {}", path.display());
        Ok(())
    }

    pub fn save_project(&mut self, path: &std::path::Path) -> Result<(), String> {
        if let Some(conn) = self.conn.as_ref() {
            db::guard_output_path(conn, path)?;
        }
        if self.path == ":memory:" {
            return Err("Save the in-memory database to a file before saving its project".into());
        }
        let mut pf = crate::project::ProjectFile::from_state(self);
        if !pf.db_path.is_empty() {
            pf.db_path = Path::new(&pf.db_path)
                .canonicalize()
                .map_err(|e| e.to_string())?
                .display()
                .to_string();
        }
        if let Some(conn) = &self.conn { crate::project_xml::capture_connection(&mut pf, conn)?; }
        pf.save(path)?;
        self.project_xml = pf.db4s_xml;
        self.checkpoint_project();
        self.project_path = Some(path.display().to_string());
        self.status_msg = format!("Project saved → {}", path.display());
        Ok(())
    }

    fn preflight_project_browse(pf: &mut crate::project::ProjectFile, conn: &Connection) -> Result<(), String> {
        if pf.browse_tabs.is_empty() {
            if let Some(table) = &pf.browse_table {
                let mut tab = BrowseTabState::new(Some(table.clone()));
                tab.filter = pf.browse_filter.clone();
                pf.browse_tabs.push(tab);
            }
        }
        for tab in &pf.browse_tabs {
            let Some(table) = &tab.table else { continue; };
            let result = (|| {
                let columns = db::table_column_names(conn, table)?;
                db::browse_table(conn, table, tab.order_col.as_deref(), tab.order_desc,
                    Self::composed_where(tab, &columns)?.as_deref(), db::DEFAULT_LIMIT, tab.offset)
            })();
            result.map_err(|e| format!("Browse tab {}: {e}", tab.title))?;
        }
        Ok(())
    }

    pub fn open_project(&mut self, path: &std::path::Path) -> Result<(), String> {
        self.guard_replace()?;
        if self.has_unsaved_drafts() {
            return Err("Save the current project before replacing its drafts/settings".into());
        }
        let mut pf = crate::project::ProjectFile::load(path)?;
        let unapplied = crate::project_xml::unapplied_settings(&pf)?;
        pf.sql_files.resize_with(pf.sql_tabs.len(), || None);
        for (index, file) in pf.sql_files.iter_mut().enumerate() {
            if let Some(file) = file {
                if file.path.is_relative() { file.path = path.parent().unwrap_or(Path::new(".")).join(&file.path); }
                file.saved = std::fs::read_to_string(&file.path).map_err(|e| format!("Cannot open SQL file {}: {e}", file.path.display()))?;
                pf.sql_tabs[index] = file.saved.clone();
            }
        }
        if !pf.db_path.is_empty()
            && pf.db_path != ":memory:"
            && Path::new(&pf.db_path).is_relative()
        {
            pf.db_path = path
                .parent()
                .unwrap_or(Path::new("."))
                .join(&pf.db_path)
                .display()
                .to_string();
        }
        if pf.db_path.is_empty() || pf.db_path == ":memory:" {
            let conn = db::open_path(":memory:")?;
            crate::project_xml::restore_connection(&pf, &conn, path)?;
            crate::project_xml::restore_browse(&mut pf, &conn)?;
            Self::preflight_project_browse(&mut pf, &conn)?;
            self.adopt_conn(conn, ":memory:".into(), false)?;
        } else {
            if !Path::new(&pf.db_path).is_file() {
                return Err("Project database does not exist".into());
            }
            if self.cipher_active && self.path == pf.db_path {
                if pf.read_only != self.read_only {
                    return Err("Reopen the encrypted database in the project's access mode before loading it".into());
                }
                // Reuse the already unlocked connection; keys are never stored in projects.
                let conn = self.conn.as_ref().ok_or("No unlocked connection")?;
                crate::project_xml::restore_browse(&mut pf, conn)?;
                Self::preflight_project_browse(&mut pf, conn)?;
                if !conn.is_autocommit() { conn.execute_batch("COMMIT").map_err(|e| e.to_string())?; }
                let result = crate::project_xml::restore_connection(&pf, conn, path);
                db::begin_deferred(conn)?;
                result?;
            } else {
                if !db::looks_like_sqlite(Path::new(&pf.db_path))? {
                    return Err(
                        "Open and unlock the encrypted database before loading its project".into(),
                    );
                }
                let conn = if pf.read_only { db::open_readonly(&pf.db_path)? } else { db::open_path(&pf.db_path)? };
                db::list_schema(&conn)?;
                crate::project_xml::restore_connection(&pf, &conn, path)?;
                crate::project_xml::restore_browse(&mut pf, &conn)?;
                Self::preflight_project_browse(&mut pf, &conn)?;
                self.adopt_conn(conn, pf.db_path.clone(), pf.read_only)?;
            }
        }
        pf.apply_to(self);
        self.refresh_pragmas()?;
        self.project_path = Some(path.display().to_string());
        self.refresh_browse()?;
        self.checkpoint_project();
        self.status_msg = format!("Project opened ← {}", path.display());
        if !unapplied.is_empty() { self.status_msg.push_str(&format!("; retained but not applied: {}", unapplied.join(", "))); }
        Ok(())
    }

    pub fn close_project(&mut self) {
        self.close_db();
        self.status_msg = "Project closed".into();
    }

    pub fn execute_current_line(&mut self, caret: usize) -> Result<(), String> {
        let sql = self.sql_editor();
        let byte = sql.char_indices().nth(caret).map_or(sql.len(), |(i, _)| i);
        let start = sql[..byte].rfind('\n').map_or(0, |i| i + 1);
        let end = sql[byte..].find('\n').map_or(sql.len(), |i| byte + i);
        self.start_sql_text(sql[start..end].to_string())
    }

    pub fn toggle_sql_line_comment(&mut self) {
        let Some(sql) = self.sql_tabs.get_mut(self.sql_tab_idx) else {
            return;
        };
        let mut out = String::new();
        for line in sql.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("--") {
                // uncomment
                let rest = trimmed.trim_start_matches('-').trim_start_matches('-');
                let rest = rest.strip_prefix(' ').unwrap_or(rest);
                let indent_len = line.len() - trimmed.len();
                out.push_str(&line[..indent_len]);
                out.push_str(rest);
            } else if !trimmed.is_empty() {
                out.push_str("-- ");
                out.push_str(line);
            } else {
                out.push_str(line);
            }
            out.push('\n');
        }
        *sql = out;
        self.status_msg = "Toggled -- comments on SQL tab".into();
    }

    pub fn apply_cell_draft(&mut self) -> Result<(), String> {
        self.guard_writable()?;
        let tab = self.browse();
        let table = tab.table.clone().ok_or("No browse table")?;
        let row = tab.sel_row.ok_or("Select a browse row first")?;
        let rowid = *tab.rowids.get(row).ok_or("No usable rowid for selection")?;
        let column = self.edit_col.trim().to_string();
        if column.is_empty() { return Err("Select a column first".into()); }
        let value = self.edited_cell_value()?;
        db::update_cell(self.conn.as_ref().unwrap(), &table, rowid, &column, &value)?;
        self.log_sql(format!("UPDATE {table} SET {column}=… WHERE rowid={rowid}"));
        self.mark_dirty();
        self.edit_saved = value;
        self.refresh_browse()?;
        self.status_msg = "Cell updated (unwritten)".into();
        Ok(())
    }

    /// Paste a tab-separated matrix into the visible browse page.
    pub fn paste_browse_tsv(&mut self, text: &str) -> Result<(), String> {
        self.guard_writable()?;
        if self.has_cell_draft() {
            return Err("Apply or discard the cell draft first".into());
        }
        let text = text.trim_end_matches(['\r', '\n']);
        if text.is_empty() {
            return Err("Clipboard is empty".into());
        }
        let (table, start_row, start_col, rowids, columns, row_count) = {
            let tab = self.browse();
            let result = tab.result.as_ref().ok_or("No browse data")?;
            (
                tab.table.clone().ok_or("No browse table")?,
                tab.sel_row_start.or(tab.sel_row).ok_or("Select a browse cell first")?,
                tab.sel_col_start.or(tab.sel_col).unwrap_or(0),
                tab.rowids.clone(),
                result.columns.clone(),
                result.rows.len(),
            )
        };
        let rows = text
            .split('\n')
            .map(|line| line.trim_end_matches('\r').split('\t').collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let width = rows.first().map_or(0, Vec::len);
        if width == 0 || rows.iter().any(|row| row.len() != width) {
            return Err("Clipboard rows have inconsistent column counts".into());
        }
        if start_row.saturating_add(rows.len()) > row_count {
            return Err("Clipboard does not fit in the visible browse page".into());
        }
        if start_col.saturating_add(width) > columns.len() {
            return Err("Clipboard does not fit in the visible columns".into());
        }
        let mut updates = Vec::with_capacity(rows.len() * width);
        for (ri, row) in rows.iter().enumerate() {
            let rowid = *rowids.get(start_row + ri).ok_or("Selected rows are not editable")?;
            for (ci, value) in row.iter().enumerate() {
                let value = if *value == "NULL" {
                    rusqlite::types::Value::Null
                } else {
                    rusqlite::types::Value::Text((*value).to_string())
                };
                updates.push((rowid, columns[start_col + ci].clone(), value));
            }
        }
        let conn = self.conn.as_ref().ok_or("No database open")?;
        db::with_savepoint(conn, || {
            for (rowid, column, value) in &updates {
                db::update_cell(conn, &table, *rowid, column, value)?;
            }
            Ok(())
        })?;
        self.log_sql(format!("-- paste {}x{} into {table}", rows.len(), width));
        self.mark_dirty();
        self.refresh_browse()?;
        self.status_msg = format!("Pasted {}x{} cells (unwritten)", rows.len(), width);
        Ok(())
    }

    pub fn has_pragma_drafts(&self) -> bool {
        self.pragmas.iter().zip(&self.pragma_drafts).any(|((_, old), draft)| old != draft)
    }

    /// Save drafts through the same write paths used by their individual controls.
    /// Destinations are chosen before calling so canceled pickers cannot partially save.
    pub fn save_documents(&mut self, database: Option<&Path>, project: Option<&Path>) -> Result<(), String> {
        if self.sql_busy { return Err("Stop or finish SQL before saving".into()); }
        if self.path == ":memory:" && database.is_none() {
            return Err("Choose a destination for the in-memory database".into());
        }
        if let Some(path) = database {
            if path.exists() { return Err("Save As requires a new path".into()); }
        }
        if self.has_project_drafts() && project.is_none() {
            return Err("Choose a project destination for the drafts/settings".into());
        }
        if let Some(path) = project {
            if let Some(conn) = &self.conn { db::guard_output_path(conn, path)?; }
            if database == Some(path) { return Err("Database and project destinations must differ".into()); }
        }
        for i in 0..self.sql_tabs.len() {
            if self.sql_tab_dirty(i) {
                if let Some(Some(file)) = self.sql_files.get(i) { self.save_sql_file(i, &file.path.clone())?; }
            }
        }
        if self.has_cell_draft() { self.apply_cell_draft()?; }
        if self.dirty { self.write_changes()?; }
        if self.has_pragma_drafts() { self.save_pragmas()?; }
        if let Some(path) = database { self.save_as(&path.to_string_lossy())?; }
        if let Some(path) = project { self.save_project(path)?; }
        self.status_msg = "All changes saved".into();
        Ok(())
    }

    pub fn discard_document_changes(&mut self) -> Result<(), String> {
        if self.sql_busy { return Err("Stop or finish SQL before discarding".into()); }
        if self.dirty {
            let conn = self.conn.as_ref().ok_or("No database open")?;
            db::rollback_and_begin(conn)?;
            self.dirty = false;
        }
        let saved: crate::project::ProjectFile = serde_json::from_value(self.saved_project.clone()).map_err(|e| e.to_string())?;
        saved.apply_to(self);
        for (i, file) in self.sql_files.iter().enumerate() {
            if let Some(file) = file {
                if let Some(text) = self.sql_tabs.get_mut(i) { *text = file.saved.clone(); }
            }
        }
        self.checkpoint_project();
        self.edit_col.clear();
        self.edit_value.clear();
        self.edit_image_rgba = None;
        self.sql_edit_states.clear();
        self.find_search.reset(self.sql_tabs.get(self.sql_tab_idx).map(String::as_str).unwrap_or_default(), 0..0);
        self.find_message.clear();
        self.refresh_schema()?;
        self.refresh_pragmas()?;
        self.refresh_browse()?;
        self.chart_points.clear();
        Ok(())
    }

    pub fn discard_cell_draft(&mut self) {
        self.edit_col.clear();
        self.edit_value.clear();
        self.edit_image_rgba = None;
        self.sync_edit_cell_from_selection();
    }

    pub fn save_results_as_view(&mut self, name: &str) -> Result<(), String> {
        self.guard_writable()?;
        let name = name.trim();
        if name.is_empty() {
            return Err("View name required".into());
        }
        let sql = self.sql_result.as_ref().and_then(|r| r.statement.as_deref())
            .ok_or("Execute a SELECT in this tab before saving its result as a view")?;
        let body = sql.trim().trim_end_matches(';');
        if body.is_empty() {
            return Err("SQL editor empty".into());
        }
        let ddl = format!("CREATE VIEW {} AS {}", db::quote_ident(name), body);
        {
            let conn = self.conn.as_ref().ok_or("No database open")?;
            conn.execute(&ddl, []).map_err(|e| e.to_string())?;
        }
        self.dirty = true;
        self.log_sql(&ddl);
        self.refresh_schema()?;
        self.status_msg = format!("Created VIEW {name}");
        Ok(())
    }

    pub fn add_cond_format_rule(&mut self) {
        let col = if self.cond_col.is_empty() {
            self.edit_col.clone()
        } else {
            self.cond_col.clone()
        };
        if col.is_empty() {
            self.status_msg = "Pick a column for conditional formatting".into();
            return;
        }
        let rgb =
            u32::from_str_radix(self.cond_color.trim_start_matches('#'), 16).unwrap_or(0xE67E22);
        self.cond_rules.push(CondFormatRule {
            table: self.browse().table.clone(),
            column: col,
            op: self.cond_op_idx,
            value: self.cond_value.clone(),
            color_rgb: rgb,
        });
        self.status_msg = format!("Added format rule ({} rules)", self.cond_rules.len());
    }

    pub fn cell_format_colors(&self) -> Vec<Vec<Option<next_gui_scene::Color>>> {
        let Some(res) = self.browse().result.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(res.rows.len());
        for row in &res.rows {
            let mut cols = Vec::with_capacity(res.columns.len());
            for (ci, cname) in res.columns.iter().enumerate() {
                let val = row.get(ci).map(|s| s.as_str()).unwrap_or("");
                let mut tint = None;
                for rule in &self.cond_rules {
                    if rule.table != self.browse().table || &rule.column != cname {
                        continue;
                    }
                    let hit = match rule.op {
                        1 => val != rule.value,
                        2 => val.contains(&rule.value),
                        3 => {
                            let a = val.parse::<f64>().ok();
                            let b = rule.value.parse::<f64>().ok();
                            matches!((a, b), (Some(a), Some(b)) if a > b)
                        }
                        4 => {
                            let a = val.parse::<f64>().ok();
                            let b = rule.value.parse::<f64>().ok();
                            matches!((a, b), (Some(a), Some(b)) if a < b)
                        }
                        _ => val == rule.value,
                    };
                    if hit {
                        let r = ((rule.color_rgb >> 16) & 0xff) as u8;
                        let g = ((rule.color_rgb >> 8) & 0xff) as u8;
                        let b = (rule.color_rgb & 0xff) as u8;
                        tint = Some(next_gui_scene::Color::rgb(r, g, b));
                        break;
                    }
                }
                cols.push(tint);
            }
            out.push(cols);
        }
        out
    }

    pub fn sync_edit_cell_from_selection(&mut self) {
        if self.has_cell_draft() { self.error_msg = Some("Apply or discard the cell draft first".into()); return; }
        let b = self.browse();
        let (Some(ri), Some(res), Some(table), Some(conn)) = (
            b.sel_row,
            b.result.as_ref(),
            b.table.as_ref(),
            self.conn.as_ref(),
        ) else {
            return;
        };
        let ci = b.sel_col.unwrap_or(0);
        let Some(col) = res.columns.get(ci).cloned() else {
            return;
        };
        let value = if let Some(rowid) = b.rowids.get(ri) {
            conn.query_row(
                &format!(
                    "SELECT {} FROM {} WHERE rowid=?1",
                    db::quote_ident(&col),
                    db::quote_ident(table)
                ),
                [rowid],
                |r| r.get::<_, rusqlite::types::Value>(0),
            )
            .ok()
        } else {
            res.rows
                .get(ri)
                .and_then(|r| r.get(ci))
                .cloned()
                .map(rusqlite::types::Value::Text)
        };
        let Some(value) = value else {
            return;
        };
        let mode = if matches!(value, rusqlite::types::Value::Blob(_)) {
            EditCellMode::Hex
        } else {
            EditCellMode::Text
        };
        self.edit_col = col;
        self.edit_type = match &value {
            rusqlite::types::Value::Null => 0,
            rusqlite::types::Value::Integer(_) => 1,
            rusqlite::types::Value::Real(_) => 2,
            rusqlite::types::Value::Text(_) => 3,
            rusqlite::types::Value::Blob(_) => 4,
        };
        self.edit_image_view = next_gui_ui::ImageView::default();
        self.edit_saved = value.clone();
        self.edit_original = value;
        self.edit_mode = mode;
        self.show_edit_value();
    }

    fn show_edit_value(&mut self) {
        use rusqlite::types::Value;
        let bytes = match &self.edit_original {
            Value::Blob(v) => v.clone(),
            Value::Text(v) => v.as_bytes().to_vec(),
            Value::Integer(v) => v.to_string().into_bytes(),
            Value::Real(v) => v.to_string().into_bytes(),
            Value::Null => Vec::new(),
        };
        self.edit_binary_len = bytes.len();
        self.edit_image_rgba = None;
        self.edit_value = match self.edit_mode {
            EditCellMode::Hex => db::to_hex(&bytes),
            EditCellMode::Image => format!("[{} bytes — read-only image preview]", bytes.len()),
            EditCellMode::Json => db::try_pretty_json(&String::from_utf8_lossy(&bytes))
                .unwrap_or_else(|| String::from_utf8_lossy(&bytes).into_owned()),
            EditCellMode::Text => String::from_utf8_lossy(&bytes).into_owned(),
        };
        if self.edit_mode == EditCellMode::Image {
            self.decode_edit_image();
        }
    }

    pub fn edited_cell_value(&self) -> Result<rusqlite::types::Value, String> {
        use rusqlite::types::Value;
        if self.edit_mode == EditCellMode::Image {
            return Err("Image preview is read-only; use Hex to edit binary data".into());
        }
        if self.edit_type == 0 {
            return Ok(Value::Null);
        }
        let decoded;
        let text = if self.edit_mode == EditCellMode::Hex {
            let bytes = db::from_hex(&self.edit_value)?;
            if self.edit_type == 4 {
                return Ok(Value::Blob(bytes));
            }
            decoded = String::from_utf8(bytes)
                .map_err(|_| "Invalid UTF-8; select BLOB for arbitrary bytes")?;
            &decoded
        } else {
            &self.edit_value
        };
        if self.edit_mode == EditCellMode::Json
            && serde_json::from_str::<serde_json::Value>(text).is_err()
        {
            return Err("Invalid JSON".into());
        }
        match self.edit_type {
            1 => text
                .trim()
                .parse::<i64>()
                .map(Value::Integer)
                .map_err(|_| "Invalid INTEGER; select TEXT to change its type".into()),
            2 => {
                let value = text.trim().parse::<f64>().map_err(|_| "Invalid REAL")?;
                if !value.is_finite() {
                    return Err("REAL must be finite".into());
                }
                Ok(Value::Real(value))
            }
            4 => {
                if let Value::Blob(bytes) = &self.edit_original {
                    if text.as_str() == String::from_utf8_lossy(bytes) {
                        return Ok(self.edit_original.clone());
                    }
                    return Err("Use Hex to edit binary data without changing its type".into());
                }
                Ok(Value::Blob(text.as_bytes().to_vec()))
            }
            _ => Ok(Value::Text(text.to_string())),
        }
    }

    pub fn set_edit_mode(&mut self, mode: EditCellMode) -> Result<(), String> {
        if self.edit_mode != EditCellMode::Image {
            self.edit_original = self.edited_cell_value()?;
        }
        self.edit_mode = mode;
        self.show_edit_value();
        Ok(())
    }

    pub fn navigate_fk(&mut self) -> Result<(), String> {
        let table = self
            .browse()
            .table
            .clone()
            .ok_or_else(|| "No browse table".to_string())?;
        let col = self.edit_col.clone();
        if col.is_empty() {
            return Err("Select a cell first".into());
        }
        let fks = {
            let conn = self.conn.as_ref().ok_or("No database open")?;
            db::list_foreign_keys(conn, &table)?
        };
        let Some(fk) = fks.iter().find(|f| f.from_col == col) else {
            return Err(format!("Column {col} has no foreign key"));
        };
        let conn = self.conn.as_ref().ok_or("No database open")?;
        let rowid = self
            .browse()
            .sel_row
            .and_then(|i| self.browse().rowids.get(i))
            .ok_or("Select a stored row with a rowid")?;
        let mut parts = Vec::new();
        for key in fks.iter().filter(|key| key.id == fk.id) {
            let value = conn
                .query_row(
                    &format!(
                        "SELECT {} FROM {} WHERE rowid=?1",
                        db::quote_ident(&key.from_col),
                        db::quote_ident(&table)
                    ),
                    [rowid],
                    |row| row.get::<_, rusqlite::types::Value>(0),
                )
                .map_err(|e| e.to_string())?;
            if value == rusqlite::types::Value::Null {
                return Err("NULL foreign key has no parent row".into());
            }
            parts.push(format!(
                "{} = {}",
                db::quote_ident(&key.to_col),
                db::sql_literal((&value).into())
            ));
        }
        let filter = parts.join(" AND ");
        self.browse_tabs
            .push(BrowseTabState::new(Some(fk.to_table.clone())));
        self.browse_tab_idx = self.browse_tabs.len() - 1;
        self.browse_mut().filter = filter;
        self.tab = MainTab::Browse;
        self.refresh_browse()?;
        self.status_msg = format!("FK → {}", fk.to_table);
        Ok(())
    }
}

impl AppState {
    pub fn probe_cipher_linked(&mut self) {
        if let Some(conn) = self.conn.as_ref() {
            self.cipher_linked = db::cipher_is_linked(conn);
        } else {
            self.cipher_linked = false;
        }
    }

    pub fn load_extension_apply(&mut self) -> Result<(), String> {
        let path = self.ext_path.trim().to_string();
        if path.is_empty() {
            return Err("Extension path required".into());
        }
        let entry = self.ext_entry.trim().to_string();
        let entry_opt = if entry.is_empty() {
            None
        } else {
            Some(entry.as_str())
        };
        {
            let conn = self.conn.as_ref().ok_or("No database open")?;
            db::load_extension(conn, &path, entry_opt)?;
        }
        self.log_sql(format!("-- load_extension {path}"));
        self.status_msg = format!("Loaded extension {path}");
        Ok(())
    }

    pub fn print_schema_to(&mut self, path: &std::path::Path) -> Result<(), String> {
        let report = {
            let conn = self.conn.as_ref().ok_or("No database open")?;
            db::guard_output_path(conn, path)?;
            db::schema_report(conn)?
        };
        std::fs::write(path, report).map_err(|e| e.to_string())?;
        self.status_msg = format!("Schema report → {}", path.display());
        Ok(())
    }

    pub fn toggle_browse_view(&mut self) {
        self.browse_view = match self.browse_view {
            BrowseViewMode::Table => BrowseViewMode::Form,
            BrowseViewMode::Form => BrowseViewMode::Table,
        };
        if matches!(self.browse_view, BrowseViewMode::Form) {
            let n = self
                .browse()
                .result
                .as_ref()
                .map(|r| r.rows.len())
                .unwrap_or(0);
            if n > 0 && self.browse().sel_row.is_none() {
                self.browse_mut().sel_row = Some(0);
                self.browse_mut().sel_col = Some(0);
            }
            self.sync_edit_cell_from_selection();
        }
    }

    pub fn form_next_record(&mut self) {
        let n = self
            .browse()
            .result
            .as_ref()
            .map(|r| r.rows.len())
            .unwrap_or(0);
        if n == 0 {
            return;
        }
        let cur = self.browse().sel_row.unwrap_or(0);
        self.browse_mut().sel_row = Some((cur + 1).min(n - 1));
        if self.browse().sel_col.is_none() {
            self.browse_mut().sel_col = Some(0);
        }
        self.sync_edit_cell_from_selection();
    }

    pub fn form_prev_record(&mut self) {
        let cur = self.browse().sel_row.unwrap_or(0);
        self.browse_mut().sel_row = Some(cur.saturating_sub(1));
        if self.browse().sel_col.is_none() {
            self.browse_mut().sel_col = Some(0);
        }
        self.sync_edit_cell_from_selection();
    }

    pub fn decode_edit_image(&mut self) {
        self.edit_image_rgba = None;
        let rusqlite::types::Value::Blob(bytes) = &self.edit_original else {
            return;
        };
        if !(db::looks_like_png(bytes) || db::looks_like_jpeg(bytes)) {
            return;
        }
        let decoded = (|| -> Result<_, String> {
            let reader = || {
                image::io::Reader::new(std::io::Cursor::new(bytes))
                    .with_guessed_format()
                    .map_err(|e| e.to_string())
            };
            let (w, h) = reader()?.into_dimensions().map_err(|e| e.to_string())?;
            // Bound retained RGBA and decoder allocations before opening untrusted BLOBs.
            if w == 0 || h == 0 || u64::from(w) * u64::from(h) > 16 * 1024 * 1024 {
                return Err("Image preview is limited to 16 megapixels".into());
            }
            let mut reader = reader()?;
            let mut limits = image::io::Limits::default();
            limits.max_image_width = Some(w);
            limits.max_image_height = Some(h);
            limits.max_alloc = Some(128 * 1024 * 1024);
            reader.limits(limits);
            let rgba = reader.decode().map_err(|e| e.to_string())?.into_rgba8();
            Ok((w, h, rgba.into_raw()))
        })();
        match decoded {
            Ok(image) => {
                self.edit_image_rgba = Some(image);
                self.edit_binary_len = bytes.len();
            }
            Err(error) => self.status_msg = format!("Image preview: {error}"),
        }
    }

    pub fn apply_cipher_encrypt_file(&mut self, dest: &std::path::Path) -> Result<(), String> {
        self.guard_replace()?;
        let page = self
            .cipher_page_size
            .trim()
            .parse()
            .map_err(|_| "Invalid page size")?;
        let kdf = self
            .cipher_kdf_iter
            .trim()
            .parse()
            .map_err(|_| "Invalid KDF iterations")?;
        let conn = self.conn.as_ref().ok_or("No database open")?;
        if !conn.is_autocommit() {
            conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
        }
        let result = db::encrypt_database_to(conn, dest, &self.cipher_passphrase, page, kdf);
        if !self.read_only {
            db::begin_deferred(conn)?;
        }
        result?;
        self.cipher_passphrase.clear();
        self.status_msg = format!("Encrypted copy → {}", dest.display());
        Ok(())
    }

    pub fn remote_add_path(&mut self) {
        let p = self.remote_draft.trim().to_string();
        if p.is_empty() {
            return;
        }
        if !self.remote_paths.contains(&p) {
            self.remote_paths.push(p);
        }
        self.remote_draft.clear();
        self.status_msg = format!(
            "Remote list: {} path(s) (local-only)",
            self.remote_paths.len()
        );
    }


}

#[cfg(test)]
mod chart_default_tests {
    use super::*;
    use crate::db;

    fn test_state() -> AppState {
        let mut state = AppState::empty();
        let mut conn = LoggedConnection::new(Connection::open_in_memory().unwrap()).unwrap();
        conn.bind_log(&state.sql_log);
        state.conn = Some(conn);
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("CREATE TABLE t(v); INSERT INTO t VALUES ('saved'); BEGIN;")
            .unwrap();
        state.path = ":memory:".into();
        state.checkpoint_project();
        state
    }

    #[test]
    fn browse_and_complete_export_share_original_filter_operators() {
        let mut state = AppState::empty();
        let conn = db::open_path(":memory:").unwrap();
        conn.execute_batch("CREATE TABLE t(n INTEGER, label TEXT);
            WITH RECURSIVE x(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM x WHERE n<603)
            INSERT INTO t SELECT n, 'row ' || n FROM x;").unwrap();
        state.conn = Some(conn);
        state.refresh_schema().unwrap();
        state.browse_tabs = vec![BrowseTabState::new(Some("t".into()))];
        state.browse_mut().col_filters = vec![">=2".into()];
        state.refresh_browse().unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows.len(), db::DEFAULT_LIMIT);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("filtered.json");
        state.export_browse(&path, true).unwrap();
        let rows: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 602);
        state.browse_mut().filter = "n <= 3".into();
        state.refresh_browse().unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows.len(), 2);
        state.browse_mut().filter.clear();
        state.browse_mut().col_filters.clear();
        state.browse_mut().global_filters = vec!["row".into(), "=2".into()];
        state.refresh_browse().unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][0], "2");
        assert_eq!(state.browse().result.as_ref().unwrap().rows.len(), 1);
        state.browse_mut().col_filters = vec![">3".into()];
        state.refresh_browse().unwrap();
        assert!(state.browse().result.as_ref().unwrap().rows.is_empty());
        state.browse_mut().global_filters.clear();
        state.browse_mut().filter.clear();
        state.browse_mut().col_filters = vec!["".into(), "/^row (?=[23]$)/".into()];
        state.refresh_browse().unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows.len(), 2);
        state.browse_mut().col_filters[1] = "/[/".into();
        assert!(state.refresh_browse().is_err());
        assert_eq!(state.browse().result.as_ref().unwrap().rows.len(), 2);
        state.browse_mut().col_filters = vec![">0) OR 1=1; DELETE FROM t; --".into()];
        state.refresh_browse().unwrap();
        assert_eq!(db::count_rows(state.conn.as_ref().unwrap(), "t").unwrap(), 603);
    }

    fn wait_sql(state: &mut AppState) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !state.poll_sql_job() {
            assert!(std::time::Instant::now() < deadline, "SQL worker timed out");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    #[test]
    fn sql_files_save_to_their_source_and_reject_external_changes() {
        let mut state = test_state();
        let dir = tempfile::tempdir().unwrap();
        let sql = dir.path().join("query.sql");
        std::fs::write(&sql, "SELECT 1").unwrap();
        state.open_sql_file(&sql).unwrap();
        let index = state.sql_tab_idx;
        assert!(!state.sql_tab_dirty(index));
        assert!(state.sql_tab_label(index).contains("query.sql"));
        *state.sql_editor_mut() = "SELECT 2".into();
        assert!(state.sql_tab_dirty(index));
        assert!(state.sql_tab_label(index).contains('●'));
        assert!(!state.has_project_drafts());
        state.save_sql_file(index, &sql).unwrap();
        assert_eq!(std::fs::read_to_string(&sql).unwrap(), "SELECT 2");
        assert!(!state.sql_tab_dirty(index));
        *state.sql_editor_mut() = "SELECT 3".into();
        std::fs::write(&sql, "SELECT 'external'").unwrap();
        assert!(state.save_sql_file(index, &sql).unwrap_err().contains("outside"));
        assert_eq!(std::fs::read_to_string(&sql).unwrap(), "SELECT 'external'");
        assert_eq!(state.sql_editor(), "SELECT 3");
        let alternate = dir.path().join("new.sql");
        state.save_sql_file(index, &alternate).unwrap();
        assert_eq!(std::fs::read_to_string(alternate).unwrap(), "SELECT 3");
    }

    #[test]
    fn sql_tab_move_and_close_keep_results_files_and_editor_sessions_together() {
        let mut state = test_state();
        state.sql_tabs[0] = "SELECT 11".into();
        state.run_sql_sync().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("query.sql");
        std::fs::write(&source, "SELECT 22").unwrap();
        state.open_sql_file(&source).unwrap();
        state.run_sql_sync().unwrap();
        state.move_sql_tab(1, 0);
        assert_eq!(state.sql_tab_idx, 0);
        assert_eq!(state.sql_result.as_ref().unwrap().rows[0][0], "22");
        assert_eq!(state.sql_files[0].as_ref().unwrap().path, source);
        state.switch_sql_tab(1);
        assert_eq!(state.sql_result.as_ref().unwrap().rows[0][0], "11");
        state.switch_sql_tab(0);
        state.close_sql_tab(0);
        assert_eq!(state.sql_tab_idx, 0);
        assert_eq!(state.sql_result.as_ref().unwrap().rows[0][0], "11");
        state.close_sql_tab(0);
        assert_eq!(state.sql_tabs, vec![String::new()]);
        assert!(state.sql_result.is_none());
    }

    #[test]
    fn sql_dirty_markers_follow_moved_and_closed_unbound_tabs() {
        let mut state = test_state();
        state.sql_tabs = vec!["SELECT 1".into(), "SELECT 2".into()];
        state.checkpoint_project();
        state.sql_tabs[1].push_str(" -- draft");
        state.move_sql_tab(1, 0);
        assert!(state.sql_tab_dirty(0));
        assert!(!state.sql_tab_dirty(1));
        assert!(state.has_project_drafts());
        state.close_sql_tab(0);
        assert_eq!(state.sql_tabs[0], "SELECT 1");
        assert!(!state.sql_tab_dirty(0));
        state.close_sql_tab(0);
        assert!(!state.sql_tab_dirty(0));
    }

    #[test]
    fn save_documents_preserves_typed_cell_and_drafts_in_database_and_project() {
        let mut state = test_state();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        state.browse_mut().sel_row = Some(0);
        state.sync_edit_cell_from_selection();
        state.edit_type = 1;
        state.edit_value = "42".into();
        *state.sql_editor_mut() = "SELECT 'saved draft';".into();
        state.ddl_draft = "CREATE VIEW example AS SELECT 1".into();
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("saved.db");
        let project = dir.path().join("saved.sqbpro");
        state.save_documents(Some(&database), Some(&project)).unwrap();
        assert!(!state.dirty);
        assert!(!state.has_unsaved_drafts());
        let db = Connection::open(&database).unwrap();
        assert_eq!(db.query_row("SELECT v FROM t", [], |row| row.get::<_, i64>(0)).unwrap(), 42);
        let saved = crate::project::ProjectFile::load(&project).unwrap();
        assert_eq!(saved.sql_tabs[0], "SELECT 'saved draft';");
        assert_eq!(saved.ddl_draft, state.ddl_draft);
        assert_eq!(saved.db_path, database.canonicalize().unwrap().display().to_string());
        assert_eq!(saved.browse_tabs[0].table.as_deref(), Some("t"));
    }

    #[test]
    fn canceled_or_invalid_save_destinations_leave_unwritten_data_and_drafts() {
        let mut state = test_state();
        state.run_sql_text_sync("INSERT INTO t VALUES(2)").unwrap();
        *state.sql_editor_mut() = "SELECT 'keep';".into();
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("saved.db");
        assert!(state.save_documents(None, None).is_err());
        assert!(state.save_documents(Some(&database), None).is_err());
        assert!(state.save_documents(Some(&database), Some(&database)).is_err());
        assert!(!database.exists());
        assert!(state.dirty);
        assert_eq!(state.sql_editor(), "SELECT 'keep';");
        let count: i64 = state.conn.as_ref().unwrap().query_row("SELECT count(*) FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 2);
        state.discard_document_changes().unwrap();
        assert!(!state.dirty);
        assert!(!state.has_unsaved_drafts());
        let count: i64 = state.conn.as_ref().unwrap().query_row("SELECT count(*) FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn cell_types_and_invalid_hex_drafts_are_preserved() {
        use rusqlite::types::Value;
        let mut state = test_state();
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("DELETE FROM t; INSERT INTO t VALUES(7)")
            .unwrap();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        state.browse_mut().sel_row = Some(0);
        state.sync_edit_cell_from_selection();
        state.edit_value = "8".into();
        assert_eq!(state.edited_cell_value().unwrap(), Value::Integer(8));
        assert!(state.has_cell_draft());
        state.edit_value = "9223372036854775808".into();
        assert!(state.edited_cell_value().is_err());
        state.edit_type = 3;
        assert_eq!(
            state.edited_cell_value().unwrap(),
            Value::Text(state.edit_value.clone())
        );
        state.edit_type = 0;
        assert_eq!(state.edited_cell_value().unwrap(), Value::Null);
        state.set_edit_mode(EditCellMode::Hex).unwrap();
        state.edit_type = 4;
        state.edit_value = "ZZ".into();
        assert!(state.set_edit_mode(EditCellMode::Text).is_err());
        assert_eq!(state.edit_mode, EditCellMode::Hex);
        assert_eq!(state.edit_value, "ZZ");
        assert!(state.refresh_browse().is_err());
        state.discard_cell_draft();
        assert_eq!(state.edit_value, "7");
        assert!(!state.has_cell_draft());
    }

    #[test]
    fn composite_implicit_and_blob_foreign_keys_navigate_stored_values() {
        let mut state = test_state();
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch(
                "CREATE TABLE parent(a BLOB,b,PRIMARY KEY(a,b));
            INSERT INTO parent VALUES(X'00FF',2),(X'00FF',3);
            CREATE TABLE child(x BLOB,y, FOREIGN KEY(x,y) REFERENCES parent);
            INSERT INTO child VALUES(X'00FF',3),(NULL,2);",
            )
            .unwrap();
        state.refresh_schema().unwrap();
        state.browse_mut().table = Some("child".into());
        state.refresh_browse().unwrap();
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_col = Some(0);
        state.sync_edit_cell_from_selection();
        state.navigate_fk().unwrap();
        assert_eq!(state.browse().table.as_deref(), Some("parent"));
        assert_eq!(state.browse().result.as_ref().unwrap().rows.len(), 1);
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][1], "3");
        state.browse_tab_idx = 0;
        state.browse_mut().sel_row = Some(1);
        state.sync_edit_cell_from_selection();
        assert!(state.navigate_fk().unwrap_err().contains("NULL"));
    }

    #[test]
    fn pragma_reports_actual_value_instead_of_false_success() {
        let mut state = test_state();
        state.refresh_pragmas().unwrap();
        let i = state
            .pragmas
            .iter()
            .position(|(k, _)| k == "auto_vacuum")
            .unwrap();
        state.pragma_drafts[i] = "1".into();
        assert!(state.save_pragmas().unwrap_err().contains("actual 0"));
        assert!(!state.conn.as_ref().unwrap().is_autocommit());
        state.refresh_pragmas().unwrap();
        let journal = state
            .pragmas
            .iter()
            .position(|(k, _)| k == "journal_mode")
            .unwrap();
        state.pragma_drafts[journal] = "OFF".into();
        state.save_pragmas().unwrap();
        assert_eq!(
            db::read_pragma(state.conn.as_ref().unwrap(), "journal_mode").unwrap(),
            "off"
        );
        state.pragma_drafts[journal] = "MEMORY".into();
        state.save_pragmas().unwrap();
    }

    #[test]
    fn csv_import_resets_old_browse_state_and_plot_rejects_bad_samples() {
        let mut state = test_state();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        state.browse_mut().col_filters = vec!["42".into()];
        state.browse_mut().offset = 500;
        state.browse_mut().order_col = Some("v".into());
        state.overlay = Some(OverlayKind::PasteCsv);
        state.csv_table = "new_csv".into();
        state.csv_paste = "new_col\nhello\n".into();
        state.apply_csv_import().unwrap();
        assert_eq!(state.browse().offset, 0);
        assert_eq!(state.browse().order_col, None);
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][0], "hello");
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch(
                "DELETE FROM t; INSERT INTO t VALUES ('abc'),(42),('1e100'),(NULL),('2.5')",
            )
            .unwrap();
        state.chart_table = Some("t".into());
        state.chart_column = "v".into();
        state.refresh_chart().unwrap();
        assert_eq!(state.chart_points, vec![42.0, 2.5]);
        state.chart_table = Some("missing".into());
        assert!(state.refresh_chart().is_err());
        assert!(state.chart_points.is_empty());
    }

    #[test]
    fn sql_results_and_view_source_belong_to_their_tab() {
        let mut state = test_state();
        *state.sql_editor_mut() = "SELECT 1 AS value;".into();
        state.run_sql_sync().unwrap();
        state.sql_tabs.push("SELECT 2 AS value;".into());
        state.switch_sql_tab(1);
        assert!(state.sql_result.is_none());
        state.run_sql_sync().unwrap();
        state.switch_sql_tab(0);
        assert_eq!(state.sql_result.as_ref().unwrap().rows[0][0], "1");
        *state.sql_editor_mut() = "SELECT 99;".into();
        state.save_results_as_view("saved_result").unwrap();
        assert_eq!(
            state
                .conn
                .as_ref()
                .unwrap()
                .query_row("SELECT * FROM saved_result", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        state.switch_sql_tab(1);
        assert_eq!(state.sql_result.as_ref().unwrap().rows[0][0], "2");
        state.start_sql_text("SELECT 3;\nSELECT 4;".into()).unwrap();
        wait_sql(&mut state);
        assert!(state.sql_log.snapshot(Some(Source::User)).0.iter().any(|e| e.text == "SELECT 3;\nSELECT 4;"));
    }

    #[test]
    fn sql_trace_covers_workers_replaced_connections_errors_and_safe_export() {
        let mut state = test_state();
        let dir = tempfile::tempdir().unwrap();
        let sql = format!("SELECT '{}';\nSELECT v FROM t;", "中文🙂".repeat(60));
        state.run_sql_text_sync(&sql).unwrap();
        state.start_sql_text("SELECT v FROM t WHERE v='saved';".into()).unwrap();
        let busy_output = dir.path().join("busy.log");
        assert!(state.export_sql_log(&busy_output).is_err());
        assert!(!busy_output.exists());
        wait_sql(&mut state);
        assert!(state.sql_log.text(Some(Source::User)).contains("SELECT v FROM t WHERE v='saved';"));
        assert!(state.run_sql_text_sync("SELECT FROM; PRAGMA key='do-not-log';").is_err());
        assert!(!state.sql_log.text(None).contains("do-not-log"));
        assert!(state.sql_log.text(Some(Source::Error)).contains("details redacted"));
        state.run_sql_text_sync("SELECT FROM;").unwrap_err();
        assert!(state.sql_log.text(Some(Source::Error)).contains("syntax error"));
        let database = dir.path().join("saved.db");
        state.save_as(database.to_str().unwrap()).unwrap();
        state.run_sql_text_sync("SELECT 'replacement connection';").unwrap();
        let before = std::fs::read(&database).unwrap();
        assert!(state.export_sql_log(&database).is_err());
        assert_eq!(std::fs::read(&database).unwrap(), before);
        state.sql_log_filter = 0;
        let output = dir.path().join("user.log");
        state.export_sql_log(&output).unwrap();
        let exported = std::fs::read_to_string(&output).unwrap();
        assert!(exported.contains(&sql));
        assert!(exported.contains("replacement connection"));
        assert!(!exported.contains("Application"));
        assert!(!exported.contains("do-not-log"));
        state.close_db();
        assert!(state.sql_log.text(Some(Source::User)).contains(&sql));
    }

    #[test]
    fn sql_and_project_drafts_survive_rollback_and_are_checkpointed_on_save() {
        let mut state = test_state();
        assert!(!state.has_unsaved_drafts());
        *state.sql_editor_mut() = "SELECT 'draft';".into();
        state.revert_changes().unwrap();
        assert!(state.has_unsaved_drafts());
        state.mark_sql_saved();
        assert!(!state.has_unsaved_drafts());
        state.frozen_cols = 2;
        state.sync_prefs_drafts();
        state.prefs_frozen_cols = 8;
        assert_eq!(state.frozen_cols, 2);
        assert!(state.has_unsaved_drafts());
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        state.cond_col = "v".into();
        state.cond_value = "saved".into();
        state.add_cond_format_rule();
        assert!(state.cell_format_colors()[0][0].is_some());
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("CREATE TABLE other(v); INSERT INTO other VALUES('saved')")
            .unwrap();
        *state.browse_mut() = BrowseTabState::new(Some("other".into()));
        state.refresh_browse().unwrap();
        assert!(
            state.cell_format_colors()[0][0].is_none(),
            "Rules must stay with their table"
        );
    }

    #[test]
    fn index_modify_keeps_sort_collation_and_rolls_back_failure() {
        let mut state = test_state();
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("CREATE INDEX ix ON t(v COLLATE NOCASE DESC)")
            .unwrap();
        state.refresh_schema().unwrap();
        state.selected = Some(SelectedObject {
            kind: ObjectKind::Index,
            name: "ix".into(),
        });
        state.open_modify_index_wizard().unwrap();
        assert!(state.wizard_index_cols.contains("NOCASE"));
        assert!(state.wizard_index_cols.contains("DESC"));
        state.apply_index_wizard().unwrap();
        state.open_modify_index_wizard().unwrap();
        state.wizard_index_cols = "missing_column".into();
        assert!(state.apply_index_wizard().is_err());
        assert!(db::list_schema(state.conn.as_ref().unwrap())
            .unwrap()
            .iter()
            .any(|o| o.name == "ix"));
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("CREATE INDEX partial_ix ON t(v) WHERE v IS NOT NULL")
            .unwrap();
        state.refresh_schema().unwrap();
        state.selected = Some(SelectedObject {
            kind: ObjectKind::Index,
            name: "partial_ix".into(),
        });
        assert!(state.open_modify_index_wizard().is_err());
    }

    #[test]
    fn image_preview_preserves_source_resolution_alpha_and_reports_corruption() {
        let mut state = test_state();
        let pixels =
            image::RgbaImage::from_fn(320, 180, |x, y| image::Rgba([x as u8, y as u8, 123, 128]));
        let expected = pixels.as_raw().clone();
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(pixels)
            .write_to(&mut bytes, image::ImageOutputFormat::Png)
            .unwrap();
        state.edit_original = rusqlite::types::Value::Blob(bytes.into_inner());
        let original = state.edit_original.clone();
        state.decode_edit_image();
        assert_eq!(state.edit_image_rgba, Some((320, 180, expected)));
        assert_eq!(state.edit_original, original);
        state.edit_original = rusqlite::types::Value::Blob(b"\x89PNG\r\n\x1a\n".to_vec());
        state.decode_edit_image();
        assert!(state.edit_image_rgba.is_none());
        assert!(state.status_msg.starts_with("Image preview:"));
        // Oversized dimensions are refused before decoding the incomplete pixel stream.
        state.edit_original = rusqlite::types::Value::Blob(vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 16, 1, 0, 0, 16, 0,
            8, 6, 0, 0, 0, 29, 97, 79, 41, 0, 0, 0, 8, 73, 68, 65, 84, 120, 156, 3, 0, 0, 0, 0, 1,
            72, 6, 137, 210, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
        ]);
        state.decode_edit_image();
        assert!(state.edit_image_rgba.is_none());
        assert_eq!(
            state.status_msg,
            "Image preview: Image preview is limited to 16 megapixels"
        );
    }

    #[test]
    fn binary_cell_edit_and_image_use_original_bytes() {
        let mut state = test_state();
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("DELETE FROM t; INSERT INTO t VALUES (X'00FF27');")
            .unwrap();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_col = Some(0);
        state.sync_edit_cell_from_selection();
        assert_eq!(state.edit_value, "00FF27");
        state.edit_value = "ff00".into();
        let value = state.edited_cell_value().unwrap();
        db::update_cell(
            state.conn.as_ref().unwrap(),
            "t",
            state.browse().rowids[0],
            "v",
            &value,
        )
        .unwrap();
        assert_eq!(
            state
                .conn
                .as_ref()
                .unwrap()
                .query_row("SELECT v FROM t", [], |r| r.get::<_, Vec<u8>>(0))
                .unwrap(),
            vec![255, 0]
        );
        state.set_edit_mode(EditCellMode::Image).unwrap();
        assert!(state.edited_cell_value().is_err());
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut bytes, image::ImageOutputFormat::Png)
            .unwrap();
        state.edit_original = rusqlite::types::Value::Blob(bytes.into_inner());
        state.decode_edit_image();
        assert_eq!(
            state.edit_image_rgba.as_ref().map(|(w, h, _)| (*w, *h)),
            Some((2, 2))
        );
    }

    #[test]
    fn save_as_flushes_pending_memory_data() {
        let mut state = test_state();
        state
            .conn
            .as_ref()
            .unwrap()
            .execute("INSERT INTO t VALUES('pending')", [])
            .unwrap();
        state.dirty = true;
        let dir = tempfile::tempdir().unwrap();
        let dst = dir.path().join("saved.db");
        state.save_as(dst.to_str().unwrap()).unwrap();
        assert!(!state.dirty);
        assert_eq!(
            db::count_rows(&Connection::open(dst).unwrap(), "t").unwrap(),
            2
        );
    }

    #[test]
    fn pragmas_apply_outside_transaction_and_refuse_dirty_edits() {
        let mut state = test_state();
        state.refresh_pragmas().unwrap();
        let i = state
            .pragmas
            .iter()
            .position(|(k, _)| k == "foreign_keys")
            .unwrap();
        state.pragma_drafts[i] = "1".into();
        state.save_pragmas().unwrap();
        assert_eq!(
            db::read_pragma(state.conn.as_ref().unwrap(), "foreign_keys").unwrap(),
            "1"
        );
        state.dirty = true;
        assert!(state.save_pragmas().is_err());
    }

    #[test]
    fn async_sql_shares_pending_edits_and_revert() {
        let mut state = test_state();
        state
            .conn
            .as_ref()
            .unwrap()
            .execute("INSERT INTO t VALUES ('pending')", [])
            .unwrap();
        state.dirty = true;
        *state.sql_editor_mut() = "INSERT INTO t VALUES ('worker'); SELECT count(*) FROM t;".into();
        state.start_sql_job().unwrap();
        assert!(state.new_memory_db().is_err());
        wait_sql(&mut state);
        assert_eq!(state.sql_result.as_ref().unwrap().rows[0][0], "3");
        assert!(state.dirty);
        state.revert_changes().unwrap();
        assert_eq!(
            db::count_rows(state.conn.as_ref().unwrap(), "t").unwrap(),
            1
        );
        *state.sql_editor_mut() = "INSERT INTO t VALUES ('committed')".into();
        state.start_sql_job().unwrap();
        wait_sql(&mut state);
        assert!(state.dirty);
        state.write_changes().unwrap();
        state.revert_changes().unwrap();
        assert_eq!(
            db::count_rows(state.conn.as_ref().unwrap(), "t").unwrap(),
            2
        );
    }

    #[test]
    fn sql_dirty_state_tracks_pragmas_attached_ddl_and_transaction_end() {
        let mut state = test_state();
        *state.sql_editor_mut() = "PRAGMA user_version=42".into();
        state.run_sql_sync().unwrap();
        assert!(state.dirty);
        *state.sql_editor_mut() = "ROLLBACK".into();
        state.start_sql_job().unwrap();
        wait_sql(&mut state);
        assert!(!state.dirty);
        assert_eq!(
            db::read_pragma(state.conn.as_ref().unwrap(), "user_version").unwrap(),
            "0"
        );
        *state.sql_editor_mut() = "ATTACH ':memory:' AS extra; CREATE TABLE extra.t(v)".into();
        state.start_sql_job().unwrap();
        wait_sql(&mut state);
        assert!(state.dirty);
        *state.sql_editor_mut() = "COMMIT".into();
        state.run_sql_sync().unwrap();
        assert!(!state.dirty);
        assert!(state
            .run_sql_text_sync("INSERT INTO t VALUES ('pending'); SELECT * FROM missing;")
            .is_err());
        assert!(
            state.dirty,
            "a failed batch still has pending successful statements"
        );
        state.revert_changes().unwrap();
        assert_eq!(
            db::count_rows(state.conn.as_ref().unwrap(), "t").unwrap(),
            1
        );
    }

    #[test]
    fn browse_pages_preserve_row_identity_and_clear_stale_edits() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("DELETE FROM t; WITH RECURSIVE c(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM c WHERE x<601) INSERT INTO t SELECT x FROM c;").unwrap();
        state.browse_mut().table = Some("t".into());
        state.browse_mut().order_col = Some("v".into());
        state.refresh_browse().unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows.len(), 500);
        assert_eq!(state.browse().total_rows, Some(601));
        state.browse_mut().sel_row = Some(0);
        state.sync_edit_cell_from_selection();
        state.browse_page(true).unwrap();
        let tab = state.browse();
        assert_eq!(tab.result.as_ref().unwrap().rows.len(), 101);
        assert_eq!(tab.result.as_ref().unwrap().rows[0][0], "501");
        assert_eq!(tab.rowids[0], 501);
        assert_eq!(tab.total_rows, Some(601));
        assert!(tab.sel_row.is_none());
        assert!(state.edit_col.is_empty());
        state.browse_page(true).unwrap();
        assert_eq!(state.browse().offset, 500);
        state.browse_page(true).unwrap();
        assert_eq!(state.browse().offset, 500);
        state.browse_page(false).unwrap();
        assert_eq!(state.browse().offset, 0);
    }

    #[test]
    fn browse_tsv_paste_updates_a_matrix_atomically() {
        let mut state = test_state();
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch(
                "DROP TABLE t; CREATE TABLE t(a INTEGER, b TEXT); INSERT INTO t VALUES (1, 'one'), (2, 'two');",
            )
            .unwrap();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_col = Some(0);
        state.paste_browse_tsv("7\tseven\n8\teight").unwrap();
        {
            let conn = state.conn.as_ref().unwrap();
            assert_eq!(
                conn.query_row("SELECT a, b FROM t ORDER BY rowid", [], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                })
                .unwrap(),
                (7, "seven".into())
            );
            assert_eq!(
                conn.query_row("SELECT a, b FROM t WHERE rowid=2", [], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                })
                .unwrap(),
                (8, "eight".into())
            );
        }
        assert!(state.dirty);
        assert!(state.paste_browse_tsv("too\tmany\tcolumns").is_err());
        assert_eq!(
            state
                .conn
                .as_ref()
                .unwrap()
                .query_row("SELECT a FROM t WHERE rowid=1", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            7
        );
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_col = Some(1);
        state.browse_mut().sel_col_start = Some(1);
        state.browse_mut().sel_col_end = Some(1);
        state.paste_browse_tsv("changed").unwrap();
        assert_eq!(
            state
                .conn
                .as_ref()
                .unwrap()
                .query_row("SELECT b FROM t WHERE rowid=1", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "changed"
        );
    }

    #[test]
    fn new_record_form_supports_required_values_defaults_null_and_revert() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(id INTEGER PRIMARY KEY, name TEXT NOT NULL, note TEXT DEFAULT 'default'); COMMIT; BEGIN;").unwrap();
        state.browse_mut().table = Some("records".into());
        state.open_insert_row().unwrap();
        assert!(state.apply_insert_row().is_err());
        state.insert_modes[1] = 2;
        state.insert_values[1] = "中文 ' name".into();
        state.apply_insert_row().unwrap();
        assert!(state.dirty);
        assert_eq!(
            state
                .conn
                .as_ref()
                .unwrap()
                .query_row("SELECT note FROM records", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "default"
        );
        state.open_insert_row().unwrap();
        state.insert_modes[1] = 2;
        state.insert_modes[2] = 1;
        state.apply_insert_row().unwrap();
        assert_eq!(
            state
                .conn
                .as_ref()
                .unwrap()
                .query_row(
                    "SELECT count(*) FROM records WHERE note IS NULL AND name=''",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        state.revert_changes().unwrap();
        assert_eq!(
            db::count_rows(state.conn.as_ref().unwrap(), "records").unwrap(),
            0
        );
    }

    #[test]
    fn project_resolves_relative_database_and_requires_memory_save_as() {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("data.db");
        Connection::open(&database)
            .unwrap()
            .execute_batch("CREATE TABLE t(v)")
            .unwrap();
        let path = dir.path().join("project.sqbpro");
        crate::project::ProjectFile {
            db_path: "data.db".into(),
            browse_tabs: vec![
                BrowseTabState::new(Some("t".into())),
                BrowseTabState::new(Some("t".into())),
            ],
            browse_tab_idx: 1,
            ..Default::default()
        }
        .save(&path)
        .unwrap();
        let mut state = test_state();
        assert!(state
            .save_project(&dir.path().join("memory.sqbpro"))
            .is_err());
        state.open_project(&path).unwrap();
        assert_eq!(state.browse_tabs.len(), 2);
        assert_eq!(state.browse_tab_idx, 1);
        assert_eq!(state.path, database.display().to_string());
        state.save_project(&path).unwrap();
        assert!(
            Path::new(&crate::project::ProjectFile::load(&path).unwrap().db_path).is_absolute()
        );
    }

    #[test]
    fn db4s_xml_project_restores_relative_sql_readonly_and_connection_settings() {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("data.db");
        let conn = db::open_path(database.to_str().unwrap()).unwrap();
        conn.execute_batch("CREATE TABLE records(id INTEGER, name TEXT); INSERT INTO records VALUES(1,'one')").unwrap();
        drop(conn);
        std::fs::write(dir.path().join("query.sql"), "SELECT '中文' AS value;").unwrap();
        let path = dir.path().join("project.sqbpro");
        std::fs::write(&path, r#"<sqlb_project><db path="data.db" readonly="1" foreign_keys="1" temp_store="2" synchronous="1"/>
<tab_browse><table title="records" table="4,7:mainrecords"/><browse_table_settings><table schema="main" name="records"><sort><column index="1" mode="1"/></sort><column_widths><column index="1" value="220"/></column_widths></table></browse_table_settings></tab_browse>
<tab_sql><sql name="file" filename="query.sql">-- reference --</sql><sql name="draft">SELECT 42;</sql><current_tab id="1"/></tab_sql></sqlb_project>"#).unwrap();
        let mut state = test_state();
        state.open_project(&path).unwrap();
        assert!(state.read_only);
        assert_eq!(state.sql_tabs, ["SELECT '中文' AS value;", "SELECT 42;"]);
        assert_eq!(state.sql_tab_idx, 1);
        assert_eq!(state.browse().order_col.as_deref(), Some("name"));
        assert_eq!(state.browse().column_widths[1], 220.0);
        assert_eq!(db::read_pragma(state.conn.as_ref().unwrap(), "foreign_keys").unwrap(), "1");
        assert!(!state.has_unsaved_drafts());
        state.save_project(&path).unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().starts_with("<?xml"));
        assert!(!state.has_unsaved_drafts());
        std::fs::write(dir.path().join("query.sql"), "SELECT 'external';").unwrap();
        state.open_project(&path).unwrap();
        assert_eq!(state.sql_tabs[0], "SELECT 'external';");
        state.close_project();
        assert!(state.conn.is_none() && state.project_xml.is_none());
    }

    #[test]
    fn invalid_xml_project_keeps_current_database_tabs_and_connection() {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("data.db");
        let conn = db::open_path(database.to_str().unwrap()).unwrap();
        conn.execute_batch("CREATE TABLE records(id)").unwrap();
        drop(conn);
        let mut state = test_state();
        let old_tabs = state.sql_tabs.clone();
        let old_path = state.path.clone();
        for source in [
            "<sqlb_project><db path='data.db'/><tab_sql><sql filename='missing.sql'/></tab_sql></sqlb_project>",
            "<sqlb_project><db path='data.db' foreign_keys='invalid'/></sqlb_project>",
            "<sqlb_project><db path='data.db'/><attached><db schema='extra' path='missing.db'/></attached></sqlb_project>",
            "<sqlb_project><db path='data.db'/><tab_browse><table table='4,7:mainrecords'/><browse_table_settings><table schema='main' name='records'><column_widths><column index='0' value='NaN'/></column_widths></table></browse_table_settings></tab_browse></sqlb_project>",
            "<sqlb_project><db path='data.db'/><tab_browse><table table='4,7:mainrecords'/><browse_table_settings><table schema='main' name='records'><filter_values><column index='0' value='/[/'/></filter_values></table></browse_table_settings></tab_browse></sqlb_project>",
        ] {
            let path = dir.path().join("invalid.sqbpro");
            std::fs::write(&path, source).unwrap();
            assert!(state.open_project(&path).is_err(), "{source}");
            assert_eq!(state.path, old_path);
            assert_eq!(state.sql_tabs, old_tabs);
            assert!(state.project_path.is_none());
            assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT v FROM t", [], |row| row.get::<_, String>(0)).unwrap(), "saved");
        }
    }

    #[test]
    fn invalid_project_browse_queries_preserve_current_session() {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("data.db");
        let conn = db::open_path(database.to_str().unwrap()).unwrap();
        conn.execute_batch("CREATE TABLE records(id); INSERT INTO records VALUES ('value')").unwrap();
        let mut state = test_state();
        let old_tabs = state.sql_tabs.clone();
        for (raw, column, global) in [
            ("id =", "", ""),
            ("1); DELETE FROM records; --", "", ""),
            ("", "/[/", ""),
            ("", "", "/[/"),
            ("", "/^(a+)+$/", ""),
        ] {
            conn.execute("UPDATE records SET id=?1", [format!("{}!", "a".repeat(100))]).unwrap();
            for inactive in [false, true] {
                let mut tab = BrowseTabState::new(Some("records".into()));
                tab.filter = raw.into();
                tab.col_filters = vec![column.into()];
                tab.global_filters = vec![global.into()];
                let project = crate::project::ProjectFile {
                    db_path: database.display().to_string(),
                    browse_tabs: if inactive { vec![BrowseTabState::new(None), tab] } else { vec![tab] },
                    ..Default::default()
                };
                let path = dir.path().join("invalid.json");
                project.save(&path).unwrap();
                assert!(state.open_project(&path).is_err(), "{raw:?} {column:?} {global:?} inactive={inactive}");
                assert_eq!(state.path, ":memory:");
                assert_eq!(state.sql_tabs, old_tabs);
                assert!(state.project_path.is_none());
                assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT v FROM t", [], |r| r.get::<_, String>(0)).unwrap(), "saved");
                assert_eq!(db::count_rows(&conn, "records").unwrap(), 1);
            }
        }
        conn.execute("DELETE FROM records", []).unwrap();
        let mut project = crate::project::ProjectFile {
            db_path: database.display().to_string(),
            browse_table: Some("records".into()),
            browse_filter: "id =".into(),
            ..Default::default()
        };
        let path = dir.path().join("legacy.json");
        project.save(&path).unwrap();
        assert!(state.open_project(&path).is_err());
        project.browse_filter = String::new();
        project.save(&path).unwrap();
        state.open_project(&path).unwrap();
        assert!(state.browse().result.as_ref().unwrap().rows.is_empty());
    }

    #[test]
    fn execute_line_uses_caret_and_preserves_entire_editor() {
        let mut state = test_state();
        let sql = "SELECT '中文';\nSELECT 42;\nSELECT 99;";
        *state.sql_editor_mut() = sql.into();
        state
            .execute_current_line("SELECT '中文';\nSELECT".chars().count())
            .unwrap();
        wait_sql(&mut state);
        assert_eq!(state.sql_editor(), sql);
        assert_eq!(state.sql_result.as_ref().unwrap().rows[0][0], "42");
    }

    #[test]
    fn sql_stop_recovers_live_connection() {
        let mut state = test_state();
        *state.sql_editor_mut() =
            "WITH RECURSIVE c(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM c) SELECT sum(x) FROM c"
                .into();
        state.start_sql_job().unwrap();
        state.request_sql_stop();
        wait_sql(&mut state);
        assert!(state.error_msg.is_some());
        assert!(state.conn.is_some());
        assert_eq!(
            db::count_rows(state.conn.as_ref().unwrap(), "t").unwrap(),
            1
        );
    }

    #[test]
    fn dirty_database_cannot_be_replaced_and_bad_open_keeps_connection() {
        let mut state = test_state();
        state.dirty = true;
        assert!(state.new_memory_db().is_err());
        state.dirty = false;
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.db");
        std::fs::write(&bad, b"not a database").unwrap();
        state.open_path(bad.to_str().unwrap()).unwrap();
        assert!(state.apply_cipher_overlay().is_err());
        assert_eq!(
            db::count_rows(state.conn.as_ref().unwrap(), "t").unwrap(),
            1
        );
    }

    #[test]
    fn pick_chart_prefers_products_price() {
        let state = AppState::new_with_demo();
        assert_eq!(state.chart_table.as_deref(), Some("products"));
        assert_eq!(state.chart_column, "price");
        assert!(!state.chart_points.is_empty());
        assert!(state.chart_points.iter().any(|v| *v > 10.0));
        let _ = db::default_demo_path();
    }

    #[test]
    fn main_tab_count_is_four() {
        assert_eq!(MainTab::ExecuteSql.index(), 3);
        assert_eq!(MainTab::from_index(4), MainTab::Structure);
    }

    #[test]
    fn create_table_ddl_smoke() {
        let cols = vec![
            ColDef {
                name: "id".into(),
                type_name: "INTEGER".into(),
                pk: true,
                notnull: false,
                unique: false,
                default_value: String::new(),
            },
            ColDef::new("name", "TEXT"),
        ];
        let ddl = db::create_table_ddl("t", &cols);
        assert!(ddl.contains("CREATE TABLE"));
        assert!(ddl.contains("PRIMARY KEY"));
    }
}
