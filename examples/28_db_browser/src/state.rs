//! Application state for the DB Browser example (DB4S chrome + Phase 2 wizards).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, InterruptHandle};

use crate::db::{
    self, ColDef, CsvImportOptions, ObjectKind, QueryResult, SchemaObject, COLUMN_TYPES,
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
}

impl DockKind {
    #[allow(dead_code)]
    pub fn from_index(i: usize) -> Self {
        match i {
            1 => DockKind::Schema,
            2 => DockKind::Log,
            3 => DockKind::Plot,
            _ => DockKind::EditCell,
        }
    }

    #[allow(dead_code)]
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn title(self) -> &'static str {
        match self {
            DockKind::EditCell => "Edit Database Cell",
            DockKind::Schema => "DB Schema",
            DockKind::Log => "SQL Log",
            DockKind::Plot => "Plot",
        }
    }
}

/// Full-window overlay wizards (Modal cannot host custom controls).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    EditTable,
    EditIndex,
    ImportCsv,
    FindReplace,
    Attach,
    PasteCsv,
}

#[derive(Debug, Clone)]
pub struct SelectedObject {
    pub kind: ObjectKind,
    pub name: String,
    pub sql: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BrowseTabState {
    pub title: String,
    pub table: Option<String>,
    pub filter: String,
    /// Parallel to current result columns; empty = no filter.
    pub col_filters: Vec<String>,
    pub order_col: Option<String>,
    pub order_desc: bool,
    pub result: Option<QueryResult>,
    pub rowids: Vec<i64>,
    pub sel_row: Option<usize>,
}

impl BrowseTabState {
    pub fn new(table: Option<String>) -> Self {
        let title = table.clone().unwrap_or_else(|| "Browse".into());
        Self {
            title,
            table,
            filter: String::new(),
            col_filters: Vec::new(),
            order_col: None,
            order_desc: false,
            result: None,
            rowids: Vec::new(),
            sel_row: None,
        }
    }
}

pub struct AppState {
    pub conn: Option<Connection>,
    pub path: String,
    pub path_draft: String,
    pub read_only: bool,
    pub dirty: bool,
    pub schema: Vec<SchemaObject>,
    pub selected: Option<SelectedObject>,
    pub tab: MainTab,
    pub status_msg: String,
    pub error_msg: Option<String>,

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
    pub find_pos: usize,

    // Execute SQL
    pub sql_tabs: Vec<String>,
    pub sql_tab_idx: usize,
    pub sql_result: Option<QueryResult>,
    pub sql_message: String,
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

    pub sql_log: Vec<String>,
    pub io_path: String,
    pub recent: VecDeque<String>,

    pub confirm_kind: ConfirmKind,
    pub confirm_open: bool,
}

pub struct SqlJob {
    pub done: Arc<AtomicBool>,
    pub result: Arc<Mutex<Option<Result<QueryResult, String>>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmKind {
    None,
    DropObject,
    DeleteRow,
    CloseDb,
    Revert,
    ExitDirty,
}

impl AppState {
    pub fn new_with_demo() -> Self {
        let demo = db::default_demo_path();
        let mut state = Self::empty();
        state.load_recent();
        match open_or_create_demo(&demo) {
            Ok(conn) => {
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
            }
            Err(e) => {
                state.status_msg = format!("Demo DB failed: {e}; using in-memory");
                if let Ok(conn) = open_shared_memory() {
                    let _ = db::seed_demo(&conn);
                    state.conn = Some(conn);
                    state.path = ":memory:".into();
                    state.path_draft = state.path.clone();
                    let _ = state.begin_txn();
                    let _ = state.refresh_schema();
                    state.pick_chart_source();
                    let _ = state.refresh_chart();
                    let _ = state.refresh_pragmas();
                }
            }
        }
        state.log_sql("-- session start");
        state
    }

