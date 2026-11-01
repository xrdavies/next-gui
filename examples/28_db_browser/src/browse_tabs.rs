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
}
