//! SQL document chrome keeps content, files, editor history and results together.
use crate::{state::{AppState, DocumentAction, MainTab}, Demo};
use next_gui_ui::{MenuItem, ModalStyle, TabAction, TextInputStyle, Theme, Ui, WidgetId};

const MENU: u64 = 200_000;

#[derive(Clone, Copy)]
pub struct Controls {
    pub dialog: WidgetId,
    pub name: WidgetId,
    pub apply: WidgetId,
    pub cancel: WidgetId,
}

impl Default for Controls {
    fn default() -> Self { Self {dialog:WidgetId(0),name:WidgetId(0),apply:WidgetId(0),cancel:WidgetId(0)} }
}

pub fn build(ui: &mut Ui, state: &AppState, theme: &Theme) -> Controls {
    let Some((_,name)) = &state.sql_rename else { return Controls::default(); };
    let mut controls = Controls::default();
    let mut style = ModalStyle::from_theme(theme); style.width=480.0; style.height=180.0;
    let mut input = TextInputStyle::from_theme(theme); input.width=440.0;
    controls.dialog = ui.dialog("Rename SQL Tab",style,|body| {
        body.text("Tab name"); controls.name=body.text_input_styled(name,"Tab name",input);
    },|footer| {
        controls.apply=footer.tool_button_styled("Rename",theme.colors.accent,theme.colors.on_accent);
        controls.cancel=footer.tool_button("Cancel");
    });
    ui.set_accessible_name(controls.name,"SQL tab name");
    ui.set_dialog_open(controls.dialog,true); ui.set_focus(Some(controls.name));
    controls
}

impl Demo {
    pub(crate) fn handle_sql_rename_key(&mut self,key:crate::KeyCode) -> bool {
        if self.state.sql_rename.is_none() || key!=crate::KeyCode::Enter || self.ui.ime_is_composing() {return false;}
        let controls=self.ids.sql_documents;
        self.handle_sql_rename_button(if self.ui.focused()==Some(controls.cancel) {controls.cancel} else {controls.apply});true
    }
    pub(crate) fn snapshot_sql_rename(&mut self) {
        if let Some((_,name))=&mut self.state.sql_rename {
            if let Some(value)=self.ui.text_input_value(self.ids.sql_documents.name) { *name=value.into(); }
        }
    }

    pub(crate) fn open_sql_document_menu(&mut self, x:f32,y:f32) -> bool {
        if self.state.tab!=MainTab::ExecuteSql || self.state.overlay.is_some() || self.state.confirm_open || self.ui.active_dialog().is_some() {return false;}
        let Some(index)=self.ui.tab_at(&self.scene,self.ids.sql_tabs,x,y) else {return false;};
        let enabled=!self.state.sql_busy;
        let items=["Rename Tab","Duplicate Tab","Close Tab"].into_iter().enumerate()
            .map(|(action,label)| {let mut item=MenuItem::new(MENU+index as u64*3+action as u64,label);item.enabled=enabled;item}).collect();
        self.ui.open_context_menu(self.ids.context_menu,items,x,y)
    }

    pub(crate) fn handle_sql_document_menu(&mut self,item:u64) -> bool {
        if !(MENU..MENU+self.state.sql_tabs.len() as u64*3).contains(&item) {return false;}
        if self.state.sql_busy {return true;}
        let index=((item-MENU)/3) as usize;
        self.snapshot_inputs();
        match (item-MENU)%3 {
            0=>self.open_sql_rename(index),
            1=>{self.state.duplicate_sql_tab(index);self.rebuild=true;},
            _=>self.request_document_action(DocumentAction::CloseSqlTab(index)),
        }
        true
    }

    fn open_sql_rename(&mut self,index:usize) {
        if !self.state.sql_busy && index<self.state.sql_tabs.len() {
            self.state.sql_rename=Some((index,self.state.sql_tab_title(index)));self.rebuild=true;
        }
    }

    pub(crate) fn poll_sql_documents(&mut self) -> bool {
        if self.state.tab!=MainTab::ExecuteSql || self.state.sql_rename.is_some() {return false;}
        if let Some((tabs,action))=self.ui.take_tab_action() {
            if tabs==self.ids.sql_tabs && !self.state.sql_busy {
                self.snapshot_inputs();
                match action {
                    TabAction::Move {from,to} => self.state.move_sql_tab(from,to),
                    TabAction::Close(index) => self.request_document_action(DocumentAction::CloseSqlTab(index)),
                    TabAction::Rename(index) => self.open_sql_rename(index),
                }
                self.rebuild=true;return true;
            }
        }
        if let Some(index)=self.ui.tabs_selected(self.ids.sql_tabs) {
            if index!=self.state.sql_tab_idx {
                self.snapshot_inputs();self.state.switch_sql_tab(index);self.rebuild=true;return true;
            }
        }
        false
    }

