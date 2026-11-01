//! Browse tab chrome keeps each cached source, selection and layout together.
use crate::{Demo,state::MainTab};
use next_gui_ui::TabAction;

impl Demo {
    pub(crate) fn poll_browse_tabs(&mut self) -> bool {
        if self.state.tab!=MainTab::Browse || self.state.overlay.is_some() || self.state.confirm_open {return false;}
        if let Some((tabs,action))=self.ui.take_tab_action() {
            if tabs==self.ids.browse_tabs {
                self.snapshot_inputs();
                let result=match action {
                    TabAction::Move {from,to}=>Some(self.state.move_browse_tab(from,to)),
                    TabAction::Close(index)=>Some(self.state.close_browse_tab_at(index)),
                    TabAction::Rename(_)=>None,
                };
                if let Some(result)=result {
                    match result {Ok(())=>self.clear_error(),Err(error)=>self.set_error(error)}
                    self.rebuild=true;return true;
                }
            }
        }
        if let Some(index)=self.ui.tabs_selected(self.ids.browse_tabs) {
            if index!=self.state.browse_tab_idx {
                self.snapshot_inputs();
                match self.state.switch_browse_tab(index) {Ok(())=>self.clear_error(),Err(error)=>self.set_error(error)}
                self.rebuild=true;return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{browse_filter::tests::{capture,fixture},state::AppState};
    use next_gui_ui::Theme;

    fn header(demo:&Demo,index:usize)->(f32,f32,f32) {
        let b=demo.scene.iter_paint_order().into_iter().find(|n|demo.ui.widget_from_node(n.id)==Some(demo.ids.browse_tabs)).unwrap().bounds;
        let y=b.y+12.0;
        let points:Vec<_>=(0..b.width as i32).map(|x|b.x+x as f32).filter(|x|demo.ui.tab_at(&demo.scene,demo.ids.browse_tabs,*x,y)==Some(index)).collect();
        (*points.first().unwrap(),*points.last().unwrap(),y)
    }

    fn close(demo:&mut Demo,index:usize) {
        let (_,end,y)=header(demo,index);demo.ui.pointer_pressed(&mut demo.scene,end-10.0,y);
        demo.ui.pointer_released();demo.snapshot_inputs();demo.poll_selection_changes();demo.do_rebuild();
    }

    #[test]
    fn desktop_browse_tabs_drag_close_and_projects_keep_sources_selections_and_drafts() {
        for (name,width,height,theme) in [("dark",1600,900,Theme::dark()),("light",1920,927,Theme::light())] {
            let mut demo=fixture(width,height,theme);demo.state.write_changes().unwrap();
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE deliveries(id INTEGER PRIMARY KEY,note);INSERT INTO deliveries VALUES(7,'kept');CREATE TABLE empty_source(v);").unwrap();demo.state.refresh_schema().unwrap();demo.state.write_changes().unwrap();
            let dir=tempfile::tempdir().unwrap();let db=dir.path().join("tabs.sqlite");demo.state.save_as(db.to_str().unwrap()).unwrap();
            demo.do_rebuild();demo.ui.set_table_selected_cells(demo.ids.browse_table,Some((0,4)),Some((0,1)));demo.snapshot_inputs();demo.poll_selection_changes();
            let selected=(demo.state.browse().sel_row,demo.state.browse().sel_row_start,demo.state.browse().sel_row_end,demo.state.browse().sel_col_start,demo.state.browse().sel_col_end);
            demo.state.add_browse_tab().unwrap();demo.state.switch_browse_table("deliveries").unwrap();demo.state.add_browse_tab().unwrap();demo.state.switch_browse_table("empty_source").unwrap();demo.state.switch_browse_tab(0).unwrap();demo.do_rebuild();
            let (x,_,y)=header(&demo,0);let (_,end,_)=header(&demo,2);
            demo.ui.pointer_pressed(&mut demo.scene,x+10.0,y);demo.ui.pointer_moved(end-5.0,y);demo.ui.sync(&mut demo.scene);
            capture(&mut demo,&format!("desktop-browse-tabs-drag-{name}"));
            demo.ui.pointer_released();demo.snapshot_inputs();demo.poll_selection_changes();demo.do_rebuild();
            assert_eq!(demo.state.browse_tab_idx,2);assert_eq!(demo.state.browse_tabs.iter().map(|t|t.table.as_deref().unwrap()).collect::<Vec<_>>(),["deliveries","empty_source","names"]);
            let current=demo.state.browse();assert_eq!((current.sel_row,current.sel_row_start,current.sel_row_end,current.sel_col_start,current.sel_col_end),selected);
            for extension in ["json","sqbpro"] {
                let file=dir.path().join(format!("tabs.{extension}"));demo.state.save_project(&file).unwrap();let mut reopened=AppState::empty();reopened.open_project(&file).unwrap();
                assert_eq!(reopened.browse_tab_idx,2);assert_eq!(reopened.browse_tabs.iter().map(|t|t.table.as_deref().unwrap()).collect::<Vec<_>>(),["deliveries","empty_source","names"]);
            }
            close(&mut demo,0);assert_eq!(demo.state.browse_tab_idx,1);assert_eq!(demo.state.browse().table.as_deref(),Some("names"));assert_eq!(demo.state.browse().sel_row,selected.0);
            demo.state.start_sql_text("SELECT 1".into()).unwrap();demo.state.tab=MainTab::Browse;demo.do_rebuild();assert!(demo.ids.browse_tabs.0!=0);assert!(!demo.ui.focusable_widgets().contains(&demo.ids.browse_tabs));
            assert!(demo.state.move_browse_tab(0,1).is_err());assert!(demo.state.close_browse_tab_at(0).is_err());close(&mut demo,0);assert_eq!(demo.state.browse_tabs.len(),2);
            let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);while demo.state.sql_busy {assert!(std::time::Instant::now()<deadline);demo.state.poll_sql_job();std::thread::sleep(std::time::Duration::from_millis(1));}demo.state.tab=MainTab::Browse;demo.do_rebuild();
            demo.ui.set_table_selected_cells(demo.ids.browse_table,Some((0,0)),Some((0,0)));demo.poll_selection_changes();demo.do_rebuild();
            demo.ui.set_text_area_value(demo.ids.edit_value,"pending tab draft");demo.snapshot_inputs();assert!(demo.state.has_cell_draft(),"column={} type={} value={:?} saved={:?} UI={:?}",demo.state.edit_col,demo.state.edit_type,demo.state.edit_value,demo.state.edit_saved,demo.ui.text_area_value(demo.ids.edit_value));
            assert!(demo.state.move_browse_tab(0,1).is_err());close(&mut demo,0);assert_eq!(demo.state.browse_tabs.len(),2);assert_eq!(demo.state.edit_value,"pending tab draft");
            demo.state.discard_cell_draft();demo.clear_error();demo.do_rebuild();close(&mut demo,0);assert_eq!(demo.state.browse_tabs.len(),1);
            capture(&mut demo,&format!("desktop-browse-tabs-final-{name}"));
            close(&mut demo,0);assert_eq!(demo.state.browse_tabs.len(),1);assert!(demo.state.browse().table.is_none());assert!(demo.state.edit_value.is_empty());
        }
    }

    fn finish(state:&mut AppState)->Result<(),String> {
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(20);
        while state.browse_work.is_some() {
            assert!(std::time::Instant::now()<deadline,"Browse tab worker did not finish");
            state.poll_browse_filter()?;std::thread::sleep(std::time::Duration::from_millis(1));
        }
        Ok(())
    }

    #[test]
    fn browse_tab_workers_publish_switch_add_and_close_after_loading_and_preserve_pending_edits() {
        let mut state=AppState::empty();crate::desktop_workflow::prepare(&mut state,"desktop-browse").unwrap();
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("orders.sqlite");state.save_as(path.to_str().unwrap()).unwrap();
        state.add_browse_tab().unwrap();state.switch_browse_table_in("main","customers").unwrap();state.goto_browse_record(300).unwrap();
        state.add_browse_tab().unwrap();state.switch_browse_table_in("main","products").unwrap();state.goto_browse_record(70).unwrap();
        state.switch_browse_tab(0).unwrap();state.goto_browse_record(100000).unwrap();
        state.browse_mut().sel_col=Some(9);state.browse_mut().sel_row_start=Some(99998);state.browse_mut().sel_row_end=Some(99999);
        state.browse_mut().sel_col_start=Some(1);state.browse_mut().sel_col_end=Some(9);state.browse_mut().scroll=(57.0,2190.0);state.sync_edit_cell_from_selection();
        state.conn.as_ref().unwrap().execute("UPDATE orders SET note='pending tab worker' WHERE id=17",[]).unwrap();state.dirty=true;
        let identity=state.browse().result.as_ref().unwrap().values.identity();let editor=state.edit_value.clone();state.browse_background=true;
        state.switch_browse_tab(1).unwrap();assert_eq!(state.browse_tab_idx,0);assert_eq!(state.browse_tabs.len(),3);assert!(state.conn.is_none());
        assert!(std::sync::Arc::ptr_eq(&identity,&state.browse().result.as_ref().unwrap().values.identity()));assert_eq!(state.edit_value,editor);
        assert!(state.move_browse_tab(0,2).is_err());assert!(state.add_browse_tab().is_err());finish(&mut state).unwrap();
        assert_eq!(state.browse_tab_idx,1);assert_eq!(state.browse().table.as_deref(),Some("customers"));assert_eq!(state.browse().sel_row,Some(299));
        state.switch_browse_tab(0).unwrap();finish(&mut state).unwrap();let tab=state.browse();
        assert_eq!((tab.sel_row,tab.sel_row_start,tab.sel_row_end,tab.sel_col,tab.sel_col_start,tab.sel_col_end),(Some(99999),Some(99998),Some(99999),Some(9),Some(1),Some(9)));
        assert_eq!(tab.scroll,(57.0,2190.0));assert_eq!(tab.total_rows,Some(100000));
        state.add_browse_tab().unwrap();assert_eq!(state.browse_tabs.len(),3);finish(&mut state).unwrap();assert_eq!(state.browse_tab_idx,3);assert_eq!(state.browse().total_rows,Some(100000));
        state.close_browse_tab_at(3).unwrap();assert_eq!(state.browse_tabs.len(),4);finish(&mut state).unwrap();assert_eq!(state.browse_tabs.len(),3);assert_eq!(state.browse_tab_idx,2);assert_eq!(state.browse().sel_row,Some(69));
        state.switch_browse_tab(1).unwrap();finish(&mut state).unwrap();state.close_browse_tab_at(1).unwrap();finish(&mut state).unwrap();assert_eq!(state.browse_tab_idx,1);assert_eq!(state.browse().table.as_deref(),Some("products"));
        state.close_browse_tab_at(1).unwrap();finish(&mut state).unwrap();assert_eq!(state.browse_tab_idx,0);assert_eq!(state.browse().table.as_deref(),Some("orders"));assert_eq!(state.browse().sel_row,Some(99999));
        state.close_browse_tab_at(0).unwrap();assert_eq!(state.browse_tabs.len(),1);assert!(state.browse().table.is_none());assert!(state.edit_value.is_empty());
        assert!(!state.conn.as_ref().unwrap().is_autocommit());assert!(state.dirty);state.write_changes().unwrap();let reader=crate::db::open_readonly(path.to_str().unwrap()).unwrap();
        assert_eq!(reader.query_row("SELECT note FROM orders WHERE id=17",[],|r|r.get::<_,String>(0)).unwrap(),"pending tab worker");
        assert_eq!(reader.query_row("SELECT count(*) FROM orders",[],|r|r.get::<_,usize>(0)).unwrap(),100000);
    }

    #[test]
    fn browse_tab_worker_failure_and_cancellation_keep_tabs_cached_source_editor_and_transaction() {
        let mut demo=fixture(1600,900,Theme::dark());demo.state.add_browse_tab().unwrap();demo.state.switch_browse_table("other").unwrap();demo.state.switch_browse_tab(0).unwrap();
        demo.state.goto_browse_record(3).unwrap();demo.state.browse_mut().sel_col=Some(0);demo.state.sync_edit_cell_from_selection();
        let old=demo.state.browse().result.as_ref().unwrap().values.identity();let editor=demo.state.edit_value.clone();demo.state.browse_tabs[1].filter="missing_column=1".into();demo.state.browse_background=true;
        for close in [false,true] {
            if close {demo.state.close_browse_tab_at(0).unwrap();} else {demo.state.switch_browse_tab(1).unwrap();}
            assert!(demo.state.conn.is_none());assert_eq!(demo.state.browse_tabs.len(),2);assert_eq!(demo.state.browse_tab_idx,0);
            assert!(finish(&mut demo.state).unwrap_err().contains("missing_column"));
            assert_eq!(demo.state.browse_tabs.len(),2);assert_eq!(demo.state.browse_tab_idx,0);assert_eq!(demo.state.edit_value,editor);
            assert!(std::sync::Arc::ptr_eq(&old,&demo.state.browse().result.as_ref().unwrap().values.identity()));assert!(!demo.state.conn.as_ref().unwrap().is_autocommit());
        }
        demo.state.browse_tabs[1].filter="v > (WITH RECURSIVE n(x) AS(VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<100000000) SELECT max(x) FROM n)".into();
        for operation in 0..3 {
            let started=std::time::Instant::now();match operation {0=>demo.state.switch_browse_tab(1),1=>demo.state.close_browse_tab_at(0),_=>demo.state.add_browse_tab()}.unwrap();
            assert!(started.elapsed()<std::time::Duration::from_secs(2));demo.do_rebuild();assert!(!demo.ui.focusable_widgets().contains(&demo.ids.browse_tabs));assert_eq!(demo.ui.button_enabled(demo.ids.btn_browse_new_tab),Some(false));
            demo.handle_button(demo.ids.btn_browse_cancel);demo.do_rebuild();assert!(demo.state.browse_work.is_none());assert_eq!(demo.state.browse_tabs.len(),2);assert_eq!(demo.state.browse_tab_idx,0);
            assert_eq!(demo.state.edit_value,editor);assert!(std::sync::Arc::ptr_eq(&old,&demo.state.browse().result.as_ref().unwrap().values.identity()));
            assert!(!demo.state.conn.as_ref().unwrap().is_autocommit());assert_eq!(crate::db::count_rows(demo.state.conn.as_ref().unwrap(),"names").unwrap(),5);
        }
    }

    #[test]
    fn desktop_browse_tab_workers_load_from_empty_source_and_keep_cancel_and_results_visible() {
        for (name,width,height,theme) in [("dark",1600,900,Theme::dark()),("light",1920,927,Theme::light())] {
            let mut demo=fixture(width,height,theme);demo.state.add_browse_tab().unwrap();demo.state.switch_browse_table("other").unwrap();
            demo.state.browse_tabs[0]=crate::state::BrowseTabState::new(None);demo.state.browse_tab_idx=0;demo.state.clear_browse_editor();demo.state.browse_background=true;demo.do_rebuild();
            demo.ui.set_tabs_selected(demo.ids.browse_tabs,1);demo.poll_selection_changes();demo.do_rebuild();assert!(demo.state.browse_work.is_some());assert_eq!(demo.state.browse_tab_idx,0);
            assert_eq!(demo.ui.button_enabled(demo.ids.btn_browse_new_tab),Some(false));assert!(demo.ids.btn_browse_cancel.0!=0);
            let cancel=demo.scene.iter_paint_order().into_iter().find(|node|demo.ui.widget_from_node(node.id)==Some(demo.ids.btn_browse_cancel)).unwrap().bounds;
            assert_eq!(demo.ui.hit_widget(&demo.scene,cancel.x+cancel.width*0.5,cancel.y+cancel.height*0.5),Some(demo.ids.btn_browse_cancel));
            capture(&mut demo,&format!("desktop-browse-tab-worker-loading-{name}"));
            let deadline=std::time::Instant::now()+std::time::Duration::from_secs(10);
            while demo.state.browse_work.is_some() {assert!(std::time::Instant::now()<deadline);demo.advance_browse_filter(std::time::Instant::now());std::thread::sleep(std::time::Duration::from_millis(1));}
            demo.do_rebuild();assert!(demo.state.error_msg.is_none(),"{:?}",demo.state.error_msg);assert_eq!(demo.state.browse_tab_idx,1);assert_eq!(demo.state.browse().result.as_ref().unwrap().rows[0][0],"42");
            assert_eq!(demo.advance_browse_filter(std::time::Instant::now()),None);assert!(demo.state.browse_work.is_none());
            capture(&mut demo,&format!("desktop-browse-tab-worker-completed-{name}"));
            demo.state.switch_browse_tab(0).unwrap();assert!(demo.state.browse().table.is_none());demo.state.add_browse_tab().unwrap();assert!(demo.state.browse_work.is_some());finish(&mut demo.state).unwrap();assert_eq!(demo.state.browse_tab_idx,2);assert!(demo.state.browse().result.is_some());
        }
    }

}
