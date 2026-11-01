use super::*;
use std::borrow::Cow;

/// Immutable Select options fetched on demand. Keep their count and order stable.
/// Reads must be bounded and local; complete database/network queries before mounting.
pub trait SelectDataSource: std::fmt::Debug + Send + Sync {
    /// Total number of options, including those outside the popup.
    fn len(&self) -> usize;
    /// Whether the source contains no options.
    fn is_empty(&self) -> bool { self.len() == 0 }
    /// Read one display label. Application values remain indexed in the source.
    fn label(&self, index: usize) -> Result<String, String>;
}

#[derive(Debug, Clone)]
pub(crate) enum SelectOptions {
    Inline(Arc<Vec<String>>),
    Source { data: Arc<dyn SelectDataSource>, count: usize },
}
impl From<Vec<String>> for SelectOptions {
    fn from(options: Vec<String>) -> Self { Self::Inline(Arc::new(options)) }
}
impl SelectOptions {
    pub fn len(&self) -> usize {
        match self { Self::Inline(options) => options.len(), Self::Source { count, .. } => *count }
    }
    pub fn is_empty(&self) -> bool { self.len() == 0 }
    pub fn label(&self, index: usize) -> Result<Cow<'_, str>, String> {
        if index >= self.len() { return Err("Select option is outside the data source".into()); }
        match self {
            Self::Inline(options) => Ok(Cow::Borrowed(&options[index])),
            Self::Source { data, .. } => data.label(index).map(Cow::Owned),
        }
    }
    pub fn source(&self) -> Option<Arc<dyn SelectDataSource>> {
        match self { Self::Source { data, .. } => Some(data.clone()), _ => None }
    }
}

impl Ui {
    /// Drain the last Select label read failure. A failed selection retains its old value.
    pub fn take_select_data_error(&self) -> Option<(WidgetId, String)> {
        self.pending_select_data_error.borrow_mut().take()
    }
}

impl FlexBuilder<'_> {
    /// Select backed by immutable, indexed options. Painting reads only visible labels.
    /// Selection uses global indices; failed label reads never change the selected value.
    pub fn data_select(&mut self, data: Arc<dyn SelectDataSource>, selected: Option<usize>, placeholder: impl Into<String>, style: SelectStyle) -> WidgetId {
        let count = data.len();
        let id = self.select(std::iter::empty::<String>(), None, placeholder, style);
        if let Kind::Select { options, .. } = &mut self.ui.widgets.get_mut(&id).unwrap().kind {
            *options = SelectOptions::Source { data, count };
        }
        self.ui.set_select_selected(id, selected);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct Data { count: usize, reads: Arc<Mutex<Vec<usize>>>, fail: Option<usize> }
    impl SelectDataSource for Data {
        fn len(&self) -> usize { self.count }
        fn label(&self, index: usize) -> Result<String, String> {
            self.reads.lock().unwrap().push(index);
            if self.fail == Some(index) { return Err("option snapshot unavailable".into()); }
            Ok(format!("Choice {index}"))
        }
    }

    #[test]
    fn million_options_read_only_visible_labels_and_keep_global_selection_and_sessions() {
        let reads = Arc::new(Mutex::new(Vec::new()));
        let data: Arc<dyn SelectDataSource> = Arc::new(Data { count: 1_000_000, reads: reads.clone(), fail: None });
        let mut ui = Ui::new(); let mut scene = Scene::new(); let mut id = WidgetId(0);
        ui.column(ColumnStyle::default(), |c| { id = c.data_select(data.clone(), Some(0), "Choose", SelectStyle::default()); });
        ui.set_focus(Some(id)); ui.sync(&mut scene);
        assert_eq!(*reads.lock().unwrap(), vec![0]);
        assert_eq!(ui.select_value(id), Some("Choice 0"));
        reads.lock().unwrap().clear();
        assert!(ui.key_input(UiKey::End)); ui.sync(&mut scene);
        assert_eq!(ui.select_selected(id), Some(999_999));
        assert_eq!(ui.select_value(id), Some("Choice 999999"));
        assert!(reads.lock().unwrap().len() < 20);
        assert!(ui.widgets[&id].aux.len() < 40);
        let (panel, visible) = ui.select_popup_window(&scene, id).unwrap();
        let step = visible.len(); let item_h = match ui.widgets[&id].kind { Kind::Select { style, .. } => style.item_height, _ => unreachable!() };
        assert!(ui.pointer_popup_item(&mut scene, id, panel.x + 8.0, panel.y + item_h * 0.5));
        assert_eq!(ui.select_selected(id), Some(visible.start));
        assert!(!ui.has_open_popup());
        assert!(ui.key_input(UiKey::Home)); assert_eq!(ui.select_selected(id), Some(0));
        assert!(ui.key_input(UiKey::PageDown)); assert!(ui.select_selected(id).unwrap() >= step - 1);
        assert!(ui.key_input(UiKey::PageUp)); assert_eq!(ui.select_selected(id), Some(0));
        let session = ui.select_state(id).unwrap();
        let mut rebuilt = Ui::new(); let mut next = WidgetId(0);
        rebuilt.column(ColumnStyle::default(), |c| { next = c.data_select(data, Some(0), "", SelectStyle::default()); });
        assert!(rebuilt.restore_select_state(next, &session)); assert!(rebuilt.has_open_popup());
        let mut changed = Ui::new(); let mut other = WidgetId(0);
        changed.column(ColumnStyle::default(), |c| { other = c.data_select(Arc::new(Data { count: 1_000_000, reads, fail: None }), Some(0), "", SelectStyle::default()); });
        assert!(!changed.restore_select_state(other, &session), "a new source with equal labels is a different session");
        assert_eq!(ui.take_select_data_error(), None);
    }

    #[test]
    fn failed_options_are_visible_but_never_replace_the_current_selection() {
        let mut ui = Ui::new(); let mut scene = Scene::new(); let mut id = WidgetId(0);
        ui.column(ColumnStyle::default(), |c| { id = c.data_select(Arc::new(Data { count: 3, reads: Arc::default(), fail: Some(1) }), Some(0), "Choose", SelectStyle::default()); });
        ui.set_focus(Some(id)); ui.key_input(UiKey::Space); ui.sync(&mut scene);
        assert_eq!(ui.take_select_data_error(), Some((id, "option snapshot unavailable".into())));
        assert_eq!(ui.take_select_data_error(), None);
        let error_node = ui.widgets[&id].aux[4]; assert!(scene.get(error_node).unwrap().visible && scene.get(error_node).unwrap().mask.is_some());
        assert!(!ui.set_select_selected(id, Some(1)));
        assert_eq!(ui.select_selected(id), Some(0)); assert_eq!(ui.select_value(id), Some("Choice 0"));
        let (panel, _) = ui.select_popup_window(&scene, id).unwrap();
        let item_h = match ui.widgets[&id].kind { Kind::Select { style, .. } => style.item_height, _ => unreachable!() };
        assert!(ui.pointer_popup_item(&mut scene, id, panel.x + 8.0, panel.y + item_h * 1.5));
        assert_eq!(ui.select_selected(id), Some(0)); assert!(ui.has_open_popup());
        assert!(ui.key_input(UiKey::Down)); assert_eq!(ui.select_selected(id), Some(0));
        assert!(ui.key_input(UiKey::End)); assert_eq!(ui.select_selected(id), Some(2));
        assert!(!ui.set_select_selected(id, Some(3))); assert_eq!(ui.select_selected(id), Some(2));
        assert!(ui.set_select_selected(id, None)); assert_eq!(ui.select_value(id), None);
    }
}
