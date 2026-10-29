//! Typed record identities shared by every Browse mutation.
use super::*;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct RowKey {
    pub columns: Arc<[String]>,
    pub values: Vec<CellValue>,
}

impl RowKey {
    pub fn predicate(&self) -> Result<String, String> {
        if self.columns.is_empty() || self.columns.len() != self.values.len() {
            return Err("Record key dimensions do not match".into());
        }
        Ok(self
            .columns
            .iter()
            .map(|column| format!("cell.{} IS ?", quote_ident(column)))
            .collect::<Vec<_>>()
            .join(" AND "))
    }

    fn match_count(&self, conn: &Connection, schema: &str, table: &str) -> Result<usize, String> {
        let sql = format!(
            "SELECT 1 FROM {} AS cell WHERE {} LIMIT 2",
            qualified_name(schema, table),
            self.predicate()?
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let mut rows = stmt
            .query(params_from_iter(&self.values))
            .map_err(|e| e.to_string())?;
        let mut count = 0;
        while rows.next().map_err(|e| e.to_string())?.is_some() {
            count += 1;
        }
        Ok(count)
    }

    fn require_unique(&self, conn: &Connection, schema: &str, table: &str) -> Result<(), String> {
        match self.match_count(conn, schema, table)? {
            1 => Ok(()),
            0 => Err("The selected record no longer exists; refresh Browse".into()),
            _ => Err("The selected record key is not unique; no rows changed".into()),
        }
    }
}

fn key_columns(conn: &Connection, schema: &str, table: &str) -> Result<Arc<[String]>, String> {
    let info = format!(
        "PRAGMA {}.table_xinfo({})",
        quote_ident(schema),
        quote_ident(table)
    );
    let mut stmt = conn.prepare(&info).map_err(|e| e.to_string())?;
    let fields = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, usize>(5)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    for alias in ["rowid", "_rowid_", "oid"] {
        if !fields
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(alias))
            && conn
                .prepare(&format!(
                    "SELECT cell.{} FROM {} AS cell LIMIT 0",
                    quote_ident(alias),
                    qualified_name(schema, table)
                ))
                .is_ok()
        {
            return Ok(vec![alias.into()].into());
        }
    }
    let mut primary = fields
        .into_iter()
        .filter(|(_, ordinal)| *ordinal > 0)
        .collect::<Vec<_>>();
    primary.sort_by_key(|(_, ordinal)| *ordinal);
    if primary.is_empty() {
        return Err("This source has no usable record key".into());
    }
    Ok(primary
        .into_iter()
        .map(|(column, _)| column)
        .collect::<Vec<_>>()
        .into())
}

