//! Application state for the DB Browser example (DB4S chrome + Phase 2 wizards).

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rusqlite::{Connection, InterruptHandle};
use crate::sql_log::{LoggedConnection, Source, SqlLog};

use crate::db::{
    self, ColDef, CsvImportOptions, ExportSqlOptions, ObjectKind, QueryResult, SchemaObject,
    COLUMN_TYPES,
};

#[derive(Debug, Clone, Default)]
pub struct ConstraintTableView {
    pub scroll: (f32, f32),
    pub widths: Vec<f32>,
    pub column: Option<usize>,
}

#[derive(Debug,Clone)]
pub struct FieldCellEdit {
    pub column: usize,
    pub original: String,
    pub reference: Option<crate::reference_editor::Draft>,
    pub input: Option<next_gui_ui::TextInputState>,
    pub autocomplete: Option<next_gui_ui::AutocompleteState>,
}

#[derive(Debug, Clone)]
pub enum ConstraintCellOriginal {
    Text(String),
    Index(crate::table_sql::NamedIndexConstraint),
}

#[derive(Debug, Clone)]
pub struct ConstraintCellEdit {
    pub page: usize,
    pub row: usize,
    pub column: usize,
    pub original: ConstraintCellOriginal,
    pub reference: Option<crate::reference_editor::Draft>,
    pub input: Option<next_gui_ui::TextInputState>,
}

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SelectedObject {
    pub schema: String,
    pub kind: ObjectKind,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BrowseTableOptions {
    pub column_names: Vec<String>,
    pub filter: String,
    pub global_filters: Vec<String>,
    pub col_filters: Vec<String>,
    pub ordering: Vec<db::OrderBy>,
    pub column_widths: Vec<f32>,
    pub frozen_cols: Option<usize>,
}

impl BrowseTableOptions {
    fn rename_columns(&mut self, renames: &[(String, String)]) {
        for name in &mut self.column_names { rename_column(name, renames); }
        for order in &mut self.ordering { rename_column(&mut order.column, renames); }
    }

    pub fn remap_columns(&mut self, columns: &[String]) {
        if !self.column_names.is_empty() && self.column_names != columns {
            let positions: Vec<_> = columns.iter().map(|name| self.column_names.iter().position(|old| old.eq_ignore_ascii_case(name))).collect();
            if !self.col_filters.is_empty() {
                self.col_filters = positions.iter().map(|i| i.and_then(|i| self.col_filters.get(i)).cloned().unwrap_or_default()).collect();
            }
            if !self.column_widths.is_empty() {
                self.column_widths = positions.iter().map(|i| i.and_then(|i| self.column_widths.get(i)).copied().unwrap_or(140.0)).collect();
            }
            self.ordering.retain_mut(|order| {
                if let Some(name) = columns.iter().find(|name| name.eq_ignore_ascii_case(&order.column)) { order.column = name.clone(); true }
                else { false }
            });
            if let Some(count) = self.frozen_cols {
                self.frozen_cols = Some(positions.iter().take_while(|i| i.is_some_and(|i| i < count)).count());
            }
        }
        self.column_names = columns.to_vec();
    }
}

fn rename_column(name: &mut String, renames: &[(String, String)]) {
    if let Some((_, target)) = renames.iter().find(|(source, _)| source.eq_ignore_ascii_case(name)) {
        *name = target.clone();
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BrowseTabState {
    pub title: String,
    #[serde(default = "main_schema")]
    pub schema: String,
    pub table: Option<String>,
    #[serde(default)]
    pub column_names: Vec<String>,
    pub filter: String,
    #[serde(default)]
    pub global_filters: Vec<String>,
    /// Parallel to current result columns; empty = no filter.
    pub col_filters: Vec<String>,
    #[serde(default)]
    pub offset: usize,
    /// Primary sort kept in the original fields for legacy JSON projects.
    pub order_col: Option<String>,
    pub order_desc: bool,
    #[serde(default)]
    pub additional_order: Vec<db::OrderBy>,
    #[serde(default)]
    pub column_widths: Vec<f32>,
    #[serde(default)]
    pub frozen_cols: Option<usize>,
    #[serde(default)]
    pub scroll: (f32, f32),
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub table_options: BTreeMap<String, BrowseTableOptions>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub schema_table_options: BTreeMap<String, BTreeMap<String, BrowseTableOptions>>,
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
    #[serde(skip)]
    pub goto_record: String,
}

impl BrowseTabState {
    pub fn stored_options(&self, schema: &str, table: &str) -> Option<&BrowseTableOptions> {
        if schema == "main" { self.table_options.get(table) }
        else { self.schema_table_options.get(schema)?.get(table) }
    }

    pub fn store_options(&mut self, schema: &str, table: &str, options: BrowseTableOptions) {
        if schema == "main" { self.table_options.insert(table.into(), options); }
        else { self.schema_table_options.entry(schema.into()).or_default().insert(table.into(), options); }
    }

    pub fn saved_options(&self) -> impl Iterator<Item = (&str, &str, &BrowseTableOptions)> {
        self.table_options.iter().map(|(table, options)| ("main", table.as_str(), options))
            .chain(self.schema_table_options.iter().flat_map(|(schema, tables)| tables.iter().map(move |(table, options)| (schema.as_str(), table.as_str(), options))))
    }
    pub fn options(&self) -> BrowseTableOptions {
        BrowseTableOptions {
            column_names: if self.column_names.is_empty() { self.result.as_ref().map(|r| r.columns.clone()).unwrap_or_default() } else { self.column_names.clone() },
            filter: self.filter.clone(),
            global_filters: self.global_filters.clone(),
            col_filters: self.col_filters.clone(),
            ordering: self.ordering(),
            column_widths: self.column_widths.clone(),
            frozen_cols: self.frozen_cols,
        }
    }

    pub fn set_options(&mut self, options: BrowseTableOptions) {
        self.column_names = options.column_names;
        self.filter = options.filter;
        self.global_filters = options.global_filters;
        self.col_filters = options.col_filters;
        self.set_ordering(options.ordering);
        self.column_widths = options.column_widths;
        self.frozen_cols = options.frozen_cols;
    }

    pub fn ordering(&self) -> Vec<db::OrderBy> {
        self.order_col.iter().map(|column| db::OrderBy { column: column.clone(), descending: self.order_desc })
            .chain(self.additional_order.iter().cloned()).collect()
    }

    pub fn set_ordering(&mut self, mut ordering: Vec<db::OrderBy>) {
        let first = if ordering.is_empty() { None } else { Some(ordering.remove(0)) };
        self.order_col = first.as_ref().map(|o| o.column.clone());
        self.order_desc = first.is_some_and(|o| o.descending);
        self.additional_order = ordering;
    }

    fn clear_selection(&mut self) {
        self.sel_row = None;
        self.sel_row_start = None;
        self.sel_row_end = None;
        self.sel_col = None;
        self.sel_col_start = None;
        self.sel_col_end = None;
    }

    // shortcut: remap within the refreshed preview; extend with whole-result virtual selection.
    fn restore_selection(&mut self, previous: &Self) {
        let (Some(row), Some(old), Some(new)) = (previous.sel_row, &previous.result, &self.result) else { return; };
        if self.schema != previous.schema || self.table != previous.table || old.columns != new.columns || old.columns.is_empty() { return; }
        let anchor = previous.sel_row_start.unwrap_or(row);
        let end = previous.sel_row_end.unwrap_or(row);
        let first = row.min(anchor).min(end);
        let last = row.max(anchor).max(end);
        if last >= old.rows.len() || [previous.sel_col, previous.sel_col_start, previous.sel_col_end]
            .into_iter().flatten().any(|col| col >= new.columns.len()) { return; }
        let start = if previous.rowids.len() == old.rows.len() && self.rowids.len() == new.rows.len() {
            let Some(start) = self.rowids.iter().position(|id| *id == previous.rowids[first]) else { return; };
            if self.rowids.get(start..start + last - first + 1) != Some(&previous.rowids[first..=last]) { return; }
            start
        } else {
            // Without a stable row key, retain only unchanged typed rows at the same positions.
            if self.offset != previous.offset || last >= new.rows.len() || (first..=last).any(|row| {
                let cols = 0..=old.columns.len() - 1;
                match (old.values.rectangle(row..=row, cols.clone()), new.values.rectangle(row..=row, cols)) {
                    (Ok(old), Ok(new)) => old != new,
                    _ => true,
                }
            }) { return; }
            first
        };
        let mapped = |row| start + row - first;
        self.sel_row = Some(mapped(row));
        self.sel_row_start = previous.sel_row_start.map(mapped);
        self.sel_row_end = previous.sel_row_end.map(mapped);
        self.sel_col = previous.sel_col;
        self.sel_col_start = previous.sel_col_start;
        self.sel_col_end = previous.sel_col_end;
        self.goto_record = self.offset.saturating_add(mapped(row)).saturating_add(1).to_string();
    }

    pub fn new(table: Option<String>) -> Self {
        Self::new_in("main", table)
    }

    pub fn new_in(schema: &str, table: Option<String>) -> Self {
        let title = table.as_ref().map(|name| if schema == "main" { name.clone() } else { db::qualified_name(schema, name) }).unwrap_or_else(|| "Browse".into());
        Self {
            title,
            schema: schema.into(),
            table,
            column_names: Vec::new(),
            filter: String::new(),
            global_filters: Vec::new(),
            col_filters: Vec::new(),
            offset: 0,
            order_col: None,
            order_desc: false,
            additional_order: Vec::new(),
            column_widths: Vec::new(),
            frozen_cols: None,
            scroll: (0.0, 0.0),
            table_options: BTreeMap::new(),
            schema_table_options: BTreeMap::new(),
            result: None,
            rowids: Vec::new(),
            sel_row: None,
            sel_row_start: None,
            sel_row_end: None,
            sel_col: None,
            sel_col_start: None,
            sel_col_end: None,
            total_rows: None,
            goto_record: "1".into(),
        }
    }
}

fn main_schema() -> String { "main".into() }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditCellMode {
    Text = 0,
    Hex = 1,
    Json = 2,
    Image = 3,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CondFormatRule {
    #[serde(default = "main_schema")]
    pub schema: String,
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
    pub schema_names: Vec<String>,
    pub schema_tree_expanded: BTreeMap<String, [bool; 5]>,
    pub schema_tree_scroll: f32,
    pub schema_group: Option<(String, u64)>,
    pub sql_completion: crate::sql_completion::Catalog,
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
    pub wizard_schema: String,
    pub wizard_scroll: (f32, f32),
    pub wizard_field_scroll: (f32, f32),
    pub wizard_field_widths: Vec<f32>,
    pub wizard_field_column: Option<usize>,
    pub wizard_field_edit: Option<FieldCellEdit>,
    pub wizard_constraint_edit: Option<ConstraintCellEdit>,
    pub wizard_column_picker: Option<crate::column_picker::Draft>,
    pub wizard_table_name: String,
    pub wizard_table_options: crate::table_sql::TableOptions,
    pub wizard_table_original: Option<SelectedObject>,
    pub wizard_cols: Vec<ColDef>,
    pub wizard_original_cols: Vec<ColDef>,
    pub wizard_col_sources: Vec<Option<String>>,
    pub wizard_table_preview_state: Option<next_gui_ui::TextAreaState>,
    pub wizard_table_name_state: Option<next_gui_ui::TextInputState>,
    pub wizard_table_page: usize,
    pub wizard_table_ratio: f32,
    pub wizard_table_advanced: bool,
    pub wizard_checks: Vec<crate::table_sql::NamedCheck>,
    pub wizard_index_constraints: Vec<crate::table_sql::NamedIndexConstraint>,
    pub wizard_key_sel: usize,
    pub wizard_key_states: Vec<Option<next_gui_ui::TextAreaState>>,
    pub wizard_key_scroll: (f32, f32),
    pub wizard_constraint_views: [ConstraintTableView; 3],
    pub wizard_foreign_keys: Vec<crate::table_sql::NamedForeignKey>,
    pub wizard_foreign_key_sel: usize,
    pub wizard_foreign_key_states: Vec<[Option<next_gui_ui::TextAreaState>; 2]>,
    pub wizard_foreign_key_scroll: (f32, f32),
    pub wizard_constraint_columns: Vec<String>,
    pub wizard_constraint_table: String,
    pub wizard_check_sel: usize,
    pub wizard_check_states: Vec<Option<next_gui_ui::TextAreaState>>,
    pub wizard_check_scroll: (f32, f32),
    pub wizard_column_edit_states: Vec<[Option<next_gui_ui::TextAreaState>; 3]>,
    pub wizard_col_sel: usize,
    pub wizard_index_name: String,
    pub wizard_index_table: String,
    pub wizard_index_cols: String,
    pub wizard_index_where: String,
    pub wizard_index_edit_states: [Option<next_gui_ui::TextAreaState>; 3],
    pub wizard_index_list_scroll: [f32; 2],
    pub wizard_index_key_sel: usize,
    pub wizard_index_available_sel: usize,
    pub wizard_index_unique: bool,
    pub wizard_index_original: Option<SelectedObject>,

    // CSV import wizard
    pub csv_work: Option<crate::csv_files::Work>,
    pub csv_files: Option<crate::csv_files::Batch>,
    pub csv_scroll: (f32, f32),
    pub csv_advanced: bool,
    pub csv_custom_delimiter: String,
    pub csv_custom_quote: String,
    pub csv_import_settings: CsvImportOptions,
    pub csv_preview: Option<Result<crate::csv_import::Data, String>>,
    pub csv_path: String,
    pub csv_table: String,
    pub csv_has_header: bool,
    pub csv_delim_idx: usize, // 0=, 1=; 2=tab 3=|
    pub csv_quote_idx: usize, // 0=" 1=' 2=none
    pub csv_encoding_idx: usize,
    pub csv_custom_encoding: String,
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
    pub chart_schema: String,
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
    pub export_dialog: Option<crate::data_export::Draft>,
    pub export_structure: bool,
    pub export_data: bool,
    pub export_drop: bool,
    pub export_keep_fk: bool,
    pub prefs_font_size: String,
    pub prefs_frozen_cols: usize,
    pub prefs_theme_dark: bool,
    pub prefs_confirm_drops: bool,
    pub prefs_sql_completion: bool,
    pub prefs_sql_upper_keywords: bool,
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
    #[cfg(test)]
    _demo_directory: Option<tempfile::TempDir>,
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
    DeleteColumn(usize),
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
    SelectSchemaNode(u64),
    Exit,
}

impl AppState {
    pub fn constraint_cell_value(&self, page: usize, row: usize, column: usize) -> Option<&str> {
        match (page, column) {
            (0, 1) => Some(if self.wizard_index_constraints.get(row)?.primary {"PRIMARY KEY"} else {"UNIQUE"}),
            (0, 2) => Some(self.wizard_index_constraints.get(row)?.name.as_deref().unwrap_or("")),
            (1, 1) => Some(self.wizard_foreign_keys.get(row)?.name.as_deref().unwrap_or("")),
            (1, 2) => Some(&self.wizard_foreign_keys.get(row)?.clause),
            (2, 0) => Some(&self.wizard_checks.get(row)?.expression),
            (2, 1) => Some(self.wizard_checks.get(row)?.name.as_deref().unwrap_or("")),
            _ => None,
        }
    }

    pub fn new_with_demo() -> Self {
        #[cfg(not(test))]
        let demo = db::default_demo_path();
        #[cfg(test)]
        let demo_directory = tempfile::tempdir().expect("create isolated demo fixture");
        #[cfg(test)]
        let demo = demo_directory.path().join("sample.db");
        let mut state = Self::empty();
        #[cfg(test)]
        { state._demo_directory = Some(demo_directory); }
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
                    .find(|o| o.schema == "main" && o.kind == ObjectKind::Table)
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
            schema_names: Vec::new(),
            schema_tree_expanded: BTreeMap::new(),
            schema_tree_scroll: 0.0,
            schema_group: None,
            sql_completion: crate::sql_completion::Catalog::default(),
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
            wizard_schema: "main".into(),
            wizard_scroll: (0.0, 0.0),
            wizard_field_scroll: (0.0, 0.0),
            wizard_field_widths: Vec::new(),
            wizard_field_column: None,
            wizard_field_edit: None,
            wizard_constraint_edit: None,
            wizard_column_picker: None,
            wizard_table_options: crate::table_sql::TableOptions::default(),
            wizard_table_name: "new_table".into(),
            wizard_table_original: None,
            wizard_cols: vec![
                ColDef {
                    name: "id".into(),
                    type_name: "INTEGER".into(),
                    pk: true,
                    auto_increment: false,
                    collation: String::new(),
                    check_expression: String::new(), foreign_key_clause: String::new(), generated: None,
                    notnull: false,
                    unique: false,
                    default_value: String::new(),
                },
                ColDef::new("name", "TEXT"),
            ],
            wizard_original_cols: Vec::new(),
            wizard_col_sources: Vec::new(),
            wizard_table_preview_state: None,
            wizard_table_name_state: None,
            wizard_table_page: 0,
            wizard_table_ratio: 0.72,
            wizard_table_advanced: false,
            wizard_checks: Vec::new(),
            wizard_index_constraints: Vec::new(),
            wizard_key_sel: 0,
            wizard_key_states: Vec::new(),
            wizard_key_scroll: (0.0, 0.0),
            wizard_constraint_views: Default::default(),
            wizard_foreign_keys: Vec::new(),
            wizard_foreign_key_sel: 0,
            wizard_foreign_key_states: Vec::new(),
            wizard_foreign_key_scroll: (0.0, 0.0),
            wizard_constraint_columns: Vec::new(),
            wizard_constraint_table: String::new(),
            wizard_check_sel: 0,
            wizard_check_states: Vec::new(),
            wizard_check_scroll: (0.0, 0.0),
            wizard_column_edit_states: Vec::new(),
            wizard_col_sel: 0,
            wizard_index_name: "idx_name".into(),
            wizard_index_table: String::new(),
            wizard_index_cols: "name".into(),
            wizard_index_where: String::new(),
            wizard_index_edit_states: [None, None, None],
            wizard_index_list_scroll: [0.0; 2],
            wizard_index_key_sel: 0,
            wizard_index_available_sel: 0,
            wizard_index_unique: false,
            wizard_index_original: None,
            csv_work: None,
            csv_files: None,
            csv_scroll: (0.0, 0.0),
            csv_advanced: false,
            csv_custom_delimiter: String::new(),
            csv_custom_quote: String::new(),
            csv_import_settings: CsvImportOptions::default(),
            csv_preview: None,
            csv_path: String::new(),
            csv_table: "imported".into(),
            csv_has_header: true,
            csv_delim_idx: 0,
            csv_quote_idx: 0,
            csv_encoding_idx: 0,
            csv_custom_encoding: String::new(),
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
            chart_schema: "main".into(),
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
            export_dialog: None,
            export_structure: true,
            export_data: true,
            export_drop: false,
            export_keep_fk: true,
            prefs_frozen_cols: 0,
            prefs_font_size: "13".into(),
            prefs_theme_dark: true,
            prefs_confirm_drops: true,
            prefs_sql_completion: true,
            prefs_sql_upper_keywords: true,
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
            #[cfg(test)]
            _demo_directory: None,
        };
        #[cfg(not(test))]
        state.load_csv_import_settings();
        state.checkpoint_project();
        state
    }

    pub fn browse(&self) -> &BrowseTabState {
        self.browse_tabs
            .get(self.browse_tab_idx)
            .or_else(|| self.browse_tabs.first())
            .expect("browse tabs")
    }

    pub fn browse_frozen_cols(&self) -> usize {
        let tab = self.browse();
        let count = tab.frozen_cols.unwrap_or(self.frozen_cols);
        tab.result.as_ref().map_or(count, |result| count.min(result.columns.len()))
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
        self.guard_csv_idle()?;
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
        self.guard_csv_idle()?;
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
            self.schema_names.clear();
            self.sql_completion = crate::sql_completion::Catalog::default();
            self.database_encoding.clear();
            return Ok(());
        };
        let schema_names = db::database_names(conn)?;
        let mut schema = Vec::new();
        for name in &schema_names { schema.extend(db::list_schema_in(conn, name)?); }
        let encoding = db::read_pragma(conn, "main.encoding")?;
        self.schema = schema;
        self.schema_names = schema_names;
        self.sql_completion = crate::sql_completion::Catalog::load(conn);
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

    #[cfg(test)]
    pub fn export_browse(&self, path: &Path, json: bool) -> Result<(), String> {
        let schema = self.browse().schema.clone();
        let conn = self.conn.as_ref().ok_or("No database open")?;
        let tab = self.browse();
        let table = tab.table.as_ref().ok_or("Select a table to export")?;
        let columns = db::table_column_names_in(conn, &schema, table)?;
        let mut options = tab.options();
        options.remap_columns(&columns);
        let mut candidate = tab.clone();
        candidate.set_options(options);
        db::export_table_in(
            conn,
            &schema,
            table,
            &candidate.ordering(),
            Self::composed_where(&candidate, &columns)?.as_deref(),
            path,
            json,
        )
    }

    pub fn refresh_browse(&mut self) -> Result<(), String> {
        let schema = self.browse().schema.clone();
        if self.has_cell_draft() { return Err("Apply or discard the cell draft first".into()); }
        if self.conn.is_none() {
            if let Some(t) = self.browse_tabs.get_mut(self.browse_tab_idx) {
                t.result = None;
                t.rowids.clear();
                t.total_rows = None;
                t.clear_selection();
            }
            self.clear_browse_editor();
            return Ok(());
        }
        let table = self.browse().table.clone();
        let Some(table) = table else {
            let t = self.browse_mut();
            t.result = None;
            t.rowids.clear();
            t.total_rows = None;
            t.clear_selection();
            self.clear_browse_editor();
            return Ok(());
        };
        let columns = db::table_column_names_in(self.conn.as_ref().unwrap(), &schema, &table)?;
        let mut options = self.browse().options();
        options.remap_columns(&columns);
        let mut candidate = self.browse().clone();
        candidate.set_options(options);
        let ordering = candidate.ordering();
        let is_view = self
            .schema
            .iter()
            .any(|o| o.schema == schema && o.name == table && o.kind == ObjectKind::View);

        let where_sql = Self::composed_where(&candidate, &columns)?;

        let total_rows = db::count_browse_rows_in(self.conn.as_ref().unwrap(), &schema, &table, where_sql.as_deref())?;
        let last_page = total_rows.saturating_sub(1) / db::DEFAULT_LIMIT * db::DEFAULT_LIMIT;
        let offset = self.browse().offset.min(last_page);
        let (total_rows, result, rowids) = {
            let conn = self.conn.as_ref().unwrap();
            if is_view {
                (
                    total_rows,
                    db::browse_table_in(
                        conn,
                        &schema,
                        &table,
                        &ordering,
                        where_sql.as_deref(),
                        db::DEFAULT_LIMIT,
                        offset,
                    )?,
                    Vec::new(),
                )
            } else {
                match db::browse_with_rowid_in(
                    conn,
                    &schema,
                    &table,
                    &ordering,
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
                            q.values.remove_column(idx);
                        }
                        (total_rows, q, rowids)
                    }
                    Err(_) => (
                        total_rows,
                        db::browse_table_in(
                            conn,
                            &schema,
                            &table,
                            &ordering,
                            where_sql.as_deref(),
                            db::DEFAULT_LIMIT,
                            offset,
                        )?,
                        Vec::new(),
                    ),
                }
            }
        };

        candidate.col_filters.resize(result.columns.len(), String::new());
        let nrows = result.rows.len();
        let truncated = result.truncated;
        let order_note = if ordering.is_empty() { String::new() } else {
            format!(" ORDER BY {}", ordering.iter().map(|order| format!("{} {}", db::quote_ident(&order.column), if order.descending { "DESC" } else { "ASC" })).collect::<Vec<_>>().join(", "))
        };
        {
            let t = self.browse_mut();
            t.set_options(candidate.options());
            t.offset = offset;
            t.title = if schema == "main" { table.clone() } else { db::qualified_name(&schema, &table) };
            t.rowids = rowids;
            t.total_rows = Some(total_rows);
            t.result = Some(result);
            t.clear_selection();
        }
        self.clear_browse_editor();
        self.status_msg = format!(
            "Browse {} — {} rows {}–{} of {}{}{}",
            if schema == "main" { table.clone() } else { db::qualified_name(&schema, &table) },
            nrows,
            if nrows == 0 { 0 } else { offset.saturating_add(1) },
            offset.saturating_add(nrows),
            total_rows,
            if truncated { " (more pages)" } else { "" },
            order_note
        );
        Ok(())
    }

    fn clear_browse_editor(&mut self) {
        self.edit_col.clear();
        self.edit_value.clear();
        self.edit_type = 0;
        self.edit_original = rusqlite::types::Value::Null;
        self.edit_saved = rusqlite::types::Value::Null;
        self.edit_image_rgba = None;
    }

    pub fn sort_browse_column(&mut self, column: String, append: bool) -> Result<(), String> {
        let column_index = self.browse().result.as_ref().and_then(|r| r.columns.iter().position(|c| *c == column))
            .ok_or("Select a valid sort column")?;
        let mut ordering = self.browse().ordering();
        if append {
            if let Some(order) = ordering.iter_mut().find(|o| o.column == column) {
                order.descending = !order.descending;
            } else { ordering.push(db::OrderBy { column, descending: false }); }
        } else if ordering.len() == 1 && ordering[0].column == column {
            ordering[0].descending = !ordering[0].descending;
        } else { ordering = vec![db::OrderBy { column, descending: false }]; }
        self.set_browse_ordering(ordering)?;
        if self.browse().result.as_ref().is_some_and(|r| !r.rows.is_empty()) {
            self.browse_mut().sel_row = Some(0);
            self.browse_mut().sel_col = Some(column_index);
        }
        self.sync_edit_cell_from_selection();
        Ok(())
    }

    pub fn set_browse_ordering(&mut self, ordering: Vec<db::OrderBy>) -> Result<(), String> {
        if self.conn.is_none() { return Err("No database open".into()); }
        if self.has_cell_draft() { return Err("Apply or discard the cell draft first".into()); }
        if self.sql_busy { return Err("Wait for SQL to finish before sorting".into()); }
        let previous = self.browse().clone();
        self.browse_mut().set_ordering(ordering);
        self.browse_mut().offset = 0;
        if let Err(error) = self.refresh_browse() {
            *self.browse_mut() = previous;
            return Err(error);
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn switch_browse_table(&mut self, name: &str) -> Result<(), String> {
        self.switch_browse_table_in("main", name)
    }

    pub fn switch_browse_table_in(&mut self, schema: &str, name: &str) -> Result<(), String> {
        if self.conn.is_none() { return Err("No database open".into()); }
        if self.has_cell_draft() { return Err("Apply or discard the cell draft first".into()); }
        if self.sql_busy { return Err("Wait for SQL to finish before switching tables".into()); }
        if !self.schema.iter().any(|o| o.schema == schema && o.name == name && matches!(o.kind, ObjectKind::Table | ObjectKind::View)) { return Err("Select a valid table or view".into()); }
        if self.browse().schema == schema && self.browse().table.as_deref() == Some(name) { return Ok(()); }
        let previous = self.browse().clone();
        let mut next = BrowseTabState::new_in(schema, Some(name.into()));
        next.table_options = previous.table_options.clone();
        next.schema_table_options = previous.schema_table_options.clone();
        if let Some(table) = &previous.table {
            next.store_options(&previous.schema, table, previous.options());
        }
        if let Some(options) = next.stored_options(schema, name).cloned() {
            if options.column_widths.iter().any(|width| !width.is_finite() || *width <= 0.0) {
                return Err("Invalid saved table column width".into());
            }
            next.set_options(options);
        }
        *self.browse_mut() = next;
        if let Err(error) = self.refresh_browse() {
            *self.browse_mut() = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Jump to a one-based record in the complete filtered/sorted result.
    pub fn goto_browse_record(&mut self, record: usize) -> Result<(), String> {
        if self.conn.is_none() { return Err("No database open".into()); }
        if self.browse().table.is_none() { return Err("Select a table first".into()); }
        if self.has_cell_draft() { return Err("Apply or discard the cell draft first".into()); }
        if self.sql_busy { return Err("Wait for SQL to finish before navigating".into()); }
        let previous = self.browse().clone();
        let target = record.saturating_sub(1);
        self.browse_mut().offset = target / db::DEFAULT_LIMIT * db::DEFAULT_LIMIT;
        if let Err(error) = self.refresh_browse() {
            *self.browse_mut() = previous;
            return Err(error);
        }
        let tab = self.browse_mut();
        if let Some(result) = tab.result.as_ref().filter(|r| !r.rows.is_empty()) {
            let row = target.saturating_sub(tab.offset).min(result.rows.len() - 1);
            tab.sel_row = Some(row);
            tab.sel_col = (!result.columns.is_empty()).then(|| previous.sel_col.unwrap_or(0).min(result.columns.len() - 1));
            tab.goto_record = (tab.offset + row + 1).to_string();
        } else { tab.goto_record = "0".into(); }
        self.sync_edit_cell_from_selection();
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
        self.chart_points = db::numeric_series_in(conn, &self.chart_schema, &table, self.chart_column.trim(), 64)?;
        Ok(())
    }

    pub fn schema_selection_has_draft(&self, node_id: u64) -> bool {
        let obj = node_id.checked_sub(10_000).and_then(|i| self.schema.get(i as usize));
        let group = node_id > 0 && node_id <= self.schema_names.len() as u64 * 5;
        if obj.is_none() && !group { return false; }
        if obj.is_some_and(|obj| self.selected.as_ref().is_some_and(|s| s.schema == obj.schema && s.kind == obj.kind && s.name == obj.name)) { return false; }
        let source = self.selected.as_ref().and_then(|s| self.schema.iter().find(|o| o.schema == s.schema && o.kind == s.kind && o.name == s.name)).and_then(|o| o.sql.as_deref()).unwrap_or_default();
        self.ddl_draft != source
    }

    pub fn select_schema_node(&mut self, node_id: u64) {
        if node_id < 10_000 {
            if node_id > 0 && node_id <= self.schema_names.len() as u64 * 5 {
                let group = (self.schema_names[((node_id - 1) / 5) as usize].clone(), (node_id - 1) % 5);
                if self.selected.is_none() && self.schema_group.as_ref() == Some(&group) { return; }
                self.schema_group = Some(group);
                self.selected = None;
                self.ddl_draft.clear();
            }
            return;
        }
        let idx = (node_id - 10_000) as usize;
        if let Some(obj) = self.schema.get(idx).cloned() {
            if self.selected.as_ref().is_some_and(|s| s.schema == obj.schema && s.kind == obj.kind && s.name == obj.name) { return; }
            self.selected = Some(SelectedObject {
                schema: obj.schema.clone(),
                kind: obj.kind,
                name: obj.name.clone(),
            });
            self.schema_group = None;
            self.ddl_draft = obj.sql.clone().unwrap_or_default();
            if matches!(obj.kind, ObjectKind::Table | ObjectKind::View) {
                if let Err(error) = self.switch_browse_table_in(&obj.schema, &obj.name) { self.error_msg = Some(error); }
            }
        }
    }

    pub fn open_insert_row(&mut self) -> Result<(), String> {
        let schema = self.browse().schema.clone();
        self.guard_writable()?;
        let table = self.browse().table.as_ref().ok_or("Select a table")?;
        self.insert_columns = db::table_columns_in(self.conn.as_ref().unwrap(), &schema, table)?;
        self.insert_values = vec![String::new(); self.insert_columns.len()];
        self.insert_modes = vec![0; self.insert_columns.len()];
        self.overlay = Some(OverlayKind::InsertRow);
        Ok(())
    }

    pub fn apply_insert_row(&mut self) -> Result<(), String> {
        let schema = self.browse().schema.clone();
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
            format!("INSERT INTO {} DEFAULT VALUES", db::qualified_name(&schema, table))
        } else {
            format!(
                "INSERT INTO {} ({}) VALUES ({})",
                db::qualified_name(&schema, table),
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
        self.wizard_schema = "main".into();
        self.wizard_scroll = (0.0, 0.0);
        self.wizard_field_scroll = (0.0, 0.0);
        self.wizard_field_widths.clear();
        self.wizard_field_column = None;
        self.wizard_field_edit = None;
        self.wizard_constraint_edit = None;
        self.wizard_column_picker = None;
        self.wizard_table_name = "new_table".into();
        self.wizard_table_original = None;
        self.wizard_table_options = crate::table_sql::TableOptions::default();
        self.wizard_cols = vec![
            ColDef {
                name: "id".into(),
                type_name: "INTEGER".into(),
                pk: true,
                auto_increment: false,
                collation: String::new(),
                check_expression: String::new(), foreign_key_clause: String::new(), generated: None,
                notnull: false,
                unique: false,
                default_value: String::new(),
            },
            ColDef::new("name", "TEXT"),
        ];
        self.wizard_col_sources.clear();
        self.wizard_original_cols.clear();
        self.wizard_table_preview_state = None;
        self.wizard_table_name_state = None;
        self.wizard_table_page = 0;
        self.wizard_table_ratio = 0.72;
        self.wizard_table_advanced = false;
        self.wizard_index_constraints = crate::table_sql::index_constraints_from_columns(&[], &self.wizard_cols, None).expect("default table key");
        self.wizard_key_sel = 0;
        self.wizard_key_states = vec![None; self.wizard_index_constraints.len()];
        self.wizard_key_scroll = (0.0, 0.0);
        self.wizard_constraint_views = Default::default();
        self.wizard_checks.clear();
        self.wizard_foreign_keys.clear();
        self.wizard_foreign_key_sel = 0;
        self.wizard_foreign_key_states.clear();
        self.wizard_foreign_key_scroll = (0.0, 0.0);
        self.wizard_constraint_columns = self.wizard_cols.iter().map(|column| column.name.clone()).collect();
        self.wizard_constraint_table = self.wizard_table_name.clone();
        self.wizard_check_sel = 0;
        self.wizard_check_states.clear();
        self.wizard_check_scroll = (0.0, 0.0);
        self.wizard_column_edit_states = vec![[None, None, None]; self.wizard_cols.len()];
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
        let cols = db::table_column_defs_in(conn, &sel.schema, &sel.name)?;
        if cols.is_empty() { return Err("Table has no editable columns".into()); }
        self.wizard_col_sources = cols.iter().map(|col| Some(col.name.clone())).collect();
        self.wizard_schema = sel.schema.clone();
        self.wizard_scroll = (0.0, 0.0);
        self.wizard_field_scroll = (0.0, 0.0);
        self.wizard_field_widths.clear();
        self.wizard_field_column = None;
        self.wizard_field_edit = None;
        self.wizard_constraint_edit = None;
        self.wizard_column_picker = None;
        self.wizard_table_name = sel.name.clone();
        self.wizard_table_options = db::table_options_in(conn, &sel.schema, &sel.name)?;
        let sql: String = conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE type='table' AND name=?1", db::quote_ident(&sel.schema)), [&sel.name], |row| row.get(0)).map_err(|error| error.to_string())?;
        self.wizard_index_constraints = crate::table_sql::table_index_constraints(&sql)?;
        self.wizard_key_sel = 0;
        self.wizard_key_states = vec![None; self.wizard_index_constraints.len()];
        self.wizard_key_scroll = (0.0, 0.0);
        self.wizard_constraint_views = Default::default();
        self.wizard_checks = crate::table_sql::table_checks(&sql)?;
        self.wizard_checks.extend(crate::table_sql::inline_checks(&sql)?);
        self.wizard_foreign_keys = crate::table_sql::inline_foreign_keys(&sql)?;
        self.wizard_foreign_keys.extend(crate::table_sql::table_foreign_keys(&sql)?);
        self.wizard_foreign_key_sel = 0;
        self.wizard_foreign_key_states = vec![[None, None]; self.wizard_foreign_keys.len()];
        self.wizard_foreign_key_scroll = (0.0, 0.0);
        self.wizard_constraint_columns = cols.iter().map(|column| column.name.clone()).collect();
        self.wizard_constraint_table = sel.name.clone();
        self.wizard_check_sel = 0;
        self.wizard_check_states = vec![None; self.wizard_checks.len()];
        self.wizard_check_scroll = (0.0, 0.0);
        self.wizard_table_page = 0;
        self.wizard_table_ratio = 0.72;
        self.wizard_table_advanced = false;
        self.wizard_table_original = Some(sel);
        self.wizard_original_cols = cols.clone();
        self.wizard_table_preview_state = None;
        self.wizard_table_name_state = None;
        self.wizard_cols = cols;
        self.wizard_column_edit_states = vec![[None, None, None]; self.wizard_cols.len()];
        self.wizard_col_sel = 0;
        self.overlay = Some(OverlayKind::EditTable);
        Ok(())
    }

    pub fn open_create_index_wizard(&mut self) {
        let tables = self.index_tables();
        let target = self.selected.as_ref().filter(|s| s.kind == ObjectKind::Table)
            .map(|s| (s.schema.as_str(), s.name.as_str()))
            .or_else(|| self.browse().table.as_deref().map(|table| (self.browse().schema.as_str(), table)))
            .filter(|target| tables.contains(target))
            .or_else(|| tables.first().copied());
        let (schema, table) = target.map(|(schema, table)| (schema.to_string(), table.to_string())).unwrap_or_else(|| ("main".into(), String::new()));
        self.wizard_schema = schema;
        self.wizard_scroll = (0.0, 0.0);
        self.wizard_index_name = format!("idx_{table}");
        self.wizard_index_table = table;
        self.wizard_index_cols = String::new();
        self.wizard_index_where.clear();
        self.wizard_index_edit_states = [None, None, None];
        self.wizard_index_list_scroll = [0.0; 2];
        self.wizard_index_key_sel = 0;
        self.wizard_index_available_sel = 0;
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
        let (table, sql): (String, Option<String>) = conn.query_row(
            &format!("SELECT tbl_name, sql FROM {}.sqlite_schema WHERE type='index' AND name=?1", db::quote_ident(&sel.schema)),
            [&sel.name], |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|e| e.to_string())?;
        let sql = sql.ok_or("Modify the table to change an automatic constraint index")?;
        let mut stmt = conn.prepare(&format!("PRAGMA {}.index_list({})", db::quote_ident(&sel.schema), db::quote_ident(&table))).map_err(|e| e.to_string())?;
        let indexes = stmt.query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, bool>(2)?, r.get::<_, String>(3)?))).map_err(|e| e.to_string())?;
        let mut flags = None;
        for index in indexes {
            let (name, unique, origin) = index.map_err(|e| e.to_string())?;
            if name == sel.name { flags = Some((unique, origin)); break; }
        }
        let (unique, origin) = flags.ok_or("Index no longer exists")?;
        if origin != "c" { return Err("Modify the table to change an automatic constraint index".into()); }
        let (columns, predicate) = crate::index_sql::definition(&sql)?;
        self.wizard_schema = sel.schema.clone();
        self.wizard_scroll = (0.0, 0.0);
        self.wizard_index_name = sel.name.clone();
        self.wizard_index_original = Some(sel);
        self.wizard_index_table = table;
        self.wizard_index_unique = unique;
        self.wizard_index_cols = columns;
        self.wizard_index_where = predicate;
        self.wizard_index_edit_states = [None, None, None];
        self.wizard_index_list_scroll = [0.0; 2];
        self.wizard_index_key_sel = 0;
        self.wizard_index_available_sel = 0;
        self.overlay = Some(OverlayKind::EditIndex);
        Ok(())
    }

    fn select_saved_object(&mut self, schema: &str, kind: ObjectKind, name: &str, clean_ddl: bool) {
        self.selected = Some(SelectedObject { schema: schema.into(), kind, name: name.into() });
        self.schema_group = None;
        if clean_ddl {
            self.ddl_draft = self.schema.iter().find(|o| o.schema == schema && o.kind == kind && o.name == name).and_then(|o| o.sql.clone()).unwrap_or_default();
        }
    }

    pub fn table_wizard_names_valid(&self) -> bool {
        !self.wizard_table_name.trim().is_empty() && self.wizard_cols.iter().enumerate().all(|(index, column)|
            !column.name.trim().is_empty() && !self.wizard_cols[..index].iter().any(|old| old.name.eq_ignore_ascii_case(&column.name)))
    }

    fn wizard_constraint_renames(&self) -> Result<Vec<(&str, &str)>, String> {
        if self.wizard_constraint_columns.len() != self.wizard_cols.len() { return Err("Constraint column identities do not match the table draft".into()); }
        Ok(self.wizard_constraint_columns.iter().zip(&self.wizard_cols)
            .filter(|(old, column)| **old != column.name).map(|(old, column)| (old.as_str(), column.name.as_str())).collect())
    }

    fn renamed_wizard_checks(&self, checks: &[crate::table_sql::NamedCheck]) -> Result<Vec<crate::table_sql::NamedCheck>, String> {
        if checks.is_empty() { return Ok(Vec::new()); }
        let renames = self.wizard_constraint_renames()?;
        if renames.is_empty() && self.wizard_constraint_table == self.wizard_table_name.trim() { return Ok(checks.to_vec()); }
        crate::table_sql::rename_checks(checks, &self.wizard_constraint_table, self.wizard_table_name.trim(), &renames)
    }

    fn renamed_wizard_foreign_keys(&self, keys: &[crate::table_sql::NamedForeignKey]) -> Result<Vec<crate::table_sql::NamedForeignKey>, String> {
        if keys.is_empty() { return Ok(Vec::new()); }
        let renames = self.wizard_constraint_renames()?;
        if renames.is_empty() && self.wizard_constraint_table == self.wizard_table_name.trim() { return Ok(keys.to_vec()); }
        crate::table_sql::rename_foreign_keys(keys, &self.wizard_constraint_table, self.wizard_table_name.trim(), &renames)
    }

    fn renamed_wizard_index_constraints(&self, keys: &[crate::table_sql::NamedIndexConstraint]) -> Result<Vec<crate::table_sql::NamedIndexConstraint>, String> {
        let renames = self.wizard_constraint_renames()?;
        if renames.is_empty() { return Ok(keys.to_vec()); }
        crate::table_sql::rename_index_constraints(keys, &renames)
    }

    pub fn table_index_constraints_from_columns(&self) -> Result<Vec<crate::table_sql::NamedIndexConstraint>, String> {
        if !self.table_wizard_names_valid() { return Err("Finish distinct column names before changing key membership".into()); }
        crate::table_sql::index_constraints_from_columns(&self.renamed_wizard_index_constraints(&self.wizard_index_constraints)?,
            &self.wizard_cols, self.wizard_table_options.primary_key_conflict)
    }

    pub fn move_wizard_column(&mut self, target: usize) -> Result<(), String> {
        let selected = self.wizard_col_sel; let count = self.wizard_cols.len();
        if selected >= count || target >= count { return Err("Select an existing column to move".into()); }
        if self.wizard_column_edit_states.len()!=count || self.wizard_constraint_columns.len()!=count
            || (self.wizard_table_original.is_some() && self.wizard_col_sources.len()!=count) {
            return Err("Column identities do not match the table draft; reopen Modify Table".into());
        }
        if selected == target { return Ok(()); }
        let range = selected.min(target)..selected.max(target)+1;
        let shift = if selected < target { range.len()-1 } else { 1 };
        self.wizard_cols[range.clone()].rotate_right(shift);
        self.wizard_column_edit_states[range.clone()].rotate_right(shift);
        self.wizard_constraint_columns[range.clone()].rotate_right(shift);
        if self.wizard_table_original.is_some() { self.wizard_col_sources[range].rotate_right(shift); }
        self.wizard_col_sel = target;
        Ok(())
    }

    pub fn remove_wizard_column(&mut self, row: usize) -> Result<(), String> {
        let count = self.wizard_cols.len();
        if row >= count { return Err("Select an existing column to remove".into()); }
        if self.wizard_constraint_columns.len() != count || self.wizard_column_edit_states.len() != count
            || (self.wizard_table_original.is_some() && self.wizard_col_sources.len() != count)
            || self.wizard_key_states.len() != self.wizard_index_constraints.len()
            || self.wizard_foreign_key_states.len() != self.wizard_foreign_keys.len()
            || self.wizard_check_states.len() != self.wizard_checks.len() {
            return Err("Column identities do not match the table draft; reopen Modify Table".into());
        }
        let owner = &self.wizard_constraint_columns[row];
        let mut indexes = Vec::new(); let mut index_states = Vec::new(); let mut key_sel = self.wizard_key_sel;
        for (index, key) in self.wizard_index_constraints.iter().enumerate() {
            let members = crate::table_sql::index_constraint_columns(key)?;
            let retained: Vec<_> = members.iter().filter(|(name, _)| !name.eq_ignore_ascii_case(owner)).collect();
            if retained.is_empty() { if index < self.wizard_key_sel { key_sel -= 1; } continue; }
            let mut next = key.clone(); let changed = retained.len() != members.len();
            if changed { next.columns = retained.iter().map(|(_, sql)| sql.as_str()).collect::<Vec<_>>().join(", "); }
            indexes.push(next); index_states.push(if changed { None } else { self.wizard_key_states[index].clone() });
        }
        let mut foreign_keys = Vec::new(); let mut foreign_states = Vec::new(); let mut fk_sel = self.wizard_foreign_key_sel;
        for (index, key) in self.wizard_foreign_keys.iter().enumerate() {
            let members = crate::table_sql::foreign_key_columns(key)?;
            let retained: Vec<_> = members.iter().filter(|(name, _)| !name.eq_ignore_ascii_case(owner)).collect();
            if retained.is_empty() { if index < self.wizard_foreign_key_sel { fk_sel -= 1; } continue; }
            let mut next = key.clone(); let changed = retained.len() != members.len();
            // Keep the parent definition for review; existing SQL validation rejects any new arity mismatch.
            if changed { next.columns = retained.iter().map(|(_, sql)| sql.as_str()).collect::<Vec<_>>().join(", "); }
            foreign_keys.push(next); foreign_states.push(if changed { [None, None] } else { self.wizard_foreign_key_states[index].clone() });
        }
        let mut checks = Vec::new(); let mut check_states = Vec::new(); let mut check_sel = self.wizard_check_sel;
        for (index, check) in self.wizard_checks.iter().enumerate() {
            if check.column.as_ref().is_some_and(|name| name.eq_ignore_ascii_case(owner)) {
                if index < self.wizard_check_sel { check_sel -= 1; } continue;
            }
            checks.push(check.clone()); check_states.push(self.wizard_check_states[index].clone());
        }
        // Constraint lists use stable identities even while draft field names are empty or duplicated.
        let mut columns = self.wizard_cols.clone();
        for (column, name) in columns.iter_mut().zip(&self.wizard_constraint_columns) { column.name = name.clone(); }
        columns.remove(row);
        let mut columns = crate::table_sql::columns_with_index_constraints(&columns, &indexes)?;
        for (column, (_, original)) in columns.iter_mut().zip(self.wizard_cols.iter().enumerate().filter(|(index, _)| *index != row)) { column.name = original.name.clone(); }
        self.wizard_cols = columns; self.wizard_constraint_columns.remove(row); self.wizard_column_edit_states.remove(row);
        if self.wizard_table_original.is_some() { self.wizard_col_sources.remove(row); }
        self.wizard_index_constraints = indexes; self.wizard_key_states = index_states;
        self.wizard_key_sel = key_sel.min(self.wizard_index_constraints.len().saturating_sub(1));
        self.wizard_table_options.index_constraints = Some(self.wizard_index_constraints.clone());
        self.wizard_table_options.primary_key_conflict = self.wizard_index_constraints.iter().find(|key| key.primary).and_then(|key| key.conflict);
        self.wizard_foreign_keys = foreign_keys; self.wizard_foreign_key_states = foreign_states;
        self.wizard_foreign_key_sel = fk_sel.min(self.wizard_foreign_keys.len().saturating_sub(1));
        self.wizard_checks = checks; self.wizard_check_states = check_states;
        self.wizard_check_sel = check_sel.min(self.wizard_checks.len().saturating_sub(1));
        self.store_wizard_checks(); self.store_wizard_foreign_keys();
        self.wizard_col_sel = (self.wizard_col_sel - usize::from(self.wizard_col_sel > row)).min(self.wizard_cols.len().saturating_sub(1));
        self.wizard_field_edit = None; self.wizard_constraint_edit = None; self.wizard_column_picker = None;
        Ok(())
    }

    pub fn store_wizard_checks(&mut self) {
        self.wizard_table_options.checks = Some(self.wizard_checks.iter().filter(|check| check.column.is_none()).cloned().collect());
        self.wizard_table_options.inline_checks = Some(self.wizard_checks.iter().filter(|check| check.column.is_some()).cloned().collect());
    }

    pub fn sync_wizard_check_fields(&mut self) -> bool {
        let mut changed = false;
        for column in &mut self.wizard_cols {
            let expression = self.wizard_checks.iter().find(|check| check.column.as_ref().is_some_and(|owner| owner.eq_ignore_ascii_case(&column.name)))
                .map(|check| check.expression.clone()).unwrap_or_default();
            if expression != column.check_expression { column.check_expression = expression; changed = true; }
        }
        changed
    }

    pub fn store_wizard_foreign_keys(&mut self) {
        self.wizard_table_options.foreign_keys = Some(self.wizard_foreign_keys.iter().filter(|key| !key.inline).cloned().collect());
        self.wizard_table_options.inline_foreign_keys = Some(self.wizard_foreign_keys.iter().filter(|key| key.inline).cloned().collect());
    }

    pub fn sync_wizard_foreign_key_fields(&mut self) -> bool {
        let mut changed = false;
        for column in &mut self.wizard_cols {
            let clause = self.wizard_foreign_keys.iter().find(|key| key.inline
                && crate::table_sql::inline_foreign_key_column(key).is_ok_and(|owner| owner.eq_ignore_ascii_case(&column.name)))
                .map(|key| key.clause.clone()).unwrap_or_default();
            if clause != column.foreign_key_clause { column.foreign_key_clause = clause; changed = true; }
        }
        changed
    }

    fn table_wizard_options(&self) -> Result<crate::table_sql::TableOptions, String> {
        let mut options = self.wizard_table_options.clone();
        if let Some(checks) = &options.checks { options.checks = Some(self.renamed_wizard_checks(checks)?); }
        if let Some(checks) = &options.inline_checks { options.inline_checks = Some(self.renamed_wizard_checks(checks)?); }
        if let Some(keys) = &options.foreign_keys { options.foreign_keys = Some(self.renamed_wizard_foreign_keys(keys)?); }
        if let Some(keys) = &options.inline_foreign_keys { options.inline_foreign_keys = Some(self.renamed_wizard_foreign_keys(keys)?); }
        if let Some(keys) = &options.index_constraints { options.index_constraints = Some(self.renamed_wizard_index_constraints(keys)?); }
        if options.primary_key_conflict.is_some() && options.index_constraints.as_ref().is_some_and(|keys| !keys.iter().any(|key| key.primary)) {
            return Err("Select a primary-key column before choosing its conflict action".into());
        }
        Ok(options)
    }

    pub fn sync_table_constraint_names(&mut self) -> Result<bool, String> {
        // Keep the last unambiguous identities while names are empty or temporarily duplicated.
        if !self.table_wizard_names_valid() { return Ok(false); }
        let checks = self.renamed_wizard_checks(&self.wizard_checks)?;
        let keys = self.renamed_wizard_foreign_keys(&self.wizard_foreign_keys)?;
        let indexes = self.renamed_wizard_index_constraints(&self.wizard_index_constraints)?;
        let changed = checks != self.wizard_checks || keys != self.wizard_foreign_keys || indexes != self.wizard_index_constraints;
        for (index, (old, new)) in self.wizard_index_constraints.iter().zip(&indexes).enumerate() {
            if old.columns != new.columns {
                // shortcut: reference renames reset key history; transform past edits when adding wizard-wide Undo.
                if let Some(session) = self.wizard_key_states.get_mut(index) { *session = None; }
            }
        }
        self.wizard_index_constraints = indexes;
        if self.wizard_table_options.index_constraints.is_some() { self.wizard_table_options.index_constraints = Some(self.wizard_index_constraints.clone()); }
        for (index, (old, new)) in self.wizard_checks.iter().zip(&checks).enumerate() {
            if old.expression != new.expression {
                // shortcut: reference renames reset CHECK history; transform past edits when adding wizard-wide Undo.
                if let Some(session) = self.wizard_check_states.get_mut(index) { *session = None; }
            }
        }
        for (index, (old, new)) in self.wizard_foreign_keys.iter().zip(&keys).enumerate() {
            if old.columns != new.columns || old.clause != new.clause {
                // shortcut: reference renames reset FK history; transform past edits when adding wizard-wide Undo.
                if let Some(session) = self.wizard_foreign_key_states.get_mut(index) { *session = [None, None]; }
            }
        }
        self.wizard_foreign_keys = keys;
        if self.wizard_table_options.foreign_keys.is_some() || self.wizard_table_options.inline_foreign_keys.is_some() { self.store_wizard_foreign_keys(); }
        self.wizard_checks = checks;
        if self.wizard_table_options.checks.is_some() || self.wizard_table_options.inline_checks.is_some() { self.store_wizard_checks(); }
        self.wizard_constraint_columns = self.wizard_cols.iter().map(|column| column.name.clone()).collect();
        self.wizard_constraint_table = self.wizard_table_name.trim().to_string();
        if changed { self.sync_wizard_foreign_key_fields(); self.sync_wizard_check_fields(); }
        Ok(changed)
    }

    pub fn table_wizard_sql(&self) -> Result<String, String> {
        let options = self.table_wizard_options()?;
        let columns = if let Some(keys) = &options.index_constraints { crate::table_sql::columns_with_index_constraints(&self.wizard_cols, keys)? } else { self.wizard_cols.clone() };
        let name = self.wizard_table_name.trim();
        if name.is_empty() { return Err("Table name required".into()); }
        if columns.is_empty() { return Err("Add at least one column".into()); }
        for (index, column) in columns.iter().enumerate() {
            if column.name.trim().is_empty() { return Err("Column name required".into()); }
            if self.wizard_cols[..index].iter().any(|old| old.name.eq_ignore_ascii_case(&column.name)) {
                return Err(format!("Duplicate column name: {}", column.name));
            }
        }
        crate::table_sql::validate_primary_key(&columns)?;
        let Some(original) = &self.wizard_table_original else {
            for column in &columns { crate::table_sql::column_definition(column, None)?; }
            return crate::table_sql::apply_table_options(&db::create_table_ddl_in(&self.wizard_schema, name, &columns), options)
                .and_then(|sql| crate::table_sql::format_table_preview(&sql));
        };
        if self.wizard_col_sources.len() != columns.len() { return Err("Column identities do not match the table draft".into()); }
        let sql = self.schema.iter().find(|object| object.schema == original.schema && object.name == original.name && object.kind == ObjectKind::Table)
            .and_then(|object| object.sql.as_deref()).ok_or("Original table definition no longer exists")?;
        let renames: Vec<_> = columns.iter().zip(&self.wizard_col_sources).filter_map(|(column, source)|
            source.as_ref().filter(|source| **source != column.name).map(|source| (source.as_str(), column.name.as_str()))).collect();
        let sql = crate::table_sql::preview_renames(sql, name, &renames)?;
        let needs_rebuild = original.schema != self.wizard_schema || crate::table_sql::requires_rebuild(&self.wizard_original_cols, &columns,
            &self.wizard_col_sources, &crate::table_sql::table_options(&sql)?, Some(&options));
        let sql = crate::table_sql::rebuild_definition(&sql, &self.wizard_schema, name, &self.wizard_original_cols,
            &columns, &self.wizard_col_sources, Some(options))?;
        let sql = if needs_rebuild { crate::table_sql::unqualify_local_checks(&sql, name)? } else { sql };
        crate::table_sql::format_table_preview(&sql)
    }

    pub fn apply_table_wizard(&mut self) -> Result<(), String> {
        self.guard_writable()?;
        if self.has_cell_draft() { return Err("Apply or discard the cell draft first".into()); }
        let options = self.table_wizard_options()?;
        let columns = if let Some(keys) = &options.index_constraints { crate::table_sql::columns_with_index_constraints(&self.wizard_cols, keys)? } else { self.wizard_cols.clone() };
        let schema = self.wizard_schema.clone();
        let name = self.wizard_table_name.trim().to_string();
        if name.is_empty() { return Err("Table name required".into()); }
        if columns.is_empty() { return Err("Add at least one column".into()); }
        crate::table_sql::validate_primary_key(&columns)?;
        if columns.iter().any(|c| c.name.trim().is_empty()) { return Err("Column name required".into()); }
        let original = self.wizard_table_original.clone();
        let renames: Vec<_> = columns.iter().zip(&self.wizard_col_sources)
            .filter_map(|(col, source)| source.as_ref().filter(|source| **source != col.name).map(|source| (source.clone(), col.name.clone()))).collect();
        let previous_source = self.selected.as_ref().and_then(|s| self.schema.iter().find(|o| o.schema == s.schema && o.kind == s.kind && o.name == s.name)).and_then(|o| o.sql.clone());
        let clean_ddl = self.ddl_draft == previous_source.unwrap_or_default();
        let conn = self.conn.as_ref().unwrap();
        let moved_views=if let Some(orig) = original.as_ref() {
            let views=db::recreate_table_columns_between(conn, (&orig.schema, &orig.name), (&schema, &name), &columns, &self.wizard_col_sources, Some(options))?;
            self.log_sql(format!("-- modify table {} → {}", db::qualified_name(&orig.schema, &orig.name), db::qualified_name(&schema, &name)));
            views
        } else {
            for column in &columns { crate::table_sql::column_definition(column, None)?; }
            let ddl = crate::table_sql::apply_table_options(&db::create_table_ddl_in(&schema, &name, &columns), options)?;
            db::execute_single_statement(conn, &ddl)?;
            self.log_sql(ddl);
            Vec::new()
        };
        self.mark_dirty();
        self.overlay = None;
        if let Some(orig) = original.as_ref().filter(|o| o.schema != schema || o.name != name) {
            for (old_name,new_name) in std::iter::once((orig.name.as_str(),name.as_str())).chain(moved_views.iter().map(|name|(name.as_str(),name.as_str()))) {
                for tab in &mut self.browse_tabs {
                    let saved = if orig.schema == "main" { tab.table_options.remove(old_name) }
                        else { tab.schema_table_options.get_mut(&orig.schema).and_then(|tables| tables.remove(old_name)) };
                    if let Some(options) = saved { tab.store_options(&schema, new_name, options); }
                    if tab.schema == orig.schema && tab.table.as_deref() == Some(old_name) {
                        tab.schema = schema.clone();
                        tab.table = Some(new_name.into());
                        tab.title = if schema == "main" { new_name.into() } else { db::qualified_name(&schema, new_name) };
                    }
                }
                for rule in &mut self.cond_rules {
                    if rule.schema == orig.schema && rule.table.as_deref() == Some(old_name) { rule.schema = schema.clone(); rule.table = Some(new_name.into()); }
                }
                if self.chart_schema == orig.schema && self.chart_table.as_deref() == Some(old_name) { self.chart_schema = schema.clone(); self.chart_table = Some(new_name.into()); }
            }
        }
        if original.is_some() && !renames.is_empty() {
            for tab in &mut self.browse_tabs {
                if let Some(mut options) = tab.stored_options(&schema, &name).cloned() {
                    options.rename_columns(&renames);
                    tab.store_options(&schema, &name, options);
                }
                if tab.schema == schema && tab.table.as_deref() == Some(&name) {
                    let mut options = tab.options();
                    options.rename_columns(&renames);
                    tab.set_options(options);
                    tab.result = None;
                    tab.rowids.clear();
                    tab.total_rows = None;
                    tab.clear_selection();
                }
            }
            for rule in &mut self.cond_rules {
                if rule.schema == schema && rule.table.as_deref() == Some(&name) { rename_column(&mut rule.column, &renames); }
            }
            if self.chart_schema == schema && self.chart_table.as_deref() == Some(&name) { rename_column(&mut self.chart_column, &renames); }
        }
        for view in &moved_views {
            let columns=db::table_column_names_in(self.conn.as_ref().unwrap(),&schema,view)?;
            let changes=renames.iter().filter(|(from,to)|!columns.iter().any(|column|column.eq_ignore_ascii_case(from)) && columns.iter().any(|column|column.eq_ignore_ascii_case(to))).cloned().collect::<Vec<_>>();
            for tab in &mut self.browse_tabs {
                if let Some(mut options)=tab.stored_options(&schema,view).cloned() {options.rename_columns(&changes);options.remap_columns(&columns);tab.store_options(&schema,view,options);}
                if tab.schema==schema && tab.table.as_deref()==Some(view) {
                    let mut options=tab.options();options.rename_columns(&changes);options.remap_columns(&columns);tab.set_options(options);tab.result=None;tab.rowids.clear();tab.total_rows=None;tab.clear_selection();
                }
            }
            for rule in &mut self.cond_rules {if rule.schema==schema && rule.table.as_deref()==Some(view) {rename_column(&mut rule.column,&changes);}}
            if self.chart_schema==schema && self.chart_table.as_deref()==Some(view) {rename_column(&mut self.chart_column,&changes);}
        }
        self.refresh_schema()?;
        self.select_saved_object(&schema, ObjectKind::Table, &name, clean_ddl);
        let browse = if self.browse().schema == schema && self.browse().table.as_deref() == Some(&name) { self.refresh_browse() }
            else { self.switch_browse_table_in(&schema, &name) };
        let browse_error = browse.err().map(|error| {
            if self.browse().schema == schema && self.browse().table.as_deref() == Some(&name) {
                let tab = self.browse_mut();
                tab.result = None;
                tab.rowids.clear();
                tab.total_rows = None;
                tab.clear_selection();
                self.clear_browse_editor();
            }
            format!("; browse unavailable: {error}")
        }).unwrap_or_default();
        let plot_error = self.refresh_chart().err().map(|e| format!("; plot unavailable: {e}")).unwrap_or_default();
        self.status_msg = format!("Table {} saved (unwritten){browse_error}{plot_error}", db::qualified_name(&schema, &name));
        Ok(())
    }

    pub fn index_wizard_column_details(&self) -> Result<Vec<(String, String)>, String> {
        db::index_table_columns(self.conn.as_ref().ok_or("No database open")?, &self.wizard_schema, &self.wizard_index_table)
    }

    pub fn index_wizard_columns(&self) -> Result<Vec<String>, String> {
        Ok(self.index_wizard_column_details()?.into_iter().map(|(name, _)| name).collect())
    }

    pub fn index_wizard_key_order(&self) -> Result<bool, String> {
        let keys = crate::index_sql::keys(&self.wizard_index_cols)?;
        let key = keys.get(self.wizard_index_key_sel).ok_or("Select an index key")?;
        crate::index_sql::key_order(self.conn.as_ref().ok_or("No database open")?, &self.wizard_schema, &self.wizard_index_table, key)
            .map(|(_, descending)| descending.unwrap_or(false))
    }

    pub fn index_wizard_sql(&self) -> String {
        db::create_index_ddl_in(&self.wizard_schema, self.wizard_index_name.trim(), &self.wizard_index_table,
            &self.wizard_index_cols, self.wizard_index_unique, &self.wizard_index_where)
    }

    pub fn apply_index_wizard(&mut self) -> Result<(), String> {
        self.guard_writable()?;
        let schema = self.wizard_schema.clone();
        let name = self.wizard_index_name.trim().to_string();
        let table = self.wizard_index_table.clone();
        let cols = self.wizard_index_cols.trim().to_string();
        let predicate = self.wizard_index_where.trim().to_string();
        if name.is_empty() || table.is_empty() || cols.is_empty() { return Err("Index needs name, table, and columns".into()); }
        if !self.index_tables().contains(&(schema.as_str(), table.as_str())) { return Err("Select a valid table for the index".into()); }
        let clean_ddl = self.ddl_draft == self.selected.as_ref().and_then(|s| self.schema.iter().find(|o| o.schema == s.schema && o.kind == s.kind && o.name == s.name)).and_then(|o| o.sql.as_deref()).unwrap_or_default();
        let conn = self.conn.as_ref().unwrap();
        db::with_savepoint(conn, || {
            if let Some(orig) = self.wizard_index_original.as_ref() { db::drop_object(conn, ObjectKind::Index, &orig.schema, &orig.name)?; }
            db::create_index_in(conn, &schema, &name, &table, &cols, self.wizard_index_unique, &predicate)
        })?;
        self.log_sql(db::create_index_ddl_in(&schema, &name, &table, &cols, self.wizard_index_unique, &predicate));
        self.mark_dirty();
        self.overlay = None;
        self.refresh_schema()?;
        self.select_saved_object(&schema, ObjectKind::Index, &name, clean_ddl);
        self.status_msg = format!("Index {} saved (unwritten)", db::qualified_name(&schema, &name));
        Ok(())
    }

    pub fn load_csv_import_settings(&mut self) {
        let opts = self.prefs.csv_import.clone().unwrap_or_else(|| CsvImportOptions { has_header: false, trim_fields: true, delimiter: self.prefs.csv_delimiter, ..CsvImportOptions::default() });
        self.csv_has_header = opts.has_header;
        self.csv_delim_idx = match opts.delimiter { ',' => 0, ';' => 1, '\t' => 2, '|' => 3, _ => 4 };
        self.csv_custom_delimiter = opts.delimiter.to_string();
        self.csv_quote_idx = match opts.quote { Some('"') => 0, Some('\'') => 1, None => 2, _ => 3 };
        self.csv_custom_quote = opts.quote.map(String::from).unwrap_or_default();
        self.csv_encoding_idx = match opts.encoding.as_str() { "UTF-8" => 0, "UTF-16" => 1, "ISO-8859-1" => 2, _ => 3 };
        self.csv_custom_encoding = opts.encoding.clone();
        self.csv_import_settings = opts;
        self.csv_preview = None;
    }

    pub fn open_csv_files(&mut self, paths: Vec<PathBuf>) -> bool {
        let Some(batch) = crate::csv_files::Batch::new(paths, self.prefs.csv_separate_tables) else { return false; };
        self.load_csv_import_settings();
        self.csv_path = batch.files[0].path.display().to_string();
        self.csv_table = batch.target(0).into();
        self.csv_files = Some(batch); self.csv_scroll = (0.0, 0.0);
        self.overlay = Some(OverlayKind::ImportCsv); true
    }

    pub fn selected_csv_path(&self) -> &Path {
        self.csv_files.as_ref().map_or_else(|| Path::new(&self.csv_path), |batch| batch.files[batch.current].path.as_path())
    }

    pub fn select_csv_file(&mut self, index: usize) -> bool {
        let Some(batch) = &mut self.csv_files else { return false; };
        if !batch.select(index, &self.csv_table) { return false; }
        self.csv_path = batch.files[index].path.display().to_string(); self.csv_table = batch.target(index).into(); self.csv_preview = None; true
    }

    pub fn refresh_csv_preview(&mut self) {
        if self.csv_preview.is_some() { return; }
        if let Some(work) = &self.csv_work {
            if let Some(data) = &work.pending {
                self.csv_preview = Some(Ok(data.preview.clone()));
            }
            return;
        }
        let options = self.csv_options();
        self.csv_preview = Some(if self.overlay == Some(OverlayKind::PasteCsv) {
            crate::csv_import::text(&self.csv_paste, &options, 20)
        } else {
            std::fs::File::open(self.selected_csv_path()).map_err(|e| e.to_string()).and_then(|file| crate::csv_import::read(file, &options, 20))
        });
    }

    pub fn csv_options(&self) -> CsvImportOptions {
        let delimiter = match self.csv_delim_idx {
            1 => ';',
            2 => '\t',
            3 => '|',
            4 | 5 => crate::csv_import::character(&self.csv_custom_delimiter, self.csv_delim_idx == 5).unwrap_or('\0'),
            _ => ',',
        };
        let quote = match self.csv_quote_idx {
            1 => Some('\''),
            2 => None,
            3 | 4 => Some(crate::csv_import::character(&self.csv_custom_quote, self.csv_quote_idx == 4).unwrap_or('\0')),
            _ => Some('"'),
        };
        CsvImportOptions {
            encoding: match self.csv_encoding_idx {1=>"UTF-16",2=>"ISO-8859-1",3=>self.csv_custom_encoding.trim(),_=>"UTF-8"}.into(),
            has_header: self.csv_has_header,
            delimiter,
            quote,
            ..self.csv_import_settings.clone()
        }
    }

    pub fn apply_csv_import(&mut self) -> Result<(), String> {
        self.start_csv_import_work()
    }

    pub fn pick_chart_source(&mut self) {
        self.chart_schema = "main".into();
        self.chart_column = if self.chart_column.trim().is_empty() {
            "price".into()
        } else {
            self.chart_column.clone()
        };
        let names = self
            .schema
            .iter()
            .filter(|o| o.schema == "main" && o.kind == ObjectKind::Table)
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

    pub fn table_schemas(&self) -> Vec<&str> {
        ["main", "temp"].into_iter().chain(self.schema_names.iter().map(String::as_str).filter(|s| !["main", "temp"].contains(s))).collect()
    }

    pub fn index_tables(&self) -> Vec<(&str, &str)> {
        self.schema.iter().filter(|o| o.kind == ObjectKind::Table).map(|o| (o.schema.as_str(), o.name.as_str())).collect()
    }

    pub fn browse_tables(&self) -> Vec<(&str, &str)> {
        self.schema.iter().filter(|o| matches!(o.kind, ObjectKind::Table | ObjectKind::View))
            .map(|o| (o.schema.as_str(), o.name.as_str())).collect()
    }

    pub fn column_type_names(&self) -> Vec<String> {
        let names=if self.wizard_table_options.strict { &["INT","INTEGER","TEXT","BLOB","REAL","ANY"][..] } else {COLUMN_TYPES};
        let mut types: Vec<_> = names.iter().map(|s| (*s).to_string()).collect();
        if let Some(column) = self.wizard_cols.get(self.wizard_col_sel) {
            if !types.contains(&column.type_name) { types.push(column.type_name.clone()); }
        }
        types
    }

    pub fn column_collation_names(&self) -> Vec<String> {
        let mut names: Vec<String> = ["", "BINARY", "NOCASE", "RTRIM"].into_iter().map(str::to_string).collect();
        if let Some(conn) = &self.conn {
            let loaded = conn.prepare("PRAGMA collation_list").and_then(|mut statement| {
                statement.query_map([], |row| row.get::<_,String>(1))?.collect::<rusqlite::Result<Vec<_>>>()
            });
            if let Ok(loaded) = loaded {
                for name in loaded { if !names.iter().any(|old| old.eq_ignore_ascii_case(&name)) { names.push(name); } }
            }
        }
        if let Some(column) = self.wizard_cols.get(self.wizard_col_sel) {
            if !names.iter().any(|old| old.eq_ignore_ascii_case(&column.collation)) { names.push(column.collation.clone()); }
        }
        names
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
        self.ddl_draft.clear();
        self.schema_group = None;
        self.schema_tree_expanded.clear();
        self.schema_tree_scroll = 0.0;
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
            .find(|o| o.schema == "main" && o.kind == ObjectKind::Table)
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
        self.schema_names.clear();
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
        self.schema_names.clear();
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
        self.guard_csv_idle()?;
        if self.sql_busy {
            return Err("Wait for SQL to finish".into());
        }
        if self.has_cell_draft() { return Err("Apply or discard the cell draft first".into()); }
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
        self.refresh_schema()?;
        // The backup replaces the connection; temp and attached databases no longer exist.
        for tab in &mut self.browse_tabs {
            tab.schema_table_options.clear();
            if tab.schema != "main" {
                let main_options = std::mem::take(&mut tab.table_options);
                *tab = BrowseTabState::new(None);
                tab.table_options = main_options;
            }
        }
        self.refresh_browse()?;
        self.push_recent(dst);
        if self.chart_schema != "main" {
            self.chart_schema = "main".into();
            self.chart_table = None;
            self.chart_points.clear();
        }
        self.status_msg = format!("Saved As {dst}");
        Ok(())
    }

    fn guard_replace(&self) -> Result<(), String> {
        self.guard_csv_idle()?;
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
        self.csv_work=None;self.csv_files=None;self.csv_preview=None;
        if matches!(self.overlay,Some(OverlayKind::ImportCsv | OverlayKind::PasteCsv)) {self.overlay=None;}
        if let Some(conn) = self.conn.take() {
            let _ = conn.execute_batch("ROLLBACK");
            drop(conn);
        }
        self.schema.clear();
        self.schema_names.clear();
        self.sql_completion = crate::sql_completion::Catalog::default();
        self.database_encoding.clear();
        self.selected = None;
        self.ddl_draft.clear();
        self.schema_group = None;
        self.schema_tree_expanded.clear();
        self.schema_tree_scroll = 0.0;
        self.browse_tabs = vec![BrowseTabState::new(None)];
        self.browse_tab_idx = 0;
        self.sql_result = None;
        self.sql_outputs.clear();
        self.pragmas.clear();
        self.chart_schema = "main".into();
        self.chart_table = None;
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

    pub fn recover_ended_transaction(&mut self) -> Result<bool, String> {
        let Some(conn) = self.conn.as_ref() else { return Ok(false); };
        if self.read_only || !conn.is_autocommit() { return Ok(false); }
        db::begin_deferred(conn)?;
        self.dirty = false;
        // ROLLBACK conflicts and triggers can invalidate every cached row identity.
        for tab in &mut self.browse_tabs {
            tab.result = None;
            tab.rowids.clear();
            tab.total_rows = None;
            tab.clear_selection();
        }
        self.chart_points.clear();
        self.refresh_schema()?;
        if self.selected.as_ref().is_some_and(|selected| !self.schema.iter().any(|o| o.schema == selected.schema && o.kind == selected.kind && o.name == selected.name)) {
            self.selected = None;
        }
        for tab in &mut self.browse_tabs {
            if tab.table.as_ref().is_some_and(|table| !self.schema.iter().any(|o| o.schema == tab.schema && o.name == *table && matches!(o.kind, ObjectKind::Table | ObjectKind::View))) {
                tab.table = None;
            }
        }
        if self.chart_table.as_ref().is_some_and(|table| !self.schema.iter().any(|o| o.schema == self.chart_schema && o.name == *table && matches!(o.kind, ObjectKind::Table | ObjectKind::View))) {
            self.chart_table = None;
        }
        if !self.has_cell_draft() { self.refresh_browse()?; }
        self.refresh_chart()?;
        Ok(true)
    }

    pub fn guard_writable(&mut self) -> Result<(), String> {
        self.guard_csv_idle()?;
        if self.conn.is_none() {
            return Err("No database open".into());
        }
        if self.read_only {
            return Err("Database is open read-only".into());
        }
        if self.recover_ended_transaction()? {
            return Err("SQLite ended the previous transaction; review earlier pending changes and retry".into());
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

    fn refresh_browse_tab(&mut self) -> Result<(), String> {
        let previous = self.browse().clone();
        self.refresh_browse()?;
        self.browse_mut().restore_selection(&previous);
        self.sync_edit_cell_from_selection();
        Ok(())
    }

    pub fn add_browse_tab(&mut self) -> Result<(), String> {
        if self.sql_busy { return Err("Wait for SQL to finish before changing browse tabs".into()); }
        if self.has_cell_draft() { return Err("Apply or discard the cell draft first".into()); }
        let (schema, table) = if let Some(table) = &self.browse().table {
            (self.browse().schema.clone(), Some(table.clone()))
        } else {
            ("main".into(), self.schema.iter().find(|o| o.schema == "main" && o.kind == ObjectKind::Table).map(|o| o.name.clone()))
        };
        let old_index = self.browse_tab_idx;
        self.browse_tabs.push(BrowseTabState::new_in(&schema, table));
        self.browse_tab_idx = self.browse_tabs.len() - 1;
        if let Err(e) = self.refresh_browse() {
            self.browse_tabs.pop();
            self.browse_tab_idx = old_index;
            return Err(e);
        }
        Ok(())
    }

    pub fn close_browse_tab(&mut self) -> Result<(), String> {
        if self.sql_busy { return Err("Wait for SQL to finish before changing browse tabs".into()); }
        if self.has_cell_draft() { return Err("Apply or discard the cell draft first".into()); }
        if self.browse_tabs.len() <= 1 {
            self.browse_tabs[0] = BrowseTabState::new(None);
            self.browse_tab_idx = 0;
            self.clear_browse_editor();
        } else {
            let old_index = self.browse_tab_idx;
            let closed = self.browse_tabs.remove(old_index);
            self.browse_tab_idx = old_index.min(self.browse_tabs.len() - 1);
            if let Err(e) = self.refresh_browse_tab() {
                self.browse_tabs.insert(old_index, closed);
                self.browse_tab_idx = old_index;
                return Err(e);
            }
        }
        self.status_msg = "Browse tab closed".into();
        Ok(())
    }

    pub fn switch_browse_tab(&mut self, index: usize) -> Result<(), String> {
        if self.sql_busy { return Err("Wait for SQL to finish before changing browse tabs".into()); }
        if self.has_cell_draft() { return Err("Apply or discard the cell draft first".into()); }
        if index >= self.browse_tabs.len() { return Err("Browse tab no longer exists".into()); }
        let old_index = self.browse_tab_idx;
        self.browse_tab_idx = index;
        if let Err(e) = self.refresh_browse_tab() {
            self.browse_tab_idx = old_index;
            return Err(e);
        }
        Ok(())
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
        self.guard_csv_idle()?;
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
        self.prefs_sql_completion = self.prefs.sql_completion;
        self.prefs_sql_upper_keywords = self.prefs.sql_upper_keywords;
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
        self.prefs.sql_completion = self.prefs_sql_completion;
        self.prefs.sql_upper_keywords = self.prefs_sql_upper_keywords;
        if let Some(c) = self.prefs_csv_delim.chars().next() {
            let delimiter = if c == 't' { '\t' } else { c };
            if delimiter != self.prefs.csv_delimiter {
                if let Some(options) = &mut self.prefs.csv_import { options.delimiter = delimiter; }
            }
            self.prefs.csv_delimiter = delimiter;
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
        for tab in &mut pf.browse_tabs {
            if tab.schema == "temp" {
                if let Some(table) = &tab.table {
                    if db::table_columns_in(conn, "temp", table)?.is_empty() {
                        let mut empty = BrowseTabState::new(None);
                        empty.table_options = std::mem::take(&mut tab.table_options);
                        empty.schema_table_options = std::mem::take(&mut tab.schema_table_options);
                        *tab = empty;
                    }
                }
            }
            if let Some(table) = tab.table.clone() {
                if tab.column_widths.iter().any(|width| !width.is_finite() || *width <= 0.0) {
                    return Err(format!("Browse tab {}: invalid column width", tab.title));
                }
                let result = (|| {
                    let columns = db::table_column_names_in(conn, &tab.schema, &table)?;
                    let mut options = tab.options();
                    options.remap_columns(&columns);
                    tab.set_options(options);
                    db::browse_table_in(conn, &tab.schema, &table, &tab.ordering(),
                        Self::composed_where(tab, &columns)?.as_deref(), db::DEFAULT_LIMIT, tab.offset)
                })();
                result.map_err(|e| format!("Browse tab {}: {e}", tab.title))?;
            }
            let active_schema = tab.schema.clone();
            let active_table = tab.table.clone();
            let active_options = tab.options();
            for (schema, table, options) in tab.table_options.iter_mut().map(|(table, options)| ("main", table, options))
                .chain(tab.schema_table_options.iter_mut().flat_map(|(schema, tables)| tables.iter_mut().map(move |(table, options)| (schema.as_str(), table, options)))) {
                if active_schema == schema && active_table.as_deref() == Some(table) { *options = active_options.clone(); continue; }
                if schema == "temp" && db::table_columns_in(conn, schema, table)?.is_empty() { continue; }
                let columns = db::table_column_names_in(conn, schema, table)?;
                if columns.is_empty() { continue; }
                if options.column_widths.iter().any(|width| !width.is_finite() || *width <= 0.0) {
                    return Err(format!("Saved table {table}: invalid column width"));
                }
                options.remap_columns(&columns);
                let mut stored = BrowseTabState::new_in(schema, Some(table.into()));
                stored.set_options(options.clone());
                let result = (|| {
                    db::browse_table_in(conn, schema, table, &stored.ordering(),
                        Self::composed_where(&stored, &columns)?.as_deref(), 1, 0)
                })();
                result.map_err(|error| format!("Saved table {table}: {error}"))?;
            }
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
        if let Err(error) = self.refresh_chart() { self.status_msg.push_str(&format!("; plot unavailable: {error}")); }
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

    pub fn apply_cell_draft(&mut self) -> Result<(), String> {
        let schema = self.browse().schema.clone();
        self.guard_writable()?;
        let tab = self.browse();
        let table = tab.table.clone().ok_or("No browse table")?;
        let row = tab.sel_row.ok_or("Select a browse row first")?;
        let rowid = *tab.rowids.get(row).ok_or("No usable rowid for selection")?;
        let column = self.edit_col.trim().to_string();
        if column.is_empty() { return Err("Select a column first".into()); }
        if self.edit_cell_generated() { return Err("Generated columns are read-only; edit their source columns".into()); }
        let value = self.edited_cell_value()?;
        db::update_cell_in(self.conn.as_ref().unwrap(), &schema, &table, rowid, &column, &value)?;
        self.log_sql(format!("UPDATE {} SET {}=… WHERE rowid={rowid}", db::qualified_name(&schema, &table), db::quote_ident(&column)));
        self.mark_dirty();
        self.edit_saved = value;
        self.refresh_browse()?;
        self.status_msg = "Cell updated (unwritten)".into();
        Ok(())
    }

    pub fn edit_cell_generated(&self) -> bool {
        // Painting must not execute metadata queries and continuously append SQL log entries.
        let tab = self.browse();
        let Some(table) = tab.table.as_deref() else { return false; };
        let Some(sql) = self.schema.iter().find(|object| object.schema == tab.schema && object.name == table
            && object.kind == ObjectKind::Table).and_then(|object| object.sql.as_deref()) else { return false; };
        let mut column = ColDef::new(self.edit_col.trim(), "");
        crate::table_sql::read_column_constraints(sql, std::slice::from_mut(&mut column)).is_ok() && column.generated.is_some()
    }

    pub fn copy_browse_cells(&self, headers: bool, sql: bool) -> Result<crate::cell_clipboard::CellClipboard, String> {
        let tab = self.browse();
        let result = tab.result.as_ref().ok_or("No browse data")?;
        let row = tab.sel_row.ok_or("Select browse cells first")?;
        let anchor = tab.sel_row_start.unwrap_or(row);
        let (first, last) = (row.min(anchor), row.max(anchor));
        let (first_col, last_col) = if let Some(col) = tab.sel_col {
            let anchor = tab.sel_col_start.unwrap_or(col);
            (col.min(anchor), col.max(anchor))
        } else { (0, result.columns.len().saturating_sub(1)) };
        if last >= result.values.len() || result.columns.is_empty() || last_col >= result.columns.len() {
            return Err("Selection no longer fits the browse page; refresh and retry".into());
        }
        let mut cells = result.values.rectangle(first..=last, first_col..=last_col)?;
        let mut columns = result.columns[first_col..=last_col].to_vec();
        if sql {
            let conn = self.conn.as_ref().ok_or("Wait for SQL to finish before copying INSERT statements")?;
            let writable = db::table_columns_in(conn, &tab.schema, tab.table.as_deref().ok_or("No browse table")?)?;
            let indices = columns.iter().enumerate().filter_map(|(index, name)| writable.iter().any(|column| column.name == *name).then_some(index)).collect::<Vec<_>>();
            if indices.is_empty() { return Err("Selected columns are computed; select a writable column to copy INSERT statements".into()); }
            columns = indices.iter().map(|index| columns[*index].clone()).collect();
            cells = cells.into_iter().map(|row| indices.iter().map(|index| row[*index].clone()).collect()).collect();
        }
        Ok(crate::cell_clipboard::CellClipboard::new(&tab.schema, tab.table.as_deref().ok_or("No browse table")?,
            &columns, cells, headers, sql))
    }

    pub fn cut_browse_cells(&mut self) -> Result<(), String> {
        let schema = self.browse().schema.clone();
        self.guard_writable()?;
        if self.has_cell_draft() { return Err("Apply or discard the cell draft first".into()); }
        let tab = self.browse();
        let result = tab.result.as_ref().ok_or("No browse data")?;
        let table = tab.table.clone().ok_or("No browse table")?;
        let row = tab.sel_row.ok_or("Select browse cells first")?;
        let anchor = tab.sel_row_start.unwrap_or(row);
        let (first, last) = (row.min(anchor), row.max(anchor));
        let (first_col, last_col) = if let Some(col) = tab.sel_col {
            let anchor = tab.sel_col_start.unwrap_or(col);
            (col.min(anchor), col.max(anchor))
        } else { (0, result.columns.len().saturating_sub(1)) };
        if last >= tab.rowids.len() || result.columns.is_empty() || last_col >= result.columns.len() {
            return Err("Selected cells are not editable; refresh and retry".into());
        }
        let conn = self.conn.as_ref().ok_or("No database open")?;
        let columns = &result.columns[first_col..=last_col];
        let info = db::table_columns_in(conn, &schema, &table)?;
        let values = columns.iter().map(|col| {
            let column = info.iter().find(|info| info.name == *col).ok_or("Selected column is not editable")?;
            Ok(if column.notnull { db::CellValue::Text(Vec::new()) } else { db::CellValue::Null })
        }).collect::<Result<Vec<_>, String>>()?;
        let updates = tab.rowids[first..=last].iter().map(|id| (*id, values.clone())).collect::<Vec<_>>();
        db::update_cells_in(conn, &schema, &table, columns, &updates)?;
        self.log_sql(format!("-- cut {}x{} from {}", updates.len(), columns.len(), db::qualified_name(&schema, &table)));
        self.mark_dirty();
        self.refresh_browse()?;
        self.status_msg = "Cut cells (unwritten)".into();
        Ok(())
    }

    /// Paste quoted TSV; one source cell fills the selected rectangle.
    pub fn paste_browse_tsv(&mut self, text: &str) -> Result<(), String> {
        let (_, rows) = db::import_csv_from_text(text, &CsvImportOptions {
            encoding: "UTF-8".into(), has_header: false, delimiter: '\t', quote: Some('"'), ..CsvImportOptions::default()
        })?;
        self.paste_browse_cells(rows.into_iter().map(|row|
            row.into_iter().map(|s| db::CellValue::Text(s.into_bytes())).collect()
        ).collect())
    }

    pub fn paste_browse_cells(&mut self, mut rows: Vec<Vec<db::CellValue>>) -> Result<(), String> {
        let schema = self.browse().schema.clone();
        self.guard_writable()?;
        if self.has_cell_draft() {
            return Err("Apply or discard the cell draft first".into());
        }
        let tab = self.browse();
        let result = tab.result.as_ref().ok_or("No browse data")?;
        let table = tab.table.clone().ok_or("No browse table")?;
        let row = tab.sel_row.ok_or("Select a browse cell first")?;
        let anchor_row = tab.sel_row_start.unwrap_or(row);
        let (start_row, end_row) = (row.min(anchor_row), row.max(anchor_row));
        let (start_col, end_col) = if let Some(col) = tab.sel_col {
            let anchor = tab.sel_col_start.unwrap_or(col);
            (col.min(anchor), col.max(anchor))
        } else { (0, result.columns.len().saturating_sub(1)) };
        let width = rows.first().map_or(0, Vec::len);
        if width == 0 || rows.iter().any(|row| row.len() != width) {
            return Err("Clipboard rows have inconsistent column counts".into());
        }
        if rows.len() == 1 && width == 1 {
            if end_row >= result.rows.len() || end_col >= result.columns.len() {
                return Err("Selection no longer fits the browse page; refresh and retry".into());
            }
            rows = vec![vec![rows[0][0].clone(); end_col - start_col + 1]; end_row - start_row + 1];
        }
        let width = rows[0].len();
        if start_row.saturating_add(rows.len()) > result.rows.len() {
            return Err("Clipboard does not fit in the visible browse page".into());
        }
        if start_col.saturating_add(width) > result.columns.len() {
            return Err("Clipboard does not fit in the visible columns".into());
        }
        let height = rows.len();
        let columns = result.columns[start_col..start_col + width].to_vec();
        let updates = rows.into_iter().enumerate().map(|(i, values)| {
            let rowid = *tab.rowids.get(start_row + i).ok_or("Selected rows are not editable")?;
            Ok((rowid, values))
        }).collect::<Result<Vec<_>, String>>()?;
        db::update_cells_in(self.conn.as_ref().ok_or("No database open")?, &schema, &table, &columns, &updates)?;
        self.log_sql(format!("-- paste {height}x{width} into {}", db::qualified_name(&schema, &table)));
        self.mark_dirty();
        self.refresh_browse()?;
        self.status_msg = format!("Pasted {height}x{width} cells (unwritten)");
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
            schema: self.browse().schema.clone(),
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
                    if rule.schema != self.browse().schema || rule.table != self.browse().table || &rule.column != cname {
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
        let (Some(ri), Some(res)) = (b.sel_row, b.result.as_ref()) else {
            return;
        };
        let ci = b.sel_col.unwrap_or(0);
        let Some(col) = res.columns.get(ci).cloned() else {
            return;
        };
        let cell = match res.values.cell(ri, ci) {
            Ok(cell) => cell,
            Err(e) => { self.clear_browse_editor(); self.error_msg = Some(e); return; }
        };
        let raw = cell.as_ref();
        if let rusqlite::types::ValueRef::Text(bytes) = raw {
            if std::str::from_utf8(bytes).is_err() {
                self.clear_browse_editor();
                self.error_msg = Some("TEXT contains invalid UTF-8; typed copy and SQL export preserve its bytes".into());
                return;
            }
        }
        let value = rusqlite::types::Value::from(raw);
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
        let schema = self.browse().schema.clone();
        if self.has_cell_draft() { return Err("Apply or discard the cell draft first".into()); }
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
            db::list_foreign_keys_in(conn, &schema, &table)?
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
                        db::qualified_name(&schema, &table)
                    ),
                    [rowid],
                    |row| row.get_ref(0).map(db::CellValue::from),
                )
                .map_err(|e| e.to_string())?;
            if value == db::CellValue::Null {
                return Err("NULL foreign key has no parent row".into());
            }
            parts.push(format!(
                "{} = {}",
                db::quote_ident(&key.to_col),
                db::sql_literal(value.as_ref())
            ));
        }
        let filter = parts.join(" AND ");
        let old_index = self.browse_tab_idx;
        self.browse_tabs
            .push(BrowseTabState::new_in(&schema, Some(fk.to_table.clone())));
        self.browse_tab_idx = self.browse_tabs.len() - 1;
        self.browse_mut().filter = filter;
        if let Err(e) = self.refresh_browse() {
            self.browse_tabs.pop();
            self.browse_tab_idx = old_index;
            return Err(e);
        }
        self.tab = MainTab::Browse;
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
            self.sql_completion = crate::sql_completion::Catalog::load(conn);
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

    pub fn form_next_record(&mut self) -> Result<(), String> {
        self.move_form_record(true)
    }

    pub fn form_prev_record(&mut self) -> Result<(), String> {
        self.move_form_record(false)
    }

    fn move_form_record(&mut self, next: bool) -> Result<(), String> {
        if self.has_cell_draft() { return Err("Apply or discard the cell draft first".into()); }
        let tab = self.browse();
        let n = tab.result.as_ref().map_or(0, |r| r.rows.len());
        if n == 0 { return Ok(()); }
        let col = tab.sel_col.unwrap_or(0);
        let row = tab.sel_row.unwrap_or(0).min(n - 1);
        let target = if tab.sel_row.is_none() {
            0
        } else if next && row + 1 == n && tab.offset.saturating_add(n) < tab.total_rows.unwrap_or(0) {
            self.browse_page(true)?;
            0
        } else if !next && row == 0 && tab.offset > 0 {
            self.browse_page(false)?;
            self.browse().result.as_ref().map_or(0, |r| r.rows.len().saturating_sub(1))
        } else if next { (row + 1).min(n - 1) }
        else { row.saturating_sub(1) };
        let tab = self.browse_mut();
        tab.clear_selection();
        if let Some(result) = &tab.result {
            if !result.rows.is_empty() {
                tab.sel_row = Some(target.min(result.rows.len() - 1));
                tab.sel_col = Some(col.min(result.columns.len().saturating_sub(1)));
            }
        }
        self.sync_edit_cell_from_selection();
        Ok(())
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
    fn generated_browse_cells_are_read_only_but_copy_export_and_insert_remain_usable() {
        let mut state=test_state();
        state.conn.as_ref().unwrap().execute_batch("ATTACH ':memory:' AS \"aux.\"\"db\"").unwrap();
        for schema in ["main","temp","aux.\"db"] {
            let table=db::qualified_name(schema,"computed");
            state.conn.as_ref().unwrap().execute_batch(&format!("CREATE TABLE {table}(qty INTEGER,total INTEGER AS(qty*4) STORED,note TEXT); INSERT INTO {table}(rowid,qty,note) VALUES(9,3,'kept')")).unwrap();
            state.refresh_schema().unwrap(); state.switch_browse_table_in(schema,"computed").unwrap();
            state.goto_browse_record(1).unwrap(); state.browse_mut().sel_col=Some(1); state.sync_edit_cell_from_selection();
            assert!(state.edit_cell_generated());
            state.edit_col=" TOTAL ".into(); assert!(state.edit_cell_generated());
            state.edit_col="total".into(); state.edit_type=1; state.edit_value="99".into();
            assert!(state.apply_cell_draft().unwrap_err().contains("read-only"));
            state.discard_cell_draft();
            assert!(state.cut_browse_cells().is_err());
            assert!(state.paste_browse_cells(vec![vec![db::CellValue::Integer(99)]]).is_err());
            assert!(state.copy_browse_cells(false,true).is_err());
            assert_eq!(state.copy_browse_cells(false,false).unwrap().text,"12");
            state.browse_mut().sel_col=None;
            let copy=state.copy_browse_cells(true,false).unwrap();
            assert!(copy.text.starts_with("qty\ttotal\tnote\n3\t12\tkept"));
            let sql=state.copy_browse_cells(false,true).unwrap().text;
            assert!(sql.contains("(\"qty\", \"note\")") && !sql.contains("\"total\""));
            state.conn.as_ref().unwrap().execute_batch(&sql).unwrap();
            state.refresh_browse().unwrap();
            assert_eq!(state.browse().total_rows,Some(2));
            state.open_insert_row().unwrap();
            assert_eq!(state.insert_columns.iter().map(|column|column.name.as_str()).collect::<Vec<_>>(),["qty","note"]);
            state.insert_modes=vec![2,2]; state.insert_values=vec!["5".into(),"inserted".into()];
            state.apply_insert_row().unwrap();
            assert_eq!(state.conn.as_ref().unwrap().query_row(&format!("SELECT total FROM {table} WHERE note='inserted'"),[],|row|row.get::<_,i64>(0)).unwrap(),20);
            state.goto_browse_record(1).unwrap(); state.browse_mut().sel_col=Some(0); state.sync_edit_cell_from_selection();
            assert!(!state.edit_cell_generated());
            state.edit_type=1; state.edit_value="4".into(); state.apply_cell_draft().unwrap();
            assert_eq!(state.conn.as_ref().unwrap().query_row(&format!("SELECT total FROM {table} WHERE rowid=9"),[],|row|row.get::<_,i64>(0)).unwrap(),16);
            state.browse_mut().sel_col=Some(1); state.sync_edit_cell_from_selection();
            let dir=tempfile::tempdir().unwrap(); let path=dir.path().join("computed.json"); state.export_browse(&path,true).unwrap();
            let rows:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            assert_eq!(rows[0]["total"],16);
        }
    }

    #[test]
    fn table_preview_retains_constraints_without_database_queries_or_mutating_pending_edits() {
        let mut state=test_state();
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE records(id INTEGER PRIMARY KEY,v TEXT CONSTRAINT named_check CHECK(length(v)>0),g AS(length(v))); INSERT INTO records VALUES(7,'kept');").unwrap();
        state.refresh_schema().unwrap();
        state.selected=Some(SelectedObject { schema:"main".into(),kind:ObjectKind::Table,name:"records".into() });
        state.open_modify_table_wizard().unwrap();
        state.wizard_cols[1].name="renamed".into(); state.wizard_cols[1].notnull=true;
        state.wizard_table_name="changed".into();
        let before=state.sql_log.text(None); let dirty=state.dirty;
        let preview=state.table_wizard_sql().unwrap();
        assert!(preview.contains("named_check") && preview.contains("length (\"renamed\")") && preview.contains("NOT NULL"),"{preview}");
        for _ in 0..3 { assert_eq!(state.table_wizard_sql().unwrap(),preview); }
        assert_eq!(state.sql_log.text(None),before); assert_eq!(state.dirty,dirty);
        assert!(!state.conn.as_ref().unwrap().is_autocommit());
        assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT v FROM records",[],|row|row.get::<_,String>(0)).unwrap(),"kept");
        state.wizard_cols[1].name="id".into(); assert!(state.table_wizard_sql().unwrap_err().contains("Duplicate"));
        state.wizard_cols[1].name="renamed".into(); state.wizard_cols[1].type_name="TEXT, injected INTEGER".into();
        assert!(state.table_wizard_sql().is_err());
        state.open_create_table_wizard(); assert!(state.wizard_original_cols.is_empty() && state.wizard_table_preview_state.is_none());
        let preview=state.table_wizard_sql().unwrap();
        state.conn.as_ref().unwrap().execute_batch(&preview).unwrap();
        assert_eq!(db::table_column_names(state.conn.as_ref().unwrap(),"new_table").unwrap(),["id","name"]);
    }

    #[test]
    fn ended_transaction_guard_invalidates_browse_identities_and_preserves_cell_draft() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("CREATE TRIGGER reject_update BEFORE UPDATE ON t WHEN new.v='rejected' BEGIN SELECT RAISE(ROLLBACK,'rejected edit'); END;").unwrap();
        state.write_changes().unwrap();
        state.conn.as_ref().unwrap().execute_batch("INSERT INTO t VALUES('pending'); CREATE TABLE pending_schema(v);").unwrap();
        state.mark_dirty();
        state.refresh_schema().unwrap();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        state.add_browse_tab().unwrap();
        state.goto_browse_record(1).unwrap();
        state.edit_value = "rejected".into();
        assert!(state.has_cell_draft());
        assert!(state.apply_cell_draft().unwrap_err().contains("rejected edit"));
        assert!(state.conn.as_ref().unwrap().is_autocommit());
        assert!(state.guard_writable().unwrap_err().contains("previous transaction"));
        assert!(!state.conn.as_ref().unwrap().is_autocommit());
        assert!(!state.dirty);
        assert!(!state.schema.iter().any(|o|o.name=="pending_schema"));
        assert!(state.browse_tabs.iter().all(|t|t.result.is_none() && t.rowids.is_empty() && t.total_rows.is_none() && t.sel_row.is_none()));
        assert_eq!(state.edit_value,"rejected");
        assert!(state.has_cell_draft());
        assert!(state.apply_cell_draft().unwrap_err().contains("Select a browse row"));
        state.discard_cell_draft();
        state.refresh_browse().unwrap();
        state.goto_browse_record(1).unwrap();
        state.edit_value = "corrected".into();
        state.apply_cell_draft().unwrap();
        state.revert_changes().unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][0],"saved");
        assert!(!state.recover_ended_transaction().unwrap());
        state.conn.as_ref().unwrap().execute_batch("ROLLBACK").unwrap();
        state.read_only = true;
        assert!(!state.recover_ended_transaction().unwrap());
        assert!(state.conn.as_ref().unwrap().is_autocommit());
        assert!(state.guard_writable().unwrap_err().contains("read-only"));
        assert!(!AppState::empty().recover_ended_transaction().unwrap());
    }

    #[test]
    fn schema_wizard_fields_reject_sql_tails_before_mutation() {
        for tail in ["DELETE FROM main.t", "PRAGMA foreign_keys=ON", "COMMIT"] {
            let mut state = test_state();
            let foreign_keys = db::read_pragma(state.conn.as_ref().unwrap(), "foreign_keys").unwrap();
            state.refresh_schema().unwrap();
            state.open_create_index_wizard();
            state.wizard_index_name = "rejected".into();
            state.wizard_index_cols = format!("v); {tail}; --");
            let result = state.apply_index_wizard();
            let conn = state.conn.as_ref().unwrap();
            let rows = db::count_rows(conn, "t").unwrap();
            assert!(result.is_err() && rows == 1, "index result={result:?}, rows={rows}, tail={tail}");
            assert!(!conn.is_autocommit());
            assert_eq!(db::read_pragma(conn, "foreign_keys").unwrap(), foreign_keys);
            assert!(!db::list_schema(conn).unwrap().iter().any(|o| o.name == "rejected"));
            assert!(!state.dirty);
            assert_eq!(state.overlay, Some(OverlayKind::EditIndex));
        }
        let mut state = test_state();
        state.refresh_schema().unwrap();
        state.open_create_table_wizard();
        state.wizard_cols = vec![ColDef { default_value: "0); DELETE FROM main.t; --".into(), ..ColDef::new("x", "TEXT") }];
        assert!(state.apply_table_wizard().is_err());
        assert_eq!(db::count_rows(state.conn.as_ref().unwrap(), "t").unwrap(), 1);
        assert!(!db::list_schema(state.conn.as_ref().unwrap()).unwrap().iter().any(|o| o.name == "new_table"));
        state.selected = Some(SelectedObject { schema: "main".into(), kind: ObjectKind::Table, name: "t".into() });
        state.open_modify_table_wizard().unwrap();
        state.wizard_cols.push(ColDef { default_value: "0; DELETE FROM main.t; --".into(), ..ColDef::new("x", "TEXT") });
        state.wizard_col_sources.push(None);
        assert!(state.apply_table_wizard().is_err());
        assert_eq!(db::count_rows(state.conn.as_ref().unwrap(), "t").unwrap(), 1);
        assert_eq!(db::table_column_names(state.conn.as_ref().unwrap(), "t").unwrap(), ["v"]);
        assert!(!state.dirty);
        state.conn.as_ref().unwrap().execute_batch("CREATE INDEX main.kept ON t(v)").unwrap();
        state.refresh_schema().unwrap();
        state.selected = Some(SelectedObject { schema: "main".into(), kind: ObjectKind::Index, name: "kept".into() });
        state.open_modify_index_wizard().unwrap();
        state.wizard_index_cols = "v); COMMIT; DELETE FROM main.t; --".into();
        assert!(state.apply_index_wizard().is_err());
        let conn = state.conn.as_ref().unwrap();
        assert!(!conn.is_autocommit());
        assert_eq!(db::count_rows(conn, "t").unwrap(), 1);
        assert!(db::list_schema(conn).unwrap().iter().any(|o| o.name == "kept"));
        assert!(!state.dirty);
    }

    #[test]
    fn browse_schema_refresh_remaps_column_filters_order_and_widths_by_name() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("CREATE TEMP TABLE same(a INTEGER,b INTEGER,c INTEGER); ATTACH ':memory:' AS aux;
            CREATE TABLE aux.same(a INTEGER,b INTEGER,c INTEGER); INSERT INTO aux.same VALUES(1,2,3),(4,5,6);").unwrap();
        state.refresh_schema().unwrap();
        state.switch_browse_table_in("aux", "same").unwrap();
        state.browse_mut().col_filters = vec![String::new(), "=2".into(), ">0".into()];
        state.browse_mut().column_widths = vec![111.0, 222.0, 333.0];
        state.browse_mut().frozen_cols = Some(2);
        state.browse_mut().set_ordering(vec![db::OrderBy { column: "b".into(), descending: true }, db::OrderBy { column: "c".into(), descending: false }]);
        state.refresh_browse().unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows.len(), 1);
        let mut project = crate::project::ProjectFile::from_state(&state);
        let mut inactive = BrowseTabState::new(Some("t".into()));
        inactive.store_options("aux", "same", state.browse().options());
        project.browse_tabs.push(inactive);
        let json = serde_json::to_string(&project).unwrap();
        state.conn.as_ref().unwrap().execute_batch("ALTER TABLE aux.same DROP COLUMN b").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let exported = dir.path().join("remapped.json");
        state.export_browse(&exported, true).unwrap();
        let rows: serde_json::Value = serde_json::from_slice(&std::fs::read(exported).unwrap()).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 2);
        assert!(rows[0].get("b").is_none());
        state.refresh_schema().unwrap();
        state.refresh_browse().unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().columns, ["a", "c"]);
        assert_eq!(state.browse().result.as_ref().unwrap().rows.len(), 2);
        assert_eq!(state.browse().col_filters, ["", ">0"]);
        assert_eq!(state.browse().column_widths, [111.0, 333.0]);
        assert_eq!(state.browse().frozen_cols, Some(1));
        assert_eq!(state.browse().ordering(), [db::OrderBy { column: "c".into(), descending: false }]);
        assert_eq!(db::table_column_names_in(state.conn.as_ref().unwrap(), "temp", "same").unwrap(), ["a", "b", "c"]);
        let mut project: crate::project::ProjectFile = serde_json::from_str(&json).unwrap();
        AppState::preflight_project_browse(&mut project, state.conn.as_ref().unwrap()).unwrap();
        for options in [project.browse_tabs[0].options(), project.browse_tabs[1].stored_options("aux", "same").unwrap().clone()] {
            assert_eq!(options.column_names, ["a", "c"]);
            assert_eq!(options.col_filters, ["", ">0"]);
            assert_eq!(options.column_widths, [111.0, 333.0]);
            assert_eq!(options.frozen_cols, Some(1));
        }
        state.browse_mut().filter = "b=2".into();
        let before = state.browse().options();
        assert!(state.refresh_browse().is_err());
        assert_eq!(state.browse().options(), before);
        let path = dir.path().join("bad-filter.json");
        assert!(state.export_browse(&path, true).is_err());
        assert!(!path.exists());
        state.browse_mut().filter.clear();
        state.refresh_browse().unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows.len(), 2);
        let mut options = state.browse().options();
        options.remap_columns(&["C".into(), "a".into(), "added".into()]);
        assert_eq!(options.col_filters, [">0", "", ""]);
        assert_eq!(options.column_widths, [333.0, 111.0, 140.0]);
        assert_eq!(options.frozen_cols, Some(0));
        assert_eq!(options.ordering[0].column, "C");
    }

    #[test]
    fn modify_table_rebuilds_type_constraints_and_column_order_without_losing_rows() {
        let mut state = test_state();
        state.refresh_schema().unwrap();
        let index = state.schema.iter().position(|o| o.schema == "main" && o.name == "t").unwrap();
        state.select_schema_node(10_000 + index as u64);
        state.open_modify_table_wizard().unwrap();
        state.wizard_cols[0].type_name = "BLOB".into();
        state.wizard_cols[0].notnull = true;
        state.wizard_cols[0].unique = true;
        state.wizard_cols[0].default_value = "'fallback'".into();
        state.wizard_cols.insert(0, ColDef { default_value: "7".into(), ..ColDef::new("first", "INTEGER") });
        state.wizard_col_sources.insert(0, None);
        state.apply_table_wizard().unwrap();
        let conn = state.conn.as_ref().unwrap();
        assert_eq!(db::table_column_names(conn, "t").unwrap(), ["first", "v"]);
        let row: (i64,String,i64) = conn.query_row("SELECT first,v,rowid FROM t", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(row, (7,"saved".into(),1));
        assert!(conn.execute("INSERT INTO t(v) VALUES('saved')", []).is_err());
        assert!(conn.execute("INSERT INTO t(v) VALUES(NULL)", []).is_err());
        conn.execute("INSERT INTO t DEFAULT VALUES", []).unwrap();
        assert_eq!(conn.query_row("SELECT v FROM t WHERE rowid=2", [], |r| r.get::<_,String>(0)).unwrap(), "fallback");
        assert!(!conn.is_autocommit());
    }

    #[test]
    fn table_wizard_renames_a_column_without_replacing_its_data() {
        let mut state = test_state();
        state.refresh_schema().unwrap();
        let index = state.schema.iter().position(|object| object.schema == "main" && object.name == "t").unwrap();
        state.select_schema_node(10_000 + index as u64);
        state.open_modify_table_wizard().unwrap();
        state.wizard_cols[0].name = "renamed.\"列".into();
        state.apply_table_wizard().unwrap();
        let conn = state.conn.as_ref().unwrap();
        assert_eq!(db::table_column_names(conn, "t").unwrap(), ["renamed.\"列"]);
        assert_eq!(db::browse_table(conn, "t", &[], None, 5, 0).unwrap().rows[0][0], "saved");
        assert!(!conn.is_autocommit());
    }

    #[test]
    fn table_wizard_column_swaps_follow_qualified_browse_plot_and_format_identities() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("CREATE TEMP TABLE t(a,b); ATTACH ':memory:' AS aux;
            CREATE TABLE aux.t(a INTEGER,b INTEGER); INSERT INTO aux.t VALUES(25,50),(26,51);").unwrap();
        state.refresh_schema().unwrap();
        let index = state.schema.iter().position(|o| o.schema == "aux" && o.name == "t").unwrap();
        state.select_schema_node(10_000 + index as u64);
        let options = BrowseTableOptions { column_names: vec!["a".into(), "b".into()],
            col_filters: vec!["=25".into(), "=50".into()], column_widths: vec![211.0, 322.0], frozen_cols: Some(1),
            ordering: vec![db::OrderBy { column: "a".into(), descending: true }], ..Default::default() };
        state.browse_mut().set_options(options.clone());
        state.refresh_browse().unwrap();
        state.browse_mut().store_options("aux", "t", options.clone());
        state.add_browse_tab().unwrap();
        state.browse_mut().set_options(options.clone());
        state.browse_mut().store_options("aux", "t", options.clone());
        state.refresh_browse().unwrap();
        let mut inactive = BrowseTabState::new(Some("t".into()));
        inactive.store_options("aux", "t", options.clone());
        inactive.store_options("temp", "t", options.clone());
        state.browse_tabs.push(inactive);
        state.chart_schema = "aux".into();
        state.chart_table = Some("t".into());
        state.chart_column = "b".into();
        state.cond_col = "a".into();
        state.cond_value = "25".into();
        state.add_cond_format_rule();
        let mut other_rule = state.cond_rules[0].clone();
        other_rule.schema = "temp".into();
        state.cond_rules.push(other_rule);
        state.open_modify_table_wizard().unwrap();
        state.wizard_cols[0].name = "b".into();
        state.wizard_cols[1].name = "a".into();
        state.wizard_table_name = "renamed.\"t".into();
        state.apply_table_wizard().unwrap();
        let mut expected = options.clone();
        expected.column_names = vec!["b".into(), "a".into()];
        expected.ordering[0].column = "b".into();
        for tab in &state.browse_tabs {
            assert_eq!(tab.stored_options("aux", "renamed.\"t"), Some(&expected));
            assert!(tab.stored_options("aux", "t").is_none());
        }
        assert_eq!(state.browse_tabs.last().unwrap().stored_options("temp", "t"), Some(&options));
        assert_eq!(state.browse().options(), expected);
        assert_eq!(state.browse().result.as_ref().unwrap().rows.len(), 1);
        assert_eq!(state.chart_column, "a");
        assert_eq!(state.chart_points, [50.0, 51.0]);
        assert_eq!(state.cond_rules[0].column, "b");
        assert_eq!(state.cond_rules[0].table.as_deref(), Some("renamed.\"t"));
        assert_eq!(state.cond_rules[1].column, "a");
        assert_eq!(state.cond_rules[1].table.as_deref(), Some("t"));
        let project = crate::project::ProjectFile::from_state(&state);
        let mut restored: crate::project::ProjectFile = serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
        AppState::preflight_project_browse(&mut restored, state.conn.as_ref().unwrap()).unwrap();
        assert_eq!(restored.browse_tabs[state.browse_tab_idx].options(), expected);
        assert_eq!(db::browse_table(state.conn.as_ref().unwrap(), "t", &[], None, 5, 0).unwrap().rows[0][0], "saved");
    }

    #[test]
    fn moved_view_settings_follow_actual_output_names_and_keep_explicit_aliases() {
        for source in ["main","aux"] {
            for target in ["main","temp","aux"].into_iter().filter(|target|*target!=source) {
                let mut state=test_state();state.conn.as_ref().unwrap().execute_batch(&format!("ATTACH ':memory:' AS aux;CREATE TABLE {}(a INTEGER,b INTEGER);INSERT INTO {} VALUES(25,50),(26,51);
                    CREATE VIEW {} AS SELECT a,b FROM records;CREATE VIEW {} AS SELECT a AS a,b AS renamed_metric FROM records;",db::qualified_name(source,"records"),db::qualified_name(source,"records"),db::qualified_name(source,"visible"),db::qualified_name(source,"aliased"))).unwrap();state.refresh_schema().unwrap();let row=state.schema.iter().position(|object|object.schema==source && object.name=="records").unwrap();state.select_schema_node(10_000+row as u64);
                let options=BrowseTableOptions {column_names:vec!["a".into(),"b".into()],col_filters:vec!["=25".into(),"=50".into()],column_widths:vec![211.0,322.0],frozen_cols:Some(1),ordering:vec![db::OrderBy {column:"a".into(),descending:true}],..Default::default()};
                state.switch_browse_table_in(source,"visible").unwrap();state.browse_mut().set_options(options.clone());state.browse_mut().store_options(source,"visible",options.clone());state.refresh_browse().unwrap();
                state.add_browse_tab().unwrap();state.switch_browse_table_in(source,"aliased").unwrap();let mut aliases=options.clone();aliases.column_names[1]="renamed_metric".into();state.browse_mut().set_options(aliases.clone());state.browse_mut().store_options(source,"aliased",aliases.clone());state.refresh_browse().unwrap();
                state.add_browse_tab().unwrap();state.switch_browse_table_in(source,"records").unwrap();
                state.chart_schema=source.into();state.chart_table=Some("visible".into());state.chart_column="b".into();
                for view in ["visible","aliased"] {state.cond_rules.push(CondFormatRule {schema:source.into(),table:Some(view.into()),column:"a".into(),op:0,value:"25".into(),color_rgb:0x123456});}
                state.open_modify_table_wizard().unwrap();state.wizard_schema=target.into();state.wizard_table_name="archive".into();state.wizard_cols[0].name="label".into();state.wizard_cols[1].name="metric".into();state.apply_table_wizard().unwrap_or_else(|error|panic!("{source} → {target}: {error}"));
                let mut expected=options.clone();expected.column_names=vec!["label".into(),"metric".into()];expected.ordering[0].column="label".into();
                assert_eq!(state.browse_tabs[0].schema,target);assert_eq!(state.browse_tabs[0].table.as_deref(),Some("visible"));assert_eq!(state.browse_tabs[0].options(),expected);assert_eq!(state.browse_tabs[0].stored_options(target,"visible"),Some(&expected));assert!(state.browse_tabs[0].stored_options(source,"visible").is_none());
                assert_eq!(state.browse_tabs[1].schema,target);assert_eq!(state.browse_tabs[1].table.as_deref(),Some("aliased"));assert_eq!(state.browse_tabs[1].options(),aliases);assert_eq!(state.browse_tabs[1].stored_options(target,"aliased"),Some(&aliases));assert!(state.browse_tabs[1].stored_options(source,"aliased").is_none());
                assert_eq!(state.chart_schema,target);assert_eq!(state.chart_table.as_deref(),Some("visible"));assert_eq!(state.chart_column,"metric");assert_eq!(state.chart_points,[50.0,51.0]);assert_eq!(state.cond_rules[0].schema,target);assert_eq!(state.cond_rules[0].column,"label");assert_eq!(state.cond_rules[1].schema,target);assert_eq!(state.cond_rules[1].column,"a");
                for index in [0,1] {state.switch_browse_tab(index).unwrap();assert_eq!(state.browse().result.as_ref().unwrap().rows.len(),1);assert_eq!(state.browse().result.as_ref().unwrap().rows[0],["25","50"]);}
                let project=crate::project::ProjectFile::from_state(&state);let mut restored:crate::project::ProjectFile=serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();AppState::preflight_project_browse(&mut restored,state.conn.as_ref().unwrap()).unwrap();assert_eq!(restored.browse_tabs[0].options(),expected);assert_eq!(restored.browse_tabs[1].options(),aliases);
            }
        }
    }

    #[test]
    fn table_moves_follow_qualified_browse_plot_format_and_project_identities() {
        for source in ["main", "temp", "aux"] {
            for target in ["main", "temp", "aux"].into_iter().filter(|target| *target != source) {
                let mut state = test_state();
                state.conn.as_ref().unwrap().execute_batch("ATTACH ':memory:' AS aux;
                    CREATE TABLE main.records(a INTEGER,b INTEGER);CREATE TABLE temp.records(a INTEGER,b INTEGER);CREATE TABLE aux.records(a INTEGER,b INTEGER);
                    INSERT INTO main.records VALUES(99,100);INSERT INTO temp.records VALUES(99,100);INSERT INTO aux.records VALUES(99,100);").unwrap();
                state.conn.as_ref().unwrap().execute_batch(&format!("DELETE FROM {};INSERT INTO {} VALUES(25,50),(26,51)",db::qualified_name(source,"records"),db::qualified_name(source,"records"))).unwrap();
                state.refresh_schema().unwrap();
                let index=state.schema.iter().position(|object|object.schema==source && object.name=="records").unwrap();
                state.select_schema_node(10_000+index as u64);
                let options=BrowseTableOptions {column_names:vec!["a".into(),"b".into()],col_filters:vec!["=25".into(),"=50".into()],column_widths:vec![211.0,322.0],frozen_cols:Some(1),ordering:vec![db::OrderBy {column:"a".into(),descending:true}],..Default::default()};
                state.browse_mut().set_options(options.clone());state.refresh_browse().unwrap();state.browse_mut().store_options(source,"records",options.clone());
                state.add_browse_tab().unwrap();state.browse_mut().set_options(options.clone());state.browse_mut().store_options(source,"records",options.clone());state.refresh_browse().unwrap();
                let other=["main","temp","aux"].into_iter().find(|schema|*schema!=source && *schema!=target).unwrap();
                let mut inactive=BrowseTabState::new(Some("t".into()));inactive.store_options(source,"records",options.clone());inactive.store_options(other,"records",options.clone());state.browse_tabs.push(inactive);
                state.chart_schema=source.into();state.chart_table=Some("records".into());state.chart_column="b".into();state.cond_col="a".into();state.cond_value="25".into();state.add_cond_format_rule();
                let mut other_rule=state.cond_rules[0].clone();other_rule.schema=other.into();state.cond_rules.push(other_rule);
                state.open_modify_table_wizard().unwrap();state.wizard_schema=target.into();state.wizard_table_name="archive.\"t".into();state.wizard_cols[0].name="label".into();
                let preview=state.table_wizard_sql().unwrap();assert!(preview.contains(&db::qualified_name(target,"archive.\"t")),"{preview}");
                state.apply_table_wizard().unwrap_or_else(|error|panic!("{source} → {target}: {error}"));
                let mut expected=options.clone();expected.column_names[0]="label".into();expected.ordering[0].column="label".into();
                for tab in &state.browse_tabs {assert_eq!(tab.stored_options(target,"archive.\"t"),Some(&expected));assert!(tab.stored_options(source,"records").is_none());}
                assert_eq!(state.browse_tabs.last().unwrap().stored_options(other,"records"),Some(&options));
                assert_eq!(state.browse().schema,target);assert_eq!(state.browse().table.as_deref(),Some("archive.\"t"));assert_eq!(state.browse().options(),expected);assert_eq!(state.browse().result.as_ref().unwrap().rows.len(),1);
                assert_eq!(state.chart_schema,target);assert_eq!(state.chart_table.as_deref(),Some("archive.\"t"));assert_eq!(state.chart_points,[50.0,51.0]);
                assert_eq!(state.cond_rules[0].schema,target);assert_eq!(state.cond_rules[0].column,"label");assert_eq!(state.cond_rules[1].schema,other);assert_eq!(state.cond_rules[1].column,"a");
                let project=crate::project::ProjectFile::from_state(&state);let mut restored:crate::project::ProjectFile=serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();AppState::preflight_project_browse(&mut restored,state.conn.as_ref().unwrap()).unwrap();assert_eq!(restored.browse_tabs[state.browse_tab_idx].options(),expected);
                assert_eq!(state.conn.as_ref().unwrap().query_row(&format!("SELECT a FROM {}",db::qualified_name(target,"records")),[],|row|row.get::<_,i64>(0)).unwrap(),99);
            }
        }
    }

    #[test]
    fn column_rename_reports_saved_schema_when_a_raw_filter_needs_manual_update() {
        let mut state = test_state();
        state.refresh_schema().unwrap();
        let index = state.schema.iter().position(|o| o.schema == "main" && o.name == "t").unwrap();
        state.select_schema_node(10_000 + index as u64);
        state.browse_mut().filter = "v = 'saved'".into();
        state.refresh_browse().unwrap();
        state.goto_browse_record(1).unwrap();
        state.open_modify_table_wizard().unwrap();
        state.wizard_cols[0].name = "renamed".into();
        state.apply_table_wizard().unwrap();
        assert!(state.status_msg.contains("saved (unwritten); browse unavailable:"), "{}", state.status_msg);
        assert!(state.overlay.is_none());
        assert!(state.dirty);
        assert_eq!(state.browse().filter, "v = 'saved'");
        assert_eq!(state.browse().column_names, ["renamed"]);
        assert!(state.browse().result.is_none());
        assert!(state.browse().rowids.is_empty());
        assert!(state.browse().total_rows.is_none());
        assert!(state.browse().sel_row.is_none());
        assert!(!state.has_cell_draft());
        assert!(state.ddl_draft.contains("renamed"));
        state.browse_mut().filter.clear();
        state.refresh_browse().unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][0], "saved");
    }

    #[test]
    fn namespace_table_wizards_keep_data_options_drafts_and_identity() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("CREATE TEMP TABLE t(v); INSERT INTO temp.t VALUES('temp');
            ATTACH ':memory:' AS \"aux.\"\"db\";
            CREATE TABLE \"aux.\"\"db\".t(v); INSERT INTO \"aux.\"\"db\".t VALUES(25);").unwrap();
        state.refresh_schema().unwrap();
        let index = state.schema.iter().position(|o| o.schema == "aux.\"db" && o.name == "t").unwrap();
        state.select_schema_node(10_000 + index as u64);
        state.browse_mut().filter = "v=25".into();
        state.browse_mut().column_widths = vec![211.0];
        let options = state.browse().options();
        state.browse_mut().store_options("aux.\"db", "t", options);
        state.add_browse_tab().unwrap();
        state.chart_schema = "aux.\"db".into();
        state.chart_table = Some("t".into());
        state.chart_column = "v".into();
        state.cond_col = "v".into();
        state.cond_value = "25".into();
        state.add_cond_format_rule();
        state.open_modify_table_wizard().unwrap();
        assert_eq!(state.wizard_schema, "aux.\"db");
        assert_eq!(state.wizard_cols[0].type_name, "");
        assert!(state.column_type_names().contains(&String::new()));
        state.wizard_cols.push(ColDef::new("extra", "TEXT"));
        state.wizard_col_sources.push(None);
        state.wizard_table_name = "renamed.\"t".into();
        state.apply_table_wizard().unwrap();
        assert!(state.browse_tabs.iter().all(|t| t.schema == "aux.\"db" && t.table.as_deref() == Some("renamed.\"t")));
        assert!(state.browse_tabs.iter().all(|t| t.stored_options("aux.\"db", "t").is_none()));
        assert_eq!(state.browse_tabs[0].stored_options("aux.\"db", "renamed.\"t").unwrap().column_widths, [211.0]);
        assert_eq!(state.selected.as_ref().unwrap().schema, "aux.\"db");
        assert_eq!(state.selected.as_ref().unwrap().name, "renamed.\"t");
        assert!(state.ddl_draft.contains("renamed."));
        assert_eq!(state.chart_table.as_deref(), Some("renamed.\"t"));
        assert_eq!(state.chart_points, [25.0]);
        assert_eq!(state.cond_rules[0].table.as_deref(), Some("renamed.\"t"));
        assert_eq!(db::browse_table(state.conn.as_ref().unwrap(), "t", &[], None, 5, 0).unwrap().rows[0][0], "saved");
        assert_eq!(db::browse_table_in(state.conn.as_ref().unwrap(), "temp", "t", &[], None, 5, 0).unwrap().rows[0][0], "temp");
        state.goto_browse_record(1).unwrap();
        state.edit_type = 3;
        state.edit_value = "keep draft".into();
        state.open_create_table_wizard();
        assert_eq!(state.wizard_schema, "main");
        assert!(state.apply_table_wizard().unwrap_err().contains("cell draft"));
        assert!(!db::list_schema(state.conn.as_ref().unwrap()).unwrap().iter().any(|o| o.name == "new_table"));
        state.discard_cell_draft();
        state.ddl_draft.push_str(" -- retained SQL draft");
        state.wizard_schema = "temp".into();
        state.wizard_table_name = "created".into();
        state.apply_table_wizard().unwrap();
        assert!(state.ddl_draft.ends_with("retained SQL draft"));
        assert_eq!(state.browse().schema, "temp");
        assert!(state.schema.iter().any(|o| o.schema == "temp" && o.name == "created"));
        state.open_modify_table_wizard().unwrap();
        state.wizard_schema = "main".into();
        state.apply_table_wizard().unwrap();
        assert_eq!(db::table_columns_in(state.conn.as_ref().unwrap(), "main", "created").unwrap().len(), 2);
        assert!(!state.schema.iter().any(|object|object.schema=="temp" && object.name=="created"));
    }

    #[test]
    fn namespace_index_wizards_read_real_pragmas_and_keep_original_on_failure() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("CREATE TEMP TABLE t(v TEXT); ATTACH ':memory:' AS \"aux.\"\"db\";
            CREATE TABLE \"aux.\"\"db\".t(v TEXT);
            CREATE TABLE pragma_index_list(fake); CREATE TABLE pragma_index_xinfo(fake);
            CREATE TEMP TABLE pragma_index_list(fake); CREATE TEMP TABLE pragma_index_xinfo(fake);
            CREATE VIEW main.not_a_table AS SELECT 1;").unwrap();
        for schema in ["main", "temp", "aux.\"db"] {
            db::create_index_in(state.conn.as_ref().unwrap(), schema, "same.\"index", "t", "v COLLATE NOCASE DESC", true, "").unwrap();
        }
        state.refresh_schema().unwrap();
        assert!(!state.index_tables().contains(&("main", "not_a_table")));
        state.selected = Some(SelectedObject { schema: "aux.\"db".into(), kind: ObjectKind::Index, name: "same.\"index".into() });
        state.open_modify_index_wizard().unwrap();
        assert_eq!(state.wizard_schema, "aux.\"db");
        assert!(state.wizard_index_unique);
        assert!(state.wizard_index_cols.contains("NOCASE"));
        assert!(state.wizard_index_cols.contains("DESC"));
        let original_columns = state.wizard_index_cols.clone();
        state.wizard_schema = "temp".into();
        state.wizard_index_name = "moved.\"index".into();
        state.wizard_index_cols = "missing_column".into();
        assert!(state.apply_index_wizard().is_err());
        assert_eq!(state.overlay, Some(OverlayKind::EditIndex));
        assert!(db::list_schema_in(state.conn.as_ref().unwrap(), "aux.\"db").unwrap().iter().any(|o| o.name == "same.\"index"));
        state.wizard_index_cols = original_columns;
        state.apply_index_wizard().unwrap();
        assert!(!state.schema.iter().any(|o| o.schema == "aux.\"db" && o.name == "same.\"index"));
        assert!(state.schema.iter().any(|o| o.schema == "temp" && o.name == "moved.\"index"));
        for schema in ["main", "temp"] { assert!(state.schema.iter().any(|o| o.schema == schema && o.name == "same.\"index")); }
        assert_eq!(state.selected.as_ref().unwrap().schema, "temp");
        state.conn.as_ref().unwrap().execute_batch("CREATE INDEX \"aux.\"\"db\".partial_ix ON t(v) WHERE v IS NOT NULL;
            CREATE INDEX \"aux.\"\"db\".expression_ix ON t(length(v));").unwrap();
        state.refresh_schema().unwrap();
        for name in ["partial_ix", "expression_ix"] {
            state.selected = Some(SelectedObject { schema: "aux.\"db".into(), kind: ObjectKind::Index, name: name.into() });
            state.open_modify_index_wizard().unwrap();
            assert_eq!(state.wizard_index_where, if name == "partial_ix" { "v IS NOT NULL" } else { "" });
            state.apply_index_wizard().unwrap();
        }
    }

    #[test]
    fn partial_expression_index_edit_reads_live_sql_and_preserves_semantics() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("ATTACH ':memory:' AS \"aux.\"\"db\"").unwrap();
        let keys = "coalesce(lower(v), '; WHERE)') COLLATE NOCASE DESC, length(v) -- key ) WHERE";
        for schema in ["main", "temp", "aux.\"db"] {
            let table = db::qualified_name(schema, "expr_t");
            let index = db::qualified_name(schema, "expr.\"index");
            state.conn.as_ref().unwrap().execute_batch(&format!("CREATE TABLE {table}(v TEXT, flag INTEGER);
                INSERT INTO {table} VALUES('Alpha',1),('alpha',0),(NULL,1);
                CREATE INDEX {index} ON expr_t(v);")).unwrap();
            state.refresh_schema().unwrap();
            state.selected = Some(SelectedObject { schema: schema.into(), kind: ObjectKind::Index, name: "expr.\"index".into() });
            // The schema list is deliberately stale; opening must read the actual definition.
            state.conn.as_ref().unwrap().execute_batch(&format!("DROP INDEX {index}; CREATE UNIQUE INDEX {index} ON expr_t(\n{keys}\n) WHERE flag=1;")).unwrap();
            state.open_modify_index_wizard().unwrap();
            assert_eq!(state.wizard_index_cols, keys);
            assert_eq!(state.wizard_index_where, "flag=1");
            assert!(state.wizard_index_unique);
            let sql_query = format!("SELECT sql FROM {}.sqlite_schema WHERE type='index' AND name=?1", db::quote_ident(schema));
            let before: String = state.conn.as_ref().unwrap().query_row(&sql_query, ["expr.\"index"], |r| r.get(0)).unwrap();
            for predicate in ["missing_column>0".to_string(), format!("flag=1; COMMIT; DELETE FROM {table}; --")] {
                state.wizard_index_where = predicate;
                assert!(state.apply_index_wizard().is_err());
                let conn = state.conn.as_ref().unwrap();
                let actual: String = conn.query_row(&sql_query, ["expr.\"index"], |r| r.get(0)).unwrap();
                assert_eq!(actual, before);
                assert!(!conn.is_autocommit());
                assert_eq!(state.overlay, Some(OverlayKind::EditIndex));
            }
            state.wizard_index_where = "flag=1".into();
            state.apply_index_wizard().unwrap();
            let conn = state.conn.as_ref().unwrap();
            assert!(conn.execute(&format!("INSERT INTO {table} VALUES('ALPHA',1)"), []).is_err());
            conn.execute(&format!("INSERT INTO {table} VALUES('ALPHA',0)"), []).unwrap();
            assert_eq!(db::count_browse_rows_in(conn, schema, "expr_t", None).unwrap(), 4);
            state.open_modify_index_wizard().unwrap();
            assert_eq!(state.wizard_index_cols, keys);
            assert_eq!(state.wizard_index_where, "flag=1");
            assert!(state.wizard_index_unique);
        }
        state.open_create_index_wizard();
        assert!(state.wizard_index_where.is_empty());
    }

    #[test]
    fn browse_namespaces_isolate_data_options_formats_and_clipboard_sql() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("CREATE TEMP TABLE t(v); INSERT INTO temp.t VALUES('temp');
            ATTACH ':memory:' AS \"aux.\"\"db\"; CREATE TABLE \"aux.\"\"db\".t(v); INSERT INTO \"aux.\"\"db\".t VALUES('attached');").unwrap();
        state.refresh_schema().unwrap();
        state.switch_browse_table("t").unwrap();
        state.browse_mut().filter = "v='saved'".into();
        state.browse_mut().column_widths = vec![123.0];
        state.cond_col = "v".into();
        state.cond_value = "saved".into();
        state.add_cond_format_rule();
        state.switch_browse_table_in("aux.\"db", "t").unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][0], "attached");
        assert!(state.browse().filter.is_empty());
        assert!(state.cell_format_colors()[0][0].is_none());
        state.goto_browse_record(1).unwrap();
        state.edit_type = 3;
        state.edit_value = "edited attached".into();
        assert!(state.switch_browse_table("t").is_err());
        assert_eq!(state.browse().schema, "aux.\"db");
        state.apply_cell_draft().unwrap();
        state.goto_browse_record(1).unwrap();
        state.paste_browse_cells(vec![vec![db::CellValue::Text(b"pasted attached".to_vec())]]).unwrap();
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_col = Some(0);
        let sql = state.copy_browse_cells(false, true).unwrap().text;
        assert!(sql.starts_with("INSERT INTO \"aux.\"\"db\".\"t\""));
        state.browse_mut().filter = "v LIKE 'pasted%'".into();
        state.browse_mut().column_widths = vec![222.0];
        state.browse_mut().frozen_cols = Some(1);
        state.switch_browse_table_in("temp", "t").unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][0], "temp");
        state.open_insert_row().unwrap();
        state.insert_modes[0] = 2;
        state.insert_values[0] = "temp inserted".into();
        state.apply_insert_row().unwrap();
        assert_eq!(state.browse().total_rows, Some(2));
        state.switch_browse_table("t").unwrap();
        assert_eq!(state.browse().filter, "v='saved'");
        assert_eq!(state.browse().column_widths, [123.0]);
        assert!(state.cell_format_colors()[0][0].is_some());
        state.switch_browse_table_in("aux.\"db", "t").unwrap();
        assert_eq!(state.browse().filter, "v LIKE 'pasted%'");
        assert_eq!(state.browse().column_widths, [222.0]);
        assert_eq!(state.browse().frozen_cols, Some(1));
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][0], "pasted attached");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("attached.json");
        state.export_browse(&path, true).unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(&path).unwrap()).unwrap()[0]["v"], "pasted attached");
        state.add_browse_tab().unwrap();
        assert_eq!(state.browse().schema, "aux.\"db");
        state.conn.as_ref().unwrap().execute_batch("COMMIT; DETACH \"aux.\"\"db\";").unwrap();
        assert!(state.refresh_browse().is_err());
        assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT v FROM main.t", [], |r| r.get::<_, String>(0)).unwrap(), "saved");
    }

    #[test]
    fn namespace_fk_navigation_and_save_as_keep_connection_identity() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("ATTACH ':memory:' AS aux;
            CREATE TABLE main.parent(main_id PRIMARY KEY, label); INSERT INTO main.parent VALUES(1,'main');
            CREATE TABLE aux.parent(aux_id PRIMARY KEY, label); INSERT INTO aux.parent VALUES(1,'attached');
            CREATE TABLE aux.child(parent_id REFERENCES parent); INSERT INTO aux.child VALUES(1);
            CREATE TEMP TABLE parent(temp_id PRIMARY KEY, label); INSERT INTO temp.parent VALUES(1,'temp');
            CREATE TEMP TABLE child(parent_id REFERENCES parent); INSERT INTO temp.child VALUES(1);
            CREATE TEMP TABLE pragma_table_info(shadow);").unwrap();
        state.refresh_schema().unwrap();
        for (schema, value) in [("aux", "attached"), ("temp", "temp")] {
            state.switch_browse_table_in(schema, "child").unwrap();
            state.goto_browse_record(1).unwrap();
            state.navigate_fk().unwrap();
            assert_eq!(state.browse().schema, schema);
            assert_eq!(state.browse().table.as_deref(), Some("parent"));
            assert_eq!(state.browse().result.as_ref().unwrap().rows[0][1], value);
            assert_eq!(state.browse().title, db::qualified_name(schema, "parent"));
        }
        state.browse_mut().table = None;
        state.add_browse_tab().unwrap();
        assert_eq!(state.browse().schema, "main");
        state.switch_browse_table_in("aux", "parent").unwrap();
        state.goto_browse_record(1).unwrap();
        state.edit_type = 1;
        state.edit_value = "2".into();
        let dir = tempfile::tempdir().unwrap();
        let copy = dir.path().join("copy.db");
        assert!(state.save_as(copy.to_str().unwrap()).is_err());
        assert!(!copy.exists());
        state.discard_cell_draft();
        state.chart_schema = "aux".into();
        state.chart_table = Some("parent".into());
        state.chart_column = "aux_id".into();
        state.refresh_chart().unwrap();
        assert_eq!(state.chart_points, [1.0]);
        state.save_as(copy.to_str().unwrap()).unwrap();
        assert!(state.chart_table.is_none() && state.chart_points.is_empty());
        assert_eq!(state.schema_names, ["main"]);
        assert!(state.browse_tabs.iter().all(|tab| tab.schema == "main" && tab.schema_table_options.is_empty()));
        assert!(state.browse().table.is_none() && state.browse().result.is_none() && state.browse().rowids.is_empty());
        assert!(state.edit_col.is_empty());
        state.switch_browse_table("parent").unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][1], "main");
    }

    #[test]
    fn structure_catalog_includes_attached_and_temp_objects() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("CREATE TEMP TABLE temp_only(x);
            ATTACH ':memory:' AS \"aux.\"\"db\";
            CREATE TABLE \"aux.\"\"db\".attached_only(x);
            ATTACH ':memory:' AS empty_db;").unwrap();
        state.refresh_schema().unwrap();
        assert!(state.schema.iter().any(|o| o.name == "attached_only"));
        assert!(state.schema.iter().any(|o| o.name == "temp_only"));
        assert_eq!(state.schema_names, ["main", "temp", "aux.\"db", "empty_db"]);
        assert_eq!(state.index_tables(), [("main", "t"), ("temp", "temp_only"), ("aux.\"db", "attached_only")]);
        let attached = state.schema.iter().position(|o| o.schema == "aux.\"db" && o.name == "attached_only").unwrap();
        state.select_schema_node(10_000 + attached as u64);
        assert_eq!(state.selected.as_ref().unwrap().schema, "aux.\"db");
        let project = crate::project::ProjectFile::from_state(&state);
        let mut restored = AppState::empty();
        project.apply_to(&mut restored);
        assert_eq!(restored.selected.as_ref().unwrap().schema, "aux.\"db");
        assert_eq!(restored.ddl_draft, state.ddl_draft);
        assert!(state.open_modify_table_wizard().is_ok());
        assert_eq!(state.browse().schema, "aux.\"db");
        assert_eq!(state.browse().table.as_deref(), Some("attached_only"));
        assert!(!state.schema_selection_has_draft(1));
        state.ddl_draft.push_str(" -- retain");
        assert!(state.schema_selection_has_draft(1));
        assert!(!state.schema_selection_has_draft(u64::MAX));
        state.conn.as_ref().unwrap().execute_batch("COMMIT; DETACH empty_db; DETACH \"aux.\"\"db\";").unwrap();
        state.refresh_schema().unwrap();
        assert_eq!(state.schema_names, ["main", "temp"]);
        assert!(!state.schema.iter().any(|o| o.name == "attached_only"));
        assert!(state.ddl_draft.ends_with(" -- retain"));
        let dir = tempfile::tempdir().unwrap();
        state.save_as(dir.path().join("copy.db").to_str().unwrap()).unwrap();
        assert_eq!(state.schema_names, ["main"]);
        assert!(!state.schema.iter().any(|o| o.schema == "temp"));
        state.close_db();
        assert!(state.schema_names.is_empty());
        assert!(state.ddl_draft.is_empty());
        state.ddl_draft = "saved earlier definition".into();
        state.new_memory_db().unwrap();
        assert!(state.ddl_draft.is_empty());
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
    fn plot_namespaces_keep_quoted_identity_and_clear_failed_samples() {
        let mut state = test_state();
        let conn = state.conn.as_ref().unwrap();
        conn.execute_batch("ATTACH ':memory:' AS \"aux.\"\"db\";").unwrap();
        for (schema, value) in [("main", 11), ("temp", 22), ("aux.\"db", 33)] {
            conn.execute_batch(&format!("CREATE TABLE {}(\"v\"\"alue\"); INSERT INTO {} VALUES({value}),('2.5'),('bad'),('1e100'),(NULL),(X'31');",
                db::qualified_name(schema, "same.\"table"), db::qualified_name(schema, "same.\"table"))).unwrap();
        }
        assert_eq!(db::numeric_series(conn, "same.\"table", "v\"alue", 1).unwrap(), [11.0]);
        assert_eq!(db::numeric_series_in(conn, "temp", "same.\"table", "v\"alue", 1).unwrap(), [22.0]);
        state.chart_schema = "aux.\"db".into();
        state.chart_table = Some("same.\"table".into());
        state.chart_column = "v\"alue".into();
        state.refresh_chart().unwrap();
        assert_eq!(state.chart_points, [33.0, 2.5]);
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE main.generated(v, doubled AS (v*2)); INSERT INTO main.generated(v) VALUES(5);").unwrap();
        assert_eq!(db::numeric_series(state.conn.as_ref().unwrap(), "generated", "DOUBLED", 10).unwrap(), [10.0]);
        state.chart_column = "missing".into();
        assert!(state.refresh_chart().is_err());
        assert!(state.chart_points.is_empty());
        state.chart_column = "v\"alue".into();
        state.refresh_chart().unwrap();
        state.conn.as_ref().unwrap().execute_batch("COMMIT; DETACH \"aux.\"\"db\";").unwrap();
        assert!(state.refresh_chart().is_err());
        assert!(state.chart_points.is_empty());
        state.close_db();
        assert_eq!(state.chart_schema, "main");
        assert!(state.chart_table.is_none());
    }

    fn csv_batch_state(contents:&[(&str,&str)]) -> (tempfile::TempDir,AppState) {
        let directory=tempfile::tempdir().unwrap();let paths=contents.iter().map(|(name,text)| {let path=directory.path().join(name);std::fs::write(&path,text).unwrap();path}).collect();
        let mut state=test_state();assert!(state.open_csv_files(paths));state.csv_has_header=true;state.csv_files.as_mut().unwrap().separate=true;
        (directory,state)
    }

    #[test]
    fn csv_batch_no_and_yes_all_import_only_accepted_files_and_preserve_pending_changes() {
        use crate::csv_files::Answer;
        let (_directory,mut state)=csv_batch_state(&[("first.csv","v\none\n"),("second.csv","v\ntwo\n"),("third.csv","v\nthree\n")]);
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE first(v TEXT);INSERT INTO first VALUES('old');CREATE TABLE second(v TEXT);INSERT INTO second VALUES('old');CREATE TABLE third(v TEXT);INSERT INTO third VALUES('old');INSERT INTO t VALUES('pending');").unwrap();
        state.start_csv_import_work().and_then(|_| state.settle_csv_test()).unwrap();assert!(state.csv_work.as_ref().unwrap().pending.is_some());assert_eq!(state.csv_work.as_ref().unwrap().target(),"first");
        state.continue_csv_import_work(Some(Answer::No)).and_then(|_| state.settle_csv_test()).unwrap();assert_eq!(state.csv_work.as_ref().unwrap().target(),"second");assert!(state.csv_files.as_ref().unwrap().files[0].done);
        state.continue_csv_import_work(Some(Answer::YesAll)).and_then(|_| state.settle_csv_test()).unwrap();assert!(state.csv_work.is_none());assert!(state.csv_files.is_none());assert!(state.overlay.is_none());
        let conn=state.conn.as_ref().unwrap();assert_eq!(db::count_rows(conn,"first").unwrap(),1);assert_eq!(db::count_rows(conn,"second").unwrap(),2);assert_eq!(db::count_rows(conn,"third").unwrap(),2);assert_eq!(db::count_rows(conn,"t").unwrap(),2);assert!(!conn.is_autocommit());assert!(state.dirty);
        assert!(state.prefs.csv_separate_tables);assert!(state.prefs.csv_import.as_ref().unwrap().has_header);
        let (_directory,mut merged)=csv_batch_state(&[("one.csv","v\none\n"),("two.csv","v\ntwo\n"),("three.csv","v\nthree\n")]);
        merged.csv_files.as_mut().unwrap().separate=false;merged.csv_table="merged".into();
        merged.start_csv_import_work().and_then(|_| merged.settle_csv_test()).unwrap();
        assert!(merged.csv_work.is_none());assert!(merged.overlay.is_none());
        assert_eq!(merged.conn.as_ref().unwrap().query_row("SELECT group_concat(v,',') FROM merged",[],|row|row.get::<_,String>(0)).unwrap(),"one,two,three");
    }

    #[test]
    fn csv_batch_cancel_and_retry_keep_unselected_files_and_never_duplicate_completed_files() {
        use crate::csv_files::Answer;
        let (_directory,mut state)=csv_batch_state(&[("first.csv","v\none\n"),("second.csv","v\ntwo\n"),("third.csv","v\nthree\n")]);
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE second(v TEXT);INSERT INTO second VALUES('old')").unwrap();state.csv_files.as_mut().unwrap().files[2].selected=false;
        state.start_csv_import_work().and_then(|_| state.settle_csv_test()).unwrap();assert_eq!(state.csv_work.as_ref().unwrap().target(),"second");assert_eq!(db::count_rows(state.conn.as_ref().unwrap(),"first").unwrap(),1);
        state.continue_csv_import_work(Some(Answer::Cancel)).and_then(|_| state.settle_csv_test()).unwrap();assert!(state.csv_work.is_none());assert_eq!(state.overlay,Some(OverlayKind::ImportCsv));assert_eq!(state.csv_files.as_ref().unwrap().queue(),[1]);
        state.start_csv_import_work().and_then(|_| state.settle_csv_test()).unwrap();state.continue_csv_import_work(Some(Answer::Yes)).and_then(|_| state.settle_csv_test()).unwrap();assert!(state.csv_work.is_none());assert!(state.overlay.is_some());assert!(state.csv_files.as_ref().unwrap().queue().is_empty());
        state.csv_files.as_mut().unwrap().files[2].selected=true;state.start_csv_import_work().and_then(|_| state.settle_csv_test()).unwrap();assert!(state.overlay.is_none());assert!(state.csv_files.is_none());
        let conn=state.conn.as_ref().unwrap();assert_eq!(db::count_rows(conn,"first").unwrap(),1);assert_eq!(db::count_rows(conn,"second").unwrap(),2);assert_eq!(db::count_rows(conn,"third").unwrap(),1);
    }

    #[test]
    fn csv_batch_failed_file_is_retryable_without_reimporting_successful_files() {
        let (directory,mut state)=csv_batch_state(&[("first.csv","v\none\n"),("second.csv","v\n\"broken")]);
        state.conn.as_ref().unwrap().execute("INSERT INTO t VALUES('pending')",[]).unwrap();
        let error=state.start_csv_import_work().and_then(|_| state.settle_csv_test()).unwrap_err();assert!(error.contains("second.csv") && error.contains("Unterminated"));assert!(state.csv_work.is_none());assert_eq!(state.csv_files.as_ref().unwrap().queue(),[1]);
        let conn=state.conn.as_ref().unwrap();assert_eq!(db::count_rows(conn,"first").unwrap(),1);assert_eq!(db::count_rows(conn,"t").unwrap(),2);assert!(!conn.is_autocommit());
        std::fs::write(directory.path().join("second.csv"),"v\nfixed\n").unwrap();state.start_csv_import_work().and_then(|_| state.settle_csv_test()).unwrap();assert!(state.overlay.is_none());
        let conn=state.conn.as_ref().unwrap();assert_eq!(db::count_rows(conn,"first").unwrap(),1);assert_eq!(conn.query_row("SELECT v FROM second",[],|row|row.get::<_,String>(0)).unwrap(),"fixed");
    }

    #[test]
    fn csv_batch_confirmation_uses_snapshot_and_cancel_keeps_current_unfinished_file() {
        use crate::csv_files::Answer;
        let (directory,mut state)=csv_batch_state(&[("first.csv","v\nunselected\n"),("second.csv","v\nconfirmed\n")]);
        state.csv_files.as_mut().unwrap().files[0].selected=false;
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE second(v TEXT)").unwrap();
        state.start_csv_import_work().and_then(|_| state.settle_csv_test()).unwrap();state.refresh_csv_preview();
        assert_eq!(state.csv_preview.as_ref().unwrap().as_ref().unwrap().rows[0][0],"confirmed");
        std::fs::write(directory.path().join("second.csv"),"v\nchanged after confirmation\n").unwrap();
        state.csv_preview=None;state.refresh_csv_preview();
        assert_eq!(state.csv_preview.as_ref().unwrap().as_ref().unwrap().rows[0][0],"confirmed");
        state.continue_csv_import_work(Some(Answer::Cancel)).and_then(|_| state.settle_csv_test()).unwrap();
        assert_eq!(state.csv_files.as_ref().unwrap().current,1);assert_eq!(state.csv_table,"second");
        state.start_csv_import_work().and_then(|_| state.settle_csv_test()).unwrap();state.continue_csv_import_work(Some(Answer::Yes)).and_then(|_| state.settle_csv_test()).unwrap();
        assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT v FROM second",[],|row|row.get::<_,String>(0)).unwrap(),"changed after confirmation");
        assert_eq!(state.csv_files.as_ref().unwrap().visible(),[0]);
    }

    #[cfg(unix)]
    #[test]
    fn csv_batch_retains_native_file_paths_without_lossy_roundtrip() {
        use std::os::unix::ffi::OsStringExt;
        let directory=tempfile::tempdir().unwrap();
        let path=directory.path().join(std::ffi::OsString::from_vec(b"native-\xff.csv".to_vec()));
        let written=std::fs::write(&path,"v\n日本\n");
        let mut state=test_state();assert!(state.open_csv_files(vec![path.clone()]));
        state.csv_has_header=true;state.csv_table="native_path".into();
        assert_eq!(state.selected_csv_path(),path.as_path());
        if let Err(error)=written {
            // macOS filesystems reject invalid UTF-8 names before a file can exist.
            assert!(cfg!(target_os="macos") && error.raw_os_error()==Some(92),"{error}");
            assert!(state.start_csv_import_work().and_then(|_| state.settle_csv_test()).is_err());
            assert!(!state.conn.as_ref().unwrap().is_autocommit());
            assert!(db::table_columns(state.conn.as_ref().unwrap(),"native_path").unwrap().is_empty());
            return;
        }
        state.refresh_csv_preview();
        assert_eq!(state.csv_preview.as_ref().unwrap().as_ref().unwrap().rows[0][0],"日本");
        state.start_csv_import_work().and_then(|_| state.settle_csv_test()).unwrap();
        assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT v FROM native_path",[],|row|row.get::<_,String>(0)).unwrap(),"日本");
    }

    #[test]
    fn csv_cancel_does_not_hide_simultaneous_sqlite_errors_or_transaction_rollback() {
        use crate::csv_files::Answer;
        for policy in ["ABORT", "ROLLBACK"] {
            let (_directory, mut state) = csv_batch_state(&[("first.csv", "v\none\n"), ("second.csv", "v\nblocked\n")]);
            let conn = state.conn.as_ref().unwrap();
            conn.execute_batch("CREATE TABLE second(v TEXT);INSERT INTO second VALUES('saved');COMMIT;BEGIN;INSERT INTO t VALUES('pending')").unwrap();
            let (entered, waiting) = std::sync::mpsc::channel();
            let (release, resume) = std::sync::mpsc::channel();
            conn.create_scalar_function("wait_csv", 0, rusqlite::functions::FunctionFlags::SQLITE_UTF8, move |_| {
                entered.send(()).map_err(|e| rusqlite::Error::UserFunctionError(Box::new(e)))?;
                resume.recv_timeout(std::time::Duration::from_secs(5)).map_err(|e| rusqlite::Error::UserFunctionError(Box::new(e)))?;
                Ok(0)
            }).unwrap();
            conn.execute_batch(&format!("CREATE TRIGGER stop_import AFTER INSERT ON second BEGIN SELECT wait_csv();SELECT RAISE({policy},'intentional SQLite error');END")).unwrap();
            state.start_csv_import_work().and_then(|_| state.settle_csv_test()).unwrap();
            state.continue_csv_import_work(Some(Answer::Yes)).unwrap();
            waiting.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
            state.continue_csv_import_work(Some(Answer::Cancel)).unwrap();
            release.send(()).unwrap();
            let error = state.settle_csv_test().unwrap_err();
            assert!(error.contains("intentional SQLite error"), "{error}");
            assert!(state.csv_work.is_none());
            let conn = state.conn.as_ref().unwrap();
            assert!(!conn.is_autocommit());
            assert_eq!(db::count_rows(conn, "second").unwrap(), 1);
            if policy == "ROLLBACK" {
                assert!(error.contains("SQLite rolled back"), "{error}");
                assert_eq!(db::count_rows(conn, "t").unwrap(), 1);
                assert!(db::table_columns(conn, "first").unwrap().is_empty());
                assert!(state.csv_files.as_ref().unwrap().queue().is_empty());
                assert!(!state.dirty);
            } else {
                assert_eq!(db::count_rows(conn, "t").unwrap(), 2);
                assert_eq!(db::count_rows(conn, "first").unwrap(), 1);
                assert_eq!(state.csv_files.as_ref().unwrap().queue(), [1]);
                assert!(state.dirty);
            }
        }
    }

    #[test]
    fn csv_batch_trigger_rollback_clears_progress_and_requires_explicit_reselection() {
        use crate::csv_files::Answer;
        let (_directory,mut state)=csv_batch_state(&[("first.csv","v\none\n"),("second.csv","v\nbad\n")]);
        state.conn.as_ref().unwrap().execute_batch("COMMIT;CREATE TABLE second(v TEXT);INSERT INTO second VALUES('saved');CREATE TRIGGER stop_import BEFORE INSERT ON second WHEN new.v='bad' BEGIN SELECT RAISE(ROLLBACK,'stop import');END;BEGIN;INSERT INTO t VALUES('pending');").unwrap();
        state.start_csv_import_work().and_then(|_| state.settle_csv_test()).unwrap();assert_eq!(state.csv_work.as_ref().unwrap().target(),"second");assert!(state.csv_files.as_ref().unwrap().files[0].done);
        let error=state.continue_csv_import_work(Some(Answer::Yes)).and_then(|_| state.settle_csv_test()).unwrap_err();assert!(error.contains("SQLite rolled back"),"{error}");assert!(state.csv_work.is_none());assert!(state.overlay.is_some());assert!(!state.dirty);
        let batch=state.csv_files.as_ref().unwrap();assert!(batch.queue().is_empty());assert!(batch.files.iter().all(|file|!file.done && !file.selected));
        let conn=state.conn.as_ref().unwrap();assert!(!conn.is_autocommit());assert_eq!(db::count_rows(conn,"t").unwrap(),1);assert_eq!(db::count_rows(conn,"second").unwrap(),1);assert!(db::table_columns(conn,"first").unwrap().is_empty());
    }

    #[test]
    fn csv_import_settings_roundtrip_custom_characters_and_upgrade_old_preferences() {
        let mut state=test_state();state.prefs.csv_import=None;state.prefs.csv_delimiter=';';state.load_csv_import_settings();assert_eq!(state.csv_options().delimiter,';');assert!(!state.csv_options().has_header);assert!(state.csv_options().trim_fields);
        let options=CsvImportOptions {delimiter:'§',quote:None,encoding:"Shift_JIS".into(),has_header:false,trim_fields:true,detect_types:false,local_conventions:true,ignore_defaults:true,fail_on_missing:true,conflict:db::CsvConflict::Replace};
        state.prefs.csv_import=Some(options.clone());let json=serde_json::to_string(&state.prefs).unwrap();state.prefs=serde_json::from_str(&json).unwrap();state.load_csv_import_settings();assert_eq!(state.csv_options(),options);
        state.csv_delim_idx=5;state.csv_custom_delimiter="U+001F".into();state.csv_quote_idx=4;state.csv_custom_quote="34".into();assert_eq!(state.csv_options().delimiter,'\u{1f}');assert_eq!(state.csv_options().quote,Some('"'));
        state.csv_custom_delimiter="55296".into();assert_eq!(state.csv_options().delimiter,'\0');state.csv_custom_quote="U+110000".into();assert_eq!(state.csv_options().quote,Some('\0'));
        state.csv_delim_idx=4;state.csv_custom_delimiter="two".into();assert_eq!(state.csv_options().delimiter,'\0');
    }

    #[test]
    fn csv_encoding_imports_real_files_and_preserves_pending_changes_on_decode_failure() {
        let mut cases=vec![("UTF-8",b"value\r\nna\xc3\xafve\r\n".to_vec(),"naïve"),("ISO-8859-1",b"value\r\ncaf\xe9\r\n".to_vec(),"café"),("windows-1252",b"value\n\x80\n".to_vec(),"€"),("Shift_JIS",b"value\n\x93\xfa\x96\x7b\n".to_vec(),"日本"),("GBK",b"value\n\xd6\xd0\xce\xc4\n".to_vec(),"中文"),("windows-1251",b"value\n\xcf\xf0\xe8\xe2\xe5\xf2\n".to_vec(),"Привет")];
        let text="value\r\n😀\r\n";
        for (label,little,wide) in [("UTF-16",true,false),("UTF-16BE",false,false),("UTF-32LE",true,true),("UTF-32BE",false,true)] {
            let mut raw=Vec::new();if wide {for ch in text.chars() {raw.extend(if little {(ch as u32).to_le_bytes()} else {(ch as u32).to_be_bytes()});}}
            else {for unit in text.encode_utf16() {raw.extend(if little {unit.to_le_bytes()} else {unit.to_be_bytes()});}}
            cases.push((label,raw,"😀"));
        }
        for (label,raw,value) in cases {
            let dir=tempfile::tempdir().unwrap();let path=dir.path().join("encoded.csv");std::fs::write(&path,&raw).unwrap();
            let mut state=test_state();state.conn.as_ref().unwrap().execute("INSERT INTO t VALUES('pending')",[]).unwrap();state.mark_dirty();
            state.csv_path=path.display().to_string();state.csv_table="imported".into();state.overlay=Some(OverlayKind::ImportCsv);state.csv_encoding_idx=3;state.csv_custom_encoding="unknown-codec".into();
            assert!(state.apply_csv_import().and_then(|_| state.settle_csv_test()).unwrap_err().contains("Unsupported CSV encoding"));assert_eq!(state.overlay,Some(OverlayKind::ImportCsv));assert!(state.dirty);
            let conn=state.conn.as_ref().unwrap();assert!(!conn.is_autocommit());assert_eq!(conn.query_row("SELECT count(*) FROM t",[],|r|r.get::<_,i64>(0)).unwrap(),2);assert_eq!(conn.query_row("SELECT count(*) FROM sqlite_schema WHERE name='imported'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
            std::fs::write(&path,[0xff]).unwrap();state.csv_encoding_idx=0;
            assert!(state.apply_csv_import().and_then(|_| state.settle_csv_test()).unwrap_err().contains("invalid or incomplete"));assert_eq!(state.overlay,Some(OverlayKind::ImportCsv));assert!(state.dirty);
            let conn=state.conn.as_ref().unwrap();assert!(!conn.is_autocommit());assert_eq!(conn.query_row("SELECT count(*) FROM t",[],|r|r.get::<_,i64>(0)).unwrap(),2);assert_eq!(conn.query_row("SELECT count(*) FROM sqlite_schema WHERE name='imported'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
            std::fs::write(&path,&raw).unwrap();
            state.csv_encoding_idx=match label {"UTF-8"=>0,"UTF-16"=>1,"ISO-8859-1"=>2,_=>3};state.csv_custom_encoding=label.into();state.apply_csv_import().and_then(|_| state.settle_csv_test()).unwrap();
            let conn=state.conn.as_ref().unwrap();assert_eq!(conn.query_row("SELECT value FROM main.imported",[],|r|r.get::<_,String>(0)).unwrap(),value,"{label}");assert!(!conn.is_autocommit());assert!(state.dirty);assert!(state.overlay.is_none());
            conn.execute_batch("ROLLBACK").unwrap();assert_eq!(conn.query_row("SELECT count(*) FROM t",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert_eq!(conn.query_row("SELECT count(*) FROM sqlite_schema WHERE name='imported'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        }
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
        state.apply_csv_import().and_then(|_| state.settle_csv_test()).unwrap();
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
            schema: "main".into(),
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
            schema: "main".into(),
            kind: ObjectKind::Index,
            name: "partial_ix".into(),
        });
        state.open_modify_index_wizard().unwrap();
        assert_eq!(state.wizard_index_where, "v IS NOT NULL");
        state.apply_index_wizard().unwrap();
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
        state.browse_view = BrowseViewMode::Form;
        state.browse_mut().sel_row = Some(499);
        state.form_next_record().unwrap();
        assert_eq!(state.browse().offset, 500);
        assert_eq!(state.browse().sel_row, Some(0));
        state.form_prev_record().unwrap();
        assert_eq!(state.browse().offset, 0);
        assert_eq!(state.browse().sel_row, Some(499));
    }

    #[test]
    fn browse_tab_switch_restores_selection_and_editor() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("INSERT INTO t VALUES('second'),('third');").unwrap();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        {
            let tab = state.browse_mut();
            tab.sel_row = Some(0);
            tab.sel_row_start = Some(2);
            tab.sel_row_end = Some(0);
            tab.sel_col = Some(0);
            tab.sel_col_start = Some(0);
            tab.sel_col_end = Some(0);
            tab.scroll = (0.0, 12.0);
        }
        state.sync_edit_cell_from_selection();
        state.add_browse_tab().unwrap();
        state.browse_mut().sel_row = Some(1);
        state.browse_mut().sel_col = Some(0);
        state.sync_edit_cell_from_selection();
        state.switch_browse_tab(0).unwrap();
        assert_eq!(state.browse().sel_row, Some(0));
        assert_eq!(state.browse().sel_row_start, Some(2));
        assert_eq!(state.browse().sel_row_end, Some(0));
        assert_eq!(state.browse().sel_col, Some(0));
        assert_eq!(state.browse().scroll, (0.0, 12.0));
        assert_eq!(state.edit_value, "saved");
        assert!(!state.has_cell_draft());
        state.close_browse_tab().unwrap();
        assert_eq!(state.browse().sel_row, Some(1));
        assert_eq!(state.edit_value, "second");
    }

    #[test]
    fn browse_tab_selection_tracks_rowids_and_clears_invalid_rectangles() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("INSERT INTO t VALUES('second'),('third');").unwrap();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        state.browse_mut().sel_row = Some(1);
        state.browse_mut().sel_row_start = Some(2);
        state.browse_mut().sel_row_end = Some(1);
        state.browse_mut().sel_col = Some(0);
        state.sync_edit_cell_from_selection();
        state.add_browse_tab().unwrap();
        state.conn.as_ref().unwrap().execute_batch("DELETE FROM t WHERE rowid=1; UPDATE t SET v='updated' WHERE rowid=2;").unwrap();
        state.switch_browse_tab(0).unwrap();
        assert_eq!(state.browse().sel_row, Some(0));
        assert_eq!(state.browse().sel_row_start, Some(1));
        assert_eq!(state.browse().sel_row_end, Some(0));
        assert_eq!(state.edit_value, "updated");
        state.switch_browse_tab(1).unwrap();
        state.conn.as_ref().unwrap().execute_batch("DELETE FROM t WHERE rowid=3;").unwrap();
        state.switch_browse_tab(0).unwrap();
        assert!(state.browse().sel_row.is_none(), "missing rows invalidate the whole rectangle");
        assert!(state.browse().sel_col.is_none());
        assert!(state.edit_col.is_empty());
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_col = Some(0);
        state.sync_edit_cell_from_selection();
        state.switch_browse_tab(1).unwrap();
        state.conn.as_ref().unwrap().execute_batch("ALTER TABLE t RENAME COLUMN v TO renamed;").unwrap();
        state.switch_browse_tab(0).unwrap();
        assert!(state.browse().sel_row.is_none());
        assert!(state.edit_col.is_empty());
    }

    #[test]
    fn browse_tab_selection_without_rowids_requires_unchanged_typed_rows() {
        for table in ["wr", "vw"] {
            let mut state = test_state();
            state.conn.as_ref().unwrap().execute_batch("CREATE TABLE wr(k TEXT PRIMARY KEY, v) WITHOUT ROWID;
                INSERT INTO wr VALUES('a',X'0102'),('b',''); CREATE VIEW vw AS SELECT * FROM wr;").unwrap();
            state.refresh_schema().unwrap();
            state.browse_mut().table = Some(table.into());
            state.refresh_browse().unwrap();
            assert!(state.browse().rowids.is_empty());
            state.browse_mut().sel_row = Some(0);
            state.browse_mut().sel_col = Some(1);
            state.sync_edit_cell_from_selection();
            state.add_browse_tab().unwrap();
            state.switch_browse_tab(0).unwrap();
            assert_eq!(state.browse().sel_row, Some(0));
            assert_eq!(state.edit_value, "0102");
            state.switch_browse_tab(1).unwrap();
            state.conn.as_ref().unwrap().execute_batch("UPDATE wr SET v=X'0304' WHERE k='a';").unwrap();
            state.switch_browse_tab(0).unwrap();
            assert!(state.browse().sel_row.is_none(), "same-length BLOB display labels are not identity");
            assert!(state.edit_col.is_empty());
        }
    }

    #[test]
    fn browse_tab_navigation_rejects_worker_owned_connection_without_mutation() {
        let mut state = test_state();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        state.add_browse_tab().unwrap();
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_col = Some(0);
        state.sync_edit_cell_from_selection();
        state.start_sql_text("SELECT 1".into()).unwrap();
        assert!(state.conn.is_none());
        assert!(state.add_browse_tab().is_err());
        assert!(state.close_browse_tab().is_err());
        assert!(state.switch_browse_tab(0).is_err());
        assert_eq!(state.browse_tabs.len(), 2);
        assert_eq!(state.browse_tab_idx, 1);
        assert_eq!(state.browse().sel_row, Some(0));
        assert_eq!(state.edit_value, "saved");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !state.poll_sql_job() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(state.conn.is_some());
    }

    #[test]
    fn browse_navigation_rejects_drafts_and_query_errors_without_losing_the_current_tab() {
        let mut state = test_state();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_col = Some(0);
        state.sync_edit_cell_from_selection();
        state.edit_value = "keep this draft".into();
        assert!(state.has_cell_draft());
        assert!(state.form_next_record().is_err());
        assert!(state.form_prev_record().is_err());
        assert!(state.add_browse_tab().is_err());
        assert!(state.close_browse_tab().is_err());
        assert!(state.switch_browse_tab(0).is_err());
        assert_eq!(state.browse_tabs.len(), 1);
        assert_eq!(state.browse().sel_row, Some(0));
        assert_eq!(state.edit_value, "keep this draft");
        state.discard_cell_draft();
        let mut bad = BrowseTabState::new(Some("t".into()));
        bad.filter = ")".into();
        state.browse_tabs.push(bad);
        assert!(state.switch_browse_tab(1).is_err());
        assert!(state.close_browse_tab().is_err());
        assert_eq!(state.browse_tab_idx, 0);
        assert_eq!(state.browse_tabs.len(), 2);
        assert_eq!(state.browse().sel_row, Some(0));
        assert_eq!(state.edit_value, "saved");
    }

    #[test]
    fn form_navigation_preserves_column_reports_page_errors_and_clamps_shrunken_pages() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("DROP TABLE t; CREATE TABLE t(a,b);
            WITH RECURSIVE c(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM c WHERE x<601) INSERT INTO t SELECT x,'v'||x FROM c;").unwrap();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        state.browse_mut().sel_row = Some(499);
        state.browse_mut().sel_col = Some(1);
        state.sync_edit_cell_from_selection();
        state.browse_mut().filter = ")".into();
        assert!(state.form_next_record().is_err());
        assert_eq!(state.browse().offset, 0);
        assert_eq!(state.browse().sel_row, Some(499));
        assert_eq!(state.browse().sel_col, Some(1));
        assert_eq!(state.edit_value, "v500");
        state.browse_mut().filter.clear();
        state.form_next_record().unwrap();
        assert_eq!(state.browse().offset, 500);
        assert_eq!(state.browse().sel_col, Some(1));
        assert_eq!(state.edit_value, "v501");
        state.form_prev_record().unwrap();
        assert_eq!(state.browse().offset, 0);
        assert_eq!(state.browse().sel_col, Some(1));
        assert_eq!(state.edit_value, "v500");
        state.browse_page(true).unwrap();
        state.browse_mut().filter = "a<3".into();
        state.refresh_browse().unwrap();
        assert_eq!(state.browse().offset, 0);
        assert_eq!(state.browse().total_rows, Some(2));
        assert_eq!(state.browse().result.as_ref().unwrap().rows.len(), 2);
        state.browse_mut().filter = "0".into();
        state.refresh_browse().unwrap();
        state.form_next_record().unwrap();
        state.form_prev_record().unwrap();
        assert_eq!(state.browse().sel_row, None);
        assert_eq!(state.browse().total_rows, Some(0));
    }

    #[test]
    fn closing_browse_tabs_keeps_one_empty_tab() {
        let mut state = test_state();
        state.browse_tabs = vec![
            BrowseTabState::new(Some("t".into())),
            BrowseTabState::new(Some("other".into())),
        ];
        state.browse_tab_idx = 1;
        state.close_browse_tab().unwrap();
        assert_eq!(state.browse_tabs.len(), 1);
        assert_eq!(state.browse().table.as_deref(), Some("t"));
        state.close_browse_tab().unwrap();
        assert_eq!(state.browse_tabs.len(), 1);
        assert!(state.browse().table.is_none());
    }

    #[test]
    fn foreign_key_navigation_preserves_non_utf8_text_keys() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE parent(k TEXT PRIMARY KEY);
            CREATE TABLE child(k REFERENCES parent(k));
            INSERT INTO parent VALUES(CAST(X'FF' AS TEXT)); INSERT INTO child SELECT k FROM parent;").unwrap();
        state.refresh_schema().unwrap();
        state.browse_mut().table = Some("child".into());
        state.refresh_browse().unwrap();
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_col = Some(0);
        state.edit_col = "k".into();
        state.navigate_fk().unwrap();
        assert_eq!(state.browse().table.as_deref(), Some("parent"));
        assert_eq!(state.browse().result.as_ref().unwrap().values.rectangle(0..=0, 0..=0).unwrap(), vec![vec![db::CellValue::Text(vec![255])]]);
    }

    #[test]
    fn cut_clears_nullable_and_required_cells_and_rolls_back_on_constraints() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("DROP TABLE t; CREATE TABLE t(a TEXT NOT NULL,b);
            INSERT INTO t VALUES('a','b'),('c','d');
            CREATE TABLE constrained(id,a, CHECK(id<>2 OR a IS NOT NULL));
            INSERT INTO constrained VALUES(1,'keep first'),(2,'keep second');").unwrap();
        state.refresh_schema().unwrap();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        state.browse_mut().sel_row = Some(1);
        state.browse_mut().sel_row_start = Some(0);
        let copy = state.copy_browse_cells(false, false).unwrap();
        state.cut_browse_cells().unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().values.rectangle(0..=1, 0..=1).unwrap(),
            vec![vec![db::CellValue::Text(Vec::new()), db::CellValue::Null]; 2]);
        assert!(state.dirty);
        state.browse_mut().sel_row = Some(0);
        state.paste_browse_cells(copy.internal_cells(&copy.html).unwrap()).unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows, vec![vec!["a", "b"], vec!["c", "d"]]);
        state.browse_mut().table = Some("constrained".into());
        state.refresh_browse().unwrap();
        state.browse_mut().sel_row = Some(1);
        state.browse_mut().sel_row_start = Some(0);
        state.browse_mut().sel_col = Some(1);
        assert!(state.cut_browse_cells().unwrap_err().contains("CHECK"));
        assert_eq!(db::execute_sql(state.conn.as_ref().unwrap(), "SELECT a FROM constrained ORDER BY id").unwrap().rows,
            vec![vec!["keep first"], vec!["keep second"]]);
        assert_eq!(state.browse().sel_row, Some(1));
    }

    #[test]
    fn typed_clipboard_roundtrip_preserves_null_empty_blob_numbers_and_raw_text() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("DROP TABLE t; CREATE TABLE t(v);
            INSERT INTO t VALUES(NULL),(''),('NULL'),(9223372036854775807),(1.25),(X'00FF'),(CAST(X'FF' AS TEXT));
            CREATE TABLE dest(v); INSERT INTO dest SELECT NULL FROM t;").unwrap();
        state.refresh_schema().unwrap();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        let original = state.browse().result.as_ref().unwrap().values.rectangle(0..=6, 0..=0).unwrap();
        state.browse_mut().sel_row = Some(6);
        state.browse_mut().sel_row_start = Some(0);
        state.browse_mut().sel_col = Some(0);
        state.sync_edit_cell_from_selection();
        assert!(state.error_msg.as_deref().unwrap().contains("invalid UTF-8"));
        let copy = state.copy_browse_cells(false, false).unwrap();
        let cells = copy.internal_cells(&copy.html).unwrap();
        state.browse_mut().table = Some("dest".into());
        state.refresh_browse().unwrap();
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_col = Some(0);
        state.paste_browse_cells(cells).unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().values.rectangle(0..=6, 0..=0).unwrap(), original);
        let types: Vec<String> = state.conn.as_ref().unwrap().prepare("SELECT typeof(v)||':'||hex(v) FROM dest ORDER BY rowid").unwrap()
            .query_map([], |r| r.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert_eq!(types, vec!["null:", "text:", "text:4E554C4C", "integer:39323233333732303336383534373735383037", "real:312E3235", "blob:00FF", "text:FF"]);
    }

    #[test]
    fn paste_updates_primary_key_and_constrained_columns_together_and_rolls_back_on_failure() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("DROP TABLE t;
            CREATE TABLE t(id INTEGER PRIMARY KEY, a INTEGER, b INTEGER, CHECK(a=b));
            INSERT INTO t VALUES(1,1,1),(2,2,2);
            CREATE TABLE pending(v); COMMIT; BEGIN; INSERT INTO pending VALUES('keep');").unwrap();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_col = Some(0);
        let error = state.paste_browse_tsv("7\t10\t10\n8\t20\t21").unwrap_err();
        assert!(error.contains("CHECK"), "{error}");
        let conn = state.conn.as_ref().unwrap();
        let rows = db::execute_sql(conn, "SELECT id,a,b FROM t ORDER BY id").unwrap().rows;
        assert_eq!(rows, vec![vec!["1", "1", "1"], vec!["2", "2", "2"]]);
        assert_eq!(conn.query_row("SELECT v FROM pending", [], |r| r.get::<_, String>(0)).unwrap(), "keep");
        assert!(!conn.is_autocommit());
        assert_eq!(state.browse().sel_row, Some(0));
        state.paste_browse_tsv("7\t10\t10\n8\t20\t20").unwrap();
        assert_eq!(db::execute_sql(state.conn.as_ref().unwrap(), "SELECT id,a,b FROM t ORDER BY id").unwrap().rows,
            vec![vec!["7", "10", "10"], vec!["8", "20", "20"]]);
    }

    #[test]
    fn missing_paste_record_rolls_back_trigger_effects_and_other_rows() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("INSERT INTO t VALUES('second');
            CREATE TRIGGER remove_next AFTER UPDATE ON t WHEN old.rowid=1 BEGIN DELETE FROM t WHERE rowid=2; END;").unwrap();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_col = Some(0);
        let original = state.browse().result.as_ref().unwrap().rows.clone();
        assert!(state.paste_browse_tsv("changed\nsecond changed").unwrap_err().contains("no longer exists"));
        assert_eq!(db::execute_sql(state.conn.as_ref().unwrap(), "SELECT v FROM t ORDER BY rowid").unwrap().rows, original);
    }

    #[test]
    fn quoted_tsv_and_single_cell_fill_preserve_text_and_empty_cells() {
        let mut state = test_state();
        state.conn.as_ref().unwrap().execute_batch("DROP TABLE t; CREATE TABLE t(a,b); INSERT INTO t VALUES('a','b'),('c','d');").unwrap();
        state.browse_mut().table = Some("t".into());
        state.refresh_browse().unwrap();
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_col = Some(0);
        state.paste_browse_tsv("\"tab\tinside\"\t\"line1\nline2 \"\"quoted\"\"\"\r\nNULL\t\r\n").unwrap();
        let rows = db::execute_sql(state.conn.as_ref().unwrap(), "SELECT a,b FROM t ORDER BY rowid").unwrap().rows;
        assert_eq!(rows, vec![vec!["tab\tinside", "line1\nline2 \"quoted\""], vec!["NULL", ""]]);
        assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT typeof(a),typeof(b) FROM t WHERE rowid=2", [], |r|
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).unwrap(), ("text".into(), "text".into()));
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_row_start = Some(1);
        state.browse_mut().sel_col = Some(0);
        state.browse_mut().sel_col_start = Some(1);
        state.paste_browse_tsv("\"\"").unwrap();
        assert_eq!(db::execute_sql(state.conn.as_ref().unwrap(), "SELECT typeof(a),length(a),typeof(b),length(b) FROM t").unwrap().rows,
            vec![vec!["text", "0", "text", "0"]; 2]);
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_col = Some(0);
        state.paste_browse_tsv("\n\n").unwrap();
        assert_eq!(db::execute_sql(state.conn.as_ref().unwrap(), "SELECT length(a) FROM t").unwrap().rows, vec![vec!["0"]; 2]);
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
        state.browse_mut().sel_row = Some(0);
        state.browse_mut().sel_col = Some(0);
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
    fn attached_project_roundtrip_keeps_active_and_saved_namespace_options() {
        let dir = tempfile::tempdir().unwrap();
        for (file, value) in [("main.db", "main"), ("attached.db", "attached")] {
            Connection::open(dir.path().join(file)).unwrap().execute_batch(&format!(
                "CREATE TABLE t(v); INSERT INTO t VALUES('{value}'); CREATE VIEW numbers AS SELECT length('{value}') AS v;")).unwrap();
        }
        let path = dir.path().join("project.sqbpro");
        let schema = "辅.库";
        std::fs::write(&path, format!("<sqlb_project><db path='main.db'/>
            <attached><db schema='{schema}' path='attached.db'/></attached>
            <tab_browse><table table='{},1:{schema}t' title='附加' unknown='retained'/>
            <browse_table_settings>
            <table schema='main' name='t'><column_widths><column index='1' value='124'/></column_widths><filter_values><column index='0' value='=main'/></filter_values></table>
            <table schema='{schema}' name='t' freeze_columns='2'><column_widths><column index='1' value='248'/></column_widths><filter_values><column index='0' value='=attached'/></filter_values></table>
            </browse_table_settings></tab_browse></sqlb_project>", schema.len())).unwrap();
        let mut state = test_state();
        state.open_project(&path).unwrap();
        assert_eq!(state.browse().schema, schema);
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][0], "attached");
        assert_eq!(state.browse().column_widths, [248.0]);
        assert_eq!(state.browse().frozen_cols, Some(1));
        state.switch_browse_table("t").unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][0], "main");
        assert_eq!(state.browse().column_widths, [124.0]);
        assert_eq!(state.browse().col_filters, ["=main"]);
        state.switch_browse_table_in(schema, "t").unwrap();
        assert_eq!(state.browse().col_filters, ["=attached"]);
        state.add_browse_tab().unwrap();
        state.browse_mut().column_widths = vec![310.0];
        state.browse_mut().frozen_cols = Some(0);
        state.chart_schema = schema.into();
        state.chart_table = Some("numbers".into());
        state.chart_column = "v".into();
        state.refresh_chart().unwrap();
        assert_eq!(state.chart_points, [8.0]);
        state.save_project(&path).unwrap();
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(source.contains("unknown=\"retained\""));
        assert!(source.contains(&format!("table=\"{},1:{schema}t\"", schema.len())));
        state.open_project(&path).unwrap();
        assert_eq!(state.browse_tabs.len(), 2);
        assert_eq!(state.browse_tab_idx, 1);
        assert_eq!(state.chart_schema, schema);
        assert_eq!(state.chart_points, [8.0]);
        assert_eq!(state.browse().schema, schema);
        assert_eq!(state.browse().column_widths, [310.0]);
        assert_eq!(state.browse().frozen_cols, Some(0));
        state.switch_browse_tab(0).unwrap();
        assert_eq!(state.browse().schema, schema);
        assert_eq!(state.browse().column_widths, [248.0]);
        assert_eq!(state.browse().frozen_cols, Some(1));
        state.switch_browse_table("t").unwrap();
        assert_eq!(state.browse().column_widths, [124.0]);
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][0], "main");
        state.chart_schema = "missing".into();
        state.save_project(&path).unwrap();
        state.open_project(&path).unwrap();
        assert!(state.chart_points.is_empty());
        assert_eq!(state.chart_schema, "missing");
        assert!(state.status_msg.contains("plot unavailable"));
    }

    #[test]
    fn reopening_a_temporary_browse_project_does_not_read_main_shadow() {
        let dir = tempfile::tempdir().unwrap();
        let database = dir.path().join("main.db");
        let conn = db::open_path(database.to_str().unwrap()).unwrap();
        conn.execute_batch("CREATE TABLE t(v); INSERT INTO t VALUES('main');").unwrap();
        let mut state = AppState::empty();
        state.adopt_conn(conn, database.display().to_string(), false).unwrap();
        state.conn.as_ref().unwrap().execute_batch("CREATE TEMP TABLE t(v); INSERT INTO temp.t VALUES('temp');").unwrap();
        state.refresh_schema().unwrap();
        state.switch_browse_table_in("temp", "t").unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][0], "temp");
        state.write_changes().unwrap();
        let path = dir.path().join("temp.sqbpro");
        state.save_project(&path).unwrap();
        state.open_project(&path).unwrap();
        assert!(state.browse().table.is_none() && state.browse().result.is_none());
        assert!(state.status_msg.contains("temporary table contents"));
        state.switch_browse_table("t").unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][0], "main");
    }

    #[test]
    fn invalid_attached_browse_project_preserves_the_existing_session() {
        let dir = tempfile::tempdir().unwrap();
        for file in ["main.db", "attached.db"] {
            Connection::open(dir.path().join(file)).unwrap().execute_batch("CREATE TABLE t(v); INSERT INTO t VALUES(1);").unwrap();
        }
        let mut state = test_state();
        let original_tabs = state.sql_tabs.clone();
        for (schema, table, filter, inactive) in [
            ("aux", "t", "v =", false), ("aux", "missing", "", false),
            ("missing", "t", "", false), ("aux", "t", "v =", true), ("missing", "t", "", true),
        ] {
            let mut tab = BrowseTabState::new_in(schema, Some(table.into()));
            tab.filter = filter.into();
            if inactive {
                let options = tab.options();
                tab = BrowseTabState::new(Some("t".into()));
                tab.store_options(schema, table, options);
            }
            let project = crate::project::ProjectFile {
                db_path: dir.path().join("main.db").display().to_string(),
                browse_tabs: vec![tab],
                db4s_xml: Some("<sqlb_project><db path='main.db'/><attached><db schema='aux' path='attached.db'/></attached></sqlb_project>".into()),
                ..Default::default()
            };
            let path = dir.path().join("invalid.sqbpro");
            project.save(&path).unwrap();
            assert!(state.open_project(&path).is_err(), "{schema} {table} {filter} inactive={inactive}");
            assert_eq!(state.path, ":memory:");
            assert_eq!(state.sql_tabs, original_tabs);
            assert!(state.project_path.is_none());
            assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT v FROM main.t", [], |r| r.get::<_, String>(0)).unwrap(), "saved");
        }
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
<tab_browse><table title="records" table="4,7:mainrecords"/><browse_table_settings><table schema="main" name="records"><sort><column index="1" mode="1"/></sort><column_widths><column index="2" value="220"/></column_widths></table></browse_table_settings></tab_browse>
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
            "<sqlb_project><db path='data.db'/><tab_browse><table table='4,7:mainrecords'/><browse_table_settings><table schema='main' name='records'><column_widths><column index='1' value='NaN'/></column_widths></table></browse_table_settings></tab_browse></sqlb_project>",
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
    fn demo_test_sessions_keep_independent_pending_database_writes() {
        let first = AppState::new_with_demo();
        let second = AppState::new_with_demo();
        assert_ne!(first.path, second.path);
        let first = first.conn.as_ref().unwrap();
        let second = second.conn.as_ref().unwrap();
        first.execute("UPDATE customers SET city='first' WHERE id=1", []).unwrap();
        second.execute("UPDATE customers SET city='second' WHERE id=1", []).unwrap();
        assert_eq!(first.query_row("SELECT city FROM customers WHERE id=1", [], |row|row.get::<_,String>(0)).unwrap(), "first");
        assert_eq!(second.query_row("SELECT city FROM customers WHERE id=1", [], |row|row.get::<_,String>(0)).unwrap(), "second");
        assert!(!first.is_autocommit() && !second.is_autocommit());
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
                auto_increment: false,
                collation: String::new(),
                check_expression: String::new(), foreign_key_clause: String::new(), generated: None,
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
