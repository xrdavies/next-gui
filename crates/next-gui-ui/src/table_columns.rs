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

