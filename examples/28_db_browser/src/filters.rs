//! DB4S v3.13.1 CondFormat::filterToSqlCondition syntax.

use rusqlite::{functions::FunctionFlags, types::ValueRef, Connection};

fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn number(value: &str) -> Option<f32> {
    let value = value.trim();
    if !value
        .bytes()
        .all(|b| b.is_ascii_digit() || b"+-.eE".contains(&b))
    {
        return None;
    }
    value.parse::<f32>().ok().filter(|v| v.is_finite())
}

pub fn validate_escape(escape: &str) -> Result<(), String> {
    if escape.chars().count() > 1 || escape.contains('\0') { return Err("Filter escape must be empty or one character, without NUL".into()); }
    Ok(())
}

pub fn condition(value: &str, escape: &str) -> Option<String> {
    if value.is_empty() || value == "%" || value == "%%" {
        return None;
    }
    if let Some((low, high)) = value.split_once('~') {
        if number(low).zip(number(high)).is_some_and(|(a, b)| a < b) {
            return Some(format!("BETWEEN {} AND {}", low.trim(), high.trim()));
        }
    }
    if value == "=NULL" {
        return Some("IS NULL".into());
    }
    if value == "<>NULL" {
        return Some("IS NOT NULL".into());
    }
    for op in [">=", "<=", "<>", ">", "<", "="] {
        if let Some(operand) = value.strip_prefix(op) {
            let operand = if operand.is_empty() { value } else { operand };
            if operand == "%" || operand == "%%" {
                return None;
            }
            let operand = if operand == "''" || (op != "=" && number(operand).is_some()) {
                operand.trim().into()
            } else {
                literal(operand)
            };
            return Some(format!("{op} {operand}"));
        }
    }
    if value.len() > 2 && value.starts_with('/') && value.ends_with('/') {
        return Some(format!("REGEXP {}", literal(&value[1..value.len() - 1])));
    }
    let operand = if value.contains('%') {
        literal(value)
    } else {
        literal(&format!("%{value}%"))
    };
    Some(if escape.is_empty() { format!("LIKE {operand}") } else { format!("LIKE {operand} ESCAPE {}", literal(escape)) })
}

fn text(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => String::new(),
        ValueRef::Integer(v) => v.to_string(),
        ValueRef::Real(v) => format!("{v:?}"),
        ValueRef::Text(v) | ValueRef::Blob(v) => String::from_utf8_lossy(v).into_owned(),
    }
}

fn compile_regexp(pattern: &str) -> Result<pcre2::bytes::Regex, pcre2::Error> {
    // Bound backtracking inside one SQLite callback, where Stop cannot interrupt it.
    pcre2::bytes::RegexBuilder::new().ucp(true).build(&format!(
        "(*LIMIT_MATCH=1000000)(*LIMIT_DEPTH=1000){pattern}"
    ))
}

pub fn validate(value: &str) -> Result<(), String> {
    if value.len() > 2 && value.starts_with('/') && value.ends_with('/') {
        compile_regexp(&value[1..value.len() - 1]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn register_regexp(conn: &Connection) -> Result<(), String> {
    conn.create_scalar_function(
        "regexp",
        2,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let regex = ctx.get_or_create_aux(0, |pattern| compile_regexp(&text(pattern)))?;
            regex
                .is_match(text(ctx.get_raw(1)).as_bytes())
                .map_err(|e| rusqlite::Error::UserFunctionError(e.into()))
        },
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_like_escape_is_quoted_and_matches_literal_wildcards() {
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(v); INSERT INTO t VALUES('A_B'),('A%B'),('A\\B'),('AXB'),(NULL);").unwrap();
        for (filter,escape,expected) in [(r"A\_B","\\","A_B"),("A!%B","!","A%B"),("A'%B","'","A%B"),("A界_B","界","A_B"),(r"A\\B","\\",r"A\B")] {
            validate_escape(escape).unwrap();let sql=format!("SELECT v FROM t WHERE v {}",condition(filter,escape).unwrap());
            let rows=conn.prepare(&sql).unwrap().query_map([],|row|row.get::<_,String>(0)).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();assert_eq!(rows,[expected],"{filter} {escape}");
        }
        let sql=format!("SELECT count(*) FROM t WHERE v {}",condition("A_B","").unwrap());assert_eq!(conn.query_row(&sql,[],|row|row.get::<_,i64>(0)).unwrap(),4);
        assert_eq!(condition("=NULL","!").as_deref(),Some("IS NULL"));assert_eq!(condition("/A.B/","!").as_deref(),Some("REGEXP 'A.B'"));
        for invalid in ["ab","🙂x","\0"] {assert!(validate_escape(invalid).is_err());}
        for valid in ["","\\","'","🙂"] {validate_escape(valid).unwrap();}
    }

    #[test]
    fn original_filter_syntax_and_sql_quoting() {
        for (input, expected) in [
            ("foo", "LIKE '%foo%'"),
            ("foo%", "LIKE 'foo%'"),
            ("a_b", "LIKE '%a_b%'"),
            (" O'Reilly ", "LIKE '% O''Reilly %'"),
            ("10~20", "BETWEEN 10 AND 20"),
            (" -2.5 ~ 1e2 ", "BETWEEN -2.5 AND 1e2"),
            ("20~10", "LIKE '%20~10%'"),
            (">=2", ">= 2"),
            ("<=+2e3", "<= +2e3"),
            ("<>3", "<> 3"),
            (">=z", ">= 'z'"),
            ("=2", "= '2'"),
            ("=NULL", "IS NULL"),
            ("<>NULL", "IS NOT NULL"),
            ("=null", "= 'null'"),
            ("=''", "= ''"),
            ("<>''", "<> ''"),
            ("''", "LIKE '%''''%'"),
            ("=", "= '='"),
            ("/a(?=b)/", "REGEXP 'a(?=b)'"),
            ("/O'Reilly/", "REGEXP 'O''Reilly'"),
            (">0 OR 1=1 --", "> '0 OR 1=1 --'"),
            (">NaN", "> 'NaN'"),
            (">1e999", "> '1e999'"),
        ] {
            assert_eq!(condition(input, "").as_deref(), Some(expected), "{input}");
        }
        for input in ["", "%", "%%"] {
            assert_eq!(condition(input, ""), None);
        }
    }

    #[test]
    fn sqlite_regexp_supports_unicode_lookaround_backreferences_and_errors() {
        let conn = crate::db::open_path(":memory:").unwrap();
        for (pattern, value, expected) in [
            (r"^\w+$", "東京", true),
            (r"(?<=ab)c", "abc", true),
            (r"^(\w+) \1$", "hello hello", true),
            ("^x", "abc", false),
        ] {
            let result: bool = conn
                .query_row("SELECT ?2 REGEXP ?1", [pattern, value], |r| r.get(0))
                .unwrap();
            assert_eq!(result, expected, "{pattern}");
        }
        assert_eq!(
            conn.query_row("SELECT NULL REGEXP '^$', 12 REGEXP '^12$'", [], |r| Ok((
                r.get::<_, bool>(0)?,
                r.get::<_, bool>(1)?
            )))
            .unwrap(),
            (true, true)
        );
        assert!(conn
            .query_row("SELECT 'a' REGEXP '['", [], |r| r.get::<_, bool>(0))
            .is_err());
        let long = format!("{}!", "a".repeat(100));
        assert!(conn
            .query_row("SELECT ?1 REGEXP '^(a+)+$'", [long], |r| r
                .get::<_, bool>(0))
            .is_err());
    }
}
