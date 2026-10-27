#![allow(dead_code)]
//! SQLite helpers for the DB Browser example (rusqlite, bundled).

use std::fs::{self, File};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use rusqlite::{params_from_iter, Connection, OpenFlags};
use crate::cell_snapshot::CellSnapshot;

/// Schema object kinds shown in the structure tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ObjectKind {
    Table,
    View,
    Index,
    Trigger,
}

impl ObjectKind {
    pub fn label(self) -> &'static str {
        match self {
            ObjectKind::Table => "Tables",
            ObjectKind::View => "Views",
            ObjectKind::Index => "Indexes",
            ObjectKind::Trigger => "Triggers",
        }
    }

    pub fn sql_type(self) -> &'static str {
        match self {
            ObjectKind::Table => "table",
            ObjectKind::View => "view",
            ObjectKind::Index => "index",
            ObjectKind::Trigger => "trigger",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SchemaObject {
    pub schema: String,
    pub kind: ObjectKind,
    pub name: String,
    pub sql: Option<String>,
    pub tbl_name: Option<String>,
}

/// Owned SQLite cell, including TEXT bytes that are not valid UTF-8.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CellValue {
    Null,
    Integer(i64),
    Real(#[serde(with = "crate::cell_snapshot::real_bits")] f64),
    Text(Vec<u8>),
    Blob(Vec<u8>),
}

impl CellValue {
    pub fn as_ref(&self) -> rusqlite::types::ValueRef<'_> {
        use rusqlite::types::ValueRef;
        match self {
            Self::Null => ValueRef::Null,
            Self::Integer(n) => ValueRef::Integer(*n),
            Self::Real(n) => ValueRef::Real(*n),
            Self::Text(bytes) => ValueRef::Text(bytes),
            Self::Blob(bytes) => ValueRef::Blob(bytes),
        }
    }
}

impl From<rusqlite::types::ValueRef<'_>> for CellValue {
    fn from(value: rusqlite::types::ValueRef<'_>) -> Self {
        use rusqlite::types::ValueRef;
        match value {
            ValueRef::Null => Self::Null,
            ValueRef::Integer(n) => Self::Integer(n),
            ValueRef::Real(n) => Self::Real(n),
            ValueRef::Text(bytes) => Self::Text(bytes.to_vec()),
            ValueRef::Blob(bytes) => Self::Blob(bytes.to_vec()),
        }
    }
}

impl rusqlite::ToSql for CellValue {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed(self.as_ref()))
    }
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub statement: Option<String>,
    pub rows: Vec<Vec<String>>,
    pub values: CellSnapshot,
    pub rows_affected: Option<u64>,
    pub truncated: bool,
    pub export_rows: Option<std::sync::Arc<tempfile::NamedTempFile>>,
}

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub cid: i64,
    pub name: String,
    pub type_name: String,
    pub notnull: bool,
    pub default_value: Option<String>,
    pub pk: bool,
}

/// Editable column definition for Create/Modify Table wizard.
#[derive(Debug, Clone)]
pub struct ColDef {
    pub name: String,
    pub type_name: String,
    pub pk: bool,
    pub auto_increment: bool,
    pub collation: String,
    pub check_expression: String,
    pub foreign_key_clause: String,
    pub notnull: bool,
    pub unique: bool,
    pub default_value: String,
}

impl ColDef {
    pub fn new(name: impl Into<String>, type_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            type_name: type_name.into(),
            pk: false,
            auto_increment: false,
            collation: String::new(),
            check_expression: String::new(), foreign_key_clause: String::new(),
            notnull: false,
            unique: false,
            default_value: String::new(),
        }
    }
}

pub fn table_options_in(conn: &Connection, schema: &str, table: &str) -> Result<crate::table_sql::TableOptions, String> {
    let sql: String = conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE type='table' AND name=?1", quote_ident(schema)), [table], |row| row.get(0)).map_err(|e| e.to_string())?;
    crate::table_sql::table_options(&sql)
}

pub const COLUMN_TYPES: &[&str] = &["INTEGER", "TEXT", "REAL", "BLOB", "NUMERIC", "VARCHAR(255)"];

/// Open connection (file or `:memory:`).
pub fn open_path(path: &str) -> Result<crate::sql_log::LoggedConnection, String> {
    let conn = if path == ":memory:" || path.is_empty() {
        Connection::open_in_memory()
    } else {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_URI;
        Connection::open_with_flags(path, flags)
    }.map_err(|e| e.to_string())?;
    crate::filters::register_regexp(&conn)?;
    crate::sql_log::LoggedConnection::new(conn)
}

