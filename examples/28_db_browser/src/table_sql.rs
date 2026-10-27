//! Preserve SQLite constraints while changing the fields exposed by the table wizard.

use fallible_iterator::FallibleIterator;
use sqlite3_parser::{
    ast::{
        Cmd, ColFlags, ColumnConstraint, ColumnDefinition, CreateTableBody, Expr, Name,
        QualifiedName, Stmt, TabFlags, TableConstraint,
    },
    lexer::sql::Parser,
};

use crate::db::{self, ColDef};

fn parse(sql: &str) -> Result<Cmd, String> {
    let mut parser = Parser::new(sql.as_bytes());
    let command = parser
        .next()
        .map_err(|error| error.to_string())?
        .ok_or("Empty schema definition")?;
    if parser.next().map_err(|error| error.to_string())?.is_some() {
        return Err("Expected one schema statement".into());
    }
    Ok(command)
}

fn identifier(value: &str) -> Name {
    Name(db::quote_ident(value).into_boxed_str())
}

fn qualified(schema: &str, name: &str) -> QualifiedName {
    QualifiedName::fullname(identifier(schema), identifier(name))
}

fn emit(mut command: Cmd) -> String {
    // Version 0.15 omits the comma between these two table options.
    let mut strict_after_rowid = false;
    if let Cmd::Stmt(Stmt::CreateTable {
        body: CreateTableBody::ColumnsAndConstraints { flags, .. },
        ..
    }) = &mut command
    {
        if flags.contains(TabFlags::WithoutRowid | TabFlags::Strict) {
            flags.remove(TabFlags::Strict);
            strict_after_rowid = true;
        }
    }
    let sql = command.to_string();
    if strict_after_rowid {
        format!("{}, STRICT;", sql.strip_suffix(';').unwrap_or(&sql))
    } else {
        sql
    }
}

fn column_expression(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Name(value) => value.eq(name),
        Expr::Id(value) => Name(value.0.clone()).eq(name),
        Expr::Collate(value, _) => column_expression(value, name),
        _ => false,
    }
}

pub fn column_definition(
    column: &ColDef,
    original: Option<&ColDef>,
) -> Result<ColumnDefinition, String> {
    let mut draft = column.clone();
    // PRAGMA strips quoting from names such as "TEXT UNIQUE"; keep their original AST.
    if original.is_some_and(|old| old.type_name.eq_ignore_ascii_case(&column.type_name)) {
        draft.type_name = "BLOB".into();
    }
    let Cmd::Stmt(Stmt::CreateTable {
        body:
            CreateTableBody::ColumnsAndConstraints {
                columns,
                constraints,
                flags,
            },
        ..
    }) = parse(&db::create_table_ddl("draft", &[draft]))?
    else {
        return Err("Invalid column definition".into());
    };
    if columns.len() != 1
        || constraints.is_some()
        || flags.intersects(TabFlags::Strict | TabFlags::WithoutRowid)
    {
        return Err("A column field cannot add columns or table constraints".into());
    }
    let mut actual = [0; 4];
    let definition = columns.into_values().next().unwrap();
    for value in &definition.constraints {
        actual[match value.constraint {
            ColumnConstraint::PrimaryKey { .. } => 0,
            ColumnConstraint::NotNull { nullable: false, .. } => 1,
            ColumnConstraint::Unique(..) => 2,
            ColumnConstraint::Default(..) => 3,
            _ => return Err("Use the constraint controls rather than adding constraints to a type/default field".into()),
        }] += 1;
    }
    let expected = [
        column.pk as usize,
        column.notnull as usize,
        column.unique as usize,
        (!column.default_value.trim().is_empty()) as usize,
    ];
    if actual != expected {
        return Err("A type/default field cannot change other column constraints".into());
    }
    Ok(definition)
}

pub fn rebuild_definition(
    sql: &str,
    schema: &str,
    temporary_name: &str,
    original: &[ColDef],
    draft: &[ColDef],
    sources: &[Option<String>],
) -> Result<String, String> {
    let mut command = parse(sql)?;
    let Cmd::Stmt(Stmt::CreateTable {
        temporary,
        if_not_exists,
        tbl_name,
        body:
            CreateTableBody::ColumnsAndConstraints {
                columns,
                constraints,
                ..
            },
    }) = &mut command
    else {
        return Err("Modify Table requires an ordinary CREATE TABLE definition".into());
    };
    let mut edited = Vec::new();
    for (col, source) in draft.iter().zip(sources) {
        let old = source
            .as_ref()
            .and_then(|source| original.iter().find(|item| item.name == *source));
        let replacement = column_definition(col, old)?;
        let mut result = if source.is_some() {
            let old = old.ok_or("Missing original column")?;
            let mut item = columns
                .values()
                .find(|item| item.col_name.eq(col.name.as_str()))
                .cloned()
                .ok_or("Original column definition no longer exists")?;
            if !old.type_name.eq_ignore_ascii_case(&col.type_name) {
                item.col_type = replacement.col_type.clone();
            }
            for category in 0..3 {
                let changed = match category {
                    0 => old.notnull != col.notnull,
                    1 => old.unique != col.unique,
                    _ => old.default_value != col.default_value,
                };
                if !changed {
                    continue;
                }
                let matches = |value: &ColumnConstraint| match category {
                    0 => matches!(value, ColumnConstraint::NotNull { .. }),
                    1 => matches!(value, ColumnConstraint::Unique(..)),
                    _ => matches!(value, ColumnConstraint::Default(..)),
                };
                let previous_name = item
                    .constraints
                    .iter()
                    .find(|value| matches(&value.constraint))
                    .and_then(|value| value.name.clone());
                item.constraints.retain(|value| !matches(&value.constraint));
                item.constraints.extend(
                    replacement
                        .constraints
                        .iter()
                        .filter(|value| matches(&value.constraint))
                        .cloned()
                        .map(|mut value| {
                            value.name = previous_name.clone();
                            value
                        }),
                );
                if category == 1 {
                    if let Some(constraints) = constraints {
                        constraints.retain(|item| {
                            !matches!(&item.constraint, TableConstraint::Unique { columns, .. }
                            if columns.len() == 1 && column_expression(&columns[0].expr, &col.name))
                        });
                    }
                }
            }
            item
        } else {
            replacement
        };
        result.col_name = identifier(&col.name);
        edited.push(result);
    }
    let mut edited = edited.into_iter();
    let previous = std::mem::take(columns);
    for column in previous.into_values() {
        if column.flags.intersects(ColFlags::GENERATED) {
            columns.insert(column.col_name.clone(), column);
        } else if let Some(column) = edited.next() {
            columns.insert(column.col_name.clone(), column);
        }
    }
    for column in edited {
        columns.insert(column.col_name.clone(), column);
    }
    *temporary = false;
    *if_not_exists = false;
    *tbl_name = qualified(schema, temporary_name);
    Ok(emit(command))
}

