//! Preserve SQLite constraints while changing the fields exposed by the table wizard.

use fallible_iterator::FallibleIterator;
use sqlite3_parser::{
    ast::{
        fmt::ToTokens, Cmd, ColumnConstraint, ColumnDefinition, CreateTableBody, Expr,
        Name, NamedColumnConstraint, NamedTableConstraint, QualifiedName, ResolveType,
        SortedColumn, SortOrder, Stmt, TabFlags, TableConstraint,
    },
    lexer::{
        sql::{Parser, TokenType, Tokenizer},
        Scanner,
    },
};

use crate::db::{self, ColDef};

struct SqlFragment<T>(T);

impl<T: ToTokens> std::fmt::Display for SqlFragment<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.to_fmt(formatter)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TableOptions {
    pub without_rowid: bool,
    pub strict: bool,
    pub primary_key_conflict: Option<ResolveType>,
    pub checks: Option<Vec<NamedCheck>>,
    pub inline_checks: Option<Vec<NamedCheck>>,
    pub foreign_keys: Option<Vec<NamedForeignKey>>,
    pub inline_foreign_keys: Option<Vec<NamedForeignKey>>,
    pub index_constraints: Option<Vec<NamedIndexConstraint>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NamedCheck {
    pub column: Option<String>,
    pub name: Option<String>,
    pub expression: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NamedForeignKey {
    pub name: Option<String>,
    pub columns: String,
    pub clause: String,
    pub inline: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NamedIndexConstraint {
    pub name: Option<String>,
    pub columns: String,
    pub primary: bool,
    pub conflict: Option<ResolveType>,
    pub auto_increment: bool,
    // INTEGER PRIMARY KEY DESC has different rowid semantics when declared inline.
    pub inline: bool,
}

pub const PRIMARY_KEY_CONFLICTS: [(&str, Option<ResolveType>); 6] = [
    ("Default", None),
    ("ROLLBACK", Some(ResolveType::Rollback)),
    ("ABORT", Some(ResolveType::Abort)),
    ("FAIL", Some(ResolveType::Fail)),
    ("IGNORE", Some(ResolveType::Ignore)),
    ("REPLACE", Some(ResolveType::Replace)),
];

pub fn table_options(sql: &str) -> Result<TableOptions, String> {
    table_options_of(&parse(sql)?)
}

fn table_options_of(command: &Cmd) -> Result<TableOptions, String> {
    let Cmd::Stmt(Stmt::CreateTable {
        body:
            CreateTableBody::ColumnsAndConstraints {
                flags,
                columns,
                constraints,
            },
        ..
    }) = command
    else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    let mut primary_key_conflict = None;
    for column in columns.values() {
        for item in &column.constraints {
            if let ColumnConstraint::PrimaryKey {
                conflict_clause, ..
            } = &item.constraint
            {
                primary_key_conflict = *conflict_clause;
            }
        }
    }
    for item in constraints.iter().flatten() {
        if let TableConstraint::PrimaryKey {
            conflict_clause, ..
        } = &item.constraint
        {
            primary_key_conflict = *conflict_clause;
        }
    }
    Ok(TableOptions {
        without_rowid: flags.contains(TabFlags::WithoutRowid),
        strict: flags.contains(TabFlags::Strict),
        primary_key_conflict,
        checks: None,
        inline_checks: None,
        foreign_keys: None,
        inline_foreign_keys: None,
        index_constraints: None,
    })
}

fn set_primary_key_conflict(command: &mut Cmd, action: Option<ResolveType>) -> Result<(), String> {
    let Cmd::Stmt(Stmt::CreateTable {
        body:
            CreateTableBody::ColumnsAndConstraints {
                columns,
                constraints,
                ..
            },
        ..
    }) = command
    else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    let mut found = false;
    for column in columns.values_mut() {
        for item in &mut column.constraints {
            if let ColumnConstraint::PrimaryKey {
                conflict_clause, ..
            } = &mut item.constraint
            {
                *conflict_clause = action;
                found = true;
            }
        }
    }
    for item in constraints.iter_mut().flatten() {
        if let TableConstraint::PrimaryKey {
            conflict_clause, ..
        } = &mut item.constraint
        {
            *conflict_clause = action;
            found = true;
        }
    }
    if action.is_some() && !found {
        return Err("Select a primary-key column before choosing its conflict action".into());
    }
    Ok(())
}

pub fn apply_table_options(sql: &str, options: TableOptions) -> Result<String, String> {
    let mut command = parse(sql)?;
    if table_options_of(&command)? == options {
        return Ok(sql.to_string());
    }
    let Cmd::Stmt(Stmt::CreateTable {
        body: CreateTableBody::ColumnsAndConstraints { flags, .. },
        ..
    }) = &mut command
    else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    flags.set(TabFlags::WithoutRowid, options.without_rowid);
    flags.set(TabFlags::Strict, options.strict);
    if let Some(keys) = options.index_constraints { set_index_constraints(&mut command, &keys)?; }
    else { set_primary_key_conflict(&mut command, options.primary_key_conflict)?; }
    if let Some(checks) = options.checks { set_table_checks(&mut command, &checks)?; }
    if let Some(checks) = options.inline_checks { set_inline_checks(&mut command, &checks)?; }
    if let Some(keys) = options.foreign_keys { set_table_foreign_keys(&mut command, &keys)?; }
    if let Some(keys) = options.inline_foreign_keys { set_inline_foreign_keys(&mut command, &keys)?; }
    Ok(emit(command))
}

fn identifier_text(name: &Name) -> String {
    let value = name.0.as_ref();
    match value.as_bytes().first() {
        Some(b'[') => value[1..value.len() - 1].to_string(),
        Some(quote @ (b'\'' | b'"' | b'`')) => value[1..value.len() - 1].replace(&format!("{0}{0}", *quote as char), &(*quote as char).to_string()),
        _ => value.to_string(),
    }
}

pub fn table_checks(sql: &str) -> Result<Vec<NamedCheck>, String> {
    let Cmd::Stmt(Stmt::CreateTable { body: CreateTableBody::ColumnsAndConstraints { constraints, .. }, .. }) = parse(sql)? else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    Ok(constraints.into_iter().flatten().filter_map(|item| {
        if let TableConstraint::Check(expression) = item.constraint {
            Some(NamedCheck { column:None, name: item.name.as_ref().map(identifier_text), expression: expression.to_string() })
        } else { None }
    }).collect())
}

pub fn rename_checks(checks: &[NamedCheck], table: &str, target: &str, renames: &[(&str, &str)]) -> Result<Vec<NamedCheck>, String> {
    checks.iter().map(|check| {
        let mut definition = check_definition(check)?;
        let TableConstraint::Check(expression) = &mut definition.constraint else { unreachable!() };
        let original = expression.to_string();
        rename_table_expression(expression, &identifier(table), Some(target), renames)?;
        let expression = expression.to_string();
        let mut result = check.clone();
        if expression != original { result.expression = expression; }
        if let Some(owner) = &mut result.column {
            let mut name = identifier(owner); rename_identifier(&mut name, renames); *owner = identifier_text(&name);
        }
        Ok(result)
    }).collect()
}

fn check_definition(check: &NamedCheck) -> Result<NamedTableConstraint, String> {
    if check.expression.trim().is_empty() { return Err("A CHECK requires an expression".into()); }
    let name = check.name.as_ref().map(|name| format!("CONSTRAINT {} ", db::quote_ident(name))).unwrap_or_default();
    let sql = format!("CREATE TABLE __next_gui_check(v, {name}CHECK({}\n));", check.expression.trim());
    let Cmd::Stmt(Stmt::CreateTable { body: CreateTableBody::ColumnsAndConstraints { columns, constraints: Some(mut constraints), flags }, .. }) = parse(&sql)? else {
        return Err("A CHECK field requires one expression".into());
    };
    if columns.len() != 1 || constraints.len() != 1 || !flags.is_empty() || !matches!(constraints[0].constraint, TableConstraint::Check(_)) {
        return Err("A CHECK field cannot add columns, other constraints or table options".into());
    }
    Ok(constraints.remove(0))
}

pub fn check_sql(check: &NamedCheck) -> Result<String, String> {
    Ok(SqlFragment(check_definition(check)?).to_string())
}

fn set_table_checks(command: &mut Cmd, checks: &[NamedCheck]) -> Result<(), String> {
    if checks.iter().any(|check| check.column.is_some()) { return Err("Column CHECKs require a column-constraint override".into()); }
    replace_table_constraints(command, checks.iter().map(check_definition).collect::<Result<Vec<_>, _>>()?, |constraint| matches!(constraint, TableConstraint::Check(_)))
}

pub fn inline_checks(sql: &str) -> Result<Vec<NamedCheck>, String> {
    let Cmd::Stmt(Stmt::CreateTable { body: CreateTableBody::ColumnsAndConstraints { columns, .. }, .. }) = parse(sql)? else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    Ok(columns.values().flat_map(|column| column.constraints.iter().filter_map(|item| {
        let ColumnConstraint::Check(expression) = &item.constraint else { return None; };
        Some(NamedCheck { column:Some(identifier_text(&column.col_name)), name:item.name.as_ref().map(identifier_text), expression:expression.to_string() })
    })).collect())
}

pub fn checks_can_swap(a: &NamedCheck, b: &NamedCheck) -> bool {
    match (&a.column, &b.column) { (None,None) => true, (Some(a),Some(b)) => a.eq_ignore_ascii_case(b), _ => false }
}

fn set_inline_checks(command: &mut Cmd, checks: &[NamedCheck]) -> Result<(), String> {
    let replacements = checks.iter().map(|check| {
        let owner = check.column.as_deref().filter(|owner| !owner.is_empty()).ok_or("A column CHECK requires its owner column")?;
        let definition = check_definition(check)?;
        let TableConstraint::Check(expression) = definition.constraint else { unreachable!() };
        Ok((owner,NamedColumnConstraint { name:definition.name, constraint:ColumnConstraint::Check(expression) }))
    }).collect::<Result<Vec<_>,String>>()?;
    let Cmd::Stmt(Stmt::CreateTable { body: CreateTableBody::ColumnsAndConstraints { columns, .. }, .. }) = command else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    for (owner,_) in &replacements {
        if !columns.values().any(|column| column.col_name.eq(*owner)) { return Err(format!("Unknown column CHECK owner: {owner}")); }
    }
    for column in columns.values_mut() {
        let mut replacements = replacements.iter().filter(|(owner,_)| column.col_name.eq(*owner)).map(|(_,item)| item.clone());
        let mut result = Vec::new();
        for item in std::mem::take(&mut column.constraints) {
            if matches!(item.constraint,ColumnConstraint::Check(..)) { if let Some(item) = replacements.next() { result.push(item); } }
            else { result.push(item); }
        }
        result.extend(replacements); column.constraints = result;
    }
    Ok(())
}

fn foreign_key_fields(item: &NamedTableConstraint) -> Option<NamedForeignKey> {
    let TableConstraint::ForeignKey { columns, clause, deref_clause } = &item.constraint else { return None; };
    Some(NamedForeignKey {
        name: item.name.as_ref().map(identifier_text),
        columns: columns.iter().map(|column| SqlFragment(&column.col_name).to_string()).collect::<Vec<_>>().join(", "),
        clause: format!("REFERENCES {}{}", SqlFragment(clause), deref_clause.as_ref().map(|defer| format!(" {}", SqlFragment(defer))).unwrap_or_default()),
        inline: false,
    })
}

pub fn table_foreign_keys(sql: &str) -> Result<Vec<NamedForeignKey>, String> {
    let Cmd::Stmt(Stmt::CreateTable { body: CreateTableBody::ColumnsAndConstraints { constraints, .. }, .. }) = parse(sql)? else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    Ok(constraints.iter().flatten().filter_map(foreign_key_fields).collect())
}

fn foreign_key_definition(key: &NamedForeignKey) -> Result<NamedTableConstraint, String> {
    if key.columns.trim().is_empty() { return Err("A foreign key requires at least one local column".into()); }
    if key.clause.trim().is_empty() { return Err("A foreign key requires a REFERENCES clause".into()); }
    let name = key.name.as_ref().map(|name| format!("CONSTRAINT {} ", db::quote_ident(name))).unwrap_or_default();
    let sql = format!("CREATE TABLE __next_gui_fk(v, {name}FOREIGN KEY({}\n) {}\n);", key.columns.trim(), key.clause.trim());
    let Cmd::Stmt(Stmt::CreateTable { body: CreateTableBody::ColumnsAndConstraints { columns, constraints: Some(mut constraints), flags }, .. }) = parse(&sql)? else {
        return Err("A foreign-key field requires one FOREIGN KEY definition".into());
    };
    if columns.len() != 1 || constraints.len() != 1 || !flags.is_empty() { return Err("Foreign-key fields cannot add columns, other constraints or table options".into()); }
    let TableConstraint::ForeignKey { columns, clause, .. } = &constraints[0].constraint else { return Err("Expected a FOREIGN KEY definition".into()); };
    if columns.is_empty() || columns.iter().chain(clause.columns.iter().flatten()).any(|column| column.order.is_some() || column.collation_name.is_some()) {
        return Err("Foreign-key columns must be column names without sorting or collations".into());
    }
    if clause.columns.as_ref().is_some_and(|parent| parent.len() != columns.len()) { return Err("Local and referenced foreign-key column counts must match".into()); }
    Ok(constraints.remove(0))
}

pub fn foreign_key_sql(key: &NamedForeignKey) -> Result<String, String> {
    if key.inline {
        let (_,constraints)=inline_foreign_key_definition(key)?;
        Ok(constraints.into_iter().map(|constraint|SqlFragment(constraint).to_string()).collect::<Vec<_>>().join(" "))
    } else {Ok(SqlFragment(foreign_key_definition(key)?).to_string())}
}

pub fn foreign_key_columns(key: &NamedForeignKey) -> Result<Vec<(String, String)>, String> {
    // Local-column selection must also work while the REFERENCES draft is incomplete.
    let local = NamedForeignKey { name: None, columns: key.columns.clone(), clause: "REFERENCES __next_gui_parent".into(), inline: false };
    let NamedTableConstraint { constraint: TableConstraint::ForeignKey { columns, .. }, .. } = foreign_key_definition(&local)? else { unreachable!() };
    Ok(columns.into_iter().map(|column| (identifier_text(&column.col_name), SqlFragment(column.col_name).to_string())).collect())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForeignKeyReference {
    pub table: String,
    pub columns: String,
    pub suffix: String,
}

#[test]
fn reference_parts_preserve_composites_named_deferrals_and_reject_other_sql() {
    for sql in ["REFERENCES \"a.b\"(\"x,y\", \"z\"\"key\") ON DELETE SET NULL ON UPDATE CASCADE DEFERRABLE INITIALLY DEFERRED",
        "REFERENCES parent MATCH simple ON UPDATE NO ACTION",
        "REFERENCES parent(id) CONSTRAINT \"later check\" DEFERRABLE INITIALLY DEFERRED"] {
        let parts=foreign_key_reference(sql).unwrap();
        assert_eq!(foreign_key_reference(&parts.sql()).unwrap(),parts);
    }
    assert_eq!(foreign_key_reference("REFERENCES \"a.b\"(\"x,y\", \"z\"\"key\")").unwrap().table,"a.b");
    assert_eq!(foreign_key_reference("REFERENCES parent").unwrap().columns,"");
    for sql in ["REFERENCES p(id) CHECK(v>0)","REFERENCES p(id), extra TEXT","REFERENCES p(id)) STRICT;--",
        "TEXT REFERENCES p(id)","DEFAULT 1 REFERENCES p(id)","REFERENCES p(id DESC)","REFERENCES p(id); DROP TABLE records;--"] {
        assert!(foreign_key_reference(sql).is_err(),"{sql}");
    }
}

pub fn foreign_key_reference(sql: &str) -> Result<ForeignKeyReference, String> {
    if sql.trim().is_empty() { return Ok(ForeignKeyReference::default()); }
    // Parse independently of local arity so an incomplete composite draft remains editable.
    let Cmd::Stmt(Stmt::CreateTable { body: CreateTableBody::ColumnsAndConstraints { columns, constraints, flags }, .. }) =
        parse(&format!("CREATE TABLE __next_gui_reference(v BLOB {sql}\n)"))? else {
        return Err("Expected a REFERENCES clause".into());
    };
    if columns.len()!=1 || constraints.as_ref().is_some_and(|items|!items.is_empty()) || !flags.is_empty() {
        return Err("A reference cannot add columns, constraints or table options".into());
    }
    let column=columns.values().next().unwrap();
    if column.col_type.as_ref().is_none_or(|kind|kind.name!="BLOB" || kind.size.is_some()) { return Err("Expected a REFERENCES clause".into()); }
    let Some(first)=column.constraints.first() else {return Err("Expected a REFERENCES clause".into());};
    let ColumnConstraint::ForeignKey {clause,deref_clause}=&first.constraint else {return Err("Expected a REFERENCES clause".into());};
    if first.name.is_some() || column.constraints.iter().skip(1).any(|item|!matches!(item.constraint,ColumnConstraint::Defer(..)))
        || clause.columns.iter().flatten().any(|column|column.order.is_some() || column.collation_name.is_some()) {
        return Err("A reference requires column names and foreign-key clauses only".into());
    }
    let mut suffix=clause.args.iter().map(|arg|SqlFragment(arg).to_string()).collect::<Vec<_>>();
    if let Some(defer)=deref_clause {suffix.push(SqlFragment(defer).to_string());}
    suffix.extend(column.constraints.iter().skip(1).map(|item|SqlFragment(item).to_string()));
    Ok(ForeignKeyReference { table:identifier_text(&clause.tbl_name),
        columns:clause.columns.iter().flatten().map(|column|SqlFragment(&column.col_name).to_string()).collect::<Vec<_>>().join(", "),
        suffix:suffix.join(" "), })
}

impl ForeignKeyReference {
    pub fn sql(&self) -> String {
        if self.table.is_empty() {return String::new();}
        format!("REFERENCES {}{}{}",db::quote_ident(&self.table),
            if self.columns.trim().is_empty() {String::new()} else {format!("({}\n)",self.columns)},
            if self.suffix.trim().is_empty() {String::new()} else {format!(" {}",self.suffix)})
    }
}

pub fn rename_foreign_keys(keys: &[NamedForeignKey], table: &str, target: &str, renames: &[(&str, &str)]) -> Result<Vec<NamedForeignKey>, String> {
    keys.iter().map(|key| {
        if key.inline {
            let (mut column, mut constraints) = inline_foreign_key_definition(key)?;
            let original_column = column.clone();
            let original_clause = constraints.iter().map(|item| SqlFragment(item).to_string()).collect::<Vec<_>>().join(" ");
            rename_identifier(&mut column, renames);
            for item in &mut constraints {
                if let ColumnConstraint::ForeignKey { clause, .. } = &mut item.constraint {
                    if clause.tbl_name.eq(table) {
                        clause.tbl_name = identifier(target);
                        for column in clause.columns.iter_mut().flatten() { rename_identifier(&mut column.col_name, renames); }
                    }
                }
            }
            let changed_clause = constraints.iter().map(|item| SqlFragment(item).to_string()).collect::<Vec<_>>().join(" ") != original_clause;
            let mut result = inline_foreign_key_fields(&column, &constraints);
            if column.0 == original_column.0 { result.columns = key.columns.clone(); }
            if !changed_clause { result.clause = key.clause.clone(); }
            return Ok(result);
        }
        let mut definition = foreign_key_definition(key)?;
        let original = SqlFragment(&definition).to_string();
        let TableConstraint::ForeignKey { columns, clause, .. } = &mut definition.constraint else { unreachable!() };
        for column in columns { rename_identifier(&mut column.col_name, renames); }
        if clause.tbl_name.eq(table) {
            clause.tbl_name = identifier(target);
            for column in clause.columns.iter_mut().flatten() { rename_identifier(&mut column.col_name, renames); }
        }
        if SqlFragment(&definition).to_string() == original { return Ok(key.clone()); }
        Ok(foreign_key_fields(&definition).unwrap())
    }).collect()
}

fn set_table_foreign_keys(command: &mut Cmd, keys: &[NamedForeignKey]) -> Result<(), String> {
    if keys.iter().any(|key| key.inline) { return Err("Column foreign keys require a column-constraint override".into()); }
    replace_table_constraints(command, keys.iter().map(foreign_key_definition).collect::<Result<Vec<_>, _>>()?, |constraint| matches!(constraint, TableConstraint::ForeignKey { .. }))
}

fn inline_foreign_key_fields(column: &Name, constraints: &[NamedColumnConstraint]) -> NamedForeignKey {
    NamedForeignKey {
        name: constraints[0].name.as_ref().map(identifier_text), columns: SqlFragment(column).to_string(), inline: true,
        clause: std::iter::once(SqlFragment(&constraints[0].constraint).to_string())
            .chain(constraints[1..].iter().map(|item| SqlFragment(item).to_string())).collect::<Vec<_>>().join(" "),
    }
}

pub fn inline_foreign_keys(sql: &str) -> Result<Vec<NamedForeignKey>, String> {
    let Cmd::Stmt(Stmt::CreateTable { body: CreateTableBody::ColumnsAndConstraints { columns, .. }, .. }) = parse(sql)? else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    Ok(columns.values().flat_map(|column| foreign_key_groups(&column.constraints).into_iter().map(|group| {
        let constraints = group.into_iter().map(|index| column.constraints[index].clone()).collect::<Vec<_>>();
        inline_foreign_key_fields(&column.col_name, &constraints)
    })).collect())
}

pub fn inline_foreign_key_column(key: &NamedForeignKey) -> Result<String, String> {
    let definition = foreign_key_definition(&NamedForeignKey { name: None, columns: key.columns.clone(), clause: "REFERENCES __next_gui_parent".into(), inline: false })?;
    let TableConstraint::ForeignKey { columns, .. } = definition.constraint else { unreachable!() };
    let [column] = columns.as_slice() else { return Err("A column foreign key requires exactly one local column; add a table foreign key for a composite reference".into()); };
    Ok(identifier_text(&column.col_name))
}

pub fn foreign_keys_can_swap(a: &NamedForeignKey, b: &NamedForeignKey) -> bool {
    if !a.inline && !b.inline { return true; }
    if !a.inline || !b.inline { return false; }
    inline_foreign_key_column(a).ok().zip(inline_foreign_key_column(b).ok()).is_some_and(|(a,b)| a.eq_ignore_ascii_case(&b))
}

pub fn normalize_foreign_key_scope(key: &mut NamedForeignKey) {
    if key.inline {
        if let Ok(NamedTableConstraint { constraint: TableConstraint::ForeignKey { columns, .. }, .. }) = foreign_key_definition(key) {
            if columns.len() > 1 { key.inline = false; }
        }
    }
}

fn inline_foreign_key_definition(key: &NamedForeignKey) -> Result<(Name, Vec<NamedColumnConstraint>), String> {
    let column = inline_foreign_key_column(key)?;
    let name = key.name.as_ref().map(|name| format!("CONSTRAINT {} ", db::quote_ident(name))).unwrap_or_default();
    let sql = format!("CREATE TABLE __next_gui_column_fk({} BLOB {name}{}\n)", db::quote_ident(&column), key.clause.trim());
    let Cmd::Stmt(Stmt::CreateTable { body: CreateTableBody::ColumnsAndConstraints { columns, constraints, flags }, .. }) = parse(&sql)? else {
        return Err("Expected one column REFERENCES clause".into());
    };
    if columns.len() != 1 || constraints.as_ref().is_some_and(|items| !items.is_empty()) || !flags.is_empty() { return Err("A column foreign-key field cannot add columns, table constraints or options".into()); }
    let definition = columns.values().next().unwrap();
    let Some(first) = definition.constraints.first() else { return Err("A column foreign key requires a REFERENCES clause".into()); };
    let ColumnConstraint::ForeignKey { clause, .. } = &first.constraint else { return Err("A column foreign-key field must begin with REFERENCES".into()); };
    if definition.constraints.iter().skip(1).any(|item| !matches!(item.constraint, ColumnConstraint::Defer(..)))
        || first.name.as_ref().map(identifier_text) != key.name { return Err("A column foreign-key field requires one REFERENCES clause and optional named deferrals".into()); }
    if clause.columns.as_ref().is_some_and(|columns| columns.len() != 1 || columns.iter().any(|column| column.order.is_some() || column.collation_name.is_some())) {
        return Err("A column foreign key requires one referenced column without sorting or collations".into());
    }
    Ok((definition.col_name.clone(), definition.constraints.clone()))
}

fn set_inline_foreign_keys(command: &mut Cmd, keys: &[NamedForeignKey]) -> Result<(), String> {
    if keys.iter().any(|key| !key.inline) { return Err("A column-constraint override requires column foreign keys".into()); }
    let replacements = keys.iter().map(inline_foreign_key_definition).collect::<Result<Vec<_>, _>>()?;
    let Cmd::Stmt(Stmt::CreateTable { body: CreateTableBody::ColumnsAndConstraints { columns, .. }, .. }) = command else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    for (name, _) in &replacements {
        if !columns.values().any(|column| column.col_name.eq(name)) { return Err(format!("Unknown column foreign-key owner: {}", identifier_text(name))); }
    }
    for column in columns.values_mut() {
        let groups = foreign_key_groups(&column.constraints);
        let mut replacements = replacements.iter().filter(|(name, _)| name.eq(&column.col_name)).map(|(_, constraints)| constraints.clone());
        let mut result = Vec::new();
        // Deferrals belong to the preceding FK even across unrelated column constraints.
        for (index, item) in std::mem::take(&mut column.constraints).into_iter().enumerate() {
            if groups.iter().any(|group| group[0] == index) { if let Some(group) = replacements.next() { result.extend(group); } }
            if !groups.iter().flatten().any(|position| *position == index) { result.push(item); }
        }
        result.extend(replacements.flatten()); column.constraints = result;
    }
    Ok(())
}

fn index_constraint_fields(item: &NamedTableConstraint, inline: bool) -> Option<NamedIndexConstraint> {
    let (columns, conflict, primary, auto_increment) = match &item.constraint {
        TableConstraint::PrimaryKey { columns, conflict_clause, auto_increment } => (columns, *conflict_clause, true, *auto_increment),
        TableConstraint::Unique { columns, conflict_clause } => (columns, *conflict_clause, false, false),
        _ => return None,
    };
    Some(NamedIndexConstraint { name: item.name.as_ref().map(identifier_text),
        columns: columns.iter().map(|column| SqlFragment(column).to_string()).collect::<Vec<_>>().join(", "),
        primary, conflict, auto_increment, inline })
}

pub fn table_index_constraints(sql: &str) -> Result<Vec<NamedIndexConstraint>, String> {
    let Cmd::Stmt(Stmt::CreateTable { body: CreateTableBody::ColumnsAndConstraints { columns, constraints, .. }, .. }) = parse(sql)? else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    let mut result = Vec::new();
    for column in columns.values() {
        for item in &column.constraints {
            let constraint = match &item.constraint {
                ColumnConstraint::PrimaryKey { order, conflict_clause, auto_increment } => TableConstraint::PrimaryKey {
                    columns: vec![SortedColumn { expr: Expr::Name(column.col_name.clone()), order: *order, nulls: None }],
                    conflict_clause: *conflict_clause, auto_increment: *auto_increment },
                ColumnConstraint::Unique(conflict_clause) => TableConstraint::Unique {
                    columns: vec![SortedColumn { expr: Expr::Name(column.col_name.clone()), order: None, nulls: None }], conflict_clause: *conflict_clause },
                _ => continue,
            };
            result.push(index_constraint_fields(&NamedTableConstraint { name: item.name.clone(), constraint }, true).unwrap());
        }
    }
    result.extend(constraints.iter().flatten().filter_map(|item| index_constraint_fields(item, false)));
    result.sort_by_key(|key| !index_constraint_requires_inline(key));
    Ok(result)
}

pub fn index_constraint_requires_inline(key: &NamedIndexConstraint) -> bool {
    if !key.inline || !key.primary || key.auto_increment { return false; }
    match index_constraint_definition(key) {
        Ok(NamedTableConstraint { constraint: TableConstraint::PrimaryKey { columns, .. }, .. }) => {
            matches!(columns.as_slice(), [SortedColumn { expr: Expr::Name(_) | Expr::Id(_), order: Some(SortOrder::Desc), .. }])
        }
        _ => true,
    }
}

fn index_constraint_definition(key: &NamedIndexConstraint) -> Result<NamedTableConstraint, String> {
    if key.columns.trim().is_empty() { return Err("An index constraint requires at least one column".into()); }
    if key.auto_increment && !key.primary { return Err("Autoincrement requires an INTEGER primary key".into()); }
    let name = key.name.as_ref().map(|name| format!("CONSTRAINT {} ", db::quote_ident(name))).unwrap_or_default();
    let kind = if key.primary { "PRIMARY KEY" } else { "UNIQUE" };
    let auto = if key.auto_increment { " AUTOINCREMENT" } else { "" };
    let conflict = key.conflict.map(|action| format!(" ON CONFLICT {}", SqlFragment(action))).unwrap_or_default();
    let sql = format!("CREATE TABLE __next_gui_key(v, {name}{kind}({}\n{auto}){conflict});", key.columns.trim());
    let Cmd::Stmt(Stmt::CreateTable { body: CreateTableBody::ColumnsAndConstraints { columns, constraints: Some(mut constraints), flags }, .. }) = parse(&sql)? else {
        return Err("An index-constraint field requires one PRIMARY KEY or UNIQUE definition".into());
    };
    if columns.len() != 1 || constraints.len() != 1 || !(flags - (TabFlags::HasPrimaryKey | TabFlags::Autoincrement)).is_empty() { return Err("Index-constraint fields cannot add columns, other constraints or table options".into()); }
    let keys = match &constraints[0].constraint {
        TableConstraint::PrimaryKey { columns, auto_increment, conflict_clause } if key.primary && *auto_increment == key.auto_increment && *conflict_clause == key.conflict => columns,
        TableConstraint::Unique { columns, .. } if !key.primary => columns,
        _ => return Err("Expected a PRIMARY KEY or UNIQUE definition".into()),
    };
    if keys.is_empty() || keys.iter().any(|key| key.nulls.is_some() || index_column_name(&key.expr).is_none()) {
        return Err("Index constraints require column names with optional COLLATE and ASC/DESC".into());
    }
    if key.inline && key.primary && !key.auto_increment
        && matches!(keys.as_slice(), [SortedColumn { expr: Expr::Collate(..), order: Some(SortOrder::Desc), .. }]) {
        return Err("Set the column collation in Fields to retain an inline PRIMARY KEY DESC's rowid semantics".into());
    }
    Ok(constraints.remove(0))
}

fn inline_index_constraint(key: &NamedIndexConstraint, definition: &NamedTableConstraint) -> Option<(Name, NamedColumnConstraint)> {
    let (members,auto_increment,conflict_clause)=match &definition.constraint {
        TableConstraint::PrimaryKey {columns,auto_increment,conflict_clause}=>(columns,*auto_increment,*conflict_clause),
        TableConstraint::Unique {columns,conflict_clause}=>(columns,false,*conflict_clause),
        _=>return None,
    };
    let [member]=members.as_slice() else {return None;};
    if !key.inline || !matches!(member.expr,Expr::Name(_)|Expr::Id(_)) || (!key.primary && member.order.is_some())
        || (auto_increment && member.order==Some(SortOrder::Desc)) {return None;}
    let constraint=if key.primary {ColumnConstraint::PrimaryKey {order:member.order,auto_increment,conflict_clause}}
        else {ColumnConstraint::Unique(conflict_clause)};
    Some((index_column_name(&member.expr)?,NamedColumnConstraint {name:definition.name.clone(),constraint}))
}

pub fn index_constraint_sql(key: &NamedIndexConstraint) -> Result<String, String> {
    let definition=index_constraint_definition(key)?;
    Ok(if let Some((_,inline))=inline_index_constraint(key,&definition) {SqlFragment(inline).to_string()}
        else {SqlFragment(definition).to_string()})
}

pub fn index_constraint_columns(key: &NamedIndexConstraint) -> Result<Vec<(String, String)>, String> {
    let columns = match index_constraint_definition(key)?.constraint {
        TableConstraint::PrimaryKey { columns, .. } | TableConstraint::Unique { columns, .. } => columns,
        _ => unreachable!(),
    };
    Ok(columns.into_iter().map(|column| (identifier_text(&index_column_name(&column.expr).unwrap()), SqlFragment(column).to_string())).collect())
}

fn index_column_name(expr: &Expr) -> Option<Name> {
    match expr {
        Expr::Name(name) => Some(name.clone()),
        Expr::Id(name) => Some(Name(name.0.clone())),
        Expr::Literal(sqlite3_parser::ast::Literal::String(name)) => Some(Name(name.clone())),
        Expr::Collate(expr, _) => index_column_name(expr),
        _ => None,
    }
}

fn rename_index_column(expr: &mut Expr, renames: &[(&str, &str)]) {
    if let Expr::Collate(expr, _) = expr { rename_index_column(expr, renames); }
    else if let Some(name) = index_column_name(expr) {
        if let Some((_, target)) = renames.iter().find(|(source, _)| name.eq(*source)) { *expr = Expr::Name(identifier(target)); }
    }
}

pub fn rename_index_constraints(keys: &[NamedIndexConstraint], renames: &[(&str, &str)]) -> Result<Vec<NamedIndexConstraint>, String> {
    keys.iter().map(|key| {
        let mut definition = index_constraint_definition(key)?;
        let original = SqlFragment(&definition).to_string();
        let columns = match &mut definition.constraint {
            TableConstraint::PrimaryKey { columns, .. } | TableConstraint::Unique { columns, .. } => columns,
            _ => unreachable!(),
        };
        for column in columns { rename_index_column(&mut column.expr, renames); }
        if SqlFragment(&definition).to_string() == original { return Ok(key.clone()); }
        Ok(index_constraint_fields(&definition, key.inline).unwrap())
    }).collect()
}

pub fn columns_with_index_constraints(columns: &[ColDef], keys: &[NamedIndexConstraint]) -> Result<Vec<ColDef>, String> {
    if keys.iter().filter(|key| key.primary).count() > 1 { return Err("A table can have only one primary key".into()); }
    let mut result = columns.to_vec();
    for column in &mut result { column.pk = false; column.unique = false; column.auto_increment = false; }
    for key in keys {
        let definition = index_constraint_definition(key)?;
        let members = match &definition.constraint {
            TableConstraint::PrimaryKey { columns, .. } | TableConstraint::Unique { columns, .. } => columns,
            _ => unreachable!(),
        };
        for member in members {
            let name = index_column_name(&member.expr).unwrap();
            let column = result.iter_mut().find(|column| name.eq(column.name.as_str())).ok_or_else(|| format!("Unknown index-constraint column: {}", identifier_text(&name)))?;
            if key.primary {
                if column.generated.is_some() { return Err("Generated columns cannot be part of a primary key".into()); }
                column.pk = true;
                column.auto_increment = key.auto_increment;
                if key.auto_increment && (members.len() != 1 || !column.type_name.eq_ignore_ascii_case("INTEGER")) {
                    return Err("Autoincrement requires one INTEGER primary-key column".into());
                }
            } else if members.len() == 1 { column.unique = true; }
        }
    }
    validate_primary_key(&result)?;
    Ok(result)
}

pub fn index_constraints_from_columns(keys: &[NamedIndexConstraint], columns: &[ColDef], conflict: Option<ResolveType>) -> Result<Vec<NamedIndexConstraint>, String> {
    let mut result = Vec::new();
    let mut primary = keys.iter().find(|key| key.primary).cloned().unwrap_or_else(|| NamedIndexConstraint { primary: true, ..Default::default() });
    let mut members = if primary.columns.trim().is_empty() { Vec::new() } else {
        let definition = index_constraint_definition(&primary)?;
        let TableConstraint::PrimaryKey { columns, .. } = definition.constraint else { unreachable!() };
        columns
    };
    members.retain(|key| columns.iter().any(|column| column.pk && column_expression(&key.expr, &identifier(&column.name))));
    for column in columns.iter().filter(|column| column.pk) {
        if !members.iter().any(|key| column_expression(&key.expr, &identifier(&column.name))) {
            members.push(SortedColumn { expr: Expr::Name(identifier(&column.name)), order: None, nulls: None });
        }
    }
    primary.columns = members.iter().map(|key| SqlFragment(key).to_string()).collect::<Vec<_>>().join(", ");
    primary.auto_increment = columns.iter().any(|column| column.auto_increment);
    primary.conflict = conflict;
    let mut primary = (!members.is_empty()).then_some(primary);
    for key in keys {
        if key.primary { if let Some(key) = primary.take() { result.push(key); } continue; }
        let definition = index_constraint_definition(key)?;
        let TableConstraint::Unique { columns: members, .. } = definition.constraint else { unreachable!() };
        if members.len() != 1 || columns.iter().any(|column| column.unique && column_expression(&members[0].expr, &identifier(&column.name))) { result.push(key.clone()); }
    }
    if let Some(key) = primary { result.push(key); }
    let flags = columns_with_index_constraints(columns, &result)?;
    for (column, current) in columns.iter().zip(flags) {
        if column.unique && !current.unique {
            result.push(NamedIndexConstraint { columns: db::quote_ident(&column.name), ..Default::default() });
        }
    }
    Ok(result)
}

fn set_index_constraints(command: &mut Cmd, keys: &[NamedIndexConstraint]) -> Result<(), String> {
    if keys.iter().filter(|key| key.primary).count() > 1 { return Err("A table can have only one primary key".into()); }
    let replacements = keys.iter().map(index_constraint_definition).collect::<Result<Vec<_>, _>>()?;
    let Cmd::Stmt(Stmt::CreateTable { body: CreateTableBody::ColumnsAndConstraints { columns, .. }, .. }) = command else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    let mut table_keys = Vec::new();
    let mut inline_keys = Vec::new();
    for (key, definition) in keys.iter().zip(replacements) {
        let members = match &definition.constraint {
            TableConstraint::PrimaryKey { columns, .. } | TableConstraint::Unique { columns, .. } => columns,
            _ => unreachable!(),
        };
        for member in members {
            if !columns.values().any(|column| column_expression(&member.expr, &column.col_name)) { return Err(format!("Unknown index-constraint column: {}", SqlFragment(&member.expr))); }
        }
        if let Some(inline)=inline_index_constraint(key,&definition) {inline_keys.push(inline);continue;}
        table_keys.push(definition);
    }
    for column in columns.values_mut() {
        let mut replacements = inline_keys.iter().filter(|(name, _)| name.eq(&column.col_name)).map(|(_, item)| item.clone());
        column.constraints.retain_mut(|item| {
            if !matches!(item.constraint, ColumnConstraint::PrimaryKey { .. } | ColumnConstraint::Unique(..)) { return true; }
            if let Some(replacement) = replacements.next() { *item = replacement; true } else { false }
        });
        column.constraints.extend(replacements);
    }
    replace_table_constraints(command, table_keys, |constraint| matches!(constraint, TableConstraint::PrimaryKey { .. } | TableConstraint::Unique { .. }))
}

fn replace_table_constraints(command: &mut Cmd, replacements: Vec<NamedTableConstraint>, matches: impl Fn(&TableConstraint) -> bool) -> Result<(), String> {
    let Cmd::Stmt(Stmt::CreateTable { body: CreateTableBody::ColumnsAndConstraints { constraints, .. }, .. }) = command else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    let mut replacements = replacements.into_iter();
    let items = constraints.get_or_insert_with(Vec::new);
    items.retain_mut(|item| {
        if !matches(&item.constraint) { return true; }
        if let Some(replacement) = replacements.next() { *item = replacement; true } else { false }
    });
    items.extend(replacements);
    if items.is_empty() { *constraints = None; }
    Ok(())
}

fn parse(sql: &str) -> Result<Cmd, String> {
    let mut scanner = Scanner::new(Tokenizer::new());
    let mut tokens = Vec::new();
    loop {
        let (start, token, _) = scanner
            .scan(sql.as_bytes())
            .map_err(|error| error.to_string())?;
        let Some((_, kind)) = token else {
            break;
        };
        tokens.push((start, kind));
    }
    let positions = tokens
        .windows(3)
        .enumerate()
        .filter_map(|(index, window)| {
            (window[0].1 == TokenType::TK_CONSTRAINT
                && (window[2].1 == TokenType::TK_DEFERRABLE
                    || (window[2].1 == TokenType::TK_NOT
                        && tokens
                            .get(index + 3)
                            .is_some_and(|token| token.1 == TokenType::TK_DEFERRABLE))))
            .then_some(window[2].0)
        })
        .collect::<Vec<_>>();
    let mut marker = "__next_gui_named_deferral".to_string();
    while sql.contains(&marker) {
        marker.push('_');
    }
    let mut protected = sql.to_string();
    // Parser 0.15 drops named DEFERRABLE; consume its name through a temporary AST-only CHECK.
    for position in positions.iter().rev() {
        protected.insert_str(*position, &format!("CHECK('{marker}') "));
    }
    let mut parser = Parser::new(protected.as_bytes());
    let mut command = parser
        .next()
        .map_err(|error| error.to_string())?
        .ok_or("Empty schema definition")?;
    if parser.next().map_err(|error| error.to_string())?.is_some() {
        return Err("Expected one schema statement".into());
    }
    let mut restored = 0;
    if let Cmd::Stmt(Stmt::CreateTable {
        body: CreateTableBody::ColumnsAndConstraints { columns, .. },
        ..
    }) = &mut command
    {
        for column in columns.values_mut() {
            let mut index = 0;
            while index + 1 < column.constraints.len() {
                if matches!(&column.constraints[index].constraint, ColumnConstraint::Check(value) if value.to_string() == format!("'{marker}'"))
                {
                    if !matches!(
                        column.constraints[index + 1].constraint,
                        ColumnConstraint::Defer(..)
                    ) {
                        return Err("Cannot preserve a named DEFERRABLE constraint".into());
                    }
                    let name = column.constraints.remove(index).name;
                    column.constraints[index].name = name;
                    restored += 1;
                }
                index += 1;
            }
        }
    }
    if restored != positions.len() {
        return Err("Cannot preserve all named DEFERRABLE constraints".into());
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
    index_column_name(expr).is_some_and(|value| value.eq(name))
}

fn rename_identifier(name: &mut Name, renames: &[(&str, &str)]) {
    if let Some((_, target)) = renames.iter().find(|(source, _)| (*name).eq(*source)) {
        *name = identifier(target);
    }
}

fn rename_table_expression(expr: &mut Expr, table: &Name, target: Option<&str>, renames: &[(&str, &str)]) -> Result<(), String> {
    match expr {
        Expr::Name(name) => rename_identifier(name, renames),
        Expr::Id(name) => {
            if let Some((_, target)) = renames.iter().find(|(source, _)| Name(name.0.clone()).eq(*source)) {
                *expr = Expr::Name(identifier(target));
            }
        }
        Expr::Qualified(owner, name) | Expr::DoublyQualified(_, owner, name) => {
            if (*owner).eq(table) {
                rename_identifier(name, renames);
                if let Some(target) = target { *owner = identifier(target); }
                else { *expr = Expr::Name(name.clone()); }
            }
        }
        Expr::Between { lhs, start, end, .. } => {
            for value in [lhs, start, end] { rename_table_expression(value, table, target, renames)?; }
        }
        Expr::Binary(lhs, _, rhs) => {
            rename_table_expression(lhs, table, target, renames)?;
            rename_table_expression(rhs, table, target, renames)?;
        }
        Expr::Case { base, when_then_pairs, else_expr } => {
            for value in base.iter_mut().chain(else_expr.iter_mut()) { rename_table_expression(value, table, target, renames)?; }
            for (when, then) in when_then_pairs {
                rename_table_expression(when, table, target, renames)?;
                rename_table_expression(then, table, target, renames)?;
            }
        }
        Expr::Cast { expr, .. } | Expr::Collate(expr, _) | Expr::IsNull(expr) | Expr::NotNull(expr) | Expr::Unary(_, expr) => rename_table_expression(expr, table, target, renames)?,
        Expr::FunctionCall { args, order_by: None, filter_over: None, .. } => {
            for value in args.iter_mut().flatten() { rename_table_expression(value, table, target, renames)?; }
        }
        Expr::InList { lhs, rhs, .. } => {
            rename_table_expression(lhs, table, target, renames)?;
            for value in rhs.iter_mut().flatten() { rename_table_expression(value, table, target, renames)?; }
        }
        Expr::Like { lhs, rhs, escape, .. } => {
            for value in [Some(lhs), Some(rhs), escape.as_mut()].into_iter().flatten() { rename_table_expression(value, table, target, renames)?; }
        }
        Expr::Parenthesized(values) => {
            for value in values { rename_table_expression(value, table, target, renames)?; }
        }
        Expr::Raise(_, value) => {
            if let Some(value) = value { rename_table_expression(value, table, target, renames)?; }
        }
        Expr::Literal(_) => {}
        _ => return Err("Cannot preview this table expression during a rename".into()),
    }
    Ok(())
}

/// Pure schema transformation for the preview; SQLite still performs actual dependency renames.
pub fn preview_renames(sql: &str, target: &str, renames: &[(&str, &str)]) -> Result<String, String> {
    let mut command = parse(sql)?;
    let Cmd::Stmt(Stmt::CreateTable { tbl_name, body: CreateTableBody::ColumnsAndConstraints { columns, constraints, .. }, .. }) = &mut command else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    let original_table = tbl_name.name.clone();
    let rename_foreign_key = |clause: &mut sqlite3_parser::ast::ForeignKeyClause| {
        if clause.tbl_name.eq(&original_table) {
            clause.tbl_name = identifier(target);
            for column in clause.columns.iter_mut().flatten() { rename_identifier(&mut column.col_name, renames); }
        }
    };
    for column in columns.values_mut() {
        rename_identifier(&mut column.col_name, renames);
        for item in &mut column.constraints {
            match &mut item.constraint {
                ColumnConstraint::Check(expr) | ColumnConstraint::Generated { expr, .. } => rename_table_expression(expr, &original_table, Some(target), renames)?,
                ColumnConstraint::ForeignKey { clause, .. } => rename_foreign_key(clause),
                _ => {}
            }
        }
    }
    for item in constraints.iter_mut().flatten() {
        match &mut item.constraint {
            TableConstraint::PrimaryKey { columns, .. } | TableConstraint::Unique { columns, .. } => {
                for column in columns { rename_index_column(&mut column.expr, renames); }
            }
            TableConstraint::Check(expr) => rename_table_expression(expr, &original_table, Some(target), renames)?,
            TableConstraint::ForeignKey { columns, clause, .. } => {
                for column in columns { rename_identifier(&mut column.col_name, renames); }
                rename_foreign_key(clause);
            }
        }
    }
    tbl_name.name = identifier(target);
    Ok(emit(command))
}

pub fn format_table_preview(sql: &str) -> Result<String, String> {
    let Cmd::Stmt(Stmt::CreateTable { tbl_name, body: CreateTableBody::ColumnsAndConstraints { columns, constraints, flags }, .. }) = parse(sql)? else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    let fields = columns.values().map(|column| SqlFragment(column).to_string())
        .chain(constraints.iter().flatten().map(|constraint| SqlFragment(constraint).to_string())).collect::<Vec<_>>();
    let options = [flags.contains(TabFlags::WithoutRowid).then_some("WITHOUT ROWID"), flags.contains(TabFlags::Strict).then_some("STRICT")]
        .into_iter().flatten().collect::<Vec<_>>().join(", ");
    Ok(format!("CREATE TABLE {} (\n    {}\n){}{};", SqlFragment(&tbl_name), fields.join(",\n    "), if options.is_empty() { "" } else { " " }, options))
}

pub fn column_definition(
    column: &ColDef,
    original: Option<&ColDef>,
) -> Result<ColumnDefinition, String> {
    if column.generated.as_ref().is_some_and(|generated| generated.expression.trim().is_empty()) {
        return Err("A generated column requires an expression".into());
    }
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
    let mut actual = [0; 9];
    let definition = columns.into_values().next().unwrap();
    for (index, value) in definition.constraints.iter().enumerate() {
        if matches!(value.constraint, ColumnConstraint::Defer(..)) {
            if !definition.constraints[..index]
                .iter()
                .any(|item| matches!(item.constraint, ColumnConstraint::ForeignKey { .. }))
            {
                return Err("DEFERRABLE must follow the column's REFERENCES clause".into());
            }
            continue;
        }
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
            ColumnConstraint::ForeignKey { .. } => 7,
            ColumnConstraint::Generated { .. } => 8,
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
        (!column.foreign_key_clause.trim().is_empty()) as usize,
        column.generated.is_some() as usize,
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

pub fn requires_rebuild(original: &[ColDef], draft: &[ColDef], sources: &[Option<String>], old_options: &TableOptions, options: Option<&TableOptions>) -> bool {
    if options.is_some_and(|options| options != old_options) { return true; }
    for (column, source) in draft.iter().zip(sources) {
        if let Some(source) = source {
            let Some(old) = original.iter().find(|old| old.name == *source) else { return true; };
            if old.pk != column.pk || old.auto_increment != column.auto_increment || !old.type_name.eq_ignore_ascii_case(&column.type_name)
                || old.notnull != column.notnull || old.default_value != column.default_value || old.unique != column.unique
                || !old.collation.eq_ignore_ascii_case(&column.collation) || old.check_expression != column.check_expression
                || old.foreign_key_clause != column.foreign_key_clause || old.generated != column.generated { return true; }
        } else if column.pk || column.unique || !column.default_value.trim().is_empty() || !column.collation.is_empty()
            || !column.check_expression.trim().is_empty() || !column.foreign_key_clause.trim().is_empty() || column.generated.is_some() { return true; }
    }
    let retained: Vec<_> = original.iter().filter(|column| sources.iter().any(|source| source.as_ref() == Some(&column.name))).map(|column| &column.name).collect();
    let requested: Vec<_> = sources.iter().filter_map(Option::as_ref).collect();
    retained != requested || sources.iter().skip_while(|source| source.is_some()).any(Option::is_some)
}

pub fn unqualify_local_checks(sql: &str, table: &str) -> Result<String, String> {
    let mut command = parse(sql)?;
    let Cmd::Stmt(Stmt::CreateTable { body: CreateTableBody::ColumnsAndConstraints { columns, constraints, .. }, .. }) = &mut command else {
        return Err("Expected an ordinary CREATE TABLE definition".into());
    };
    // CHECK identifiers are local to this table; a prefix cannot survive the legacy temporary-table rename.
    for column in columns.values_mut() {
        for item in &mut column.constraints {
            if let ColumnConstraint::Check(expression) = &mut item.constraint { rename_table_expression(expression, &identifier(table), None, &[])?; }
        }
    }
    for item in constraints.iter_mut().flatten() {
        if let TableConstraint::Check(expression) = &mut item.constraint { rename_table_expression(expression, &identifier(table), None, &[])?; }
    }
    Ok(emit(command))
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
    let effective_columns;
    let draft = if let Some(keys) = options.as_ref().and_then(|options| options.index_constraints.as_deref()) {
        effective_columns = columns_with_index_constraints(draft, keys)?;
        effective_columns.as_slice()
    } else { draft };
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
    if let Some(options) = &options {
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
                // Fields edits the first CHECK; Check Constraints can override every column declaration.
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
            if old.foreign_key_clause != col.foreign_key_clause {
                // Fields edits the first FK; the Foreign Keys page can override every column declaration.
                let positions = foreign_key_indices(&item.constraints);
                let mut replacement = foreign_key_indices(&replacement.constraints)
                    .into_iter()
                    .map(|index| replacement.constraints[index].clone())
                    .collect::<Vec<_>>();
                for (new, old) in replacement.iter_mut().zip(&positions) {
                    new.name = item.constraints[*old].name.clone();
                }
                let start = positions.first().copied().unwrap_or(item.constraints.len());
                for index in positions.into_iter().rev() {
                    item.constraints.remove(index);
                }
                item.constraints.splice(start..start, replacement);
            }
            if old.generated != col.generated {
                let position = item.constraints.iter().position(|value| matches!(value.constraint, ColumnConstraint::Generated { .. }));
                let replacement = replacement.constraints.iter().find(|value| matches!(value.constraint, ColumnConstraint::Generated { .. })).cloned();
                match (position, replacement) {
                    (Some(position), Some(mut value)) => {
                        value.name = item.constraints[position].name.clone();
                        item.constraints[position] = value;
                    }
                    (Some(position), None) => { item.constraints.remove(position); }
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
    columns.clear();
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
    if let Some(options) = options {
        if let Some(keys) = options.index_constraints { set_index_constraints(&mut command, &keys)?; }
        else { set_primary_key_conflict(&mut command, options.primary_key_conflict)?; }
        if let Some(checks) = options.checks { set_table_checks(&mut command, &checks)?; }
        if let Some(checks) = options.inline_checks { set_inline_checks(&mut command, &checks)?; }
        if let Some(keys) = options.foreign_keys { set_table_foreign_keys(&mut command, &keys)?; }
        if let Some(keys) = options.inline_foreign_keys { set_inline_foreign_keys(&mut command, &keys)?; }
    }
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

fn foreign_key_indices(constraints: &[NamedColumnConstraint]) -> Vec<usize> {
    foreign_key_groups(constraints).into_iter().next().unwrap_or_default()
}

fn foreign_key_groups(constraints: &[NamedColumnConstraint]) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (index, item) in constraints.iter().enumerate() {
        match item.constraint {
            ColumnConstraint::ForeignKey { .. } => groups.push(vec![index]),
            ColumnConstraint::Defer(..) => { if let Some(group) = groups.last_mut() { group.push(index); } }
            _ => {}
        }
    }
    groups
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
                    Some(identifier_text(collation_name))
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
        definition.foreign_key_clause = foreign_key_indices(&column.constraints)
            .into_iter()
            .map(|index| if matches!(column.constraints[index].constraint, ColumnConstraint::Defer(..)) {
                SqlFragment(&column.constraints[index]).to_string()
            } else { SqlFragment(&column.constraints[index].constraint).to_string() })
            .collect::<Vec<_>>()
            .join(" ");
        definition.generated = column.constraints.iter().find_map(|item| {
            if let ColumnConstraint::Generated { expr, typ } = &item.constraint {
                Some(db::GeneratedColumn { expression: expr.to_string(), stored: typ.as_ref().is_some_and(|storage| storage.0.eq_ignore_ascii_case("STORED")) })
            } else { None }
        });
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
    #[test]
    fn constraint_grid_sql_fragments_keep_inline_semantics_names_and_deferrals() {
        let mut primary=super::NamedIndexConstraint {name:Some("primary \"key".into()),columns:"id".into(),primary:true,auto_increment:true,inline:true,..Default::default()};
        let unique=super::NamedIndexConstraint {name:Some("compound unique".into()),columns:"a COLLATE NOCASE DESC,b ASC".into(),conflict:Some(sqlite3_parser::ast::ResolveType::Fail),..Default::default()};
        let check=super::NamedCheck {column:Some("name".into()),name:Some("check\nname".into()),expression:"length(name)>0 AND name <> 'line\nbreak'".into()};
        let inline=super::NamedForeignKey {name:Some("parent \"reference".into()),columns:"p".into(),clause:"REFERENCES parent(id) ON UPDATE CASCADE CONSTRAINT deadline DEFERRABLE INITIALLY DEFERRED".into(),inline:true};
        let table=super::NamedForeignKey {name:Some("compound parent".into()),columns:"a,b".into(),clause:"REFERENCES parent(code,region) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED".into(),inline:false};
        let pk=super::index_constraint_sql(&primary).unwrap();assert!(pk.contains("AUTOINCREMENT"));assert!(!pk.contains("(id"));
        let fk=super::foreign_key_sql(&inline).unwrap();assert!(fk.contains("deadline") && fk.contains("INITIALLY DEFERRED"));assert!(!fk.contains("FOREIGN KEY"));
        let conn=rusqlite::Connection::open_in_memory().unwrap();conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE parent(id INTEGER PRIMARY KEY,code TEXT,region INTEGER,UNIQUE(code,region)); INSERT INTO parent VALUES(1,'a',7);").unwrap();
        conn.execute_batch(&format!("CREATE TABLE records(id INTEGER {pk},name TEXT {},p INTEGER {fk},a TEXT,b INTEGER,{},{});",super::check_sql(&check).unwrap(),super::index_constraint_sql(&unique).unwrap(),super::foreign_key_sql(&table).unwrap())).unwrap();
        conn.execute("INSERT INTO records(name,p,a,b) VALUES('ok',1,'a',7)",[]).unwrap();assert_eq!(conn.last_insert_rowid(),1);
        assert!(conn.execute("INSERT INTO records(name) VALUES('')",[]).is_err());assert!(conn.execute("INSERT INTO records(name) VALUES('line'||char(10)||'break')",[]).is_err());assert!(conn.execute("INSERT INTO records(name,a,b) VALUES('ok','A',7)",[]).is_err());
        conn.execute_batch("BEGIN; INSERT INTO records(name,p) VALUES('pending',2); INSERT INTO parent VALUES(2,'b',8); COMMIT;").unwrap();
        primary.auto_increment=false;primary.columns="id DESC".into();let sql=super::index_constraint_sql(&primary).unwrap();assert!(sql.contains("PRIMARY KEY DESC"));
        conn.execute_batch(&format!("CREATE TABLE descending(id INTEGER {sql},value); INSERT INTO descending(id,value) VALUES(900,'kept');")).unwrap();assert_eq!(conn.query_row("SELECT rowid FROM descending",[],|row|row.get::<_,i64>(0)).unwrap(),1);
        primary.columns="id); DROP TABLE parent; --".into();assert!(super::index_constraint_sql(&primary).is_err());
        let mut bad=check;bad.expression="1); DROP TABLE parent; --".into();assert!(super::check_sql(&bad).is_err());
        let mut bad=table;bad.clause="REFERENCES parent(id)); DROP TABLE parent; --".into();assert!(super::foreign_key_sql(&bad).is_err());
        assert_eq!(conn.query_row("SELECT count(*) FROM records",[],|row|row.get::<_,i64>(0)).unwrap(),2);
    }

    use super::*;
    use rusqlite::Connection;

    #[test]
    fn all_inline_checks_preserve_named_scopes_dependencies_and_pending_transactions() {
        for schema in ["main","temp","aux.\"db"] {
            let conn=Connection::open_in_memory().unwrap(); let table=db::qualified_name(schema,"records");
            conn.execute_batch(&format!("ATTACH ':memory:' AS \"aux.\"\"db\"; PRAGMA foreign_keys=ON;
                CREATE TABLE {table}(id INTEGER PRIMARY KEY AUTOINCREMENT,
                    v INTEGER CONSTRAINT positive CHECK(v>0) NOT NULL CONSTRAINT ceiling CHECK(v<10),
                    note TEXT CONSTRAINT note_check CHECK(length(note)>0) DEFAULT 'v CHECK',
                    g INTEGER AS(v*2) STORED, CONSTRAINT table_check CHECK(g<40));
                INSERT INTO {table}(id,v) VALUES(7,2),(11,4),(99,4); DELETE FROM {table} WHERE id=99;
                CREATE TABLE {}(note TEXT); CREATE INDEX {} ON records(v); CREATE VIEW {} AS SELECT id,g FROM records;
                CREATE TRIGGER {} AFTER INSERT ON records BEGIN INSERT INTO audit VALUES(new.note); END;
                BEGIN; INSERT INTO {table}(id,v) VALUES(13,3);",
                db::qualified_name(schema,"audit"),db::qualified_name(schema,"by_value"),db::qualified_name(schema,"visible"),db::qualified_name(schema,"audit_insert"))).unwrap();
            let original:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='records'",db::quote_ident(schema)),[],|row|row.get(0)).unwrap();
            let mut checks=inline_checks(&original).unwrap(); assert_eq!(checks.len(),3);
            assert_eq!(checks.iter().map(|check|check.column.as_deref().unwrap()).collect::<Vec<_>>(),vec!["v","v","note"]);
            checks[0].name=Some("positive.\"约束".into()); checks[0].expression="records.v > 1 AND length('v')=1".into(); checks[1].expression="v < 20".into();
            checks=rename_checks(&checks,"records","orders",&[("v","amount"),("note","memo")]).unwrap();
            assert_eq!(checks[0].column.as_deref(),Some("amount")); assert!(checks[0].expression.contains("'v'"));
            let mut options=table_options(&original).unwrap(); options.inline_checks=Some(checks);
            let mut draft=db::table_column_defs_in(&conn,schema,"records").unwrap(); let sources:Vec<_>=draft.iter().map(|column|Some(column.name.clone())).collect();
            draft[1].name="amount".into(); draft[2].name="memo".into();
            let memo = if schema == "temp" {
                // SQLite rejects TEMP trigger references while renaming their referenced column.
                assert!(db::recreate_table_columns_in(&conn,schema,"records","orders",&draft,&sources,Some(options.clone())).is_err());
                assert_eq!(conn.query_row("SELECT sql FROM temp.sqlite_schema WHERE name='records'",[],|row|row.get::<_,String>(0)).unwrap(),original);
                assert_eq!(conn.query_row("SELECT count(*) FROM temp.records",[],|row|row.get::<_,i64>(0)).unwrap(),3); assert!(!conn.is_autocommit());
                draft[2].name="note".into(); options.inline_checks=Some(rename_checks(&inline_checks(&original).unwrap(),"records","orders",&[("v","amount")]).unwrap());
                let checks=options.inline_checks.as_mut().unwrap(); checks[0].name=Some("positive.\"约束".into()); checks[0].expression="amount > 1 AND length('v')=1".into(); checks[1].expression="amount < 20".into();
                "note"
            } else { "memo" };
            db::recreate_table_columns_in(&conn,schema,"records","orders",&draft,&sources,Some(options)).unwrap();
            let target=db::qualified_name(schema,"orders");
            let saved:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='orders'",db::quote_ident(schema)),[],|row|row.get(0)).unwrap();
            let checks=inline_checks(&saved).unwrap(); assert_eq!(checks.len(),3); assert_eq!(checks[0].name.as_deref(),Some("positive.\"约束"));
            assert_eq!(checks[2].column.as_deref(),Some(memo)); assert_eq!(table_checks(&saved).unwrap().len(),1);
            assert_eq!(conn.query_row(&format!("SELECT sum(rowid)+sum(id)+sum(g) FROM {target}"),[],|row|row.get::<_,i64>(0)).unwrap(),80);
            assert_eq!(conn.query_row(&format!("SELECT count(*) FROM {}",db::qualified_name(schema,"audit")),[],|row|row.get::<_,i64>(0)).unwrap(),1);
            for update in ["amount=1","amount=20","amount=NULL",&format!("{memo}=''"), "amount=21"] { assert!(conn.execute(&format!("UPDATE {target} SET {update}"),[]).is_err(),"{schema}/{update}"); }
            conn.execute(&format!("UPDATE {target} SET amount=10 WHERE id=7"),[]).unwrap();
            let draft=db::table_column_defs_in(&conn,schema,"orders").unwrap(); let sources:Vec<_>=draft.iter().map(|column|Some(column.name.clone())).collect();
            let mut options=table_options(&saved).unwrap(); options.inline_checks=Some(Vec::new());
            db::recreate_table_columns_in(&conn,schema,"orders","orders",&draft,&sources,Some(options)).unwrap();
            conn.execute(&format!("INSERT INTO {target}(amount,{memo}) VALUES(0,'')"),[]).unwrap();
            assert_eq!(conn.query_row(&format!("SELECT id FROM {target} WHERE amount=0"),[],|row|row.get::<_,i64>(0)).unwrap(),100);
            assert!(conn.execute(&format!("INSERT INTO {target}(amount) VALUES(NULL)"),[]).is_err());
            assert!(conn.execute(&format!("INSERT INTO {target}(amount) VALUES(20)"),[]).is_err());
            assert_eq!(conn.query_row(&format!("SELECT count(*) FROM {}",db::qualified_name(schema,"visible")),[],|row|row.get::<_,i64>(0)).unwrap(),4);
            assert!(!conn.is_autocommit()); conn.execute_batch("ROLLBACK").unwrap();
            assert_eq!(conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='records'",db::quote_ident(schema)),[],|row|row.get::<_,String>(0)).unwrap(),original);
        }
    }

    #[test]
    fn inline_check_overrides_reject_invalid_owners_expressions_and_scope_atomically() {
        let conn=Connection::open_in_memory().unwrap();conn.execute_batch("CREATE TABLE t(v INTEGER CHECK(v>0),note TEXT); INSERT INTO t VALUES(2,'kept'); BEGIN; INSERT INTO t VALUES(3,'pending');").unwrap();
        let original:String=conn.query_row("SELECT sql FROM sqlite_schema WHERE name='t'",[],|row|row.get(0)).unwrap();
        let draft=db::table_column_defs_in(&conn,"main","t").unwrap();let sources:Vec<_>=draft.iter().map(|column|Some(column.name.clone())).collect();
        for (owner,expression) in [(None,"v>0"),(Some(""),"v>0"),(Some("missing"),"v>0"),(Some("v"),""),(Some("v"),"v>5"),
            (Some("v"),"v>?1"),(Some("v"),"abs(v)>"),(Some("v"),"v>0),CHECK(1"),(Some("v"),"v>0)); DELETE FROM t; --")] {
            let check=NamedCheck {column:owner.map(str::to_string),name:Some("safe.\"名".into()),expression:expression.into()};
            let options=TableOptions {inline_checks:Some(vec![check]),..Default::default()};
            assert!(db::recreate_table_columns_in(&conn,"main","t","t",&draft,&sources,Some(options)).is_err(),"{owner:?}/{expression}");
            assert_eq!(conn.query_row("SELECT count(*) FROM t",[],|row|row.get::<_,i64>(0)).unwrap(),2); assert!(!conn.is_autocommit());
            assert_eq!(conn.query_row("SELECT sql FROM sqlite_schema WHERE name='t'",[],|row|row.get::<_,String>(0)).unwrap(),original);
        }
        let checks=inline_checks(&original).unwrap();
        assert!(apply_table_options("CREATE TABLE created(v INTEGER)",TableOptions {checks:Some(checks.clone()),..Default::default()}).is_err());
        let ddl=apply_table_options("CREATE TABLE created(v INTEGER)",TableOptions {inline_checks:Some(checks),..Default::default()}).unwrap();
        db::execute_single_statement(&conn,&ddl).unwrap();assert!(conn.execute("INSERT INTO created VALUES(0)",[]).is_err());conn.execute("INSERT INTO created VALUES(1)",[]).unwrap();
    }

    #[test]
    fn all_inline_foreign_keys_preserve_named_deferrals_dependencies_and_pending_violations() {
        for schema in ["main","temp","aux.\"db"] {
            let conn=Connection::open_in_memory().unwrap(); let table=db::qualified_name(schema,"records");
            let parent=db::qualified_name(schema,"parent"); let other=db::qualified_name(schema,"other_parent"); let audit=db::qualified_name(schema,"audit");
            conn.execute_batch(&format!("PRAGMA foreign_keys=ON; ATTACH ':memory:' AS \"aux.\"\"db\";
                CREATE TABLE {parent}(id INTEGER PRIMARY KEY); INSERT INTO {parent} VALUES(1),(2);
                CREATE TABLE {other}(id INTEGER PRIMARY KEY); INSERT INTO {other} VALUES(1),(2);
                CREATE TABLE {table}(id INTEGER PRIMARY KEY AUTOINCREMENT,
                    p INTEGER CONSTRAINT first_parent REFERENCES parent(id) CHECK(p>0) CONSTRAINT \"deadline\" DEFERRABLE INITIALLY DEFERRED NOT NULL
                        CONSTRAINT second_parent REFERENCES other_parent(id) ON UPDATE CASCADE DEFERRABLE INITIALLY IMMEDIATE,
                    q INTEGER REFERENCES records(id) DEFERRABLE INITIALLY DEFERRED,
                    g INTEGER AS(p+1) STORED, note TEXT DEFAULT 'REFERENCES CONSTRAINT DEFERRABLE',
                    CONSTRAINT previous_record FOREIGN KEY(q) REFERENCES records(id));
                INSERT INTO {table}(id,p,q) VALUES(7,1,NULL),(11,2,7),(99,2,NULL); DELETE FROM {table} WHERE id=99;
                CREATE TABLE {audit}(note TEXT); CREATE INDEX {} ON records(p); CREATE VIEW {} AS SELECT id,g FROM records;
                CREATE TRIGGER {} AFTER INSERT ON records BEGIN INSERT INTO audit VALUES(new.note); END;
                BEGIN; INSERT INTO {other} VALUES(3); INSERT INTO {table}(id,p) VALUES(13,3); INSERT INTO {audit} VALUES('pending');",
                db::qualified_name(schema,"by_parent"),db::qualified_name(schema,"visible"),db::qualified_name(schema,"audit_insert"))).unwrap();
            let original:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='records'",db::quote_ident(schema)),[],|row|row.get(0)).unwrap();
            let mut keys=inline_foreign_keys(&original).unwrap(); assert_eq!(keys.len(),3); assert!(keys.iter().all(|key|key.inline));
            assert!(keys[0].clause.contains("CONSTRAINT \"deadline\" DEFERRABLE"));
            keys[0].name=Some("first.\"父键".into()); keys[0].clause=keys[0].clause.replace("\"deadline\"","\"new deadline\"");
            keys[1].clause=keys[1].clause.replace("INITIALLY IMMEDIATE","INITIALLY DEFERRED");
            keys=rename_foreign_keys(&keys,"records","orders",&[("p","parent_value"),("q","previous")]).unwrap();
            assert!(keys[0].clause.contains("parent (id)") && keys[2].clause.contains("\"orders\""));
            let mut options=table_options(&original).unwrap(); options.inline_foreign_keys=Some(keys.clone());
            let mut draft=db::table_column_defs_in(&conn,schema,"records").unwrap(); let sources:Vec<_>=draft.iter().map(|column|Some(column.name.clone())).collect();
            draft[1].name="parent_value".into(); draft[2].name="previous".into();
            db::recreate_table_columns_in(&conn,schema,"records","orders",&draft,&sources,Some(options)).unwrap_or_else(|error|panic!("{schema}: {error}"));
            let saved:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='orders'",db::quote_ident(schema)),[],|row|row.get(0)).unwrap();
            assert_eq!(inline_foreign_keys(&saved).unwrap(),keys); assert_eq!(table_foreign_keys(&saved).unwrap().len(),1);
            let target=db::qualified_name(schema,"orders");
            assert_eq!(conn.query_row(&format!("SELECT sum(rowid)+sum(id)+sum(g) FROM {target}"),[],|row|row.get::<_,i64>(0)).unwrap(),71);
            assert_eq!(conn.query_row(&format!("SELECT count(*) FROM {audit}"),[],|row|row.get::<_,i64>(0)).unwrap(),2);
            assert!(conn.execute_batch("COMMIT").is_err()); assert!(!conn.is_autocommit());
            conn.execute(&format!("INSERT INTO {parent} VALUES(3)"),[]).unwrap();
            conn.execute(&format!("UPDATE {other} SET id=4 WHERE id=1"),[]).unwrap();
            assert_eq!(conn.query_row(&format!("SELECT parent_value,g FROM {target} WHERE id=7"),[],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?))).unwrap(),(4,5));
            assert!(conn.execute_batch("COMMIT").is_err()); conn.execute(&format!("INSERT INTO {parent} VALUES(4)"),[]).unwrap();
            let draft=db::table_column_defs_in(&conn,schema,"orders").unwrap(); let sources:Vec<_>=draft.iter().map(|column|Some(column.name.clone())).collect();
            let mut options=table_options(&saved).unwrap(); options.inline_foreign_keys=Some(Vec::new());
            db::recreate_table_columns_in(&conn,schema,"orders","orders",&draft,&sources,Some(options)).unwrap();
            assert!(db::table_column_defs_in(&conn,schema,"orders").unwrap().iter().all(|column|column.foreign_key_clause.is_empty()));
            conn.execute(&format!("INSERT INTO {target}(parent_value) VALUES(999)"),[]).unwrap();
            assert_eq!(conn.query_row(&format!("SELECT id FROM {target} WHERE parent_value=999"),[],|row|row.get::<_,i64>(0)).unwrap(),100);
            assert!(conn.execute(&format!("INSERT INTO {target}(parent_value) VALUES(NULL)"),[]).is_err());
            assert!(conn.execute(&format!("INSERT INTO {target}(parent_value) VALUES(-1)"),[]).is_err());
            assert!(conn.execute(&format!("INSERT INTO {target}(parent_value,previous) VALUES(1,99999)"),[]).is_err());
            assert_eq!(conn.query_row(&format!("SELECT count(*) FROM {}",db::qualified_name(schema,"visible")),[],|row|row.get::<_,i64>(0)).unwrap(),4);
            conn.execute_batch("ROLLBACK").unwrap();
            assert_eq!(conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='records'",db::quote_ident(schema)),[],|row|row.get::<_,String>(0)).unwrap(),original);
        }
    }

    #[test]
    fn inline_foreign_key_overrides_reject_other_constraints_owners_and_statement_tails() {
        let conn=Connection::open_in_memory().unwrap(); conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE parent(id PRIMARY KEY); INSERT INTO parent VALUES(1); CREATE TABLE t(v INTEGER CONSTRAINT original REFERENCES parent(id) CONSTRAINT deadline DEFERRABLE INITIALLY DEFERRED,note TEXT CHECK(length(note)>0)); INSERT INTO t VALUES(1,'kept'); BEGIN; INSERT INTO t VALUES(1,'pending');").unwrap();
        let original:String=conn.query_row("SELECT sql FROM sqlite_schema WHERE name='t'",[],|row|row.get(0)).unwrap();
        let draft=db::table_column_defs_in(&conn,"main","t").unwrap(); let sources:Vec<_>=draft.iter().map(|column|Some(column.name.clone())).collect();
        for (columns,clause) in [("v",""),("v,note","REFERENCES parent(id)"),("missing","REFERENCES parent(id)"),("v DESC","REFERENCES parent(id)"),
            ("v","REFERENCES parent(id,id)"),("v","REFERENCES parent(id DESC)"),("v","REFERENCES missing(id)"),
            ("v","REFERENCES parent(id) CHECK(1)"),("v","REFERENCES parent(id) NOT NULL"),("v","REFERENCES parent(id) REFERENCES parent(id)"),
            ("v","REFERENCES parent(id)); DELETE FROM t; --"),("v","REFERENCES parent(?1)"),("v","REFERENCES parent(id)),CHECK(1)"),("v","DEFERRABLE REFERENCES parent(id)")] {
            let key=NamedForeignKey {name:Some("safe.\"键".into()),columns:columns.into(),clause:clause.into(),inline:true};
            let options=TableOptions {inline_foreign_keys:Some(vec![key]),..Default::default()};
            assert!(db::recreate_table_columns_in(&conn,"main","t","t",&draft,&sources,Some(options)).is_err(),"{columns}/{clause}");
            assert_eq!(conn.query_row("SELECT sql FROM sqlite_schema WHERE name='t'",[],|row|row.get::<_,String>(0)).unwrap(),original);
            assert_eq!(conn.query_row("SELECT count(*) FROM t",[],|row|row.get::<_,i64>(0)).unwrap(),2); assert!(!conn.is_autocommit());
        }
        let key=inline_foreign_keys(&original).unwrap().remove(0);
        let ddl=apply_table_options("CREATE TABLE created(v INTEGER,note TEXT)",TableOptions {inline_foreign_keys:Some(vec![key]),..Default::default()}).unwrap();
        db::execute_single_statement(&conn,&ddl).unwrap(); conn.execute("INSERT INTO created(v) VALUES(2)",[]).unwrap();
        assert!(conn.execute_batch("COMMIT").is_err()); conn.execute_batch("INSERT INTO parent VALUES(2); COMMIT;").unwrap();
        let sql:String=conn.query_row("SELECT sql FROM sqlite_schema WHERE name='created'",[],|row|row.get(0)).unwrap();
        assert!(sql.contains("CONSTRAINT deadline DEFERRABLE"),"{sql}");
    }

    #[test]
    fn index_constraints_preserve_inline_semantics_dependencies_and_sequences() {
        for schema in ["main", "temp", "aux.\"db"] {
            let conn = Connection::open_in_memory().unwrap();
            let table = db::qualified_name(schema, "records");
            conn.execute_batch(&format!("PRAGMA foreign_keys=ON; ATTACH ':memory:' AS \"aux.\"\"db\";
                CREATE TABLE {table}(id INTEGER CONSTRAINT record_key PRIMARY KEY AUTOINCREMENT,
                    a TEXT COLLATE NOCASE CONSTRAINT one UNIQUE ON CONFLICT FAIL, b TEXT,
                    g TEXT AS(a||b) STORED, parent INTEGER REFERENCES records(id),
                    CONSTRAINT pair UNIQUE(b COLLATE BINARY DESC,a ASC) ON CONFLICT IGNORE,
                    CONSTRAINT positive CHECK(id>0));
                INSERT INTO {table}(id,a,b,parent) VALUES(7,'A','west',NULL),(11,'B','east',7),(99,'reserve','sequence',NULL);
                DELETE FROM {table} WHERE id=99;
                CREATE TABLE {}(note TEXT); CREATE INDEX {} ON records(b);
                CREATE VIEW {} AS SELECT id,g FROM records;
                CREATE TRIGGER {} AFTER INSERT ON records BEGIN INSERT INTO audit VALUES(new.g); END;
                BEGIN; INSERT INTO {} VALUES('pending');", db::qualified_name(schema,"audit"),db::qualified_name(schema,"by_b"),
                db::qualified_name(schema,"visible"),db::qualified_name(schema,"audit_insert"),db::qualified_name(schema,"audit"))).unwrap();
            let sql: String = conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='records'",db::quote_ident(schema)),[],|row|row.get(0)).unwrap();
            let mut keys = table_index_constraints(&sql).unwrap();
            assert_eq!(keys.len(),3); assert!(keys[0].primary && keys[0].inline && keys[0].auto_increment);
            assert!(keys[1].inline && !keys[2].inline);
            keys[0].columns = db::quote_ident("id"); keys[1].columns = db::quote_ident("a");
            keys[0].name = Some("record.\"主键".into()); keys[0].conflict = Some(ResolveType::Fail);
            keys[1].name = Some("single unique".into()); keys[1].conflict = Some(ResolveType::Ignore);
            keys[2].columns = "a COLLATE NOCASE DESC, b ASC".into(); keys[2].conflict = Some(ResolveType::Abort);
            let mut options = table_options(&sql).unwrap(); options.index_constraints = Some(keys.clone());
            let columns = db::table_column_defs_in(&conn,schema,"records").unwrap();
            let sources: Vec<_> = columns.iter().map(|column|Some(column.name.clone())).collect();
            db::recreate_table_columns_in(&conn,schema,"records","renamed",&columns,&sources,Some(options)).unwrap_or_else(|error|panic!("{schema}: {error}"));
            let saved: String = conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='renamed'",db::quote_ident(schema)),[],|row|row.get(0)).unwrap();
            assert_eq!(table_index_constraints(&saved).unwrap(), keys);
            let target = db::qualified_name(schema,"renamed");
            let rows = conn.prepare(&format!("SELECT rowid,id,g,parent FROM {target} ORDER BY id")).unwrap().query_map([],|row| Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?,row.get::<_,String>(2)?,row.get::<_,Option<i64>>(3)?))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
            assert_eq!(rows,vec![(7,7,"Awest".into(),None),(11,11,"Beast".into(),Some(7))]);
            assert_eq!(conn.query_row(&format!("SELECT seq FROM {} WHERE name='renamed'",db::qualified_name(schema,"sqlite_sequence")),[],|row|row.get::<_,i64>(0)).unwrap(),99);
            assert_eq!(conn.execute(&format!("INSERT INTO {target}(a,b) VALUES('a','duplicate')"),[]).unwrap(),0);
            conn.execute(&format!("INSERT INTO {target}(a,b,parent) VALUES('C','north',7)"),[]).unwrap();
            assert_eq!(conn.query_row(&format!("SELECT id FROM {target} WHERE a='C'"),[],|row|row.get::<_,i64>(0)).unwrap(),101);
            assert_eq!(conn.query_row(&format!("SELECT count(*) FROM {}",db::qualified_name(schema,"visible")),[],|row|row.get::<_,i64>(0)).unwrap(),3);
            assert_eq!(conn.query_row(&format!("SELECT count(*) FROM {}",db::qualified_name(schema,"audit")),[],|row|row.get::<_,i64>(0)).unwrap(),2);
            assert!(conn.execute(&format!("DELETE FROM {target} WHERE id=7"),[]).is_err());
            assert!(!conn.is_autocommit()); conn.execute_batch("ROLLBACK").unwrap();
            assert_eq!(conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='records'",db::quote_ident(schema)),[],|row|row.get::<_,String>(0)).unwrap(),sql);
        }
        for inline in [true,false] {
            let conn=Connection::open_in_memory().unwrap();
            let definition=if inline { "id INTEGER CONSTRAINT identity PRIMARY KEY DESC, v TEXT UNIQUE" } else { "id INTEGER,v TEXT UNIQUE,CONSTRAINT identity PRIMARY KEY(id DESC)" };
            conn.execute_batch(&format!("CREATE TABLE t({definition}); INSERT INTO t(rowid,id,v) VALUES(7,11,'kept'); BEGIN;")).unwrap();
            let sql: String=conn.query_row("SELECT sql FROM sqlite_schema WHERE name='t'",[],|row|row.get(0)).unwrap();
            let mut keys=table_index_constraints(&sql).unwrap(); keys.iter_mut().find(|key|key.primary).unwrap().name=Some("renamed key".into());
            let mut options=table_options(&sql).unwrap(); options.index_constraints=Some(keys);
            let columns=db::table_column_defs_in(&conn,"main","t").unwrap(); let sources:Vec<_>=columns.iter().map(|column|Some(column.name.clone())).collect();
            db::recreate_table_columns_in(&conn,"main","t","t",&columns,&sources,Some(options)).unwrap();
            assert_eq!(conn.query_row("SELECT rowid,id FROM t",[],|row|Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?))).unwrap(),if inline {(7,11)} else {(11,11)});
        }
    }

    #[test]
    fn index_constraints_validate_boundaries_and_never_drop_duplicate_rows() {
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(a TEXT,b INTEGER); INSERT INTO t(rowid,a,b) VALUES(7,'same',1),(11,'same',2); BEGIN; UPDATE t SET b=b+10;").unwrap();
        let columns=db::table_column_defs_in(&conn,"main","t").unwrap(); let sources:Vec<_>=columns.iter().map(|column|Some(column.name.clone())).collect();
        let sql:String=conn.query_row("SELECT sql FROM sqlite_schema WHERE name='t'",[],|row|row.get(0)).unwrap();
        let mut invalid=Vec::new();
        for text in ["", "a+1", "lower(a)", "missing", "?1", "a NULLS LAST", "a),CHECK(1)", "a)); DROP TABLE t; --", "a),UNIQUE(b"] {
            invalid.push(vec![NamedIndexConstraint {columns:text.into(), ..Default::default()}]);
        }
        invalid.push(vec![NamedIndexConstraint { columns:"a".into(),primary:true,auto_increment:true,..Default::default()}]);
        invalid.push(vec![NamedIndexConstraint { columns:"b".into(),auto_increment:true,..Default::default()}]);
        invalid.push(vec![NamedIndexConstraint { columns:"b COLLATE NOCASE DESC".into(),primary:true,inline:true,..Default::default()}]);
        invalid.push(vec![NamedIndexConstraint { columns:"a".into(),primary:true,..Default::default()},NamedIndexConstraint { columns:"b".into(),primary:true,..Default::default()}]);
        for keys in invalid {
            let options=TableOptions {index_constraints:Some(keys.clone()),..Default::default()};
            assert!(db::recreate_table_columns_in(&conn,"main","t","t",&columns,&sources,Some(options)).is_err(),"{keys:?}");
            assert_eq!(conn.query_row("SELECT sql FROM sqlite_schema WHERE name='t'",[],|row|row.get::<_,String>(0)).unwrap(),sql);
            assert_eq!(conn.query_row("SELECT sum(rowid)+sum(b) FROM t",[],|row|row.get::<_,i64>(0)).unwrap(),41); assert!(!conn.is_autocommit());
        }
        for (_,conflict) in PRIMARY_KEY_CONFLICTS {
            let key=NamedIndexConstraint {columns:"a".into(),conflict,..Default::default()};
            let options=TableOptions {index_constraints:Some(vec![key]),..Default::default()};
            assert!(db::recreate_table_columns_in(&conn,"main","t","t",&columns,&sources,Some(options)).is_err(),"{conflict:?}");
            assert_eq!(conn.query_row("SELECT count(*) FROM t",[],|row|row.get::<_,i64>(0)).unwrap(),2); assert!(!conn.is_autocommit());
        }
        let keys=vec![NamedIndexConstraint { name:Some("quoted.\"键".into()),columns:"b DESC, a COLLATE NOCASE ASC".into(),conflict:Some(ResolveType::Fail),..Default::default()}];
        let renamed=rename_index_constraints(&keys,&[("a","b"),("b","a")]).unwrap();
        assert_eq!(renamed[0].name,keys[0].name); assert_eq!(renamed[0].columns,"\"a\" DESC, \"b\" COLLATE NOCASE ASC");
        let options=TableOptions {index_constraints:Some(keys),..Default::default()};
        db::recreate_table_columns_in(&conn,"main","t","t",&columns,&sources,Some(options)).unwrap();
        assert!(conn.execute("INSERT INTO t VALUES('SAME',11)",[]).is_err());
        let options=TableOptions {index_constraints:Some(Vec::new()),..Default::default()};
        db::recreate_table_columns_in(&conn,"main","t","t",&columns,&sources,Some(options)).unwrap();
        conn.execute("INSERT INTO t VALUES('SAME',11)",[]).unwrap();
    }

    #[test]
    fn index_constraint_removal_retains_implicit_not_null_and_insert_conflict_policies() {
        for schema in ["main","temp","aux.\"db"] {
            for flags in [" STRICT"," WITHOUT ROWID"," WITHOUT ROWID, STRICT"] {
                let conn=Connection::open_in_memory().unwrap(); let table=db::qualified_name(schema,"t");
                conn.execute_batch(&format!("ATTACH ':memory:' AS \"aux.\"\"db\"; CREATE TABLE {table}(a TEXT,b INTEGER,CONSTRAINT identity PRIMARY KEY(a,b)){flags}; INSERT INTO {table} VALUES('kept',7); BEGIN;")).unwrap();
                let columns=db::table_column_defs_in(&conn,schema,"t").unwrap(); assert!(columns.iter().all(|column|column.notnull));
                let sources:Vec<_>=columns.iter().map(|column|Some(column.name.clone())).collect();
                let options=TableOptions {strict:flags.contains("STRICT"),index_constraints:Some(Vec::new()),..Default::default()};
                db::recreate_table_columns_in(&conn,schema,"t","t",&columns,&sources,Some(options)).unwrap();
                let saved=db::table_column_defs_in(&conn,schema,"t").unwrap();
                assert!(saved.iter().all(|column|!column.pk && column.notnull));
                assert!(conn.execute(&format!("INSERT INTO {table} VALUES(NULL,9)"),[]).is_err());
                conn.execute(&format!("INSERT INTO {table} VALUES('kept',7)"),[]).unwrap();
                assert_eq!(conn.query_row(&format!("SELECT count(*) FROM {table}"),[],|row|row.get::<_,i64>(0)).unwrap(),2);
            }
        }
        for (_,conflict) in PRIMARY_KEY_CONFLICTS {
            let conn=Connection::open_in_memory().unwrap();
            conn.execute_batch("CREATE TABLE t(a TEXT,b INTEGER); INSERT INTO t VALUES('base',0); BEGIN;").unwrap();
            let columns=db::table_column_defs_in(&conn,"main","t").unwrap(); let sources:Vec<_>=columns.iter().map(|column|Some(column.name.clone())).collect();
            let options=TableOptions {index_constraints:Some(vec![NamedIndexConstraint { columns:"'a' DESC".into(),conflict,..Default::default()}]),..Default::default()};
            db::recreate_table_columns_in(&conn,"main","t","t",&columns,&sources,Some(options)).unwrap();
            let result=conn.execute("INSERT INTO t VALUES('new',1),('base',2),('later',3)",[]);
            let rows=conn.prepare("SELECT a,b FROM t ORDER BY a").unwrap().query_map([],|row|Ok((row.get::<_,String>(0)?,row.get::<_,i64>(1)?))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
            match conflict {
                Some(ResolveType::Ignore) => { assert_eq!(result.unwrap(),2); assert_eq!(rows,vec![("base".into(),0),("later".into(),3),("new".into(),1)]); }
                Some(ResolveType::Replace) => { assert_eq!(result.unwrap(),3); assert_eq!(rows,vec![("base".into(),2),("later".into(),3),("new".into(),1)]); }
                Some(ResolveType::Fail) => { assert!(result.is_err()); assert_eq!(rows,vec![("base".into(),0),("new".into(),1)]); }
                _ => { assert!(result.is_err()); assert_eq!(rows,vec![("base".into(),0)]); }
            }
            assert_eq!(conn.is_autocommit(),conflict==Some(ResolveType::Rollback));
        }
        let key=NamedIndexConstraint {columns:"'a' COLLATE NOCASE DESC".into(),..Default::default()};
        assert_eq!(rename_index_constraints(&[key],&[("a","A")]).unwrap()[0].columns,"\"A\" COLLATE NOCASE DESC");
        assert!(index_constraint_definition(&NamedIndexConstraint {columns:"a AUTOINCREMENT".into(),primary:true,..Default::default()}).is_err());
    }

    #[test]
    fn check_identity_rewrites_preserve_literals_functions_types_and_unrelated_sessions() {
        let check=NamedCheck { column:None, name:Some("a".into()), expression:"t.a < b AND abs(a)>0 AND CAST(a AS a)>0 AND a COLLATE a != 'a'".into() };
        let unchanged=NamedCheck { column:None, name:None,expression:"length('kept') > 0 -- keep comment".into() };
        let renamed=rename_checks(&[check.clone(),unchanged.clone()],"t","renamed",&[("a","b"),("b","a")]).unwrap();
        assert_eq!(renamed[0].name,check.name); assert_eq!(renamed[1],unchanged);
        let expression=&renamed[0].expression;
        assert!(expression.contains("\"renamed\".\"b\" < \"a\"") && expression.contains("abs (\"b\")") && expression.contains("AS a") && expression.contains("COLLATE a") && expression.contains("'a'"),"{expression}");
        assert!(rename_checks(&[NamedCheck { column:None, name:None,expression:"a +".into() }],"t","renamed",&[("a","b")]).is_err());
        assert_eq!(check.expression,"t.a < b AND abs(a)>0 AND CAST(a AS a)>0 AND a COLLATE a != 'a'");
        let case = rename_checks(&[NamedCheck { column:None, name:None,expression:"t.a > 0".into() }],"t","T",&[("a","A")]).unwrap();
        assert_eq!(case[0].expression,"\"T\".\"A\" > 0");
    }

    #[test]
    fn table_preview_matches_saved_renames_without_rewriting_literals_functions_or_other_tables() {
        for schema in ["main","temp","aux.\"db"] {
            for options in [""," WITHOUT ROWID, STRICT"] {
                let conn=Connection::open_in_memory().unwrap();
                let table=db::qualified_name(schema,"t");
                conn.execute_batch(&format!("PRAGMA foreign_keys=ON; ATTACH ':memory:' AS \"aux.\"\"db\";
                    CREATE TABLE {}(b INTEGER PRIMARY KEY); INSERT INTO {} VALUES(1),(2),(3);
                    CREATE TABLE {table}(a TEXT CONSTRAINT a PRIMARY KEY ON CONFLICT FAIL,b INTEGER,abs INTEGER,
                        g INTEGER CONSTRAINT generated_value AS(CASE WHEN b BETWEEN 1 AND 4 THEN abs(-b)+CAST((b) AS INTEGER)+abs ELSE 1 END) STORED,
                        p TEXT CONSTRAINT own_key REFERENCES t(a) DEFERRABLE INITIALLY DEFERRED,
                        literal TEXT DEFAULT ('a b t abs'),
                        CONSTRAINT rules CHECK(t.b IN (1,2,3) AND b IS NOT NULL AND a LIKE '%' ESCAPE '!' AND length(a COLLATE NOCASE)>0),
                        CONSTRAINT pair UNIQUE(b,g), CONSTRAINT external_key FOREIGN KEY(b) REFERENCES outside(b)){options};
                    INSERT INTO {table}(a,b,abs,p) VALUES('parent',1,3,'parent'); BEGIN;",db::qualified_name(schema,"outside"),db::qualified_name(schema,"outside"))).unwrap();
                let original=db::table_column_defs_in(&conn,schema,"t").unwrap();
                let sources:Vec<_>=original.iter().map(|column|Some(column.name.clone())).collect();
                let mut draft=original.clone();
                for (column,name) in draft.iter_mut().zip(["b","a","magnitude","calculated","p","literal"]) { column.name=name.into(); }
                let sql:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name='t'",db::quote_ident(schema)),[],|row|row.get(0)).unwrap();
                let target="renamed.\"表";
                let renamed=preview_renames(&sql,target,&[("a","b"),("b","a"),("abs","magnitude"),("g","calculated")]).unwrap();
                let options=table_options(&sql).unwrap();
                let preview=format_table_preview(&rebuild_definition(&renamed,schema,target,&original,&draft,&sources,Some(options.clone())).unwrap()).unwrap();
                assert!(preview.contains("\n    ") && preview.contains("\n)"));
                db::recreate_table_columns_in(&conn,schema,"t",target,&draft,&sources,Some(options)).unwrap_or_else(|error|panic!("{schema}: {error}"));
                let saved:String=conn.query_row(&format!("SELECT sql FROM {}.sqlite_schema WHERE name=?1",db::quote_ident(schema)),[target],|row|row.get(0)).unwrap();
                let mut saved=parse(&saved).unwrap();
                let Cmd::Stmt(Stmt::CreateTable { tbl_name,.. })=&mut saved else { unreachable!() };
                *tbl_name=qualified(schema,target);
                assert_eq!(parse(&preview).unwrap(),saved,"{schema}: {preview}");
                let target=db::qualified_name(schema,target);
                assert_eq!(conn.query_row(&format!("SELECT calculated FROM {target} WHERE b='parent' AND a=1 AND magnitude=3 AND literal='a b t abs' AND p='parent'"),[],|row|row.get::<_,i64>(0)).unwrap(),5);
                conn.execute(&format!("UPDATE {target} SET a=2"),[]).unwrap();
                assert_eq!(conn.query_row(&format!("SELECT calculated FROM {target}"),[],|row|row.get::<_,i64>(0)).unwrap(),7);
                assert!(!conn.is_autocommit());
            }
        }
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(a TEXT,b \"a\" DEFAULT \"a\",g AS(coalesce(a,'a'))); INSERT INTO t(a) VALUES('kept'); BEGIN;").unwrap();
        let original=db::table_column_defs_in(&conn,"main","t").unwrap();
        let mut draft=original.clone(); draft[0].name="changed".into();
        let sources=original.iter().map(|column|Some(column.name.clone())).collect::<Vec<_>>();
        let sql:String=conn.query_row("SELECT sql FROM sqlite_schema WHERE name='t'",[],|row|row.get(0)).unwrap();
        let renamed=preview_renames(&sql,"t",&[("a","changed")]).unwrap();
        let preview=rebuild_definition(&renamed,"main","copy",&original,&draft,&sources,None).unwrap();
        conn.execute_batch(&preview).unwrap(); conn.execute("INSERT INTO copy(changed) VALUES('next')",[]).unwrap();
        assert_eq!(conn.query_row("SELECT b FROM copy",[],|row|row.get::<_,String>(0)).unwrap(),"a");
        assert_eq!(db::table_columns(&conn,"copy").unwrap()[1].type_name,"a");
    }

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
            "CREATE TABLE t(v REFERENCES parent(id) CHECK(v>0) CONSTRAINT [时限] /* comment */ DEFERRABLE INITIALLY DEFERRED)",
            "CREATE TABLE t(v REFERENCES parent(id) CONSTRAINT first DEFERRABLE CONSTRAINT last NOT DEFERRABLE INITIALLY IMMEDIATE,note DEFAULT 'CONSTRAINT text DEFERRABLE __next_gui_named_deferral')",
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
