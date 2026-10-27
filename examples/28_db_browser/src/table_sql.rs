//! Preserve SQLite constraints while changing the fields exposed by the table wizard.

use fallible_iterator::FallibleIterator;
use sqlite3_parser::{
    ast::{
        Cmd, ColFlags, ColumnConstraint, ColumnDefinition, CreateTableBody, Expr, Name,
        NamedTableConstraint, QualifiedName, SortedColumn, Stmt, TabFlags, TableConstraint,
    },
    lexer::sql::Parser,
};

use crate::db::{self, ColDef};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TableOptions {
    pub without_rowid: bool,
    pub strict: bool,
}

pub fn table_options(sql: &str) -> Result<TableOptions, String> {
    let Cmd::Stmt(Stmt::CreateTable {
        body: CreateTableBody::ColumnsAndConstraints { flags, .. },
        ..
    }) = parse(sql)?
    else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    Ok(TableOptions {
        without_rowid: flags.contains(TabFlags::WithoutRowid),
        strict: flags.contains(TabFlags::Strict),
    })
}

pub fn apply_table_options(sql: &str, options: TableOptions) -> Result<String, String> {
    let mut command = parse(sql)?;
    let Cmd::Stmt(Stmt::CreateTable {
        body: CreateTableBody::ColumnsAndConstraints { flags, .. },
        ..
    }) = &mut command
    else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    if flags.contains(TabFlags::WithoutRowid) == options.without_rowid
        && flags.contains(TabFlags::Strict) == options.strict
    {
        return Ok(sql.to_string());
    }
    flags.set(TabFlags::WithoutRowid, options.without_rowid);
    flags.set(TabFlags::Strict, options.strict);
    Ok(emit(command))
}

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

fn column_expression(expr: &Expr, name: &Name) -> bool {
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
    let mut actual = [0; 7];
    let definition = columns.into_values().next().unwrap();
    for value in &definition.constraints {
        if let ColumnConstraint::PrimaryKey { auto_increment, .. } = value.constraint {
            actual[4] += auto_increment as usize;
        }
        actual[match value.constraint {
            ColumnConstraint::PrimaryKey { .. } => 0,
            ColumnConstraint::NotNull { nullable: false, .. } => 1,
            ColumnConstraint::Unique(..) => 2,
            ColumnConstraint::Default(..) => 3,
            ColumnConstraint::Collate { .. } => 5,
            ColumnConstraint::Check(..) => 6,
            _ => return Err("Use the constraint controls rather than adding constraints to a type/default field".into()),
        }] += 1;
    }
    let expected = [
        column.pk as usize,
        column.notnull as usize,
        column.unique as usize,
        (!column.default_value.trim().is_empty()) as usize,
        column.auto_increment as usize,
        (!column.collation.is_empty()) as usize,
        (!column.check_expression.trim().is_empty()) as usize,
    ];
    if actual != expected {
        return Err("A type/default field cannot change other column constraints".into());
    }
    Ok(definition)
}

pub fn validate_primary_key(columns: &[ColDef]) -> Result<(), String> {
    let key_count = columns.iter().filter(|column| column.pk).count();
    if columns
        .iter()
        .any(|column| column.auto_increment && (!column.pk || key_count != 1))
    {
        return Err("Autoincrement requires one INTEGER primary-key column".into());
    }
    Ok(())
}