/// Open an existing DB read-only (no CREATE).
pub fn open_readonly(path: &str) -> Result<crate::sql_log::LoggedConnection, String> {
    if path == ":memory:" || path.is_empty() {
        return Err("read-only open requires a file path".into());
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI;
    let conn = Connection::open_with_flags(path, flags).map_err(|e| e.to_string())?;
    crate::filters::register_regexp(&conn)?;
    crate::sql_log::LoggedConnection::new(conn)
}

/// Begin a deferred write transaction (DB4S Write Changes model).
pub fn begin_deferred(conn: &Connection) -> Result<(), String> {
    if conn.is_autocommit() {
        conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Include schema/PRAGMA writes and attached databases, not just changed row counts.
pub fn has_pending_changes(conn: &Connection) -> Result<bool, String> {
    conn.transaction_state(None)
        .map(|state| state == rusqlite::TransactionState::Write)
        .map_err(|e| e.to_string())
}

/// COMMIT pending edits and open a fresh transaction.
pub fn commit_and_begin(conn: &Connection) -> Result<(), String> {
    if !conn.is_autocommit() {
        conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
    }
    begin_deferred(conn)
}

/// ROLLBACK pending edits and open a fresh transaction.
pub fn rollback_and_begin(conn: &Connection) -> Result<(), String> {
    if !conn.is_autocommit() {
        conn.execute_batch("ROLLBACK").map_err(|e| e.to_string())?;
    }
    begin_deferred(conn)
}

/// Snapshot the live database, including WAL pages, to a new destination.
pub fn copy_database_file(conn: &Connection, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        return Err("Save As requires a new path; existing databases are never overwritten".into());
    }
    let parent = dst
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    conn.backup(rusqlite::DatabaseName::Main, file.path(), None)
        .map_err(|e| e.to_string())?;
    file.persist_noclobber(dst).map_err(|e| e.to_string())?;
    Ok(())
}

/// Create a new empty DB; never truncate an existing database.
pub fn create_new(path: &str) -> Result<crate::sql_log::LoggedConnection, String> {
    if path != ":memory:" {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
    }
    open_path(path)
}

/// Seed demo schema used on first launch / “New demo DB”.
pub fn seed_demo(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        DROP TABLE IF EXISTS order_items;
        DROP TABLE IF EXISTS orders;
        DROP TABLE IF EXISTS products;
        DROP TABLE IF EXISTS customers;
        CREATE TABLE customers (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT,
            city TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        );
        CREATE TABLE products (
            id INTEGER PRIMARY KEY,
            sku TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            price REAL NOT NULL CHECK(price >= 0),
            stock INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE orders (
            id INTEGER PRIMARY KEY,
            customer_id INTEGER NOT NULL REFERENCES customers(id),
            total REAL NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'open',
            placed_at TEXT DEFAULT (datetime('now'))
        );
        CREATE TABLE order_items (
            id INTEGER PRIMARY KEY,
            order_id INTEGER NOT NULL REFERENCES orders(id),
            product_id INTEGER NOT NULL REFERENCES products(id),
            qty INTEGER NOT NULL CHECK(qty > 0),
            unit_price REAL NOT NULL
        );
        CREATE INDEX idx_orders_customer ON orders(customer_id);
        CREATE INDEX idx_items_order ON order_items(order_id);
        CREATE VIEW v_order_summary AS
            SELECT o.id AS order_id, c.name AS customer, o.total, o.status, o.placed_at
            FROM orders o JOIN customers c ON c.id = o.customer_id;
        CREATE TRIGGER trg_orders_total AFTER INSERT ON order_items
        BEGIN
            UPDATE orders SET total = (
                SELECT COALESCE(SUM(qty * unit_price), 0) FROM order_items WHERE order_id = NEW.order_id
            ) WHERE id = NEW.order_id;
        END;
        INSERT INTO customers (name, email, city) VALUES
            ('Alice', 'alice@example.com', 'Shanghai'),
            ('Bob', 'bob@example.com', 'Beijing'),
            ('Carol', 'carol@example.com', 'Shenzhen');
        INSERT INTO products (sku, name, price, stock) VALUES
            ('SKU-100', 'Widget', 19.9, 100),
            ('SKU-200', 'Gadget', 49.5, 40),
            ('SKU-300', 'Doohickey', 9.99, 250),
            ('SKU-400', 'Thingamajig', 120.0, 12);
        INSERT INTO orders (customer_id, status) VALUES (1, 'open'), (2, 'shipped'), (1, 'open');
        INSERT INTO order_items (order_id, product_id, qty, unit_price) VALUES
            (1, 1, 2, 19.9),
            (1, 3, 5, 9.99),
            (2, 2, 1, 49.5),
            (3, 4, 1, 120.0),
            (3, 1, 3, 19.9);
        "#,
    )
    .map_err(|e| e.to_string())
}

pub fn list_schema(conn: &Connection) -> Result<Vec<SchemaObject>, String> {
    list_schema_in(conn, "main")
}

pub fn database_names(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare("PRAGMA database_list").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| row.get(1)).map_err(|e| e.to_string())?;
    rows
        .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn list_schema_in(conn: &Connection, schema: &str) -> Result<Vec<SchemaObject>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT type, name, tbl_name, sql FROM {}.sqlite_schema
             WHERE name NOT GLOB 'sqlite_*'
             ORDER BY CASE type
                WHEN 'table' THEN 0
                WHEN 'view' THEN 1
                WHEN 'index' THEN 2
                WHEN 'trigger' THEN 3
                ELSE 4 END, name COLLATE NOCASE", quote_ident(schema),
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let type_s: String = row.get(0)?;
            let kind = match type_s.as_str() {
                "table" => ObjectKind::Table,
                "view" => ObjectKind::View,
                "index" => ObjectKind::Index,
                "trigger" => ObjectKind::Trigger,
                _ => ObjectKind::Table,
            };
            Ok(SchemaObject {
                schema: schema.into(),
                kind,
                name: row.get(1)?,
                tbl_name: row.get(2)?,
                sql: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn table_columns(conn: &Connection, table: &str) -> Result<Vec<ColumnInfo>, String> {
    table_columns_in(conn, "main", table)
}

pub fn table_columns_in(conn: &Connection, schema: &str, table: &str) -> Result<Vec<ColumnInfo>, String> {
    let q = format!("PRAGMA {}.table_info({})", quote_ident(schema), quote_ident(table));
    let mut stmt = conn.prepare(&q).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ColumnInfo {
                cid: row.get(0)?,
                name: row.get(1)?,
                type_name: row.get::<_, String>(2).unwrap_or_default(),
                notnull: row.get::<_, i64>(3).unwrap_or(0) != 0,
                default_value: row.get(4)?,
                pk: row.get::<_, i64>(5).unwrap_or(0) != 0,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn table_column_defs_in(conn: &Connection, schema: &str, table: &str) -> Result<Vec<ColDef>, String> {
    let sql: String = conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE type='table' AND name=?1", quote_ident(schema)), [table], |row| row.get(0)).map_err(|e| e.to_string())?;
    let auto_increment = crate::table_sql::autoincrement_column(&sql)?;
    let mut unique_columns = Vec::new();
    let mut stmt = conn.prepare(&format!("PRAGMA {}.index_list({})", quote_ident(schema), quote_literal(table))).map_err(|e| e.to_string())?;
    let indexes = stmt.query_map([], |row| Ok((row.get::<_, String>(1)?, row.get::<_, String>(3)?))).map_err(|e| e.to_string())?;
    for index in indexes {
        let (index, origin) = index.map_err(|e| e.to_string())?;
        if origin != "u" { continue; }
        let mut stmt = conn.prepare(&format!("PRAGMA {}.index_info({})", quote_ident(schema), quote_literal(&index))).map_err(|e| e.to_string())?;
        let columns = stmt.query_map([], |row| row.get::<_, Option<String>>(2)).map_err(|e| e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?;
        if let [Some(column)] = columns.as_slice() { unique_columns.push(column.clone()); }
    }
    let mut columns = table_columns_in(conn, schema, table)?.into_iter().map(|col| ColDef {
        auto_increment: auto_increment.as_ref().is_some_and(|name| name.eq(col.name.as_str())),
        collation: String::new(), check_expression: String::new(), foreign_key_clause: String::new(),
        unique: unique_columns.iter().any(|name| name.eq_ignore_ascii_case(&col.name)),
        name: col.name, type_name: col.type_name, pk: col.pk, notnull: col.notnull,
        default_value: col.default_value.unwrap_or_default(),
    }).collect::<Vec<_>>();
    crate::table_sql::read_column_constraints(&sql, &mut columns)?;
    Ok(columns)
}

pub fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

pub fn qualified_name(schema: &str, name: &str) -> String {
    format!("{}.{}", quote_ident(schema), quote_ident(name))
}

fn cell_to_string(value: rusqlite::types::ValueRef<'_>) -> String {
    use rusqlite::types::ValueRef;
    match value {
        ValueRef::Null => String::new(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => {
            if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{:.0}", f)
            } else {
                format!("{f}")
            }
        }
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
        ValueRef::Blob(b) => format!("<blob {} bytes>", b.len()),
    }
}

pub const DEFAULT_LIMIT: usize = 500;

pub fn index_table_columns(conn: &Connection, schema: &str, table: &str) -> Result<Vec<(String, String)>, String> {
    let mut stmt = conn.prepare(&format!("PRAGMA {}.table_xinfo({})", quote_ident(schema), quote_ident(table))).map_err(|e| e.to_string())?;
    let columns = stmt.query_map([], |row| Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, i64>(6)?))).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for column in columns {
        let (name, type_name, hidden) = column.map_err(|e| e.to_string())?;
        if hidden != 1 { out.push((name, type_name)); }
    }
    if out.is_empty() { return Err(format!("No indexable columns in {}", qualified_name(schema, table))); }
    Ok(out)
}

/// Browse a table/view with optional ORDER BY, WHERE filter, and limit.
pub fn table_column_names(conn: &Connection, table: &str) -> Result<Vec<String>, String> {
    table_column_names_in(conn, "main", table)
}

pub fn table_column_names_in(conn: &Connection, schema: &str, table: &str) -> Result<Vec<String>, String> {
    let stmt = conn
        .prepare(&format!("SELECT * FROM {} LIMIT 0", qualified_name(schema, table)))
        .map_err(|e| e.to_string())?;
    Ok(stmt.column_names().into_iter().map(String::from).collect())
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OrderBy {
    pub column: String,
    pub descending: bool,
}

fn append_order_by(conn: &Connection, schema: &str, table: &str, sql: &mut String, ordering: &[OrderBy]) -> Result<(), String> {
    if ordering.is_empty() { return Ok(()); }
    let columns = table_column_names_in(conn, schema, table)?;
    if let Some(order) = ordering.iter().find(|order| !columns.iter().any(|column| column.eq_ignore_ascii_case(&order.column))) {
        return Err(format!("Sort column does not exist: {}", order.column));
    }
    sql.push_str(" ORDER BY ");
    for (index, order) in ordering.iter().enumerate() {
        if index > 0 { sql.push_str(", "); }
        sql.push_str(&quote_ident(&order.column));
        sql.push_str(if order.descending { " DESC" } else { " ASC" });
    }
    Ok(())
}

pub fn browse_table(
    conn: &Connection,
    table: &str,
    ordering: &[OrderBy],
    filter: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<QueryResult, String> {
    browse_table_in(conn, "main", table, ordering, filter, limit, offset)
}

pub fn browse_table_in(
    conn: &Connection, schema: &str,
    table: &str,
    ordering: &[OrderBy],
    filter: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<QueryResult, String> {
    let limit = if limit == 0 { DEFAULT_LIMIT } else { limit };
    let mut sql = format!("SELECT * FROM {}", qualified_name(schema, table));
    if let Some(f) = filter.map(str::trim).filter(|s| !s.is_empty()) {
        sql.push_str(" WHERE ");
        sql.push_str(f);
    }
    append_order_by(conn, schema, table, &mut sql, ordering)?;
    sql.push_str(&format!(
        " LIMIT {} OFFSET {offset}",
        limit.saturating_add(1)
    ));
    query_preview(conn, &sql, limit)
}

/// Count the rows matching the same filter used by the browse preview.
pub fn count_browse_rows(
    conn: &Connection,
    table: &str,
    filter: Option<&str>,
) -> Result<usize, String> {
    count_browse_rows_in(conn, "main", table, filter)
}

pub fn count_browse_rows_in(
    conn: &Connection, schema: &str,
    table: &str,
    filter: Option<&str>,
) -> Result<usize, String> {
    let mut sql = format!("SELECT COUNT(*) FROM {}", qualified_name(schema, table));
    if let Some(f) = filter.map(str::trim).filter(|s| !s.is_empty()) {
        sql.push_str(" WHERE ");
        sql.push_str(f);
    }
    let mut stmt = prepare_browse(conn, &sql)?;
    let count: i64 = stmt
        .query_row([], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    usize::try_from(count).map_err(|_| "Browse row count is negative".into())
}

pub(super) fn prepare_single_statement<'conn>(conn: &'conn Connection, sql: &str) -> Result<rusqlite::Statement<'conn>, String> {
    let stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    // Expanded SQL contains only the first statement; never prepare a PRAGMA in the tail.
    if stmt.expanded_sql().as_deref() != Some(sql) {
        return Err("SQL fields require a single statement without parameters".into());
    }
    Ok(stmt)
}

pub fn execute_single_statement(conn: &Connection, sql: &str) -> Result<(), String> {
    prepare_single_statement(conn, sql)?.execute([]).map(|_| ()).map_err(|e| e.to_string())
}

fn prepare_browse<'conn>(conn: &'conn Connection, sql: &str) -> Result<rusqlite::Statement<'conn>, String> {
    let stmt = prepare_single_statement(conn, sql)?;
    if !stmt.readonly() {
        return Err("Browse requires a single read-only statement".into());
    }
    Ok(stmt)
}

fn query_preview(conn: &Connection, sql: &str, limit: usize) -> Result<QueryResult, String> {
    let mut stmt = prepare_browse(conn, sql)?;
    let columns = stmt
        .column_names()
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>();
    let mut query = stmt.query([]).map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    let mut values = CellSnapshot::default();
    let mut truncated = false;
    while let Some(row) = query.next().map_err(|e| e.to_string())? {
        if rows.len() == limit {
            truncated = true;
            break;
        }
        let cells: Vec<CellValue> = (0..columns.len())
            .map(|i| row.get_ref(i).map(CellValue::from))
            .collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())?;
        rows.push(cells.iter().map(|cell| cell_to_string(cell.as_ref())).collect());
        values.push(cells)?;
    }
    Ok(QueryResult {
        statement: None,
        columns,
        rows,
        values,
        rows_affected: None,
        truncated,
        export_rows: None,
    })
}

/// Run arbitrary SQL. For multi-statement scripts, only the last result set is returned;
/// non-query statements return rows_affected.
pub fn execute_sql(conn: &Connection, sql: &str) -> Result<QueryResult, String> {
    let sql = sql.trim();
    if sql.is_empty() {
        return Err("empty SQL".into());
    }

    let mut batch = rusqlite::Batch::new(conn, sql);
    let mut result = None;
    while let Some(mut stmt) = batch.next().map_err(|e| e.to_string())? {
        let columns: Vec<String> = stmt
            .column_names()
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let col_count = columns.len();
        let readonly = stmt.readonly();
        let statement = readonly.then(|| stmt.expanded_sql()).flatten();
        let mut rows_iter = stmt.query([]).map_err(|e| e.to_string())?;
        let mut export_rows = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
        let mut output = std::io::BufWriter::new(&mut export_rows);
        let mut rows = Vec::new();
        let mut cell_values = CellSnapshot::default();
        let mut truncated = false;
        while let Some(row) = rows_iter.next().map_err(|e| e.to_string())? {
            let values = (0..col_count)
                .map(|i| json_value(row.get_ref(i).map_err(|e| e.to_string())?))
                .collect::<Result<Vec<_>, String>>()?;
            serde_json::to_writer(&mut output, &values).map_err(|e| e.to_string())?;
            writeln!(output).map_err(|e| e.to_string())?;
            if rows.len() == DEFAULT_LIMIT {
                truncated = true;
                continue;
            }
            let cells: Vec<CellValue> = (0..col_count)
                .map(|i| row.get_ref(i).map(CellValue::from))
                .collect::<rusqlite::Result<_>>().map_err(|e| e.to_string())?;
            rows.push(cells.iter().map(|cell| cell_to_string(cell.as_ref())).collect());
            cell_values.push(cells)?;
        }
        output.flush().map_err(|e| e.to_string())?;
        drop(output);
        result = Some(QueryResult {
            statement,
            export_rows: Some(std::sync::Arc::new(export_rows)),
            columns,
            rows,
            values: cell_values,
            rows_affected: (!readonly).then(|| conn.changes()),
            truncated,
        });
    }
    result.ok_or_else(|| "No SQL statement to execute".into())
}

pub fn vacuum(conn: &Connection) -> Result<(), String> {
    // VACUUM cannot run inside a transaction.
    if !conn.is_autocommit() {
        conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
    }
    let result = conn.execute_batch("VACUUM").map_err(|e| e.to_string());
    begin_deferred(conn)?;
    result
}

pub fn pragma_check(conn: &Connection, name: &str) -> Result<QueryResult, String> {
    execute_sql(conn, &format!("PRAGMA {name}"))
}

pub fn row_counts_sql(conn: &Connection) -> Result<QueryResult, String> {
    let schema = list_schema(conn)?;
    let mut rows = Vec::new();
    let mut values = CellSnapshot::default();
    for obj in schema.iter().filter(|o| o.kind == ObjectKind::Table) {
        let n = count_rows(conn, &obj.name).unwrap_or(-1);
        rows.push(vec![obj.name.clone(), n.to_string()]);
        values.push(vec![CellValue::Text(obj.name.as_bytes().to_vec()), CellValue::Integer(n)])?;
    }
    Ok(QueryResult {
        statement: None,
        export_rows: None,
        columns: vec!["table".into(), "rows".into()],
        rows,
        values,
        rows_affected: None,
        truncated: false,
    })
}

pub fn count_rows(conn: &Connection, table: &str) -> Result<i64, String> {
    let sql = format!("SELECT COUNT(*) FROM {}", qualified_name("main", table));
    conn.query_row(&sql, [], |r| r.get(0))
        .map_err(|e| e.to_string())
}

pub fn insert_row(
    conn: &Connection,
    table: &str,
    columns: &[String],
    values: &[String],
) -> Result<u64, String> {
    if columns.len() != values.len() {
        return Err("column/value length mismatch".into());
    }
    if columns.is_empty() {
        let sql = format!("INSERT INTO {} DEFAULT VALUES", qualified_name("main", table));
        conn.execute(&sql, []).map_err(|e| e.to_string())?;
        return Ok(conn.changes());
    }
    let cols = columns
        .iter()
        .map(|c| quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = (1..=columns.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO {} ({cols}) VALUES ({placeholders})",
        qualified_name("main", table)
    );
    let params: Vec<&str> = values.iter().map(|s| s.as_str()).collect();
    conn.execute(&sql, params_from_iter(params))
        .map_err(|e| e.to_string())?;
    Ok(conn.changes())
}

pub fn delete_row_by_rowid(conn: &Connection, table: &str, rowid: i64) -> Result<u64, String> {
    delete_row_by_rowid_in(conn, "main", table, rowid)
}

pub fn delete_row_by_rowid_in(conn: &Connection, schema: &str, table: &str, rowid: i64) -> Result<u64, String> {
    let sql = format!("DELETE FROM {} WHERE rowid = ?1", qualified_name(schema, table));
    conn.execute(&sql, [rowid]).map_err(|e| e.to_string())?;
    Ok(conn.changes())
}

pub fn update_cell(
    conn: &Connection,
    table: &str,
    rowid: i64,
    column: &str,
    value: &rusqlite::types::Value,
) -> Result<u64, String> {
    update_cell_in(conn, "main", table, rowid, column, value)
}

pub fn update_cell_in(
    conn: &Connection, schema: &str,
    table: &str,
    rowid: i64,
    column: &str,
    value: &rusqlite::types::Value,
) -> Result<u64, String> {
    let sql = format!(
        "UPDATE {} SET {} = ?1 WHERE rowid = ?2",
        qualified_name(schema, table),
        quote_ident(column)
    );
    conn.execute(&sql, rusqlite::params![value, rowid])
        .map_err(|e| e.to_string())?;
    Ok(conn.changes())
}

/// Update a rectangular set of existing records as one undoable operation.
pub fn update_cells(
    conn: &Connection,
    table: &str,
    columns: &[String],
    updates: &[(i64, Vec<CellValue>)],
) -> Result<(), String> {
    update_cells_in(conn, "main", table, columns, updates)
}

pub fn update_cells_in(
    conn: &Connection, schema: &str,
    table: &str,
    columns: &[String],
    updates: &[(i64, Vec<CellValue>)],
) -> Result<(), String> {
    if columns.is_empty() || updates.is_empty()
        || updates.iter().any(|(_, row)| row.len() != columns.len())
    {
        return Err("Cell update dimensions do not match".into());
    }
    let assignments = columns.iter().enumerate()
        .map(|(i, col)| format!("{} = ?{}", quote_ident(col), i + 1))
        .collect::<Vec<_>>().join(", ");
    let sql = format!("UPDATE {} SET {assignments} WHERE rowid = ?{}", qualified_name(schema, table), columns.len() + 1);
    with_savepoint(conn, || {
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        for (rowid, values) in updates {
            let key = CellValue::Integer(*rowid);
            let changed = stmt.execute(params_from_iter(values.iter().chain(std::iter::once(&key))))
                .map_err(|e| e.to_string())?;
            if changed != 1 {
                return Err(format!("Record {rowid} changed or no longer exists; refresh and retry"));
            }
        }
        Ok(())
    })
}

pub fn browse_with_rowid(
    conn: &Connection,
    table: &str,
    ordering: &[OrderBy],
    filter: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<QueryResult, String> {
    browse_with_rowid_in(conn, "main", table, ordering, filter, limit, offset)
}

pub fn browse_with_rowid_in(
    conn: &Connection, schema: &str,
    table: &str,
    ordering: &[OrderBy],
    filter: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<QueryResult, String> {
    if conn
        .prepare(&format!("SELECT * FROM {}", qualified_name(schema, table)))
        .map_err(|e| e.to_string())?
        .column_names()
        .iter()
        .any(|c| c.eq_ignore_ascii_case("rowid") || c.eq_ignore_ascii_case("__rowid__"))
    {
        return Err("Grid editing requires an unshadowed rowid".into());
    }
    let limit = if limit == 0 { DEFAULT_LIMIT } else { limit };
    let mut sql = format!("SELECT rowid AS __rowid__, * FROM {}", qualified_name(schema, table));
    if let Some(f) = filter.map(str::trim).filter(|s| !s.is_empty()) {
        sql.push_str(" WHERE ");
        sql.push_str(f);
    }
    append_order_by(conn, schema, table, &mut sql, ordering)?;
    sql.push_str(&format!(
        " LIMIT {} OFFSET {offset}",
        limit.saturating_add(1)
    ));
    query_preview(conn, &sql, limit)
}

pub fn drop_object(conn: &Connection, kind: ObjectKind, schema: &str, name: &str) -> Result<(), String> {
    let kw = match kind {
        ObjectKind::Table => "TABLE",
        ObjectKind::View => "VIEW",
        ObjectKind::Index => "INDEX",
        ObjectKind::Trigger => "TRIGGER",
    };
    let sql = format!("DROP {kw} {}.{}", quote_ident(schema), quote_ident(name));
    execute_single_statement(conn, &sql)
}

pub fn create_table_simple(conn: &Connection, name: &str, columns_ddl: &str) -> Result<(), String> {
    let sql = format!(
        "CREATE TABLE {} ({})",
        quote_ident(name),
        columns_ddl.trim().trim_end_matches(',')
    );
    execute_single_statement(conn, &sql)
}

pub fn create_index(
    conn: &Connection,
    name: &str,
    table: &str,
    columns: &str,
    unique: bool,
) -> Result<(), String> {
    create_index_in(conn, "main", name, table, columns, unique, "")
}

pub fn create_index_in(
    conn: &Connection,
    schema: &str,
    name: &str,
    table: &str,
    columns: &str,
    unique: bool,
    predicate: &str,
) -> Result<(), String> {
    execute_single_statement(conn, &create_index_ddl_in(schema, name, table, columns, unique, predicate))
}

pub fn create_index_ddl_in(
    schema: &str, name: &str, table: &str, columns: &str, unique: bool, predicate: &str,
) -> String {
    let uniq = if unique { "UNIQUE " } else { "" };
    // SQLite takes the database from the index name; ON must use an unqualified table.
    let mut sql = format!(
        "CREATE {uniq}INDEX {} ON {} (\n{}\n)",
        qualified_name(schema, name),
        quote_ident(table),
        columns.trim()
    );
    if !predicate.trim().is_empty() { sql.push_str(" WHERE "); sql.push_str(predicate.trim()); }
    sql
}

/// Common pragmas for the Edit Pragmas tab.
pub const PRAGMA_KEYS: &[&str] = &[
    "journal_mode",
    "synchronous",
    "foreign_keys",
    "cache_size",
    "temp_store",
    "locking_mode",
    "auto_vacuum",
    "busy_timeout",
    "recursive_triggers",
    "wal_autocheckpoint",
];

pub fn read_pragma(conn: &Connection, key: &str) -> Result<String, String> {
    let sql = format!("PRAGMA {key}");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    if let Some(row) = rows.next().map_err(|e| e.to_string())? {
        Ok(cell_to_string(row.get_ref(0).map_err(|e| e.to_string())?))
    } else {
        Ok(String::new())
    }
}

pub fn write_pragma(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    if !PRAGMA_KEYS.contains(&key) {
        return Err("Unsupported pragma".into());
    }
    conn.pragma_update(None, key, value)
        .map_err(|e| e.to_string())
}

pub fn read_all_pragmas(conn: &Connection) -> Vec<(String, String)> {
    PRAGMA_KEYS
        .iter()
        .map(|k| {
            let v = read_pragma(conn, k).unwrap_or_else(|e| format!("err: {e}"));
            ((*k).to_string(), v)
        })
        .collect()
}

/// Very small CSV writer (RFC4180-ish quoting).
pub fn export_csv(path: &Path, columns: &[String], rows: &[Vec<String>]) -> Result<(), String> {
    let mut f = File::create(path).map_err(|e| e.to_string())?;
    writeln!(
        f,
        "{}",
        columns
            .iter()
            .map(|c| csv_escape(c))
            .collect::<Vec<_>>()
            .join(",")
    )
    .map_err(|e| e.to_string())?;
    for row in rows {
        let line = (0..columns.len())
            .map(|i| csv_escape(row.get(i).map(|s| s.as_str()).unwrap_or("")))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(f, "{line}").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct CsvImportOptions {
    pub has_header: bool,
    pub delimiter: char,
    pub quote: Option<char>,
}

impl Default for CsvImportOptions {
    fn default() -> Self {
        Self {
            has_header: true,
            delimiter: ',',
            quote: Some('"'),
        }
    }
}

/// Parse a CSV file into header + rows (comma / header defaults).
pub fn import_csv_parse(path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    import_csv_parse_opts(path, &CsvImportOptions::default())
}

/// Parse CSV with delimiter / quote / header options.
pub fn import_csv_parse_opts(
    path: &Path,
    opts: &CsvImportOptions,
) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let raw = fs::read(path).map_err(|e| e.to_string())?;
    let text = decode_csv_bytes(&raw)?;
    import_csv_from_text(&text, opts)
}

fn decode_csv_bytes(raw: &[u8]) -> Result<String, String> {
    if raw.starts_with(&[0xff, 0xfe, 0, 0]) || raw.starts_with(&[0, 0, 0xfe, 0xff]) {
        return Err("CSV UTF-32 is not supported; convert to UTF-8 or UTF-16".into());
    }
    if raw.starts_with(&[0xff, 0xfe]) {
        let units = raw[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        if raw[2..].len() % 2 != 0 {
            return Err("CSV UTF-16LE has an incomplete code unit".into());
        }
        return String::from_utf16(&units).map_err(|e| format!("CSV UTF-16LE is invalid: {e}"));
    }
    if raw.starts_with(&[0xfe, 0xff]) {
        let units = raw[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        if raw[2..].len() % 2 != 0 {
            return Err("CSV UTF-16BE has an incomplete code unit".into());
        }
        return String::from_utf16(&units).map_err(|e| format!("CSV UTF-16BE is invalid: {e}"));
    }
    String::from_utf8(raw.to_vec()).map_err(|e| format!("CSV must be UTF-8 or UTF-16 with BOM: {e}"))
}

/// Parse CSV text (clipboard / file contents).
pub fn import_csv_from_text(
    text: &str,
    opts: &CsvImportOptions,
) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut closed = false;
    let mut chars = text.trim_start_matches('\u{feff}').chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if Some(c) == opts.quote {
                if chars.peek() == Some(&c) {
                    field.push(c);
                    chars.next();
                } else {
                    quoted = false;
                    closed = true;
                }
            } else {
                field.push(c);
            }
        } else if Some(c) == opts.quote && field.is_empty() && !closed {
            quoted = true;
        } else if c == opts.delimiter {
            record.push(std::mem::take(&mut field));
            closed = false;
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            record.push(std::mem::take(&mut field));
            records.push(std::mem::take(&mut record));
            closed = false;
        } else if closed {
            return Err("Unexpected text after CSV closing quote".into());
        } else {
            field.push(c);
        }
    }
    if quoted {
        return Err("Unterminated CSV quoted field".into());
    }
    if !field.is_empty() || !record.is_empty() || closed {
        record.push(field);
        records.push(record);
    }
    if records.is_empty() {
        return Err("empty CSV".into());
    }
    let columns = if opts.has_header {
        records.remove(0)
    } else {
        (0..records[0].len())
            .map(|i| format!("column{i}"))
            .collect()
    };
    if columns
        .iter()
        .enumerate()
        .any(|(i, c)| c.is_empty() || columns[..i].iter().any(|p| p.eq_ignore_ascii_case(c)))
        || records.iter().any(|r| r.len() != columns.len())
    {
        return Err("CSV needs nonempty headers and the same number of fields in every row".into());
    }
    Ok((columns, records))
}

pub fn guard_output_path(conn: &Connection, path: &Path) -> Result<(), String> {
    let resolve = |p: &Path| {
        p.canonicalize().or_else(|_| {
            let parent = p
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            parent
                .canonicalize()
                .map(|parent| parent.join(p.file_name().unwrap_or_default()))
        })
    };
    let destination = resolve(path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("PRAGMA database_list")
        .map_err(|e| e.to_string())?;
    let files = stmt
        .query_map([], |row| row.get::<_, String>(2))
        .map_err(|e| e.to_string())?;
    for file in files {
        let file = file.map_err(|e| e.to_string())?;
        if file.is_empty() {
            continue;
        }
        for suffix in ["", "-wal", "-shm", "-journal"] {
            if resolve(Path::new(&format!("{file}{suffix}"))).ok().as_ref() == Some(&destination) {
                return Err("Export cannot overwrite an open database or its journal files".into());
            }
        }
    }
    Ok(())
}

/// Export the complete filtered/sorted table, independent of the grid preview limit.
pub fn export_table(
    conn: &Connection,
    table: &str,
    ordering: &[OrderBy],
    filter: Option<&str>,
    path: &Path,
    json: bool,
) -> Result<(), String> {
    export_table_in(conn, "main", table, ordering, filter, path, json)
}

pub fn export_table_in(
    conn: &Connection, schema: &str,
    table: &str,
    ordering: &[OrderBy],
    filter: Option<&str>,
    path: &Path,
    json: bool,
) -> Result<(), String> {
    guard_output_path(conn, path)?;
    let mut sql = format!("SELECT * FROM {}", qualified_name(schema, table));
    if let Some(filter) = filter.filter(|s| !s.is_empty()) {
        sql.push_str(&format!(" WHERE {filter}"));
    }
    append_order_by(conn, schema, table, &mut sql, ordering)?;
    let mut stmt = prepare_browse(conn, &sql)?;
    let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut out = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    if json {
        write!(out, "[").map_err(|e| e.to_string())?;
    } else {
        writeln!(
            out,
            "{}",
            columns
                .iter()
                .map(|s| csv_escape(s))
                .collect::<Vec<_>>()
                .join(",")
        )
        .map_err(|e| e.to_string())?;
    }
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    let mut first = true;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let mut values = Vec::new();
        let mut object = serde_json::Map::new();
        for (i, col) in columns.iter().enumerate() {
            use rusqlite::types::ValueRef;
            let value = row.get_ref(i).map_err(|e| e.to_string())?;
            let display = match value {
                ValueRef::Blob(v) => format!("X'{}'", to_hex(v)),
                _ => cell_to_string(value),
            };
            if json {
                let value = json_value(value)?;
                object.insert(col.clone(), value);
            } else {
                values.push(csv_escape(&display));
            }
        }
        if json {
            if !first {
                write!(out, ",").map_err(|e| e.to_string())?;
            }
            serde_json::to_writer(&mut out, &object).map_err(|e| e.to_string())?;
        } else {
            writeln!(out, "{}", values.join(",")).map_err(|e| e.to_string())?;
        }
        first = false;
    }
    if json {
        writeln!(out, "]").map_err(|e| e.to_string())?;
    }
    out.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

fn json_value(value: rusqlite::types::ValueRef<'_>) -> Result<serde_json::Value, String> {
    use rusqlite::types::ValueRef;
    Ok(match value {
        ValueRef::Null => serde_json::Value::Null,
        ValueRef::Integer(v) => v.into(),
        ValueRef::Real(v) => serde_json::Number::from_f64(v)
            .map(serde_json::Value::Number)
            .unwrap_or_else(|| v.to_string().into()),
        ValueRef::Text(v) => String::from_utf8_lossy(v).into_owned().into(),
        ValueRef::Blob(v) => format!("X'{}'", to_hex(v)).into(),
    })
}

/// Export the completed SQL result snapshot without executing the SQL again.
pub fn export_query_result(
    conn: &Connection,
    result: &QueryResult,
    path: &Path,
    json: bool,
) -> Result<(), String> {
    guard_output_path(conn, path)?;
    let source = result
        .export_rows
        .as_ref()
        .ok_or("Execute SQL before exporting results")?;
    if result.columns.is_empty() {
        return Err("The statement returned no columns".into());
    }
    if json
        && result
            .columns
            .iter()
            .enumerate()
            .any(|(i, c)| result.columns[..i].contains(c))
    {
        return Err("JSON needs unique column names; use SQL aliases or export CSV".into());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut out = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    if json {
        write!(out, "[").map_err(|e| e.to_string())?;
    } else {
        writeln!(
            out,
            "{}",
            result
                .columns
                .iter()
                .map(|c| csv_escape(c))
                .collect::<Vec<_>>()
                .join(",")
        )
        .map_err(|e| e.to_string())?;
    }
    for (i, line) in std::io::BufReader::new(source.reopen().map_err(|e| e.to_string())?)
        .lines()
        .enumerate()
    {
        let values: Vec<serde_json::Value> =
            serde_json::from_str(&line.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if json {
            if i > 0 {
                write!(out, ",").map_err(|e| e.to_string())?;
            }
            let object: serde_json::Map<_, _> =
                result.columns.iter().cloned().zip(values).collect();
            serde_json::to_writer(&mut out, &object).map_err(|e| e.to_string())?;
        } else {
            let values = values
                .into_iter()
                .map(|v| match v {
                    serde_json::Value::Null => String::new(),
                    serde_json::Value::String(s) => s,
                    v => v.to_string(),
                })
                .map(|s| csv_escape(&s))
                .collect::<Vec<_>>();
            writeln!(out, "{}", values.join(",")).map_err(|e| e.to_string())?;
        }
    }
    if json {
        writeln!(out, "]").map_err(|e| e.to_string())?;
    }
    out.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

/// ATTACH DATABASE.
pub fn attach_database(conn: &Connection, path: &str, schema: &str) -> Result<(), String> {
    attach_database_mode(conn, path, schema, false)
}

pub fn attach_database_mode(conn: &Connection, path: &str, schema: &str, read_only: bool) -> Result<(), String> {
    let uri;
    let path = if read_only {
        let absolute = fs::canonicalize(path).map_err(|e| e.to_string())?;
        let mut encoded = String::new();
        for byte in absolute.to_string_lossy().bytes() {
            if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) { encoded.push(byte as char); }
            else { encoded.push_str(&format!("%{byte:02X}")); }
        }
        uri = format!("file:{encoded}?mode=ro");
        &uri
    } else { path };
    let sql = format!(
        "ATTACH DATABASE {} AS {}",
        quote_literal(path),
        quote_ident(schema)
    );
    execute_single_statement(conn, &sql)
}

fn quote_literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Build CREATE TABLE DDL from column defs.
pub fn create_table_ddl(name: &str, cols: &[ColDef]) -> String {
    create_table_ddl_in("main", name, cols)
}

pub fn create_table_ddl_in(schema: &str, name: &str, cols: &[ColDef]) -> String {
    let mut parts = Vec::new();
    let mut pk_cols = Vec::new();
    for c in cols {
        let mut piece = quote_ident(&c.name);
        let ty = c.type_name.trim();
        if !ty.is_empty() {
            piece.push(' ');
            piece.push_str(ty);
        }
        if c.pk {
            pk_cols.push(quote_ident(&c.name));
        }
        if c.notnull {
            piece.push_str(" NOT NULL");
        }
        if c.unique {
            piece.push_str(" UNIQUE");
        }
        if !c.collation.is_empty() {
            piece.push_str(" COLLATE ");
            piece.push_str(&quote_ident(&c.collation));
        }
        if !c.default_value.trim().is_empty() {
            piece.push_str(" DEFAULT (");
            piece.push_str(c.default_value.trim());
            piece.push(')');
        }
        if !c.check_expression.trim().is_empty() {
            piece.push_str(" CHECK (");
            piece.push_str(c.check_expression.trim());
            piece.push_str("\n)");
        }
        if !c.foreign_key_clause.trim().is_empty() {
            piece.push(' ');
            piece.push_str(c.foreign_key_clause.trim());
            piece.push('\n');
        }
        parts.push(piece);
    }
    match pk_cols.len() {
        0 => {}
        1 => {
            let index = cols.iter().position(|c| c.pk).unwrap();
            parts[index].push_str(" PRIMARY KEY");
            if cols[index].auto_increment { parts[index].push_str(" AUTOINCREMENT"); }
        }
        _ => parts.push(format!("PRIMARY KEY ({})", pk_cols.join(", "))),
    }
    format!(
        "CREATE TABLE {} (\n  {}\n)",
        qualified_name(schema, name),
        parts.join(",\n  ")
    )
}

/// Apply native ALTER operations so SQLite preserves constraints and dependencies.
pub fn recreate_table(
    conn: &Connection,
    old_name: &str,
    new_name: &str,
    cols: &[ColDef],
) -> Result<(), String> {
    recreate_table_in(conn, "main", old_name, new_name, cols)
}

pub fn recreate_table_in(
    conn: &Connection,
    schema: &str,
    old_name: &str,
    new_name: &str,
    cols: &[ColDef],
) -> Result<(), String> {
    let existing = table_column_defs_in(conn, schema, old_name)?;
    let sources: Vec<_> = cols.iter().map(|col| existing.iter().find(|old| old.name == col.name).map(|old| old.name.clone())).collect();
    if sources.iter().any(Option::is_none) && existing.iter().any(|old| !sources.iter().any(|source| source.as_ref() == Some(&old.name))) {
        return Err("Use ALTER TABLE RENAME COLUMN in Execute SQL to rename a column".into());
    }
    let mut draft = cols.to_vec();
    for (col, source) in draft.iter_mut().zip(&sources) {
        if let Some(old) = source.as_ref().and_then(|source| existing.iter().find(|old| old.name == *source)) {
            if !old.type_name.eq_ignore_ascii_case(&col.type_name) || old.pk != col.pk || old.notnull != col.notnull
                || old.default_value != col.default_value || col.unique || (col.auto_increment && !old.auto_increment)
                || (!col.collation.is_empty() && !old.collation.eq_ignore_ascii_case(&col.collation))
                || (!col.check_expression.is_empty() && old.check_expression != col.check_expression)
                || (!col.foreign_key_clause.is_empty() && old.foreign_key_clause != col.foreign_key_clause) {
                return Err("Changing existing column definitions requires an explicit SQL migration; the wizard preserves existing constraints".into());
            }
            col.unique = old.unique;
            col.auto_increment = old.auto_increment;
            col.collation = old.collation.clone();
            col.check_expression = old.check_expression.clone();
            col.foreign_key_clause = old.foreign_key_clause.clone();
        }
    }
    recreate_table_columns_in(conn, schema, old_name, new_name, &draft, &sources, None)
}

/// Apply column edits using explicit original identities; None denotes a new column.
pub fn recreate_table_columns_in(
    conn: &Connection,
    schema: &str,
    old_name: &str,
    new_name: &str,
    cols: &[ColDef],
    sources: &[Option<String>],
    options: Option<crate::table_sql::TableOptions>,
) -> Result<(), String> {
    let existing = table_column_defs_in(conn, schema, old_name)?;
    if existing.is_empty() {
        return Err(format!("Table {} has no editable columns", qualified_name(schema, old_name)));
    }
    if cols.is_empty() || sources.len() != cols.len() {
        return Err("Column identities do not match the table draft; reopen Modify Table".into());
    }
    crate::table_sql::validate_primary_key(cols)?;
    let mut rebuild = match options { Some(options) => table_options_in(conn, schema, old_name)? != options, None => false };
    for (index, (col, source)) in cols.iter().zip(sources).enumerate() {
        if cols[..index].iter().any(|old| old.name.eq_ignore_ascii_case(&col.name)) {
            return Err(format!("Duplicate column name: {}", col.name));
        }
        if let Some(source) = source {
            if sources[..index].iter().any(|old| old.as_ref() == Some(source)) {
                return Err(format!("Duplicate original column identity: {source}"));
            }
            let old = existing.iter().find(|c| c.name == *source).ok_or_else(|| format!("Original column no longer exists: {source}"))?;
            crate::table_sql::column_definition(col, Some(old))?;
            rebuild |= old.pk != col.pk || old.auto_increment != col.auto_increment || !old.type_name.eq_ignore_ascii_case(&col.type_name) || old.notnull != col.notnull
                || old.default_value != col.default_value || old.unique != col.unique || !old.collation.eq_ignore_ascii_case(&col.collation)
                || old.check_expression != col.check_expression || old.foreign_key_clause != col.foreign_key_clause;
        } else {
            crate::table_sql::column_definition(col, None)?;
            rebuild |= col.pk || col.unique || !col.default_value.trim().is_empty() || !col.collation.is_empty() || !col.check_expression.trim().is_empty() || !col.foreign_key_clause.trim().is_empty();
        }
    }
    let added: Vec<_> = cols
        .iter()
        .zip(sources)
        .filter_map(|(col, source)| source.is_none().then_some(col))
        .collect();
    let removed: Vec<_> = existing
        .iter()
        .filter(|c| !sources.iter().any(|source| source.as_ref() == Some(&c.name)))
        .collect();
    if !removed.is_empty() {
        let mut stmt = conn.prepare(&format!("PRAGMA {}.index_list({})", quote_ident(schema), quote_literal(old_name))).map_err(|e| e.to_string())?;
        let indexes = stmt.query_map([], |r| r.get::<_, String>(1)).map_err(|e| e.to_string())?;
        for index in indexes {
            let index = index.map_err(|e| e.to_string())?;
            let mut keys = conn.prepare(&format!("PRAGMA {}.index_info({})", quote_ident(schema), quote_literal(&index))).map_err(|e| e.to_string())?;
            let columns = keys.query_map([], |r| r.get::<_, Option<String>>(2)).map_err(|e| e.to_string())?;
            for column in columns {
                let column = column.map_err(|e| e.to_string())?;
                // Quoted attached columns can become string expressions during SQLite DROP reparsing.
                // shortcut: expression-index dependencies need an SQL migration until the wizard parses expressions.
                if column.as_ref().is_none_or(|name| removed.iter().any(|c| c.name.eq_ignore_ascii_case(name))) {
                    return Err(format!("Preserve index {}: remove its dependencies with an explicit SQL migration", qualified_name(schema, &index)));
                }
            }
        }
    }
    let retained: Vec<_> = existing
        .iter()
        .filter(|c| sources.iter().any(|source| source.as_ref() == Some(&c.name)))
        .map(|c| &c.name)
        .collect();
    let requested: Vec<_> = sources.iter().filter_map(Option::as_ref).collect();
    rebuild |= retained != requested || sources.iter().skip_while(|source| source.is_some()).any(Option::is_some);
    let mut reserved = table_column_names_in(conn, schema, old_name)?;
    reserved.extend(cols.iter().map(|col| col.name.clone()));
    let mut renames = Vec::new();
    for (col, source) in cols.iter().zip(sources) {
        if let Some(source) = source.as_ref().filter(|source| **source != col.name) {
            let mut number = reserved.len();
            let temporary = loop {
                let candidate = format!("__next_gui_column_{number}");
                if !reserved.iter().any(|name| name.eq_ignore_ascii_case(&candidate)) { break candidate; }
                number += 1;
            };
            reserved.push(temporary.clone());
            renames.push((source, &col.name, temporary));
        }
    }
    with_savepoint(conn, || {
        // Temporary column names let swaps and case-only renames preserve identity.
        for (source, _, temporary) in &renames {
            execute_single_statement(conn, &format!("ALTER TABLE {} RENAME COLUMN {} TO {}",
                qualified_name(schema, old_name), quote_ident(source), quote_ident(temporary)))
                .map_err(|error| format!("Cannot rename column {source}: {error}"))?;
        }
        // shortcut: clear/save PK membership before removing its column; native DROP validates other dependencies.
        for col in removed {
            execute_single_statement(conn, &format!(
                "ALTER TABLE {} DROP COLUMN {}",
                qualified_name(schema, old_name),
                quote_ident(&col.name)
            ))?;
        }
        for (_, target, temporary) in &renames {
            execute_single_statement(conn, &format!("ALTER TABLE {} RENAME COLUMN {} TO {}",
                qualified_name(schema, old_name), quote_ident(temporary), quote_ident(target)))
                .map_err(|error| format!("Cannot rename column to {target}: {error}"))?;
        }
        if rebuild {
            crate::table_migration::rebuild(conn, schema, old_name, &existing, cols, sources, options)?;
        } else {
            for col in added {
                let ddl = create_table_ddl("unused", std::slice::from_ref(col));
                let definition = ddl.split_once("(\n  ").unwrap().1.trim_end_matches("\n)");
                execute_single_statement(conn, &format!(
                    "ALTER TABLE {} ADD COLUMN {definition}", qualified_name(schema, old_name)
                ))?;
            }
        }
        if old_name != new_name {
            let setting = rusqlite::config::DbConfig::SQLITE_DBCONFIG_LEGACY_ALTER_TABLE;
            let legacy = conn.db_config(setting).map_err(|e| e.to_string())?;
            if legacy { conn.set_db_config(setting, false).map_err(|e| e.to_string())?; }
            let renamed = execute_single_statement(conn, &format!(
                "ALTER TABLE {} RENAME TO {}",
                qualified_name(schema, old_name),
                quote_ident(new_name)
            ));
            if legacy {
                conn.set_db_config(setting, true).map_err(|e| format!("{}Restoring legacy ALTER setting failed: {e}",
                    renamed.as_ref().err().map(|error| format!("{error}; ")).unwrap_or_default()))?;
            }
            renamed?;
        }
        Ok(())
    })
}

pub fn with_savepoint<T>(
    conn: &Connection,
    action: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    conn.execute_batch("SAVEPOINT e28_operation")
        .map_err(|e| e.to_string())?;
    match action().and_then(|value| {
        conn.execute_batch("RELEASE e28_operation")
            .map_err(|e| e.to_string())?;
        Ok(value)
    }) {
        Ok(value) => Ok(value),
        Err(error) => {
            conn.execute_batch("ROLLBACK TO e28_operation; RELEASE e28_operation")
                .map_err(|e| format!("{error}; rollback failed: {e}"))?;
            Err(error)
        }
    }
}

pub fn import_csv_into_table(
    conn: &Connection,
    table: &str,
    columns: &[String],
    rows: &[Vec<String>],
    create_if_missing: bool,
) -> Result<u64, String> {
    with_savepoint(conn, || {
        if create_if_missing {
            let exists: bool = conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            if !exists {
                let ddl = columns
                    .iter()
                    .map(|c| format!("{} TEXT", quote_ident(c)))
                    .collect::<Vec<_>>()
                    .join(", ");
                create_table_simple(conn, table, &ddl)?;
            }
        }
        let mut n = 0u64;
        for row in rows {
            if row.len() != columns.len() {
                return Err("CSV row has the wrong number of fields".into());
            }
            let mut vals = Vec::with_capacity(columns.len());
            for i in 0..columns.len() {
                vals.push(row.get(i).cloned().unwrap_or_default());
            }
            n += insert_row(conn, table, columns, &vals)?;
        }
        Ok(n)
    })
}

/// SQL dump (schema + data) similar to `.dump`.
pub fn export_sql_dump(conn: &Connection, path: &Path) -> Result<(), String> {
    export_sql_dump_opts(conn, path, &ExportSqlOptions::default())
}

pub fn import_sql_file(conn: &Connection, path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    with_savepoint(conn, || {
        conn.execute_batch("PRAGMA defer_foreign_keys=ON")
            .map_err(|e| e.to_string())?;
        let mut batch = rusqlite::Batch::new(conn, &text);
        loop {
            let control = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let flag = control.clone();
            conn.authorizer(Some(move |ctx: rusqlite::hooks::AuthContext<'_>| {
                if matches!(
                    ctx.action,
                    rusqlite::hooks::AuthAction::Transaction { .. }
                        | rusqlite::hooks::AuthAction::Savepoint { .. }
                        | rusqlite::hooks::AuthAction::Pragma {
                            pragma_name: "foreign_keys",
                            ..
                        }
                ) {
                    flag.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                rusqlite::hooks::Authorization::Allow
            }));
            let next = batch.next();
            conn.authorizer(
                None::<fn(rusqlite::hooks::AuthContext<'_>) -> rusqlite::hooks::Authorization>,
            );
            let Some(mut stmt) = next.map_err(|e| e.to_string())? else {
                break;
            };
            // Dump transaction wrappers must not commit the user's other pending edits.
            if control.load(std::sync::atomic::Ordering::Relaxed) {
                continue;
            }
            let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
            while rows.next().map_err(|e| e.to_string())?.is_some() {}
        }
        Ok(())
    })
}

/// Finite numeric samples from the first N source rows for charts.
pub fn numeric_series(
    conn: &Connection,
    table: &str,
    column: &str,
    limit: usize,
) -> Result<Vec<f32>, String> {
    numeric_series_in(conn, "main", table, column, limit)
}

pub fn numeric_series_in(
    conn: &Connection,
    schema: &str,
    table: &str,
    column: &str,
    limit: usize,
) -> Result<Vec<f32>, String> {
    if !table_column_names_in(conn, schema, table)?.iter().any(|name| name.eq_ignore_ascii_case(column)) {
        return Err(format!("Unknown plot column {column}"));
    }
    let sql = format!(
        "SELECT {} FROM {} LIMIT {limit}",
        quote_ident(column),
        qualified_name(schema, table),
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, rusqlite::types::Value>(0))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        let v = match r.map_err(|e| e.to_string())? {
            rusqlite::types::Value::Integer(v) => v as f64,
            rusqlite::types::Value::Real(v) => v,
            rusqlite::types::Value::Text(v) => match v.trim().parse::<f64>() {
                Ok(v) => v,
                Err(_) => continue,
            },
            _ => continue,
        } as f32;
        if v.is_finite() {
            out.push(v);
        }
    }
    Ok(out)
}

pub fn default_demo_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sample.db")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn generated_schema_statements_keep_literal_semicolons_and_reject_tails() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        assert!(create_table_simple(&conn, "rejected", "v); PRAGMA foreign_keys=ON; --").is_err());
        assert_eq!(read_pragma(&conn, "foreign_keys").unwrap(), "0");
        assert!(table_columns(&conn, "rejected").unwrap().is_empty());
        create_table_simple(&conn, "quoted;\"表", "amount DECIMAL(10,2), marker TEXT DEFAULT ('中文; literal')").unwrap();
        create_index(&conn, "index;\"名", "quoted;\"表", "length(marker || ';') DESC", false).unwrap();
        conn.execute(&format!("INSERT INTO {} DEFAULT VALUES", quote_ident("quoted;\"表")), []).unwrap();
        let result = browse_table(&conn, "quoted;\"表", &[], None, 10, 0).unwrap();
        assert_eq!(result.rows[0][1], "中文; literal");
        assert_eq!(table_columns(&conn, "quoted;\"表").unwrap()[0].type_name, "DECIMAL(10,2)");
    }

    #[test]
    fn data_helpers_target_quoted_attached_and_temp_namespaces() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("ATTACH ':memory:' AS \"aux.\"\"db\";").unwrap();
        for schema in ["main", "temp", "aux.\"db"] {
            let table = qualified_name(schema, "same.\"table");
            conn.execute_batch(&format!("CREATE TABLE {table}(id INTEGER PRIMARY KEY, v TEXT);
                INSERT INTO {table} VALUES(1, 'original'), (2, 'second');")).unwrap();
        }
        let order = vec![OrderBy { column: "id".into(), descending: true }];
        let dir = tempfile::tempdir().unwrap();
        for schema in ["temp", "aux.\"db"] {
            let table = "same.\"table";
            assert_eq!(table_columns_in(&conn, schema, table).unwrap().len(), 2);
            assert_eq!(table_column_names_in(&conn, schema, table).unwrap(), ["id", "v"]);
            assert_eq!(count_browse_rows_in(&conn, schema, table, Some("id=1")).unwrap(), 1);
            let rows = browse_table_in(&conn, schema, table, &order, Some("id>0"), 1, 0).unwrap();
            assert_eq!(rows.rows[0][0], "2");
            assert!(rows.truncated);
            let rows = browse_with_rowid_in(&conn, schema, table, &[], None, 500, 0).unwrap();
            assert_eq!(rows.columns[0], "__rowid__");
            update_cell_in(&conn, schema, table, 1, "v", &rusqlite::types::Value::Text(schema.into())).unwrap();
            let values = [(1, vec![CellValue::Text(b"atomic".to_vec())]), (999, vec![CellValue::Null])];
            assert!(update_cells_in(&conn, schema, table, &["v".into()], &values).is_err());
            let out = dir.path().join(if schema == "temp" { "temp.json" } else { "attached.json" });
            export_table_in(&conn, schema, table, &[], Some("id=1"), &out, true).unwrap();
            let json: serde_json::Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
            assert_eq!(json[0]["v"], schema);
            assert_eq!(delete_row_by_rowid_in(&conn, schema, table, 2).unwrap(), 1);
            assert_eq!(count_browse_rows_in(&conn, schema, table, None).unwrap(), 1);
        }
        assert_eq!(count_rows(&conn, "same.\"table").unwrap(), 2);
        assert_eq!(browse_table(&conn, "same.\"table", &[], None, 500, 0).unwrap().rows[0][1], "original");
        assert!(browse_table_in(&conn, "missing\"; DROP TABLE main.t", "same.\"table", &[], None, 500, 0).is_err());
    }

    #[test]
    fn mapped_column_renames_preserve_qualified_dependencies_and_rollback_every_step() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; ATTACH ':memory:' AS \"aux.\"\"db\"; BEGIN;").unwrap();
        for schema in ["main", "temp", "aux.\"db"] {
            let parent = qualified_name(schema, "parent");
            conn.execute_batch(&format!("CREATE TABLE {parent}(id INTEGER PRIMARY KEY AUTOINCREMENT,
                a TEXT NOT NULL DEFAULT 'fallback' CHECK(a != 'bad'), b TEXT DEFAULT 'B',
                g TEXT GENERATED ALWAYS AS(a || '!') STORED);
                CREATE TABLE {}(p REFERENCES parent(id) ON DELETE CASCADE);
                CREATE TABLE {}(a,b);
                CREATE UNIQUE INDEX {} ON parent(lower(a)) WHERE b != 'excluded';
                CREATE VIEW {} AS SELECT id,a,b,g FROM parent;
                CREATE TRIGGER {} AFTER INSERT ON {parent} BEGIN INSERT INTO child VALUES(new.id); INSERT INTO audit VALUES(new.a,new.b); END;
                INSERT INTO {parent}(id,a,b) VALUES(1,'A','B'),(100,'removed','gone');
                DELETE FROM {parent} WHERE id=100;",
                qualified_name(schema, "child"), qualified_name(schema, "audit"), qualified_name(schema, "ix"),
                qualified_name(schema, "viewed"), qualified_name(schema, "tr"))).unwrap();
            if schema == "temp" {
                // SQLite 3.45 reparses temp triggers/views twice during RENAME COLUMN.
                let before: Vec<_> = list_schema(&conn).unwrap().into_iter().map(|o| (o.schema, o.name, o.sql)).collect();
                let mut cols = table_column_defs_in(&conn, schema, "parent").unwrap();
                let sources: Vec<_> = cols.iter().map(|col| Some(col.name.clone())).collect();
                cols[0].name = "ID".into();
                assert!(recreate_table_columns_in(&conn, schema, "parent", "parent", &cols, &sources, None).is_err());
                let after: Vec<_> = list_schema(&conn).unwrap().into_iter().map(|o| (o.schema, o.name, o.sql)).collect();
                assert_eq!(after, before);
                conn.execute_batch("DROP TRIGGER temp.tr; DROP VIEW temp.viewed;").unwrap();
            }
            let mut cols = table_column_defs_in(&conn, schema, "parent").unwrap();
            let mut sources: Vec<_> = cols.iter().map(|col| Some(col.name.clone())).collect();
            cols[0].name = "ID".into();
            cols[1].name = "b".into();
            cols[2].name = "a".into();
            let original_schema: Vec<_> = list_schema(&conn).unwrap().into_iter().map(|o| (o.schema, o.name, o.sql)).collect();
            let mut bad = cols.clone();
            bad.push(ColDef { default_value: "0; PRAGMA foreign_keys=OFF; --".into(), ..ColDef::new("tail", "TEXT") });
            let mut bad_sources = sources.clone();
            bad_sources.push(None);
            assert!(recreate_table_columns_in(&conn, schema, "parent", "changed", &bad, &bad_sources, None).is_err());
            let after: Vec<_> = list_schema(&conn).unwrap().into_iter().map(|o| (o.schema, o.name, o.sql)).collect();
            assert_eq!(after, original_schema);
            assert_eq!(count_browse_rows_in(&conn, schema, "child", None).unwrap(), 1);
            assert_eq!(conn.query_row("PRAGMA foreign_keys", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
            assert!(!conn.is_autocommit());
            assert!(recreate_table_columns_in(&conn, schema, "parent", "parent", &cols, &sources[..2], None).is_err());
            let mut invalid = sources.clone();
            invalid[1] = invalid[0].clone();
            assert!(recreate_table_columns_in(&conn, schema, "parent", "parent", &cols, &invalid, None).is_err());
            invalid[1] = Some("missing".into());
            assert!(recreate_table_columns_in(&conn, schema, "parent", "parent", &cols, &invalid, None).is_err());
            let mut duplicate = cols.clone();
            duplicate[2].name = "B".into();
            assert!(recreate_table_columns_in(&conn, schema, "parent", "parent", &duplicate, &sources, None).is_err());
            cols.push(ColDef::new("untyped", ""));
            sources.push(None);
            recreate_table_columns_in(&conn, schema, "parent", "changed.\"parent", &cols, &sources, None).unwrap_or_else(|error| panic!("{schema}: {error}"));
            let changed = qualified_name(schema, "changed.\"parent");
            let row: (i64, String, String, String, Option<String>) = conn.query_row(&format!("SELECT \"ID\",a,b,g,untyped FROM {changed}"), [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).unwrap();
            assert_eq!(row, (1,"B".into(),"A".into(),"A!".into(),None));
            if schema != "temp" { assert_eq!(count_browse_rows_in(&conn, schema, "viewed", None).unwrap(), 1); }
            assert_eq!(conn.query_row(&format!("SELECT \"table\" FROM pragma_foreign_key_list('child', {})", quote_literal(schema)), [], |r| r.get::<_, String>(0)).unwrap(), "changed.\"parent");
            assert!(conn.execute(&format!("INSERT INTO {changed}(a,b) VALUES('new','a')"), []).is_err());
            assert!(conn.execute(&format!("INSERT INTO {changed}(a,b) VALUES('new','bad')"), []).is_err());
            conn.execute(&format!("INSERT INTO {changed}(a,b) VALUES('new B','new A')"), []).unwrap();
            assert_eq!(conn.last_insert_rowid(), 101);
            assert_eq!(count_browse_rows_in(&conn, schema, "child", None).unwrap(), if schema == "temp" { 1 } else { 2 });
            let audit: (String,String) = conn.query_row(&format!("SELECT a,b FROM {} ORDER BY rowid DESC LIMIT 1", qualified_name(schema, "audit")), [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
            assert_eq!(audit, if schema == "temp" { ("removed".into(),"gone".into()) } else { ("new A".into(),"new B".into()) });
            assert_eq!(conn.query_row(&format!("SELECT count(*) FROM pragma_foreign_key_check(NULL,{})", quote_literal(schema)), [], |r| r.get::<_, i64>(0)).unwrap(), 0);
        }
        conn.execute_batch("ROLLBACK").unwrap();
        assert!(list_schema(&conn).unwrap().iter().all(|object| object.name != "changed.\"parent"));
    }

    #[test]
    fn mapped_column_rename_rolls_back_earlier_steps_when_a_view_becomes_ambiguous() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE p(id,a); CREATE TABLE q(a);
            CREATE VIEW joined AS SELECT * FROM p JOIN q USING(a); INSERT INTO p VALUES(1,'kept');
            INSERT INTO q VALUES('kept'); BEGIN; INSERT INTO p VALUES(2,'pending');").unwrap();
        let before: Vec<_> = list_schema(&conn).unwrap().into_iter().map(|o| (o.schema, o.name, o.sql)).collect();
        let cols = [ColDef::new("ID", ""), ColDef::new("changed", "")];
        let sources = [Some("id".into()), Some("a".into())];
        assert!(recreate_table_columns_in(&conn, "main", "p", "p", &cols, &sources, None).is_err());
        let after: Vec<_> = list_schema(&conn).unwrap().into_iter().map(|o| (o.schema, o.name, o.sql)).collect();
        assert_eq!(after, before);
        assert_eq!(count_rows(&conn, "p").unwrap(), 2);
        assert_eq!(count_rows(&conn, "joined").unwrap(), 1);
        assert_eq!(conn.query_row("PRAGMA foreign_keys", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
        assert!(!conn.is_autocommit());
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(count_rows(&conn, "p").unwrap(), 1);
    }

    #[test]
    fn table_wizard_creates_and_adds_columns_without_inventing_types() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("ATTACH ':memory:' AS \"aux.\"\"db\";").unwrap();
        for schema in ["main", "temp", "aux.\"db"] {
            let mut cols = vec![ColDef::new("untyped.\"col", ""), ColDef::new("amount", "DECIMAL(10,2)"), ColDef::new("label", "VARYING CHARACTER(48)")];
            execute_single_statement(&conn, &create_table_ddl_in(schema, "custom", &cols)).unwrap();
            let types: Vec<_> = table_columns_in(&conn, schema, "custom").unwrap().into_iter().map(|col| col.type_name).collect();
            assert_eq!(types, ["", "DECIMAL(10,2)", "VARYING CHARACTER(48)"]);
            conn.execute_batch(&format!("INSERT INTO {} VALUES(7, 1.5, 'original');", qualified_name(schema, "custom"))).unwrap();
            cols.push(ColDef::new("new_untyped", "   "));
            recreate_table_in(&conn, schema, "custom", "custom", &cols).unwrap();
            assert_eq!(table_columns_in(&conn, schema, "custom").unwrap().last().unwrap().type_name, "");
            let stored: (String, i64, f64, String) = conn.query_row(&format!("SELECT typeof({}), {}, amount, label FROM {}", quote_ident("untyped.\"col"), quote_ident("untyped.\"col"), qualified_name(schema, "custom")), [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).unwrap();
            assert_eq!(stored, ("integer".into(), 7, 1.5, "original".into()));
        }
    }

    #[test]
    fn schema_wizard_helpers_preserve_quoted_namespace_and_atomic_alter() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; ATTACH ':memory:' AS \"aux.\"\"db\";").unwrap();
        let table = "same.\"table";
        let cols = [ColDef { name: "id".into(), type_name: "INTEGER".into(), pk: true, auto_increment: false,
            collation: String::new(),
            check_expression: String::new(), foreign_key_clause: String::new(),
            notnull: false, unique: false, default_value: String::new() }, ColDef::new("value.\"col", "TEXT")];
        for schema in ["main", "temp", "aux.\"db"] {
            conn.execute_batch(&create_table_ddl_in(schema, table, &cols)).unwrap();
            conn.execute_batch(&format!("INSERT INTO {} VALUES(1,'original');
                CREATE TABLE {}(p REFERENCES {}(id));
                CREATE TRIGGER {} AFTER INSERT ON {} BEGIN INSERT INTO child VALUES(new.id); END;",
                qualified_name(schema, table), qualified_name(schema, "child"), quote_ident(table),
                qualified_name(schema, "tr"), qualified_name(schema, table))).unwrap();
            create_index_in(&conn, schema, "same.\"idx", table, &quote_ident("value.\"col"), true, "").unwrap();
        }
        conn.execute_batch("BEGIN").unwrap();
        for schema in ["main", "temp", "aux.\"db"] {
            let mut modified = cols.to_vec();
            modified.push(ColDef::new("added.\"col", "TEXT"));
            recreate_table_in(&conn, schema, table, "renamed.\"table", &modified).unwrap();
            let renamed = qualified_name(schema, "renamed.\"table");
            conn.execute_batch(&format!("INSERT INTO {renamed}(id,{}) VALUES(2,'second')", quote_ident("value.\"col"))).unwrap();
            assert_eq!(count_browse_rows_in(&conn, schema, "child", None).unwrap(), 1);
            assert!(conn.execute_batch(&format!("INSERT INTO {renamed}(id,{}) VALUES(3,'original')", quote_ident("value.\"col"))).is_err());
            modified.push(ColDef::new("rollback_col", "TEXT"));
            // Adding succeeds, then renaming onto the existing child fails; both must roll back.
            assert!(recreate_table_in(&conn, schema, "renamed.\"table", "child", &modified).is_err());
            assert!(!table_column_names_in(&conn, schema, "renamed.\"table").unwrap().contains(&"rollback_col".into()));
            let mut removed = modified[..3].to_vec();
            removed.remove(1);
            assert!(recreate_table_in(&conn, schema, "renamed.\"table", "renamed.\"table", &removed).is_err());
            assert_eq!(table_column_names_in(&conn, schema, "renamed.\"table").unwrap().len(), 3);
            // An unrelated plain column can still be dropped without weakening the index.
            recreate_table_in(&conn, schema, "renamed.\"table", "renamed.\"table", &cols).unwrap();
            assert_eq!(count_browse_rows_in(&conn, schema, "renamed.\"table", None).unwrap(), 2);
        }
        assert!(recreate_table_in(&conn, "missing", "renamed.\"table", "x", &cols).is_err());
        assert!(recreate_table_in(&conn, "main", "missing", "x", &[]).is_err());
        assert!(create_index_in(&conn, "missing\"; DROP TABLE main.child; --", "ix", "child", "p", false, "").is_err());
        assert_eq!(count_rows(&conn, "child").unwrap(), 1);
        conn.execute_batch("ROLLBACK").unwrap();
        drop_object(&conn, ObjectKind::Index, "aux.\"db", "same.\"idx").unwrap();
        create_index_in(&conn, "aux.\"db", "expression_idx", table, &format!("length({})", quote_ident("value.\"col")), false, "").unwrap();
        let error = recreate_table_in(&conn, "aux.\"db", table, table, &cols[..1]).unwrap_err();
        assert!(error.contains("expression_idx"));
        assert_eq!(table_column_names_in(&conn, "aux.\"db", table).unwrap().len(), 2);
        // The main wrappers remain explicit even when temp contains the same names.
        create_index(&conn, "wrapper_idx", table, "id", false).unwrap();
        assert!(list_schema(&conn).unwrap().iter().any(|o| o.schema == "main" && o.name == "wrapper_idx"));
        assert!(!list_schema(&conn).unwrap().iter().any(|o| o.schema == "temp" && o.name == "wrapper_idx"));
        conn.execute_batch(&create_table_ddl("wrapper_table", &cols)).unwrap();
        assert_eq!(table_columns(&conn, "wrapper_table").unwrap().len(), 2);
    }

    #[test]
    fn foreign_key_metadata_resolves_the_parent_in_the_same_database() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("ATTACH ':memory:' AS \"aux.\"\"db\";
            CREATE TABLE main.parent(main_id INTEGER PRIMARY KEY);
            CREATE TABLE main.child(main_ref REFERENCES parent);").unwrap();
        for schema in ["temp", "aux.\"db"] {
            conn.execute_batch(&format!("CREATE TABLE {}(other_id INTEGER PRIMARY KEY);
                CREATE TABLE {}(other_ref REFERENCES parent);", qualified_name(schema, "parent"), qualified_name(schema, "child"))).unwrap();
            let keys = list_foreign_keys_in(&conn, schema, "child").unwrap();
            assert_eq!(keys[0].from_col, "other_ref");
            assert_eq!(keys[0].to_col, "other_id");
        }
        assert_eq!(list_foreign_keys(&conn, "child").unwrap()[0].to_col, "main_id");
    }

    #[test]
    fn main_browse_helpers_ignore_same_named_temp_tables() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(v); INSERT INTO main.t VALUES(1);
            CREATE TEMP TABLE t(v, temp_only); INSERT INTO temp.t(v) VALUES(99);").unwrap();
        let result = browse_table(&conn, "t", &[], None, 500, 0).unwrap();
        assert_eq!(result.rows[0][0], "1");
        update_cell(&conn, "t", 1, "v", &rusqlite::types::Value::Integer(2)).unwrap();
        assert_eq!(conn.query_row("SELECT v FROM main.t", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
        assert_eq!(conn.query_row("SELECT v FROM temp.t", [], |r| r.get::<_, i64>(0)).unwrap(), 99);
        assert_eq!(table_columns(&conn, "t").unwrap().len(), 1);
        insert_row(&conn, "t", &["v".into()], &["3".into()]).unwrap();
        assert_eq!(count_rows(&conn, "t").unwrap(), 2);
        delete_row_by_rowid(&conn, "t", 2).unwrap();
        recreate_table(&conn, "t", "t", &[ColDef::new("v", ""), ColDef::new("added", "TEXT")]).unwrap();
        assert_eq!(table_column_names(&conn, "t").unwrap(), ["v", "added"]);
        assert!(conn.prepare("SELECT temp_only FROM temp.t").is_ok());
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("main.json");
        export_table(&conn, "t", &[], None, &out, true).unwrap();
        let values: serde_json::Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(values[0]["v"], 2);
    }

    #[test]
    fn schema_reads_and_drops_keep_namespace_identity() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("ATTACH ':memory:' AS \"aux.\"\"db\";").unwrap();
        for schema in ["main", "temp", "aux.\"db"] {
            let table = qualified_name(schema, "same.\"name");
            let view = qualified_name(schema, "same_view");
            let index = qualified_name(schema, "same_index");
            let trigger = qualified_name(schema, "same_trigger");
            conn.execute_batch(&format!("CREATE TABLE {table}(v);
                CREATE VIEW {view} AS SELECT * FROM {};
                CREATE INDEX {index} ON {}(v);
                CREATE TRIGGER {trigger} AFTER INSERT ON {} BEGIN SELECT 1; END;",
                quote_ident("same.\"name"), quote_ident("same.\"name"), quote_ident("same.\"name"))).unwrap();
            assert_eq!(list_schema_in(&conn, schema).unwrap().len(), 4);
        }
        assert!(list_schema(&conn).unwrap().iter().all(|o| o.schema == "main"));
        for (kind, name) in [(ObjectKind::Trigger, "same_trigger"), (ObjectKind::Index, "same_index"), (ObjectKind::View, "same_view"), (ObjectKind::Table, "same.\"name")] {
            drop_object(&conn, kind, "aux.\"db", name).unwrap();
            assert_eq!(list_schema_in(&conn, "main").unwrap().len(), 4);
            assert_eq!(list_schema_in(&conn, "temp").unwrap().len(), 4);
            assert!(drop_object(&conn, kind, "aux.\"db", name).is_err());
        }
        assert!(list_schema_in(&conn, "aux.\"db").unwrap().is_empty());
        assert!(list_schema_in(&conn, "missing\"; DROP TABLE main.\"same.\"\"name").is_err());
        drop_object(&conn, ObjectKind::Table, "main", "same.\"name").unwrap();
        assert_eq!(list_schema_in(&conn, "temp").unwrap().len(), 4);
    }

    fn tmp_db() -> (PathBuf, crate::sql_log::LoggedConnection) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("e28-test-{nanos}.db"));
        let _ = fs::remove_file(&path);
        let conn = create_new(path.to_str().unwrap()).unwrap();
        (path, conn)
    }

    #[test]
    fn sql_import_is_atomic_and_does_not_commit_pending_edits() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE existing(v); BEGIN; INSERT INTO existing VALUES(1);")
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("import.sql");
        fs::write(&path, "-- dump\nPRAGMA foreign_keys=OFF; BEGIN; CREATE TABLE imported(v); INSERT INTO imported VALUES('a;b'); COMMIT;").unwrap();
        import_sql_file(&conn, &path).unwrap();
        assert_eq!(count_rows(&conn, "imported").unwrap(), 1);
        rollback_and_begin(&conn).unwrap();
        assert_eq!(count_rows(&conn, "existing").unwrap(), 0);
        assert!(count_rows(&conn, "imported").is_err());
        fs::write(
            &path,
            "CREATE TABLE broken(v); INSERT INTO missing VALUES(1);",
        )
        .unwrap();
        assert!(import_sql_file(&conn, &path).is_err());
        assert!(count_rows(&conn, "broken").is_err());
    }

    #[test]
    fn csv_multiline_and_atomic_import() {
        let options = CsvImportOptions::default();
        let (cols, rows) = import_csv_from_text(
            "name,note\r\n中文,\"line 1\nline 2\"\r\nempty,\"\"\r\n",
            &options,
        )
        .unwrap();
        assert_eq!(rows[0][1], "line 1\nline 2");
        assert_eq!(rows[1][1], "");
        assert!(import_csv_from_text("a,b\n\"unterminated", &options).is_err());
        assert!(import_csv_from_text("a,b\n1,2,3", &options).is_err());
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(name TEXT UNIQUE, note TEXT); BEGIN;")
            .unwrap();
        let mut duplicates = rows.clone();
        duplicates.push(rows[0].clone());
        assert!(import_csv_into_table(&conn, "t", &cols, &duplicates, false).is_err());
        assert_eq!(count_rows(&conn, "t").unwrap(), 0);
        import_csv_into_table(&conn, "t", &cols, &rows, false).unwrap();
        assert_eq!(
            conn.query_row("SELECT typeof(note) FROM t WHERE name='empty'", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            "text"
        );
    }

    #[test]
    fn table_alter_keeps_constraints_triggers_and_foreign_keys() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE parent(id INTEGER PRIMARY KEY, v TEXT UNIQUE CHECK(length(v)>0)); CREATE TABLE child(p REFERENCES parent(id)); CREATE INDEX ix ON parent(v); CREATE TRIGGER tr AFTER INSERT ON parent BEGIN INSERT INTO child VALUES(new.id); END; INSERT INTO parent VALUES(1,'one'); BEGIN;").unwrap();
        let mut cols: Vec<_> = table_columns(&conn, "parent")
            .unwrap()
            .into_iter()
            .map(|c| ColDef {
                name: c.name,
                type_name: c.type_name,
                pk: c.pk,
                auto_increment: false,
                collation: String::new(),
                check_expression: String::new(), foreign_key_clause: String::new(),
                notnull: c.notnull,
                unique: false,
                default_value: c.default_value.unwrap_or_default(),
            })
            .collect();
        cols.push(ColDef::new("extra", "TEXT"));
        recreate_table(&conn, "parent", "renamed", &cols).unwrap();
        conn.execute("INSERT INTO renamed(id,v) VALUES(2,'two')", [])
            .unwrap();
        assert_eq!(count_rows(&conn, "child").unwrap(), 2);
        assert!(conn
            .execute("INSERT INTO renamed(id,v) VALUES(3,'one')", [])
            .is_err());
        assert!(conn
            .execute("INSERT INTO renamed(id,v) VALUES(3,'')", [])
            .is_err());
        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='child'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(sql.contains("renamed"));
        cols[1].type_name = "BLOB".into();
        assert!(recreate_table(&conn, "renamed", "oops", &cols).is_err());
        assert_eq!(count_rows(&conn, "renamed").unwrap(), 2);
    }

    #[test]
    fn complete_table_exports_keep_more_than_preview_limit() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(v); WITH RECURSIVE c(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM c WHERE x<601) INSERT INTO t SELECT x FROM c;").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rows.json");
        export_table(&conn, "t", &[], None, &path, true).unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(rows.len(), 601);
        assert_eq!(rows[600]["v"], 601);
        export_table(&conn, "t", &[], Some("v>500"), &path, false).unwrap();
        assert_eq!(fs::read_to_string(path).unwrap().lines().count(), 102);
    }

    #[test]
    fn sql_result_export_keeps_full_typed_snapshot_without_reexecuting() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(v); WITH RECURSIVE c(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM c WHERE x<601) INSERT INTO t SELECT x FROM c;").unwrap();
        let result = execute_sql(
            &conn,
            "UPDATE t SET v=v+1 RETURNING v, NULL AS n, '' AS empty, X'00FF' AS bytes",
        )
        .unwrap();
        assert!(result.truncated);
        assert_eq!(result.rows.len(), 500);
        conn.execute("DELETE FROM t", []).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("result.json");
        export_query_result(&conn, &result, &path, true).unwrap();
        let rows: Vec<serde_json::Value> =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(rows.len(), 601);
        assert_eq!(rows[600]["v"], 602);
        assert!(rows[0]["n"].is_null());
        assert_eq!(rows[0]["empty"], "");
        assert_eq!(rows[0]["bytes"], "X'00FF'");
        export_query_result(&conn, &result, &path, false).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 602);
        assert_eq!(count_rows(&conn, "t").unwrap(), 0);
        let result = execute_sql(&conn, "SELECT 1 AS v, 2 AS v").unwrap();
        assert!(export_query_result(&conn, &result, &path, true).is_err());
        assert!(execute_sql(&conn, "SELECT 9e999").is_ok());
    }

    #[test]
    fn encrypted_export_preserves_source_and_custom_settings() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(v); INSERT INTO t VALUES(42); PRAGMA user_version=7;")
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("encrypted.db");
        encrypt_database_to(&conn, &path, "secret", 8192, 1000).unwrap();
        assert!(!looks_like_sqlite(&path).unwrap());
        let restored = Connection::open(&path).unwrap();
        apply_cipher_key(&restored, "secret", 8192, 1000).unwrap();
        assert_eq!(read_pragma(&restored, "user_version").unwrap(), "7");
        assert_eq!(count_rows(&restored, "t").unwrap(), 1);
        assert!(encrypt_database_to(&conn, &path, "secret", 8192, 1000).is_err());
        assert_eq!(count_rows(&conn, "t").unwrap(), 1);
    }

    #[test]
    fn save_as_copies_wal_and_memory_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("wal.db");
        let conn = Connection::open(&src).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE t(v); PRAGMA wal_checkpoint(TRUNCATE); INSERT INTO t VALUES (42);").unwrap();
        let dst = dir.path().join("copy.db");
        copy_database_file(&conn, &dst).unwrap();
        assert_eq!(
            Connection::open(&dst)
                .unwrap()
                .query_row("SELECT v FROM t", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            42
        );
        assert!(copy_database_file(&conn, &src).is_err());
        assert!(create_new(src.to_str().unwrap()).is_err());
        let mem = Connection::open_in_memory().unwrap();
        mem.execute_batch("CREATE TABLE t(v); INSERT INTO t VALUES ('memory');")
            .unwrap();
        let dst = dir.path().join("memory.db");
        copy_database_file(&mem, &dst).unwrap();
        assert_eq!(
            Connection::open(dst)
                .unwrap()
                .query_row("SELECT v FROM t", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "memory"
        );
    }

    #[test]
    fn exports_reject_open_database_and_journal_destinations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE t(v); PRAGMA journal_mode=WAL;")
            .unwrap();
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let dest = PathBuf::from(format!("{}{suffix}", path.display()));
            assert!(guard_output_path(&conn, &dest).is_err(), "{suffix}");
        }
        assert!(guard_output_path(&conn, &dir.path().join("export.sql")).is_ok());
    }

    #[test]
    fn generated_rowid_is_not_used_as_an_edit_or_dump_identity() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(v, rowid GENERATED ALWAYS AS (1)); INSERT INTO t(_rowid_,v) VALUES(7,'first'),(9,'second');").unwrap();
        assert!(browse_with_rowid(&conn, "t", &[], None, 500, 0).is_err());
        let dir = tempfile::tempdir().unwrap();
        let dump = dir.path().join("dump.sql");
        export_sql_dump(&conn, &dump).unwrap();
        let restored = Connection::open_in_memory().unwrap();
        restored
            .execute_batch(&fs::read_to_string(dump).unwrap())
            .unwrap();
        assert_eq!(
            restored
                .query_row("SELECT v FROM t WHERE _rowid_=9", [], |r| r
                    .get::<_, String>(0))
                .unwrap(),
            "second"
        );
    }

    #[test]
    fn browse_filter_cannot_execute_a_second_statement() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(v); INSERT INTO t VALUES(1)")
            .unwrap();
        let filter = Some("1; DELETE FROM t; --");
        assert!(browse_table(&conn, "t", &[], filter, 500, 0).is_err());
        assert!(browse_with_rowid(&conn, "t", &[], filter, 500, 0).is_err());
        assert!(count_browse_rows(&conn, "t", filter).is_err());
        assert_eq!(count_browse_rows(&conn, "t", Some("v = 1")).unwrap(), 1);
        assert_eq!(count_rows(&conn, "t").unwrap(), 1);
    }

    #[test]
    fn browse_rejects_tail_without_preparing_pragma_side_effects() {
        for rowids in [false, true] {
            for filter in ["1; PRAGMA foreign_keys=ON; --", "1; PRAGMA key='fixture-key'; --"] {
                let conn = Connection::open_in_memory().unwrap();
                conn.execute_batch("CREATE TABLE t(v); INSERT INTO t VALUES ('; literal'); PRAGMA foreign_keys=OFF;").unwrap();
                let browse = if rowids { browse_with_rowid } else { browse_table };
                assert!(browse(&conn, "t", &[], Some(filter), 500, 0).is_err());
                assert_eq!(read_pragma(&conn, "foreign_keys").unwrap(), "0");
                let dir = tempfile::tempdir().unwrap();
                let output = dir.path().join("keep.json");
                fs::write(&output, "previous export").unwrap();
                assert!(export_table(&conn, "t", &[], Some(filter), &output, true).is_err());
                assert_eq!(fs::read_to_string(&output).unwrap(), "previous export");
                copy_database_file(&conn, &dir.path().join("copy.db")).unwrap();
                assert_eq!(browse(&conn, "t", &[], Some("v='; literal' /* ; harmless */"), 500, 0).unwrap().rows.len(), 1);
            }
        }
    }

    #[test]
    fn dump_roundtrip_preserves_all_rows_types_triggers_and_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let source = Connection::open_in_memory().unwrap();
        source.execute_batch("CREATE TABLE t(id INTEGER PRIMARY KEY AUTOINCREMENT, v, g GENERATED ALWAYS AS (id+1)); CREATE TABLE log(v); CREATE TRIGGER audit AFTER INSERT ON t BEGIN INSERT INTO log VALUES (new.id); END;").unwrap();
        for i in 0..601 {
            source.execute("INSERT INTO t(v) VALUES (?1)", [i]).unwrap();
        }
        for v in [
            rusqlite::types::Value::Null,
            rusqlite::types::Value::Text("".into()),
            rusqlite::types::Value::Text("quote'中文\0tail".into()),
            rusqlite::types::Value::Blob(vec![0, 255, 39]),
            rusqlite::types::Value::Real(1.0),
            rusqlite::types::Value::Real(f64::INFINITY),
        ] {
            source.execute("INSERT INTO t(v) VALUES (?1)", [v]).unwrap();
        }
        source
            .execute_batch("INSERT INTO t(id) VALUES (1000); DELETE FROM t WHERE id=1000;")
            .unwrap();
        for opts in [
            ExportSqlOptions::default(),
            ExportSqlOptions {
                drop_if_exists: true,
                ..Default::default()
            },
        ] {
            let path = dir.path().join("dump.sql");
            export_sql_dump_opts(&source, &path, &opts).unwrap();
            let dest = Connection::open_in_memory().unwrap();
            dest.execute_batch(&fs::read_to_string(&path).unwrap())
                .unwrap();
            for table in ["t", "log", "sqlite_sequence"] {
                let read = |conn: &Connection| {
                    let mut stmt = conn
                        .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
                        .unwrap();
                    let n = stmt.column_count();
                    stmt.query_map([], |r| {
                        (0..n)
                            .map(|i| r.get::<_, rusqlite::types::Value>(i))
                            .collect::<rusqlite::Result<Vec<_>>>()
                    })
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap()
                };
                assert_eq!(read(&source), read(&dest), "table {table}");
            }
            dest.execute("INSERT INTO t(v) VALUES (0)", []).unwrap();
            assert_eq!(dest.last_insert_rowid(), 1001);
        }
    }

    #[test]
    fn sql_batch_handles_quotes_comments_triggers_and_returning() {
        let conn = Connection::open_in_memory().unwrap();
        let result = execute_sql(&conn, "-- heading;\n CREATE TABLE t(v); CREATE TRIGGER tr AFTER INSERT ON t BEGIN UPDATE t SET v=v||';'; END; INSERT INTO t VALUES ('a;b'); SELECT v FROM t; -- trailing;").unwrap();
        assert_eq!(result.rows, vec![vec!["a;b;".to_string()]]);
        let result = execute_sql(&conn, "INSERT INTO t VALUES ('c') RETURNING v;").unwrap();
        assert_eq!(result.rows, vec![vec!["c".to_string()]]);
        assert_eq!(count_rows(&conn, "t").unwrap(), 2);
        begin_deferred(&conn).unwrap();
        conn.execute("INSERT INTO t VALUES ('pending')", [])
            .unwrap();
        begin_deferred(&conn).unwrap();
        rollback_and_begin(&conn).unwrap();
        assert_eq!(count_rows(&conn, "t").unwrap(), 2);
    }

    #[test]
    fn seed_and_browse() {
        let (path, conn) = tmp_db();
        seed_demo(&conn).unwrap();
        let schema = list_schema(&conn).unwrap();
        assert!(schema.iter().any(|o| o.name == "customers"));
        assert!(schema.iter().any(|o| o.kind == ObjectKind::View));
        let q = browse_table(&conn, "products", &[OrderBy { column: "price".into(), descending: true }], None, 10, 0).unwrap();
        assert!(q.columns.contains(&"sku".into()));
        assert!(!q.rows.is_empty());
        let preview = browse_table(&conn, "products", &[], None, 2, 0).unwrap();
        assert_eq!(preview.rows.len(), 2);
        assert!(preview.truncated);
        let n = count_rows(&conn, "customers").unwrap();
        assert_eq!(n, 3);
        drop(conn);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn csv_decoder_rejects_incomplete_and_invalid_utf16_and_utf32_boms() {
        for raw in [vec![0xff,0xfe,0], vec![0xfe,0xff,0], vec![0xff,0xfe,0,0xd8], vec![0xfe,0xff,0xd8,0]] {
            assert!(decode_csv_bytes(&raw).is_err(), "{raw:?}");
        }
        for raw in [vec![0xff,0xfe,0,0,65,0,0,0], vec![0,0,0xfe,0xff,0,0,0,65]] {
            assert!(decode_csv_bytes(&raw).unwrap_err().contains("UTF-32"));
        }
        assert_eq!(decode_csv_bytes(&[0xff,0xfe,0x3d,0xd8,0,0xde]).unwrap(), "😀");
        assert_eq!(decode_csv_bytes(&[0xfe,0xff,0xd8,0x3d,0xde,0]).unwrap(), "😀");
    }

    #[test]
    fn csv_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "e28-csv-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let csv_path = dir.join("t.csv");
        export_csv(
            &csv_path,
            &["a".into(), "b".into()],
            &[
                vec!["1".into(), "hello, world".into()],
                vec!["2".into(), "x\"y".into()],
            ],
        )
        .unwrap();
        let (cols, rows) = import_csv_parse(&csv_path).unwrap();
        assert_eq!(cols, vec!["a", "b"]);
        assert_eq!(rows[0][1], "hello, world");
        assert_eq!(rows[1][1], "x\"y");
        for (name, little_endian) in [("utf16le.csv", true), ("utf16be.csv", false)] {
            let mut bytes = vec![if little_endian { 0xff } else { 0xfe }, if little_endian { 0xfe } else { 0xff }];
            for unit in "a,b\n1,東京\n".encode_utf16() {
                bytes.extend(if little_endian { unit.to_le_bytes() } else { unit.to_be_bytes() });
            }
            let path = dir.join(name);
            fs::write(&path, bytes).unwrap();
            let (cols, rows) = import_csv_parse(&path).unwrap();
            assert_eq!(cols, vec!["a", "b"]);
            assert_eq!(rows[0][1], "東京");
        }

        let (db_path, conn) = tmp_db();
        import_csv_into_table(&conn, "imported", &cols, &rows, true).unwrap();
        let q = browse_table(&conn, "imported", &[], None, 10, 0).unwrap();
        assert_eq!(q.rows.len(), 2);
        drop(conn);
        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn sql_dump_and_pragma() {
        let (path, conn) = tmp_db();
        seed_demo(&conn).unwrap();
        write_pragma(&conn, "foreign_keys", "ON").unwrap();
        let fk = read_pragma(&conn, "foreign_keys").unwrap();
        assert!(fk == "1" || fk.eq_ignore_ascii_case("on") || fk == "true");
        let dump = path.with_extension("sql");
        export_sql_dump(&conn, &dump).unwrap();
        let text = fs::read_to_string(&dump).unwrap();
        assert!(text.contains("CREATE TABLE"));
        assert!(text.contains("INSERT INTO"));
        drop(conn);
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(dump);
    }

    #[test]
    fn sqlcipher_linked() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(
            cipher_is_linked(&conn),
            "bundled-sqlcipher-vendored-openssl should expose cipher_version"
        );
    }

    #[test]
    fn sqlcipher_key_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "e28-cipher-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("enc.db");
        {
            let conn = Connection::open(&path).unwrap();
            assert!(cipher_is_linked(&conn));
            apply_cipher_key(&conn, "secret", 4096, 256000).unwrap();
            conn.execute_batch("CREATE TABLE t(x INTEGER); INSERT INTO t VALUES (42);")
                .unwrap();
        }
        let conn2 = Connection::open(&path).unwrap();
        apply_cipher_key(&conn2, "secret", 4096, 256000).unwrap();
        let n: i64 = conn2
            .query_row("SELECT x FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 42);
        let _ = fs::remove_dir_all(dir);
    }
}

