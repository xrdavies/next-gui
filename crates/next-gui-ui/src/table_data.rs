use super::*;
use std::borrow::Cow;

/// Immutable table rows fetched on demand. Keep the row count stable for this source.
/// Reads should be bounded and local; perform network queries before building the table.
pub trait TableDataSource: std::fmt::Debug + Send + Sync {
    /// Number of rows, including rows outside the viewport.
    fn len(&self) -> usize;
    /// Whether this source has no rows.
    fn is_empty(&self) -> bool { self.len() == 0 }
    /// Read one row. Failures are displayed inline and reported through `Ui::take_table_data_error`.
    fn row(&self, index: usize) -> Result<Vec<String>, String>;
}

#[derive(Debug, Clone)]
pub(crate) enum TableRows {
    Inline(Arc<Vec<Vec<String>>>),
    Source { data: Arc<dyn TableDataSource>, count: usize },
}
impl From<Vec<Vec<String>>> for TableRows {
    fn from(rows: Vec<Vec<String>>) -> Self { Self::Inline(Arc::new(rows)) }
}
impl TableRows {
    pub fn len(&self) -> usize {
        match self { Self::Inline(rows) => rows.len(), Self::Source { count, .. } => *count }
    }
    pub fn is_empty(&self) -> bool { self.len() == 0 }
    pub fn row(&self, index: usize) -> Result<Cow<'_, [String]>, String> {
        if index >= self.len() { return Err("Table row is outside the data source".into()); }
        match self {
            Self::Inline(rows) => Ok(Cow::Borrowed(&rows[index])),
            Self::Source { data, .. } => data.row(index).map(Cow::Owned),
        }
    }
}

impl Ui {
    /// Replace rows with an indexed source and retain the table's filter controls.
    /// Clamp selection and clear stale styles/checks like `set_table_rows`.
    pub fn set_table_data_source(&mut self, id: WidgetId, data: Arc<dyn TableDataSource>) -> bool {
        if !matches!(
            self.widgets.get(&id).map(|widget| &widget.kind),
            Some(Kind::Table { .. })
        ) {
            return false;
        }
        let count = data.len();
        self.set_table_body_rows(id, TableRows::Source { data, count });
        if let Some(Widget {
            kind: Kind::Table { virtualized, .. },
            ..
        }) = self.widgets.get_mut(&id)
        {
            *virtualized = true;
        }
        true
    }
    /// Drain the last row read failure from painting or copying a table.
    /// Failed copies return no text, so partial data never reaches the clipboard.
    pub fn take_table_data_error(&self) -> Option<(WidgetId, String)> {
        self.pending_table_data_error.borrow_mut().take()
    }
}

impl FlexBuilder<'_> {
    /// Indexed rows with the same aligned filter controls as `filtered_table`.
    pub fn filtered_data_table(
        &mut self,
        columns: Vec<TableColumn>,
        data: Arc<dyn TableDataSource>,
        values: &[String],
        style: TableStyle,
        input_style: TextInputStyle,
    ) -> (WidgetId, Vec<WidgetId>) {
        let (table, fields) = self.filtered_table(columns, Vec::new(), values, style, input_style);
        self.ui.set_table_data_source(table, data);
        (table, fields)
    }

    /// Virtualized table backed by an immutable, on-demand row source.
    /// Painting reads only visible rows plus a small overscan; selection and copying
    /// use global row indices. Existing cell editing is application-driven.
    pub fn data_table(
        &mut self,
        columns: Vec<TableColumn>,
        data: Arc<dyn TableDataSource>,
        style: TableStyle,
    ) -> WidgetId {
        let count = data.len();
        let id = self.virtual_table(columns, Vec::new(), style);
        if let Kind::Table { rows, .. } = &mut self.ui.widgets.get_mut(&id).unwrap().kind {
            *rows = TableRows::Source { data, count };
        }
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct Data { count: usize, reads: Arc<Mutex<Vec<usize>>>, fail: Option<usize> }
    impl TableDataSource for Data {
        fn len(&self) -> usize { self.count }
        fn row(&self, index: usize) -> Result<Vec<String>, String> {
            self.reads.lock().unwrap().push(index);
            if self.fail == Some(index) { return Err("snapshot read failed".into()); }
            Ok(vec![index.to_string(), format!("row\t{index}\nquoted \"text\"")])
        }
    }

    #[test]
    fn million_row_source_only_reads_visible_rows_and_navigates_copies_global_ranges() {
        let mut ui = Ui::new(); let mut scene = Scene::new(); let mut table = WidgetId(0);
        let reads = Arc::new(Mutex::new(Vec::new()));
        let data = Arc::new(Data { count: 1_000_000, reads: reads.clone(), fail: None });
        ui.column(ColumnStyle::default(), |c| {
            table = c.data_table(vec![TableColumn::new("Row", 100.0), TableColumn::new("Text", 250.0)], data, TableStyle { width: 300.0, height: 180.0, ..Default::default() });
        });
        ui.set_table_frozen_cols(table, 1);
        ui.set_focus(Some(table)); ui.sync(&mut scene);
        assert!(reads.lock().unwrap().len() < 20);
        assert!(ui.widgets[&table].aux.len() < 120);
        reads.lock().unwrap().clear();
        assert!(ui.key_input(UiKey::End)); ui.sync(&mut scene);
        assert_eq!(ui.table_selected_row(table), Some(999_999));
        assert!(reads.lock().unwrap().contains(&999_999));
        assert!(reads.lock().unwrap().len() < 20);
        assert!(ui.key_input(UiKey::SelectUp));
        assert!(ui.set_table_selected_cells(table, Some((999_999, 999_998)), Some((1, 0))));
        reads.lock().unwrap().clear();
        let expected = (999_998..=999_999).map(|i| format!("{i}\t\"row\t{i}\nquoted \"\"text\"\"\"")).collect::<Vec<_>>().join("\n");
        assert_eq!(ui.focused_text_for_copy(), Some(expected));
        assert_eq!(*reads.lock().unwrap(), vec![999_998, 999_999]);
        assert!(ui.key_input(UiKey::PageUp)); ui.sync(&mut scene);
        assert!(ui.table_selected_row(table).unwrap() < 999_998);
        assert!(ui.key_input(UiKey::Home)); ui.sync(&mut scene);
        assert_eq!(ui.table_selected_row(table), Some(0));
        ui.set_table_rows(table, vec![vec!["replacement".into()]]);
        reads.lock().unwrap().clear(); ui.sync(&mut scene);
        assert!(reads.lock().unwrap().is_empty());
        assert_eq!(ui.table_selected_row(table), Some(0));
        assert_eq!(ui.take_table_data_error(), None);
    }

    #[test]
    fn failed_row_reads_are_visible_and_never_copy_partial_results() {
        let mut ui = Ui::new(); let mut scene = Scene::new(); let mut table = WidgetId(0);
        ui.column(ColumnStyle::default(), |c| {
            table = c.data_table(vec![TableColumn::new("Row", 280.0)], Arc::new(Data { count: 3, reads: Arc::default(), fail: Some(1) }), TableStyle::default());
        });
        ui.sync(&mut scene);
        assert_eq!(ui.take_table_data_error(), Some((table, "snapshot read failed".into())));
        assert_eq!(ui.take_table_data_error(), None);
        let error_row = &ui.widgets[&table].aux[1 + 3 + 2];
        assert!(scene.get(*error_row).unwrap().mask.is_some());
        assert!(ui.set_table_selected_rows(table, Some((0, 2))));
        assert_eq!(ui.table_selected_text(table), None);
        assert_eq!(ui.take_table_data_error(), Some((table, "snapshot read failed".into())));
        assert!(ui.set_table_selected_rows(table, Some((2, 2))));
        assert_eq!(ui.table_selected_text(table).as_deref(), Some("2"));
    }
}

