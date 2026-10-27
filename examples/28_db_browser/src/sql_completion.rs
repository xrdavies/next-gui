//! Cached SQLite metadata and lexical completion context; never executes editor SQL.

use std::ops::Range;
use std::sync::OnceLock;

use next_gui_ui::TextAreaCompletionItem;
use rusqlite::Connection;

use crate::{
    db::quote_ident,
    sql_syntax::{self, Kind},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    Keyword,
    Function,
    Schema,
    Table,
    Column,
}

impl Category {
    fn label(self) -> &'static str {
        match self {
            Self::Keyword => "Keyword",
            Self::Function => "Function",
            Self::Schema => "Database",
            Self::Table => "Table",
            Self::Column => "Column",
        }
    }
}

#[derive(Debug, Clone)]
struct Entry {
    path: Vec<String>,
    category: Category,
}

#[derive(Debug, Clone)]
pub struct Catalog {
    entries: Vec<Entry>,
    pub warning: Option<String>,
}

impl Default for Catalog {
    fn default() -> Self {
        let mut catalog = Self {
            entries: Vec::new(),
            warning: None,
        };
        // SQLite owns each non-NUL-terminated keyword slice for the library lifetime.
        unsafe {
            for i in 0..rusqlite::ffi::sqlite3_keyword_count() {
                let mut ptr = std::ptr::null();
                let mut len = 0;
                if rusqlite::ffi::sqlite3_keyword_name(i, &mut ptr, &mut len)
                    == rusqlite::ffi::SQLITE_OK
                    && !ptr.is_null()
                    && len > 0
                {
                    if let Ok(name) =
                        std::str::from_utf8(std::slice::from_raw_parts(ptr.cast(), len as usize))
                    {
                        for name in [name.to_owned(), name.to_ascii_lowercase()] {
                            catalog.add(vec![name], Category::Keyword);
                        }
                    }
                }
            }
        }
        static BUILTINS: OnceLock<Result<Vec<String>, String>> = OnceLock::new();
        match BUILTINS.get_or_init(|| {
            Connection::open_in_memory()
                .and_then(|conn| function_names(&conn))
                .map_err(|e| e.to_string())
        }) {
            Ok(names) => catalog.add_functions(names),
            Err(error) => catalog.warning = Some(error.clone()),
        }
        catalog
    }
}

impl Catalog {
    fn add(&mut self, path: Vec<String>, category: Category) {
        self.entries.push(Entry { path, category });
    }

    pub fn load(conn: &Connection) -> Self {
        let mut catalog = Self::default();
        if let Err(error) = catalog.read(conn) {
            catalog.warning = Some(error.to_string());
        }
        catalog
    }

    fn add_functions(&mut self, names: &[String]) {
        for name in names {
            for name in [name.to_ascii_lowercase(), name.to_ascii_uppercase()] {
                self.add(vec![name], Category::Function);
            }
        }
    }

