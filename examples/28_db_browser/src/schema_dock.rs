//! Cached schema navigation and source-checked SQL insertion, independent of Structure selection.
use std::collections::BTreeSet;

use next_gui_ui::{
    FlexBuilder, MenuItem, RowStyle, SplitterStyle, TextAreaState, TextAreaStyle, TextInputState,
    TextInputStyle, Theme, TreeNode, TreeStyle, Ui, WidgetId,
};

use crate::{
    db,
    state::{AppState, MainTab},
    Demo, KeyCode, Modifiers,
};

const INSERT: u64 = 1100;
const DEFINITION: u64 = 1101;
const BROWSE: u64 = 1102;
const SELECT_QUERY: u64 = 1103;
const QUALIFY: u64 = 1104;
const QUOTE: u64 = 1105;

pub const QA_SQL: &str = "CREATE TABLE records(id INTEGER PRIMARY KEY, \"value.名\" TEXT, computed AS(length(\"value.名\")));
    INSERT INTO records VALUES(1,'保留'); CREATE TEMP TABLE records(temp_only);
    ATTACH ':memory:' AS \"aux.\"\"db\";
    CREATE TABLE \"aux.\"\"db\".records(id INTEGER PRIMARY KEY, \"value.名\" TEXT);
    INSERT INTO \"aux.\"\"db\".records VALUES(2,'attached');
    CREATE INDEX \"aux.\"\"db\".by_value ON records(\"value.名\");
    CREATE VIEW \"aux.\"\"db\".summary AS SELECT * FROM records;";

pub fn prepare_qa(state: &mut AppState, narrow: bool) {
    state.new_memory_db().unwrap();
    state.conn.as_ref().unwrap().execute_batch(QA_SQL).unwrap();
    state.write_changes().unwrap();
    state.refresh_schema().unwrap();
    state.tab = MainTab::ExecuteSql;
    state.dock_edit = false;
    state.dock_schema = true;
    state.dock_focus = crate::state::DockKind::Schema;
    *state.sql_editor_mut() = "SELECT ? AS \"保留\";".into();
    state.sql_edit_states.clear();
    if narrow {
        state.prefs.font_size = 18.0;
    }
    state.schema_dock.selected = Some(vec![
        "aux.\"db".into(),
        "table".into(),
        "records".into(),
        "value.名".into(),
    ]);
}

// Path lengths distinguish database / object / column, including empty and dotted identifiers.
type Path = Vec<String>;

#[derive(Debug, Clone, Default)]
pub struct State {
    pub filter: String,
    selected: Option<Path>,
    collapsed: BTreeSet<Path>,
    scroll: f32,
    ratio: Option<f32>,
    filter_view: Option<TextInputState>,
    preview: Option<(Path, TextAreaState)>,
}

#[derive(Clone)]
pub struct Controls {
    pub tree: WidgetId,
    pub filter: WidgetId,
    pub preview: WidgetId,
    insert: WidgetId,
    definition: WidgetId,
    browse: WidgetId,
    options: WidgetId,
    splitter: WidgetId,
    paths: Vec<Path>,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            tree: WidgetId(0),
            filter: WidgetId(0),
            preview: WidgetId(0),
            insert: WidgetId(0),
            definition: WidgetId(0),
            browse: WidgetId(0),
            options: WidgetId(0),
            splitter: WidgetId(0),
            paths: Vec::new(),
        }
    }
}

fn object<'a>(state: &'a AppState, path: &[String]) -> Option<&'a db::SchemaObject> {
    if path.len() < 3 {
        return None;
    }
    state
        .schema
        .iter()
        .find(|o| o.schema == path[0] && o.kind.sql_type() == path[1] && o.name == path[2])
        .filter(|o| {
            path.len() == 3
                || path.len() == 4
                    && state
                        .sql_completion
                        .columns_in(&o.schema, &o.name)
                        .any(|c| c == path[3])
        })
}

