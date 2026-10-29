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
    pub sql_upper_keywords: bool,
    pub csv_delimiter: char,
    pub csv_import: Option<crate::db::CsvImportOptions>,
    pub data_export: crate::data_export::Options,
    pub sql_export: crate::db::ExportSqlOptions,
    pub csv_separate_tables: bool,
    pub frozen_cols_default: usize,
    pub toolbar_styles: [usize; 5],
    pub editor_word_wrap: bool,
    pub auto_switch_mode: bool,
    pub indent_compact: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            font_size: 13.0,
            default_theme_dark: true,
            confirm_drops: true,
            sql_completion: true,
            sql_upper_keywords: true,
            csv_delimiter: ',',
            csv_import: None,
            data_export: crate::data_export::Options::default(),
            sql_export: crate::db::ExportSqlOptions::default(),
            csv_separate_tables: false,
            frozen_cols_default: 0,
            toolbar_styles: [2, 2, 0, 0, 0],
            editor_word_wrap: true,
            auto_switch_mode: true,
            indent_compact: false,
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
        for style in &mut p.toolbar_styles { *style = (*style).min(4); }
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

pub const TOOLBAR_GROUPS: [&str; 5] = ["Main window", "Database Structure", "Browse Data", "Execute SQL", "Edit Cell"];
pub const TOOLBAR_STYLES: [&str; 5] = ["Only display the icon", "Only display the text", "Text beside the icon", "Text under the icon", "Follow the style"];

pub fn toolbar_layout(style: usize) -> next_gui_ui::ButtonIconLayout {
    use next_gui_ui::ButtonIconLayout::*;
    // shortcut: Follow Style uses the Qt macOS/Windows fallback; add Linux desktop-style integration on that host.
    match style { 1 => TextOnly, 2 => TextBesideIcon, 3 => TextUnderIcon, _ => IconOnly }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_preferences_enable_completion_and_explicit_false_round_trips() {
        assert!(!serde_json::from_str::<Prefs>("{}").unwrap().indent_compact);
        let indent:Prefs=serde_json::from_str(r#"{"indent_compact":true}"#).unwrap();
        assert!(serde_json::from_str::<Prefs>(&serde_json::to_string(&indent).unwrap()).unwrap().indent_compact);
        assert!(serde_json::from_str::<Prefs>("{}").unwrap().auto_switch_mode);
        let auto:Prefs=serde_json::from_str(r#"{"auto_switch_mode":false}"#).unwrap();
        assert!(!serde_json::from_str::<Prefs>(&serde_json::to_string(&auto).unwrap()).unwrap().auto_switch_mode);
        assert!(serde_json::from_str::<Prefs>("{}").unwrap().editor_word_wrap);
        let wrap:Prefs=serde_json::from_str(r#"{"editor_word_wrap":false}"#).unwrap();
        assert!(!serde_json::from_str::<Prefs>(&serde_json::to_string(&wrap).unwrap()).unwrap().editor_word_wrap);
        assert!(serde_json::from_str::<Prefs>("{}").unwrap().sql_completion);
        assert!(serde_json::from_str::<Prefs>("{}").unwrap().sql_upper_keywords);
        let prefs: Prefs = serde_json::from_str(r#"{"sql_completion":false,"sql_upper_keywords":false}"#).unwrap();
        let restored: Prefs = serde_json::from_str(&serde_json::to_string(&prefs).unwrap()).unwrap();
        assert!(!restored.sql_completion && !restored.sql_upper_keywords);
        assert_eq!(restored.toolbar_styles, [2, 2, 0, 0, 0]);
        let prefs: Prefs = serde_json::from_str(r#"{"toolbar_styles":[0,1,2,3,4]}"#).unwrap();
        let restored: Prefs = serde_json::from_str(&serde_json::to_string(&prefs).unwrap()).unwrap();
        assert_eq!(restored.toolbar_styles, [0,1,2,3,4]);
        use next_gui_ui::ButtonIconLayout::*;
        assert_eq!(restored.toolbar_styles.map(toolbar_layout), [IconOnly,TextOnly,TextBesideIcon,TextUnderIcon,IconOnly]);
    }
}