/// Options for Export → Database to SQL file (DB4S ExportSqlDialog).
#[derive(Debug, Clone)]
pub struct ExportSqlOptions {
    pub structure: bool,
    pub data: bool,
    pub drop_if_exists: bool,
    pub keep_fk: bool,
}

impl Default for ExportSqlOptions {
    fn default() -> Self {
        Self {
            structure: true,
            data: true,
            drop_if_exists: false,
            keep_fk: true,
        }
    }
}

pub fn export_sql_dump_opts(
    conn: &Connection,
    path: &Path,
    opts: &ExportSqlOptions,
) -> Result<(), String> {
    guard_output_path(conn, path)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut f = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let snapshot = if conn.is_autocommit() {
        Some(conn.unchecked_transaction().map_err(|e| e.to_string())?)
    } else {
        None
    };
    let schema = list_schema(conn)?;
    if schema.iter().any(|o| {
        o.sql
            .as_deref()
            .unwrap_or("")
            .to_ascii_uppercase()
            .starts_with("CREATE VIRTUAL TABLE")
    }) {
        return Err("SQL dump of virtual tables is unsupported; use Save Database As".into());
    }
    if opts.keep_fk {
        writeln!(f, "PRAGMA foreign_keys=OFF;").map_err(|e| e.to_string())?;
    }
    writeln!(f, "BEGIN TRANSACTION;").map_err(|e| e.to_string())?;
    if opts.structure {
        if opts.drop_if_exists {
            for obj in schema.iter().rev() {
                writeln!(
                    f,
                    "DROP {} IF EXISTS {};",
                    obj.kind.sql_type(),
                    quote_ident(&obj.name)
                )
                .map_err(|e| e.to_string())?;
            }
        }
        for obj in schema.iter().filter(|o| o.kind == ObjectKind::Table) {
            if let Some(sql) = &obj.sql {
                writeln!(f, "{};", sql.trim().trim_end_matches(';')).map_err(|e| e.to_string())?;
            }
        }
    }
    if opts.data {
        for obj in schema.iter().filter(|o| o.kind == ObjectKind::Table) {
            // table_info excludes generated columns, which cannot be inserted into.
            let mut cols: Vec<String> = table_columns(conn, &obj.name)?
                .iter()
                .map(|c| quote_ident(&c.name))
                .collect();
            let all_columns: Vec<String> = conn
                .prepare(&format!("SELECT * FROM {}", qualified_name("main", &obj.name)))
                .map_err(|e| e.to_string())?
                .column_names()
                .iter()
                .map(|s| s.to_string())
                .collect();
            // Preserve implicit rowids where an unshadowed alias is available.
            for alias in ["rowid", "_rowid_", "oid"] {
                if !all_columns.iter().any(|c| c.eq_ignore_ascii_case(alias))
                    && conn
                        .prepare(&format!("SELECT {alias} FROM {}", qualified_name("main", &obj.name)))
                        .is_ok()
                {
                    cols.insert(0, alias.into());
                    break;
                }
            }
            let cols = cols.join(", ");
            let mut stmt = conn
                .prepare(&format!("SELECT {cols} FROM {}", qualified_name("main", &obj.name)))
                .map_err(|e| e.to_string())?;
            let n = stmt.column_count();
            let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
            while let Some(row) = rows.next().map_err(|e| e.to_string())? {
                let values = (0..n)
                    .map(|i| row.get_ref(i).map(sql_literal))
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(|e| e.to_string())?;
                writeln!(
                    f,
                    "INSERT INTO {} ({cols}) VALUES ({});",
                    quote_ident(&obj.name),
                    values.join(", ")
                )
                .map_err(|e| e.to_string())?;
            }
        }
        if conn
            .prepare("SELECT name, seq FROM main.sqlite_sequence")
            .is_ok()
        {
            writeln!(f, "DELETE FROM sqlite_sequence;").map_err(|e| e.to_string())?;
            let mut stmt = conn
                .prepare("SELECT name, seq FROM main.sqlite_sequence")
                .map_err(|e| e.to_string())?;
            let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
            while let Some(row) = rows.next().map_err(|e| e.to_string())? {
                writeln!(
                    f,
                    "INSERT INTO sqlite_sequence VALUES ({}, {});",
                    sql_literal(row.get_ref(0).map_err(|e| e.to_string())?),
                    sql_literal(row.get_ref(1).map_err(|e| e.to_string())?)
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }
    // Triggers must be created after rows, or restore can change data a second time.
    if opts.structure {
        for obj in schema.iter().filter(|o| o.kind != ObjectKind::Table) {
            if let Some(sql) = &obj.sql {
                writeln!(f, "{};", sql.trim().trim_end_matches(';')).map_err(|e| e.to_string())?;
            }
        }
    }
    writeln!(f, "COMMIT;").map_err(|e| e.to_string())?;
    if opts.keep_fk {
        writeln!(f, "PRAGMA foreign_keys=ON;").map_err(|e| e.to_string())?;
    }
    drop(snapshot);
    f.as_file().sync_all().map_err(|e| e.to_string())?;
    f.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn sql_literal(value: rusqlite::types::ValueRef<'_>) -> String {
    use rusqlite::types::ValueRef;
    match value {
        ValueRef::Null => "NULL".into(),
        ValueRef::Integer(v) => v.to_string(),
        ValueRef::Real(v) if v == f64::INFINITY => "9e999".into(),
        ValueRef::Real(v) if v == f64::NEG_INFINITY => "-9e999".into(),
        ValueRef::Real(v) => format!("{v:?}"),
        ValueRef::Text(v) => format!("CAST(X'{}' AS TEXT)", to_hex(v)),
        ValueRef::Blob(v) => format!("X'{}'", to_hex(v)),
    }
}

/// Foreign-key metadata for a table column (to parent table/col).
#[derive(Debug, Clone)]
pub struct FkRef {
    pub id: i64,
    pub seq: i64,
    pub from_col: String,
    pub to_table: String,
    pub to_col: String,
}

pub fn list_foreign_keys(conn: &Connection, table: &str) -> Result<Vec<FkRef>, String> {
    list_foreign_keys_in(conn, "main", table)
}

pub fn list_foreign_keys_in(conn: &Connection, schema: &str, table: &str) -> Result<Vec<FkRef>, String> {
    let sql = format!("PRAGMA {}.foreign_key_list({})", quote_ident(schema), quote_ident(table));
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(FkRef {
                id: r.get(0)?,
                seq: r.get(1)?,
                from_col: r.get::<_, String>(3)?,
                to_table: r.get::<_, String>(2)?,
                to_col: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        let mut fk = row.map_err(|e| e.to_string())?;
        if fk.to_col.is_empty() {
            let sql = format!("PRAGMA {}.table_info({})", quote_ident(schema), quote_ident(&fk.to_table));
            let mut parent = conn.prepare(&sql).map_err(|e| e.to_string())?;
            let mut columns = parent.query([]).map_err(|e| e.to_string())?;
            while let Some(column) = columns.next().map_err(|e| e.to_string())? {
                if column.get::<_, i64>(5).map_err(|e| e.to_string())? == fk.seq + 1 {
                    fk.to_col = column.get(1).map_err(|e| e.to_string())?;
                    break;
                }
            }
            if fk.to_col.is_empty() { return Err("Cannot resolve parent primary key".into()); }
        }
        out.push(fk);
    }
    Ok(out)
}

/// Detect whether a file looks like a plain SQLite DB (vs SQLCipher / unknown).
pub fn looks_like_sqlite(path: &Path) -> Result<bool, String> {
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let mut buf = [0u8; 16];
    use std::io::Read;
    let n = f.read(&mut buf).map_err(|e| e.to_string())?;
    if n < 16 {
        return Ok(false);
    }
    Ok(&buf[..15] == b"SQLite format 3")
}

/// True when this build links SQLCipher (`PRAGMA cipher_version` works).
pub fn cipher_is_linked(conn: &Connection) -> bool {
    conn.query_row("PRAGMA cipher_version;", [], |r| r.get::<_, String>(0))
        .is_ok()
}

/// Apply SQLCipher key settings. On stock bundled SQLite this returns a clear error.
pub fn apply_cipher_key(
    conn: &Connection,
    passphrase: &str,
    page_size: i64,
    kdf_iter: i64,
) -> Result<(), String> {
    validate_cipher_settings(passphrase, page_size, kdf_iter)?;
    if !cipher_is_linked(conn) {
        return Err("SQLCipher not linked".into());
    }
    conn.pragma_update(None, "key", passphrase)
        .map_err(|e| e.to_string())?;
    conn.pragma_update(None, "cipher_page_size", page_size)
        .map_err(|e| e.to_string())?;
    conn.pragma_update(None, "kdf_iter", kdf_iter)
        .map_err(|e| e.to_string())?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })
    .map_err(|e| format!("Cipher key rejected: {e}"))?;
    Ok(())
}

fn validate_cipher_settings(passphrase: &str, page_size: i64, kdf_iter: i64) -> Result<(), String> {
    if passphrase.is_empty()
        || !(512..=65536).contains(&page_size)
        || !(page_size as u64).is_power_of_two()
        || kdf_iter <= 0
    {
        return Err("A nonempty key, power-of-two page size (512–65536) and positive KDF iterations are required".into());
    }
    Ok(())
}

/// Hex-encode bytes for Edit Cell hex mode.
pub fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02X}"));
    }
    s
}