#[cfg(test)]
mod indexed_style_tests {
    use super::*;
    use std::sync::Mutex;
    #[derive(Debug)]
    struct Rows;
    impl TableDataSource for Rows {
        fn len(&self) -> usize {
            1_000_000
        }
        fn row(&self, index: usize) -> Result<Vec<String>, String> {
            Ok(vec![index.to_string()])
        }
    }
    #[derive(Debug)]
    struct Styles(Arc<Mutex<Vec<usize>>>);
    impl TableCellStyleSource for Styles {
        fn cell(&self, row: usize, _column: usize) -> Option<TableCellStyle> {
            self.0.lock().unwrap().push(row);
            Some(TableCellStyle {
                foreground: Some(Color::rgb(100, 30, 40)),
                size: (row == 999999).then_some(f32::NAN),
                ..Default::default()
            })
        }
    }
    #[test]
    fn filtered_indexed_rows_and_styles_read_only_visible_cells_and_reset_together() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut table = WidgetId(0);
        let mut filters = Vec::new();
        let reads = Arc::new(Mutex::new(Vec::new()));
        ui.column(
            ColumnStyle {
                width: 340.0,
                height: 240.0,
                ..Default::default()
            },
            |column| {
                (table, filters) = column.filtered_data_table(
                    vec![TableColumn::new("Record", 280.0)],
                    Arc::new(Rows),
                    &["".into()],
                    TableStyle {
                        width: 300.0,
                        height: 200.0,
                        ..Default::default()
                    },
                    TextInputStyle::default(),
                );
            },
        );
        assert_eq!(filters.len(), 1);
        assert!(ui.set_table_cell_style_source(table, Arc::new(Styles(reads.clone()))));
        ui.set_focus(Some(table));
        ui.sync(&mut scene);
        assert!(reads.lock().unwrap().len() < 20);
        assert!(ui.widgets[&table].aux.len() < 100);
        assert!(ui.take_table_data_error().is_none());
        reads.lock().unwrap().clear();
        ui.key_input(UiKey::End);
        ui.sync(&mut scene);
        assert_eq!(ui.table_selected_row(table), Some(999999));
        assert!(reads.lock().unwrap().contains(&999999));
        assert!(reads.lock().unwrap().len() < 20);
        assert!(ui.take_table_data_error().unwrap().1.contains("font size"));
        ui.set_table_selected_rows(table, Some((999998, 999999)));
        assert_eq!(
            ui.table_selected_text(table).as_deref(),
            Some("999998\n999999")
        );
        reads.lock().unwrap().clear();
        ui.set_table_rows(table, vec![vec!["replacement".into()]]);
        ui.sync(&mut scene);
        assert!(reads.lock().unwrap().is_empty());
        assert_eq!(ui.table_selected_row(table), Some(0));
        assert!(ui.take_table_data_error().is_none());
    }
}
