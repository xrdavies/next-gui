//! Example-specific JSON project files; not upstream DB4S XML.

use std::fs;
use std::path::Path;

use crate::state::{AppState, BrowseTabState, MainTab};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ProjectFile {
    pub db_path: String,
    pub main_tab: usize,
    pub browse_table: Option<String>,
    pub browse_filter: String,
    pub browse_tabs: Vec<BrowseTabState>,
    pub browse_tab_idx: usize,
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
            browse_tabs: state.browse_tabs.clone(),
            browse_tab_idx: state.browse_tab_idx,
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
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(&mut file, self).map_err(|e| e.to_string())?;
        file.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("Expected an e28 JSON project (DB4S XML is unsupported): {e}"))?;
        if !value.get("db_path").is_some_and(|v| v.is_string()) {
            return Err("Project is missing db_path".into());
        }
        serde_json::from_value(value).map_err(|e| e.to_string())
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
            state.sql_edit_states.clear();
            state.sql_tab_idx = self.sql_tab_idx.min(state.sql_tabs.len().saturating_sub(1));
        }
        if !self.browse_tabs.is_empty() {
            state.browse_tabs = self.browse_tabs.clone();
            state.browse_tab_idx = self.browse_tab_idx.min(state.browse_tabs.len() - 1);
        } else if let Some(t) = &self.browse_table {
            state.browse_tabs = vec![BrowseTabState::new(Some(t.clone()))];
            state.browse_tab_idx = 0;
            state.browse_mut().filter = self.browse_filter.clone();
        }
    }
}

impl Default for ProjectFile {
    fn default() -> Self {
        Self {
            db_path: String::new(),
            main_tab: 0,
            browse_table: None,
            browse_filter: String::new(),
            browse_tabs: Vec::new(),
            browse_tab_idx: 0,
            sql_tabs: Vec::new(),
            sql_tab_idx: 0,
            dock_edit: true,
            dock_schema: true,
            dock_log: true,
            dock_plot: true,
            frozen_cols: 0,
            toolbar_db: true,
            toolbar_extra: true,
            toolbar_project: true,
            toolbar_sql: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_roundtrips_unicode_and_rejects_invalid_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.sqbpro");
        let mut browse = BrowseTabState::new(Some("表".into()));
        browse.filter = "id>1".into();
        browse.offset = 500;
        browse.order_col = Some("name".into());
        browse.column_widths = vec![100.0, 180.0];
        browse.scroll = (15.0, 40.0);
        let project = ProjectFile {
            db_path: "数据库.db".into(),
            browse_tabs: vec![browse],
            sql_tabs: vec!["SELECT '中文\n\\\"';".into()],
            ..Default::default()
        };
        project.save(&path).unwrap();
        let restored = ProjectFile::load(&path).unwrap();
        assert_eq!(restored.db_path, project.db_path);
        assert_eq!(restored.sql_tabs, project.sql_tabs);
        assert_eq!(restored.browse_tabs[0].offset, 500);
        assert_eq!(restored.browse_tabs[0].order_col.as_deref(), Some("name"));
        assert_eq!(restored.browse_tabs[0].filter, "id>1");
        assert_eq!(restored.browse_tabs[0].column_widths, vec![100.0, 180.0]);
        assert_eq!(restored.browse_tabs[0].scroll, (15.0, 40.0));
        fs::write(&path, "<sqlb_project/>").unwrap();
        assert!(ProjectFile::load(&path).is_err());
        fs::write(&path, "{}").unwrap();
        assert!(ProjectFile::load(&path).is_err());
    }
}