pub fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() % 2 != 0 {
        return Err("hex length must be even".into());
    }
    let mut out = Vec::with_capacity(clean.len() / 2);
    let bytes = clean.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let h = std::str::from_utf8(&bytes[i..i + 2]).map_err(|e| e.to_string())?;
        out.push(u8::from_str_radix(h, 16).map_err(|e| e.to_string())?);
        i += 2;
    }
    Ok(out)
}

pub fn looks_like_png(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && &bytes[..8] == b"\x89PNG\r\n\x1a\n"
}

pub fn looks_like_jpeg(bytes: &[u8]) -> bool {
    bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF
}

pub fn try_pretty_json(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    serde_json::to_string_pretty(&value).ok()
}

/// Export a new encrypted copy without changing the source key.
pub fn encrypt_database_to(
    conn: &Connection,
    dest: &Path,
    passphrase: &str,
    page_size: i64,
    kdf_iter: i64,
) -> Result<(), String> {
    validate_cipher_settings(passphrase, page_size, kdf_iter)?;
    if !cipher_is_linked(conn) {
        return Err("SQLCipher not linked".into());
    }
    if dest.exists() {
        return Err("Encrypted export requires a new destination".into());
    }
    let parent = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    conn.execute(
        "ATTACH DATABASE ?1 AS e28_encrypted KEY ?2",
        rusqlite::params![file.path().to_string_lossy(), passphrase],
    )
    .map_err(|e| e.to_string())?;
    let result = (|| {
        conn.pragma_update(
            Some(rusqlite::DatabaseName::Attached("e28_encrypted")),
            "cipher_page_size",
            page_size,
        )
        .map_err(|e| e.to_string())?;
        conn.pragma_update(
            Some(rusqlite::DatabaseName::Attached("e28_encrypted")),
            "kdf_iter",
            kdf_iter,
        )
        .map_err(|e| e.to_string())?;
        conn.execute_batch("SELECT sqlcipher_export('e28_encrypted')")
            .map_err(|e| e.to_string())?;
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        conn.pragma_update(
            Some(rusqlite::DatabaseName::Attached("e28_encrypted")),
            "user_version",
            version,
        )
        .map_err(|e| e.to_string())
    })();
    let detach = conn
        .execute_batch("DETACH DATABASE e28_encrypted")
        .map_err(|e| e.to_string());
    result?;
    detach?;
    file.persist_noclobber(dest).map_err(|e| e.to_string())?;
    Ok(())
}

/// Load a SQLite extension (.so/.dylib/.dll). Requires `load_extension` feature.
pub fn load_extension(conn: &Connection, path: &str, entry: Option<&str>) -> Result<(), String> {
    // rusqlite marks extension loading unsafe; caller controls path/entry.
    unsafe {
        conn.load_extension_enable().map_err(|e| e.to_string())?;
        let r = match entry {
            Some(e) if !e.is_empty() => conn.load_extension(path, Some(e)),
            _ => conn.load_extension(path, None::<&str>),
        };
        let _ = conn.load_extension_disable();
        r.map_err(|e| e.to_string())
    }
}

/// Human-readable schema report for Print Schema.
pub fn schema_report(conn: &Connection) -> Result<String, String> {
    let schema = list_schema(conn)?;
    let mut out = String::from("-- Schema report (e28 / DB4S Print Schema)\n");
    out.push_str("-- Generated for connection\n\n");
    for obj in &schema {
        out.push_str(&format!("-- {:?} `{}`\n", obj.kind, obj.name));
        if let Some(sql) = &obj.sql {
            if !sql.is_empty() {
                out.push_str(sql.trim());
                out.push_str(";\n\n");
            }
        }
    }
    Ok(out)
}
