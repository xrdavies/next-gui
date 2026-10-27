//! User preferences persisted under the example dir or ~/.config/e28_db_browser.

use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub font_size: f32,
    pub default_theme_dark: bool,
    pub confirm_drops: bool,
    pub sql_completion: bool,
    pub csv_delimiter: char,
    pub frozen_cols_default: usize,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            font_size: 13.0,
            default_theme_dark: true,
            confirm_drops: true,
            sql_completion: true,
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
        let mut p: Self = serde_json::from_str(&text).unwrap_or_default();
        p.font_size = p.font_size.clamp(8.0, 32.0);
        p.frozen_cols_default = p.frozen_cols_default.min(20);
        p
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, text).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_preferences_enable_completion_and_explicit_false_round_trips() {
        assert!(serde_json::from_str::<Prefs>("{}").unwrap().sql_completion);
        let prefs: Prefs = serde_json::from_str(r#"{"sql_completion":false}"#).unwrap();
        assert!(!serde_json::from_str::<Prefs>(&serde_json::to_string(&prefs).unwrap()).unwrap().sql_completion);
    }
}
