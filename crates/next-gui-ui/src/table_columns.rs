use super::*;

pub(super) fn visible_table_columns(columns: &[TableColumn]) -> Vec<usize> {
    let mut order: Vec<_> = (0..columns.len()).filter(|&i| columns[i].visible).collect();
    order.sort_by_key(|&i| columns[i].display_order);
    order
}

pub(super) fn selected_table_columns(columns: &[TableColumn], range: Option<(usize, usize)>) -> Vec<usize> {
    let order = visible_table_columns(columns);
    let Some((a, b)) = range else { return order; };
    let Some(a) = order.iter().position(|&i| i == a) else { return Vec::new(); };
    let Some(b) = order.iter().position(|&i| i == b) else { return Vec::new(); };
    order[a.min(b)..=a.max(b)].to_vec()
}

impl Ui {
    /// A completed header drag, with source-column indices in the new visible order.
    pub fn take_table_column_order(&mut self) -> Option<(WidgetId, Vec<usize>)> {
        self.pending_table_column_order.take()
    }

    pub(super) fn move_table_header(&mut self, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() { return false; }
        let Some(DragState::TableHeader { table, column, bounds, origin, order, moved, .. }) = self.drag.clone() else { return false; };
        if !moved && (x-origin.0).abs().max((y-origin.1).abs()) < 5.0 { return false; }
        let Some(Kind::Table { columns, style, scroll_x, frozen_cols, .. }) = self.widgets.get(&table).map(|w| &w.kind) else { return false; };
        let mut target = order.clone();
        let mut marker = None;
        let mut scroll = None;
        let label_height = self.table_header_label_height(table).unwrap_or(style.header_height);
        if y >= bounds.y-24.0 && y <= bounds.y+label_height+24.0 {
            let cells = table_column_rects(bounds, columns, style.padding, *scroll_x, *frozen_cols);
            let px = x.clamp(bounds.x+style.padding, bounds.x+bounds.width-style.padding);
            let position = order.iter().position(|&i| px < cells[i].0+columns[i].width*0.5).unwrap_or(order.len());
            marker = if let Some(&i) = order.get(position) { Some(cells[i].0) }
                else { order.last().map(|&i| cells[i].0+columns[i].width) };
            let old = order.iter().position(|&i| i == column).unwrap();
            target.remove(old);
            target.insert(position- usize::from(old < position), column);
            let frozen_width: f32 = order.iter().take(*frozen_cols).map(|&i| columns[i].width).sum();
            let left = bounds.x+style.padding+frozen_width;
            if x > bounds.x+bounds.width-12.0 { scroll = Some(*scroll_x+20.0); }
            else if x < left+12.0 && column != *order.first().unwrap() { scroll = Some((*scroll_x-20.0).max(0.0)); }
        }
        let scrolling = scroll.is_some_and(|sx| {let sy=self.table_scroll_offset(table).unwrap().1;self.set_any_scroll(table,sx,sy)});
        if let Some(DragState::TableHeader { target: next, marker: mark, moved, next_repeat, .. }) = &mut self.drag {
            *next = target; *mark = marker; *moved = true;
            *next_repeat=scrolling.then(||std::time::Instant::now()+std::time::Duration::from_millis(33));
        }
        true
    }

    /// Visible columns in display order. Indices always identify the original data columns.
    pub fn table_visible_columns(&self, table: WidgetId) -> Option<Vec<usize>> {
        let Kind::Table { columns, .. } = &self.widgets.get(&table)?.kind else { return None; };
        Some(visible_table_columns(columns))
    }

