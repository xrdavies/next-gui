//! Per-connection SQLite tracing and categorized, bounded session history.

use std::cell::Cell;
use std::collections::VecDeque;
use std::ffi::{c_void, CStr};
use std::ops::Deref;
use std::sync::{Arc, Mutex};

use rusqlite::{ffi, Connection};

const RETAINED_PER_SOURCE: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    User,
    Application,
    Error,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Self::User => "User",
            Self::Application => "Application",
            Self::Error => "Error",
        }
    }

    pub fn from_filter(index: usize) -> Option<Self> {
        [Self::User, Self::Application, Self::Error]
            .get(index)
            .copied()
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub sequence: u64,
    pub source: Source,
    pub text: String,
}

#[derive(Default)]
struct Store {
    entries: [VecDeque<Entry>; 3],
    evicted: [usize; 3],
    next: u64,
}

#[derive(Clone, Default)]
pub struct SqlLog(Arc<Mutex<Store>>);

impl SqlLog {
    pub fn revision(&self) -> u64 {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).next
    }

    pub fn push(&self, source: Source, text: impl Into<String>) {
        let text = text.into();
        let text = if sensitive(&text) {
            "[SQLCipher key statement redacted]".into()
        } else {
            text
        };
        let mut store = self.0.lock().unwrap_or_else(|e| e.into_inner());
        store.next += 1;
        let sequence = store.next;
        let index = source as usize;
        store.entries[index].push_back(Entry {
            sequence,
            source,
            text,
        });
        if store.entries[index].len() > RETAINED_PER_SOURCE {
            store.entries[index].pop_front();
            store.evicted[index] += 1;
        }
    }

    pub fn snapshot(&self, source: Option<Source>) -> (Vec<Entry>, usize) {
        let store = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let mut entries = Vec::new();
        let mut evicted = 0;
        for (index, buffer) in store.entries.iter().enumerate() {
            if source.is_none_or(|s| s as usize == index) {
                entries.extend(buffer.iter().cloned());
                evicted += store.evicted[index];
            }
        }
        entries.sort_by_key(|e| e.sequence);
        (entries, evicted)
    }

    pub fn text(&self, source: Option<Source>) -> String {
        let (entries, evicted) = self.snapshot(source);
        let mut out = format!(
            "-- {} retained; {evicted} earlier entries evicted\n",
            entries.len()
        );
        for entry in entries {
            out.push_str(&format!(
                "\n-- #{} · {}\n{}\n",
                entry.sequence,
                entry.source.label(),
                entry.text
            ));
        }
        out
    }

    pub fn clear(&self, source: Option<Source>) {
        let mut store = self.0.lock().unwrap_or_else(|e| e.into_inner());
        store.next += 1;
        for index in 0..3 {
            if source.is_none_or(|s| s as usize == index) {
                store.entries[index].clear();
                store.evicted[index] = 0;
            }
        }
    }
}

/// Conservatively redact an entire batch containing SQLCipher key/rekey syntax.
pub fn sensitive(sql: &str) -> bool {
    for uri in sql.to_ascii_lowercase().split("file:").skip(1) {
        if let Some((_, query)) = uri.split_once('?') {
            for parameter in query.split('&') {
                let Some((name, _)) = parameter.split_once('=') else {
                    continue;
                };
                let mut decoded = Vec::new();
                let mut bytes = name.as_bytes().iter().copied();
                while let Some(b) = bytes.next() {
                    if b == b'%' {
                        let hex: Vec<u8> = bytes.by_ref().take(2).collect();
                        if let Ok(hex) = std::str::from_utf8(&hex) {
                            if let Ok(value) = u8::from_str_radix(hex, 16) {
                                if value == 0 {
                                    break;
                                }
                                decoded.push(value);
                                continue;
                            }
                        }
                    }
                    decoded.push(b);
                }
                if matches!(decoded.as_slice(), b"key" | b"hexkey" | b"textkey") {
                    return true;
                }
            }
        }
    }
    fn statement(tokens: &[String]) -> bool {
        let tokens = if tokens.first().is_some_and(|s| s == "explain") {
            let start = if tokens.get(1).is_some_and(|s| s == "query")
                && tokens.get(2).is_some_and(|s| s == "plan")
            {
                3
            } else {
                1
            };
            &tokens[start..]
        } else {
            tokens
        };
        match tokens.first().map(String::as_str) {
            Some("pragma") => tokens
                .iter()
                .skip(1)
                .take(2)
                .any(|s| matches!(s.as_str(), "key" | "rekey" | "hexkey" | "hexrekey")),
            Some("attach") => tokens.iter().any(|s| s == "key"),
            _ => false,
        }
    }
    let mut input = sql.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(c) = input.next() {
        if c == '-' && input.peek() == Some(&'-') {
            input.next();
            for c in input.by_ref() {
                if c == '\n' {
                    break;
                }
            }
        } else if c == '/' && input.peek() == Some(&'*') {
            input.next();
            while let Some(c) = input.next() {
                if c == '*' && input.peek() == Some(&'/') {
                    input.next();
                    break;
                }
            }
        } else if c == ';' {
            if statement(&tokens) {
                return true;
            }
            tokens.clear();
        } else if ['\'', '"', '`', '['].contains(&c) {
            let end = if c == '[' { ']' } else { c };
            let mut token = String::new();
            while let Some(c) = input.next() {
                if c == end {
                    if input.peek() != Some(&end) {
                        break;
                    }
                    input.next();
                }
                token.push(c);
            }
            tokens.push(token.to_ascii_lowercase());
        } else if c.is_ascii_alphanumeric() || c == '_' {
            let mut token = c.to_string();
            while input
                .peek()
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
            {
                token.push(input.next().unwrap());
            }
            tokens.push(token.to_ascii_lowercase());
        }
    }
    statement(&tokens)
}

