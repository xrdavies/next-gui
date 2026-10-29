//! Optional SQLite rowid column uses the same typed cells and mutation guards as table fields.
use crate::{db, state::AppState};

pub const QA_SQL: &str = "CREATE TABLE measurements(region TEXT,reading REAL,note TEXT,doubled REAL GENERATED ALWAYS AS (reading*2));
WITH RECURSIVE seq(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM seq WHERE n<18)
INSERT INTO measurements(region,reading,note) SELECT CASE WHEN n%2=0 THEN 'Tokyo' ELSE 'Paris' END,n*1.25,'Measurement '||n FROM seq;";

impl AppState {
    pub fn set_show_rowid(&mut self, show: bool) -> Result<(), String> {
        self.guard_background_idle()?;
        if self.sql_busy { return Err("Wait for SQL to finish before changing Browse columns".into()); }
        if self.has_cell_draft() { return Err("Apply or discard the cell draft first".into()); }
        let conn = self.conn.as_ref().ok_or("No database open")?;
        let table = self.browse().table.as_deref().ok_or("Select a table first")?;
        if show && db::implicit_rowid_column_in(conn, &self.browse().schema, table)?.is_none() {
            return Err("This source has no accessible rowid".into());
        }
        if self.browse().show_row_id == show { return Ok(()); }
        let previous = self.browse().clone();
        let frozen = previous.frozen_cols.unwrap_or(self.frozen_cols).min(previous.result.as_ref().map_or(0, |result| result.columns.len()));
        let mut options = previous.options();
        options.remap_source(conn, &previous.schema, table, show)?;
        self.browse_mut().set_options(options);
        if let Err(error) = self.refresh_browse() { *self.browse_mut() = previous; return Err(error); }
        self.browse_mut().frozen_cols = Some(if show && frozen > 0 { frozen + 1 } else if show { 0 } else { frozen.saturating_sub(1) });
        if let (Some(old), Some(new)) = (&previous.result, &self.browse().result) {
            let map = |index: Option<usize>| index.and_then(|index| old.columns.get(index)).and_then(|name| new.columns.iter().position(|column| column == name));
            let columns = [previous.sel_col, previous.sel_col_start, previous.sel_col_end].map(map);
            let rows = [previous.sel_row, previous.sel_row_start, previous.sel_row_end].map(|index| index.and_then(|index| previous.row_keys.get(index)).and_then(|key| self.browse().row_keys.iter().position(|candidate| candidate == key)));
            let contiguous = previous.sel_row.zip(rows[0]).is_some_and(|(old, new)| {
                let first = old.min(previous.sel_row_start.unwrap_or(old)).min(previous.sel_row_end.unwrap_or(old));
                let last = old.max(previous.sel_row_start.unwrap_or(old)).max(previous.sel_row_end.unwrap_or(old));
                new.checked_sub(old - first).is_some_and(|start| self.browse().row_keys.get(start..start + last - first + 1) == previous.row_keys.get(first..=last))
            });
            if contiguous && [previous.sel_col, previous.sel_col_start, previous.sel_col_end].into_iter().zip(columns).all(|(old, new)| old.is_none() || new.is_some()) {
                let tab = self.browse_mut();
                [tab.sel_row, tab.sel_row_start, tab.sel_row_end] = rows;
                [tab.sel_col, tab.sel_col_start, tab.sel_col_end] = columns;
                self.sync_edit_cell_from_selection();
            }
        }
        self.status_msg = if show { "Rowid column shown" } else { "Rowid column hidden" }.into();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{state::{MainTab, DockKind}, Demo, PresentBackend, Theme, UiKey};
    use rusqlite::{Connection, types::Value};

    fn fixture() -> AppState {
        let mut state = AppState::empty();
        state.new_memory_db().unwrap();
        state.conn.as_ref().unwrap().execute_batch(QA_SQL).unwrap();
        state.write_changes().unwrap();
        state.refresh_schema().unwrap();
        state.tab = MainTab::Browse;
        state.prefs.auto_switch_mode = false;
        state.switch_browse_table("measurements").unwrap();
        state
    }

    fn select(state: &mut AppState, row: usize, col: usize) {
        state.browse_mut().clear_selection();
        state.browse_mut().sel_row = Some(row);
        state.browse_mut().sel_col = Some(col);
        state.sync_edit_cell_from_selection();
    }

    #[test]
    fn implicit_rowid_projection_uses_real_qualified_aliases_and_exact_values() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("ATTACH ':memory:' AS \"aux space\";
            CREATE TABLE main.t(a); INSERT INTO main.t(rowid,a) VALUES(9007199254740993,x'00FF');
            CREATE TEMP TABLE t(rowid TEXT, a); INSERT INTO temp.t(_rowid_,rowid,a) VALUES(-9,'shadow',2);
            CREATE TABLE \"aux space\".t(rowid TEXT, _rowid_ TEXT, a); INSERT INTO \"aux space\".t(oid,rowid,_rowid_,a) VALUES(42,'shadow','shadow2',3);
            CREATE TABLE wr(k TEXT PRIMARY KEY) WITHOUT ROWID; CREATE VIEW v AS SELECT * FROM main.t;
            CREATE TABLE all_shadowed(rowid, _rowid_, oid, k PRIMARY KEY);
            CREATE TABLE generated_shadow(a, rowid GENERATED ALWAYS AS (a));
            CREATE VIRTUAL TABLE words USING fts5(body); INSERT INTO words(rowid,body) VALUES(17,'hello');").unwrap();
        for (schema, table, alias, value) in [("main","t","rowid",9_007_199_254_740_993), ("temp","t","_rowid_",-9), ("aux space","t","oid",42), ("main","words","rowid",17)] {
            assert_eq!(db::implicit_rowid_column_in(&conn,schema,table).unwrap().as_deref(),Some(alias));
            let (mut result, keys) = db::browse_with_keys_in(&conn,schema,table,&[],None,500,0).unwrap();
            let ordinary = result.columns.clone();
            db::show_rowid_column(&mut result,&keys,alias).unwrap();
            assert_eq!(result.columns[0],alias);
            assert_eq!(result.columns[1..],ordinary);
            assert_eq!(result.rows[0][0],value.to_string());
            assert_eq!(result.values.cell(0,0).unwrap(),db::CellValue::Integer(value));
            let full=db::browse_snapshot_in(&conn,schema,table,&[],None,true).unwrap();
            assert_eq!(full.columns,result.columns);
            assert_eq!(full.values.rectangle(0..=0,0..=full.columns.len()-1).unwrap(),result.values.rectangle(0..=0,0..=result.columns.len()-1).unwrap());
            let invalid=db::RowKey {columns:vec!["not rowid".into()].into(),values:vec![db::CellValue::Integer(value)]};
            assert!(db::show_rowid_column(&mut result,&[invalid],alias).is_err());
        }
        let (_,keys)=db::browse_with_keys_in(&conn,"main","words",&[],None,500,0).unwrap();
        let key=db::update_cell_in(&conn,"main","words",&keys[0],"rowid",&Value::Text("00042".into())).unwrap().unwrap();
        assert_eq!(key.values,[db::CellValue::Integer(42)]);assert_eq!(db::read_keyed_cell_in(&conn,"main","words",&key,"body").unwrap(),db::CellValue::Text(b"hello".to_vec()));
        for table in ["wr","v","all_shadowed"] { assert!(db::implicit_rowid_column_in(&conn,"main",table).unwrap().is_none()); }
        assert_eq!(db::implicit_rowid_column_in(&conn,"main","generated_shadow").unwrap().as_deref(),Some("_rowid_"));
        assert!(db::browse_snapshot_in(&conn,"main","v",&[],None,true).is_err());
        assert!(db::browse_snapshot_in(&conn,"main","t",&[],Some("1; DELETE FROM t"),true).is_err());
        assert_eq!(db::count_rows(&conn,"t").unwrap(),1);
    }

    #[test]
    fn rowid_toggle_retains_named_settings_and_selection_and_guards_mutations() {
        let mut state=fixture();
        state.browse_mut().column_widths=vec![211.0,222.0,233.0,244.0];
        state.browse_mut().col_filters=vec![String::new(),">0".into(),String::new(),String::new()];
        state.browse_mut().frozen_cols=Some(1);
        select(&mut state,0,1);
        state.set_show_rowid(true).unwrap();
        assert_eq!(state.browse().column_names,["rowid","region","reading","note","doubled"]);
        assert_eq!(state.browse().column_widths,[140.0,211.0,222.0,233.0,244.0]);
        assert_eq!(state.browse().col_filters,["","",">0","",""]);
        assert_eq!(state.browse().frozen_cols,Some(2));
        assert_eq!(state.browse().sel_col,Some(2));
        assert_eq!(state.edit_saved,Value::Real(1.25));
        state.browse_mut().col_filters[0]=">=10".into();
        state.set_browse_ordering(vec![db::OrderBy {column:"rowid".into(),descending:true}]).unwrap();
        assert_eq!(state.browse().total_rows,Some(9));
        assert_eq!(state.browse().result.as_ref().unwrap().rows[0][0],"18");
        select(&mut state,0,0);
        let clipboard=state.copy_browse_cells(true,false).unwrap();
        assert!(clipboard.text.contains("18"));
        assert!(state.copy_browse_cells(false,true).unwrap().text.contains("rowid"));
        assert!(state.cut_browse_cells().unwrap_err().contains("cannot be cleared"));
        assert!(!state.dirty);
        state.edit_value="9007199254740993".into();
        assert!(state.set_show_rowid(false).unwrap_err().contains("draft"));
        state.apply_cell_draft().unwrap();
        assert_eq!(state.edit_saved,Value::Integer(9_007_199_254_740_993));
        assert_eq!(state.browse().result.as_ref().unwrap().values.cell(0,0).unwrap(),db::CellValue::Integer(9_007_199_254_740_993));
        assert!(state.dirty);
        state.edit_value="17".into();
        assert!(state.apply_cell_draft().is_err());
        assert_eq!(state.edit_value,"17");
        state.discard_cell_draft();
        state.read_only=true;
        assert!(state.apply_cell_draft().unwrap_err().contains("read-only"));
        state.set_show_rowid(false).unwrap();
        assert_eq!(state.browse().column_names,["region","reading","note","doubled"]);
        assert_eq!(state.browse().frozen_cols,Some(1));
        assert_eq!(state.browse().column_widths,[211.0,222.0,233.0,244.0]);
        assert!(state.browse().sel_row.is_none());
        state.set_show_rowid(true).unwrap();
        state.read_only=false;
        state.browse_mut().col_filters.fill(String::new());
        state.refresh_browse().unwrap();
        select(&mut state,0,0);
        state.paste_browse_tsv("100").unwrap();
        assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT region FROM measurements WHERE rowid=100",[],|row|row.get::<_,String>(0)).unwrap(),"Tokyo");
        select(&mut state,0,4);
        assert!(state.edit_cell_generated());
        assert!(state.apply_cell_draft().unwrap_err().contains("read-only"));
        state.sql_busy=true;
        assert!(state.set_show_rowid(false).is_err());
        state.sql_busy=false;
        state.conn.as_ref().unwrap().execute_batch("CREATE VIEW locked AS SELECT region FROM measurements; CREATE TABLE wr(k PRIMARY KEY) WITHOUT ROWID;").unwrap();
        state.refresh_schema().unwrap();
        for table in ["locked","wr"] {
            state.switch_browse_table(table).unwrap();
            assert!(state.browse().rowid_column.is_none());
            assert!(state.set_show_rowid(true).is_err());
        }
        state.switch_browse_table("measurements").unwrap();
        state.revert_changes().unwrap();
        assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT rowid FROM measurements ORDER BY rowid LIMIT 1",[],|row|row.get::<_,i64>(0)).unwrap(),1);
    }

    #[test]
    fn rowid_rectangle_failure_rolls_back_every_cell_and_retains_prior_pending_changes() {
        let mut state=fixture();state.set_show_rowid(true).unwrap();select(&mut state,0,0);
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE pending(v); INSERT INTO pending VALUES('keep');").unwrap();state.mark_dirty();
        assert!(state.paste_browse_cells(vec![
            vec![db::CellValue::Integer(100),db::CellValue::Text(b"changed".to_vec())],
            vec![db::CellValue::Integer(3),db::CellValue::Text(b"conflict".to_vec())],
        ]).is_err());
        assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT region FROM measurements WHERE rowid=1",[],|row|row.get::<_,String>(0)).unwrap(),"Paris");
        assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT count(*) FROM measurements WHERE rowid=100",[],|row|row.get::<_,i64>(0)).unwrap(),0);
        assert_eq!(db::count_rows(state.conn.as_ref().unwrap(),"pending").unwrap(),1);assert!(state.dirty);assert!(!state.conn.as_ref().unwrap().is_autocommit());
        assert!(state.paste_browse_cells(vec![vec![db::CellValue::Null]]).is_err());
        state.paste_browse_cells(vec![vec![db::CellValue::Integer(100)],vec![db::CellValue::Integer(101)]]).unwrap();
        assert_eq!(state.conn.as_ref().unwrap().query_row("SELECT reading FROM measurements WHERE rowid=101",[],|row|row.get::<_,f64>(0)).unwrap(),2.5);
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE empty(a)").unwrap();state.refresh_schema().unwrap();state.switch_browse_table("empty").unwrap();state.set_show_rowid(true).unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().columns,["rowid","a"]);assert_eq!(state.browse().total_rows,Some(0));assert!(state.browse().row_keys.is_empty());
    }

    #[test]
    fn rowid_settings_follow_implicit_alias_changes_without_becoming_shadow_field_filters() {
        let mut state=fixture();state.set_show_rowid(true).unwrap();
        state.browse_mut().col_filters[0]=">=2".into();state.browse_mut().column_widths=vec![65.0,211.0,222.0,233.0,244.0];state.browse_mut().frozen_cols=Some(2);
        state.set_browse_ordering(vec![db::OrderBy {column:"rowid".into(),descending:true}]).unwrap();
        for (column,alias) in [("rowid","_rowid_"),("_rowid_","oid")] {
            state.conn.as_ref().unwrap().execute_batch(&format!("ALTER TABLE measurements ADD COLUMN {} TEXT",db::quote_ident(column))).unwrap();
            state.refresh_schema().unwrap();state.refresh_browse().unwrap();
            assert!(state.browse().show_row_id);assert_eq!(state.browse().column_names[0],alias);
            assert_eq!(state.browse().col_filters[0],">=2");assert_eq!(state.browse().col_filters.last().unwrap(),"");
            assert_eq!(state.browse().column_widths[0],65.0);assert_eq!(state.browse().order_col.as_deref(),Some(alias));assert_eq!(state.browse().total_rows,Some(17));
        }
        state.conn.as_ref().unwrap().execute_batch("ALTER TABLE measurements ADD COLUMN oid TEXT").unwrap();state.refresh_schema().unwrap();state.refresh_browse().unwrap();
        assert!(!state.browse().show_row_id);assert!(state.browse().rowid_column.is_none());assert!(state.browse().col_filters.iter().all(String::is_empty));
        assert!(state.browse().order_col.is_none());assert_eq!(state.browse().total_rows,Some(18));assert_eq!(state.browse().frozen_cols,Some(1));
    }

    #[test]
    fn rowid_full_plot_export_projects_and_source_settings_keep_column_identity() {
        let mut state=fixture();state.set_show_rowid(true).unwrap();
        state.conn.as_ref().unwrap().execute_batch("WITH RECURSIVE seq(n) AS (VALUES(19) UNION ALL SELECT n+1 FROM seq WHERE n<1205) INSERT INTO measurements(rowid,region,reading,note) SELECT n,'Tokyo',n*1.25,'full'||n FROM seq;").unwrap();
        state.browse_mut().col_filters=vec![">=10".into(),String::new(),String::new(),String::new(),String::new()];
        state.set_browse_ordering(vec![db::OrderBy {column:"rowid".into(),descending:true}]).unwrap();
        assert_eq!(state.browse().result.as_ref().unwrap().rows.len(),500);
        state.plot.settings.manual=false;state.sync_current_plot().unwrap();assert!(state.plot.check(1,1,true));assert!(state.plot.check(3,2,true));state.sync_current_plot().unwrap();
        state.start_plot_load().unwrap();
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);
        while !state.poll_plot_load().unwrap() {assert!(std::time::Instant::now()<deadline);std::thread::sleep(std::time::Duration::from_millis(1));}
        assert!(state.plot.has_full());assert_eq!(state.plot.data.series[0].points.len(),1196);
        assert_eq!(state.plot.data.series[0].points[0].y,1205.0*1.25);
        let dir=tempfile::tempdir().unwrap();let out=dir.path().join("rows.csv");state.export_browse(&out,false).unwrap();
        let csv=std::fs::read_to_string(out).unwrap();assert!(csv.starts_with("region,reading,note,doubled"));assert_eq!(csv.lines().count(),1197);
        let options=state.browse().options();state.switch_browse_table("measurements").unwrap();assert_eq!(state.browse().options(),options);
        state.conn.as_ref().unwrap().execute_batch("CREATE TABLE other(a); INSERT INTO other VALUES(1);").unwrap();state.refresh_schema().unwrap();
        state.switch_browse_table("other").unwrap();assert!(!state.browse().show_row_id);
        state.switch_browse_table("measurements").unwrap();assert!(state.browse().show_row_id);assert_eq!(state.browse().col_filters[0],">=10");
        let project=crate::project::ProjectFile::from_state(&state);
        let project:crate::project::ProjectFile=serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
        assert!(project.browse_tabs[0].show_row_id);
        state.write_changes().unwrap();state.save_as(&dir.path().join("qa.sqlite").to_string_lossy()).unwrap();
        let path=dir.path().join("qa.sqbpro");state.save_project(&path).unwrap();state.open_project(&path).unwrap();assert!(state.browse().show_row_id);
        state.set_show_rowid(false).unwrap();assert_eq!(state.browse().result.as_ref().unwrap().columns.len(),4);
        state.browse_mut().sel_row=Some(2);state.browse_mut().sel_col=Some(1);state.sync_edit_cell_from_selection();
        state.set_show_rowid(true).unwrap();assert_eq!(state.browse().sel_col,Some(2));
        state.browse_mut().col_filters[0]=">=1000".into();state.refresh_browse().unwrap();select(&mut state,0,1);
        let key=state.browse().row_keys[0].clone();state.set_show_rowid(false).unwrap();
        assert_eq!(state.browse().row_keys[state.browse().sel_row.unwrap()],key);
    }

