use super::*;

fn expanded(nodes: &[TreeTableNode], id: u64) -> Option<bool> {
    nodes.iter().find_map(|node| if node.id == id { Some(node.expanded) } else { expanded(&node.children, id) })
}
fn set_expanded(nodes: &mut [TreeTableNode], id: u64, value: bool) -> bool {
    for node in nodes {
        if node.id == id { node.expanded = value; return true; }
        if set_expanded(&mut node.children, id, value) { return true; }
    }
    false
}
fn ancestors(nodes: &mut [TreeTableNode], id: u64) -> bool {
    for node in nodes {
        if node.id == id { return true; }
        if ancestors(&mut node.children, id) { node.expanded = true; return true; }
    }
    false
}
impl Ui {
    pub(crate) fn tree_table_expanded(&self, id: WidgetId, node: u64) -> Option<bool> {
        let Kind::TreeTable { roots, .. } = &self.widgets.get(&id)?.kind else { return None; };
        expanded(roots, node)
    }
    pub(crate) fn set_tree_table_expanded(&mut self, id: WidgetId, node: u64, value: bool) {
        if let Some(Widget { kind: Kind::TreeTable { roots, .. }, .. }) = self.widgets.get_mut(&id) { set_expanded(roots, node, value); }
    }
    pub(crate) fn tree_table_node_at(&self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> Option<u64> {
        let widget = self.widgets.get(&id)?;
        let Kind::TreeTable { roots, style, scroll_y, .. } = &widget.kind else { return None; };
        if !style.row_height.is_finite() || style.row_height <= 0.0 { return None; }
        let bounds = scene.get(widget.node?)?.bounds;
        let body = Rect::new(bounds.x, bounds.y + style.header_height, bounds.width, (bounds.height-style.header_height).max(0.0));
        if !body.contains(x,y) { return None; }
        flatten_tree_table(roots).get(((y-body.y+scroll_y)/style.row_height).floor() as usize).map(|node| node.id)
    }
    pub(crate) fn reveal_tree_table_selection(&mut self, id: WidgetId) {
        if let Some(Widget { kind: Kind::TreeTable { roots, selected: Some(selected), .. }, .. }) = self.widgets.get_mut(&id) { ancestors(roots,*selected); }
        let Some(Widget { kind: Kind::TreeTable { roots, selected: Some(selected), style, scroll_y, .. }, .. }) = self.widgets.get(&id) else { return; };
        let Some(row) = flatten_tree_table(roots).iter().position(|row| row.id == *selected) else { return; };
        let top=row as f32*style.row_height;let bottom=top+style.row_height;let height=(style.height-style.header_height).max(0.0);
        let next=if top<*scroll_y {top} else if bottom>*scroll_y+height {bottom-height} else {*scroll_y};
        self.set_any_scroll(id,0.0,next);
    }
    pub(crate) fn key_tree_table_input(&mut self, id: WidgetId, key: UiKey) -> bool {
        let Some(Widget { kind: Kind::TreeTable { roots, selected, style, .. }, .. }) = self.widgets.get(&id) else { return false; };
        let rows=flatten_tree_table(roots);if rows.is_empty() {return false;}
        let index=selected.and_then(|id|rows.iter().position(|row|row.id==id));let current=index.unwrap_or(0);
        let page=((style.height-style.header_height)/style.row_height.max(1.0)).floor().max(1.0) as usize;
        let next=match key {
            UiKey::Down=>Some(index.map_or(0,|i|(i+1).min(rows.len()-1))),
            UiKey::Up=>Some(current.saturating_sub(1)),UiKey::Home=>Some(0),UiKey::End=>Some(rows.len()-1),
            UiKey::PageDown=>Some(current.saturating_add(page).min(rows.len()-1)),UiKey::PageUp=>Some(current.saturating_sub(page)),
            UiKey::Left=>{
                if index.is_some() && rows[current].has_children && rows[current].expanded {self.set_tree_table_expanded(id,rows[current].id,false);return true;}
                (0..current).rev().find(|&i|rows[i].depth<rows[current].depth)
            }
            UiKey::Right|UiKey::Enter=>{
                if index.is_some() && rows[current].has_children {
                    if !rows[current].expanded {self.set_tree_table_expanded(id,rows[current].id,true);return true;}
                    Some((current+1).min(rows.len()-1))
                } else {None}
            }
            _=>None,
        };
        if let Some(next)=next {self.set_tree_selected(id,Some(rows[next].id));self.reveal_tree_table_selection(id);true} else {false}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tree_table_navigation_hit_testing_and_paint_stay_in_the_viewport() {
        let mut ui=Ui::new();let mut scene=Scene::new();let mut table=WidgetId(0);
        let fields=(0..2000).map(|i|TreeTableNode::leaf(100+i,format!("field {i}"),vec![if i==0 {"very long definition ".repeat(10000)} else {format!("TEXT DEFAULT {i}")}])).collect();
        let style=TreeTableStyle {width:600.0,height:240.0,..Default::default()};
        ui.column(ColumnStyle {width:620.0,height:300.0,..Default::default()},|c|table=c.tree_table(vec![TableColumn::new("Schema",400.0)],vec![TreeTableNode::new(1,"table",Vec::new(),fields)],style));
        ui.set_focus(Some(table));ui.sync(&mut scene);
        assert!(ui.widgets[&table].aux.len()<80);
        let bounds=scene.get(ui.widgets[&table].node.unwrap()).unwrap().bounds;
        let edge=bounds.x+style.tree_col_width;
        ui.pointer_pressed(&mut scene,edge,bounds.y+5.0);ui.pointer_moved(edge+70.0,bounds.y+5.0);ui.pointer_released();
        assert_eq!(ui.table_column_widths(table).unwrap()[0],style.tree_col_width+70.0);
        assert!(!ui.set_table_column_width(table,1,f32::NAN));assert!(ui.set_table_column_width(table,1,450.0));
        assert_eq!(ui.tree_node_at(&scene,table,bounds.x+50.0,bounds.y+5.0),None);
        assert_eq!(ui.tree_node_at(&scene,table,bounds.x+50.0,bounds.y+style.header_height+5.0),Some(1));
        ui.key_input(UiKey::End);ui.sync(&mut scene);assert_eq!(ui.tree_selected(table),Some(2099));assert!(ui.tree_scroll_offset(table).unwrap()>40000.0);assert!(ui.widgets[&table].aux.len()<80);
        ui.key_input(UiKey::PageUp);assert!(ui.tree_selected(table).unwrap()<2099);
        ui.key_input(UiKey::Left);assert_eq!(ui.tree_selected(table),Some(1));
        ui.key_input(UiKey::Left);assert_eq!(ui.tree_expanded(table,1),Some(false));
        ui.set_tree_selected(table,Some(1100));ui.reveal_tree_selection(table);assert_eq!(ui.tree_expanded(table,1),Some(true));
        ui.sync(&mut scene);assert!(ui.widgets[&table].aux.len()<80);
        ui.key_input(UiKey::Home);ui.key_input(UiKey::Right);assert_eq!(ui.tree_selected(table),Some(100));ui.sync(&mut scene);
        let body=Rect::new(bounds.x,bounds.y+style.header_height,bounds.width,bounds.height-style.header_height);
        for &node in ui.widgets[&table].aux.iter().skip(2) {
            let node=scene.get(node).unwrap();if node.visible&&node.mask.is_some() {assert!(node.clip.is_some_and(|clip|clip.y>=body.y&&clip.y+clip.height<=body.y+body.height));}
        }
    }
}
