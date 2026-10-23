//! Application state for the DB Browser example (DB4S chrome parity).

use std::path::PathBuf;

use rusqlite::Connection;

use crate::db::{self, ObjectKind, QueryResult, SchemaObject};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainTab {
    Structure = 0,
    Browse = 1,
    Pragmas = 2,
    ExecuteSql = 3,
}

impl MainTab {
    #[allow(dead_code)]
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

/// Which right-side dock panel is focused when several are visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockKind {
    EditCell = 0,
    Schema = 1,
    Log = 2,
    Plot = 3,
}

impl DockKind {
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

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SelectedObject {
    pub kind: ObjectKind,
    pub name: String,
    pub sql: Option<String>,
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

    // Dock visibility (View menu)
    pub dock_edit: bool,
    pub dock_schema: bool,
    pub dock_log: bool,
    pub dock_plot: bool,
    pub dock_focus: DockKind,

    // Browse
    pub browse_table: Option<String>,
    pub browse_filter: String,
    pub browse_order_col: Option<String>,
    pub browse_order_desc: bool,
    pub browse_result: Option<QueryResult>,
    pub browse_rowids: Vec<i64>,
    pub edit_col: String,
    pub edit_value: String,
    pub browse_sel_row: Option<usize>,

    // Structure helpers
    pub new_table_name: String,
    pub new_table_cols: String,
    pub new_index_name: String,
    pub new_index_on: String,
    pub ddl_draft: String,

    // Execute SQL (multi-tab stub: editors list)
    pub sql_tabs: Vec<String>,
    pub sql_tab_idx: usize,
    pub sql_result: Option<QueryResult>,
    pub sql_message: String,

    // Pragmas
    pub pragmas: Vec<(String, String)>,
    pub pragma_value_draft: String,

    // Plot dock
    pub chart_table: Option<String>,
    pub chart_column: String,
    pub chart_points: Vec<f32>,

    // Log
    pub sql_log: Vec<String>,

    // Import/export last path
    pub io_path: String,

    // Confirm modal
    pub confirm_kind: ConfirmKind,
    pub confirm_open: bool,
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
        match open_or_create_demo(&demo) {
            Ok(conn) => {
                state.path = demo.display().to_string();
                state.path_draft = state.path.clone();
                state.conn = Some(conn);
                let _ = state.begin_txn();
                state.status_msg = format!("Opened demo DB {}", state.path);
                let _ = state.refresh_schema();
                if let Some(t) = state
                    .schema
                    .iter()
                    .find(|o| o.kind == ObjectKind::Table)
                    .map(|o| o.name.clone())
                {
                    state.browse_table = Some(t);
                    let _ = state.refresh_browse();
                }
                state.pick_chart_source();
                let _ = state.refresh_chart();
                let _ = state.refresh_pragmas();
            }
            Err(e) => {
                state.status_msg = format!("Demo DB failed: {e}; using in-memory");
                if let Ok(conn) = db::open_path(":memory:") {
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
            browse_table: None,
            browse_filter: String::new(),
            browse_order_col: None,
            browse_order_desc: false,
            browse_result: None,
            browse_rowids: Vec::new(),
            edit_col: String::new(),
            edit_value: String::new(),
            browse_sel_row: None,
            new_table_name: "new_table".into(),
            new_table_cols: "id INTEGER PRIMARY KEY, name TEXT".into(),
            new_index_name: "idx_name".into(),
            new_index_on: "new_table(name)".into(),
            ddl_draft: String::new(),
            sql_tabs: vec!["SELECT name, type FROM sqlite_master ORDER BY type, name;\n".into()],
            sql_tab_idx: 0,
            sql_result: None,
            sql_message: "Ready — Execute SQL to see results.".into(),
            pragmas: Vec::new(),
            pragma_value_draft: String::new(),
            chart_table: None,
            chart_column: "price".into(),
            chart_points: Vec::new(),
            sql_log: Vec::new(),
            io_path: String::new(),
            confirm_kind: ConfirmKind::None,
            confirm_open: false,
        }
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
            return Ok(());
        };
        self.pragmas = db::read_all_pragmas(conn);
        Ok(())
    }

    pub fn refresh_browse(&mut self) -> Result<(), String> {
        let Some(conn) = self.conn.as_ref() else {
            self.browse_result = None;
            self.browse_rowids.clear();
            return Ok(());
        };
        let Some(table) = self.browse_table.clone() else {
            self.browse_result = None;
            self.browse_rowids.clear();
            return Ok(());
        };
        let is_view = self
            .schema
            .iter()
            .any(|o| o.name == table && o.kind == ObjectKind::View);
        let filter = if self.browse_filter.trim().is_empty() {
            None
        } else {
            Some(self.browse_filter.as_str())
        };
        let order = self.browse_order_col.as_deref();
        let result = if is_view {
            db::browse_table(conn, &table, order, self.browse_order_desc, filter, 500)?
        } else {
            match db::browse_with_rowid(conn, &table, order, self.browse_order_desc, filter, 500) {
                Ok(mut q) => {
                    self.browse_rowids.clear();
                    if let Some(idx) = q.columns.iter().position(|c| c == "__rowid__") {
                        for row in &q.rows {
                            let id = row
                                .get(idx)
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(-1);
                            self.browse_rowids.push(id);
                        }
                        q.columns.remove(idx);
                        for row in &mut q.rows {
                            if idx < row.len() {
                                row.remove(idx);
                            }
                        }
                    }
                    q
                }
                Err(_) => {
                    self.browse_rowids.clear();
                    db::browse_table(conn, &table, order, self.browse_order_desc, filter, 500)?
                }
            }
        };
        if is_view {
            self.browse_rowids.clear();
        }
        let order_note = match &self.browse_order_col {
            Some(c) => format!(
                " ORDER BY {} {}",
                c,
                if self.browse_order_desc { "DESC" } else { "ASC" }
            ),
            None => String::new(),
        };
        self.status_msg = format!(
            "Browse {} — {} rows{}{}",
            table,
            result.rows.len(),
            if result.truncated { " (truncated)" } else { "" },
            order_note
        );
        self.browse_result = Some(result);
        Ok(())
    }

    pub fn sync_edit_cell_from_selection(&mut self) {
        let Some(row) = self.browse_sel_row else {
            return;
        };
        let Some(res) = self.browse_result.as_ref() else {
            return;
        };
        let Some(cells) = res.rows.get(row) else {
            return;
        };
        if self.edit_col.trim().is_empty() {
            if let Some(c0) = res.columns.first() {
                self.edit_col = c0.clone();
            }
        }
        if let Some(ci) = res.columns.iter().position(|c| c == &self.edit_col) {
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
                self.browse_table = Some(obj.name.clone());
                let _ = self.refresh_browse();
            }
        }
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

    fn adopt_conn(&mut self, conn: Connection, path: String, read_only: bool) -> Result<(), String> {
        self.conn = Some(conn);
        self.path = path;
        self.path_draft = self.path.clone();
        self.read_only = read_only;
        self.dirty = false;
        self.begin_txn()?;
        self.refresh_schema()?;
        self.refresh_pragmas()?;
        self.browse_table = self
            .schema
            .iter()
            .find(|o| o.kind == ObjectKind::Table)
            .map(|o| o.name.clone());
        self.pick_chart_source();
        self.refresh_browse()?;
        self.refresh_chart()?;
        self.error_msg = None;
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
        self.browse_result = None;
        self.sql_result = None;
        self.adopt_conn(conn, path.to_string(), false)?;
        self.status_msg = format!("Created {path}");
        Ok(())
    }

    pub fn new_memory_db(&mut self) -> Result<(), String> {
        let conn = db::open_path(":memory:")?;
        self.log_sql("-- new in-memory");
        self.schema.clear();
        self.browse_result = None;
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
        self.browse_table = Some("products".into());
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
        self.browse_result = None;
        self.browse_rowids.clear();
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

#[cfg(test)]
mod chart_default_tests {
    use super::*;
    use crate::db;

    #[test]
    fn pick_chart_prefers_products_price() {
        let state = AppState::new_with_demo();
        assert_eq!(state.chart_table.as_deref(), Some("products"));
        assert_eq!(state.chart_column, "price");
        assert!(
            !state.chart_points.is_empty(),
            "expected numeric price series, got {:?}",
            state.chart_points
        );
        assert!(state.chart_points.iter().any(|v| *v > 10.0));
        let _ = db::default_demo_path();
    }

    #[test]
    fn main_tab_count_is_four() {
        assert_eq!(MainTab::ExecuteSql.index(), 3);
        assert_eq!(MainTab::from_index(4), MainTab::Structure);
    }
}
