//! Attached database actions preserve drafts and never implicitly commit pending writes.
use crate::{
    db,
    state::{AppState, BrowseTabState, ConfirmKind},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub schema: String,
    file: String,
    database: String,
}

impl Target {
    pub(crate) fn read(state: &AppState, schema: &str) -> Result<Self, String> {
        if schema.eq_ignore_ascii_case("main") || schema.eq_ignore_ascii_case("temp") {
            return Err("Select an attached database".into());
        }
        let conn = state.conn.as_ref().ok_or("No database open")?;
        let mut stmt = conn
            .prepare("PRAGMA database_list")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (name, file) = row.map_err(|e| e.to_string())?;
            if name == schema {
                return Ok(Self {
                    schema: name,
                    file,
                    database: state.path.clone(),
                });
            }
        }
        Err("The attached database no longer exists".into())
    }

    pub fn description(&self) -> String {
        let note = if self.file.is_empty() {
            "This in-memory database and its contents will be lost."
        } else {
            "The database file remains on disk."
        };
        format!(
            "Detach database {} from this session?\n{note}",
            db::quote_ident(&self.schema)
        )
    }
}

impl AppState {
    fn guard_detach(&self) -> Result<(), String> {
        self.guard_background_idle()?;
        if self.sql_busy {
            return Err("Stop or finish SQL before detaching a database".into());
        }
        if self.has_cell_draft() {
            return Err("Apply or discard the cell draft first".into());
        }
        if self.external_cell.is_some() {
            return Err("Apply or cancel the external editing session first".into());
        }
        let conn = self.conn.as_ref().ok_or("No database open")?;
        if self.dirty || db::has_pending_changes(conn)? {
            return Err("Write Changes or Revert before detaching a database".into());
        }
        Ok(())
    }

    pub fn prepare_detach(&mut self, schema: &str) -> Result<(), String> {
        self.guard_detach()?;
        self.prepare_detach_target(Target::read(self, schema)?)
    }

    pub fn prepare_detach_target(&mut self, target: Target) -> Result<(), String> {
        self.guard_detach()?;
        if Target::read(self, &target.schema)? != target {
            return Err("The attached database changed; choose Detach again".into());
        }
        self.pending_detach = Some(target);
        self.confirm_kind = ConfirmKind::Detach;
        self.confirm_open = true;
        Ok(())
    }

