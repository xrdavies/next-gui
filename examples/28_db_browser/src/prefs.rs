//! User preferences persisted under the example dir or ~/.config/e28_db_browser.

use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Prefs {
    pub font_size: f32,
    pub default_theme_dark: bool,
    pub confirm_drops: bool,
    pub csv_delimiter: char,
    pub frozen_cols_default: usize,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            font_size: 13.0,
            default_theme_dark: true,
            confirm_drops: true,
            csv_delimiter: ',',
            frozen_cols_default: 0,
        }
    }
}

impl Prefs {
    pub fn config_path() -> PathBuf {
        if let Ok(home) = std::env::var("HOME") {
            let dir = PathBuf::from(home).join(".config").join("e28_db_browser");
            let _ = fs::create_dir_all(&dir);
            return dir.join("prefs.json");
        }
        PathBuf::from("examples/28_db_browser/.e28_prefs")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let Ok(text) = fs::read_to_string(&path) else {
            return Self::default();
        };
        let mut p = Self::default();
        if let Some(v) = extract_f32(&text, "font_size") {
            p.font_size = v;
        }
        if let Some(v) = extract_bool(&text, "default_theme_dark") {
            p.default_theme_dark = v;
        }
        if let Some(v) = extract_bool(&text, "confirm_drops") {
            p.confirm_drops = v;
        }
        if let Some(s) = extract_str(&text, "csv_delimiter") {
            if let Some(c) = s.chars().next() {
                p.csv_delimiter = c;
            }
        }
        if let Some(v) = extract_usize(&text, "frozen_cols_default") {
            p.frozen_cols_default = v;
        }
        p
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let s = format!(
            "{{
  \"font_size\": {},
  \"default_theme_dark\": {},
  \"confirm_drops\": {},
  \"csv_delimiter\": \"{}\",
  \"frozen_cols_default\": {}
}}
",
            self.font_size,
            self.default_theme_dark,
            self.confirm_drops,
            escape_char(self.csv_delimiter),
            self.frozen_cols_default
        );
        fs::write(path, s).map_err(|e| e.to_string())
    }
}

fn escape_char(c: char) -> String {
    match c {
        '"' => "\\\"".into(),
        '\\' => "\\\\".into(),
        '\t' => "\\t".into(),
        c => c.to_string(),
    }
}

fn extract_str(text: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let i = text.find(&pat)?;
    let rest = &text[i + pat.len()..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    if !after.starts_with('"') {
        return None;
    }
    let mut out = String::new();
    let b = after.as_bytes();
    let mut i = 1;
    while i < b.len() {
        match b[i] {
            b'"' => return Some(out),
            b'\\' if i + 1 < b.len() => {
                out.push(match b[i + 1] {
                    b'n' => '\n',
                    b't' => '\t',
                    b'"' => '"',
                    b'\\' => '\\',
                    c => c as char,
                });
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

fn extract_bool(text: &str, key: &str) -> Option<bool> {
    let pat = format!("\"{key}\"");
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

fn extract_usize(text: &str, key: &str) -> Option<usize> {
    let pat = format!("\"{key}\"");
    let i = text.find(&pat)?;
    let rest = &text[i + pat.len()..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    let num: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    num.parse().ok()
}

fn extract_f32(text: &str, key: &str) -> Option<f32> {
    let pat = format!("\"{key}\"");
    let i = text.find(&pat)?;
    let rest = &text[i + pat.len()..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    let num: String = after
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    num.parse().ok()
}