    /// Set visible columns and their order without changing data, styles or column identities.
    /// Reject duplicates, invalid indices and hiding every column. Omitted columns stay hidden.
    pub fn set_table_visible_columns(&mut self, table: WidgetId, order: &[usize]) -> bool {
        let Some(Widget { kind: Kind::Table { columns, selected_col, selected_col_range, editor, .. }, .. }) = self.widgets.get_mut(&table) else { return false; };
        if order.is_empty() && !columns.is_empty() { return false; }
        let mut seen = vec![false; columns.len()];
        for &i in order {
            if i >= columns.len() || seen[i] { return false; }
            seen[i] = true;
        }
        if visible_table_columns(columns) == order { return false; }
        // A live editor must be committed or canceled before its column can disappear.
        if editor.is_some_and(|editor| !seen[editor.column]) { return false; }
        for (i, column) in columns.iter_mut().enumerate() { column.visible = seen[i]; }
        for (position, &i) in order.iter().enumerate() { columns[i].display_order = position; }
        if selected_col.is_some_and(|i| !seen.get(i).copied().unwrap_or(false)) { *selected_col = order.first().copied(); }
        if selected_col_range.is_some_and(|(a, b)| !seen.get(a).copied().unwrap_or(false) || !seen.get(b).copied().unwrap_or(false)) {
            *selected_col_range = selected_col.map(|i| (i, i));
        }
        let (x, y) = self.table_scroll_offset(table).unwrap();
        self.set_any_scroll(table, x, y);
        true
    }

