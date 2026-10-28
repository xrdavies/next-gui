use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct TableCellEditor {
    pub row: usize,
    pub column: usize,
    pub widget: WidgetId,
}

/// An editable table cell was activated or its standard editor was finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableCellEditEvent {
    /// Build/focus an editor for this cell; apply `key` after selecting its text.
    Begin {
        /// Zero-based body row.
        row: usize,
        /// Zero-based column.
        column: usize,
        /// Initial character/deletion key, or None for Enter/F2/pointer/Tab.
        key: Option<UiKey>,
    },
    /// Commit on Enter or restore the value from before editing on Escape.
    End {
        /// True when Escape requested cancellation.
        cancel: bool,
    },
}

impl Ui {
    /// Opt into cell editing. Selected-click, Enter/F2 or typing queues a Begin event.
    /// Columns are sorted/deduplicated for Tab/BackTab navigation. Check cells toggle instead.
    pub fn set_table_editable_columns(&mut self, id: WidgetId, mut editable: Vec<usize>) -> bool {
        let Some(Widget {kind:Kind::Table {columns,editable_columns,..},..})=self.widgets.get_mut(&id) else {return false;};
        if editable.iter().any(|&column|column>=columns.len()) {return false;}
        editable.sort_unstable();editable.dedup();*editable_columns=editable;true
    }

    /// The standard input currently mounted inside this table, if any.
    pub fn table_cell_editor(&self, id: WidgetId) -> Option<WidgetId> {
        match &self.widgets.get(&id)?.kind {Kind::Table {editor:Some(editor),..}=>Some(editor.widget),_=>None}
    }

    /// Whether a widget belongs to the active cell editor, including a composite row's controls.
    pub fn table_cell_editor_contains(&self, table: WidgetId, widget: WidgetId) -> bool {
        self.table_cell_editor(table).is_some_and(|editor| self.widget_contains(editor, widget))
    }

    /// Drain a cell edit request. Snapshot the previous editor before changing rows/rebuilding.
    pub fn take_table_cell_edit(&mut self) -> Option<(WidgetId,TableCellEditEvent)> {self.pending_table_cell_edit.take()}

    pub(super) fn begin_table_cell_edit(&mut self,id:WidgetId,row:usize,column:usize,key:Option<UiKey>)->bool {
        let Some(Widget {kind:Kind::Table {rows,editable_columns,checks,..},..})=self.widgets.get(&id) else {return false;};
        if row>=rows.len() || !editable_columns.contains(&column) || checks.get(row).and_then(|row|row.get(column)).copied().flatten().is_some() {return false;}
        self.set_table_selected_row(id,Some(row));self.set_table_selected_col(id,Some(column));self.reveal_table_selection(id);
        self.pending_table_cell_edit=Some((id,TableCellEditEvent::Begin {row,column,key}));true
    }

    pub(super) fn key_table_cell_editor(&mut self,key:UiKey)->Option<bool> {
        let focused=self.focus?;
        let (table,editor)=self.widgets.iter().find_map(|(&table,w)|match w.kind {
            Kind::Table {editor:Some(editor),..} if self.widget_contains(editor.widget,focused)=>Some((table,editor)),_=>None})?;
        if matches!(self.widgets.get(&focused).map(|w|&w.kind),Some(Kind::TextInput {preedit,..}|Kind::Autocomplete {preedit,..}|Kind::TextArea {preedit,..}) if !preedit.is_empty()) {return None;}
        if key==UiKey::Enter && self.is_button(focused) {return None;}
        let cancel=match key {
            UiKey::Enter if self.open_popup.is_none()=>false,
            UiKey::Escape=>true,
            UiKey::Tab|UiKey::BackTab=> {
                let mut controls=Vec::new();self.collect_focusable_dfs(editor.widget,&mut controls);
                let position=controls.iter().position(|&control|control==focused)?;
                let next=if key==UiKey::Tab {position.checked_add(1)} else {position.checked_sub(1)};
                if let Some(control)=next.and_then(|index|controls.get(index)).copied() {
                    self.close_popup_internal();self.focus=Some(control);return Some(true);
                }
                let Some(Widget {kind:Kind::Table {editable_columns,rows,checks,..},..})=self.widgets.get(&table) else {return None;};
                let position=editable_columns.iter().position(|&column|column==editor.column)?;
                let index=editor.row*editable_columns.len()+position;
                let mut next=if key==UiKey::Tab {index.checked_add(1)} else {index.checked_sub(1)};
                let target=loop {
                    let Some(index)=next.filter(|&index|index<rows.len()*editable_columns.len()) else {break None;};
                    let (row,column)=(index/editable_columns.len(),editable_columns[index%editable_columns.len()]);
                    if checks.get(row).and_then(|row|row.get(column)).copied().flatten().is_none() {break Some((row,column));}
                    next=if key==UiKey::Tab {index.checked_add(1)} else {index.checked_sub(1)};
                };
                self.close_popup_internal();self.focus=Some(table);
                if let Some((row,column))=target {
                    if self.begin_table_cell_edit(table,row,column,None) {return Some(true);}
                }
                false
            }
            _=>return None,
        };
        self.focus=Some(table);self.pending_table_cell_edit=Some((table,TableCellEditEvent::End {cancel}));Some(true)
    }