fn insertion(state: &AppState, path: &[String], definition: bool) -> Option<String> {
    let o = object(state, path)?;
    if definition {
        return o.sql.clone().filter(|s| !s.is_empty());
    }
    let name = |s: &str| {
        if state.prefs.schema_quoted_names {
            db::quote_ident(s)
        } else {
            s.to_owned()
        }
    };
    let mut names = if state.prefs.schema_qualified_names {
        vec![name(&o.schema), name(&o.name)]
    } else {
        vec![name(&o.name)]
    };
    if path.len() == 4 {
        if !state.prefs.schema_qualified_names {
            names.clear();
        }
        names.push(name(&path[3]));
    }
    let text = names.join(".");
    Some(
        if path.len() == 3
            && state.prefs.schema_select_query
            && matches!(o.kind, db::ObjectKind::Table | db::ObjectKind::View)
        {
            format!("SELECT * FROM {text};\n")
        } else {
            text
        },
    )
}

fn model(state: &AppState) -> (Vec<TreeNode>, Vec<Path>) {
    let mut paths = Vec::new();
    let mut node = |path: Path, label: String, children: Vec<TreeNode>| {
        let mut n = TreeNode::new(paths.len() as u64 + 1, label, children);
        n.expanded = !state.schema_dock.collapsed.contains(&path);
        paths.push(path);
        n
    };
    let filter = state.schema_dock.filter.to_lowercase();
    let mut roots = Vec::new();
    for schema in &state.schema_names {
        let mut objects = Vec::new();
        let database_matches = schema.to_lowercase().contains(&filter);
        for o in state.schema.iter().filter(|o| &o.schema == schema) {
            let path = vec![schema.clone(), o.kind.sql_type().into(), o.name.clone()];
            let matches = database_matches
                || o.name.to_lowercase().contains(&filter)
                || o.kind.sql_type().contains(&filter);
            let mut columns = Vec::new();
            for column in state
                .sql_completion
                .columns_in(schema, &o.name)
                .filter(|_| matches!(o.kind, db::ObjectKind::Table | db::ObjectKind::View))
            {
                if matches || column.to_lowercase().contains(&filter) {
                    let mut key = path.clone();
                    key.push(column.into());
                    columns.push(node(key, column.into(), Vec::new()));
                }
            }
            if matches || !columns.is_empty() {
                let mut item = node(path, format!("{} · {}", o.name, o.kind.sql_type()), columns);
                if !filter.is_empty() {
                    item.expanded = true;
                }
                objects.push(item);
            }
        }
        if database_matches || !objects.is_empty() {
            let mut database = node(vec![schema.clone()], schema.clone(), objects);
            if !filter.is_empty() {
                database.expanded = true;
            }
            roots.push(database);
        }
    }
    (roots, paths)
}

pub fn build(
    p: &mut FlexBuilder<'_>,
    state: &AppState,
    theme: &Theme,
    width: f32,
    height: f32,
) -> Controls {
    let (roots, paths) = model(state);
    let mut c = Controls {
        paths,
        ..Controls::default()
    };
    let mut input = TextInputStyle::from_theme(theme);
    input.width = (width - 8.0).max(1.0);
    c.filter = p.text_input_styled(
        &state.schema_dock.filter,
        "Filter objects and fields…",
        input,
    );
    let mut row = RowStyle::from_theme(theme);
    row.width = input.width;
    row.height = input.height.max(28.0);
    row.padding = 0.0;
    p.scroll_row(row, |r| {
        c.insert = r.tool_button("Insert");
        c.definition = r.tool_button("Insert SQL");
        c.browse = r.tool_button("Browse");
        c.options = r.tool_button("Options");
    });
    let available = (height - input.height - row.height - 12.0).max(40.0);
    let mut split = SplitterStyle::from_theme(theme);
    split.axis = next_gui_layout::Axis::Horizontal;
    split.width = input.width;
    split.height = available;
    split.ratio = state.schema_dock.ratio.unwrap_or(0.65).clamp(0.25, 0.85);
    let mut tree = TreeStyle::from_theme(theme);
    tree.width = input.width;
    tree.height = ((available - split.sash_size) * split.ratio).max(1.0);
    let mut preview = TextAreaStyle::from_theme(theme);
    preview.width = input.width;
    preview.height = (available - split.sash_size - tree.height).max(1.0);
    let text = state
        .schema_dock
        .selected
        .as_deref()
        .filter(|path| c.paths.iter().any(|p| p == path))
        .and_then(|path| object(state, path))
        .and_then(|o| o.sql.as_deref())
        .unwrap_or("");
    c.splitter = p.splitter(
        split,
        |p| {
            c.tree = p.tree(roots, tree);
        },
        |p| {
            c.preview = p.text_area(text, "Select an object to inspect its SQL", preview);
        },
    );
    c
}