pub fn qualify_dependency(sql: &str, schema: &str) -> Result<String, String> {
    let mut command = parse(sql)?;
    match &mut command {
        Cmd::Stmt(Stmt::CreateIndex { idx_name, .. }) => {
            idx_name.db_name = Some(identifier(schema))
        }
        Cmd::Stmt(Stmt::CreateTrigger {
            temporary,
            trigger_name,
            ..
        }) => {
            *temporary = false;
            trigger_name.db_name = Some(identifier(schema));
        }
        _ => return Err("Expected an index or trigger definition".into()),
    }
    Ok(emit(command))
}

pub fn trigger_table_schema(sql: &str) -> Result<Option<Name>, String> {
    let Cmd::Stmt(Stmt::CreateTrigger { tbl_name, .. }) = parse(sql)? else {
        return Err("Expected a trigger definition".into());
    };
    Ok(tbl_name.db_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn sqlite_schema_roundtrips_preserve_advanced_constraints_and_table_options() {
        for sql in [
            "CREATE TABLE t(id INTEGER PRIMARY KEY AUTOINCREMENT, v TEXT CONSTRAINT nn NOT NULL ON CONFLICT FAIL DEFAULT ('x') COLLATE NOCASE UNIQUE ON CONFLICT IGNORE CHECK(length(v)>0), p INTEGER REFERENCES parent(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED)",
            "CREATE TABLE t(a TEXT, b INTEGER, CONSTRAINT pk PRIMARY KEY(a DESC,b COLLATE NOCASE) ON CONFLICT REPLACE, CONSTRAINT uq UNIQUE(b) ON CONFLICT ABORT, CONSTRAINT ck CHECK(coalesce(b,0)>0), CONSTRAINT fk FOREIGN KEY(a) REFERENCES parent(id) ON UPDATE SET NULL MATCH SIMPLE DEFERRABLE INITIALLY DEFERRED) WITHOUT ROWID, STRICT",
            "CREATE TABLE t(a ANY, b INTEGER GENERATED ALWAYS AS(length(a)) STORED, c TEXT AS (a || ')') VIRTUAL) STRICT",
            "CREATE TABLE t(a, b DECIMAL(10,2), \"desc\" \"arbitrary type\")",
            "CREATE TABLE t(a TEXT PRIMARY KEY, b INTEGER) STRICT, WITHOUT ROWID",
            "CREATE TABLE t(\"constraint\" TEXT, \"primary\" BLOB, [列,名] TEXT CHECK([列,名] != ';,)'))",
            "CREATE TABLE t(asc TEXT, desc INTEGER, \"DEFERRABLE\" INT, [v] VARCHAR(5))",
        ] {
            let connection = Connection::open_in_memory().unwrap();
            connection.execute_batch("CREATE TABLE parent(id TEXT PRIMARY KEY)").unwrap();
            connection.execute_batch(sql).unwrap();
            let original = db::table_column_defs_in(&connection, "main", "t").unwrap();
            let sources: Vec<_> = original.iter().map(|column| Some(column.name.clone())).collect();
            let generated = rebuild_definition(sql, "main", "copy", &original, &original, &sources).unwrap();
            connection.execute_batch(&generated).unwrap();
            assert_eq!(db::table_column_names(&connection, "t").unwrap(), db::table_column_names(&connection, "copy").unwrap());
            assert_eq!(parse(&generated).unwrap(), parse(&emit(parse(&generated).unwrap())).unwrap());
        }
        assert!(parse("CREATE TABLE t(v); DROP TABLE t").is_err());
        assert!(column_definition(&ColDef::new("v", "TEXT NOT NULL"), None).is_err());
        assert!(column_definition(&ColDef::new("v", "TEXT, extra INT"), None).is_err());
        assert!(column_definition(
            &ColDef {
                default_value: "0 CHECK(v>0)".into(),
                ..ColDef::new("v", "INTEGER")
            },
            None
        )
        .is_err());
        assert!(column_definition(&ColDef::new("v", "\"type containing UNIQUE\""), None).is_ok());
        assert!(column_definition(
            &ColDef {
                pk: true,
                unique: true,
                ..ColDef::new("v", "INTEGER")
            },
            None
        )
        .is_ok());
        assert!(rebuild_definition(
            "CREATE VIRTUAL TABLE t USING fts5(v)",
            "main",
            "copy",
            &[],
            &[],
            &[]
        )
        .is_err());
    }
}
