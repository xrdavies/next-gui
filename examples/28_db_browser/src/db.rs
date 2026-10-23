#![allow(dead_code)]
//! SQLite helpers for the DB Browser example (rusqlite, bundled).

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use rusqlite::{params_from_iter, Connection, OpenFlags};

/// Schema object kinds shown in the structure tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub kind: ObjectKind,
    pub name: String,
    pub sql: Option<String>,
    pub tbl_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub rows_affected: Option<u64>,
    pub truncated: bool,
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

/// Open connection (file or `:memory:`).
pub fn open_path(path: &str) -> Result<Connection, String> {
    if path == ":memory:" || path.is_empty() {
        Connection::open_in_memory().map_err(|e| e.to_string())
    } else {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE;
        Connection::open_with_flags(path, flags).map_err(|e| e.to_string())
    }
}

/// Create a new empty file DB (truncate if exists).
pub fn create_new(path: &str) -> Result<Connection, String> {
    if path != ":memory:" {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        if Path::new(path).exists() {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
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
    let mut stmt = conn
        .prepare(
            "SELECT type, name, tbl_name, sql FROM sqlite_master
             WHERE name NOT LIKE 'sqlite_%'
             ORDER BY CASE type
                WHEN 'table' THEN 0
                WHEN 'view' THEN 1
                WHEN 'index' THEN 2
                WHEN 'trigger' THEN 3
                ELSE 4 END, name COLLATE NOCASE",
        )
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
    let q = format!("PRAGMA table_info({})", quote_ident(table));
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

pub fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
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

const DEFAULT_LIMIT: usize = 500;

/// Browse a table/view with optional ORDER BY, WHERE filter, and limit.
pub fn browse_table(
    conn: &Connection,
    table: &str,
    order_col: Option<&str>,
    order_desc: bool,
    filter: Option<&str>,
    limit: usize,
) -> Result<QueryResult, String> {
    let limit = if limit == 0 { DEFAULT_LIMIT } else { limit };
    let mut sql = format!("SELECT * FROM {}", quote_ident(table));
    if let Some(f) = filter.map(str::trim).filter(|s| !s.is_empty()) {
        sql.push_str(" WHERE ");
        sql.push_str(f);
    }
    if let Some(col) = order_col.map(str::trim).filter(|s| !s.is_empty()) {
        sql.push_str(" ORDER BY ");
        sql.push_str(&quote_ident(col));
        if order_desc {
            sql.push_str(" DESC");
        } else {
            sql.push_str(" ASC");
        }
    }
    sql.push_str(&format!(" LIMIT {limit}"));
    execute_sql(conn, &sql)
}

/// Run arbitrary SQL. For multi-statement scripts, only the last result set is returned;
/// non-query statements return rows_affected.
pub fn execute_sql(conn: &Connection, sql: &str) -> Result<QueryResult, String> {
    let sql = sql.trim();
    if sql.is_empty() {
        return Err("empty SQL".into());
    }

    // Prefer query when the (last non-empty) statement looks like a SELECT/WITH/PRAGMA that returns rows.
    let last = sql
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .last()
        .unwrap_or(sql);
    let head = last
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();

    let returns_rows = matches!(
        head.as_str(),
        "select" | "with" | "pragma" | "explain" | "values"
    );

    if returns_rows && !sql.contains(';') {
        return query_sql(conn, sql);
    }

    if returns_rows && sql.matches(';').count() == 0 {
        return query_sql(conn, sql);
    }

    // Multi-statement or DML/DDL: execute_batch then optionally re-query last SELECT.
    if !returns_rows {
        conn.execute_batch(sql).map_err(|e| e.to_string())?;
        let affected = conn.changes();
        return Ok(QueryResult {
            columns: vec!["rows_affected".into()],
            rows: vec![vec![affected.to_string()]],
            rows_affected: Some(affected),
            truncated: false,
        });
    }

    // Mixed script ending in SELECT: run everything then query the last statement alone.
    let before = sql[..sql.rfind(last).unwrap_or(0)].trim_end_matches(';').trim();
    if !before.is_empty() {
        conn.execute_batch(before).map_err(|e| e.to_string())?;
    }
    query_sql(conn, last)
}

fn query_sql(conn: &Connection, sql: &str) -> Result<QueryResult, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let columns: Vec<String> = stmt.column_names().iter().map(|s| (*s).to_string()).collect();
    let col_count = columns.len();
    let mut rows_iter = stmt.query([]).map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    let mut truncated = false;
    while let Some(row) = rows_iter.next().map_err(|e| e.to_string())? {
        if rows.len() >= DEFAULT_LIMIT {
            truncated = true;
            break;
        }
        let mut cells = Vec::with_capacity(col_count);
        for i in 0..col_count {
            cells.push(cell_to_string(row.get_ref(i).map_err(|e| e.to_string())?));
        }
        rows.push(cells);
    }
    Ok(QueryResult {
        columns,
        rows,
        rows_affected: None,
        truncated,
    })
}

pub fn vacuum(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("VACUUM").map_err(|e| e.to_string())
}

pub fn count_rows(conn: &Connection, table: &str) -> Result<i64, String> {
    let sql = format!("SELECT COUNT(*) FROM {}", quote_ident(table));
    conn.query_row(&sql, [], |r| r.get(0))
        .map_err(|e| e.to_string())
}

pub fn insert_row(conn: &Connection, table: &str, columns: &[String], values: &[String]) -> Result<u64, String> {
    if columns.len() != values.len() {
        return Err("column/value length mismatch".into());
    }
    if columns.is_empty() {
        let sql = format!("INSERT INTO {} DEFAULT VALUES", quote_ident(table));
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
        quote_ident(table)
    );
    let params: Vec<&str> = values.iter().map(|s| s.as_str()).collect();
    conn.execute(&sql, params_from_iter(params))
        .map_err(|e| e.to_string())?;
    Ok(conn.changes())
}

pub fn delete_row_by_rowid(conn: &Connection, table: &str, rowid: i64) -> Result<u64, String> {
    let sql = format!("DELETE FROM {} WHERE rowid = ?1", quote_ident(table));
    conn.execute(&sql, [rowid]).map_err(|e| e.to_string())?;
    Ok(conn.changes())
}

pub fn update_cell(
    conn: &Connection,
    table: &str,
    rowid: i64,
    column: &str,
    value: &str,
) -> Result<u64, String> {
    let sql = format!(
        "UPDATE {} SET {} = ?1 WHERE rowid = ?2",
        quote_ident(table),
        quote_ident(column)
    );
    conn.execute(&sql, rusqlite::params![value, rowid])
        .map_err(|e| e.to_string())?;
    Ok(conn.changes())
}

pub fn browse_with_rowid(
    conn: &Connection,
    table: &str,
    order_col: Option<&str>,
    order_desc: bool,
    filter: Option<&str>,
    limit: usize,
) -> Result<QueryResult, String> {
    let limit = if limit == 0 { DEFAULT_LIMIT } else { limit };
    let mut sql = format!("SELECT rowid AS __rowid__, * FROM {}", quote_ident(table));
    if let Some(f) = filter.map(str::trim).filter(|s| !s.is_empty()) {
        sql.push_str(" WHERE ");
        sql.push_str(f);
    }
    if let Some(col) = order_col.map(str::trim).filter(|s| !s.is_empty()) {
        sql.push_str(" ORDER BY ");
        sql.push_str(&quote_ident(col));
        sql.push_str(if order_desc { " DESC" } else { " ASC" });
    }
    sql.push_str(&format!(" LIMIT {limit}"));
    execute_sql(conn, &sql)
}

pub fn drop_object(conn: &Connection, kind: ObjectKind, name: &str) -> Result<(), String> {
    let kw = match kind {
        ObjectKind::Table => "TABLE",
        ObjectKind::View => "VIEW",
        ObjectKind::Index => "INDEX",
        ObjectKind::Trigger => "TRIGGER",
    };
    let sql = format!("DROP {kw} IF EXISTS {}", quote_ident(name));
    conn.execute_batch(&sql).map_err(|e| e.to_string())
}

pub fn create_table_simple(conn: &Connection, name: &str, columns_ddl: &str) -> Result<(), String> {
    let sql = format!(
        "CREATE TABLE {} ({})",
        quote_ident(name),
        columns_ddl.trim().trim_end_matches(',')
    );
    conn.execute_batch(&sql).map_err(|e| e.to_string())
}

pub fn create_index(
    conn: &Connection,
    name: &str,
    table: &str,
    columns: &str,
    unique: bool,
) -> Result<(), String> {
    let uniq = if unique { "UNIQUE " } else { "" };
    let sql = format!(
        "CREATE {uniq}INDEX {} ON {} ({})",
        quote_ident(name),
        quote_ident(table),
        columns.trim()
    );
    conn.execute_batch(&sql).map_err(|e| e.to_string())
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
    let sql = format!("PRAGMA {key} = {value}");
    conn.execute_batch(&sql).map_err(|e| e.to_string())
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
    writeln!(f, "{}", columns.iter().map(|c| csv_escape(c)).collect::<Vec<_>>().join(","))
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

/// Parse a simple CSV file into header + rows.
pub fn import_csv_parse(path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let f = File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(f);
    let mut lines = reader.lines();
    let header_line = lines
        .next()
        .ok_or_else(|| "empty CSV".to_string())?
        .map_err(|e| e.to_string())?;
    let columns = parse_csv_line(&header_line);
    let mut rows = Vec::new();
    for line in lines {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        rows.push(parse_csv_line(&line));
    }
    Ok((columns, rows))
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if in_quotes {
                    if chars.peek() == Some(&'"') {
                        cur.push('"');
                        chars.next();
                    } else {
                        in_quotes = false;
                    }
                } else {
                    in_quotes = true;
                }
            }
            ',' if !in_quotes => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

pub fn import_csv_into_table(
    conn: &Connection,
    table: &str,
    columns: &[String],
    rows: &[Vec<String>],
    create_if_missing: bool,
) -> Result<u64, String> {
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
        let mut vals = Vec::with_capacity(columns.len());
        for i in 0..columns.len() {
            vals.push(row.get(i).cloned().unwrap_or_default());
        }
        n += insert_row(conn, table, columns, &vals)?;
    }
    Ok(n)
}

/// SQL dump (schema + data) similar to `.dump`.
pub fn export_sql_dump(conn: &Connection, path: &Path) -> Result<(), String> {
    let mut f = File::create(path).map_err(|e| e.to_string())?;
    writeln!(f, "BEGIN TRANSACTION;").map_err(|e| e.to_string())?;
    let schema = list_schema(conn)?;
    for obj in &schema {
        if let Some(sql) = &obj.sql {
            if !sql.is_empty() {
                writeln!(f, "{};", sql.trim().trim_end_matches(';')).map_err(|e| e.to_string())?;
            }
        }
    }
    for obj in schema.iter().filter(|o| o.kind == ObjectKind::Table) {
        let q = browse_table(conn, &obj.name, None, false, None, 100_000)?;
        let cols = q
            .columns
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        for row in &q.rows {
            let vals = row
                .iter()
                .map(|v| {
                    if v.is_empty() {
                        "NULL".to_string()
                    } else {
                        format!("'{}'", v.replace('\'', "''"))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(
                f,
                "INSERT INTO {} ({cols}) VALUES ({vals});",
                quote_ident(&obj.name)
            )
            .map_err(|e| e.to_string())?;
        }
    }
    writeln!(f, "COMMIT;").map_err(|e| e.to_string())?;
    Ok(())
}

pub fn import_sql_file(conn: &Connection, path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    conn.execute_batch(&text).map_err(|e| e.to_string())
}

/// Numeric series from a column for charts (first N finite values).
pub fn numeric_series(conn: &Connection, table: &str, column: &str, limit: usize) -> Result<Vec<f32>, String> {
    let sql = format!(
        "SELECT CAST({} AS REAL) FROM {} WHERE typeof({}) IN ('integer','real') OR CAST({} AS REAL) IS NOT NULL LIMIT {limit}",
        quote_ident(column),
        quote_ident(table),
        quote_ident(column),
        quote_ident(column),
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, f64>(0))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        let v = r.map_err(|e| e.to_string())?;
        if v.is_finite() {
            out.push(v as f32);
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

    fn tmp_db() -> (PathBuf, Connection) {
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
    fn seed_and_browse() {
        let (path, conn) = tmp_db();
        seed_demo(&conn).unwrap();
        let schema = list_schema(&conn).unwrap();
        assert!(schema.iter().any(|o| o.name == "customers"));
        assert!(schema.iter().any(|o| o.kind == ObjectKind::View));
        let q = browse_table(&conn, "products", Some("price"), true, None, 10).unwrap();
        assert!(q.columns.contains(&"sku".into()));
        assert!(!q.rows.is_empty());
        let n = count_rows(&conn, "customers").unwrap();
        assert_eq!(n, 3);
        drop(conn);
        let _ = fs::remove_file(path);
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
            &[vec!["1".into(), "hello, world".into()], vec!["2".into(), "x\"y".into()]],
        )
        .unwrap();
        let (cols, rows) = import_csv_parse(&csv_path).unwrap();
        assert_eq!(cols, vec!["a", "b"]);
        assert_eq!(rows[0][1], "hello, world");
        assert_eq!(rows[1][1], "x\"y");

        let (db_path, conn) = tmp_db();
        import_csv_into_table(&conn, "imported", &cols, &rows, true).unwrap();
        let q = browse_table(&conn, "imported", None, false, None, 10).unwrap();
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
}
