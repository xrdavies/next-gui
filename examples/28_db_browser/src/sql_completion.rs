//! Cached SQLite metadata and lexical completion context; never executes editor SQL.

use std::borrow::Cow;
use std::collections::BTreeMap;
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

struct RegisteredFunction {
    name: String,
    arity: i32,
    builtin: bool,
}

#[derive(Debug, Clone)]
pub struct Catalog {
    entries: Vec<Entry>,
    functions: BTreeMap<String, BTreeMap<i32, bool>>,
    pub warning: Option<String>,
}

impl Default for Catalog {
    fn default() -> Self {
        let mut catalog = Self {
            entries: Vec::new(),
            functions: BTreeMap::new(),
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
                        catalog.add(vec![name.to_ascii_uppercase()], Category::Keyword);
                    }
                }
            }
        }
        for word in sql_syntax::EXTRA_KEYWORDS {
            catalog.add(vec![(*word).to_owned()], Category::Keyword);
        }
        static BUILTINS: OnceLock<Result<Vec<RegisteredFunction>, String>> = OnceLock::new();
        match BUILTINS.get_or_init(|| {
            Connection::open_in_memory()
                .and_then(|conn| function_signatures(&conn))
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

    fn add_functions(&mut self, signatures: &[RegisteredFunction]) {
        self.functions.clear();
        for function in signatures {
            self.functions
                .entry(function.name.to_ascii_lowercase())
                .or_default()
                .entry(function.arity)
                .and_modify(|builtin| *builtin &= function.builtin)
                .or_insert(function.builtin);
        }
        let names = self.functions.keys().cloned().collect::<Vec<_>>();
        for name in names {
            for name in [name.to_ascii_lowercase(), name.to_ascii_uppercase()] {
                self.add(vec![name], Category::Function);
            }
        }
    }

    fn read(&mut self, conn: &Connection) -> rusqlite::Result<()> {
        let functions = function_signatures(conn)?;
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
        upper_keywords: bool,
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
            let name = if entry.category == Category::Keyword && !upper_keywords {
                Cow::Owned(name.to_ascii_lowercase())
            } else {
                Cow::Borrowed(name.as_str())
            };
            if !name.starts_with(&context.prefix) {
                continue;
            }
            let text = if matches!(entry.category, Category::Keyword | Category::Function) {
                name.to_string()
            } else {
                identifier_text(&name, context.quote)
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

    pub fn calltip(&self, sql: &str, caret: usize) -> Option<(usize, String)> {
        let context =
            crate::sql_calltip::context(sql, caret, |name| self.functions.contains_key(name))?;
        let arities = self.functions.get(&context.name)?;
        let forms = arities
            .iter()
            .map(|(&arity, &builtin)| {
                let help = builtin
                    .then(|| crate::sql_function_help::lookup(&context.name, arity))
                    .flatten();
                let params = help
                    .map(|(params, _)| params.to_owned())
                    .unwrap_or_else(|| {
                        if arity < 0 {
                            "arg1, …".to_owned()
                        } else {
                            (1..=arity)
                                .map(|i| format!("arg{i}"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        }
                    });
                (
                    arity,
                    params,
                    help.map(|(_, text)| text).unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        let active = forms
            .iter()
            .find(|(arity, _, _)| {
                usize::try_from(*arity).is_ok_and(|arity| arity > context.argument)
            })
            .or_else(|| forms.iter().find(|(arity, _, _)| *arity < 0))
            .or_else(|| forms.last())?;
        let signatures = forms
            .iter()
            .map(|(_, params, _)| format!("{}({params})", context.name))
            .collect::<Vec<_>>()
            .join("\n");
        let status = if active.0 == 0 {
            "No arguments".to_owned()
        } else {
            let params = active.1.split(", ").collect::<Vec<_>>();
            let parameter = params
                .get(context.argument)
                .or_else(|| params.last().filter(|p| p.contains('…')));
            if let Some(parameter) = parameter.filter(|p| !active.2.is_empty() && **p != "…") {
                format!(
                    "Argument {} · {}",
                    context.argument + 1,
                    parameter.trim_end_matches('…')
                )
            } else {
                format!("Argument {}", context.argument + 1)
            }
        };
        Some((
            context.anchor,
            format!(
                "{signatures}\n{status}{}{}",
                if active.2.is_empty() { "" } else { "\n" },
                active.2
            ),
        ))
    }
}

fn function_signatures(conn: &Connection) -> rusqlite::Result<Vec<RegisteredFunction>> {
    // PRAGMA cannot be shadowed by a user table named pragma_function_list.
    let mut stmt = conn.prepare("PRAGMA function_list")?;
    let rows = stmt.query_map([], |r| {
        Ok(RegisteredFunction {
            name: r.get("name")?,
            arity: r.get("narg")?,
            builtin: r.get("builtin")?,
        })
    })?;
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
            && !sql_syntax::keyword(name) =>
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
        c.suggest(sql, sql.chars().count(), true, true)
            .map(|(_, v)| v.into_iter().map(|i| i.text).collect())
            .unwrap_or_default()
    }

    #[test]
    fn user_tables_cannot_shadow_function_metadata() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE pragma_function_list(name, narg); INSERT INTO pragma_function_list VALUES('fake_metadata_function', 2147483647)").unwrap();
        let c = Catalog::load(&conn);
        assert_eq!(suggestions(&c, "low"), ["lower"]);
        assert!(suggestions(&c, "fake_metadata_").is_empty());
    }

    #[test]
    fn descriptive_calltips_name_parameters_and_respect_custom_overrides() {
        let conn = Connection::open_in_memory().unwrap();
        let builtin = Catalog::load(&conn);
        let sql = "substr('中文', 1,";
        let text = builtin.calltip(sql, sql.chars().count()).unwrap().1;
        assert!(text.contains("substr(string, start, length)"), "{text}");
        assert!(text.contains("Argument 3 · length"), "{text}");
        conn.create_scalar_function(
            "substr",
            3,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            |_| Ok(1),
        )
        .unwrap();
        let custom = Catalog::load(&conn);
        let text = custom.calltip(sql, sql.chars().count()).unwrap().1;
        assert!(text.contains("substr(arg1, arg2, arg3)"), "{text}");
        assert!(!text.contains("Argument 3 · length"), "{text}");
        assert!(!text.contains("Extract a substring"), "{text}");
        assert!(Catalog::default()
            .calltip(sql, sql.chars().count())
            .unwrap()
            .1
            .contains("Argument 3 · length"));
    }

    #[test]
    fn descriptive_calltips_follow_fixed_variadic_date_and_window_argument_roles() {
        let c = Catalog::default();
        for (sql, parameter) in [
            ("instr('abc',", "substring"),
            ("strftime('%Y', 'now', '+1 day',", "modifiers"),
            ("coalesce(NULL, 1, 2, 3,", "values"),
            ("printf('%s %s', 'a',", "values"),
            ("lead(value, 2,", "default"),
            ("group_concat(value,", "separator"),
            ("max(1, 2, 3,", "values"),
        ] {
            let text = c.calltip(sql, sql.chars().count()).unwrap().1;
            assert!(text.contains(&format!(" · {parameter}\n")), "{text}");
        }
        let zero = c.calltip("row_number(", 11).unwrap().1;
        assert!(zero.contains("No arguments\nRow position"));
        assert!(!zero.contains("Argument 1"));
    }

    #[test]
    fn math_help_follows_engine_availability_and_sqlite_argument_order() {
        let conn = Connection::open_in_memory().unwrap();
        let available = conn.prepare("SELECT atan2(1, 0)").is_ok();
        let c = Catalog::load(&conn);
        assert_eq!(c.calltip("atan2(1,", 8).is_some(), available);
        if available {
            let text = c.calltip("atan2(1,", 8).unwrap().1;
            assert!(text.contains("atan2(y, x)\nArgument 2 · x"), "{text}");
            let angle: f64 = conn
                .query_row("SELECT atan2(1, 0)", [], |r| r.get(0))
                .unwrap();
            assert!((angle - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        }
        conn.create_scalar_function(
            "coalesce",
            -1,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            |_| Ok(1),
        )
        .unwrap();
        let c = Catalog::load(&conn);
        let custom = c.calltip("coalesce(NULL,", 14).unwrap().1;
        assert_eq!(custom, "coalesce(arg1, …)\nArgument 2");
    }

    #[test]
    fn cached_calltips_include_overloads_variadic_zero_and_connection_specific_functions() {
        let conn = Connection::open_in_memory().unwrap();
        for arity in [0, 2] {
            conn.create_scalar_function(
                "custom_hint",
                arity,
                rusqlite::functions::FunctionFlags::SQLITE_UTF8,
                |_| Ok(1),
            )
            .unwrap();
        }
        let c = Catalog::load(&conn);
        drop(conn);
        let hint = |sql: &str| c.calltip(sql, sql.chars().count()).unwrap().1;
        assert_eq!(
            hint("custom_hint("),
            "custom_hint()\ncustom_hint(arg1, arg2)\nArgument 1"
        );
        assert_eq!(
            hint("random("),
            "random()\nNo arguments\nPseudorandom signed integer."
        );
        assert_eq!(
            hint("coalesce(1,"),
            "coalesce(first, second, values…)\nArgument 2 · second\nFirst non-NULL value."
        );
        let overloads = hint("substr('x', 1,");
        assert!(overloads.contains("substr(string, start)\nsubstr(string, start, length)"));
        assert!(overloads.contains("Argument 3 · length"));
        assert!(Catalog::default().calltip("custom_hint(", 12).is_none());
        assert!(c.calltip("unknown(", 8).is_none());
    }

    #[test]
    fn original_extra_editor_words_are_completion_candidates_and_syntax_keywords() {
        let c = Catalog::default();
        for word in [
            "ANY", "ROWID", "STORED", "STRICT", "INT", "INTEGER", "REAL", "TEXT", "BLOB",
            "NUMERIC", "CHAR",
        ] {
            let (_, candidates) = c.suggest(word, word.len(), true, true).unwrap();
            assert!(
                candidates
                    .iter()
                    .any(|item| item.text == word && item.label.ends_with("Keyword")),
                "missing original keyword candidate {word}"
            );
            assert_eq!(
                sql_syntax::tokens(word),
                vec![(0..word.len(), Kind::Keyword)],
                "missing original keyword color {word}"
            );
        }
    }

    #[test]
    fn keyword_case_preference_preserves_function_and_object_names() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE MixedCase(ColumnCase)")
            .unwrap();
        let c = Catalog::load(&conn);
        let candidates = |sql: &str, upper| {
            c.suggest(sql, sql.chars().count(), true, upper)
                .unwrap()
                .1
                .into_iter()
                .map(|i| i.text)
                .collect::<Vec<_>>()
        };
        assert_eq!(candidates("SELE", true), ["SELECT"]);
        assert!(candidates("sele", true).is_empty());
        assert_eq!(candidates("sele", false), ["select"]);
        assert!(candidates("SELE", false).is_empty());
        for upper in [true, false] {
            assert_eq!(candidates("LOW", upper), ["LOWER"]);
            assert_eq!(candidates("low", upper), ["lower"]);
            assert_eq!(candidates("MixedCase.Co", upper), ["ColumnCase"]);
        }
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
        assert_eq!(suggestions(&c, "SELECT sele"), vec!["\"select\""]);
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
        let (range, items) = c.suggest(sql, caret, true, true).unwrap();
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
            assert!(
                c.suggest(sql, sql.chars().count(), true, true).is_none(),
                "{sql}"
            );
        }
        assert!(c.suggest("se", 2, false, true).is_none());
        assert!(c.suggest("sel", 3, false, true).is_some());
        assert!(c.suggest("SELECT \"", 8, true, true).is_some());
        for sql in ["-- comment\n", "/* comment */"] {
            assert!(c.suggest(sql, sql.chars().count(), true, true).is_some());
        }
        let (range, items) = c.suggest("SELECT", 3, true, true).unwrap();
        assert_eq!(range, 0..6);
        assert!(items.iter().any(|i| i.text == "SELECT"));
        assert!(c.suggest("x", usize::MAX, true, true).is_none());
    }
}