pub fn rebuild_definition(
    sql: &str,
    schema: &str,
    temporary_name: &str,
    original: &[ColDef],
    draft: &[ColDef],
    sources: &[Option<String>],
    options: Option<TableOptions>,
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
                flags,
            },
    }) = &mut command
    else {
        return Err("Modify Table requires an ordinary CREATE TABLE definition".into());
    };
    if let Some(options) = options {
        flags.set(TabFlags::WithoutRowid, options.without_rowid);
        flags.set(TabFlags::Strict, options.strict);
    }
    let key_changed = original.iter().any(|old| {
        old.pk
            != draft
                .iter()
                .zip(sources)
                .any(|(col, source)| source.as_ref() == Some(&old.name) && col.pk)
    }) || draft
        .iter()
        .zip(sources)
        .any(|(col, source)| source.is_none() && col.pk);
    let previous_key = constraints
        .as_ref()
        .and_then(|items| {
            items
                .iter()
                .find(|item| matches!(item.constraint, TableConstraint::PrimaryKey { .. }))
        })
        .cloned()
        .or_else(|| {
            columns.values().find_map(|col| {
                col.constraints.iter().find_map(|item| {
                    if let ColumnConstraint::PrimaryKey {
                        order,
                        conflict_clause,
                        auto_increment,
                    } = &item.constraint
                    {
                        Some(NamedTableConstraint {
                            name: item.name.clone(),
                            constraint: TableConstraint::PrimaryKey {
                                columns: vec![SortedColumn {
                                    expr: Expr::Name(col.col_name.clone()),
                                    order: *order,
                                    nulls: None,
                                }],
                                auto_increment: *auto_increment,
                                conflict_clause: *conflict_clause,
                            },
                        })
                    } else {
                        None
                    }
                })
            })
        });
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
            if old.check_expression != col.check_expression {
                // shortcut: this field edits the first column CHECK; expose every constraint in the advanced tab.
                let position = item
                    .constraints
                    .iter()
                    .position(|value| matches!(value.constraint, ColumnConstraint::Check(..)));
                let replacement = replacement
                    .constraints
                    .iter()
                    .find(|value| matches!(value.constraint, ColumnConstraint::Check(..)))
                    .cloned();
                match (position, replacement) {
                    (Some(position), Some(mut value)) => {
                        value.name = item.constraints[position].name.clone();
                        item.constraints[position] = value;
                    }
                    (Some(position), None) => {
                        item.constraints.remove(position);
                    }
                    (None, Some(value)) => item.constraints.push(value),
                    (None, None) => {}
                }
            }
            for category in 0..4 {
                let changed = match category {
                    0 => {
                        old.notnull != col.notnull
                            || (old.pk
                                && (!col.pk
                                    || !flags
                                        .intersects(TabFlags::Strict | TabFlags::WithoutRowid))
                                && col.notnull
                                && !item.constraints.iter().any(|value| {
                                    matches!(
                                        value.constraint,
                                        ColumnConstraint::NotNull {
                                            nullable: false,
                                            ..
                                        }
                                    )
                                }))
                    }
                    1 => old.unique != col.unique,
                    2 => old.default_value != col.default_value,
                    _ => !old.collation.eq_ignore_ascii_case(&col.collation),
                };
                if !changed {
                    continue;
                }
                let matches = |value: &ColumnConstraint| match category {
                    0 => matches!(value, ColumnConstraint::NotNull { .. }),
                    1 => matches!(value, ColumnConstraint::Unique(..)),
                    2 => matches!(value, ColumnConstraint::Default(..)),
                    _ => matches!(value, ColumnConstraint::Collate { .. }),
                };
                let mut previous = item
                    .constraints
                    .iter()
                    .filter(|value| matches(&value.constraint));
                let previous_name = (if category == 3 {
                    previous.next_back()
                } else {
                    previous.next()
                })
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
                            if columns.len() == 1 && column_expression(&columns[0].expr, &identifier(&col.name)))
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
    if key_changed {
        for col in columns.values_mut() {
            col.constraints
                .retain(|item| !matches!(item.constraint, ColumnConstraint::PrimaryKey { .. }));
        }
        let constraints = constraints.get_or_insert_with(Vec::new);
        constraints.retain(|item| !matches!(item.constraint, TableConstraint::PrimaryKey { .. }));
        let (name, mut keys, conflict) = match previous_key {
            Some(NamedTableConstraint {
                name,
                constraint:
                    TableConstraint::PrimaryKey {
                        columns,
                        conflict_clause,
                        ..
                    },
            }) => (name, columns, conflict_clause),
            _ => (None, Vec::new(), None),
        };
        keys.retain(|key| {
            draft
                .iter()
                .any(|col| col.pk && column_expression(&key.expr, &identifier(&col.name)))
        });
        for col in draft.iter().filter(|col| col.pk) {
            if !keys
                .iter()
                .any(|key| column_expression(&key.expr, &identifier(&col.name)))
            {
                keys.push(SortedColumn {
                    expr: Expr::Name(identifier(&col.name)),
                    order: None,
                    nulls: None,
                });
            }
        }
        if !keys.is_empty() {
            constraints.push(NamedTableConstraint {
                name,
                constraint: TableConstraint::PrimaryKey {
                    columns: keys,
                    auto_increment: false,
                    conflict_clause: conflict,
                },
            });
        }
    }
    if constraints.as_ref().is_some_and(Vec::is_empty) {
        *constraints = None;
    }
    for column in columns.values_mut() {
        for item in &mut column.constraints {
            if let ColumnConstraint::PrimaryKey { auto_increment, .. } = &mut item.constraint {
                *auto_increment = draft
                    .iter()
                    .any(|col| col.auto_increment && column.col_name.eq(col.name.as_str()));
            }
        }
    }
    for item in constraints.iter_mut().flatten() {
        if let TableConstraint::PrimaryKey { auto_increment, .. } = &mut item.constraint {
            *auto_increment = draft.iter().any(|col| col.auto_increment);
        }
    }
    *temporary = false;
    *if_not_exists = false;
    *tbl_name = qualified(schema, temporary_name);
    Ok(emit(command))
}