impl Controls {
    pub fn restore(&self, ui: &mut Ui, state: &AppState, theme: &Theme) {
        if let Some(path) = &state.schema_dock.selected {
            ui.set_tree_selected(
                self.tree,
                self.paths
                    .iter()
                    .position(|p| p == path)
                    .map(|i| i as u64 + 1),
            );
        }
        ui.set_tree_scroll_offset(self.tree, state.schema_dock.scroll);
        if let Some(view) = &state.schema_dock.filter_view {
            ui.restore_text_input_state(self.filter, view);
        }
        ui.set_text_area_read_only(self.preview, true);
        if let Some((path, view)) = &state.schema_dock.preview {
            if state.schema_dock.selected.as_ref() == Some(path) {
                ui.restore_text_area_state(self.preview, view);
            }
        }
        crate::sql_syntax::apply(ui, self.preview, theme);
        let path = state.schema_dock.selected.as_deref().unwrap_or_default();
        let idle = !state.sql_busy
            && state.guard_background_idle().is_ok()
            && !state.has_cell_draft()
            && self.paths.iter().any(|p| p == path);
        ui.set_button_enabled(self.insert, idle && insertion(state, path, false).is_some());
        ui.set_button_enabled(
            self.definition,
            idle && insertion(state, path, true).is_some(),
        );
        ui.set_button_enabled(
            self.browse,
            idle && object(state, path)
                .is_some_and(|o| matches!(o.kind, db::ObjectKind::Table | db::ObjectKind::View)),
        );
        for (id, label) in [
            (self.tree, "Database schema objects and fields"),
            (self.filter, "Filter database schema"),
            (self.preview, "Selected object SQL · read only"),
            (
                self.insert,
                "Insert selected name or SELECT into the current SQL editor",
            ),
            (
                self.definition,
                "Insert selected object SQL into the current SQL editor",
            ),
            (self.browse, "Browse the selected table or view"),
            (self.options, "Schema insertion options"),
            (self.splitter, "Resize schema tree and SQL preview"),
        ] {
            ui.set_accessible_name(id, label);
        }
    }

    fn focus_ids(&self) -> [WidgetId; 8] {
        [
            self.filter,
            self.tree,
            self.preview,
            self.insert,
            self.definition,
            self.browse,
            self.options,
            self.splitter,
        ]
    }
}

impl Demo {
    pub(crate) fn snapshot_schema_dock(&mut self) {
        let c = &self.ids.schema;
        if c.tree.0 == 0 {
            return;
        }
        if let Some(filter) = self.ui.text_input_value(c.filter) {
            let was_unfiltered = self.state.schema_dock.filter.is_empty();
            self.state.schema_dock.filter_view = self.ui.text_input_state(c.filter);
            if filter != self.state.schema_dock.filter {
                self.state.schema_dock.filter = filter.into();
                self.state.schema_dock.scroll = 0.0;
                self.rebuild = true;
            } else {
                self.state.schema_dock.scroll =
                    self.ui.tree_scroll_offset(c.tree).unwrap_or_default();
            }
            if was_unfiltered && self.state.schema_dock.filter.is_empty() {
                for (i, path) in c.paths.iter().enumerate() {
                    if self.ui.tree_expanded(c.tree, i as u64 + 1) == Some(false) {
                        self.state.schema_dock.collapsed.insert(path.clone());
                    } else {
                        self.state.schema_dock.collapsed.remove(path);
                    }
                }
            }
        }
        let selected = self
            .ui
            .tree_selected(c.tree)
            .and_then(|id| id.checked_sub(1))
            .and_then(|i| c.paths.get(i as usize))
            .cloned();
        if selected != self.state.schema_dock.selected {
            self.state.schema_dock.selected = selected;
            self.state.schema_dock.preview = None;
            self.rebuild = true;
        }
        if let Some(ratio) = self.ui.splitter_ratio(c.splitter) {
            self.state.schema_dock.ratio = Some(ratio);
        }
        if let Some(path) = &self.state.schema_dock.selected {
            // An old preview must not donate its viewport/history to a newly selected object.
            if self.ui.text_area_value(c.preview)
                == object(&self.state, path).and_then(|o| o.sql.as_deref())
            {
                if let Some(view) = self.ui.text_area_state(c.preview) {
                    self.state.schema_dock.preview = Some((path.clone(), view));
                }
            }
        }
    }