    pub(crate) fn handle_sql_rename_button(&mut self,id:WidgetId) {
        if id==self.ids.sql_documents.apply {
            self.snapshot_sql_rename();
            if let Some((index,name))=self.state.sql_rename.take() {
                if index<self.state.sql_tabs.len() && !self.state.sql_busy {
                    self.state.sql_tab_titles.resize(self.state.sql_tabs.len(),None);
                    self.state.sql_tab_titles[index]=Some(name);
                }
            }
        } else if id==self.ids.sql_documents.cancel {self.state.sql_rename=None;}
        else {return;}
        self.close_sql_rename();
    }

    pub(crate) fn handle_sql_rename_dismiss(&mut self) {
        if self.ui.take_dialog_dismiss()==Some(self.ids.sql_documents.dialog) {self.close_sql_rename();}
    }

    fn close_sql_rename(&mut self) {
        self.state.sql_rename=None;self.do_rebuild();self.ui.set_focus(Some(self.ids.sql_tabs));self.ui.sync(&mut self.scene);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{browse_filter::tests::{capture,fixture}, UiKey};
    use next_gui_ui::ModalAction;

    fn tab_edges(demo:&Demo,index:usize)->(f32,f32,f32) {
        let b=demo.scene.iter_paint_order().into_iter().find(|n|demo.ui.widget_from_node(n.id)==Some(demo.ids.sql_tabs)).unwrap().bounds;
        let y=b.y+15.0;
        let points:Vec<_>=(0..b.width as i32).map(|x|b.x+x as f32).filter(|x|demo.ui.tab_at(&demo.scene,demo.ids.sql_tabs,*x,y)==Some(index)).collect();
        (*points.first().unwrap(),*points.last().unwrap(),y)
    }

    #[test]
    fn desktop_sql_document_drag_rename_duplicate_close_and_projects_keep_sources() {
        for (name,width,height,theme) in [("dark",1600,900,Theme::dark()),("light",1920,927,Theme::light())] {
            let mut demo=fixture(width,height,theme);demo.state.write_changes().unwrap();demo.state.tab=MainTab::ExecuteSql;
            let dir=tempfile::tempdir().unwrap();let db=dir.path().join("tabs.sqlite");demo.state.save_as(db.to_str().unwrap()).unwrap();
            demo.state.sql_tabs=vec!["SELECT name,amount FROM names".into()];demo.state.run_sql_sync().unwrap();demo.do_rebuild();
            demo.ui.set_focus(Some(demo.ids.sql_area));demo.ui.select_text_area_range(demo.ids.sql_area,0..0);
            demo.ui.replace_text_area_selection(demo.ids.sql_area,"-- first draft\n");demo.snapshot_inputs();
            demo.state.sql_tab_titles=vec![Some("First report".into())];
            let file=dir.path().join("file.sql");std::fs::write(&file,"SELECT 22 AS bound_file").unwrap();demo.state.open_sql_file(&file).unwrap();
            demo.state.run_sql_sync().unwrap();demo.do_rebuild();demo.action_new_sql_tab();demo.do_rebuild();demo.ui.set_text_area_value(demo.ids.sql_area,"SELECT 33 AS third");demo.snapshot_inputs();demo.state.run_sql_sync().unwrap();demo.do_rebuild();
            let (start,_,y)=tab_edges(&demo,0);let (_,end,_)=tab_edges(&demo,2);
            demo.ui.pointer_pressed(&mut demo.scene,start+10.0,y);demo.snapshot_inputs();demo.poll_selection_changes();
            assert!(demo.ui.pointer_moved(end-3.0,y));demo.ui.sync(&mut demo.scene);
            capture(&mut demo,&format!("desktop-sql-tabs-drag-{name}"));
            demo.ui.pointer_released();demo.snapshot_inputs();demo.poll_selection_changes();demo.do_rebuild();
            assert_eq!(demo.state.sql_tab_idx,1);assert_eq!(demo.state.sql_files[0].as_ref().unwrap().path,file);
            assert_eq!(demo.state.sql_tab_title(2),"First report");assert!(demo.state.sql_tabs[2].starts_with("-- first draft"));
            assert_eq!(demo.state.sql_result.as_ref().unwrap().rows[0][0],"33");
            let (start,_,y)=tab_edges(&demo,2);
            demo.ui.pointer_pressed(&mut demo.scene,start+10.0,y);demo.ui.pointer_released();demo.snapshot_inputs();demo.poll_selection_changes();demo.do_rebuild();
            assert_eq!(demo.state.sql_tab_idx,2);assert_eq!(demo.state.sql_result.as_ref().unwrap().rows.len(),5);
            demo.ui.set_focus(Some(demo.ids.sql_area));assert!(demo.ui.key_input(UiKey::Undo));demo.snapshot_inputs();
            assert_eq!(demo.state.sql_editor(),"SELECT name,amount FROM names");
            let (start,_,y)=tab_edges(&demo,2);
            demo.ui.pointer_pressed(&mut demo.scene,start+10.0,y);demo.ui.pointer_released();demo.snapshot_inputs();demo.poll_selection_changes();demo.do_rebuild();
            assert!(demo.state.sql_rename.is_some(),"double click survives the rebuild");
            demo.ui.set_focus(Some(demo.ids.sql_documents.name));demo.ui.key_input(UiKey::SelectAll);demo.ui.paste_into_focused_text("发货 & Report <三>");
            demo.do_rebuild();capture(&mut demo,&format!("desktop-sql-tabs-rename-{name}"));
            assert_eq!(demo.ui.text_input_value(demo.ids.sql_documents.name),Some("发货 & Report <三>"));
            demo.handle_button(demo.ids.sql_documents.apply);demo.do_rebuild();assert_eq!(demo.state.sql_tab_title(2),"发货 & Report <三>");
            demo.open_sql_rename(2);demo.do_rebuild();demo.ui.paste_into_focused_text(" canceled");demo.ui.key_input(UiKey::Escape);demo.handle_sql_rename_dismiss();demo.do_rebuild();
            assert_eq!(demo.state.sql_tab_title(2),"发货 & Report <三>");assert_eq!(demo.ui.focused(),Some(demo.ids.sql_tabs));
            demo.open_sql_rename(0);demo.do_rebuild();demo.ui.key_input(UiKey::SelectAll);demo.ui.paste_into_focused_text("Linked SQL file");
            assert!(demo.handle_sql_rename_key(crate::KeyCode::Enter));demo.do_rebuild();assert_eq!(demo.state.sql_tab_title(0),"Linked SQL file");
            assert!(demo.handle_sql_document_menu(MENU+2*3+1));demo.do_rebuild();
            assert_eq!(demo.state.sql_tab_title(3),"发货  Report <三> (1)");assert_eq!(demo.state.sql_tabs[3],demo.state.sql_tabs[2]);
            assert!(demo.state.sql_files.get(3).is_none_or(Option::is_none));assert!(demo.state.sql_result.is_none());
            let (_,end,y)=tab_edges(&demo,3);demo.ui.pointer_pressed(&mut demo.scene,end-10.0,y);demo.snapshot_inputs();demo.poll_selection_changes();demo.do_rebuild();
            assert!(demo.state.confirm_open);demo.resolve_document_action(ModalAction::Tertiary);demo.do_rebuild();assert_eq!(demo.state.sql_tabs.len(),4);
            assert!(demo.handle_sql_document_menu(MENU+3*3+2));demo.resolve_document_action(ModalAction::Secondary);demo.do_rebuild();assert_eq!(demo.state.sql_tabs.len(),3);
            assert_eq!(std::fs::read_to_string(&file).unwrap(),"SELECT 22 AS bound_file");
            capture(&mut demo,&format!("desktop-sql-tabs-final-{name}"));
            for extension in ["json","sqbpro"] {
                let project=dir.path().join(format!("tabs.{extension}"));demo.state.save_project(&project).unwrap();
                let mut reopened=AppState::empty();reopened.open_project(&project).unwrap();
                assert_eq!(reopened.sql_tabs,demo.state.sql_tabs);assert_eq!(reopened.sql_tab_title(2),"发货 & Report <三>");
                assert_eq!(reopened.sql_files[0].as_ref().unwrap().path,file);
                if extension=="sqbpro" {
                    let mut text=std::fs::read_to_string(&project).unwrap();let a=text.find("<next_gui").unwrap();let b=text.find("</next_gui>").unwrap()+"</next_gui>".len();text.replace_range(a..b,"");std::fs::write(&project,text).unwrap();
                    let mut reopened=AppState::empty();reopened.open_project(&project).unwrap();assert_eq!(reopened.sql_tab_title(2),"发货 & Report <三>");
                }
            }
        }
    }
}
