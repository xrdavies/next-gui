//! Preserve SQLite constraints while changing the fields exposed by the table wizard.

use fallible_iterator::FallibleIterator;
use sqlite3_parser::{
    ast::{
        fmt::ToTokens, Cmd, ColumnConstraint, ColumnDefinition, CreateTableBody, Expr,
        Name, NamedColumnConstraint, NamedTableConstraint, QualifiedName, ResolveType,
        SortedColumn, Stmt, TabFlags, TableConstraint,
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
    pub foreign_keys: Option<Vec<NamedForeignKey>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NamedCheck {
    pub name: Option<String>,
    pub expression: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NamedForeignKey {
    pub name: Option<String>,
    pub columns: String,
    pub clause: String,
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
        foreign_keys: None,
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
    set_primary_key_conflict(&mut command, options.primary_key_conflict)?;
    if let Some(checks) = options.checks { set_table_checks(&mut command, &checks)?; }
    if let Some(keys) = options.foreign_keys { set_table_foreign_keys(&mut command, &keys)?; }
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
            Some(NamedCheck { name: item.name.as_ref().map(identifier_text), expression: expression.to_string() })
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
        if expression == original { return Ok(check.clone()); }
        Ok(NamedCheck { name: check.name.clone(), expression })
    }).collect()
}

fn check_definition(check: &NamedCheck) -> Result<NamedTableConstraint, String> {
    if check.expression.trim().is_empty() { return Err("A table CHECK requires an expression".into()); }
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

fn set_table_checks(command: &mut Cmd, checks: &[NamedCheck]) -> Result<(), String> {
    replace_table_constraints(command, checks.iter().map(check_definition).collect::<Result<Vec<_>, _>>()?, |constraint| matches!(constraint, TableConstraint::Check(_)))
}

fn foreign_key_fields(item: &NamedTableConstraint) -> Option<NamedForeignKey> {
    let TableConstraint::ForeignKey { columns, clause, deref_clause } = &item.constraint else { return None; };
    Some(NamedForeignKey {
        name: item.name.as_ref().map(identifier_text),
        columns: columns.iter().map(|column| SqlFragment(&column.col_name).to_string()).collect::<Vec<_>>().join(", "),
        clause: format!("REFERENCES {}{}", SqlFragment(clause), deref_clause.as_ref().map(|defer| format!(" {}", SqlFragment(defer))).unwrap_or_default()),
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

pub fn rename_foreign_keys(keys: &[NamedForeignKey], table: &str, target: &str, renames: &[(&str, &str)]) -> Result<Vec<NamedForeignKey>, String> {
    keys.iter().map(|key| {
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
    replace_table_constraints(command, keys.iter().map(foreign_key_definition).collect::<Result<Vec<_>, _>>()?, |constraint| matches!(constraint, TableConstraint::ForeignKey { .. }))
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
    match expr {
        Expr::Name(value) => value.eq(name),
        Expr::Id(value) => Name(value.0.clone()).eq(name),
        Expr::Collate(value, _) => column_expression(value, name),
        _ => false,
    }
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
                for column in columns { rename_table_expression(&mut column.expr, &original_table, Some(target), renames)?; }
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
            if old.foreign_key_clause != col.foreign_key_clause {
                // shortcut: the field edits the first inline FK; the advanced tab must expose every FK.
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
        set_primary_key_conflict(&mut command, options.primary_key_conflict)?;
        if let Some(checks) = options.checks { set_table_checks(&mut command, &checks)?; }
        if let Some(keys) = options.foreign_keys { set_table_foreign_keys(&mut command, &keys)?; }
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
    let Some(start) = constraints
        .iter()
        .position(|item| matches!(item.constraint, ColumnConstraint::ForeignKey { .. }))
    else {
        return Vec::new();
    };
    let mut indices = vec![start];
    // SQLite associates DEFERRABLE with the last FK, even across CHECK/NOT NULL constraints.
    for (index, item) in constraints.iter().enumerate().skip(start + 1) {
        match item.constraint {
            ColumnConstraint::ForeignKey { .. } => break,
            ColumnConstraint::Defer(..) => indices.push(index),
            _ => {}
        }
    }
    indices
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
            .map(|index| SqlFragment(&column.constraints[index].constraint).to_string())
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
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn check_identity_rewrites_preserve_literals_functions_types_and_unrelated_sessions() {
        let check=NamedCheck { name:Some("a".into()), expression:"t.a < b AND abs(a)>0 AND CAST(a AS a)>0 AND a COLLATE a != 'a'".into() };
        let unchanged=NamedCheck { name:None,expression:"length('kept') > 0 -- keep comment".into() };
        let renamed=rename_checks(&[check.clone(),unchanged.clone()],"t","renamed",&[("a","b"),("b","a")]).unwrap();
        assert_eq!(renamed[0].name,check.name); assert_eq!(renamed[1],unchanged);
        let expression=&renamed[0].expression;
        assert!(expression.contains("\"renamed\".\"b\" < \"a\"") && expression.contains("abs (\"b\")") && expression.contains("AS a") && expression.contains("COLLATE a") && expression.contains("'a'"),"{expression}");
        assert!(rename_checks(&[NamedCheck { name:None,expression:"a +".into() }],"t","renamed",&[("a","b")]).is_err());
        assert_eq!(check.expression,"t.a < b AND abs(a)>0 AND CAST(a AS a)>0 AND a COLLATE a != 'a'");
        let case = rename_checks(&[NamedCheck { name:None,expression:"t.a > 0".into() }],"t","T",&[("a","A")]).unwrap();
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