    fn empty() -> Self {
        Self {
            conn: None,
            path: String::new(),
            path_draft: String::new(),
            read_only: false,
            dirty: false,
            schema: Vec::new(),
            selected: None,
            tab: MainTab::Structure,
            status_msg: "Ready".into(),
            error_msg: None,
            dock_edit: true,
            dock_schema: true,
            dock_log: true,
            dock_plot: true,
            dock_focus: DockKind::EditCell,
            browse_tabs: vec![BrowseTabState::new(None)],
            browse_tab_idx: 0,
            edit_col: String::new(),
            edit_value: String::new(),
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
            find_pos: 0,
            sql_tabs: vec!["SELECT name, type FROM sqlite_master ORDER BY type, name;\n".into()],
            sql_tab_idx: 0,
            sql_result: None,
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
            sql_log: Vec::new(),
            io_path: String::new(),
            recent: VecDeque::new(),
            confirm_kind: ConfirmKind::None,
            confirm_open: false,
        }
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

    pub fn any_dock_visible(&self) -> bool {
        self.dock_edit || self.dock_schema || self.dock_log || self.dock_plot
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
        let s = stmt.into();
        self.sql_log.push(s);
        if self.sql_log.len() > 200 {
            let drain = self.sql_log.len() - 200;
            self.sql_log.drain(0..drain);
        }
    }

    pub fn refresh_schema(&mut self) -> Result<(), String> {
        let Some(conn) = self.conn.as_ref() else {
            self.schema.clear();
            return Ok(());
        };
        self.schema = db::list_schema(conn)?;
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

    #[allow(dead_code)]
    fn composed_where(tab: &BrowseTabState) -> Option<String> {
        let mut parts = Vec::new();
        if !tab.filter.trim().is_empty() {
            parts.push(format!("({})", tab.filter.trim()));
        }
        if let Some(res) = tab.result.as_ref() {
            for (i, col) in res.columns.iter().enumerate() {
                if let Some(f) = tab.col_filters.get(i) {
                    let f = f.trim();
                    if !f.is_empty() {
                        let lit = f.replace('\'', "''");
                        parts.push(format!("{} LIKE '%{}%'", db::quote_ident(col), lit));
                    }
                }
            }
        } else {
            // Before first load, col_filters may already be set after prior refresh —
            // handled on next refresh when columns known.
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" AND "))
        }
    }

    pub fn refresh_browse(&mut self) -> Result<(), String> {
        if self.conn.is_none() {
            if let Some(t) = self.browse_tabs.get_mut(self.browse_tab_idx) {
                t.result = None;
                t.rowids.clear();
            }
            return Ok(());
        }
        let (table, order, desc, filter_global, mut col_filters, prev_cols) = {
            let t = self.browse_mut();
            (
                t.table.clone(),
                t.order_col.clone(),
                t.order_desc,
                t.filter.clone(),
                t.col_filters.clone(),
                t.result.as_ref().map(|r| r.columns.clone()).unwrap_or_default(),
            )
        };
        let Some(table) = table else {
            let t = self.browse_mut();
            t.result = None;
            t.rowids.clear();
            return Ok(());
        };
        let is_view = self
            .schema
            .iter()
            .any(|o| o.name == table && o.kind == ObjectKind::View);

        let mut parts = Vec::new();
        if !filter_global.trim().is_empty() {
            parts.push(format!("({})", filter_global.trim()));
        }
        for (i, col) in prev_cols.iter().enumerate() {
            if let Some(f) = col_filters.get(i) {
                let f = f.trim();
                if !f.is_empty() {
                    let lit = f.replace('\'', "''");
                    parts.push(format!("{} LIKE '%{}%'", db::quote_ident(col), lit));
                }
            }
        }
        let where_sql = if parts.is_empty() { None } else { Some(parts.join(" AND ")) };

        let (result, rowids) = {
            let conn = self.conn.as_ref().unwrap();
            if is_view {
                (
                    db::browse_table(conn, &table, order.as_deref(), desc, where_sql.as_deref(), 500)?,
                    Vec::new(),
                )
            } else {
                match db::browse_with_rowid(
                    conn, &table, order.as_deref(), desc, where_sql.as_deref(), 500,
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
                                if idx < row.len() { row.remove(idx); }
                            }
                        }
                        (q, rowids)
                    }
                    Err(_) => (
                        db::browse_table(conn, &table, order.as_deref(), desc, where_sql.as_deref(), 500)?,
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
                Some(c) => format!(" ORDER BY {} {}", c, if t.order_desc { "DESC" } else { "ASC" }),
                None => String::new(),
            }
        };
        {
            let t = self.browse_mut();
            t.col_filters = col_filters;
            t.title = table.clone();
            t.rowids = rowids;
            t.result = Some(result);
        }
        self.status_msg = format!(
            "Browse {} — {} rows{}{}",
            table,
            nrows,
            if truncated { " (truncated)" } else { "" },
            order_note
        );
        Ok(())
    }

