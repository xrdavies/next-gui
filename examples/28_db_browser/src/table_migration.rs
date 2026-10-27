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
    options: Option<table_sql::TableOptions>,
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
        table_sql::rebuild_definition(&sql, schema, &temporary, original, draft, sources, options)?;
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
                .filter(|(column, source)| source.is_some() && column.generated.is_none())
                .map(|(column, _)| db::quote_ident(&column.name))
                .collect();
            let mut from = columns.clone();
            let mut copy_order = String::new();
            let new_primary = integer_primary_key(conn, schema, &temporary)?;
            let new_without_rowid = without_rowid(conn, schema, &temporary)?;
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
            if new_primary.is_none() && !old_without_rowid && !new_without_rowid {
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
                    if draft
                        .iter()
                        .any(|column| column.name == primary && column.auto_increment)
                    {
                        // Existing IDs must precede NULLs so automatic allocation cannot collide with them.
                        copy_order = format!(" ORDER BY {} IS NULL", db::quote_ident(&primary));
                    } else {
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
                    "INSERT OR ABORT INTO {} ({}) SELECT {} FROM {}{}",
                    db::qualified_name(schema, &temporary),
                    columns.join(","),
                    from.join(","),
                    db::qualified_name(schema, table),
                    copy_order
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
    let result = result
        .map_err(|error: String| error.replace(&temporary, &db::qualified_name(schema, table)));
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
    fn named_table_checks_edit_clear_and_create_without_losing_other_constraints_or_pending_data() {
        for schema in ["main","temp","aux.\"db"] {
            let conn=Connection::open_in_memory().unwrap();
            let table=db::qualified_name(schema,"checked");
            let audit=db::qualified_name(schema,"audit");
            conn.execute_batch(&format!("PRAGMA foreign_keys=ON; ATTACH ':memory:' AS \"aux.\"\"db\";
                CREATE TABLE {}(id INTEGER PRIMARY KEY); INSERT INTO {} VALUES(1);
                CREATE TABLE {table}(id INTEGER PRIMARY KEY AUTOINCREMENT,v INTEGER CONSTRAINT inline_check CHECK(v>0),p REFERENCES parent(id),g INTEGER AS(v*2) STORED,
                    CONSTRAINT \"limit.\"\"名\" CHECK(v<20),CHECK(id>0),UNIQUE(v,g));
                CREATE TABLE {audit}(v); CREATE INDEX {} ON checked(g);
                CREATE VIEW {} AS SELECT id,v,g FROM checked;
                CREATE TRIGGER {} AFTER UPDATE OF v ON {table} BEGIN INSERT INTO audit VALUES(new.g); END;
                INSERT INTO {table}(id,v,p) VALUES(7,2,1),(11,4,1),(99,6,1); DELETE FROM {table} WHERE id=99;
                BEGIN; INSERT INTO {audit} VALUES('pending');",db::qualified_name(schema,"parent"),db::qualified_name(schema,"parent"),db::qualified_name(schema,"ix"),db::qualified_name(schema,"viewed"),db::qualified_name(schema,"tr"))).unwrap();
            let sql=|conn:&Connection| conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='checked'",db::quote_ident(schema)),[],|row|row.get::<_,String>(0)).unwrap();
            let original=table_sql::table_checks(&sql(&conn)).unwrap();
            assert_eq!(original[0].name.as_deref(),Some("limit.\"名")); assert!(original[1].name.is_none());
            let draft=db::table_column_defs_in(&conn,schema,"checked").unwrap();
            let sources:Vec<_>=draft.iter().map(|column|Some(column.name.clone())).collect();
            let mut checks=vec![original[1].clone(),table_sql::NamedCheck { name:Some("new.\"约束".into()),expression:"v < 3".into() },table_sql::NamedCheck { name:None,expression:"g = v * 2 -- expression comment".into() }];
            let mut options=db::table_options_in(&conn,schema,"checked").unwrap(); options.checks=Some(checks.clone());
            assert!(db::recreate_table_columns_in(&conn,schema,"checked","checked",&draft,&sources,Some(options.clone())).is_err());
            assert_eq!(table_sql::table_checks(&sql(&conn)).unwrap(),original);
            checks[1].expression="v < 10".into(); options.checks=Some(checks);
            db::recreate_table_columns_in(&conn,schema,"checked","checked",&draft,&sources,Some(options.clone())).unwrap();
            let checks=table_sql::table_checks(&sql(&conn)).unwrap(); assert_eq!(checks.len(),3); assert_eq!(checks[1].name.as_deref(),Some("new.\"约束"));
            assert_eq!(count(&conn,&format!("SELECT count(*) FROM {table} WHERE (rowid=7 AND v=2 AND g=4) OR (rowid=11 AND v=4 AND g=8)")),2);
            assert_eq!(sequence(&conn,schema,"checked").unwrap(),Some(99));
            assert_eq!(count(&conn,&format!("SELECT count(*) FROM {audit}")),1);
            assert!(conn.execute(&format!("UPDATE {table} SET v=11 WHERE id=7"),[]).is_err());
            assert!(conn.execute(&format!("UPDATE {table} SET v=0 WHERE id=7"),[]).is_err());
            assert!(conn.execute(&format!("UPDATE {table} SET p=999 WHERE id=7"),[]).is_err());
            conn.execute(&format!("UPDATE {table} SET v=5 WHERE id=7"),[]).unwrap();
            assert_eq!(count(&conn,&format!("SELECT count(*) FROM {audit}")),2);
            options.checks=Some(Vec::new());
            db::recreate_table_columns_in(&conn,schema,"checked","checked",&draft,&sources,Some(options)).unwrap();
            assert!(table_sql::table_checks(&sql(&conn)).unwrap().is_empty());
            conn.execute(&format!("UPDATE {table} SET v=11 WHERE id=7"),[]).unwrap();
            assert!(conn.execute(&format!("UPDATE {table} SET v=0 WHERE id=7"),[]).is_err());
            assert!(foreign_key_violations(&conn,schema).unwrap().is_empty());
            assert!(conn.db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY).unwrap()); assert!(!conn.is_autocommit());
            conn.execute_batch("ROLLBACK").unwrap(); assert_eq!(table_sql::table_checks(&sql(&conn)).unwrap(),original);
            assert_eq!(count(&conn,&format!("SELECT count(*) FROM {audit}")),0);
            let columns=[ColDef::new("v","INTEGER")];
            let sql=table_sql::apply_table_options(&db::create_table_ddl_in(schema,"created",&columns),table_sql::TableOptions {
                checks:Some(vec![table_sql::NamedCheck { name:Some("positive".into()),expression:"v > 0".into() },table_sql::NamedCheck { name:None,expression:"v < 5".into() }]),..Default::default()
            }).unwrap();
            db::execute_single_statement(&conn,&sql).unwrap();
            let created=db::qualified_name(schema,"created");
            assert!(conn.execute(&format!("INSERT INTO {created} VALUES(0),(8)"),[]).is_err()); conn.execute(&format!("INSERT INTO {created} VALUES(3),(NULL)"),[]).unwrap();
        }
    }

    #[test]
    fn table_check_fields_reject_constraint_and_statement_injection_atomically() {
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE t(v INTEGER CHECK(v>0),CONSTRAINT old CHECK(v<10)); INSERT INTO t VALUES(2); BEGIN; INSERT INTO t VALUES(3);").unwrap();
        let draft=db::table_column_defs_in(&conn,"main","t").unwrap();
        for expression in ["","-- empty","v) , injected INTEGER --","v) , CHECK(1) --","v) , UNIQUE(v) --","v)); PRAGMA foreign_keys=OFF; --","?1","(SELECT v)","missing>0"] {
            let options=table_sql::TableOptions { checks:Some(vec![table_sql::NamedCheck { name:Some("safe.\"名".into()),expression:expression.into() }]),..Default::default() };
            assert!(db::recreate_table_columns_in(&conn,"main","t","t",&draft,&[Some("v".into())],Some(options)).is_err(),"{expression}");
            assert_eq!(count(&conn,"SELECT count(*) FROM t WHERE v IN (2,3)"),2);
            let sql:String=conn.query_row("SELECT sql FROM sqlite_schema WHERE name='t'",[],|row|row.get(0)).unwrap();
            assert_eq!(table_sql::table_checks(&sql).unwrap()[0].name.as_deref(),Some("old"));
            assert_eq!(db::table_column_names(&conn,"t").unwrap(),["v"]);
            assert!(conn.db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY).unwrap()); assert!(!conn.is_autocommit());
        }
    }

    #[test]
    fn primary_key_conflict_actions_match_sqlite_statement_and_transaction_semantics() {
        for (_, action) in table_sql::PRIMARY_KEY_CONFLICTS {
            let conn = Connection::open_in_memory().unwrap();
            conn.execute_batch("CREATE TABLE t(k TEXT PRIMARY KEY,v TEXT); INSERT INTO t VALUES('original','kept'); BEGIN;").unwrap();
            let draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
            let options = table_sql::TableOptions {
                primary_key_conflict: action,
                ..Default::default()
            };
            db::recreate_table_columns_in(
                &conn,
                "main",
                "t",
                "t",
                &draft,
                &[Some("k".into()), Some("v".into())],
                Some(options.clone()),
            )
            .unwrap();
            assert_eq!(db::table_options_in(&conn, "main", "t").unwrap(), options);
            conn.execute_batch("COMMIT; BEGIN; INSERT INTO t VALUES('pending','unwritten');")
                .unwrap();
            let result = conn.execute("INSERT INTO t VALUES('new','before conflict'),('original','replacement'),('after','after conflict')",[]);
            use sqlite3_parser::ast::ResolveType;
            match action {
                Some(ResolveType::Rollback) => {
                    assert!(result.is_err());
                    assert!(conn.is_autocommit());
                    assert_eq!(count(&conn, "SELECT count(*) FROM t"), 1);
                }
                Some(ResolveType::Ignore) => {
                    assert_eq!(result.unwrap(), 2);
                    assert_eq!(count(&conn, "SELECT count(*) FROM t"), 4);
                }
                Some(ResolveType::Replace) => {
                    assert_eq!(result.unwrap(), 3);
                    assert_eq!(count(&conn, "SELECT count(*) FROM t"), 4);
                }
                Some(ResolveType::Fail) => {
                    assert!(result.is_err());
                    assert_eq!(count(&conn, "SELECT count(*) FROM t"), 3);
                }
                None | Some(ResolveType::Abort) => {
                    assert!(result.is_err());
                    assert_eq!(count(&conn, "SELECT count(*) FROM t"), 2);
                }
            }
            assert_eq!(
                conn.query_row("SELECT v FROM t WHERE k='original'", [], |r| r
                    .get::<_, String>(0))
                    .unwrap(),
                if action == Some(ResolveType::Replace) {
                    "replacement"
                } else {
                    "kept"
                }
            );
            if !conn.is_autocommit() {
                conn.execute_batch("ROLLBACK").unwrap();
            }
            assert_eq!(count(&conn, "SELECT count(*) FROM t"), 1);
        }
    }

    #[test]
    fn primary_key_conflict_edits_keep_names_key_details_and_other_policies_across_schemas() {
        for schema in ["main", "temp", "aux.\"db"] {
            let conn = Connection::open_in_memory().unwrap();
            conn.execute_batch("ATTACH ':memory:' AS \"aux.\"\"db\";")
                .unwrap();
            let table = db::qualified_name(schema, "t");
            conn.execute_batch(&format!("CREATE TABLE {table}(a TEXT,b INTEGER,v TEXT CONSTRAINT other_unique UNIQUE ON CONFLICT IGNORE,CONSTRAINT named_key PRIMARY KEY(a COLLATE NOCASE DESC,b ASC) ON CONFLICT FAIL); INSERT INTO {table}(rowid,a,b,v) VALUES(7,'Alpha',1,'kept'); BEGIN;")).unwrap();
            let draft = db::table_column_defs_in(&conn, schema, "t").unwrap();
            let sources = [Some("a".into()), Some("b".into()), Some("v".into())];
            let mut options = db::table_options_in(&conn, schema, "t").unwrap();
            assert_eq!(
                options.primary_key_conflict,
                Some(sqlite3_parser::ast::ResolveType::Fail)
            );
            for action in [
                Some(sqlite3_parser::ast::ResolveType::Ignore),
                Some(sqlite3_parser::ast::ResolveType::Replace),
                None,
            ] {
                options.primary_key_conflict = action;
                db::recreate_table_columns_in(
                    &conn,
                    schema,
                    "t",
                    "t",
                    &draft,
                    &sources,
                    Some(options.clone()),
                )
                .unwrap();
                assert_eq!(db::table_options_in(&conn, schema, "t").unwrap(), options);
                assert_eq!(count(&conn, &format!("SELECT rowid FROM {table}")), 7);
                assert_eq!(
                    conn.execute(&format!("INSERT INTO {table} VALUES('Other',2,'kept')"), [])
                        .unwrap(),
                    0,
                    "the unrelated unique policy remains IGNORE"
                );
                let sql: String = conn
                    .query_row(
                        &format!(
                            "SELECT sql FROM {}.sqlite_schema WHERE name='t'",
                            db::quote_ident(schema)
                        ),
                        [],
                        |r| r.get(0),
                    )
                    .unwrap();
                for fragment in ["named_key", "other_unique", "COLLATE NOCASE DESC", "b ASC"] {
                    assert!(sql.contains(fragment), "{sql}");
                }
            }
            assert!(
                conn.execute(
                    &format!("INSERT INTO {table} VALUES('alpha',1,'different')"),
                    []
                )
                .is_err(),
                "key collation and default ABORT remain effective"
            );
            conn.execute_batch("ROLLBACK").unwrap();
            assert_eq!(
                db::table_options_in(&conn, schema, "t")
                    .unwrap()
                    .primary_key_conflict,
                Some(sqlite3_parser::ast::ResolveType::Fail)
            );
        }
    }

    #[test]
    fn duplicate_copy_aborts_even_for_rollback_ignore_or_replace_primary_keys() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(id INTEGER PRIMARY KEY,desired TEXT); INSERT INTO t VALUES(1,'duplicate'),(2,'duplicate'); BEGIN; INSERT INTO t VALUES(3,'pending');").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
        draft[0].pk = false;
        draft[1].pk = true;
        for action in [
            sqlite3_parser::ast::ResolveType::Rollback,
            sqlite3_parser::ast::ResolveType::Ignore,
            sqlite3_parser::ast::ResolveType::Replace,
        ] {
            let options = table_sql::TableOptions {
                primary_key_conflict: Some(action),
                ..Default::default()
            };
            assert!(db::recreate_table_columns_in(
                &conn,
                "main",
                "t",
                "t",
                &draft,
                &[Some("id".into()), Some("desired".into())],
                Some(options.clone())
            )
            .is_err());
            assert!(
                !conn.is_autocommit(),
                "copy must not roll back the user's transaction"
            );
            assert_eq!(
                count(&conn, "SELECT count(*) FROM t WHERE id IN(1,2,3)"),
                3,
                "copy must not discard or replace source rows"
            );
            assert_eq!(
                db::table_options_in(&conn, "main", "t")
                    .unwrap()
                    .primary_key_conflict,
                None
            );
        }
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(count(&conn, "SELECT count(*) FROM t"), 2);
    }

    #[test]
    fn table_options_preserve_data_keys_dependencies_and_pending_rollback() {
        for schema in ["main", "temp", "aux.\"db"] {
            let conn = Connection::open_in_memory().unwrap();
            conn.execute_batch("PRAGMA foreign_keys=ON; ATTACH ':memory:' AS \"aux.\"\"db\";")
                .unwrap();
            let table = db::qualified_name(schema, "t");
            conn.execute_batch(&format!("CREATE TABLE {table}(a TEXT,b INTEGER,v TEXT CHECK(length(v)>0),CONSTRAINT named_key PRIMARY KEY(a,b),CONSTRAINT named_unique UNIQUE(v)); INSERT INTO {table}(rowid,a,b,v) VALUES(7,'Alpha',1,'one'),(9,'Beta',2,'two');
                CREATE TABLE {}(a,b,FOREIGN KEY(a,b) REFERENCES t(a,b)); INSERT INTO {} VALUES('Alpha',1); CREATE INDEX {} ON t(b); CREATE TABLE {}(v); CREATE TRIGGER {} AFTER INSERT ON t BEGIN INSERT INTO audit VALUES(new.v); END; BEGIN;",db::qualified_name(schema,"child"),db::qualified_name(schema,"child"),db::qualified_name(schema,"ix"),db::qualified_name(schema,"audit"),db::qualified_name(schema,"tr"))).unwrap();
            let sources = [Some("a".into()), Some("b".into()), Some("v".into())];
            let mut draft = db::table_column_defs_in(&conn, schema, "t").unwrap();
            let mut options = table_sql::TableOptions {
                strict: true,
                without_rowid: false,
                primary_key_conflict: None,
            checks: None,
            };
            db::recreate_table_columns_in(&conn, schema, "t", "t", &draft, &sources, Some(options.clone()))
                .unwrap();
            assert_eq!(db::table_options_in(&conn, schema, "t").unwrap(), options);
            assert_eq!(
                count(
                    &conn,
                    &format!("SELECT count(*) FROM {table} WHERE rowid IN(7,9)")
                ),
                2
            );
            assert!(conn
                .execute(
                    &format!("INSERT INTO {table} VALUES('Other','invalid integer','valid')"),
                    []
                )
                .is_err());
            draft = db::table_column_defs_in(&conn, schema, "t").unwrap();
            options.without_rowid = true;
            db::recreate_table_columns_in(&conn, schema, "t", "t", &draft, &sources, Some(options.clone()))
                .unwrap();
            assert_eq!(db::table_options_in(&conn, schema, "t").unwrap(), options);
            assert!(conn.prepare(&format!("SELECT rowid FROM {table}")).is_err());
            assert_eq!(count(&conn,&format!("SELECT count(*) FROM {table} WHERE (a='Alpha' AND b=1 AND v='one') OR (a='Beta' AND b=2 AND v='two')")),2);
            assert_eq!(
                count(
                    &conn,
                    &format!(
                        "SELECT count(*) FROM {}",
                        db::qualified_name(schema, "child")
                    )
                ),
                1
            );
            assert_eq!(
                count(
                    &conn,
                    &format!(
                        "SELECT count(*) FROM {}",
                        db::qualified_name(schema, "audit")
                    )
                ),
                0
            );
            assert!(conn
                .execute(
                    &format!("INSERT INTO {table} VALUES('Alpha',1,'different')"),
                    []
                )
                .is_err());
            draft = db::table_column_defs_in(&conn, schema, "t").unwrap();
            assert!(draft[0].notnull && draft[1].notnull);
            options = table_sql::TableOptions::default();
            db::recreate_table_columns_in(&conn, schema, "t", "t", &draft, &sources, Some(options.clone()))
                .unwrap();
            assert_eq!(db::table_options_in(&conn, schema, "t").unwrap(), options);
            assert!(conn.prepare(&format!("SELECT rowid FROM {table}")).is_ok());
            assert!(
                db::table_columns_in(&conn, schema, "t").unwrap()[0].notnull,
                "keep the requested NN when implicit WITHOUT ROWID rules end"
            );
            assert!(conn
                .execute(&format!("INSERT INTO {table} VALUES(NULL,3,'another')"), [])
                .is_err());
            conn.execute(
                &format!("INSERT INTO {table} VALUES('Other',3,'three')"),
                [],
            )
            .unwrap();
            assert_eq!(
                count(
                    &conn,
                    &format!(
                        "SELECT count(*) FROM {}",
                        db::qualified_name(schema, "audit")
                    )
                ),
                1
            );
            assert!(conn
                .execute(&format!("INSERT INTO {table} VALUES('Last',4,'')"), [])
                .is_err());
            let sql: String = conn
                .query_row(
                    &format!(
                        "SELECT sql FROM {}.sqlite_schema WHERE name='t'",
                        db::quote_ident(schema)
                    ),
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(sql.contains("named_key") && sql.contains("named_unique"));
            assert_eq!(count(&conn, "PRAGMA foreign_keys"), 1);
            assert!(!conn.is_autocommit());
            conn.execute_batch("ROLLBACK").unwrap();
            assert_eq!(
                db::table_options_in(&conn, schema, "t").unwrap(),
                table_sql::TableOptions::default()
            );
            assert_eq!(
                count(
                    &conn,
                    &format!("SELECT count(*) FROM {table} WHERE rowid IN(7,9)")
                ),
                2
            );
            assert!(!db::table_columns_in(&conn, schema, "t").unwrap()[0].notnull);
        }
    }

    #[test]
    fn incompatible_table_options_roll_back_types_null_keys_autoincrement_and_pending_violations() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE t(id TEXT PRIMARY KEY,n INTEGER); INSERT INTO t(rowid,id,n) VALUES(7,'first','not an integer'); CREATE TABLE no_key(v INTEGER); CREATE TABLE ai(id INTEGER PRIMARY KEY AUTOINCREMENT); CREATE TABLE unsupported(v NUMERIC); BEGIN; INSERT INTO t(rowid,id,n) VALUES(9,'pending','pending text'),(11,NULL,3);").unwrap();
        let strict = table_sql::TableOptions {
            strict: true,
            without_rowid: false,
            primary_key_conflict: None,
            checks: None,
        };
        let without_rowid = table_sql::TableOptions {
            strict: false,
            without_rowid: true,
            primary_key_conflict: None,
            checks: None,
        };
        for (name, options) in [
            ("t", strict.clone()),
            ("t", without_rowid.clone()),
            ("no_key", without_rowid.clone()),
            ("ai", without_rowid.clone()),
            ("unsupported", strict.clone()),
        ] {
            let draft = db::table_column_defs_in(&conn, "main", name).unwrap();
            let sources: Vec<_> = draft.iter().map(|col| Some(col.name.clone())).collect();
            assert!(
                db::recreate_table_columns_in(
                    &conn,
                    "main",
                    name,
                    name,
                    &draft,
                    &sources,
                    Some(options.clone())
                )
                .is_err(),
                "{name}/{options:?}"
            );
            assert_eq!(
                db::table_options_in(&conn, "main", name).unwrap(),
                table_sql::TableOptions::default()
            );
            assert_eq!(
                count(&conn, "SELECT count(*) FROM t WHERE rowid IN(7,9,11)"),
                3
            );
            assert_eq!(count(&conn, "PRAGMA foreign_keys"), 1);
            assert!(!conn.is_autocommit());
        }
        conn.execute("UPDATE t SET id='filled' WHERE rowid=11", [])
            .unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
        draft[1].type_name = "ANY".into();
        db::recreate_table_columns_in(
            &conn,
            "main",
            "t",
            "t",
            &draft,
            &[Some("id".into()), Some("n".into())],
            Some(strict),
        )
        .unwrap();
        assert_eq!(
            conn.query_row("SELECT typeof(n) FROM t WHERE rowid=9", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "text"
        );
        conn.execute_batch("CREATE TABLE parent(id INTEGER PRIMARY KEY); CREATE TABLE bad(k TEXT PRIMARY KEY,p REFERENCES parent DEFERRABLE INITIALLY DEFERRED); INSERT INTO bad(rowid,k,p) VALUES(7,'kept',99);").unwrap();
        let draft = db::table_column_defs_in(&conn, "main", "bad").unwrap();
        assert!(db::recreate_table_columns_in(
            &conn,
            "main",
            "bad",
            "bad",
            &draft,
            &[Some("k".into()), Some("p".into())],
            Some(without_rowid)
        )
        .is_err());
        assert_eq!(count(&conn, "SELECT rowid FROM bad"), 7);
        assert!(
            !db::table_options_in(&conn, "main", "bad")
                .unwrap()
                .without_rowid
        );
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(count(&conn, "SELECT count(*) FROM t"), 1);
        assert_eq!(
            db::table_columns(&conn, "t").unwrap()[1].type_name,
            "INTEGER"
        );
    }

    #[test]
    fn inline_foreign_key_edits_preserve_names_other_keys_and_qualified_dependencies() {
        for schema in ["main", "temp", "aux.\"db"] {
            let conn = Connection::open_in_memory().unwrap();
            conn.execute_batch("PRAGMA foreign_keys=ON; ATTACH ':memory:' AS \"aux.\"\"db\";")
                .unwrap();
            let table = db::qualified_name(schema, "t");
            let p = db::qualified_name(schema, "parent");
            let q = db::qualified_name(schema, "other_parent");
            conn.execute_batch(&format!("CREATE TABLE {p}(id INTEGER PRIMARY KEY); INSERT INTO {p} VALUES(1); CREATE TABLE {q}(id INTEGER PRIMARY KEY); INSERT INTO {q} VALUES(1); CREATE TABLE {table}(v INTEGER CONSTRAINT chosen REFERENCES parent(id) ON DELETE RESTRICT,w INTEGER REFERENCES parent(id),n INTEGER,CONSTRAINT table_key FOREIGN KEY(n) REFERENCES parent(id)); INSERT INTO {table}(rowid,v,w,n) VALUES(7,1,1,1); CREATE INDEX {} ON t(n); CREATE TABLE {}(v); CREATE TRIGGER {} AFTER INSERT ON t BEGIN INSERT INTO audit VALUES(new.v); END; BEGIN; INSERT INTO {table}(rowid,v,w,n) VALUES(9,1,1,1);",db::qualified_name(schema,"ix"),db::qualified_name(schema,"audit"),db::qualified_name(schema,"tr"))).unwrap();
            let mut draft = db::table_column_defs_in(&conn, schema, "t").unwrap();
            assert!(draft[0].foreign_key_clause.contains("REFERENCES parent"));
            assert!(draft[2].foreign_key_clause.is_empty());
            draft[0].foreign_key_clause="REFERENCES other_parent(id) ON UPDATE CASCADE ON DELETE SET NULL MATCH SIMPLE DEFERRABLE INITIALLY DEFERRED".into();
            let sources = [Some("v".into()), Some("w".into()), Some("n".into())];
            apply(&conn, schema, "t", &draft, &sources).unwrap();
            let actual = db::table_column_defs_in(&conn, schema, "t").unwrap();
            assert!(
                actual[0].foreign_key_clause.contains("INITIALLY DEFERRED"),
                "{}",
                actual[0].foreign_key_clause
            );
            let sql: String = conn
                .query_row(
                    &format!(
                        "SELECT sql FROM {}.sqlite_schema WHERE name='t'",
                        db::quote_ident(schema)
                    ),
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(
                sql.contains("CONSTRAINT chosen") && sql.contains("CONSTRAINT table_key"),
                "{sql}"
            );
            assert_eq!(
                count(
                    &conn,
                    &format!("SELECT count(*) FROM {table} WHERE rowid IN(7,9)")
                ),
                2
            );
            assert_eq!(
                count(
                    &conn,
                    &format!(
                        "SELECT count(*) FROM {}",
                        db::qualified_name(schema, "audit")
                    )
                ),
                1,
                "copy fired the trigger"
            );
            conn.execute(&format!("UPDATE {q} SET id=11 WHERE id=1"), [])
                .unwrap();
            assert_eq!(
                count(&conn, &format!("SELECT count(*) FROM {table} WHERE v=11")),
                2
            );
            conn.execute(&format!("INSERT INTO {table} VALUES(99,1,1)"), [])
                .unwrap();
            conn.execute(&format!("INSERT INTO {q} VALUES(99)"), [])
                .unwrap();
            conn.execute(&format!("DELETE FROM {q} WHERE id=11"), [])
                .unwrap();
            assert_eq!(
                count(
                    &conn,
                    &format!("SELECT count(*) FROM {table} WHERE v IS NULL")
                ),
                2
            );
            draft[0].foreign_key_clause.clear();
            apply(&conn, schema, "t", &draft, &sources).unwrap();
            assert!(db::table_column_defs_in(&conn, schema, "t").unwrap()[0]
                .foreign_key_clause
                .is_empty());
            conn.execute(&format!("INSERT INTO {table} VALUES(999,1,1)"), [])
                .unwrap();
            assert!(conn
                .execute(&format!("INSERT INTO {table} VALUES(999,999,1)"), [])
                .is_err());
            assert!(conn
                .execute(&format!("INSERT INTO {table} VALUES(999,1,999)"), [])
                .is_err());
            assert_eq!(
                count(
                    &conn,
                    &format!(
                        "SELECT count(*) FROM {}.sqlite_schema WHERE name IN('ix','tr')",
                        db::quote_ident(schema)
                    )
                ),
                2
            );
            assert_eq!(count(&conn, "PRAGMA foreign_keys"), 1);
            assert!(!conn.is_autocommit());
            conn.execute_batch("ROLLBACK").unwrap();
            assert_eq!(count(&conn, &format!("SELECT count(*) FROM {table}")), 1);
            assert!(db::table_column_defs_in(&conn, schema, "t").unwrap()[0]
                .foreign_key_clause
                .contains("RESTRICT"));
        }
    }

    #[test]
    fn inline_foreign_key_deferral_survives_separated_constraints_and_keeps_other_references() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE parent(id PRIMARY KEY); INSERT INTO parent VALUES(1); CREATE TABLE other_parent(id PRIMARY KEY); INSERT INTO other_parent VALUES(1),(2); CREATE TABLE t(v CONSTRAINT chosen REFERENCES parent(id) CHECK(v>0) CONSTRAINT timing DEFERRABLE INITIALLY DEFERRED CONSTRAINT other REFERENCES other_parent(id) NOT DEFERRABLE INITIALLY IMMEDIATE); INSERT INTO t VALUES(1); BEGIN;").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
        assert!(draft[0].foreign_key_clause.contains("INITIALLY DEFERRED"));
        assert!(!draft[0].foreign_key_clause.contains("other_parent"));
        draft[0].foreign_key_clause =
            "REFERENCES parent(id) NOT DEFERRABLE INITIALLY IMMEDIATE".into();
        apply(&conn, "main", "t", &draft, &[Some("v".into())]).unwrap();
        assert!(conn.execute("INSERT INTO t VALUES(2)", []).is_err());
        draft[0].foreign_key_clause.clear();
        apply(&conn, "main", "t", &draft, &[Some("v".into())]).unwrap();
        let actual = db::table_column_defs_in(&conn, "main", "t").unwrap();
        assert!(actual[0].foreign_key_clause.contains("other_parent"));
        assert!(actual[0].foreign_key_clause.contains("INITIALLY IMMEDIATE"));
        conn.execute("INSERT INTO t VALUES(2)", []).unwrap();
        assert!(conn.execute("INSERT INTO t VALUES(3)", []).is_err());
        assert!(conn.execute("INSERT INTO t VALUES(-1)", []).is_err());
        conn.execute_batch("ROLLBACK").unwrap();
        assert!(db::table_column_defs_in(&conn, "main", "t").unwrap()[0]
            .foreign_key_clause
            .contains("INITIALLY DEFERRED"));
    }

    #[test]
    fn named_inline_foreign_key_deferrals_survive_other_edits_and_pending_violations() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE parent(id PRIMARY KEY); INSERT INTO parent VALUES(1); CREATE TABLE t(v REFERENCES parent(id) CHECK(v>0) CONSTRAINT \"时限\" /* keep name */ DEFERRABLE INITIALLY DEFERRED,note TEXT DEFAULT 'CONSTRAINT text DEFERRABLE __next_gui_named_deferral'); INSERT INTO t(v) VALUES(1); BEGIN; INSERT INTO t(v) VALUES(2);").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
        assert!(draft[0].foreign_key_clause.contains("INITIALLY DEFERRED"));
        draft[1].type_name = "VARCHAR(255)".into();
        apply(
            &conn,
            "main",
            "t",
            &draft,
            &[Some("v".into()), Some("note".into())],
        )
        .unwrap();
        let sql: String = conn
            .query_row("SELECT sql FROM sqlite_schema WHERE name='t'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(
            sql.contains("CONSTRAINT \"时限\" DEFERRABLE INITIALLY DEFERRED"),
            "{sql}"
        );
        assert_eq!(
            conn.query_row("SELECT note FROM t LIMIT 1", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "CONSTRAINT text DEFERRABLE __next_gui_named_deferral"
        );
        conn.execute("INSERT INTO t(v) VALUES(3)", []).unwrap();
        assert!(conn.execute_batch("COMMIT").is_err());
        assert!(!conn.is_autocommit());
        conn.execute_batch("INSERT INTO parent VALUES(2),(3); COMMIT;")
            .unwrap();
        assert_eq!(count(&conn, "SELECT count(*) FROM t"), 3);
    }

    #[test]
    fn invalid_inline_foreign_keys_cannot_mutate_data_or_other_constraints() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE parent(id INTEGER PRIMARY KEY); INSERT INTO parent VALUES(1); CREATE TABLE non_unique(id); CREATE TABLE t(v INTEGER); INSERT INTO t(rowid,v) VALUES(7,1); BEGIN; INSERT INTO t(rowid,v) VALUES(9,2);").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
        let sources = [Some("v".into())];
        for clause in [
            "REFERENCES parent(id)",
            "REFERENCES absent(id)",
            "REFERENCES non_unique(id)",
            "REFERENCES parent(missing)",
            "REFERENCES parent(id) UNIQUE",
            "REFERENCES parent(id) REFERENCES parent(id)",
            "REFERENCES parent(id)); DELETE FROM t; --",
            "CHECK(v>0)",
            "REFERENCES main.parent(id)",
        ] {
            draft[0].foreign_key_clause = clause.into();
            assert!(
                apply(&conn, "main", "t", &draft, &sources).is_err(),
                "{clause}"
            );
            assert_eq!(
                count(&conn, "SELECT count(*) FROM t WHERE rowid IN(7,9)"),
                2
            );
            assert!(db::table_column_defs_in(&conn, "main", "t").unwrap()[0]
                .foreign_key_clause
                .is_empty());
            assert_eq!(count(&conn, "PRAGMA foreign_keys"), 1);
            assert!(!conn.is_autocommit());
        }
        draft[0].foreign_key_clause.clear();
        for field in [0, 1] {
            let mut injected = ColDef::new("v", "INTEGER");
            if field == 0 {
                injected.type_name = "INTEGER REFERENCES parent(id)".into();
            } else {
                injected.default_value = "1) REFERENCES parent(id) DEFAULT(1".into();
            }
            assert!(table_sql::column_definition(&injected, None).is_err());
        }
        conn.execute_batch("INSERT INTO parent VALUES(2)").unwrap();
        draft[0].foreign_key_clause = "REFERENCES parent(id)".into();
        apply(&conn, "main", "t", &draft, &sources).unwrap();
        assert!(conn.execute("INSERT INTO t VALUES(3)", []).is_err());
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(count(&conn, "SELECT count(*) FROM t"), 1);
    }

    #[test]
    fn create_and_add_inline_foreign_keys_keep_actions_and_enforce_references() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE parent(id INTEGER PRIMARY KEY); INSERT INTO parent VALUES(0),(1); CREATE TABLE t(v); INSERT INTO t VALUES('kept'); BEGIN;").unwrap();
        for (number, action) in [
            "NO ACTION",
            "RESTRICT",
            "CASCADE",
            "SET NULL",
            "SET DEFAULT",
        ]
        .into_iter()
        .enumerate()
        {
            let name = format!("created{number}");
            let column = ColDef {
                foreign_key_clause: format!(
                    "REFERENCES parent ON UPDATE {action} ON DELETE {action} -- tail comment"
                ),
                default_value: "0".into(),
                ..ColDef::new("v", "INTEGER")
            };
            table_sql::column_definition(&column, None).unwrap();
            db::execute_single_statement(&conn, &db::create_table_ddl(&name, &[column])).unwrap();
            let actual: (String, String) = conn
                .query_row(
                    &format!("PRAGMA foreign_key_list({})", db::quote_ident(&name)),
                    [],
                    |r| Ok((r.get(5)?, r.get(6)?)),
                )
                .unwrap();
            assert_eq!(actual, (action.into(), action.into()));
            assert!(conn
                .execute(
                    &format!("INSERT INTO {} VALUES(99)", db::quote_ident(&name)),
                    []
                )
                .is_err());
        }
        let mut draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
        draft.push(ColDef {
            foreign_key_clause: "REFERENCES parent(id) ON DELETE CASCADE".into(),
            default_value: "1".into(),
            ..ColDef::new("p", "INTEGER")
        });
        apply(&conn, "main", "t", &draft, &[Some("v".into()), None]).unwrap();
        assert_eq!(count(&conn, "SELECT p FROM t"), 1);
        conn.execute("DELETE FROM parent WHERE id=1", []).unwrap();
        assert_eq!(count(&conn, "SELECT count(*) FROM t"), 0);
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(count(&conn, "SELECT count(*) FROM t"), 1);
    }

    #[test]
    fn column_checks_edit_first_named_constraint_and_keep_other_constraints_and_dependencies() {
        for schema in ["main", "temp", "aux.\"db"] {
            let conn = Connection::open_in_memory().unwrap();
            conn.execute_batch("ATTACH ':memory:' AS \"aux.\"\"db\";")
                .unwrap();
            let table = db::qualified_name(schema, "t");
            conn.execute_batch(&format!("CREATE TABLE {table}(v TEXT CONSTRAINT chosen CHECK(length(v)>0) CONSTRAINT kept CHECK(v!='bad') UNIQUE, n INTEGER, CONSTRAINT table_check CHECK(n>0)); INSERT INTO {table}(rowid,v,n) VALUES(7,'Alpha',1),(9,'Beta',2);
                CREATE INDEX {} ON t(n); CREATE TABLE {}(v); CREATE TRIGGER {} AFTER INSERT ON t BEGIN INSERT INTO audit VALUES(new.v); END; BEGIN;", db::qualified_name(schema,"ix"), db::qualified_name(schema,"audit"), db::qualified_name(schema,"tr"))).unwrap();
            let mut draft = db::table_column_defs_in(&conn, schema, "t").unwrap();
            assert!(draft[0]
                .check_expression
                .split_whitespace()
                .collect::<String>()
                .contains("length(v)"));
            assert!(draft[1].check_expression.is_empty());
            draft[0].check_expression = "length(v) >= 4\nAND v != 'none'".into();
            let sources = [Some("v".into()), Some("n".into())];
            apply(&conn, schema, "t", &draft, &sources).unwrap();
            let sql: String = conn
                .query_row(
                    &format!(
                        "SELECT sql FROM {}.sqlite_schema WHERE name='t'",
                        db::quote_ident(schema)
                    ),
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            for name in ["chosen", "kept", "table_check"] {
                assert!(sql.contains(&format!("CONSTRAINT {name}")), "{sql}");
            }
            assert_eq!(
                count(
                    &conn,
                    &format!(
                        "SELECT count(*) FROM {}",
                        db::qualified_name(schema, "audit")
                    )
                ),
                0,
                "copy must not fire the user trigger"
            );
            assert!(conn
                .execute(&format!("INSERT INTO {table} VALUES('abc',1)"), [])
                .is_err());
            assert_eq!(
                count(
                    &conn,
                    &format!("SELECT count(*) FROM {table} WHERE rowid IN(7,9)")
                ),
                2
            );
            draft[0].check_expression.clear();
            apply(&conn, schema, "t", &draft, &sources).unwrap();
            let definitions = db::table_column_defs_in(&conn, schema, "t").unwrap();
            assert!(
                definitions[0].check_expression.contains("bad"),
                "other column CHECK remains editable after reopening"
            );
            conn.execute(&format!("INSERT INTO {table} VALUES('X',1)"), [])
                .unwrap();
            assert!(conn
                .execute(&format!("INSERT INTO {table} VALUES('bad',1)"), [])
                .is_err());
            assert!(conn
                .execute(&format!("INSERT INTO {table} VALUES('other',0)"), [])
                .is_err());
            assert!(conn
                .execute(&format!("INSERT INTO {table} VALUES('Alpha',1)"), [])
                .is_err());
            assert_eq!(
                count(
                    &conn,
                    &format!(
                        "SELECT count(*) FROM {}",
                        db::qualified_name(schema, "audit")
                    )
                ),
                1
            );
            assert_eq!(
                count(
                    &conn,
                    &format!(
                        "SELECT count(*) FROM {}.sqlite_schema WHERE name='ix'",
                        db::quote_ident(schema)
                    )
                ),
                1
            );
            assert!(!conn.is_autocommit());
            conn.execute_batch("ROLLBACK").unwrap();
            assert_eq!(count(&conn, &format!("SELECT count(*) FROM {table}")), 2);
            assert!(db::table_column_defs_in(&conn, schema, "t").unwrap()[0]
                .check_expression
                .split_whitespace()
                .collect::<String>()
                .contains("length(v)"));
        }
    }

    #[test]
    fn invalid_column_checks_roll_back_pending_data_and_create_add_enforce_checks() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE t(v TEXT,n INTEGER); INSERT INTO t(rowid,v,n) VALUES(7,'kept',1); BEGIN; INSERT INTO t(rowid,v,n) VALUES(9,'pending',2);").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
        let sources = [Some("v".into()), Some("n".into())];
        for expression in [
            "length(v)>20",
            "missing_column>0",
            "v IN (SELECT v FROM t)",
            "1) UNIQUE CHECK(1",
            "1)); DELETE FROM t; --",
            "?1",
        ] {
            draft[0].check_expression = expression.into();
            assert!(
                apply(&conn, "main", "t", &draft, &sources).is_err(),
                "{expression}"
            );
            assert_eq!(
                count(&conn, "SELECT count(*) FROM t WHERE rowid IN(7,9)"),
                2
            );
            assert!(db::table_column_defs_in(&conn, "main", "t").unwrap()[0]
                .check_expression
                .is_empty());
            assert_eq!(count(&conn, "PRAGMA foreign_keys"), 1);
            assert!(!conn.is_autocommit());
        }
        draft[0].check_expression.clear();
        draft.push(ColDef {
            check_expression: "extra >= 0".into(),
            default_value: "3".into(),
            ..ColDef::new("extra", "INTEGER")
        });
        apply(
            &conn,
            "main",
            "t",
            &draft,
            &[Some("v".into()), Some("n".into()), None],
        )
        .unwrap();
        assert_eq!(count(&conn, "SELECT sum(extra) FROM t"), 6);
        assert!(conn
            .execute("INSERT INTO t VALUES('bad',3,-1)", [])
            .is_err());
        let column = ColDef {
            check_expression: "length(v)>0 -- final comment\n".into(),
            ..ColDef::new("v", "TEXT")
        };
        crate::table_sql::column_definition(&column, None).unwrap();
        db::execute_single_statement(&conn, &db::create_table_ddl("created", &[column])).unwrap();
        assert!(conn.execute("INSERT INTO created VALUES('')", []).is_err());
        conn.execute("INSERT INTO created VALUES('valid')", [])
            .unwrap();
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(count(&conn, "SELECT count(*) FROM t"), 1);
        assert_eq!(
            db::table_column_defs_in(&conn, "main", "t").unwrap().len(),
            2
        );
    }

    #[test]
    fn collation_edits_preserve_constraints_and_actual_comparison_semantics() {
        for schema in ["main", "temp", "aux.\"db"] {
            let conn = Connection::open_in_memory().unwrap();
            conn.execute_batch("ATTACH ':memory:' AS \"aux.\"\"db\";")
                .unwrap();
            let table = db::qualified_name(schema, "t");
            conn.execute_batch(&format!("CREATE TABLE {table}(v TEXT CONSTRAINT ignored COLLATE BINARY CONSTRAINT chosen COLLATE 'NOCASE' CONSTRAINT uq UNIQUE CHECK(length(v)>0),n INTEGER); INSERT INTO {table}(rowid,v,n) VALUES(7,'Alpha',1),(9,'Beta',2); BEGIN;")).unwrap();
            let mut draft = db::table_column_defs_in(&conn, schema, "t").unwrap();
            assert_eq!(draft[0].collation, "NOCASE");
            let sources = [Some("v".into()), Some("n".into())];
            assert_eq!(
                count(
                    &conn,
                    &format!("SELECT count(*) FROM {table} WHERE v='alpha'")
                ),
                1
            );
            draft[0].collation = "RTRIM".into();
            apply(&conn, schema, "t", &draft, &sources).unwrap();
            assert_eq!(
                count(
                    &conn,
                    &format!("SELECT count(*) FROM {table} WHERE v='Alpha  '")
                ),
                1
            );
            assert_eq!(
                count(
                    &conn,
                    &format!("SELECT count(*) FROM {table} WHERE v='alpha'")
                ),
                0
            );
            assert_eq!(
                count(
                    &conn,
                    &format!("SELECT count(*) FROM {table} WHERE rowid IN(7,9)")
                ),
                2
            );
            let sql: String = conn
                .query_row(
                    &format!(
                        "SELECT sql FROM {}.sqlite_schema WHERE name='t'",
                        db::quote_ident(schema)
                    ),
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(sql.contains("CONSTRAINT chosen"), "{sql}");
            assert!(!sql.contains("CONSTRAINT ignored"), "{sql}");
            assert!(sql.contains("CONSTRAINT uq"), "{sql}");
            assert!(conn
                .execute(&format!("INSERT INTO {table}(v) VALUES('')"), [])
                .is_err());
            assert!(conn
                .execute(&format!("INSERT INTO {table}(v) VALUES('Alpha ')"), [])
                .is_err());
            draft[0].collation.clear();
            apply(&conn, schema, "t", &draft, &sources).unwrap();
            assert!(db::table_column_defs_in(&conn, schema, "t").unwrap()[0]
                .collation
                .is_empty());
            conn.execute(&format!("INSERT INTO {table}(v) VALUES('Alpha ')"), [])
                .unwrap();
            conn.execute_batch("ROLLBACK").unwrap();
            assert_eq!(
                db::table_column_defs_in(&conn, schema, "t").unwrap()[0].collation,
                "NOCASE"
            );
            let created = [ColDef {
                collation: "NOCASE".into(),
                unique: true,
                ..ColDef::new("v", "TEXT")
            }];
            db::execute_single_statement(
                &conn,
                &db::create_table_ddl_in(schema, "created", &created),
            )
            .unwrap();
            conn.execute(
                &format!(
                    "INSERT INTO {} VALUES('one')",
                    db::qualified_name(schema, "created")
                ),
                [],
            )
            .unwrap();
            assert!(conn
                .execute(
                    &format!(
                        "INSERT INTO {} VALUES('ONE')",
                        db::qualified_name(schema, "created")
                    ),
                    []
                )
                .is_err());
        }
    }

    #[test]
    fn collation_conflicts_foreign_keys_and_unknown_names_roll_back() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE t(v TEXT UNIQUE); INSERT INTO t(rowid,v) VALUES(7,'A'),(9,'a'); BEGIN; INSERT INTO t(rowid,v) VALUES(11,'pending');").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
        draft[0].collation = "NOCASE".into();
        assert!(apply(&conn, "main", "t", &draft, &[Some("v".into())]).is_err());
        assert_eq!(
            count(&conn, "SELECT count(*) FROM t WHERE rowid IN(7,9,11)"),
            3
        );
        draft[0].collation = "NOCASE\"); DROP TABLE t; --".into();
        assert!(apply(&conn, "main", "t", &draft, &[Some("v".into())]).is_err());
        assert_eq!(count(&conn, "SELECT count(*) FROM t"), 3);
        conn.execute_batch("CREATE TABLE parent(v TEXT COLLATE NOCASE UNIQUE); INSERT INTO parent VALUES('Alpha'); CREATE TABLE child(p REFERENCES parent(v)); INSERT INTO child VALUES('alpha');").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "parent").unwrap();
        draft[0].collation = "BINARY".into();
        assert!(apply(&conn, "main", "parent", &draft, &[Some("v".into())])
            .unwrap_err()
            .contains("foreign key"));
        assert!(foreign_key_violations(&conn, "main").unwrap().is_empty());
        assert_eq!(
            db::table_column_defs_in(&conn, "main", "parent").unwrap()[0].collation,
            "NOCASE"
        );
        assert!(conn
            .db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY)
            .unwrap());
        assert!(!conn.is_autocommit());
        conn.execute_batch("ROLLBACK").unwrap();
        assert_eq!(count(&conn, "SELECT count(*) FROM t"), 2);
    }

    #[test]
    fn autoincrement_edits_preserve_ids_assign_nulls_and_track_sequence_in_every_schema() {
        for schema in ["main", "temp", "aux.\"db"] {
            let conn = Connection::open_in_memory().unwrap();
            conn.execute_batch("ATTACH ':memory:' AS \"aux.\"\"db\"; PRAGMA foreign_keys=ON;")
                .unwrap();
            let table = db::qualified_name(schema, "t");
            conn.execute_batch(&format!("CREATE TABLE {table}(id TEXT,code TEXT PRIMARY KEY); INSERT INTO {table}(rowid,id,code) VALUES(7,NULL,'first'),(9,'2','second'),(11,NULL,'third'); BEGIN;")).unwrap();
            let mut draft = db::table_column_defs_in(&conn, schema, "t").unwrap();
            draft[0].type_name = "INTEGER".into();
            draft[0].pk = true;
            draft[0].auto_increment = true;
            draft[1].pk = false;
            let sources = [Some("id".into()), Some("code".into())];
            apply(&conn, schema, "t", &draft, &sources).unwrap();
            assert!(db::table_column_defs_in(&conn, schema, "t").unwrap()[0].auto_increment);
            assert_eq!(
                count(
                    &conn,
                    &format!("SELECT id FROM {table} WHERE code='second'")
                ),
                2
            );
            assert_eq!(
                count(
                    &conn,
                    &format!(
                        "SELECT count(*) FROM {table} WHERE id>2 AND code IN('first','third')"
                    )
                ),
                2
            );
            conn.execute_batch(&format!(
                "INSERT INTO {table} VALUES(100,'removed'); DELETE FROM {table} WHERE id=100;"
            ))
            .unwrap();
            draft[1].default_value = "'new'".into();
            apply(&conn, schema, "t", &draft, &sources).unwrap();
            conn.execute(&format!("INSERT INTO {table} DEFAULT VALUES"), [])
                .unwrap();
            assert_eq!(count(&conn, &format!("SELECT max(id) FROM {table}")), 101);
            draft[0].auto_increment = false;
            apply(&conn, schema, "t", &draft, &sources).unwrap();
            assert!(!db::table_column_defs_in(&conn, schema, "t").unwrap()[0].auto_increment);
            assert!(db::table_column_defs_in(&conn, schema, "t").unwrap()[0].pk);
            assert_eq!(sequence(&conn, schema, "t").unwrap(), None);
            conn.execute_batch("ROLLBACK").unwrap();
            assert_eq!(
                count(
                    &conn,
                    &format!("SELECT count(*) FROM {table} WHERE id IS NULL")
                ),
                2
            );
            assert!(!db::table_column_defs_in(&conn, schema, "t").unwrap()[0].auto_increment);
            let created = [
                ColDef {
                    pk: true,
                    auto_increment: true,
                    ..ColDef::new("id", "INTEGER")
                },
                ColDef::new("v", "TEXT"),
            ];
            db::execute_single_statement(
                &conn,
                &db::create_table_ddl_in(schema, "created", &created),
            )
            .unwrap();
            assert!(db::table_column_defs_in(&conn, schema, "created").unwrap()[0].auto_increment);
        }
    }

    #[test]
    fn invalid_autoincrement_types_values_and_key_combinations_roll_back() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE t(id TEXT,v TEXT); INSERT INTO t VALUES('not an integer','kept'); BEGIN;",
        )
        .unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "t").unwrap();
        let sources = [Some("id".into()), Some("v".into())];
        draft[0].pk = true;
        draft[0].auto_increment = true;
        assert!(apply(&conn, "main", "t", &draft, &sources).is_err());
        draft[0].type_name = "INTEGER".into();
        assert!(apply(&conn, "main", "t", &draft, &sources).is_err());
        draft[1].pk = true;
        assert!(apply(&conn, "main", "t", &draft, &sources)
            .unwrap_err()
            .contains("Autoincrement"));
        draft[0].pk = false;
        assert!(apply(&conn, "main", "t", &draft, &sources).is_err());
        assert_eq!(
            count(
                &conn,
                "SELECT count(*) FROM t WHERE id='not an integer' AND v='kept'"
            ),
            1
        );
        assert!(!conn.is_autocommit());
        assert!(!db::table_columns(&conn, "t").unwrap()[0].pk);
        conn.execute_batch("ROLLBACK").unwrap();
        conn.execute_batch("CREATE TABLE exhausted(id TEXT); INSERT INTO exhausted VALUES('9223372036854775807'),(NULL); BEGIN;").unwrap();
        let mut draft = db::table_column_defs_in(&conn, "main", "exhausted").unwrap();
        draft[0].pk = true;
        draft[0].auto_increment = true;
        draft[0].type_name = "INTEGER".into();
        assert!(apply(&conn, "main", "exhausted", &draft, &[Some("id".into())]).is_err());
        assert_eq!(
            count(&conn, "SELECT count(*) FROM exhausted WHERE id IS NULL"),
            1
        );
        assert_eq!(count(&conn, "SELECT count(*) FROM exhausted"), 2);
        conn.execute_batch("ROLLBACK").unwrap();
    }

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
        draft[0].auto_increment = false;
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
            None,
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
            None,
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

    #[test]
    fn generated_columns_recompute_and_freeze_with_qualified_dependencies_and_rowids() {
        for schema in ["main", "temp", "aux.\"db"] {
            let conn = Connection::open_in_memory().unwrap();
            let table = db::qualified_name(schema, "computed");
            let audit = db::qualified_name(schema, "audit");
            let child = db::qualified_name(schema, "child");
            let view = db::qualified_name(schema, "viewed");
            conn.execute_batch(&format!("PRAGMA foreign_keys=ON; ATTACH ':memory:' AS \"aux.\"\"db\";
                CREATE TABLE {table}(id INTEGER PRIMARY KEY AUTOINCREMENT,v INTEGER,
                    g INTEGER CONSTRAINT generation AS(v*2) STORED CONSTRAINT nn NOT NULL CHECK(g>0) UNIQUE,frozen INTEGER);
                CREATE TABLE {audit}(v); CREATE TABLE {child}(p REFERENCES computed(g));
                CREATE VIEW {view} AS SELECT id,v,g FROM computed;
                CREATE INDEX {} ON computed(g) WHERE g>0;
                CREATE TRIGGER {} AFTER UPDATE OF v ON {table} BEGIN INSERT INTO audit VALUES(new.g); END;
                INSERT INTO {table}(id,v,frozen) VALUES(7,3,99),(100,5,99); DELETE FROM {table} WHERE id=100;
                BEGIN; INSERT INTO {audit} VALUES('pending');", db::qualified_name(schema,"ix"),db::qualified_name(schema,"tr"))).unwrap();
            let original = db::table_column_defs_in(&conn,schema,"computed").unwrap();
            assert_eq!(original[2].generated.as_ref().unwrap(),&db::GeneratedColumn { expression:"v * 2".into(),stored:true });
            let mut draft = vec![original[2].clone(),original[0].clone(),original[1].clone(),original[3].clone()];
            draft[0].generated = Some(db::GeneratedColumn { expression:"amount * 3".into(),stored:false });
            draft[2].name = "amount".into();
            draft[3].generated = Some(db::GeneratedColumn { expression:"id + amount".into(),stored:true });
            apply(&conn,schema,"computed",&draft,&[Some("g".into()),Some("id".into()),Some("v".into()),Some("frozen".into())]).unwrap_or_else(|error|panic!("{schema}: {error}"));
            assert_eq!(count(&conn,&format!("SELECT count(*) FROM {table} WHERE rowid=7 AND g=9 AND frozen=10 AND amount=3")),1);
            assert_eq!(count(&conn,&format!("SELECT g FROM {view}")),9);
            assert_eq!(count(&conn,&format!("SELECT count(*) FROM {audit}")),1,"copy must not fire the update trigger");
            assert_eq!(sequence(&conn,schema,"computed").unwrap(),Some(100));
            let sql:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='computed'",db::quote_ident(schema)),[],|row|row.get(0)).unwrap();
            assert!(sql.contains("CONSTRAINT generation") && sql.contains("CONSTRAINT nn"));
            conn.execute_batch(&format!("INSERT INTO {child} VALUES(9)")).unwrap();
            let original=db::table_column_defs_in(&conn,schema,"computed").unwrap();
            let sources:Vec<_>=original.iter().map(|column|Some(column.name.clone())).collect();
            let mut invalid=original.clone(); invalid[0].generated.as_mut().unwrap().expression="amount * 4".into();
            assert!(apply(&conn,schema,"computed",&invalid,&sources).is_err(),"orphaning a generated parent key must roll back");
            assert_eq!(count(&conn,&format!("SELECT g FROM {table}")),9);
            let mut draft=original;
            draft[0].generated=None; draft[3].generated=None;
            draft.push(ColDef { generated:Some(db::GeneratedColumn { expression:"amount + 1".into(),stored:true }),..ColDef::new("added","INTEGER") });
            let mut sources=sources; sources.push(None);
            apply(&conn,schema,"computed",&draft,&sources).unwrap();
            conn.execute_batch(&format!("UPDATE {table} SET amount=4; INSERT INTO {table}(amount) VALUES(5)")).unwrap_err();
            // The later NOT NULL failure leaves the earlier UPDATE pending and the frozen values unchanged.
            assert_eq!(count(&conn,&format!("SELECT count(*) FROM {table} WHERE rowid=7 AND g=9 AND frozen=10 AND added=5")),1);
            assert_eq!(count(&conn,&format!("SELECT count(*) FROM {audit}")),2);
            assert!(foreign_key_violations(&conn,schema).unwrap().is_empty());
            assert!(conn.db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY).unwrap());
            assert!(!conn.is_autocommit());
            let draft=db::table_column_defs_in(&conn,schema,"computed").unwrap();
            let sources:Vec<_>=draft.iter().take(4).map(|column|Some(column.name.clone())).collect();
            apply(&conn,schema,"computed",&draft[..4],&sources).unwrap();
            assert_eq!(db::table_column_names_in(&conn,schema,"computed").unwrap(),["g","id","amount","frozen"]);
            conn.execute_batch("ROLLBACK").unwrap();
            assert_eq!(db::table_column_names_in(&conn,schema,"computed").unwrap(),["id","v","g","frozen"]);
            assert_eq!(count(&conn,&format!("SELECT count(*) FROM {table} WHERE id=7 AND g=6 AND frozen=99")),1);
            assert_eq!(count(&conn,&format!("SELECT count(*) FROM {audit}")),0);
        }
    }

    #[test]
    fn invalid_generated_definitions_preserve_schema_rows_and_pending_transaction() {
        for schema in ["main","temp","aux.\"db"] {
            let conn=Connection::open_in_memory().unwrap();
            let table=db::qualified_name(schema,"computed");
            conn.execute_batch(&format!("PRAGMA foreign_keys=ON; ATTACH ':memory:' AS \"aux.\"\"db\";
                CREATE TABLE {table}(v INTEGER,g INTEGER AS(v*2) STORED NOT NULL UNIQUE CHECK(g>0));
                INSERT INTO {table}(rowid,v) VALUES(7,3),(11,4); BEGIN;
                CREATE TABLE {}(v); INSERT INTO {} VALUES('pending');",db::qualified_name(schema,"pending"),db::qualified_name(schema,"pending"))).unwrap();
            let original=db::table_column_defs_in(&conn,schema,"computed").unwrap();
            let sources=[Some("v".into()),Some("g".into())];
            let before:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='computed'",db::quote_ident(schema)),[],|row|row.get(0)).unwrap();
            for expression in ["","-- empty","random()","g + 1","(SELECT v)","?1","v / 0","1","v) STORED, injected TEXT --","v); PRAGMA foreign_keys=OFF; --"] {
                let mut draft=original.clone();
                draft[1].generated=Some(db::GeneratedColumn { expression:expression.into(),stored:false });
                assert!(apply(&conn,schema,"computed",&draft,&sources).is_err(),"{schema}/{expression}");
                let after:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='computed'",db::quote_ident(schema)),[],|row|row.get(0)).unwrap();
                assert_eq!(after,before);
                assert_eq!(count(&conn,&format!("SELECT count(*) FROM {table} WHERE (rowid=7 AND v=3 AND g=6) OR (rowid=11 AND v=4 AND g=8)")),2);
                assert_eq!(count(&conn,&format!("SELECT count(*) FROM {}",db::qualified_name(schema,"pending"))),1);
                assert!(conn.db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_FKEY).unwrap());
                assert!(!conn.is_autocommit());
            }
            for category in 0..3 {
                let mut draft=original.clone();
                match category { 0=>draft[1].default_value="7".into(),1=>draft[1].pk=true,_=>draft[0].generated=Some(db::GeneratedColumn { expression:"7".into(),stored:false }) }
                assert!(apply(&conn,schema,"computed",&draft,&sources).is_err());
            }
        }
    }

    fn apply(
        conn: &Connection,
        schema: &str,
        table: &str,
        draft: &[ColDef],
        sources: &[Option<String>],
    ) -> Result<(), String> {
        db::recreate_table_columns_in(conn, schema, table, table, draft, sources, None)
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
                original[2].clone(),
            ];
            draft[1].type_name = "BLOB".into();
            draft[1].notnull = true;
            draft[1].default_value = "'new default'".into();
            let sources = [None, Some("v".into()), Some("id".into()), Some("g".into())];
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