struct TraceContext {
    log: SqlLog,
    source: Cell<Source>,
}

/// Own the callback context for exactly as long as its SQLite connection, including worker moves.
pub struct LoggedConnection {
    inner: Connection,
    context: Box<TraceContext>,
}

impl LoggedConnection {
    pub fn new(inner: Connection) -> Result<Self, String> {
        let mut conn = Self {
            inner,
            context: Box::new(TraceContext {
                log: SqlLog::default(),
                source: Cell::new(Source::Application),
            }),
        };
        // The boxed context stays at a stable address until Drop unregisters this callback.
        let code = unsafe {
            ffi::sqlite3_trace_v2(
                conn.inner.handle(),
                ffi::SQLITE_TRACE_STMT as u32,
                Some(trace_statement),
                (&mut *conn.context as *mut TraceContext).cast(),
            )
        };
        if code != ffi::SQLITE_OK {
            return Err(format!("Cannot install SQL trace: {code}"));
        }
        Ok(conn)
    }

    pub fn bind_log(&mut self, log: &SqlLog) {
        if Arc::ptr_eq(&self.context.log.0, &log.0) {
            return;
        }
        let (entries, _) = self.context.log.snapshot(None);
        for entry in entries {
            log.push(entry.source, entry.text);
        }
        let evicted = self
            .context
            .log
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .evicted;
        let mut store = log.0.lock().unwrap_or_else(|e| e.into_inner());
        for (dest, count) in store.evicted.iter_mut().zip(evicted) {
            *dest += count;
        }
        drop(store);
        self.context.log = log.clone();
    }

    pub fn user_sql<T>(&self, run: impl FnOnce() -> T) -> T {
        struct Restore<'a>(&'a Cell<Source>, Source);
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.0.set(self.1);
            }
        }
        let _restore = Restore(
            &self.context.source,
            self.context.source.replace(Source::User),
        );
        run()
    }
}

impl Deref for LoggedConnection {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        &self.inner
    }
}

impl Drop for LoggedConnection {
    fn drop(&mut self) {
        // Ownership prevents concurrent SQLite use; unregister before releasing callback storage.
        unsafe {
            ffi::sqlite3_trace_v2(self.inner.handle(), 0, None, std::ptr::null_mut());
        }
    }
}