    pub fn detach_confirmed(&mut self) -> Result<(), String> {
        let target = self
            .pending_detach
            .take()
            .ok_or("The detach request is no longer current")?;
        if self.confirm_kind == ConfirmKind::Detach {
            self.confirm_open = false;
            self.confirm_kind = ConfirmKind::None;
        }
        self.guard_detach()?;
        if Target::read(self, &target.schema)? != target {
            return Err("The attached database changed; choose Detach again".into());
        }
        db::detach_database(self.conn.as_ref().unwrap(), &target.schema)?;
        self.log_sql(format!(
            "DETACH DATABASE {};",
            db::quote_ident(&target.schema)
        ));
        let schema = target.schema.as_str();
        let definition_was_clean = self
            .selected
            .as_ref()
            .and_then(|selected| {
                self.schema.iter().find(|o| {
                    o.schema == selected.schema
                        && o.kind == selected.kind
                        && o.name == selected.name
                })
            })
            .is_some_and(|o| o.sql.as_deref().unwrap_or_default() == self.ddl_draft);
        // TEMP views may depend on the detached schema, so invalidate every table cache.
        for tab in &mut self.browse_tabs {
            tab.schema_table_options.remove(schema);
            if tab.schema == schema {
                let main = std::mem::take(&mut tab.table_options);
                let others = std::mem::take(&mut tab.schema_table_options);
                *tab = BrowseTabState::new(None);
                tab.table_options = main;
                tab.schema_table_options = others;
            }
            tab.result = None;
            tab.row_keys.clear();
            tab.rowid_column = None;
            tab.total_rows = None;
            tab.clear_selection();
        }
        if !self.sql_cell_read_only() {
            self.clear_browse_editor();
        }
        if self
            .selected
            .as_ref()
            .is_some_and(|selected| selected.schema == schema)
        {
            self.selected = None;
            if definition_was_clean {
                self.ddl_draft.clear();
            }
        }
        if self
            .schema_group
            .as_ref()
            .is_some_and(|(name, _)| name == schema)
        {
            self.schema_group = None;
        }
        self.schema_tree_expanded.remove(schema);
        self.schema_dock.remove_schema(schema);
        self.cond_rules.retain(|rule| rule.schema != schema);
        self.cond_match.get_mut().take();
        if matches!(self.plot.source, Some(crate::plot::Source::Table { .. })) {
            self.plot.clear();
        }
        self.plot.settings.configs.retain(|config| !matches!(&config.source, crate::plot::Source::Table { schema: name, .. } if name == schema));
        if self.chart_schema == schema {
            self.chart_schema = "main".into();
            self.chart_table = None;
            self.chart_column.clear();
        }
        self.chart_points = std::sync::Arc::from([]);
        self.chart_state = next_gui_ui::PlotState::default();
        self.schema.retain(|object| object.schema != schema);
        self.schema_names.retain(|name| name != schema);
        self.sql_completion = crate::sql_completion::Catalog::default();
        let schema_error = self.refresh_schema().err();
        let browse_error = self.refresh_browse().err();
        let chart_error = self.refresh_chart().err();
        self.status_msg = format!("Detached {}", db::quote_ident(schema));
        if let Some(error) = schema_error {
            self.status_msg
                .push_str(&format!("; Schema unavailable: {error}"));
        }
        if let Some(error) = browse_error {
            self.status_msg
                .push_str(&format!("; Browse unavailable: {error}"));
        }
        if let Some(error) = chart_error {
            self.status_msg
                .push_str(&format!("; Plot unavailable: {error}"));
        }
        Ok(())
    }

