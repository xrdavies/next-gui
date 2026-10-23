//! Application state for the DB Browser example.

use std::path::PathBuf;

use rusqlite::Connection;

use crate::db::{self, ObjectKind, QueryResult, SchemaObject};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainTab {
    Structure = 0,
    Browse = 1,
    Pragmas = 2,
    ExecuteSql = 3,
    Charts = 4,
    SqlLog = 5,
}

impl MainTab {
    pub fn from_index(i: usize) -> Self {
        match i {
            1 => MainTab::Browse,
            2 => MainTab::Pragmas,
            3 => MainTab::ExecuteSql,
            4 => MainTab::Charts,
            5 => MainTab::SqlLog,
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
            MainTab::Charts => "Charts",
            MainTab::SqlLog => "SQL Log",
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
    pub schema: Vec<SchemaObject>,
    pub selected: Option<SelectedObject>,
    pub tab: MainTab,
    pub status_msg: String,
    pub error_msg: Option<String>,

    // Browse
    pub browse_table: Option<String>,
    pub browse_filter: String,
    pub browse_order_col: Option<String>,
    pub browse_order_desc: bool,
    pub browse_result: Option<QueryResult>,
    pub browse_rowids: Vec<i64>,
    pub edit_col: String,
    pub edit_value: String,

    // Structure helpers
    pub new_table_name: String,
    pub new_table_cols: String,
    pub ddl_draft: String,

    // Execute SQL
    pub sql_editor: String,
    pub sql_result: Option<QueryResult>,

    // Pragmas (key, value draft)
    pub pragmas: Vec<(String, String)>,
    pub pragma_value_draft: String,

    // Charts
    pub chart_table: Option<String>,
    pub chart_column: String,
    pub chart_points: Vec<f32>,

    // Log
    pub sql_log: Vec<String>,

    // Import/export paths
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
                state.status_msg = format!("Opened demo DB {}", state.path);
                let _ = state.refresh_schema();
                if let Some(t) = state
                    .schema
                    .iter()
                    .find(|o| o.kind == ObjectKind::Table)
                    .map(|o| o.name.clone())
                {
                    state.browse_table = Some(t.clone());
                    state.chart_table = Some(t);
                    let _ = state.refresh_browse();
                    let _ = state.refresh_chart();
                }
                let _ = state.refresh_pragmas();
            }
            Err(e) => {
                state.status_msg = format!("Demo DB failed: {e}; using in-memory");
                if let Ok(conn) = db::open_path(":memory:") {
                    let _ = db::seed_demo(&conn);
                    state.conn = Some(conn);
                    state.path = ":memory:".into();
                    state.path_draft = state.path.clone();
                    let _ = state.refresh_schema();
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
            schema: Vec::new(),
            selected: None,
            tab: MainTab::Structure,
            status_msg: "Ready".into(),
            error_msg: None,
            browse_table: None,
            browse_filter: String::new(),
            browse_order_col: None,
            browse_order_desc: false,
            browse_result: None,
            browse_rowids: Vec::new(),
            edit_col: String::new(),
            edit_value: String::new(),
            new_table_name: "new_table".into(),
            new_table_cols: "id INTEGER PRIMARY KEY, name TEXT".into(),
            ddl_draft: String::new(),
            sql_editor: "SELECT name, type FROM sqlite_master ORDER BY type, name;\n".into(),
            sql_result: None,
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
        // Prefer rowid browse for base tables; views fall back to plain SELECT.
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
                        // Hide __rowid__ from the grid.
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
        let msg = format!(
            "Browse {} — {} rows{}",
            table,
            result.rows.len(),
            if result.truncated { " (truncated)" } else { "" }
        );
        self.status_msg = msg;
        self.browse_result = Some(result);
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
        self.chart_points =
            db::numeric_series(conn, &table, self.chart_column.trim(), 64).unwrap_or_default();
        Ok(())
    }

    pub fn select_schema_node(&mut self, node_id: u64) {
        // Groups 1..4; objects at 10_000 + idx
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
                self.chart_table = Some(obj.name);
                let _ = self.refresh_browse();
                let _ = self.refresh_chart();
            }
        }
    }

    pub fn table_names(&self) -> Vec<String> {
        self.schema
            .iter()
            .filter(|o| matches!(o.kind, ObjectKind::Table | ObjectKind::View))
            .map(|o| o.name.clone())
            .collect()
    }

    pub fn open_path(&mut self, path: &str) -> Result<(), String> {
        let conn = db::open_path(path)?;
        self.conn = Some(conn);
        self.path = if path.is_empty() {
            ":memory:".into()
        } else {
            path.to_string()
        };
        self.path_draft = self.path.clone();
        self.log_sql(format!("-- open {}", self.path));
        self.refresh_schema()?;
        self.refresh_pragmas()?;
        self.browse_table = self
            .schema
            .iter()
            .find(|o| o.kind == ObjectKind::Table)
            .map(|o| o.name.clone());
        self.chart_table = self.browse_table.clone();
        self.refresh_browse()?;
        self.refresh_chart()?;
        self.status_msg = format!("Opened {}", self.path);
        self.error_msg = None;
        Ok(())
    }

    pub fn create_new_db(&mut self, path: &str) -> Result<(), String> {
        let conn = db::create_new(path)?;
        self.conn = Some(conn);
        self.path = path.to_string();
        self.path_draft = self.path.clone();
        self.schema.clear();
        self.browse_result = None;
        self.sql_result = None;
        self.log_sql(format!("-- create {}", self.path));
        self.refresh_pragmas()?;
        self.status_msg = format!("Created {}", self.path);
        self.error_msg = None;
        Ok(())
    }

    pub fn new_demo_db(&mut self) -> Result<(), String> {
        let path = db::default_demo_path();
        let conn = db::create_new(path.to_str().unwrap())?;
        db::seed_demo(&conn)?;
        self.conn = Some(conn);
        self.path = path.display().to_string();
        self.path_draft = self.path.clone();
        self.log_sql("-- seed demo");
        self.refresh_schema()?;
        self.refresh_pragmas()?;
        self.browse_table = Some("products".into());
        self.chart_table = Some("products".into());
        self.chart_column = "price".into();
        self.refresh_browse()?;
        self.refresh_chart()?;
        self.status_msg = format!("Demo DB ready at {}", self.path);
        Ok(())
    }

    pub fn close_db(&mut self) {
        self.conn = None;
        self.schema.clear();
        self.selected = None;
        self.browse_result = None;
        self.browse_rowids.clear();
        self.sql_result = None;
        self.pragmas.clear();
        self.chart_points.clear();
        self.path.clear();
        self.log_sql("-- close");
        self.status_msg = "Database closed".into();
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