pub fn browse_with_keys_in(
    conn: &Connection,
    schema: &str,
    table: &str,
    ordering: &[OrderBy],
    filter: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<(QueryResult, Vec<RowKey>), String> {
    let columns = key_columns(conn, schema, table)?;
    let limit = if limit == 0 { DEFAULT_LIMIT } else { limit };
    let keys = columns
        .iter()
        .map(|column| format!("{}.{}", quote_ident(table), quote_ident(column)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut sql = format!(
        "SELECT {keys}, {}.* FROM {}",
        quote_ident(table),
        qualified_name(schema, table)
    );
    if let Some(filter) = filter.map(str::trim).filter(|s| !s.is_empty()) {
        sql.push_str(" WHERE ");
        sql.push_str(filter);
    }
    append_order_by(conn, schema, table, &mut sql, ordering)?;
    sql.push_str(&format!(
        " LIMIT {} OFFSET {offset}",
        limit.saturating_add(1)
    ));
    let mut result = query_preview(conn, &sql, limit)?;
    let mut row_keys = Vec::with_capacity(result.rows.len());
    for row in result.values.rows()? {
        row_keys.push(RowKey {
            columns: columns.clone(),
            values: row?[..columns.len()].to_vec(),
        });
    }
    result.columns.drain(..columns.len());
    for row in &mut result.rows {
        row.drain(..columns.len());
    }
    for _ in &*columns {
        result.values.remove_column(0);
    }
    Ok((result, row_keys))
}

pub fn read_keyed_cell_in(
    conn: &Connection,
    schema: &str,
    table: &str,
    key: &RowKey,
    column: &str,
) -> Result<CellValue, String> {
    key.require_unique(conn, schema, table)?;
    let sql = format!(
        "SELECT cell.{} FROM {} AS cell WHERE {}",
        quote_ident(column),
        qualified_name(schema, table),
        key.predicate()?
    );
    conn.query_row(&sql, params_from_iter(&key.values), |row| {
        row.get_ref(0).map(CellValue::from)
    })
    .map_err(|e| e.to_string())
}

pub fn delete_row_in(
    conn: &Connection,
    schema: &str,
    table: &str,
    key: &RowKey,
) -> Result<u64, String> {
    with_savepoint(conn, || {
        key.require_unique(conn, schema, table)?;
        let sql = format!(
            "DELETE FROM {} AS cell WHERE {}",
            qualified_name(schema, table),
            key.predicate()?
        );
        let changed = conn
            .execute(&sql, params_from_iter(&key.values))
            .map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err("The row deletion was ignored; no rows changed".into());
        }
        Ok(changed as u64)
    })
}

pub fn update_cell_in(
    conn: &Connection,
    schema: &str,
    table: &str,
    key: &RowKey,
    column: &str,
    value: &rusqlite::types::Value,
) -> Result<Option<RowKey>, String> {
    let value = CellValue::from(rusqlite::types::ValueRef::from(value));
    Ok(update_cells_in(
        conn,
        schema,
        table,
        &[column.into()],
        &[(key.clone(), vec![value])],
    )?
    .remove(0))
}

/// Every target is checked inside the same savepoint; one stale/ambiguous key rolls back the rectangle.
pub fn update_cells_in(
    conn: &Connection,
    schema: &str,
    table: &str,
    columns: &[String],
    updates: &[(RowKey, Vec<CellValue>)],
) -> Result<Vec<Option<RowKey>>, String> {
    if columns.is_empty()
        || updates.is_empty()
        || updates.iter().any(|(_, row)| row.len() != columns.len())
    {
        return Err("Cell update dimensions do not match".into());
    }
    let available = table_columns_in(conn, schema, table)?;
    if columns.iter().any(|column| {
        !available
            .iter()
            .any(|field| field.name.eq_ignore_ascii_case(column))
    }) {
        return Err("Selected column is not editable".into());
    }
    let virtual_table: bool = conn.query_row(&format!("SELECT sql LIKE 'CREATE VIRTUAL TABLE%' FROM {}.sqlite_schema WHERE type='table' AND name=?", quote_ident(schema)), [table], |row| row.get(0)).map_err(|e| e.to_string())?;
    let assignments = columns
        .iter()
        .map(|column| format!("{} = ?", quote_ident(column)))
        .collect::<Vec<_>>()
        .join(", ");
    with_savepoint(conn, || {
        let mut updated = Vec::with_capacity(updates.len());
        for (key, values) in updates {
            key.require_unique(conn, schema, table)?;
            let mut sql = format!(
                "UPDATE {} AS cell SET {assignments} WHERE {}",
                qualified_name(schema, table),
                key.predicate()?
            );
            let params = || params_from_iter(values.iter().chain(&key.values));
            let returned = if virtual_table {
                // SQLite cannot RETURNING virtual updates; retain only a still-unique original key.
                if conn.execute(&sql, params()).map_err(|e| e.to_string())? != 1 {
                    return Err("The cell update was ignored; draft retained".into());
                }
                key.clone()
            } else {
                sql.push_str(" RETURNING ");
                sql.push_str(
                    &key.columns
                        .iter()
                        .map(|column| quote_ident(column))
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
                let mut rows = stmt.query(params()).map_err(|e| e.to_string())?;
                let row = rows
                    .next()
                    .map_err(|e| e.to_string())?
                    .ok_or("The cell update was ignored; draft retained")?;
                let values = (0..key.columns.len())
                    .map(|index| row.get_ref(index).map(CellValue::from))
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(|e| e.to_string())?;
                if rows.next().map_err(|e| e.to_string())?.is_some() {
                    return Err("Record key changed multiple rows; no rows changed".into());
                }
                RowKey {
                    columns: key.columns.clone(),
                    values,
                }
            };
            // AFTER triggers may move/delete the record after RETURNING computed its key.
            updated.push((returned.match_count(conn, schema, table)? == 1).then_some(returned));
        }
        Ok(updated)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composite_keys_keep_raw_types_order_affinity_and_quoted_namespaces() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("ATTACH ':memory:' AS \"aux.\"\"db\"; CREATE TABLE keeper(v); INSERT INTO keeper VALUES('saved'); BEGIN; INSERT INTO keeper VALUES('pending');").unwrap();
        for schema in ["main", "temp", "aux.\"db"] {
            let table = "key.\"table";
            let name = qualified_name(schema, table);
            conn.execute_batch(&format!("CREATE TABLE {name}(a TEXT COLLATE NOCASE, b BLOB, c INTEGER, v, rowid, __rowid__ AS(c+1), PRIMARY KEY(c,b,a)) WITHOUT ROWID;
                INSERT INTO {name}(a,b,c,v,rowid) VALUES(CAST(X'80FF' AS TEXT),X'00FF',9007199254740993,'before',-8),('Alpha',X'01',2,'second',-8);")).unwrap();
            let (result, keys) = browse_with_keys_in(
                &conn,
                schema,
                table,
                &[],
                Some(&format!("{}.c>0", quote_ident(table))),
                500,
                0,
            )
            .unwrap();
            assert_eq!(result.columns, ["a", "b", "c", "v", "rowid", "__rowid__"]);
            let key = keys
                .iter()
                .find(|key| key.values[0] == CellValue::Integer(9007199254740993))
                .unwrap();
            assert_eq!(key.columns.as_ref(), ["c", "b", "a"]);
            assert_eq!(
                key.values,
                [
                    CellValue::Integer(9007199254740993),
                    CellValue::Blob(vec![0, 255]),
                    CellValue::Text(vec![128, 255])
                ]
            );
            assert_eq!(
                read_keyed_cell_in(&conn, schema, table, key, "v").unwrap(),
                CellValue::Text(b"before".to_vec())
            );
            let updated = update_cell_in(
                &conn,
                schema,
                table,
                key,
                "c",
                &rusqlite::types::Value::Text("00042".into()),
            )
            .unwrap()
            .unwrap();
            assert_eq!(updated.values[0], CellValue::Integer(42));
            assert!(read_keyed_cell_in(&conn, schema, table, key, "v").is_err());
            let updated = update_cell_in(
                &conn,
                schema,
                table,
                &updated,
                "v",
                &rusqlite::types::Value::Blob(vec![255, 0]),
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                read_keyed_cell_in(&conn, schema, table, &updated, "v").unwrap(),
                CellValue::Blob(vec![255, 0])
            );
            assert_eq!(delete_row_in(&conn, schema, table, &updated).unwrap(), 1);
            let mut case_key = keys
                .iter()
                .find(|key| key.values[0] == CellValue::Integer(2))
                .unwrap()
                .clone();
            case_key.values[2] = CellValue::Text(b"aLPHA".to_vec());
            assert_eq!(
                read_keyed_cell_in(&conn, schema, table, &case_key, "v").unwrap(),
                CellValue::Text(b"second".to_vec())
            );
            assert!(browse_with_keys_in(
                &conn,
                schema,
                table,
                &[],
                Some("1; DELETE FROM keeper;--"),
                500,
                0
            )
            .is_err());
            assert_eq!(count_rows(&conn, "keeper").unwrap(), 2);
        }
        assert!(!conn.is_autocommit());
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(count_rows(&conn, "keeper").unwrap(), 1);
    }

    #[test]
    fn shadowed_aliases_and_nullable_keys_never_target_multiple_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE shadow(v, rowid AS(1), _rowid_, oid); INSERT INTO shadow(v) VALUES('first'),('second');
            CREATE TABLE nullable(rowid,_rowid_,oid,k TEXT PRIMARY KEY,v); INSERT INTO nullable(k,v) VALUES(NULL,'a'),(NULL,'b'),('unique','c');").unwrap();
        assert!(browse_with_keys_in(&conn, "main", "shadow", &[], None, 500, 0).is_err());
        let (_, keys) = browse_with_keys_in(&conn, "main", "nullable", &[], None, 500, 0).unwrap();
        assert_eq!(keys[0].columns.as_ref(), ["k"]);
        assert!(update_cell_in(
            &conn,
            "main",
            "nullable",
            &keys[0],
            "v",
            &rusqlite::types::Value::Null
        )
        .unwrap_err()
        .contains("not unique"));
        assert!(delete_row_in(&conn, "main", "nullable", &keys[0])
            .unwrap_err()
            .contains("not unique"));
        let updates = [
            (
                keys[2].clone(),
                vec![CellValue::Text(b"must roll back".to_vec())],
            ),
            (keys[0].clone(), vec![CellValue::Null]),
        ];
        assert!(update_cells_in(&conn, "main", "nullable", &["v".into()], &updates).is_err());
        assert_eq!(
            read_keyed_cell_in(&conn, "main", "nullable", &keys[2], "v").unwrap(),
            CellValue::Text(b"c".to_vec())
        );
        let mut invalid = keys[2].clone();
        invalid.columns = vec!["missing".into()].into();
        assert!(read_keyed_cell_in(&conn, "main", "nullable", &invalid, "v").is_err());
        invalid.columns = vec!["k".into(), "v".into()].into();
        assert!(delete_row_in(&conn, "main", "nullable", &invalid).is_err());
        assert!(update_cell_in(
            &conn,
            "main",
            "nullable",
            &keys[2],
            "missing",
            &rusqlite::types::Value::Null
        )
        .is_err());
        assert_eq!(count_rows(&conn, "nullable").unwrap(), 3);
        conn.execute_batch("CREATE TABLE aliases(rowid AS(1),_rowid_ AS(2),v); INSERT INTO aliases(oid,v) VALUES(9007199254740993,'literal');").unwrap();
        let (_, keys) = browse_with_keys_in(&conn, "main", "aliases", &[], None, 500, 0).unwrap();
        assert_eq!(keys[0].columns.as_ref(), ["oid"]);
        assert_eq!(keys[0].values, [CellValue::Integer(9007199254740993)]);
        update_cell_in(
            &conn,
            "main",
            "aliases",
            &keys[0],
            "v",
            &rusqlite::types::Value::Text("saved".into()),
        )
        .unwrap();
        assert_eq!(
            conn.query_row("SELECT v FROM aliases", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "saved"
        );
        conn.execute_batch("CREATE TABLE reals(k REAL PRIMARY KEY,v) WITHOUT ROWID; INSERT INTO reals VALUES(1.25,'real');").unwrap();
        let (_, keys) = browse_with_keys_in(&conn, "main", "reals", &[], None, 500, 0).unwrap();
        assert_eq!(keys[0].values, [CellValue::Real(1.25)]);
        update_cell_in(
            &conn,
            "main",
            "reals",
            &keys[0],
            "v",
            &rusqlite::types::Value::Null,
        )
        .unwrap();
        assert_eq!(
            read_keyed_cell_in(&conn, "main", "reals", &keys[0], "v").unwrap(),
            CellValue::Null
        );
    }

    #[test]
    fn record_mutations_handle_virtual_tables_ignored_updates_and_trigger_moves() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE ordinary(id INTEGER PRIMARY KEY,v); INSERT INTO ordinary VALUES(9007199254740993,'before');
            CREATE TABLE keeper(v); INSERT INTO keeper VALUES('saved'); BEGIN; INSERT INTO keeper VALUES('pending');
            CREATE VIRTUAL TABLE docs USING fts5(v); INSERT INTO docs(rowid,v) VALUES(9,'alpha');
            CREATE VIRTUAL TABLE boxes USING rtree(id,x0,x1); INSERT INTO boxes VALUES(7,1,2);").unwrap();
        for (table, column, value) in [
            ("docs", "v", rusqlite::types::Value::Text("beta".into())),
            ("boxes", "x1", rusqlite::types::Value::Real(3.5)),
        ] {
            let (_, keys) = browse_with_keys_in(&conn, "main", table, &[], None, 500, 0).unwrap();
            assert!(
                update_cell_in(&conn, "main", table, &keys[0], column, &value)
                    .unwrap()
                    .is_some()
            );
            assert_eq!(
                read_keyed_cell_in(&conn, "main", table, &keys[0], column)
                    .unwrap()
                    .as_ref(),
                rusqlite::types::ValueRef::from(&value)
            );
            assert_eq!(delete_row_in(&conn, "main", table, &keys[0]).unwrap(), 1);
        }
        let (_, keys) = browse_with_keys_in(&conn, "main", "ordinary", &[], None, 500, 0).unwrap();
        let moved = update_cell_in(
            &conn,
            "main",
            "ordinary",
            &keys[0],
            "id",
            &rusqlite::types::Value::Text("00042".into()),
        )
        .unwrap()
        .unwrap();
        assert_eq!(moved.values, [CellValue::Integer(42)]);
        conn.execute_batch("CREATE TRIGGER deny_update BEFORE UPDATE ON ordinary BEGIN SELECT RAISE(IGNORE); END; CREATE TRIGGER deny_delete BEFORE DELETE ON ordinary BEGIN SELECT RAISE(IGNORE); END;").unwrap();
        assert!(update_cell_in(
            &conn,
            "main",
            "ordinary",
            &moved,
            "v",
            &rusqlite::types::Value::Null
        )
        .unwrap_err()
        .contains("ignored"));
        assert!(delete_row_in(&conn, "main", "ordinary", &moved)
            .unwrap_err()
            .contains("ignored"));
        assert_eq!(
            read_keyed_cell_in(&conn, "main", "ordinary", &moved, "v").unwrap(),
            CellValue::Text(b"before".to_vec())
        );
        conn.execute_batch("DROP TRIGGER deny_update; DROP TRIGGER deny_delete; CREATE TRIGGER move_key AFTER UPDATE OF v ON ordinary BEGIN UPDATE ordinary SET id=id+100 WHERE id=new.id; END;").unwrap();
        assert!(update_cell_in(
            &conn,
            "main",
            "ordinary",
            &moved,
            "v",
            &rusqlite::types::Value::Text("moved".into())
        )
        .unwrap()
        .is_none());
        assert_eq!(
            conn.query_row("SELECT id FROM ordinary", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            142
        );
        assert!(!conn.is_autocommit());
        assert_eq!(count_rows(&conn, "keeper").unwrap(), 2);
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(count_rows(&conn, "keeper").unwrap(), 1);
    }
}