    pub fn structure_detach_schema(&self) -> Option<&str> {
        self.selected
            .as_ref()
            .map(|o| o.schema.as_str())
            .or_else(|| {
                self.schema_group
                    .as_ref()
                    .map(|(schema, _)| schema.as_str())
            })
            .filter(|schema| {
                *schema != "main"
                    && *schema != "temp"
                    && self.schema_names.iter().any(|name| name == schema)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CondFormatRule, MainTab, SelectedObject};

    fn fixture() -> AppState {
        let mut state = AppState::empty();
        state.new_memory_db().unwrap();
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch(crate::schema_dock::QA_SQL)
            .unwrap();
        state.write_changes().unwrap();
        state.refresh_schema().unwrap();
        state.prefs.auto_switch_mode = false;
        state
    }

    #[test]
    fn detach_preserves_writes_files_and_other_aliases_and_releases_clean_read_locks() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("attached.db");
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE keep(v); INSERT INTO keep VALUES('saved'); BEGIN")
            .unwrap();
        db::attach_database(&conn, file.to_str().unwrap(), "aux.\"db").unwrap();
        conn.execute_batch(
            "CREATE TABLE \"aux.\"\"db\".records(v); INSERT INTO \"aux.\"\"db\".records VALUES(1)",
        )
        .unwrap();
        assert!(db::detach_database(&conn, "aux.\"db")
            .unwrap_err()
            .contains("Write Changes"));
        assert!(db::has_pending_changes(&conn).unwrap());
        db::commit_and_begin(&conn).unwrap();
        db::attach_database(&conn, file.to_str().unwrap(), "same_file").unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM \"aux.\"\"db\".records", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
        assert!(!conn.is_autocommit());
        let bytes = std::fs::read(&file).unwrap();
        for schema in [
            "main",
            "MAIN",
            "temp",
            "TEMP",
            "missing",
            "aux.\"db\"; DROP TABLE main.keep;--",
        ] {
            assert!(db::detach_database(&conn, schema).is_err());
        }
        db::detach_database(&conn, "aux.\"db").unwrap();
        assert!(!conn.is_autocommit());
        assert_eq!(db::database_names(&conn).unwrap(), ["main", "same_file"]);
        assert_eq!(
            conn.query_row("SELECT v FROM same_file.records", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(std::fs::read(&file).unwrap(), bytes);
        {
            let mut stmt = conn.prepare("SELECT v FROM same_file.records").unwrap();
            let mut rows = stmt.query([]).unwrap();
            assert!(rows.next().unwrap().is_some());
            assert!(db::detach_database(&conn, "same_file").is_err());
            assert!(db::database_names(&conn)
                .unwrap()
                .iter()
                .any(|name| name == "same_file"));
        }
        conn.execute_batch("INSERT INTO main.keep VALUES('pending')")
            .unwrap();
        assert!(db::detach_database(&conn, "same_file").is_err());
        db::rollback_and_begin(&conn).unwrap();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM main.keep", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        let ro = rusqlite::Connection::open_with_flags(
            &file,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        db::attach_database_mode(&ro, file.to_str().unwrap(), "read_only_alias", true).unwrap();
        db::detach_database(&ro, "read_only_alias").unwrap();
        assert!(ro.is_autocommit());
        assert!(file.exists());
    }

    #[test]
    fn detach_reconciles_qualified_tabs_settings_and_plots_without_replaying_sql_or_losing_drafts()
    {
        let mut state = fixture();
        let aux = "aux.\"db";
        state.tab = MainTab::Browse;
        for (i, schema) in ["main", "temp", aux].into_iter().enumerate() {
            if i > 0 {
                state.add_browse_tab().unwrap();
            }
            state.switch_browse_table_in(schema, "records").unwrap();
            state.browse_mut().column_widths = vec![180.0, 260.0];
            let options = state.browse().options();
            state
                .browse_mut()
                .schema_table_options
                .entry(schema.into())
                .or_default()
                .insert("records".into(), options);
        }
        state
            .run_sql_text_sync("SELECT 9223372036854775807 AS snapshot;")
            .unwrap();
        let snapshot = state.sql_result.as_ref().unwrap().values.identity();
        *state.sql_editor_mut() = "SELECT 'independent SQL draft';".into();
        state.selected = Some(SelectedObject {
            schema: aux.into(),
            kind: db::ObjectKind::Table,
            name: "records".into(),
        });
        state.ddl_draft = "-- independent DDL draft".into();
        state.chart_schema = aux.into();
        state.chart_table = Some("records".into());
        state.chart_column = "id".into();
        state.refresh_chart().unwrap();
        assert!(!state.chart_points.is_empty());
        for schema in ["main", aux] {
            state.cond_rules.push(CondFormatRule {
                schema: schema.into(),
                table: Some("records".into()),
                column: "id".into(),
                op: 2,
                value: "2".into(),
                color_rgb: 0x336699,
                style: Default::default(),
                native_format: None,
            });
            state.plot.settings.configs.push(crate::plot::Config {
                source: crate::plot::Source::Table {
                    schema: schema.into(),
                    table: "records".into(),
                },
                x: None,
                series: Vec::new(),
            });
        }
        state.plot.settings.configs.push(crate::plot::Config {
            source: crate::plot::Source::Query(0),
            x: None,
            series: Vec::new(),
        });
        state.prepare_detach(aux).unwrap();
        assert!(state
            .pending_detach
            .as_ref()
            .unwrap()
            .description()
            .contains("will be lost"));
        state.detach_confirmed().unwrap();
        assert!(!state.dirty);
        assert!(!state.conn.as_ref().unwrap().is_autocommit());
        assert_eq!(state.browse_tabs.len(), 3);
        assert_eq!(state.browse().schema, "main");
        assert!(state.browse().table.is_none());
        assert!(state.edit_col.is_empty());
        assert!(state.selected.is_none());
        assert_eq!(state.ddl_draft, "-- independent DDL draft");
        assert_eq!(state.sql_editor(), "SELECT 'independent SQL draft';");
        assert_eq!(
            state.sql_result.as_ref().unwrap().values.identity(),
            snapshot
        );
        assert!(state.chart_table.is_none());
        assert!(state.chart_points.is_empty());
        assert!(state.plot.source.is_none());
        assert_eq!(state.plot.settings.configs.len(), 2);
        assert_eq!(state.cond_rules.len(), 1);
        assert!(!state.schema_names.iter().any(|name| name == aux));
        assert!(state
            .sql_completion
            .columns_in(aux, "records")
            .next()
            .is_none());
        for tab in &state.browse_tabs {
            assert!(!tab.schema_table_options.contains_key(aux));
            assert!(tab.result.is_none());
            assert!(tab.row_keys.is_empty());
        }
        for (i, value) in [(0, "保留"), (1, "")] {
            state.switch_browse_tab(i).unwrap();
            assert_eq!(state.browse().column_widths, [180.0, 260.0]);
            if i == 0 {
                assert_eq!(state.browse().result.as_ref().unwrap().rows[0][1], value);
            }
        }
        assert!(state.detach_confirmed().is_err());
    }

    #[test]
    fn detach_guards_dirty_flags_real_writes_drafts_workers_stale_targets_and_dependent_temp_views()
    {
        let mut state = fixture();
        let aux = "aux.\"db";
        state.dirty = true;
        assert!(state
            .prepare_detach(aux)
            .unwrap_err()
            .contains("Write Changes"));
        state.dirty = false;
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("INSERT INTO main.records VALUES(2,'untracked')")
            .unwrap();
        assert!(state
            .prepare_detach(aux)
            .unwrap_err()
            .contains("Write Changes"));
        state.revert_changes().unwrap();
        state.tab = MainTab::Browse;
        state.switch_browse_table_in("main", "records").unwrap();
        state.goto_browse_record(1).unwrap();
        state.browse_mut().sel_col = Some(1);
        state.sync_edit_cell_from_selection();
        state.edit_value = "pending cell".into();
        assert!(state
            .prepare_detach(aux)
            .unwrap_err()
            .contains("cell draft"));
        assert_eq!(state.edit_value, "pending cell");
        state.discard_cell_draft();
        state.sql_busy = true;
        assert!(state
            .prepare_detach(aux)
            .unwrap_err()
            .contains("finish SQL"));
        state.sql_busy = false;
        state.prepare_detach(aux).unwrap();
        state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("INSERT INTO main.records VALUES(2,'after confirmation')")
            .unwrap();
        assert!(state
            .detach_confirmed()
            .unwrap_err()
            .contains("Write Changes"));
        assert!(db::has_pending_changes(state.conn.as_ref().unwrap()).unwrap());
        assert!(db::database_names(state.conn.as_ref().unwrap())
            .unwrap()
            .iter()
            .any(|name| name == aux));
        state.revert_changes().unwrap();
        let parked = state.conn.take();
        assert!(state.prepare_detach(aux).is_err());
        state.conn = parked;
        state.prepare_detach(aux).unwrap();
        db::detach_database(state.conn.as_ref().unwrap(), aux).unwrap();
        assert!(state.detach_confirmed().is_err());
        assert!(state.pending_detach.is_none());
        db::attach_database(state.conn.as_ref().unwrap(), ":memory:", aux).unwrap();
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE \"aux.\"\"db\".records(v); INSERT INTO \"aux.\"\"db\".records VALUES(1); CREATE TEMP VIEW dependent AS SELECT * FROM \"aux.\"\"db\".records;").unwrap();
        state.write_changes().unwrap();
        state.refresh_schema().unwrap();
        state.switch_browse_table_in("temp", "dependent").unwrap();
        assert!(state.browse().result.is_some());
        state.prepare_detach(aux).unwrap();
        state.detach_confirmed().unwrap();
        assert!(state.browse().result.is_none());
        assert!(state.status_msg.contains("Browse unavailable"));
        assert_eq!(
            db::count_rows(state.conn.as_ref().unwrap(), "records").unwrap(),
            1
        );
    }
}