    /// Selected source-column indices in display order, excluding hidden columns.
    pub fn table_selected_column_indices(&self, table: WidgetId) -> Option<Vec<usize>> {
        let Kind::Table { columns, selected_col, selected_col_range, .. } = &self.widgets.get(&table)?.kind else { return None; };
        Some(selected_table_columns(columns, selected_col_range.or_else(|| selected_col.map(|i| (i, i)))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_header_drag_scrolls_wide_tables_and_release_stops_repetition() {
        let mut ui=Ui::new();let mut scene=Scene::new();let mut table=WidgetId(0);
        ui.column(ColumnStyle {width:300.0,height:200.0,padding:0.0,..Default::default()},|p| {
            table=p.table((0..12).map(|i|TableColumn::new(i.to_string(),100.0)).collect(),Vec::new(),
                TableStyle {width:300.0,height:180.0,padding:0.0,reorder_columns:true,..Default::default()});
        });
        ui.sync(&mut scene);let b=scene.get(ui.widgets[&table].node.unwrap()).unwrap().bounds;
        ui.pointer_pressed(&mut scene,b.x+20.0,b.y+12.0);ui.pointer_moved(b.x+298.0,b.y+12.0);
        let before=ui.table_scroll_offset(table).unwrap().0;assert!(ui.pointer_repeat_delay().is_some());
        std::thread::sleep(std::time::Duration::from_millis(40));assert!(ui.repeat_pointer());
        assert!(ui.table_scroll_offset(table).unwrap().0>before);
        ui.pointer_released();assert!(ui.pointer_repeat_delay().is_none());assert!(!ui.repeat_pointer());
    }

    #[test]
    fn header_drag_preserves_source_columns_defers_sort_and_cancels_without_reordering() {
        let mut ui = Ui::new(); let mut scene = Scene::new(); let mut table = WidgetId(0);
        ui.column(ColumnStyle { width:400.0,height:220.0,padding:0.0,..Default::default() }, |p| {
            table = p.table((0..5).map(|i| TableColumn::new(format!("C{i}"),100.0)).collect(),
                vec![vec!["a".into(),"hidden".into(),"c".into(),"d".into(),"e".into()]],
                TableStyle { width:400.0,height:200.0,padding:0.0,reorder_columns:true,..Default::default() });
        });
        ui.set_table_visible_columns(table,&[3,0,2,4]); ui.set_table_frozen_cols(table,1); ui.sync(&mut scene);
        let b = scene.get(ui.widgets[&table].node.unwrap()).unwrap().bounds;
        let (x,y) = (b.x+25.0,b.y+12.0);
        ui.pointer_pressed(&mut scene,x,y); assert_eq!(ui.take_table_header_click(),None);
        ui.pointer_moved(x+2.0,y); ui.pointer_released();
        assert_eq!(ui.take_table_header_click(),Some((table,3))); assert_eq!(ui.take_table_column_order(),None);
        ui.pointer_pressed(&mut scene,x,y); ui.pointer_moved(b.x+280.0,y); ui.sync(&mut scene);
        assert_eq!(ui.table_visible_columns(table).unwrap(),[3,0,2,4]);
        assert!(scene.get(*ui.widgets[&table].aux.last().unwrap()).unwrap().visible);
        ui.cancel_pointer_capture(); ui.pointer_released(); assert_eq!(ui.take_table_column_order(),None);
        ui.pointer_pressed(&mut scene,x,y); ui.pointer_moved(b.x+280.0,y); ui.pointer_released(); ui.sync(&mut scene);
        assert_eq!(ui.take_table_column_order(),Some((table,vec![0,2,3,4])));
        assert_eq!(ui.take_table_header_click(),None);
        assert_eq!(ui.table_cell_at(&scene,table,b.x+25.0,b.y+40.0),Some((0,0)));
        assert!(!scene.get(*ui.widgets[&table].aux.last().unwrap()).unwrap().visible);
        // Resize grips retain priority over the header drag gesture.
        ui.pointer_pressed(&mut scene,b.x+100.0,y); ui.pointer_moved(b.x+130.0,y); ui.pointer_released();
        assert_eq!(ui.table_column_widths(table).unwrap()[0],130.0);
        assert_eq!(ui.take_table_column_order(),None); assert_eq!(ui.take_table_header_click(),None);
        ui.pointer_pressed(&mut scene,b.x+25.0,y); ui.pointer_moved(b.x+250.0,b.y+150.0); ui.pointer_released();
        assert_eq!(ui.take_table_column_order(),None); assert_eq!(ui.take_table_header_click(),None);
    }

    #[test]
    fn column_layout_keeps_source_identity_filters_keyboard_ranges_and_resize() {
        let mut ui = Ui::new(); let mut scene = Scene::new();
        let mut table = WidgetId(0); let mut fields = Vec::new();
        ui.column(ColumnStyle { width: 360.0, height: 240.0, padding: 0.0, ..Default::default() }, |p| {
            (table, fields) = p.header_filtered_table(
                (0..4).map(|i| TableColumn::new(format!("C{i}"), 100.0)).collect(),
                vec![vec!["a".into(), "hidden".into(), "c".into(), "d".into()]], &[],
                TableStyle { width: 360.0, height: 200.0, padding: 0.0, header_height: 24.0, ..Default::default() },
                TextInputStyle { height: 24.0, ..Default::default() });
        });
        ui.sync(&mut scene);
        assert!(ui.set_table_visible_columns(table, &[3, 0, 2]));
        for invalid in [&[][..], &[0, 0], &[4]] { assert!(!ui.set_table_visible_columns(table, invalid)); }
        assert_eq!(ui.table_visible_columns(table).unwrap(), [3, 0, 2]);
        ui.set_table_frozen_cols(table, 1); ui.sync(&mut scene);
        let grid = scene.get(ui.widgets[&table].node.unwrap()).unwrap().bounds;
        for (x, col) in [(20.0, 3), (120.0, 0), (220.0, 2)] {
            assert_eq!(ui.table_header_column_at(&scene, table, grid.x+x, grid.y+12.0), Some(col));
            assert_eq!(ui.table_cell_at(&scene, table, grid.x+x, grid.y+ui.table_header_height(table).unwrap()+8.0), Some((0,col)));
        }
        assert!(!ui.focusable_widgets().contains(&fields[1]));
        let hidden = scene.get(ui.widgets[&fields[1]].node.unwrap()).unwrap();
        assert!(hidden.bounds.is_empty());
        assert!(hidden.clip.unwrap().is_empty());
        let focus = ui.focusable_widgets();
        assert!(focus.iter().position(|i| *i == fields[3]).unwrap() < focus.iter().position(|i| *i == fields[0]).unwrap());
        ui.set_table_selected_cells(table, Some((0,0)), Some((3,3))); ui.set_focus(Some(table));
        ui.key_input(UiKey::SelectRight); ui.key_input(UiKey::SelectRight);
        assert_eq!(ui.table_selected_column_indices(table).unwrap(), [3,0,2]);
        assert_eq!(ui.table_selected_text(table).as_deref(), Some("d\ta\tc"));
        ui.key_input(UiKey::Left); assert_eq!(ui.table_selected_col(table), Some(0));
        assert!(ui.set_table_column_width(table, 3, 150.0)); ui.sync(&mut scene);
        assert_eq!(ui.table_header_column_at(&scene, table, grid.x+160.0, grid.y+12.0), Some(0));
        assert!(ui.set_table_visible_columns(table, &[0,1,2,3])); ui.sync(&mut scene);
        assert!(ui.focusable_widgets().contains(&fields[1]));
    }
}