    pub fn sync_edit_cell_from_selection(&mut self) {
        let (cols, cells) = {
            let t = self.browse();
            let Some(row) = t.sel_row else { return };
            let Some(res) = t.result.as_ref() else { return };
            let Some(cells) = res.rows.get(row).cloned() else { return };
            (res.columns.clone(), cells)
        };
        if self.edit_col.trim().is_empty() {
            if let Some(c0) = cols.first() {
                self.edit_col = c0.clone();
            }
        }
        if let Some(ci) = cols.iter().position(|c| c == &self.edit_col) {
            self.edit_value = cells.get(ci).cloned().unwrap_or_default();
        }
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
        self.chart_points =
            db::numeric_series(conn, &table, self.chart_column.trim(), 64).unwrap_or_default();
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
                sql: obj.sql.clone(),
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
        self.wizard_index_name = sel.name.clone();
        self.wizard_index_original = Some(sel.name.clone());
        // Best-effort parse FROM sql: CREATE [UNIQUE] INDEX name ON table (cols)
        let sql = sel.sql.unwrap_or_default();
        self.wizard_index_unique = sql.to_ascii_uppercase().contains("UNIQUE");
        if let Some(on) = sql.split(" ON ").nth(1).or_else(|| sql.split(" on ").nth(1)) {
            let on = on.trim();
            if let Some(paren) = on.find('(') {
                self.wizard_index_table = on[..paren].trim().trim_matches('"').to_string();
                self.wizard_index_cols = on[paren + 1..]
                    .trim()
                    .trim_end_matches(';')
                    .trim_end_matches(')')
                    .to_string();
            }
        }
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
        if let Some(orig) = self.wizard_index_original.clone() {
            let _ = db::drop_object(conn, ObjectKind::Index, &orig);
        }
        db::create_index(conn, &name, &table, &cols, self.wizard_index_unique)?;
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
        let n = db::import_csv_into_table(
            self.conn.as_ref().unwrap(),
            &table,
            &cols,
            &rows,
            true,
        )?;
        self.log_sql(format!("-- import csv → {table} ({n} rows)"));
        self.mark_dirty();
        self.overlay = None;
        self.refresh_schema()?;
        {
            let t = self.browse_mut();
            t.table = Some(table.clone());
            t.title = table.clone();
        }
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
        let mut chosen = None;
        for pref in preferred {
            if names.iter().any(|n| n == pref) {
                chosen = Some(pref.to_string());
                break;
            }
        }
        if chosen.is_none() {
            if let Some(conn) = self.conn.as_ref() {
                for name in &names {
                    if let Ok(pts) = db::numeric_series(conn, name, self.chart_column.trim(), 8) {
                        if !pts.is_empty() {
                            chosen = Some(name.clone());
                            break;
                        }
                    }
                }
            }
        }
        self.chart_table = chosen.or_else(|| names.first().cloned());
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

    fn adopt_conn(&mut self, conn: Connection, path: String, read_only: bool) -> Result<(), String> {
        self.conn = Some(conn);
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
        Ok(())
    }

    pub fn open_path(&mut self, path: &str) -> Result<(), String> {
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
        let conn = db::open_readonly(path)?;
        self.log_sql(format!("-- open read-only {path}"));
        self.adopt_conn(conn, path.to_string(), true)?;
        self.status_msg = format!("Opened read-only {path}");
        Ok(())
    }

    pub fn create_new_db(&mut self, path: &str) -> Result<(), String> {
        let conn = db::create_new(path)?;
        self.log_sql(format!("-- create {path}"));
        self.schema.clear();
        self.sql_result = None;
        self.adopt_conn(conn, path.to_string(), false)?;
        self.status_msg = format!("Created {path}");
        Ok(())
    }

    pub fn new_memory_db(&mut self) -> Result<(), String> {
        let conn = open_shared_memory()?;
        self.log_sql("-- new in-memory");
        self.schema.clear();
        self.sql_result = None;
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
        if self.dirty {
            self.write_changes()?;
        }
        db::copy_database_file(&self.path, dst)?;
        self.open_path(dst)?;
        self.status_msg = format!("Saved As {dst}");
        Ok(())
    }

    pub fn close_db(&mut self) {
        if let Some(conn) = self.conn.take() {
            let _ = conn.execute_batch("ROLLBACK");
            drop(conn);
        }
        self.schema.clear();
        self.selected = None;
        self.browse_tabs = vec![BrowseTabState::new(None)];
        self.browse_tab_idx = 0;
        self.sql_result = None;
        self.pragmas.clear();
        self.chart_points.clear();
        self.path.clear();
        self.read_only = false;
        self.dirty = false;
        self.log_sql("-- close");
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
        let _ = std::fs::write(Self::recent_path(), body);
    }

    pub fn find_next_in_sql(&mut self) -> bool {
        let needle = self.find_text.clone();
        if needle.is_empty() {
            return false;
        }
        let text = self.sql_editor().to_string();
        let start = self.find_pos.min(text.len());
        if let Some(rel) = text[start..].find(&needle) {
            self.find_pos = start + rel + needle.len();
            true
        } else if let Some(rel) = text.find(&needle) {
            self.find_pos = rel + needle.len();
            true
        } else {
            false
        }
    }

    pub fn replace_one_in_sql(&mut self) -> bool {
        let needle = self.find_text.clone();
        let repl = self.replace_text.clone();
        if needle.is_empty() {
            return false;
        }
        let text = self.sql_editor().to_string();
        if let Some(idx) = text.find(&needle) {
            let mut new = String::new();
            new.push_str(&text[..idx]);
            new.push_str(&repl);
            new.push_str(&text[idx + needle.len()..]);
            *self.sql_editor_mut() = new;
            self.find_pos = idx + repl.len();
            true
        } else {
            false
        }
    }

    pub fn replace_all_in_sql(&mut self) -> usize {
        let needle = self.find_text.clone();
        let repl = self.replace_text.clone();
        if needle.is_empty() {
            return 0;
        }
        let text = self.sql_editor().to_string();
        let count = text.matches(&needle).count();
        *self.sql_editor_mut() = text.replace(&needle, &repl);
        self.find_pos = 0;
        count
    }


    pub fn add_browse_tab(&mut self) {
        let table = self
            .browse()
            .table
            .clone()
            .or_else(|| {
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
        db::attach_database(self.conn.as_ref().unwrap(), &path, &schema)?;
        self.log_sql(format!("ATTACH DATABASE '{path}' AS {schema}"));
        self.overlay = None;
        self.refresh_schema()?;
        self.status_msg = format!("Attached `{path}` as {schema}");
        Ok(())
    }

    pub fn save_pragmas(&mut self) -> Result<(), String> {
        self.guard_writable()?;
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
            for (k, v) in &pairs {
                let vv = v.trim();
                if vv.is_empty() {
                    continue;
                }
                let out = match k.as_str() {
                    "foreign_keys" | "recursive_triggers" => {
                        if matches!(vv.to_ascii_lowercase().as_str(), "1" | "on" | "true") {
                            "ON"
                        } else {
                            "OFF"
                        }
                    }
                    _ => vv,
                };
                db::write_pragma(conn, k, out)?;
                logs.push(format!("PRAGMA {k} = {out}"));
            }
        }
        for l in logs {
            self.log_sql(l);
        }
        self.mark_dirty();
        self.refresh_pragmas()?;
        self.status_msg = "Pragmas saved (may need Write Changes)".into();
        Ok(())
    }

    pub fn run_sql_sync(&mut self) -> Result<(), String> {
        let sql = self.sql_editor().to_string();
        if sql.trim().is_empty() {
            return Err("SQL is empty".into());
        }
        self.sql_cancel.store(false, Ordering::SeqCst);
        let conn = self.conn.as_ref().ok_or("No database open")?;
        // Cooperative cancel via progress handler
        let cancel = self.sql_cancel.clone();
        let _ = conn.progress_handler(1000, Some(move || cancel.load(Ordering::SeqCst)));
        self.sql_interrupt = Some(conn.get_interrupt_handle());
        self.sql_busy = true;
        let result = db::execute_sql(conn, &sql);
        let _ = conn.progress_handler(0, None::<fn() -> bool>);
        self.sql_busy = false;
        self.sql_interrupt = None;
        match result {
            Ok(res) => {
                let n = res.rows.len();
                self.sql_result = Some(res);
                self.sql_message = format!("OK — {n} result row(s)");
                self.status_msg = "SQL ok".into();
                self.error_msg = None;
                self.log_sql(sql.lines().next().unwrap_or("SQL").to_string());
                let _ = self.refresh_schema();
                Ok(())
            }
            Err(e) => {
                self.sql_result = None;
                self.error_msg = Some(e.clone());
                self.sql_message = e.clone();
                self.status_msg = e.clone();
                Err(e)
            }
        }
    }

    /// Start SQL on a background connection (file path or shared-memory URI) so Stop can interrupt.
    pub fn start_sql_job(&mut self) -> Result<(), String> {
        if self.sql_busy {
            return Err("SQL already running".into());
        }
        let sql = self.sql_editor().to_string();
        if sql.trim().is_empty() {
            return Err("SQL is empty".into());
        }
        let path = self.path.clone();
        let read_only = self.read_only;
        self.sql_cancel.store(false, Ordering::SeqCst);
        let cancel = self.sql_cancel.clone();
        let done = Arc::new(AtomicBool::new(false));
        let result: Arc<Mutex<Option<Result<QueryResult, String>>>> =
            Arc::new(Mutex::new(None));
        let done_t = done.clone();
        let result_t = result.clone();

        // Open worker connection
        let worker = if path == ":memory:" || path.is_empty() {
            open_shared_memory()?
        } else if read_only {
            db::open_readonly(&path)?
        } else {
            db::open_path(&path)?
        };
        self.sql_interrupt = Some(worker.get_interrupt_handle());
        self.sql_busy = true;
        self.sql_message = "Running…".into();
        self.status_msg = "SQL running…".into();

        let sql_for_log = sql.lines().next().unwrap_or("SQL").to_string();
        std::thread::spawn(move || {
            let cancel2 = cancel.clone();
            let _ = worker.progress_handler(500, Some(move || cancel2.load(Ordering::SeqCst)));
            let res = db::execute_sql(&worker, &sql);
            let _ = worker.progress_handler(0, None::<fn() -> bool>);
            if let Ok(mut g) = result_t.lock() {
                *g = Some(res);
            }
            done_t.store(true, Ordering::SeqCst);
        });
        self.sql_job = Some(SqlJob { done, result });
        self.log_sql(sql_for_log);
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
        let Some(job) = self.sql_job.as_ref() else {
            return false;
        };
        if !job.done.load(Ordering::SeqCst) {
            return false;
        }
        let result = job
            .result
            .lock()
            .ok()
            .and_then(|mut g| g.take())
            .unwrap_or_else(|| Err("SQL job lost result".into()));
        self.sql_job = None;
        self.sql_busy = false;
        self.sql_interrupt = None;
        match result {
            Ok(res) => {
                let n = res.rows.len();
                self.sql_result = Some(res);
                self.sql_message = format!("OK — {n} result row(s)");
                self.status_msg = "SQL ok".into();
                self.error_msg = None;
                let _ = self.refresh_schema();
            }
            Err(e) => {
                self.sql_result = None;
                self.error_msg = Some(e.clone());
                self.sql_message = e.clone();
                self.status_msg = e;
            }
        }
        true
    }
}

fn open_or_create_demo(path: &PathBuf) -> Result<Connection, String> {
    if path.exists() {
        db::open_path(path.to_str().unwrap_or(":memory:"))
    } else {
        let conn = db::create_new(path.to_str().unwrap())?;
        db::seed_demo(&conn)?;
        Ok(conn)
    }
}

fn open_shared_memory() -> Result<Connection, String> {
    // Shared-cache URI so a worker connection can see the same :memory: DB.
    use rusqlite::OpenFlags;
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_URI;
    Connection::open_with_flags("file:e28_mem?mode=memory&cache=shared", flags)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod chart_default_tests {
    use super::*;
    use crate::db;

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
