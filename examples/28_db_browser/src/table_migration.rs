//! Rebuild an ordinary SQLite table without committing the user's transaction.

use std::collections::HashSet;

use rusqlite::{config::DbConfig, Connection, OptionalExtension};

use crate::{
    db::{self, ColDef},
    table_sql,
};

type ForeignKeyViolation = (String, Option<i64>, String, i64);

fn foreign_key_violations(
    conn: &Connection,
    schema: &str,
) -> Result<HashSet<ForeignKeyViolation>, String> {
    conn.prepare(&format!(
        "PRAGMA {}.foreign_key_check",
        db::quote_ident(schema)
    ))
    .map_err(|error| error.to_string())?
    .query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })
    .map_err(|error| error.to_string())?
    .collect::<rusqlite::Result<HashSet<_>>>()
    .map_err(|error| error.to_string())
}

fn rowid_alias(conn: &Connection, schema: &str, table: &str) -> Result<Option<String>, String> {
    let names = db::table_column_names_in(conn, schema, table)?;
    for name in ["rowid", "_rowid_", "oid"] {
        if !names.iter().any(|column| column.eq_ignore_ascii_case(name))
            && conn
                .prepare(&format!(
                    "SELECT {name} FROM {} LIMIT 0",
                    db::qualified_name(schema, table)
                ))
                .is_ok()
        {
            return Ok(Some(name.into()));
        }
    }
    Ok(None)
}

fn without_rowid(conn: &Connection, schema: &str, table: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA {}.table_list", db::quote_ident(schema)))
        .map_err(|error| error.to_string())?;
    let tables = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, bool>(4)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    for result in tables {
        let (name, kind, without_rowid) = result.map_err(|error| error.to_string())?;
        if name == table {
            if kind != "table" {
                return Err(
                    "Rebuilding virtual or shadow tables requires a module-specific migration"
                        .into(),
                );
            }
            return Ok(without_rowid);
        }
    }
    Err("Original table no longer exists".into())
}

fn integer_primary_key(
    conn: &Connection,
    schema: &str,
    table: &str,
) -> Result<Option<String>, String> {
    if without_rowid(conn, schema, table)? {
        return Ok(None);
    }
    let mut stmt = conn
        .prepare(&format!(
            "PRAGMA {}.index_list({})",
            db::quote_ident(schema),
            db::quote_ident(table)
        ))
        .map_err(|error| error.to_string())?;
    let origins = stmt
        .query_map([], |row| row.get::<_, String>(3))
        .map_err(|error| error.to_string())?;
    for origin in origins {
        if origin.map_err(|error| error.to_string())? == "pk" {
            return Ok(None);
        }
    }
    Ok(db::table_columns_in(conn, schema, table)?
        .into_iter()
        .find(|col| col.pk && col.type_name.eq_ignore_ascii_case("INTEGER"))
        .map(|col| col.name))
}