    pub(crate) fn schema_dock_focus(&self) -> Option<usize> {
        self.ids
            .schema
            .focus_ids()
            .iter()
            .position(|id| id.0 != 0 && self.ui.focused() == Some(*id))
    }

    pub(crate) fn restore_schema_dock_focus(&mut self, index: Option<usize>) {
        if self.state.overlay.is_none() && !self.state.confirm_open {
            if let Some(id) = index
                .and_then(|i| self.ids.schema.focus_ids().get(i).copied())
                .filter(|id| id.0 != 0)
            {
                self.ui.set_focus(Some(id));
            }
        }
    }

    fn schema_dock_action(&mut self, action: u64) -> Result<(), String> {
        self.state.guard_background_idle()?;
        if self.state.sql_busy {
            return Err("Wait for SQL to finish".into());
        }
        if self.state.has_cell_draft() {
            return Err("Apply or discard the cell draft first".into());
        }
        let path = self
            .state
            .schema_dock
            .selected
            .clone()
            .ok_or("Select a schema object or field")?;
        if !self.ids.schema.paths.contains(&path) {
            return Err("Select a visible schema object or field".into());
        }
        let o = object(&self.state, &path).ok_or("The selected schema object no longer exists")?;
        if action == BROWSE {
            if !matches!(o.kind, db::ObjectKind::Table | db::ObjectKind::View) {
                return Err("Select a table or view".into());
            }
            let (schema, name) = (o.schema.clone(), o.name.clone());
            self.state.switch_browse_table_in(&schema, &name)?;
            self.state.tab = MainTab::Browse;
            self.clear_error();
            self.rebuild = true;
            return Ok(());
        }
        let text = insertion(&self.state, &path, action == DEFINITION)
            .ok_or("This object has no SQL definition")?;
        self.state.tab = MainTab::ExecuteSql;
        self.rebuild = true;
        self.do_rebuild();
        if !self
            .ui
            .replace_text_area_selection(self.ids.sql_area, &text)
        {
            return Err("Cannot insert into the current SQL editor".into());
        }
        self.ui.set_focus(Some(self.ids.sql_area));
        self.snapshot_inputs();
        self.clear_error();
        self.rebuild = true;
        Ok(())
    }

    pub(crate) fn handle_schema_dock_menu(&mut self, action: u64) -> bool {
        if !(INSERT..=QUOTE).contains(&action) {
            return false;
        }
        match action {
            SELECT_QUERY => {
                self.state.prefs.schema_select_query = !self.state.prefs.schema_select_query
            }
            QUALIFY => {
                self.state.prefs.schema_qualified_names = !self.state.prefs.schema_qualified_names
            }
            QUOTE => self.state.prefs.schema_quoted_names = !self.state.prefs.schema_quoted_names,
            _ => {
                if let Err(error) = self.schema_dock_action(action) {
                    self.set_error(error);
                }
                return true;
            }
        }
        if let Err(error) = self.state.prefs.save() {
            self.set_error(error);
        }
        self.rebuild = true;
        true
    }

    pub(crate) fn handle_schema_dock_button(&mut self, id: WidgetId) -> bool {
        let c = &self.ids.schema;
        if id.0 == 0 {
            return false;
        }
        if id == c.options {
            if let Some(node) = self
                .scene
                .iter_paint_order()
                .into_iter()
                .find(|node| self.ui.widget_from_node(node.id) == Some(id))
            {
                let b = node.bounds;
                self.ui.open_context_menu(
                    self.ids.context_menu,
                    vec![
                        MenuItem::new(SELECT_QUERY, "Insert SELECT for tables and views")
                            .checked(self.state.prefs.schema_select_query),
                        MenuItem::new(QUALIFY, "Use qualified names")
                            .checked(self.state.prefs.schema_qualified_names),
                        MenuItem::new(QUOTE, "Quote identifiers")
                            .checked(self.state.prefs.schema_quoted_names),
                    ],
                    b.x,
                    b.y + b.height,
                );
                self.ui.sync(&mut self.scene);
            }
            return true;
        }
        let action = if id == c.insert {
            INSERT
        } else if id == c.definition {
            DEFINITION
        } else if id == c.browse {
            BROWSE
        } else {
            return false;
        };
        if self.ui.button_enabled(id) == Some(false) {
            return true;
        }
        if let Err(error) = self.schema_dock_action(action) {
            self.set_error(error);
        }
        true
    }