    #[test]
    fn rowid_header_keyboard_toggle_filter_apply_and_rebuild_are_coherent() {
        let folder=std::env::var("NEXT_GUI_ROWID_QA_OUT").ok().map(std::path::PathBuf::from);
        if let Some(folder)=&folder {std::fs::create_dir_all(folder).unwrap();}
        for (name,theme) in [("dark",Theme::dark()),("light",Theme::light())] {
            for (width,height,font) in [(1400,860,13.0),(800,600,18.0)] {
                let mut demo=Demo::new(width,height,PresentBackend::Softbuffer);
                demo.state=fixture();demo.state.prefs.font_size=font;demo.theme=crate::ui_build::browser_theme(theme,font);
                demo.state.dock_focus=DockKind::Plot;demo.state.body_ratio=0.45;demo.state.dock_ratio=0.62;
                select(&mut demo.state,0,1);demo.do_rebuild();
                let capture=|demo:&mut Demo,stage:&str| {if let Some(folder)=&folder {demo.ui.sync(&mut demo.scene);crate::plot_export::capture(demo,folder,&format!("rowid-{stage}-{name}-{width}"));}};
                let header=|demo:&Demo| {
                    let bounds=demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(demo.ids.browse_table)).unwrap().bounds;
                    (bounds.x+24.0,bounds.y+next_gui_ui::TableStyle::from_theme(&demo.theme).header_height/2.0)
                };
                let (x,y)=header(&demo);assert!(demo.open_browse_header_menu(x,y));
                demo.ui.key_input(UiKey::End);
                capture(&mut demo,"menu");
                demo.ui.key_input(UiKey::Enter);
                let (menu,item)=demo.ui.take_menu_action().unwrap();assert_eq!(menu,demo.ids.context_menu);assert_eq!(item,crate::mid::BROWSE_SHOW_ROWID);
                assert!(demo.handle_browse_header_action(item));demo.do_rebuild();
                assert!(demo.state.browse().show_row_id);assert_eq!(demo.ui.table_selected_col(demo.ids.browse_table),Some(2));
                assert!(demo.state.plot.check(1,1,true));assert!(demo.state.plot.check(3,2,true));demo.state.sync_current_plot().unwrap();
                assert_eq!(demo.ids.browse_col_filters.len(),5);
                demo.ui.set_text_input(demo.ids.browse_col_filters[0],">=10");demo.snapshot_inputs();demo.handle_button(demo.ids.btn_browse_go);demo.do_rebuild();
                assert_eq!(demo.state.browse().total_rows,Some(9));
                capture(&mut demo,"filtered");
                select(&mut demo.state,0,0);demo.do_rebuild();
                demo.ui.set_focus(Some(demo.ids.edit_value));demo.ui.key_input(UiKey::SelectAll);demo.ui.paste_into_focused_text("9007199254740993");demo.snapshot_inputs();
                demo.ui.set_focus(Some(demo.ids.btn_edit_cell));demo.ui.key_input(UiKey::Enter);let button=demo.ui.take_button_activate().unwrap();demo.handle_button(button);demo.do_rebuild();
                assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);
                assert_eq!(demo.state.edit_saved,Value::Integer(9_007_199_254_740_993));assert_eq!(demo.ui.focused(),Some(demo.ids.btn_edit_cell));
                capture(&mut demo,"applied");
                let (x,y)=header(&demo);assert!(demo.open_browse_header_menu(x,y));demo.ui.key_input(UiKey::End);demo.ui.key_input(UiKey::Enter);let (_,item)=demo.ui.take_menu_action().unwrap();assert!(demo.handle_browse_header_action(item));demo.do_rebuild();
                assert!(!demo.state.browse().show_row_id);assert_eq!(demo.state.browse().total_rows,Some(9));
                assert_eq!(demo.ids.browse_col_filters.len(),4);assert_eq!(demo.state.browse().result.as_ref().unwrap().columns,["region","reading","note","doubled"]);
                capture(&mut demo,"hidden");
                assert_eq!(demo.state.browse().rowid_filter,">=10");
                demo.handle_button(demo.ids.btn_browse_clear_filters);demo.do_rebuild();
                assert_eq!(demo.state.browse().total_rows,Some(18));assert!(demo.state.browse().rowid_filter.is_empty());
                capture(&mut demo,"cleared");
            }
        }
    }
}
