//! DB4S-style `.sqbpro` project files (JSON subset of XML project).

use std::fs;
use std::path::{Path, PathBuf};

use crate::state::{AppState, BrowseTabState, MainTab};

#[derive(Debug, Clone, Default)]
pub struct ProjectFile {
    pub db_path: String,
    pub main_tab: usize,
    pub browse_table: Option<String>,
    pub browse_filter: String,
    pub sql_tabs: Vec<String>,
    pub sql_tab_idx: usize,
    pub dock_edit: bool,
    pub dock_schema: bool,
    pub dock_log: bool,
    pub dock_plot: bool,
    pub frozen_cols: usize,
    pub toolbar_db: bool,
    pub toolbar_extra: bool,
    pub toolbar_project: bool,
    pub toolbar_sql: bool,
}

impl ProjectFile {
    pub fn from_state(state: &AppState) -> Self {
        let b = state.browse();
        Self {
            db_path: state.path.clone(),
            main_tab: state.tab.index(),
            browse_table: b.table.clone(),
            browse_filter: b.filter.clone(),
            sql_tabs: state.sql_tabs.clone(),
            sql_tab_idx: state.sql_tab_idx,
            dock_edit: state.dock_edit,
            dock_schema: state.dock_schema,
            dock_log: state.dock_log,
            dock_plot: state.dock_plot,
            frozen_cols: state.frozen_cols,
            toolbar_db: state.toolbar_db,
            toolbar_extra: state.toolbar_extra,
            toolbar_project: state.toolbar_project,
            toolbar_sql: state.toolbar_sql,
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let mut s = String::from("{\n");
        s.push_str(&format!("  \"version\": 1,\n"));
        s.push_str(&format!("  \"db_path\": {},\n", json_str(&self.db_path)));
        s.push_str(&format!("  \"main_tab\": {},\n", self.main_tab));
        match &self.browse_table {
            Some(t) => s.push_str(&format!("  \"browse_table\": {},\n", json_str(t))),
            None => s.push_str("  \"browse_table\": null,\n"),
        }
        s.push_str(&format!("  \"browse_filter\": {},\n", json_str(&self.browse_filter)));
        s.push_str("  \"sql_tabs\": [\n");
        for (i, t) in self.sql_tabs.iter().enumerate() {
            s.push_str(&format!("    {}", json_str(t)));
            if i + 1 < self.sql_tabs.len() {
                s.push(',');
            }
            s.push('\n');
        }
        s.push_str("  ],\n");
        s.push_str(&format!("  \"sql_tab_idx\": {},\n", self.sql_tab_idx));
        s.push_str(&format!("  \"dock_edit\": {},\n", self.dock_edit));
        s.push_str(&format!("  \"dock_schema\": {},\n", self.dock_schema));
        s.push_str(&format!("  \"dock_log\": {},\n", self.dock_log));
        s.push_str(&format!("  \"dock_plot\": {},\n", self.dock_plot));
        s.push_str(&format!("  \"frozen_cols\": {},\n", self.frozen_cols));
        s.push_str(&format!("  \"toolbar_db\": {},\n", self.toolbar_db));
        s.push_str(&format!("  \"toolbar_extra\": {},\n", self.toolbar_extra));
        s.push_str(&format!("  \"toolbar_project\": {},\n", self.toolbar_project));
        s.push_str(&format!("  \"toolbar_sql\": {}\n", self.toolbar_sql));
        s.push_str("}\n");
        fs::write(path, s).map_err(|e| e.to_string())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut p = ProjectFile::default();
        p.dock_edit = true;
        p.dock_schema = true;
        p.dock_log = true;
        p.dock_plot = true;
        p.toolbar_db = true;
        p.toolbar_extra = true;
        p.toolbar_project = true;
        p.toolbar_sql = true;
        p.db_path = extract_str(&text, "db_path").unwrap_or_default();
        p.main_tab = extract_usize(&text, "main_tab").unwrap_or(0);
        p.browse_table = extract_str(&text, "browse_table");
        p.browse_filter = extract_str(&text, "browse_filter").unwrap_or_default();
        p.sql_tabs = extract_str_array(&text, "sql_tabs");
        p.sql_tab_idx = extract_usize(&text, "sql_tab_idx").unwrap_or(0);
        p.dock_edit = extract_bool(&text, "dock_edit").unwrap_or(true);
        p.dock_schema = extract_bool(&text, "dock_schema").unwrap_or(true);
        p.dock_log = extract_bool(&text, "dock_log").unwrap_or(true);
        p.dock_plot = extract_bool(&text, "dock_plot").unwrap_or(true);
        p.frozen_cols = extract_usize(&text, "frozen_cols").unwrap_or(0);
        p.toolbar_db = extract_bool(&text, "toolbar_db").unwrap_or(true);
        p.toolbar_extra = extract_bool(&text, "toolbar_extra").unwrap_or(true);
        p.toolbar_project = extract_bool(&text, "toolbar_project").unwrap_or(true);
        p.toolbar_sql = extract_bool(&text, "toolbar_sql").unwrap_or(true);
        Ok(p)
    }

    pub fn apply_to(&self, state: &mut AppState) {
        state.tab = MainTab::from_index(self.main_tab);
        state.dock_edit = self.dock_edit;
        state.dock_schema = self.dock_schema;
        state.dock_log = self.dock_log;
        state.dock_plot = self.dock_plot;
        state.frozen_cols = self.frozen_cols;
        state.toolbar_db = self.toolbar_db;
        state.toolbar_extra = self.toolbar_extra;
        state.toolbar_project = self.toolbar_project;
        state.toolbar_sql = self.toolbar_sql;
        if !self.sql_tabs.is_empty() {
            state.sql_tabs = self.sql_tabs.clone();
            state.sql_tab_idx = self.sql_tab_idx.min(state.sql_tabs.len().saturating_sub(1));
        }
        if let Some(t) = &self.browse_table {
            state.browse_tabs = vec![BrowseTabState::new(Some(t.clone()))];
            state.browse_tab_idx = 0;
            state.browse_mut().filter = self.browse_filter.clone();
        }
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn extract_str(text: &str, key: &str) -> Option<String> {
    let pat = format!("\"{}\"", key);
    let i = text.find(&pat)?;
    let rest = &text[i + pat.len()..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    if after.starts_with("null") {
        return None;
    }
    if !after.starts_with('"') {
        return None;
    }
    parse_json_string(after)
}

fn parse_json_string(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'"') {
        return None;
    }
    let mut out = String::new();
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => return Some(out),
            b'\\' if i + 1 < bytes.len() => {
                match bytes[i + 1] {
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'u' if i + 5 < bytes.len() => {
                        let hex = std::str::from_utf8(&bytes[i + 2..i + 6]).ok()?;
                        let cp = u32::from_str_radix(hex, 16).ok()?;
                        if let Some(ch) = char::from_u32(cp) {
                            out.push(ch);
                        }
                        i += 4;
                    }
                    c => out.push(c as char),
                }
                i += 2;
            }
            c => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    None
}

fn extract_usize(text: &str, key: &str) -> Option<usize> {
    let pat = format!("\"{}\"", key);
    let i = text.find(&pat)?;
    let rest = &text[i + pat.len()..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    let num: String = after
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    num.parse().ok()
}

fn extract_bool(text: &str, key: &str) -> Option<bool> {
    let pat = format!("\"{}\"", key);
    let i = text.find(&pat)?;
    let rest = &text[i + pat.len()..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    if after.starts_with("true") {
        Some(true)
    } else if after.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn extract_str_array(text: &str, key: &str) -> Vec<String> {
    let pat = format!("\"{}\"", key);
    let Some(i) = text.find(&pat) else {
        return Vec::new();
    };
    let rest = &text[i + pat.len()..];
    let Some(bracket) = rest.find('[') else {
        return Vec::new();
    };
    let mut s = &rest[bracket + 1..];
    let mut out = Vec::new();
    loop {
        s = s.trim_start();
        if s.starts_with(']') || s.is_empty() {
            break;
        }
        if s.starts_with(',') {
            s = &s[1..];
            continue;
        }
        if let Some(v) = parse_json_string(s) {
            // Scan length of JSON string literal
            let mut j = 1usize;
            let b = s.as_bytes();
            while j < b.len() {
                if b[j] == b'\\' {
                    j += 2;
                    continue;
                }
                if b[j] == b'"' {
                    break;
                }
                j += 1;
            }
            s = &s[j + 1..];
            out.push(v);
        } else {
            break;
        }
    }
    out
}

pub fn default_project_name(db_path: &str) -> PathBuf {
    let p = Path::new(db_path);
    if db_path.is_empty() || db_path == ":memory:" {
        PathBuf::from("untitled.sqbpro")
    } else {
        p.with_extension("sqbpro")
    }
}
