//! Column presentation never changes the indexed snapshot or SQLite identities.
use crate::{mid, Demo};

impl Demo {
    pub(crate) fn handle_browse_column_action(&mut self, item: u64) -> bool {
        let count = self.state.browse().result.as_ref().map_or(0, |r| r.columns.len());
        if !(mid::BROWSE_HIDE_COLUMN..=mid::BROWSE_RESET_COLUMNS).contains(&item)
            && !(mid::BROWSE_VISIBLE_COLUMN..mid::BROWSE_VISIBLE_COLUMN + count as u64).contains(&item) { return false; }
        self.ui.close_popup();
        let column = self.browse_header_column.take();
        let result = (|| {
            self.state.guard_background_idle()?;
            if self.state.has_cell_draft() || self.state.external_cell.is_some() { return Err("Apply or discard the cell draft first".to_string()); }
            let tab = self.state.browse_mut();
            let names = tab.result.as_ref().ok_or("No browse data")?.columns.clone();
            let mut order = tab.visible_columns();
            match item {
                mid::BROWSE_RESET_COLUMNS => { tab.hidden_columns.clear(); tab.column_order.clear(); }
                mid::BROWSE_COLUMN_LEFT | mid::BROWSE_COLUMN_RIGHT => {
                    let position = column.and_then(|col| order.iter().position(|&i| i == col)).ok_or("Choose a visible column")?;
                    let next = if item == mid::BROWSE_COLUMN_LEFT { position.checked_sub(1) } else { position.checked_add(1).filter(|&i| i < order.len()) };
                    if let Some(next) = next { order.swap(position, next); }
                    // Keep hidden columns' relative positions when they are made visible again.
                    let mut all = tab.column_order.clone();
                    for name in &names { if !all.contains(name) { all.push(name.clone()); } }
                    let mut visible = order.iter();
                    for name in &mut all { if !tab.hidden_columns.contains(name) { *name = names[*visible.next().ok_or("Column layout changed; reopen the menu")?].clone(); } }
                    tab.column_order = all;
                }
                _ => {
                    let col = if item == mid::BROWSE_HIDE_COLUMN { column.ok_or("Choose a column")? } else { (item - mid::BROWSE_VISIBLE_COLUMN) as usize };
                    let name = names.get(col).ok_or("Column no longer exists")?;
                    if let Some(hidden) = tab.hidden_columns.iter().position(|n| n == name) { tab.hidden_columns.remove(hidden); }
                    else if order.len() > 1 { tab.hidden_columns.push(name.clone()); }
                    else { return Err("At least one column must remain visible".into()); }
                }
            }
            let visible = tab.visible_columns();
            if tab.sel_col.is_some_and(|col| !visible.contains(&col)) { tab.sel_col = visible.first().copied(); }
            // Preserve the active cell; reset rectangle anchors after changing its geometry.
            tab.sel_col_start = tab.sel_col; tab.sel_col_end = tab.sel_col;
            if let Some(frozen) = &mut tab.frozen_cols { *frozen = (*frozen).min(visible.len()); }
            self.state.sync_edit_cell_from_selection();
            self.state.status_msg = "Column layout updated".into();
            Ok(())
        })();
        match result { Ok(()) => self.clear_error(), Err(error) => self.set_error(error) }
        self.reveal_browse_selection = true;
        self.rebuild = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::{browse_filter::tests::{capture, fixture}, db, mid, state::AppState, UiKey};
    use next_gui_ui::Theme;

    #[test]
    fn desktop_column_actions_preserve_typed_edits_filters_and_native_project_settings() {
        for (name, width, height, theme) in [("dark",1600,900,Theme::dark()), ("light",1920,927,Theme::light())] {
            let mut demo = fixture(width,height,theme);
            demo.state.conn.as_ref().unwrap().execute_batch("CREATE TABLE columns_test(id INTEGER PRIMARY KEY,hidden TEXT,a TEXT,b BLOB);INSERT INTO columns_test VALUES(1,'keep','alpha',X'00FF'),(2,'keep2','beta',X'01');").unwrap();
            demo.state.refresh_schema().unwrap(); demo.state.switch_browse_table("columns_test").unwrap();
            demo.state.write_changes().unwrap(); demo.do_rebuild();
            let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("columns.sqlite");
            demo.state.save_as(path.to_str().unwrap()).unwrap();
            let action = |demo: &mut crate::Demo, col, item| {
                demo.browse_header_column = Some(col); assert!(demo.handle_browse_column_action(item));
                assert!(demo.state.error_msg.is_none(), "{:?}",demo.state.error_msg); demo.do_rebuild();
            };
            action(&mut demo,1,mid::BROWSE_HIDE_COLUMN);
            action(&mut demo,3,mid::BROWSE_COLUMN_LEFT);
            assert_eq!(demo.state.browse().visible_columns(), [0,3,2]);
            assert_eq!(demo.ui.table_visible_columns(demo.ids.browse_table).unwrap(), [0,3,2]);
            demo.state.browse_mut().frozen_cols = Some(2);
            let select = |demo: &mut crate::Demo| {
                demo.ui.set_table_selected_cells(demo.ids.browse_table,Some((0,1)),Some((3,2)));
                demo.ui.set_focus(Some(demo.ids.browse_table)); demo.poll_selection_changes(); demo.do_rebuild();
            };
            select(&mut demo);
            let copied = demo.state.copy_browse_cells(true,false).unwrap();
            assert!(copied.text.starts_with("b\ta\n"), "{}", copied.text);
            demo.state.paste_browse_cells(vec![vec![db::CellValue::Blob(vec![7,0,255]),db::CellValue::Text(b"reordered".to_vec())]]).unwrap();
            demo.state.write_changes().unwrap();
            let reader = db::open_readonly(path.to_str().unwrap()).unwrap();
            assert_eq!(reader.query_row("SELECT hidden,a,hex(b) FROM columns_test WHERE id=1",[],|r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).unwrap(),("keep".into(),"reordered".into(),"0700FF".into()));
            demo.do_rebuild(); select(&mut demo);
            demo.ui.key_input(UiKey::Left); demo.poll_selection_changes(); assert_eq!(demo.state.browse().sel_col,Some(3));
            demo.do_rebuild(); capture(&mut demo,&format!("desktop-columns-{name}"));
            demo.state.switch_browse_table("other").unwrap(); demo.state.switch_browse_table("columns_test").unwrap();
            assert_eq!(demo.state.browse().visible_columns(),[0,3,2]);
            for extension in ["json","sqbpro"] {
                let project = dir.path().join(format!("columns.{extension}")); demo.state.save_project(&project).unwrap();
                let mut reopened = AppState::empty(); reopened.open_project(&project).unwrap();
                assert_eq!(reopened.browse().visible_columns(),[0,3,2]);
                assert_eq!(reopened.browse().hidden_columns,["hidden"]);
                assert_eq!(reopened.browse_frozen_cols(),2);
                if extension == "sqbpro" {
                    let text = std::fs::read_to_string(&project).unwrap();
                    let start = text.find("<next_gui").unwrap(); let end = text.find("</next_gui>").unwrap()+"</next_gui>".len();
                    let mut native = text; native.replace_range(start..end, "");
                    std::fs::write(&project,native).unwrap();
                    let mut reopened = AppState::empty(); reopened.open_project(&project).unwrap();
                    assert_eq!(reopened.browse().visible_columns(),[0,3,2]);
                    assert_eq!(reopened.browse_frozen_cols(),2);
                }
            }
            action(&mut demo,2,mid::BROWSE_RESET_COLUMNS);
            assert_eq!(demo.state.browse().visible_columns(),[0,1,2,3]);
        }
    }
}