unsafe extern "C" fn trace_statement(
    _: u32,
    context: *mut c_void,
    statement: *mut c_void,
    sql: *mut c_void,
) -> i32 {
    // SQLite supplies live statement/context and NUL-terminated SQL for SQLITE_TRACE_STMT.
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let context = unsafe { &*context.cast::<TraceContext>() };
        let sql = unsafe { CStr::from_ptr(sql.cast()) }.to_string_lossy();
        struct SqliteText(*mut std::ffi::c_char);
        impl Drop for SqliteText {
            fn drop(&mut self) {
                unsafe {
                    ffi::sqlite3_free(self.0.cast());
                }
            }
        }
        let text = if sql.starts_with("--") {
            sql.into_owned()
        } else {
            let expanded = SqliteText(unsafe { ffi::sqlite3_expanded_sql(statement.cast()) });
            if expanded.0.is_null() {
                sql.into_owned()
            } else {
                unsafe { CStr::from_ptr(expanded.0) }
                    .to_string_lossy()
                    .into_owned()
            }
        };
        context.log.push(context.source.get(), text);
    }));
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_is_connection_scoped_and_survives_worker_moves() {
        let first = SqlLog::default();
        let second = SqlLog::default();
        let mut a = LoggedConnection::new(Connection::open_in_memory().unwrap()).unwrap();
        let mut b = LoggedConnection::new(Connection::open_in_memory().unwrap()).unwrap();
        a.bind_log(&first);
        b.bind_log(&second);
        a.execute_batch(
            "CREATE TABLE t(v); CREATE TRIGGER changed AFTER INSERT ON t BEGIN SELECT 123; END;",
        )
        .unwrap();
        let a = std::thread::spawn(move || {
            a.user_sql(|| a.execute("INSERT INTO t VALUES (?1)", ["bound '中文' value"]))
                .unwrap();
            a
        })
        .join()
        .unwrap();
        a.query_row("SELECT count(*) FROM t", [], |r| r.get::<_, i64>(0))
            .unwrap();
        b.execute_batch("SELECT 'second connection'").unwrap();
        let first_text = first.text(None);
        assert!(first_text.contains("INSERT INTO t VALUES ('bound ''中文'' value')"));
        assert!(first_text.contains("-- TRIGGER changed"));
        assert!(!first_text.contains("second connection"));
        assert!(first
            .snapshot(Some(Source::User))
            .0
            .iter()
            .any(|e| e.text.starts_with("INSERT")));
        assert!(first
            .snapshot(Some(Source::Application))
            .0
            .iter()
            .any(|e| e.text.starts_with("SELECT count")));
        drop(a);
        drop(b);
        assert!(second.text(None).contains("second connection"));
    }

    #[test]
    fn retained_history_is_complete_categorized_and_eviction_is_visible() {
        let log = SqlLog::default();
        let long = format!("SELECT '{}';\nSELECT 2;", "中文🙂".repeat(100));
        log.push(Source::User, &long);
        for n in 0..205 {
            log.push(Source::Application, format!("SELECT {n}"));
        }
        assert!(log.text(Some(Source::User)).contains(&long));
        assert_eq!(log.snapshot(Some(Source::Application)).0.len(), 200);
        assert_eq!(log.snapshot(None).1, 5);
        log.clear(Some(Source::Application));
        assert_eq!(log.snapshot(None).0.len(), 1);
        assert_eq!(log.snapshot(None).1, 0);
        log.push(Source::Error, "error");
        log.clear(None);
        assert!(log.snapshot(None).0.is_empty());
    }

    #[test]
    fn cipher_keys_never_enter_history() {
        let log = SqlLog::default();
        for sql in [
            "PRAGMA key='private';",
            "/* lead */ PrAgMa/* ( */main.\"rekey\"('private');",
            "SELECT ';'; -- comment\nPRAGMA 'key'='private';",
            "PRAGMA [main].[key] = 'private';",
            "ATTACH DATABASE 'data' AS encrypted KEY 'private';",
            "EXPLAIN PRAGMA \"key\"='private';",
            "PRAGMA hexkey='private';",
            "PRAGMA main.hexrekey='private';",
            "ATTACH DATABASE 'file:db?textkey=private' AS encrypted;",
            "-- open file:db?mode=ro&%6B%65%79=private",
            "-- open file:db?key%00ignored=private",
        ] {
            assert!(sensitive(sql), "{sql}");
            log.push(Source::User, sql);
        }
        for sql in [
            "CREATE TABLE t(id PRIMARY KEY)",
            "SELECT 'PRAGMA key=not executed'",
            "PRAGMA foreign_keys=1",
            "ATTACH DATABASE 'keychain.db' AS extra",
        ] {
            assert!(!sensitive(sql), "{sql}");
        }
        assert!(!log.text(None).contains("private"));
        let mut conn = LoggedConnection::new(Connection::open_in_memory().unwrap()).unwrap();
        conn.bind_log(&log);
        conn.execute_batch("PRAGMA key='trace-private';").unwrap();
        conn.execute(
            "ATTACH DATABASE ':memory:' AS encrypted KEY ?1",
            ["bound-private"],
        )
        .unwrap();
        conn.execute(
            "ATTACH DATABASE ?1 AS uri_target",
            ["file:log-fixture?mode=memory&key=uri-private"],
        )
        .unwrap();
        assert!(!log.text(None).contains("trace-private"));
        assert!(!log.text(None).contains("bound-private"));
        assert!(!log.text(None).contains("uri-private"));
    }
}