fn sequence(conn: &Connection, schema: &str, table: &str) -> Result<Option<i64>, String> {
    let exists: bool = conn
        .query_row(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM {}.sqlite_schema WHERE name='sqlite_sequence')",
                db::quote_ident(schema)
            ),
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !exists {
        return Ok(None);
    }
    conn.query_row(
        &format!(
            "SELECT seq FROM {}.sqlite_sequence WHERE name=?1",
            db::quote_ident(schema)
        ),
        [table],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub fn rebuild(
    conn: &Connection,
    schema: &str,
    table: &str,
    original: &[ColDef],
    draft: &[ColDef],
    sources: &[Option<String>],
) -> Result<(), String> {
    let old_without_rowid = without_rowid(conn, schema, table)?;
    let schema_sql = format!(
        "SELECT sql FROM {}.sqlite_schema WHERE type='table' AND name=?1",
        db::quote_ident(schema)
    );
    let sql: String = conn
        .query_row(&schema_sql, [table], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    let mut number = 0;
    let temporary = loop {
        let name = format!("__next_gui_rebuild_{number}");
        let exists: bool = conn
            .query_row(
                &format!(
                    "SELECT EXISTS(SELECT 1 FROM {}.sqlite_schema WHERE name=?1 COLLATE NOCASE)",
                    db::quote_ident(schema)
                ),
                [&name],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if !exists {
            break name;
        }
        number += 1;
    };
    let definition =
        table_sql::rebuild_definition(&sql, schema, &temporary, original, draft, sources)?;
    let mut stmt = conn.prepare(&format!("SELECT sql FROM {}.sqlite_schema WHERE tbl_name=?1 AND type IN ('index','trigger') AND sql IS NOT NULL ORDER BY type,name", db::quote_ident(schema))).map_err(|error| error.to_string())?;
    let mut dependencies = stmt
        .query_map([table], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?
        .map(|sql| table_sql::qualify_dependency(&sql.map_err(|error| error.to_string())?, schema))
        .collect::<Result<Vec<_>, String>>()?;
    if !schema.eq_ignore_ascii_case("temp") {
        let mut stmt = conn
            .prepare("SELECT sql FROM temp.sqlite_schema WHERE type='trigger' AND tbl_name=?1")
            .map_err(|error| error.to_string())?;
        let triggers = stmt
            .query_map([table], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        for trigger in triggers {
            let sql = trigger.map_err(|error| error.to_string())?;
            match table_sql::trigger_table_schema(&sql)? {
                Some(name) if name.eq(schema) => dependencies.push(table_sql::qualify_dependency(&sql, "temp")?),
                Some(_) => {}
                None => return Err("Qualify the target database of temporary triggers before rebuilding this table".into()),
            }
        }
    }
    let mut views = Vec::new();
    for object in db::list_schema(conn)? {
        if object.kind == db::ObjectKind::View {
            let query = format!(
                "SELECT * FROM {} LIMIT 0",
                db::qualified_name(&object.schema, &object.name)
            );
            if conn.prepare(&query).is_ok() {
                views.push(query);
            }
        }
    }
    let prior_violations = foreign_key_violations(conn, schema)?;
    // SQLite reports no row identity for WITHOUT ROWID violations.
    if prior_violations
        .iter()
        .any(|violation| violation.1.is_none())
    {
        return Err(
            "Resolve existing foreign key violations in WITHOUT ROWID tables before rebuilding"
                .into(),
        );
    }
    let old_sequence = if table_sql::autoincrement_column(&definition)?.is_some() {
        sequence(conn, schema, table)?
    } else {
        None
    };
    let foreign_keys = conn
        .db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY)
        .map_err(|error| error.to_string())?;
    let legacy_alter = conn
        .db_config(DbConfig::SQLITE_DBCONFIG_LEGACY_ALTER_TABLE)
        .map_err(|error| error.to_string())?;
    let result = (|| {
        // PRAGMA foreign_keys cannot change inside the pending user transaction.
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY, false)
            .map_err(|error| error.to_string())?;
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_LEGACY_ALTER_TABLE, true)
            .map_err(|error| error.to_string())?;
        db::with_savepoint(conn, || {
            db::execute_single_statement(conn, &definition)?;
            let mut columns: Vec<_> = draft
                .iter()
                .zip(sources)
                .filter(|(_, source)| source.is_some())
                .map(|(column, _)| db::quote_ident(&column.name))
                .collect();
            let mut from = columns.clone();
            let new_primary = integer_primary_key(conn, schema, &temporary)?;
            let old_primary = integer_primary_key(conn, schema, table)?;
            if new_primary.is_some()
                && new_primary != old_primary
                && prior_violations
                    .iter()
                    .any(|violation| violation.0.eq_ignore_ascii_case(table))
            {
                return Err(
                    "Resolve existing foreign key violations before changing the rowid key".into(),
                );
            }
            if new_primary.is_none() && !old_without_rowid {
                let source = rowid_alias(conn, schema, table)?
                    .or(old_primary.clone())
                    .ok_or(
                        "All original rowid aliases are shadowed; provide an explicit migration",
                    )?;
                let target = rowid_alias(conn, schema, &temporary)?
                    .ok_or("All new rowid aliases are shadowed; provide an explicit migration")?;
                columns.insert(0, db::quote_ident(&target));
                from.insert(0, db::quote_ident(&source));
            }
            if let Some(primary) = new_primary {
                let retained = draft
                    .iter()
                    .zip(sources)
                    .any(|(column, source)| column.name == primary && source.is_some());
                if !retained && !old_without_rowid {
                    let source = rowid_alias(conn, schema, table)?
                        .or(old_primary.clone())
                        .ok_or(
                        "All original rowid aliases are shadowed; provide an explicit migration",
                    )?;
                    columns.insert(0, db::quote_ident(&primary));
                    from.insert(0, db::quote_ident(&source));
                } else if retained && old_primary.as_ref() != Some(&primary) {
                    let has_null: bool = conn
                        .query_row(
                            &format!(
                                "SELECT EXISTS(SELECT 1 FROM {} WHERE {} IS NULL)",
                                db::qualified_name(schema, table),
                                db::quote_ident(&primary)
                            ),
                            [],
                            |row| row.get(0),
                        )
                        .map_err(|error| error.to_string())?;
                    if has_null {
                        return Err(
                            "Fill NULL primary keys before converting to an INTEGER rowid key"
                                .into(),
                        );
                    }
                }
            }
            if columns.is_empty() {
                return Err("A rebuild must retain a source column or rowid".into());
            }
            let before: i64 = conn
                .query_row(
                    &format!("SELECT count(*) FROM {}", db::qualified_name(schema, table)),
                    [],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            db::execute_single_statement(
                conn,
                &format!(
                    "INSERT OR ABORT INTO {} ({}) SELECT {} FROM {}",
                    db::qualified_name(schema, &temporary),
                    columns.join(","),
                    from.join(","),
                    db::qualified_name(schema, table)
                ),
            )?;
            let after: i64 = conn
                .query_row(
                    &format!(
                        "SELECT count(*) FROM {}",
                        db::qualified_name(schema, &temporary)
                    ),
                    [],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            if before != after {
                return Err("Table rebuild changed the row count".into());
            }
            db::execute_single_statement(
                conn,
                &format!("DROP TABLE {}", db::qualified_name(schema, table)),
            )?;
            db::execute_single_statement(
                conn,
                &format!(
                    "ALTER TABLE {} RENAME TO {}",
                    db::qualified_name(schema, &temporary),
                    db::quote_ident(table)
                ),
            )?;
            for dependency in &dependencies {
                db::execute_single_statement(conn, dependency)?;
            }
            if let Some(sequence) = old_sequence {
                conn.execute(
                    &format!(
                        "UPDATE {}.sqlite_sequence SET seq=max(seq,?2) WHERE name=?1",
                        db::quote_ident(schema)
                    ),
                    rusqlite::params![table, sequence],
                )
                .map_err(|error| error.to_string())?;
                conn.execute(&format!("INSERT INTO {}.sqlite_sequence(name,seq) SELECT ?1,?2 WHERE NOT EXISTS(SELECT 1 FROM {}.sqlite_sequence WHERE name=?1)", db::quote_ident(schema), db::quote_ident(schema)), rusqlite::params![table, sequence]).map_err(|error| error.to_string())?;
            }
            for view in &views {
                conn.prepare(view).map_err(|error| error.to_string())?;
            }
            if foreign_key_violations(conn, schema)?
                .difference(&prior_violations)
                .next()
                .is_some()
            {
                return Err("Table rebuild introduced a foreign key violation".into());
            }
            Ok(())
        })
    })();
    let mut restoration = Vec::new();
    if let Err(error) =
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_LEGACY_ALTER_TABLE, legacy_alter)
    {
        restoration.push(error.to_string());
    }
    if let Err(error) = conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY, foreign_keys) {
        restoration.push(error.to_string());
    }
    if !restoration.is_empty() {
        return Err(format!(
            "{}; restoring SQLite configuration failed: {}",
            result.err().unwrap_or_else(|| "Rebuild applied".into()),
            restoration.join("; ")
        ));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_key_membership_changes_preserve_rows_and_other_constraints() {
        for schema in ["main", "temp", "aux.\"db"] {
            let conn = Connection::open_in_memory().unwrap();
            conn.execute_batch("ATTACH ':memory:' AS \"aux.\"\"db\";")
                .unwrap();
            let table = db::qualified_name(schema, "t");
            conn.execute_batch(&format!("CREATE TABLE {table}(a TEXT,b INTEGER,c TEXT UNIQUE CHECK(c!='bad')); INSERT INTO {table}(rowid,a,b,c) VALUES(7,'one',1,'first'),(9,'two',2,'second'); BEGIN;")).unwrap();
            let mut draft = db::table_column_defs_in(&conn, schema, "t").unwrap();
            let sources = [Some("a".into()), Some("b".into()), Some("c".into())];
            draft[0].pk = true;
            draft[1].pk = true;
            apply(&conn, schema, "t", &draft, &sources).unwrap();
            assert_eq!(
                count(
                    &conn,
                    &format!("SELECT count(*) FROM {table} WHERE rowid IN(7,9)")
                ),
                2
            );
            assert!(conn
                .execute(
                    &format!("INSERT OR ABORT INTO {table} VALUES('one',1,'third')"),
                    []
                )
                .is_err());
            draft[0].pk = false;
            apply(&conn, schema, "t", &draft, &sources).unwrap();
            assert_eq!(
                count(
                    &conn,
                    &format!("SELECT count(*) FROM {table} WHERE rowid IN(1,2)")
                ),
                2
            );
            draft[1].pk = false;
            apply(&conn, schema, "t", &draft, &sources).unwrap();
            conn.execute(&format!("INSERT INTO {table} VALUES('one',1,'third')"), [])
                .unwrap();
            assert!(conn
                .execute(&format!("INSERT INTO {table} VALUES('other',3,'bad')"), [])
                .is_err());
            assert!(conn
                .execute(
                    &format!("INSERT INTO {table} VALUES('other',3,'first')"),
                    []
                )
                .is_err());
            assert!(!conn.is_autocommit());
            conn.execute_batch("ROLLBACK").unwrap();
            assert!(!db::table_columns_in(&conn, schema, "t")
                .unwrap()
                .iter()
                .any(|column| column.pk));
            assert_eq!(
                count(
                    &conn,
                    &format!("SELECT count(*) FROM {table} WHERE rowid IN(7,9)")
                ),
                2
            );
        }
    }

    #[test]
    fn composite_primary_key_edits_keep_names_order_conflicts_and_implicit_not_null() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(a TEXT,b INTEGER,c TEXT,CONSTRAINT named_pk PRIMARY KEY(b DESC,a COLLATE NOCASE) ON CONFLICT FAIL) WITHOUT ROWID, STRICT;
            INSERT INTO t VALUES('kept',1,'new key'); BEGIN;").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
        let sources = [Some("a".into()), Some("b".into()), Some("c".into())];
        draft[0].pk = false;
        draft[2].pk = true;
        apply(&conn, "main", "t", &draft, &sources).unwrap();
        let sql: String = conn
            .query_row("SELECT sql FROM sqlite_schema WHERE name='t'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(sql.contains("CONSTRAINT named_pk"), "{sql}");
        assert!(sql.contains("ON CONFLICT FAIL"), "{sql}");
        assert!(sql.contains("b DESC"), "{sql}");
        let order: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('t') WHERE pk>0 ORDER BY pk")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(order, ["b", "c"]);
        assert!(db::table_columns(&conn, "t").unwrap()[0].notnull);
        assert!(conn
            .execute("INSERT INTO t VALUES(NULL,2,'another')", [])
            .is_err());
        draft[1].pk = false;
        draft[2].pk = false;
        assert!(apply(&conn, "main", "t", &draft, &sources).is_err());
        assert_eq!(count(&conn, "SELECT count(*) FROM t"), 1);
        conn.execute_batch("ROLLBACK").unwrap();
    }

    #[test]
    fn primary_key_edits_preserve_new_column_rowids_and_remove_obsolete_sequences() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(id INTEGER PRIMARY KEY AUTOINCREMENT,v TEXT UNIQUE); INSERT INTO t VALUES(1,'kept'),(100,'removed'); DELETE FROM t WHERE id=100; BEGIN;").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
        draft[0].pk = false;
        draft.push(ColDef {
            pk: true,
            ..ColDef::new("new_id", "INTEGER")
        });
        apply(
            &conn,
            "main",
            "t",
            &draft,
            &[Some("id".into()), Some("v".into()), None],
        )
        .unwrap();
        assert_eq!(
            count(
                &conn,
                "SELECT count(*) FROM t WHERE rowid=1 AND id=1 AND new_id=1 AND v='kept'"
            ),
            1
        );
        assert_eq!(
            count(&conn, "SELECT count(*) FROM sqlite_sequence WHERE name='t'"),
            0
        );
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(
            count(&conn, "SELECT seq FROM sqlite_sequence WHERE name='t'"),
            100
        );
        conn.execute_batch("CREATE TABLE q(\"quoted id\" INTEGER,PRIMARY KEY(\"quoted id\" AUTOINCREMENT)); INSERT INTO q VALUES(1),(100); DELETE FROM q WHERE \"quoted id\"=100; BEGIN;").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "q").unwrap();
        draft[0].default_value = "1".into();
        apply(&conn, "main", "q", &draft, &[Some("quoted id".into())]).unwrap();
        conn.execute("INSERT INTO q DEFAULT VALUES", []).unwrap();
        assert_eq!(count(&conn, "SELECT max(\"quoted id\") FROM q"), 101);
        conn.execute_batch("ROLLBACK").unwrap();
    }

    #[test]
    fn invalid_primary_key_changes_leave_data_and_pending_edits_intact() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE t(id INTEGER PRIMARY KEY,a INTEGER,b TEXT); INSERT INTO t VALUES(7,NULL,'duplicate'),(9,2,'duplicate'); BEGIN; INSERT INTO t VALUES(11,3,'pending');").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
        let sources = [Some("id".into()), Some("a".into()), Some("b".into())];
        draft[0].pk = false;
        draft[1].pk = true;
        assert!(apply(&conn, "main", "t", &draft, &sources)
            .unwrap_err()
            .contains("NULL primary keys"));
        draft[1].pk = false;
        draft[2].pk = true;
        assert!(apply(&conn, "main", "t", &draft, &sources).is_err());
        assert_eq!(
            count(&conn, "SELECT count(*) FROM t WHERE rowid IN(7,9,11)"),
            3
        );
        assert!(db::table_columns(&conn, "t").unwrap()[0].pk);
        assert!(!conn.is_autocommit());
        assert!(conn
            .db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY)
            .unwrap());
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(count(&conn, "SELECT count(*) FROM t"), 2);
        conn.execute_batch("CREATE TABLE parent(k PRIMARY KEY); CREATE TABLE child(id INTEGER PRIMARY KEY,new_id INTEGER,p REFERENCES parent(k) DEFERRABLE INITIALLY DEFERRED); BEGIN; INSERT INTO child VALUES(7,9,999);").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "child").unwrap();
        draft[0].pk = false;
        draft[1].pk = true;
        assert!(apply(
            &conn,
            "main",
            "child",
            &draft,
            &[Some("id".into()), Some("new_id".into()), Some("p".into())]
        )
        .unwrap_err()
        .contains("rowid key"));
        assert_eq!(count(&conn, "SELECT count(*) FROM child WHERE rowid=7"), 1);
        assert!(conn.execute_batch("COMMIT").is_err());
        conn.execute_batch("ROLLBACK").unwrap();
    }

    #[test]
    fn table_rename_updates_dependencies_even_when_the_session_uses_legacy_alter() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=OFF; PRAGMA legacy_alter_table=ON;
            CREATE TABLE parent(id INTEGER PRIMARY KEY,v TEXT); CREATE TABLE child(p REFERENCES parent(id));
            CREATE VIEW viewed AS SELECT v FROM parent; INSERT INTO parent VALUES(1,'kept'); INSERT INTO child VALUES(1); BEGIN;").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "parent").unwrap();
        draft[1].type_name = "BLOB".into();
        db::recreate_table_columns_in(
            &conn,
            "main",
            "parent",
            "renamed",
            &draft,
            &[Some("id".into()), Some("v".into())],
        )
        .unwrap();
        assert_eq!(count(&conn, "SELECT count(*) FROM viewed"), 1);
        assert!(foreign_key_violations(&conn, "main").unwrap().is_empty());
        assert!(conn
            .db_config(DbConfig::SQLITE_DBCONFIG_LEGACY_ALTER_TABLE)
            .unwrap());
        assert!(!conn
            .db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY)
            .unwrap());
        assert!(!conn.is_autocommit());
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(count(&conn, "SELECT count(*) FROM parent"), 1);
        let error = db::recreate_table_columns_in(
            &conn,
            "main",
            "parent",
            "child",
            &draft,
            &[Some("id".into()), Some("v".into())],
        )
        .unwrap_err();
        assert!(error.contains("already another table"), "{error}");
        assert!(conn
            .db_config(DbConfig::SQLITE_DBCONFIG_LEGACY_ALTER_TABLE)
            .unwrap());
        assert!(!conn
            .db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY)
            .unwrap());
        assert_eq!(
            db::table_columns(&conn, "parent").unwrap()[1].type_name,
            "TEXT"
        );
        assert_eq!(count(&conn, "SELECT count(*) FROM viewed"), 1);
    }

    #[test]
    fn existing_without_rowid_violations_cannot_hide_new_invalid_foreign_keys() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=OFF; CREATE TABLE parent(v INTEGER UNIQUE); INSERT INTO parent VALUES(1);
            CREATE TABLE child(k TEXT PRIMARY KEY,p TEXT REFERENCES parent(v)) WITHOUT ROWID;
            INSERT INTO child VALUES('already bad','999'),('would become bad','01'); BEGIN;").unwrap();
        conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY, true)
            .unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "parent").unwrap();
        draft[0].type_name = "TEXT".into();
        assert!(apply(&conn, "main", "parent", &draft, &[Some("v".into())]).is_err());
        assert_eq!(
            db::table_columns(&conn, "parent").unwrap()[0].type_name,
            "INTEGER"
        );
        assert_eq!(
            count(&conn, "SELECT count(*) FROM pragma_foreign_key_check"),
            1
        );
        assert!(conn
            .db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY)
            .unwrap());
        assert!(!conn.is_autocommit());
        conn.execute_batch("DELETE FROM child WHERE k='already bad';
            CREATE TABLE moved(id TEXT PRIMARY KEY,p REFERENCES parent(v) DEFERRABLE INITIALLY DEFERRED);
            INSERT INTO moved(rowid,id,p) VALUES(7,'9',999);").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "moved").unwrap();
        draft[0].type_name = "INTEGER".into();
        assert!(apply(
            &conn,
            "main",
            "moved",
            &draft,
            &[Some("id".into()), Some("p".into())]
        )
        .unwrap_err()
        .contains("rowid key"));
        assert_eq!(
            count(&conn, "SELECT count(*) FROM moved WHERE rowid=7 AND id='9'"),
            1
        );
        assert!(conn.execute_batch("COMMIT").is_err());
        conn.execute_batch("ROLLBACK").unwrap();
    }

    fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    fn apply(
        conn: &Connection,
        schema: &str,
        table: &str,
        draft: &[ColDef],
        sources: &[Option<String>],
    ) -> Result<(), String> {
        db::recreate_table_columns_in(conn, schema, table, table, draft, sources)
    }

    #[test]
    fn rebuild_preserves_qualified_dependencies_generated_values_sequence_and_pending_edits() {
        for schema in ["main", "temp", "aux.\"db"] {
            let conn = Connection::open_in_memory().unwrap();
            let parent = db::qualified_name(schema, "parent");
            conn.execute_batch(&format!("PRAGMA foreign_keys=ON; ATTACH ':memory:' AS \"aux.\"\"db\";
                CREATE TABLE {}(sentinel); INSERT INTO {} VALUES('kept');
                CREATE TABLE {parent}(id INTEGER PRIMARY KEY AUTOINCREMENT,
                    v TEXT CONSTRAINT uq UNIQUE ON CONFLICT IGNORE COLLATE NOCASE CONSTRAINT ck CHECK(v!='bad') CONSTRAINT dv DEFAULT 'fallback',
                    g TEXT GENERATED ALWAYS AS(v || '!') STORED);
                CREATE TABLE {}(p REFERENCES parent(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED);
                CREATE TABLE {}(v);
                CREATE UNIQUE INDEX {} ON parent(lower(v)) WHERE v IS NOT NULL;
                CREATE VIEW {} AS SELECT id,v,g FROM parent;
                CREATE TRIGGER {} AFTER INSERT ON {parent} BEGIN INSERT INTO child VALUES(new.id); INSERT INTO audit VALUES(new.v); END;
                INSERT INTO {parent}(id,v) VALUES(1,'one'),(100,'removed'); DELETE FROM {parent} WHERE id=100;
                BEGIN; INSERT INTO {parent}(id,v) VALUES(2,'pending');",
                db::qualified_name(schema,"__next_gui_rebuild_0"), db::qualified_name(schema,"__next_gui_rebuild_0"),
                db::qualified_name(schema,"child"),db::qualified_name(schema,"audit"),db::qualified_name(schema,"ix"),
                db::qualified_name(schema,"viewed"),db::qualified_name(schema,"tr"))).unwrap();
            if schema != "temp" {
                conn.execute_batch(&format!("CREATE TEMP TABLE extra_audit(v); CREATE TRIGGER temp.extra AFTER INSERT ON {parent} BEGIN INSERT INTO extra_audit VALUES(new.v); END;")).unwrap();
            }
            let original = db::table_column_defs_in(&conn, schema, "parent").unwrap();
            assert!(original[1].unique);
            let mut draft = vec![
                ColDef {
                    default_value: "7".into(),
                    ..ColDef::new("first", "INTEGER")
                },
                original[1].clone(),
                original[0].clone(),
            ];
            draft[1].type_name = "BLOB".into();
            draft[1].notnull = true;
            draft[1].default_value = "'new default'".into();
            let sources = [None, Some("v".into()), Some("id".into())];
            apply(&conn, schema, "parent", &draft, &sources)
                .unwrap_or_else(|error| panic!("{schema}: {error}"));
            assert_eq!(
                db::table_columns_in(&conn, schema, "parent")
                    .unwrap()
                    .iter()
                    .map(|col| col.name.as_str())
                    .collect::<Vec<_>>(),
                ["first", "v", "id"]
            );
            let row: (i64, String, String, i64) = conn
                .query_row(
                    &format!("SELECT first,v,g,id FROM {parent} WHERE id=2"),
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .unwrap();
            assert_eq!(row, (7, "pending".into(), "pending!".into(), 2));
            let sql: String = conn
                .query_row(
                    &format!(
                        "SELECT sql FROM {}.sqlite_schema WHERE name='parent'",
                        db::quote_ident(schema)
                    ),
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(sql.contains("CONSTRAINT dv"));
            assert_eq!(
                db::count_browse_rows_in(&conn, schema, "child", None).unwrap(),
                2
            );
            assert_eq!(
                db::count_browse_rows_in(&conn, schema, "audit", None).unwrap(),
                3
            );
            assert_eq!(
                db::count_browse_rows_in(&conn, schema, "viewed", None).unwrap(),
                2
            );
            assert_eq!(sequence(&conn, schema, "parent").unwrap(), Some(100));
            assert_eq!(
                db::count_browse_rows_in(&conn, schema, "__next_gui_rebuild_0", None).unwrap(),
                1
            );
            assert!(conn
                .execute(&format!("INSERT INTO {parent}(v) VALUES('bad')"), [])
                .is_err());
            assert!(conn
                .execute(&format!("INSERT INTO {parent}(v) VALUES(NULL)"), [])
                .is_err());
            assert!(conn
                .execute(
                    &format!("INSERT OR ABORT INTO {parent}(v) VALUES('ONE')"),
                    []
                )
                .is_err());
            conn.execute(&format!("INSERT INTO {parent} DEFAULT VALUES"), [])
                .unwrap();
            assert_eq!(conn.last_insert_rowid(), 101);
            assert_eq!(
                db::count_browse_rows_in(&conn, schema, "child", None).unwrap(),
                3
            );
            assert_eq!(
                db::count_browse_rows_in(&conn, schema, "audit", None).unwrap(),
                4
            );
            if schema != "temp" {
                assert_eq!(count(&conn, "SELECT count(*) FROM temp.extra_audit"), 1);
            }
            assert!(conn
                .db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY)
                .unwrap());
            assert!(!conn
                .db_config(DbConfig::SQLITE_DBCONFIG_LEGACY_ALTER_TABLE)
                .unwrap());
            assert!(foreign_key_violations(&conn, schema).unwrap().is_empty());
            assert!(!conn.is_autocommit());
            conn.execute_batch("ROLLBACK").unwrap();
            assert_eq!(
                db::table_column_names_in(&conn, schema, "parent").unwrap(),
                ["id", "v", "g"]
            );
            assert_eq!(
                db::count_browse_rows_in(&conn, schema, "parent", None).unwrap(),
                1
            );
            assert_eq!(
                db::count_browse_rows_in(&conn, schema, "child", None).unwrap(),
                1
            );
        }
    }

    #[test]
    fn rebuild_never_silently_ignores_conflicts_and_restores_connection_flags_on_failure() {
        for foreign_keys in [false, true] {
            for legacy_alter in [false, true] {
                let conn = Connection::open_in_memory().unwrap();
                conn.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY, foreign_keys)
                    .unwrap();
                conn.set_db_config(DbConfig::SQLITE_DBCONFIG_LEGACY_ALTER_TABLE, legacy_alter)
                    .unwrap();
                conn.execute_batch("CREATE TABLE t(v TEXT CONSTRAINT uq UNIQUE ON CONFLICT IGNORE); INSERT INTO t(rowid,v) VALUES(7,'1'),(9,'1.0'); BEGIN; INSERT INTO t(rowid,v) VALUES(12,'pending');").unwrap();
                let mut draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
                draft[0].type_name = "INTEGER".into();
                let before = db::list_schema(&conn)
                    .unwrap()
                    .into_iter()
                    .map(|item| (item.name, item.sql))
                    .collect::<Vec<_>>();
                assert!(apply(&conn, "main", "t", &draft, &[Some("v".into())]).is_err());
                assert_eq!(count(&conn, "SELECT count(*) FROM t"), 3);
                assert_eq!(
                    count(&conn, "SELECT count(*) FROM t WHERE rowid IN (7,9,12)"),
                    3
                );
                assert_eq!(
                    db::list_schema(&conn)
                        .unwrap()
                        .into_iter()
                        .map(|item| (item.name, item.sql))
                        .collect::<Vec<_>>(),
                    before
                );
                assert_eq!(
                    conn.db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY)
                        .unwrap(),
                    foreign_keys
                );
                assert_eq!(
                    conn.db_config(DbConfig::SQLITE_DBCONFIG_LEGACY_ALTER_TABLE)
                        .unwrap(),
                    legacy_alter
                );
                assert!(!conn.is_autocommit());
                draft[0].type_name = "TEXT".into();
                draft[0].default_value = "'after'".into();
                apply(&conn, "main", "t", &draft, &[Some("v".into())]).unwrap();
                assert_eq!(
                    count(&conn, "SELECT count(*) FROM t WHERE rowid IN (7,9,12)"),
                    3
                );
                assert_eq!(
                    conn.db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY)
                        .unwrap(),
                    foreign_keys
                );
                assert_eq!(
                    conn.db_config(DbConfig::SQLITE_DBCONFIG_LEGACY_ALTER_TABLE)
                        .unwrap(),
                    legacy_alter
                );
                conn.execute_batch("ROLLBACK").unwrap();
                assert_eq!(count(&conn, "SELECT count(*) FROM t"), 2);
            }
        }
    }

    #[test]
    fn rebuild_checks_new_foreign_key_violations_but_preserves_pending_deferred_violations() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE t(v INTEGER UNIQUE); INSERT INTO t VALUES(1);
            CREATE TABLE child(p TEXT REFERENCES t(v) DEFERRABLE INITIALLY DEFERRED); INSERT INTO child VALUES('01'); BEGIN;").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
        draft[0].type_name = "TEXT".into();
        assert!(apply(&conn, "main", "t", &draft, &[Some("v".into())])
            .unwrap_err()
            .contains("foreign key"));
        assert_eq!(
            db::table_columns(&conn, "t").unwrap()[0].type_name,
            "INTEGER"
        );
        assert!(foreign_key_violations(&conn, "main").unwrap().is_empty());
        conn.execute_batch("INSERT INTO child VALUES('999')")
            .unwrap();
        draft[0].type_name = "INTEGER".into();
        draft[0].default_value = "7".into();
        apply(&conn, "main", "t", &draft, &[Some("v".into())]).unwrap();
        assert_eq!(foreign_key_violations(&conn, "main").unwrap().len(), 1);
        assert!(conn.execute_batch("COMMIT").is_err());
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(count(&conn, "SELECT count(*) FROM child"), 1);
    }

    #[test]
    fn rebuild_preserves_strict_without_rowid_composite_constraints_and_removes_single_unique_only()
    {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(a TEXT, b INTEGER, CONSTRAINT pk PRIMARY KEY(a DESC,b) ON CONFLICT FAIL,
            CONSTRAINT uq UNIQUE(b) ON CONFLICT IGNORE, CONSTRAINT ck CHECK(b>0)) WITHOUT ROWID, STRICT;
            INSERT INTO t VALUES('kept',2); BEGIN;").unwrap();
        let original = db::table_column_defs_in(&conn, "main", "t").unwrap();
        assert!(original[1].unique);
        let mut draft = vec![original[1].clone(), original[0].clone()];
        draft[0].type_name = "ANY".into();
        draft[0].unique = false;
        apply(
            &conn,
            "main",
            "t",
            &draft,
            &[Some("b".into()), Some("a".into())],
        )
        .unwrap();
        assert!(without_rowid(&conn, "main", "t").unwrap());
        let sql: String = conn
            .query_row("SELECT sql FROM sqlite_schema WHERE name='t'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(sql.contains("STRICT"));
        assert!(sql.contains("CONSTRAINT pk"));
        assert!(sql.contains("CONSTRAINT ck"));
        assert!(!sql.contains("CONSTRAINT uq"));
        conn.execute("INSERT INTO t(a,b) VALUES('second',2)", [])
            .unwrap();
        assert!(conn
            .execute("INSERT INTO t(a,b) VALUES('bad',0)", [])
            .is_err());
        assert!(conn
            .execute("INSERT OR ABORT INTO t(a,b) VALUES('kept',2)", [])
            .is_err());
        assert!(conn
            .execute("INSERT INTO t(a,b) VALUES(zeroblob(3),3)", [])
            .is_err());
        assert_eq!(count(&conn, "SELECT count(*) FROM t"), 2);
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(db::table_column_names(&conn, "t").unwrap(), ["a", "b"]);
    }

    #[test]
    fn rebuild_rejects_unavailable_rowids_and_nulls_in_new_integer_primary_keys() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE shadow(rowid,_rowid_,oid,v); INSERT INTO shadow VALUES('r','u','o','kept');
            CREATE TABLE t(id TEXT PRIMARY KEY,v); INSERT INTO t VALUES(NULL,'kept'); BEGIN;").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "shadow").unwrap();
        draft[3].default_value = "7".into();
        let sources: Vec<_> = draft.iter().map(|col| Some(col.name.clone())).collect();
        assert!(apply(&conn, "main", "shadow", &draft, &sources)
            .unwrap_err()
            .contains("rowid aliases"));
        assert_eq!(count(&conn, "SELECT count(*) FROM shadow"), 1);
        let mut draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
        draft[0].type_name = "INTEGER".into();
        assert!(apply(
            &conn,
            "main",
            "t",
            &draft,
            &[Some("id".into()), Some("v".into())]
        )
        .unwrap_err()
        .contains("NULL primary keys"));
        assert_eq!(count(&conn, "SELECT count(*) FROM t WHERE id IS NULL"), 1);
        assert_eq!(db::table_columns(&conn, "t").unwrap()[0].type_name, "TEXT");
        conn.execute_batch("CREATE TABLE descending(id INTEGER PRIMARY KEY DESC,v); INSERT INTO descending(rowid,id,v) VALUES(7,101,'kept'),(11,202,'second');").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "descending").unwrap();
        draft[1].default_value = "'new'".into();
        apply(
            &conn,
            "main",
            "descending",
            &draft,
            &[Some("id".into()), Some("v".into())],
        )
        .unwrap();
        assert_eq!(count(&conn,"SELECT count(*) FROM descending WHERE (rowid=7 AND id=101) OR (rowid=11 AND id=202)"),2);
        conn.execute_batch("CREATE TABLE quoted(v \"TEXT UNIQUE\",w \"INTEGER NOT NULL\",n); INSERT INTO quoted(rowid,v,w) VALUES(9,'kept',7);").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "quoted").unwrap();
        draft[2].default_value = "7".into();
        let sources: Vec<_> = draft
            .iter()
            .map(|column| Some(column.name.clone()))
            .collect();
        draft[0].name = "renamed".into();
        apply(&conn, "main", "quoted", &draft, &sources).unwrap();
        assert_eq!(
            db::table_columns(&conn, "quoted").unwrap()[0].type_name,
            "TEXT UNIQUE"
        );
        assert_eq!(
            db::table_columns(&conn, "quoted").unwrap()[1].type_name,
            "INTEGER NOT NULL"
        );
        assert_eq!(
            count(
                &conn,
                "SELECT count(*) FROM quoted WHERE rowid=9 AND renamed='kept' AND w=7"
            ),
            1
        );
    }
}