    pub(super) fn paint_table_editor(&mut self,scene:&mut Scene,id:WidgetId) {
        let Some(Widget {kind:Kind::Table {editor:Some(editor),columns,rows,scroll_x,scroll_y,frozen_cols,style,..},node:Some(host),..})=self.widgets.get(&id) else {return;};
        let editor=*editor;let host=*host;let bounds=scene.get(host).map(|node|node.bounds).unwrap_or_else(Rect::empty);
        let Some(column)=columns.get(editor.column).filter(|_|editor.row<rows.len()) else {self.hide_widget_subtree(scene,editor.widget);return;};
        let (x,clip)=table_column_rects(bounds,columns,style.padding,*scroll_x,*frozen_cols)[editor.column];
        let y=bounds.y+style.header_height+editor.row as f32*style.row_height-*scroll_y;
        let rect=Rect::new(x+1.0,y+1.0,(column.width-2.0).max(1.0),(style.row_height-2.0).max(1.0));
        let clip=clip.intersect(&Rect::new(bounds.x,bounds.y+style.header_height,bounds.width,(bounds.height-style.header_height).max(0.0))).intersect(&rect);
        let Some(node)=self.widgets.get(&editor.widget).and_then(|w|w.node) else {return;};
        if clip.is_empty() {self.hide_widget_subtree(scene,editor.widget);return;}
        self.place_child(scene,editor.widget,rect);scene.set_clip(node,Some(clip));
        self.layout_widget(scene,editor.widget);
        self.paint_widget(scene,editor.widget);scene.set_clip(node,Some(clip));scene.raise(node);
    }
}