    fn read(&mut self, conn: &Connection) -> rusqlite::Result<()> {
        let functions = function_names(conn)?;
        self.entries
            .retain(|entry| entry.category != Category::Function);
        self.add_functions(&functions);
        let mut stmt = conn.prepare("PRAGMA database_list")?;
        let mut schemas = stmt
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if !schemas.iter().any(|s| s == "temp") {
            schemas.push("temp".into());
        }
        for schema in schemas {
            self.add(vec![schema.clone()], Category::Schema);
            let mut tables = conn.prepare(&format!(
                "SELECT name FROM {}.sqlite_schema WHERE type IN ('table','view') ORDER BY name",
                quote_ident(&schema)
            ))?;
            for table in tables.query_map([], |r| r.get::<_, String>(0))? {
                let table = table?;
                self.add(vec![table.clone()], Category::Table);
                self.add(vec![schema.clone(), table.clone()], Category::Table);
                let columns = (|| {
                    let mut stmt = conn.prepare("SELECT name FROM pragma_table_xinfo(?1, ?2)")?;
                    let columns = stmt
                        .query_map([&table, &schema], |r| r.get::<_, String>(0))?
                        .collect::<rusqlite::Result<Vec<_>>>()?;
                    Ok::<_, rusqlite::Error>(columns)
                })();
                match columns {
                    Ok(columns) => {
                        for column in columns {
                            self.add(vec![column.clone()], Category::Column);
                            self.add(vec![table.clone(), column.clone()], Category::Column);
                            self.add(
                                vec![schema.clone(), table.clone(), column],
                                Category::Column,
                            );
                        }
                    }
                    Err(error) => {
                        self.warning = Some(format!("{schema}.{table}: {error}"));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn suggest(
        &self,
        sql: &str,
        caret: usize,
        explicit: bool,
    ) -> Option<(Range<usize>, Vec<TextAreaCompletionItem>)> {
        // shortcut: synchronous lexical/catalog scans; move off-thread when profiling large scripts/schemas.
        let context = Context::parse(sql, caret)?;
        if !explicit && context.prefix.chars().count() < 3 {
            return None;
        }
        let mut items = Vec::new();
        for entry in &self.entries {
            if context.quote.is_some()
                && matches!(entry.category, Category::Keyword | Category::Function)
            {
                continue;
            }
            if entry.path.len() != context.parents.len() + 1
                || !entry
                    .path
                    .iter()
                    .zip(&context.parents)
                    .all(|(a, b)| a.eq_ignore_ascii_case(b))
            {
                continue;
            }
            let name = entry.path.last().unwrap();
            if !name.starts_with(&context.prefix) {
                continue;
            }
            let text = if matches!(entry.category, Category::Keyword | Category::Function) {
                name.clone()
            } else {
                identifier_text(name, context.quote)
            };
            let label = format!(
                "{}{}  ·  {}",
                name.replace('\n', "\\n").replace('\r', "\\r"),
                if entry.category == Category::Function {
                    "()"
                } else {
                    ""
                },
                entry.category.label()
            );
            items.push(TextAreaCompletionItem { label, text });
        }
        items.sort_by(|a, b| a.label.cmp(&b.label));
        items.dedup();
        Some((context.range, items))
    }
}

fn function_names(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT DISTINCT name FROM pragma_function_list ORDER BY name")?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    rows.collect()
}

fn identifier_text(name: &str, quote: Option<char>) -> String {
    match quote {
        Some('`') => format!("`{}`", name.replace('`', "``")),
        Some('[') if !name.contains(']') => format!("[{name}]"),
        Some(_) => quote_ident(name),
        None if name
            .chars()
            .next()
            .is_some_and(|c| !c.is_ascii_digit() && c != '$' && sql_syntax::identifier(c))
            && name.chars().all(sql_syntax::identifier)
            && unsafe {
                rusqlite::ffi::sqlite3_keyword_check(
                    name.as_ptr().cast(),
                    name.len().try_into().unwrap_or(0),
                ) == 0
            } =>
        {
            name.to_owned()
        }
        None => quote_ident(name),
    }
}

struct Context {
    range: Range<usize>,
    parents: Vec<String>,
    prefix: String,
    quote: Option<char>,
}

impl Context {
    fn parse(sql: &str, caret: usize) -> Option<Self> {
        let chars: Vec<_> = sql.chars().collect();
        if caret > chars.len() {
            return None;
        }
        let prefix: String = chars[..caret].iter().collect();
        if sql_syntax::tokens(&prefix).iter().any(|(r, k)| {
            r.end == caret
                && matches!(k, Kind::Comment | Kind::Literal | Kind::Parameter)
                && !(*k == Kind::Comment
                    && (prefix.ends_with('\n')
                        || chars[r.start..caret].starts_with(&['/', '*'])
                            && prefix.ends_with("*/")))
        }) {
            return None;
        }
        let quoted = sql_syntax::tokens(sql)
            .into_iter()
            .filter_map(|(r, k)| (k == Kind::Identifier).then_some(r))
            .collect::<Vec<_>>();
        let current = quoted.iter().find(|r| r.start < caret && caret <= r.end);
        let (start, end, quote, name) = if let Some(r) = current {
            let quote = chars[r.start];
            let closer = if quote == '[' { ']' } else { quote };
            let closed = if quote == '[' {
                chars.get(caret - 1) == Some(&closer)
            } else {
                chars[r.start + 1..caret]
                    .iter()
                    .rev()
                    .take_while(|c| **c == quote)
                    .count()
                    % 2
                    == 1
            };
            let end_prefix = if caret == r.end && closed {
                caret - 1
            } else {
                caret
            };
            (
                r.start,
                r.end,
                Some(quote),
                decode(&chars[r.start + 1..end_prefix], quote),
            )
        } else {
            let mut start = caret;
            while start > 0 && sql_syntax::identifier(chars[start - 1]) {
                start -= 1;
            }
            let mut end = caret;
            while end < chars.len() && sql_syntax::identifier(chars[end]) {
                end += 1;
            }
            if chars.get(start).is_some_and(char::is_ascii_digit) {
                return None;
            }
            (start, end, None, chars[start..caret].iter().collect())
        };
        let mut parents = Vec::new();
        let mut before = start;
        while before > 0 && chars[before - 1] == '.' {
            before -= 1;
            let name = if let Some(r) = quoted.iter().find(|r| r.end == before) {
                let name = decode(&chars[r.start + 1..r.end - 1], chars[r.start]);
                before = r.start;
                name
            } else {
                let end = before;
                while before > 0 && sql_syntax::identifier(chars[before - 1]) {
                    before -= 1;
                }
                if before == end {
                    return None;
                }
                chars[before..end].iter().collect()
            };
            parents.push(name);
        }
        parents.reverse();
        // shortcut: SQLite object paths only; add query-scope alias/CTE resolution with semantic completion.
        Some(Self {
            range: start..end,
            parents,
            prefix: name,
            quote,
        })
    }
}

fn decode(chars: &[char], quote: char) -> String {
    let raw: String = chars.iter().collect();
    if quote == '[' {
        raw
    } else {
        raw.replace(&format!("{quote}{quote}"), &quote.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn suggestions(c: &Catalog, sql: &str) -> Vec<String> {
        c.suggest(sql, sql.chars().count(), true)
            .map(|(_, v)| v.into_iter().map(|i| i.text).collect())
            .unwrap_or_default()
    }

    #[test]
    fn completion_catalog_reads_all_schemas_columns_and_live_functions_without_changes() {
        let conn = Connection::open_in_memory().unwrap();
        conn.create_scalar_function(
            "custom_completion",
            0,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            |_| Ok(1),
        )
        .unwrap();
        conn.execute_batch("CREATE TABLE \"Order Items\"(\"select\", \"中文列\", \"a\"\"b\"); CREATE TEMP TABLE transient(temp_column); ATTACH ':memory:' AS extra; CREATE TABLE extra.contacts(email, full_name); CREATE VIEW broken AS SELECT * FROM missing;").unwrap();
        let changes = conn.total_changes();
        let c = Catalog::load(&conn);
        assert!(c.warning.as_ref().is_some_and(|w| w.contains("broken")));
        assert_eq!(suggestions(&c, "SELECT SELE"), vec!["SELECT"]);
        assert_eq!(suggestions(&c, "SELECT sele"), vec!["\"select\"", "select"]);
        assert!(suggestions(&c, "SELECT low").contains(&"lower".into()));
        assert_eq!(
            suggestions(&c, "SELECT custom_comp"),
            vec!["custom_completion"]
        );
        let disconnected = Catalog::default();
        assert_eq!(suggestions(&disconnected, "SELECT low"), vec!["lower"]);
        assert!(suggestions(&disconnected, "SELECT custom_comp").is_empty());
        assert!(suggestions(&c, "SELECT temp.").contains(&"transient".into()));
        assert_eq!(suggestions(&c, "SELECT extra.contacts.ema"), vec!["email"]);
        assert_eq!(
            suggestions(&c, "SELECT \"Order Items\".sel"),
            vec!["\"select\""]
        );
        assert_eq!(
            suggestions(&c, "SELECT \"Order Items\".\"a\"\""),
            vec!["\"a\"\"b\""]
        );
        assert_eq!(
            suggestions(&c, "SELECT [Order Items].[中文"),
            vec!["[中文列]"]
        );
        assert_eq!(
            suggestions(&c, "SELECT `Order Items`.`中文"),
            vec!["`中文列`"]
        );
        assert!(suggestions(&c, "SELECT \"Order Items\".\"").contains(&"\"select\"".into()));
        let sql = "SELECT main.\"Order Items\".\"中文列\"";
        let caret = sql.chars().position(|ch| ch == '文').unwrap() + 1;
        let (range, items) = c.suggest(sql, caret, true).unwrap();
        assert_eq!(items.len(), 1);
        let replaced = format!(
            "{}{}{}",
            sql.chars().take(range.start).collect::<String>(),
            items[0].text,
            sql.chars().skip(range.end).collect::<String>()
        );
        assert_eq!(
            replaced, sql,
            "mid-identifier completion consumes the existing closing quote"
        );
        assert_eq!(conn.total_changes(), changes);
        assert!(conn.is_autocommit());
    }

    #[test]
    fn completion_context_suppresses_literals_comments_parameters_and_replaces_the_whole_identifier(
    ) {
        let c = Catalog::default();
        for sql in [
            "'sele",
            "-- sele",
            "/* sele",
            "X'AB",
            ":sele",
            "$name::sele",
            "12sel",
            "SELECT 1 -- */",
        ] {
            assert!(c.suggest(sql, sql.chars().count(), true).is_none(), "{sql}");
        }
        assert!(c.suggest("se", 2, false).is_none());
        assert!(c.suggest("sel", 3, false).is_some());
        assert!(c.suggest("SELECT \"", 8, true).is_some());
        for sql in ["-- comment\n", "/* comment */"] {
            assert!(c.suggest(sql, sql.chars().count(), true).is_some());
        }
        let (range, items) = c.suggest("SELECT", 3, true).unwrap();
        assert_eq!(range, 0..6);
        assert!(items.iter().any(|i| i.text == "SELECT"));
        assert!(c.suggest("x", usize::MAX, true).is_none());
    }
}