    pub(crate) fn handle_schema_dock_key(&mut self, key: KeyCode, modifiers: Modifiers) -> bool {
        if self.state.overlay.is_some()
            || self.state.confirm_open
            || self.ui.has_open_popup()
            || modifiers.alt
            || self.ids.schema.tree.0 == 0
            || self.ui.focused() != Some(self.ids.schema.tree)
        {
            return false;
        }
        if key != KeyCode::Enter {
            return false;
        }
        self.snapshot_inputs();
        if let Err(error) = self.schema_dock_action(if modifiers.commandish() {
            BROWSE
        } else if modifiers.shift {
            DEFINITION
        } else {
            INSERT
        }) {
            self.set_error(error);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PresentBackend, UiKey};

    fn fixture(theme: Theme, width: u32, height: u32, size: f32) -> Demo {
        let mut d = Demo::new(width, height, PresentBackend::Softbuffer);
        d.theme = theme;
        prepare_qa(&mut d.state, false);
        d.state.prefs.font_size = size;
        d.do_rebuild();
        d
    }

    fn path(parts: &[&str]) -> Path {
        parts.iter().map(|s| (*s).into()).collect()
    }

    fn select(d: &mut Demo, parts: &[&str]) {
        let key = path(parts);
        let id = d.ids.schema.paths.iter().position(|p| p == &key).unwrap() as u64 + 1;
        d.ui.set_tree_selected(d.ids.schema.tree, Some(id));
        d.snapshot_inputs();
        d.do_rebuild();
        d.ui.set_focus(Some(d.ids.schema.tree));
        d.ui.sync(&mut d.scene);
    }

    #[test]
    fn qualified_cached_tree_filters_fields_and_retains_collapsed_navigation() {
        let mut d = fixture(Theme::dark(), 1400, 860, 13.0);
        let cached = d.state.conn.take();
        let (_, paths) = model(&d.state);
        for key in [
            path(&["main", "table", "records", "computed"]),
            path(&["temp", "table", "records", "temp_only"]),
            path(&["aux.\"db", "table", "records", "value.名"]),
            path(&["aux.\"db", "view", "summary", "id"]),
            path(&["aux.\"db", "index", "by_value"]),
        ] {
            assert!(paths.contains(&key), "{key:?}");
        }
        d.state.conn = cached;
        select(&mut d, &["main", "table", "records"]);
        let id = d.ui.tree_selected(d.ids.schema.tree).unwrap();
        d.ui.set_tree_expanded(d.ids.schema.tree, id, false);
        d.snapshot_inputs();
        d.do_rebuild();
        assert_eq!(d.ui.tree_expanded(d.ids.schema.tree, id), Some(false));
        d.ui.set_focus(Some(d.ids.schema.filter));
        d.ui.key_input(UiKey::SelectAll);
        assert!(d.ui.paste_into_focused_text("temp_ONLY"));
        d.snapshot_inputs();
        d.do_rebuild();
        assert_eq!(d.ui.focused(), Some(d.ids.schema.filter));
        assert_eq!(d.ids.schema.paths.len(), 3);
        assert!(d
            .ids
            .schema
            .paths
            .contains(&path(&["temp", "table", "records", "temp_only"])));
        assert!(d.ui.text_input_can_undo(d.ids.schema.filter));
        d.ui.key_input(UiKey::Undo);
        d.snapshot_inputs();
        d.do_rebuild();
        assert!(d.state.schema_dock.filter.is_empty());
        let id = d
            .ids
            .schema
            .paths
            .iter()
            .position(|p| p == &path(&["main", "table", "records"]))
            .unwrap() as u64
            + 1;
        assert_eq!(d.ui.tree_expanded(d.ids.schema.tree, id), Some(false));
        assert_eq!(d.state.selected.as_ref().map(|s| s.name.as_str()), None);
    }

    #[test]
    fn schema_insert_preserves_sql_selection_undo_tabs_and_never_executes_sql() {
        let mut d = fixture(Theme::light(), 1400, 860, 13.0);
        let original = "SELECT '保留';\nSELECT ?;";
        d.ui.set_text_area_value(d.ids.sql_area, original);
        let at = original.chars().position(|c| c == '?').unwrap();
        d.ui.select_text_area_range(d.ids.sql_area, at..at + 1);
        d.snapshot_inputs();
        d.state.prefs.schema_qualified_names = true;
        select(&mut d, &["aux.\"db", "table", "records", "value.名"]);
        assert!(d.handle_schema_dock_key(KeyCode::Enter, Modifiers::default()));
        let inserted = "\"aux.\"\"db\".\"records\".\"value.名\"";
        assert_eq!(d.state.sql_editor(), original.replace('?', inserted));
        assert_eq!(d.ui.focused(), Some(d.ids.sql_area));
        d.do_rebuild();
        assert!(d.ui.key_input(UiKey::Undo));
        d.snapshot_inputs();
        assert_eq!(d.state.sql_editor(), original);
        assert_eq!(d.ui.text_area_selection(d.ids.sql_area), Some(at..at + 1));
        assert!(d.ui.key_input(UiKey::Redo));
        d.snapshot_inputs();
        assert_eq!(d.state.sql_editor(), original.replace('?', inserted));
        assert!(!d.state.dirty);
        let conn = d.state.conn.as_ref().unwrap();
        assert_eq!(
            conn.query_row("SELECT \"value.名\" FROM main.records", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "保留"
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM temp.records", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        select(&mut d, &["aux.\"db", "view", "summary"]);
        assert_eq!(
            insertion(
                &d.state,
                d.state.schema_dock.selected.as_ref().unwrap(),
                false
            )
            .unwrap(),
            "SELECT * FROM \"aux.\"\"db\".\"summary\";\n"
        );
        d.state.prefs.schema_select_query = false;
        d.state.prefs.schema_quoted_names = false;
        assert_eq!(
            insertion(
                &d.state,
                d.state.schema_dock.selected.as_ref().unwrap(),
                false
            )
            .unwrap(),
            "aux.\"db.summary"
        );
        let roundtrip: crate::prefs::Prefs =
            serde_json::from_str(&serde_json::to_string(&d.state.prefs).unwrap()).unwrap();
        assert!(
            !roundtrip.schema_quoted_names
                && !roundtrip.schema_select_query
                && roundtrip.schema_qualified_names
        );
        let old: crate::prefs::Prefs = serde_json::from_str("{}").unwrap();
        assert!(old.schema_select_query && old.schema_quoted_names && !old.schema_qualified_names);
    }

    #[test]
    fn schema_preview_browse_and_stale_objects_are_source_checked() {
        let mut d = fixture(Theme::dark(), 1400, 860, 13.0);
        select(&mut d, &["aux.\"db", "table", "records"]);
        let sql =
            d.ui.text_area_value(d.ids.schema.preview)
                .unwrap()
                .to_string();
        assert!(sql.contains("value.名"));
        d.ui.set_focus(Some(d.ids.schema.preview));
        assert!(!d.ui.paste_into_focused_text("DROP TABLE records;"));
        assert_eq!(
            d.ui.text_area_value(d.ids.schema.preview),
            Some(sql.as_str())
        );
        d.schema_dock_action(BROWSE).unwrap();
        d.do_rebuild();
        assert_eq!(d.state.tab, MainTab::Browse);
        assert_eq!(d.state.browse().schema, "aux.\"db");
        assert_eq!(
            d.state.browse().result.as_ref().unwrap().rows[0][1],
            "attached"
        );
        select(&mut d, &["aux.\"db", "index", "by_value"]);
        assert!(d.schema_dock_action(BROWSE).is_err());
        let before = d.state.sql_editor().to_string();
        d.state
            .conn
            .as_ref()
            .unwrap()
            .execute_batch("DROP INDEX \"aux.\"\"db\".by_value;")
            .unwrap();
        d.state.refresh_schema().unwrap();
        assert!(d.schema_dock_action(DEFINITION).is_err());
        assert_eq!(d.state.sql_editor(), before);
        d.do_rebuild();
        assert_eq!(d.ui.tree_selected(d.ids.schema.tree), None);
        assert_eq!(d.ui.button_enabled(d.ids.schema.definition), Some(false));
        d.state.close_db();
        d.do_rebuild();
        assert!(d.ids.schema.paths.is_empty());
        assert_eq!(d.ui.text_area_value(d.ids.schema.preview), Some(""));
    }

    #[test]
    fn schema_actions_keep_cell_drafts_and_worker_owned_state_untouched() {
        let mut d = fixture(Theme::dark(), 1400, 860, 13.0);
        d.state.switch_browse_table("records").unwrap();
        d.state.tab = MainTab::Browse;
        d.state.browse_mut().sel_row = Some(0);
        d.state.browse_mut().sel_col = Some(1);
        d.state.sync_edit_cell_from_selection();
        d.state.edit_value = "pending cell".into();
        d.do_rebuild();
        let before = d.state.sql_editor().to_string();
        assert!(d.schema_dock_action(INSERT).unwrap_err().contains("draft"));
        assert_eq!(d.state.edit_value, "pending cell");
        assert_eq!(d.state.sql_editor(), before);
        d.state.discard_cell_draft();
        d.state.sql_busy = true;
        assert!(d.schema_dock_action(BROWSE).is_err());
        assert_eq!(d.state.browse().schema, "main");
        d.state.sql_busy = false;
    }

    #[test]
    fn schema_layout_options_and_preview_fit_themes_fonts_and_rebuilds() {
        for (name, theme, w, h, size, cell) in [
            ("dark-wide", Theme::dark(), 1400, 860, 13.0, true),
            ("light-wide", Theme::light(), 1400, 860, 18.0, false),
            ("dark-narrow", Theme::dark(), 800, 600, 24.0, false),
            ("light-narrow", Theme::light(), 800, 600, 18.0, true),
        ] {
            let mut d = fixture(theme, w, h, size);
            d.state.dock_edit = cell;
            d.do_rebuild();
            for id in [d.ids.schema.tree, d.ids.schema.preview, d.ids.schema.filter] {
                let b = d
                    .scene
                    .iter_paint_order()
                    .into_iter()
                    .find(|n| d.ui.widget_from_node(n.id) == Some(id))
                    .unwrap()
                    .bounds;
                assert!(b.width > 100.0 && b.height > 10.0, "{name}: {b:?}");
                assert!(
                    b.x >= 0.0
                        && b.y >= 0.0
                        && b.x + b.width <= w as f32
                        && b.y + b.height <= h as f32,
                    "{name}: {b:?}"
                );
            }
            capture(&mut d, name);
            d.ui.set_focus(Some(d.ids.schema.options));
            assert!(d.handle_schema_dock_button(d.ids.schema.options));
            assert_eq!(d.ui.popup_owner(), Some(d.ids.context_menu));
            capture(&mut d, &format!("{name}-options"));
            d.ui.close_popup();
            d.handle_schema_dock_menu(QUALIFY);
            d.do_rebuild();
            assert_eq!(d.ui.focused(), Some(d.ids.schema.options));
            assert!(d.state.prefs.schema_qualified_names);
        }
    }

    fn capture(d: &mut Demo, name: &str) {
        let Ok(folder) = std::env::var("NEXT_GUI_SCHEMA_QA_OUT") else {
            return;
        };
        use next_gui_render::{encode_scene, Renderer};
        let folder = std::path::Path::new(&folder);
        std::fs::create_dir_all(folder).unwrap();
        d.ui.sync(&mut d.scene);
        d.scene.mark_full_dirty(d.win_w as f32, d.win_h as f32);
        let dirty = d.scene.take_dirty();
        let commands = encode_scene(&d.scene, &dirty);
        let clear = crate::ui_build::clear_color(&d.theme);
        let mut soft = next_gui_render_soft::SoftRenderer::new(d.win_w, d.win_h, clear);
        soft.render(&commands, &dirty);
        let mut gpu = next_gui_render_gpu::GpuRenderer::try_new(d.win_w, d.win_h, clear)
            .expect("Schema QA requires GPU");
        gpu.render(&commands, &dirty);
        let a = soft.frame_pixels().unwrap().0;
        let b = gpu.frame_pixels().unwrap().0;
        let delta = a
            .iter()
            .zip(b)
            .flat_map(|(a, b)| {
                [0, 8, 16].map(|s| (((a >> s) & 255) as i32 - ((b >> s) & 255) as i32).abs())
            })
            .max()
            .unwrap();
        assert!(delta <= 2, "{name}: full frame delta {delta}");
        for (backend, pixels) in [("soft", a), ("metal", b)] {
            crate::write_ppm(
                folder
                    .join(format!("{name}-{backend}.ppm"))
                    .to_str()
                    .unwrap(),
                pixels,
                d.win_w,
                d.win_h,
            )
            .unwrap();
        }
        eprintln!("Schema QA {name}: delta {delta}");
    }
}