impl FlexBuilder<'_> {
    /// Mount a TextInput, Autocomplete, Select or composite Row inside a cell, outside flex layout.
    /// Rebuild without this call to end the editor. Invalid coordinates skip the builder.
    pub fn table_cell_editor<F>(&mut self,table:WidgetId,row:usize,column:usize,build:F)->Option<WidgetId>
    where F:FnOnce(&mut FlexBuilder<'_>)->WidgetId {
        if !matches!(self.ui.widgets.get(&table).map(|w|&w.kind),Some(Kind::Table {rows,columns,editor:None,..}) if row<rows.len() && column<columns.len()) {return None;}
        let widget=build(&mut FlexBuilder {ui:self.ui,parent:table});
        if !matches!(self.ui.widgets.get(&widget).map(|w|&w.kind),Some(Kind::TextInput {..}|Kind::Autocomplete {..}|Kind::Select {..}|Kind::Row {..})) {return None;}
        if let Some(Widget {kind:Kind::Table {editor,..},..})=self.ui.widgets.get_mut(&table) {*editor=Some(TableCellEditor {row,column,widget});}
        Some(widget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_input_blocks_focus_pointer_clipboard_ime_and_preserves_undo() {
        for theme in [Theme::dark(),Theme::light()] {
            let mut ui=Ui::new();ui.set_theme(theme);let mut scene=Scene::new();let mut input=WidgetId(0);let mut button=WidgetId(0);
            ui.row(RowStyle {width:400.0,..Default::default()},|r| {input=r.text_input_styled("before","",TextInputStyle::from_theme(&theme));button=r.tool_button("Next");});
            ui.sync(&mut scene);ui.set_focus(Some(input));ui.key_input(UiKey::SelectAll);ui.paste_into_focused_text("after");ui.ime_preedit("候選",Some((0,3)));
            assert!(ui.set_text_input_enabled(input,false));assert!(!ui.set_text_input_enabled(input,false));
            assert_eq!(ui.text_input_enabled(input),Some(false));assert_eq!(ui.text_input_enabled(button),None);
            ui.set_focus(Some(input));assert_eq!(ui.focused(),None);assert!(!ui.focusable_widgets().contains(&input));
            assert!(!ui.paste_into_focused_text("bad"));assert!(!ui.ime_commit("bad"));assert!(!ui.ime_preedit("bad",None));
            assert!(!ui.key_text_input(input,UiKey::Char('x')));assert!(!ui.key_text_input(input,UiKey::Undo));assert_eq!(ui.text_input_value(input),Some("after"));
            ui.sync(&mut scene);let node=scene.get(ui.widgets[&input].node.unwrap()).unwrap();let bounds=node.bounds;
            assert_eq!(node.fill,theme.colors.muted);assert_eq!(scene.get(ui.widgets[&input].label_node.unwrap()).unwrap().fill,theme.colors.disabled);
            assert!(ui.pointer_pressed(&mut scene,bounds.x+5.0,bounds.y+5.0));ui.pointer_released();assert_eq!(ui.focused(),None);
            ui.key_input(UiKey::Tab);assert_eq!(ui.focused(),Some(button));
            assert!(ui.set_text_input_enabled(input,true));ui.set_focus(Some(input));ui.key_input(UiKey::Undo);assert_eq!(ui.text_input_value(input),Some("before"));
            ui.key_input(UiKey::Redo);assert_eq!(ui.text_input_value(input),Some("after"));ui.sync(&mut scene);assert_eq!(scene.get(ui.widgets[&input].node.unwrap()).unwrap().fill,theme.colors.input_bg);
            let dialog=ui.dialog("Options",ModalStyle::default(),|c|{c.tool_button("Done");},|_|{});
            assert!(ui.set_dialog_open(dialog,true));ui.set_text_input_enabled(input,false);assert!(ui.set_dialog_open(dialog,false));
            assert_eq!(ui.focused(),None);assert!(!ui.text_input_wants_ime());assert!(!ui.ime_commit("late commit"));assert_eq!(ui.text_input_value(input),Some("after"));
        }
    }

    #[test]
    fn composite_cell_rows_layout_clip_and_navigate_controls_before_neighboring_cells() {
        for theme in [Theme::dark(),Theme::light()] {
            let mut ui=Ui::new();ui.set_theme(theme);ui.set_viewport(700.0,400.0);
            let mut scene=Scene::new();let mut table=WidgetId(0);let mut parent=WidgetId(0);let mut target=WidgetId(0);let mut clause=WidgetId(0);let mut reset=WidgetId(0);let mut editor=WidgetId(0);
            ui.column(ColumnStyle {width:640.0,..Default::default()},|c| {
                table=c.table(vec![TableColumn::new("Name",80.0),TableColumn::new("References",360.0),TableColumn::new("SQL",300.0)],(0..12).map(|index|vec![format!("row{index}"),"parent(id)".into(),"SQL".into()]).collect(),TableStyle {width:620.0,height:160.0,..TableStyle::from_theme(&theme)});
                editor=c.table_cell_editor(table,0,1,|cell|cell.row(RowStyle {width:358.0,height:24.0,padding:0.0,gap:0.0,background:None,..Default::default()},|r| {
                    parent=r.select(["parent","other"],Some(0),"",SelectStyle {width:80.0,..SelectStyle::from_theme(&theme)});
                    target=r.select(["id","key"],Some(0),"",SelectStyle {width:70.0,..SelectStyle::from_theme(&theme)});
                    clause=r.text_input_styled("ON UPDATE CASCADE","",TextInputStyle {width:120.0,..TextInputStyle::from_theme(&theme)});
                    reset=r.tool_button("Reset");
                })).unwrap();
            });
            ui.set_table_editable_columns(table,vec![0,1]);ui.set_table_frozen_cols(table,1);ui.sync(&mut scene);
            assert_eq!(ui.table_cell_editor(table),Some(editor));
            for child in [parent,target,clause,reset] {assert!(ui.table_cell_editor_contains(table,child));}
            assert!(!ui.table_cell_editor_contains(table,table));
            let root=scene.get(ui.widgets[&editor].node.unwrap()).unwrap().clone();
            let first=scene.get(ui.widgets[&parent].node.unwrap()).unwrap().bounds;
            let last=scene.get(ui.widgets[&reset].node.unwrap()).unwrap().bounds;
            assert_eq!(first.x,root.bounds.x);assert!(last.x>first.x && last.x+last.width<=root.bounds.x+root.bounds.width);
            ui.set_focus(Some(parent));assert!(ui.key_input(UiKey::Space));ui.sync(&mut scene);
            assert!(ui.has_open_popup());assert!(ui.key_input(UiKey::Escape));assert_eq!(ui.focused(),Some(parent));assert_eq!(ui.take_table_cell_edit(),None);
            for child in [target,clause,reset] {assert!(ui.key_input(UiKey::Tab));assert_eq!(ui.focused(),Some(child));assert_eq!(ui.take_table_cell_edit(),None);}
            ui.set_text_input_enabled(clause,false);ui.set_focus(Some(target));ui.key_input(UiKey::Tab);assert_eq!(ui.focused(),Some(reset));ui.set_text_input_enabled(clause,true);
            assert!(ui.key_input(UiKey::Enter));assert_eq!(ui.take_button_activate(),Some(reset));assert_eq!(ui.take_table_cell_edit(),None);
            ui.key_input(UiKey::BackTab);assert_eq!(ui.focused(),Some(clause));ui.key_input(UiKey::SelectAll);ui.ime_preedit("候選",Some((0,3)));
            assert!(ui.key_input(UiKey::Escape));assert_eq!(ui.take_table_cell_edit(),None);assert_eq!(ui.focused(),Some(clause));
            assert!(ui.key_input(UiKey::Escape));assert_eq!(ui.take_table_cell_edit(),Some((table,TableCellEditEvent::End {cancel:true})));
            ui.set_focus(Some(parent));ui.key_input(UiKey::BackTab);assert_eq!(ui.take_table_cell_edit(),Some((table,TableCellEditEvent::Begin {row:0,column:0,key:None})));
            ui.set_focus(Some(reset));ui.key_input(UiKey::Tab);assert_eq!(ui.take_table_cell_edit(),Some((table,TableCellEditEvent::Begin {row:1,column:0,key:None})));
            ui.set_focus(Some(clause));ui.key_input(UiKey::Enter);assert_eq!(ui.take_table_cell_edit(),Some((table,TableCellEditEvent::End {cancel:false})));
            ui.set_table_scroll_offset(table,100.0,0.0);ui.sync(&mut scene);
            let root=scene.get(ui.widgets[&editor].node.unwrap()).unwrap();let clip=root.clip.unwrap();
            assert!(clip.x>root.bounds.x && clip.x>=first.x-1.0);
            assert_eq!(ui.hit_widget(&scene,clip.x-3.0,first.y+5.0),Some(table));
            ui.set_table_scroll_offset(table,0.0,1000.0);ui.sync(&mut scene);assert!(ui.widgets[&editor].node.is_some_and(|node|!scene.get(node).unwrap().visible));
            ui.set_table_scroll_offset(table,0.0,0.0);ui.sync(&mut scene);let reset_bounds=scene.get(ui.widgets[&reset].node.unwrap()).unwrap().bounds;
            assert_eq!(ui.hit_widget(&scene,reset_bounds.x+reset_bounds.width/2.0,reset_bounds.y+reset_bounds.height/2.0),Some(reset));
        }
    }

    #[test]
    fn cell_editor_is_a_clipped_table_child_with_history_ime_and_tab_requests() {
        let mut ui=Ui::new();let mut scene=Scene::new();let mut table=WidgetId(0);let mut editor=WidgetId(0);
        ui.column(ColumnStyle {width:320.0,height:200.0,..Default::default()},|c| {
            table=c.table(vec![TableColumn::new("Frozen",100.0),TableColumn::new("Text",150.0),TableColumn::new("Edited",180.0)],
                (0..5).map(|_|vec!["first".into(),"middle".into(),"value".into()]).collect(),TableStyle {width:300.0,height:150.0,..Default::default()});
            assert_eq!(c.table_cell_editor(table,99,2,|_|panic!("invalid row builder")),None);
            editor=c.table_cell_editor(table,1,2,|cell|cell.text_input_styled("value","",TextInputStyle::default())).unwrap();
        });
        assert!(ui.set_table_editable_columns(table,vec![2,0,1,2]));
        assert!(ui.set_table_check_cells(table,vec![vec![None,Some(false)];5]));assert!(!ui.set_table_editable_columns(table,vec![3]));
        ui.set_table_frozen_cols(table,1);ui.set_table_selected_row(table,Some(1));ui.set_table_selected_col(table,Some(2));ui.reveal_table_selection(table);ui.set_focus(Some(editor));ui.sync(&mut scene);
        assert!(ui.focusable_widgets().contains(&editor));let table_node=ui.widgets[&table].node.unwrap();let editor_node=ui.widgets[&editor].node.unwrap();assert_eq!(scene.get(editor_node).unwrap().parent,Some(table_node));
        let bounds=scene.get(table_node).unwrap().bounds;let input=scene.get(editor_node).unwrap();assert!(input.visible);assert!(input.bounds.x>=bounds.x+100.0);assert!(input.clip.unwrap().x+input.clip.unwrap().width<=bounds.x+bounds.width);
        assert_eq!(ui.hit_widget(&scene,input.bounds.x+5.0,input.bounds.y+5.0),Some(editor));
        ui.key_input(UiKey::SelectAll);ui.paste_into_focused_text("edited");assert_eq!(ui.text_input_value(editor),Some("edited"));ui.key_input(UiKey::Undo);assert_eq!(ui.text_input_value(editor),Some("value"));ui.key_input(UiKey::Redo);
        assert!(ui.ime_preedit("候選",Some((0,2))));assert!(ui.key_input(UiKey::Escape));assert_eq!(ui.take_table_cell_edit(),None);assert_eq!(ui.text_input_value(editor),Some("edited"));
        assert!(ui.key_input(UiKey::Tab));assert_eq!(ui.take_table_cell_edit(),Some((table,TableCellEditEvent::Begin {row:2,column:0,key:None})));assert_eq!(ui.focused(),Some(table));
        ui.set_focus(Some(editor));assert!(ui.key_input(UiKey::BackTab));assert_eq!(ui.take_table_cell_edit(),Some((table,TableCellEditEvent::Begin {row:1,column:0,key:None})));
        ui.set_focus(Some(editor));assert!(ui.key_input(UiKey::Enter));assert_eq!(ui.take_table_cell_edit(),Some((table,TableCellEditEvent::End {cancel:false})));
        ui.set_focus(Some(editor));assert!(ui.key_input(UiKey::Escape));assert_eq!(ui.take_table_cell_edit(),Some((table,TableCellEditEvent::End {cancel:true})));
        ui.set_focus(Some(table));ui.set_table_selected_row(table,Some(0));ui.set_table_selected_col(table,Some(0));assert!(ui.key_input(UiKey::Edit));assert_eq!(ui.take_table_cell_edit(),Some((table,TableCellEditEvent::Begin {row:0,column:0,key:None})));
        assert!(ui.key_input(UiKey::Char('x')));assert_eq!(ui.take_table_cell_edit(),Some((table,TableCellEditEvent::Begin {row:0,column:0,key:Some(UiKey::Char('x'))})));
        ui.set_table_selected_col(table,Some(1));assert!(!ui.key_input(UiKey::Edit));
        ui.set_focus(Some(editor));ui.set_table_rows(table,Vec::new());ui.sync(&mut scene);assert_eq!(ui.focused(),None);assert!(!ui.focusable_widgets().contains(&editor));assert!(!scene.get(editor_node).unwrap().visible);
    }

    #[test]
    fn select_cell_keeps_popup_focus_and_routes_commit_cancel_and_tab_before_grid_navigation() {
        let mut ui=Ui::new();let mut scene=Scene::new();let mut table=WidgetId(0);let mut editor=WidgetId(0);
        ui.column(ColumnStyle {width:400.0,height:220.0,..Default::default()},|c| {
            table=c.table(vec![TableColumn::new("Type",160.0),TableColumn::new("Name",150.0)],vec![vec!["UNIQUE".into(),"key_name".into()]],TableStyle {width:340.0,height:130.0,..Default::default()});
            editor=c.table_cell_editor(table,0,0,|cell|cell.select(["PRIMARY KEY","UNIQUE"],Some(1),"",SelectStyle::default())).unwrap();
        });
        ui.set_table_editable_columns(table,vec![0,1]);ui.set_focus(Some(editor));ui.sync(&mut scene);
        assert!(ui.key_input(UiKey::Space));assert!(ui.has_open_popup());assert!(ui.key_input(UiKey::Up));assert_eq!(ui.select_selected(editor),Some(0));
        assert!(ui.key_input(UiKey::Escape));assert!(!ui.has_open_popup());assert_eq!(ui.focused(),Some(editor));assert_eq!(ui.take_table_cell_edit(),None);
        assert!(ui.key_input(UiKey::Escape));assert_eq!(ui.take_table_cell_edit(),Some((table,TableCellEditEvent::End {cancel:true})));
        ui.set_focus(Some(editor));assert!(ui.key_input(UiKey::Space));assert!(ui.key_input(UiKey::Down));assert_eq!(ui.select_selected(editor),Some(1));assert!(ui.key_input(UiKey::Enter));assert_eq!(ui.take_table_cell_edit(),None);assert_eq!(ui.focused(),Some(editor));
        assert!(ui.key_input(UiKey::Enter));assert_eq!(ui.take_table_cell_edit(),Some((table,TableCellEditEvent::End {cancel:false})));
        ui.set_focus(Some(editor));ui.key_input(UiKey::Space);assert!(ui.key_input(UiKey::Tab));assert!(!ui.has_open_popup());assert_eq!(ui.take_table_cell_edit(),Some((table,TableCellEditEvent::Begin {row:0,column:1,key:None})));
        ui.set_focus(Some(editor));ui.set_table_scroll_offset(table,120.0,0.0);ui.sync(&mut scene);let node=scene.get(ui.widgets[&editor].node.unwrap()).unwrap();let host=scene.get(ui.widgets[&table].node.unwrap()).unwrap();let clip=node.clip.unwrap();assert!(clip.x>=host.bounds.x && clip.x+clip.width<=host.bounds.x+host.bounds.width);
        let (x,y)=(node.bounds.x+10.0,node.bounds.y+10.0);assert!(ui.pointer_pressed(&mut scene,x,y));ui.pointer_released();assert!(ui.has_open_popup());ui.key_input(UiKey::Escape);assert_eq!(ui.focused(),Some(editor));
    }

    #[test]
    fn autocomplete_cell_accepts_suggestions_before_ending_and_hides_with_its_tab() {
        let mut ui=Ui::new();let mut scene=Scene::new();let mut table=WidgetId(0);let mut editor=WidgetId(0);let mut tabs=WidgetId(0);
        ui.column(ColumnStyle {width:400.0,height:220.0,..Default::default()},|c| {
            tabs=c.tabs(TabsStyle {width:380.0,height:200.0,..Default::default()},|pages| {
                pages.page("Table",|c| {
                    table=c.table(vec![TableColumn::new("Type",180.0)],vec![vec!["INTEGER".into()]],TableStyle {width:340.0,height:150.0,..Default::default()});
                    editor=c.table_cell_editor(table,0,0,|cell|cell.autocomplete_placeholder(["INTEGER","TEXT"],"INTEGER","",AutocompleteStyle::default())).unwrap();
                });pages.page("Other",|c|{c.text("Other");});
            });
        });
        ui.set_table_editable_columns(table,vec![0]);ui.set_focus(Some(editor));ui.sync(&mut scene);ui.key_input(UiKey::SelectAll);ui.paste_into_focused_text("EX");
        assert!(ui.has_open_popup());assert!(ui.key_input(UiKey::Enter));assert_eq!(ui.autocomplete_value(editor),Some("TEXT"));assert_eq!(ui.take_table_cell_edit(),None);assert!(ui.key_input(UiKey::Enter));assert_eq!(ui.take_table_cell_edit(),Some((table,TableCellEditEvent::End {cancel:false})));
        ui.set_focus(Some(editor));ui.key_input(UiKey::SelectAll);ui.paste_into_focused_text("IN");assert!(ui.key_input(UiKey::Escape));assert_eq!(ui.take_table_cell_edit(),None);assert!(ui.key_input(UiKey::Escape));assert_eq!(ui.take_table_cell_edit(),Some((table,TableCellEditEvent::End {cancel:true})));
        ui.set_tabs_selected(tabs,1);ui.sync(&mut scene);let w=&ui.widgets[&editor];assert!(!scene.get(w.node.unwrap()).unwrap().visible);assert!(w.aux.iter().all(|node|!scene.get(*node).unwrap().visible));
        ui.set_tabs_selected(tabs,0);ui.set_focus(Some(editor));ui.sync(&mut scene);ui.key_input(UiKey::SelectAll);ui.paste_into_focused_text("IN");assert!(ui.has_open_popup());ui.set_table_rows(table,Vec::new());assert!(!ui.has_open_popup());assert_eq!(ui.focused(),None);
    }
}