pub fn autoincrement_column(sql: &str) -> Result<Option<Name>, String> {
    let Cmd::Stmt(Stmt::CreateTable {
        body:
            CreateTableBody::ColumnsAndConstraints {
                columns,
                constraints,
                ..
            },
        ..
    }) = parse(sql)?
    else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    for col in columns.values() {
        if col.constraints.iter().any(|item| {
            matches!(
                item.constraint,
                ColumnConstraint::PrimaryKey {
                    auto_increment: true,
                    ..
                }
            )
        }) {
            return Ok(Some(col.col_name.clone()));
        }
    }
    for constraint in constraints.into_iter().flatten() {
        if let TableConstraint::PrimaryKey {
            columns: keys,
            auto_increment: true,
            ..
        } = constraint.constraint
        {
            if let [key] = keys.as_slice() {
                return Ok(columns
                    .values()
                    .find(|col| column_expression(&key.expr, &col.col_name))
                    .map(|col| col.col_name.clone()));
            }
        }
    }
    Ok(None)
}

pub fn read_column_constraints(sql: &str, definitions: &mut [ColDef]) -> Result<(), String> {
    let Cmd::Stmt(Stmt::CreateTable {
        body: CreateTableBody::ColumnsAndConstraints { columns, .. },
        ..
    }) = parse(sql)?
    else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    for definition in definitions {
        let column = columns
            .values()
            .find(|column| column.col_name.eq(definition.name.as_str()))
            .ok_or("Column definition no longer exists")?;
        definition.collation = column
            .constraints
            .iter()
            .rev()
            .find_map(|item| {
                if let ColumnConstraint::Collate { collation_name } = &item.constraint {
                    let value = collation_name.0.as_ref();
                    let name = match value.as_bytes().first() {
                        Some(b'[') => value[1..value.len() - 1].to_string(),
                        Some(quote @ (b'\'' | b'"' | b'`')) => value[1..value.len() - 1].replace(
                            &format!("{0}{0}", *quote as char),
                            &(*quote as char).to_string(),
                        ),
                        _ => value.to_string(),
                    };
                    Some(name)
                } else {
                    None
                }
            })
            .unwrap_or_default();
        definition.check_expression = column
            .constraints
            .iter()
            .find_map(|item| {
                if let ColumnConstraint::Check(expression) = &item.constraint {
                    Some(expression.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default();
    }
    Ok(())
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
            let generated = rebuild_definition(sql, "main", "copy", &original, &original, &sources, None).unwrap();
            connection.execute_batch(&generated).unwrap();
            assert_eq!(db::table_column_names(&connection, "t").unwrap(), db::table_column_names(&connection, "copy").unwrap());
            assert_eq!(parse(&generated).unwrap(), parse(&emit(parse(&generated).unwrap())).unwrap());
        }
        assert!(parse("CREATE TABLE t(v); DROP TABLE t").is_err());
        assert!(column_definition(&ColDef::new("v", "TEXT NOT NULL"), None).is_err());
        assert!(column_definition(&ColDef::new("v", "TEXT COLLATE NOCASE"), None).is_err());
        let mut columns = [
            ColDef::new("v", "TEXT"),
            ColDef::new("w", "TEXT"),
            ColDef::new("x", "TEXT"),
        ];
        read_column_constraints("CREATE TABLE t(v TEXT COLLATE [NOCASE],w TEXT COLLATE `RTRIM`,x TEXT COLLATE 'cus''tom')", &mut columns).unwrap();
        assert_eq!(
            columns.map(|column| column.collation),
            ["NOCASE", "RTRIM", "cus'tom"]
        );
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
            &[],
            None
        )
        .is_err());
    }
}
