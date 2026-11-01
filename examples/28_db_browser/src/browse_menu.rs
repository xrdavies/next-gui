//! Browse cell context actions share the keyboard's typed clipboard paths.
use crate::{state::MainTab, Demo, KeyCode, Modifiers, WindowCtx};
use next_gui_ui::MenuItem;
use std::sync::Arc;

const FORMATS: u64 = 280;
const NULL: u64 = 281;
const CUT: u64 = 282;
const COPY: u64 = 283;
const HEADERS: u64 = 284;
const SQL: u64 = 285;
const PASTE: u64 = 286;
const FILTERS: u64 = 287;
const EXACT: u64 = 288;
const FIRST_FILTER: u64 = 289;
const FILTER_ACTIONS: [(&str, &str, &str); 9] = [
    ("Containing", "", ""), ("Not containing", "/^((?!", ").)*$/"),
    ("Not equal to", "<>", ""), ("Greater than", ">", ""), ("Less than", "<", ""),
    ("Greater or equal", ">=", ""), ("Less or equal", "<=", ""),
    ("Between this and…", "~", ""), ("Regular expression", "/", "/"),
];

fn filter_expression(value: &crate::db::CellValue, item: u64, escape: &str) -> Option<String> {
    use crate::db::CellValue;
    let mut text = match value {
        CellValue::Null => "NULL".into(), CellValue::Integer(value) => value.to_string(),
        CellValue::Real(value) => value.to_string(),
        CellValue::Text(bytes) => {
            let text = std::str::from_utf8(bytes).ok()?;
            if text.contains('\0') { return None; }
            if text.is_empty() { "''".into() } else { text.into() }
        }
        CellValue::Blob(_) => return None,
    };
    if item == EXACT {
        if matches!(value, CellValue::Text(bytes) if matches!(bytes.as_slice(), b"NULL" | b"''" | b"%" | b"%%")) {
            let mut literal = String::new();
            for ch in text.chars() {
                if "\\^$.|?*+()[]{}".contains(ch) { literal.push('\\'); }
                literal.push(ch);
            }
            return Some(format!("/^{literal}$/"));
        }
        return Some(format!("={text}"));
    }
    let (_, prefix, suffix) = FILTER_ACTIONS.get(item.checked_sub(FIRST_FILTER)? as usize)?;
    if prefix.is_empty() && text.starts_with(['<', '>', '=', '/']) { text.insert_str(0, escape); }
    Some(if *prefix == "~" { format!("{text}~") } else { format!("{prefix}{text}{suffix}") })
}

pub(crate) struct Target {
    path: String,
    tab: usize,
    schema: String,
    table: String,
    values: Arc<()>,
    selection: [Option<usize>; 4],
}


impl Target {
    fn read(demo: &Demo) -> Option<Self> {
        let tab = demo.state.browse();
        Some(Self { path: demo.state.path.clone(), tab: demo.state.browse_tab_idx,
            schema: tab.schema.clone(), table: tab.table.clone()?, values: tab.result.as_ref()?.values.identity(),
            selection: [tab.sel_row, tab.sel_col, tab.sel_row_start, tab.sel_col_start] })
    }

    fn matches(&self, demo: &Demo) -> bool {
        Self::read(demo).is_some_and(|other| self.path == other.path && self.tab == other.tab
            && self.schema == other.schema && self.table == other.table
            && Arc::ptr_eq(&self.values, &other.values) && self.selection == other.selection)
    }
}

impl Demo {
    fn browse_cell_menu_available(&self) -> bool {
        self.state.tab == MainTab::Browse && self.state.overlay.is_none() && !self.state.confirm_open
            && self.ui.active_dialog().is_none() && !self.state.sql_busy && self.state.conn.is_some()
            && self.state.guard_background_idle().is_ok() && !self.state.has_cell_draft()
            && self.state.external_cell.is_none() && !self.ui.ime_is_composing()
    }

    fn browse_cell_menu_items(&self) -> Vec<MenuItem> {
        let action = |id, label: &str, enabled| if enabled { MenuItem::new(id, label) } else { MenuItem::disabled(id, label) };
        let writable = !self.state.read_only && self.state.guard_browse_editable().is_ok();
        let cell = self.browse_menu_cell();
        let filters = FILTER_ACTIONS.iter().enumerate().map(|(index, (label, _, _))| {
            let item = FIRST_FILTER + index as u64;
            action(item, label, cell.as_ref().is_some_and(|cell| filter_expression(cell, item, &self.state.prefs.filter_escape).is_some()))
        }).collect();
        vec![action(EXACT, "Use as Exact Filter", cell.as_ref().is_some_and(|cell| filter_expression(cell, EXACT, &self.state.prefs.filter_escape).is_some())),
            MenuItem::submenu(FILTERS, "Use in Filter Expression", filters),
            MenuItem::new(FORMATS, "Edit Conditional Formats…"), MenuItem::separator(),
            action(NULL, "Set to NULL", writable), MenuItem::separator(), action(CUT, "Cut", writable),
            MenuItem::new(COPY, "Copy"), MenuItem::new(HEADERS, "Copy with Headers"),
            MenuItem::new(SQL, "Copy as SQL"), action(PASTE, "Paste", writable)]
    }

    fn browse_menu_cell(&self) -> Option<crate::db::CellValue> {
        let tab = self.state.browse();
        tab.result.as_ref()?.values.cell(tab.sel_row?, tab.sel_col?).ok()
    }

    fn open_selected_browse_cell_menu(&mut self, x: f32, y: f32) -> bool {
        if !self.browse_cell_menu_available() || self.state.browse().sel_row.is_none()
            || self.state.browse().sel_col.is_none() { return false; }
        self.cancel_browse_filter();
        self.browse_cell_context = Target::read(self);
        self.ui.set_focus(Some(self.ids.browse_table));
        self.ui.set_accessible_name(self.ids.context_menu, "Browse cell actions");
        self.ui.open_context_menu(self.ids.context_menu, self.browse_cell_menu_items(), x, y)
    }

    pub(crate) fn open_browse_cell_menu(&mut self, x: f32, y: f32) -> bool {
        if !self.browse_cell_menu_available() { return false; }
        let table = self.ids.browse_table;
        let Some((row, column)) = self.ui.table_cell_at(&self.scene, table, x, y) else { return false; };
        self.snapshot_inputs();
        let inside = self.ui.table_selected_rows(table).is_some_and(|(a, b)| (a..=b).contains(&row))
            && self.ui.table_selected_cols(table).is_some_and(|(a, b)| (a..=b).contains(&column));
        if !inside { self.ui.set_table_selected_cells(table, Some((row, row)), Some((column, column))); }
        self.ui.set_focus(Some(table));
        self.poll_selection_changes();
        if self.rebuild { self.do_rebuild(); }
        self.open_selected_browse_cell_menu(x, y)
    }

    pub(crate) fn handle_browse_cell_context_key(&mut self, key: KeyCode, modifiers: Modifiers) -> bool {
        if key != KeyCode::Function(10) || !modifiers.shift || modifiers.commandish() || modifiers.alt
            || self.ui.focused() != Some(self.ids.browse_table) || !self.browse_cell_menu_available() { return false; }
        self.snapshot_inputs(); self.poll_selection_changes();
        if self.rebuild { self.do_rebuild(); }
        let table = self.ids.browse_table;
        let bounds = self.scene.iter_paint_order().into_iter().find(|node| self.ui.widget_from_node(node.id) == Some(table)).map(|node| node.bounds);
        bounds.is_some_and(|bounds| self.open_selected_browse_cell_menu(bounds.x + 16.0, bounds.y + 32.0))
    }

    pub(crate) fn copy_browse_to_clipboard(&mut self, ctx: &mut WindowCtx<'_>, cut: bool, headers: bool, sql: bool) -> Result<(), String> {
        let copy = self.state.copy_browse_cells(headers, sql)?;
        if copy.html.is_empty() { ctx.clipboard_set_text(&copy.text) }
        else { ctx.clipboard_set_html(&copy.html, &copy.text) }.map_err(|e| e.to_string())?;
        self.cell_clipboard = Some(copy);
        if cut { self.state.cut_browse_cells()?; self.rebuild = true; }
        Ok(())
    }

    pub(crate) fn paste_browse_from_clipboard(&mut self, ctx: &mut WindowCtx<'_>, text: &str) -> Result<(), String> {
        let cells = ctx.clipboard_get_html().map_err(|e| e.to_string())?
            .and_then(|html| self.cell_clipboard.as_ref().and_then(|copy| copy.internal_cells(&html)));
        if let Some(cells) = cells { self.state.paste_browse_cells(cells)?; }
        else { self.state.paste_browse_tsv(text)?; }
        self.rebuild = true; Ok(())
    }

    fn browse_cell_action(&mut self, item: u64) -> Result<(), String> {
        if item == EXACT || (FIRST_FILTER..FIRST_FILTER + FILTER_ACTIONS.len() as u64).contains(&item) {
            let expression = self.browse_menu_cell().and_then(|value| filter_expression(&value, item, &self.state.prefs.filter_escape)).ok_or("This cell cannot be used in a text filter")?;
            let input = self.state.browse().sel_col.and_then(|column| self.ids.browse_col_filters.get(column)).copied().ok_or("Choose a loaded column filter")?;
            self.ui.set_focus(Some(input)); self.ui.key_input(crate::UiKey::SelectAll);
            self.ui.paste_into_focused_text(&expression); self.snapshot_inputs(); self.flush_browse_filter();
            return Ok(());
        }
        match item {
            FORMATS => {
                let column = self.state.browse().sel_col.and_then(|column| self.state.browse().result.as_ref()?.columns.get(column)).cloned().ok_or("Select a loaded column")?;
                let target = Target::read(self);
                self.state.begin_cond_formats_for_column(&column)?;
                self.browse_format_return = target;
                Ok(())
            }
            NULL => self.state.paste_browse_cells(vec![vec![crate::db::CellValue::Null]]),
            _ => Err("Unknown Browse cell action".into()),
        }
    }

    fn take_browse_cell_target(&mut self) -> Result<(), String> {
        let valid = self.browse_cell_menu_available() && self.browse_cell_context.as_ref().is_some_and(|target| target.matches(self));
        self.browse_cell_context = None; self.ui.close_popup();
        if valid { Ok(()) } else { Err("The Browse source or selection changed; reopen its context menu".into()) }
    }

    pub(crate) fn restore_browse_format_focus(&mut self) {
        if self.state.overlay.is_none() {
            if let Some(target) = self.browse_format_return.take() {
                if target.matches(self) && self.state.tab == MainTab::Browse {
                    self.ui.set_focus(Some(self.ids.browse_table)); self.ui.sync(&mut self.scene);
                }
            }
        }
    }

    pub(crate) fn handle_browse_cell_menu_action(&mut self, item: u64, ctx: &mut WindowCtx<'_>) -> bool {
        if !(FORMATS..=PASTE).contains(&item) && item != EXACT && !(FIRST_FILTER..FIRST_FILTER + FILTER_ACTIONS.len() as u64).contains(&item) { return false; }
        let result = self.take_browse_cell_target().and_then(|()| {
            self.ui.set_focus(Some(self.ids.browse_table));
            match item {
                COPY | CUT | HEADERS | SQL => self.copy_browse_to_clipboard(ctx, item == CUT, item == HEADERS, item == SQL),
                PASTE => match ctx.clipboard_get_text() {
                    Ok(Some(text)) => self.paste_browse_from_clipboard(ctx, &text),
                    Ok(None) => Ok(()), Err(error) => Err(error.to_string()),
                },
                _ => self.browse_cell_action(item),
            }
        });
        match result { Ok(()) => self.clear_error(), Err(error) => self.set_error(error) }
        self.rebuild = true; true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{browse_filter::tests::{capture, fixture}, state::OverlayKind, UiKey};
    use next_gui_ui::Theme;

    fn point(demo: &Demo, row: usize, column: usize) -> (f32, f32) {
        let table = demo.ids.browse_table;
        let bounds = demo.scene.iter_paint_order().into_iter().find(|node| demo.ui.widget_from_node(node.id) == Some(table)).unwrap().bounds;
        for y in (bounds.y as i32..(bounds.y + bounds.height) as i32).step_by(4) {
            for x in (bounds.x as i32..(bounds.x + bounds.width) as i32).step_by(4) {
                if demo.ui.table_cell_at(&demo.scene, table, x as f32, y as f32) == Some((row, column)) {
                    return (x as f32, y as f32);
                }
            }
        }
        panic!("cell {row},{column} is not visible");
    }

    fn choose(demo: &mut Demo, item: u64) {
        let items = demo.browse_cell_menu_items();
        assert!(items.iter().any(|action| action.id == item && action.enabled));
        demo.ui.key_input(UiKey::Home);
        for _ in items.iter().take_while(|action| action.id != item).filter(|action| action.enabled && action.id != 0) { demo.ui.key_input(UiKey::Down); }
        demo.ui.key_input(UiKey::Enter);
        assert_eq!(demo.ui.take_menu_action(), Some((demo.ids.context_menu, item)));
        demo.take_browse_cell_target().unwrap();
        demo.browse_cell_action(item).unwrap(); demo.rebuild = true; demo.do_rebuild();
    }

    #[test]
    fn cell_filter_expressions_keep_typed_null_literals_and_original_operator_forms() {
        use crate::db::CellValue;
        assert_eq!(filter_expression(&CellValue::Integer(42), EXACT, "\\").as_deref(), Some("=42"));
        for (value, expected) in [(CellValue::Null, "=NULL"),
            (CellValue::Text(Vec::new()), "=''"), (CellValue::Text(b"NULL".to_vec()), "/^NULL$/"),
            (CellValue::Text(b"''".to_vec()), "/^''$/"), (CellValue::Text(b"%".to_vec()), "/^%$/"),
            (CellValue::Text(b"%%".to_vec()), "/^%%$/"), (CellValue::Text(b"O'Reilly".to_vec()), "=O'Reilly")] {
            assert_eq!(filter_expression(&value, EXACT, "\\").as_deref(), Some(expected));
            let conn = crate::db::open_path(":memory:").unwrap();
            let condition = crate::filters::condition(expected, "\\").unwrap();
            assert!(conn.query_row(&format!("SELECT ?1 {condition}"), [&value], |row| row.get::<_, bool>(0)).unwrap(), "{expected}");
            if matches!(value, CellValue::Text(_)) && expected.starts_with('/') {
                assert!(!conn.query_row(&format!("SELECT NULL {condition}"), [], |row| row.get::<_, bool>(0)).unwrap());
            }
        }
        let text = CellValue::Text(b"3".to_vec());
        for (index, expected) in ["3", "/^((?!3).)*$/", "<>3", ">3", "<3", ">=3", "<=3", "3~", "/3/"].into_iter().enumerate() {
            assert_eq!(filter_expression(&text, FIRST_FILTER + index as u64, "\\").as_deref(), Some(expected));
        }
        assert_eq!(filter_expression(&CellValue::Text(b"=data".to_vec()), FIRST_FILTER, "!").as_deref(), Some("!=data"));
        for value in [CellValue::Blob(b"text".to_vec()), CellValue::Text(vec![0xFF]), CellValue::Text(b"a\0b".to_vec())] {
            for item in [EXACT, FIRST_FILTER, FIRST_FILTER + 8] { assert!(filter_expression(&value, item, "\\").is_none()); }
        }
    }

    #[test]
    fn cell_exact_filter_uses_background_query_and_retains_filter_undo() {
        let mut demo = fixture(1400, 860, Theme::light()); demo.state.write_changes().unwrap();
        let (x, y) = point(&demo, 2, 1); assert!(demo.open_browse_cell_menu(x, y));
        choose(&mut demo, EXACT);
        assert_eq!(demo.state.browse().col_filters[1], "=3");
        let now = std::time::Instant::now(); demo.advance_browse_filter(now);
        crate::browse_filter::tests::settle(&mut demo, now);
        assert_eq!(demo.state.browse().result.as_ref().unwrap().rows, [vec!["beta", "3"]]);
        let input = demo.ids.browse_col_filters[1]; assert_eq!(demo.ui.focused(), Some(input));
        demo.ui.key_input(UiKey::Undo); assert_eq!(demo.ui.text_input_value(input), Some(""));
        demo.ui.key_input(UiKey::Redo); assert_eq!(demo.ui.text_input_value(input), Some("=3"));
        assert!(!demo.state.dirty);
    }

    #[test]
    fn cell_filter_submenu_applies_to_the_active_column_and_rejects_binary_values() {
        let mut demo = fixture(1400, 860, Theme::light()); demo.state.write_changes().unwrap();
        let (x, y) = point(&demo, 2, 1); assert!(demo.open_browse_cell_menu(x, y));
        demo.ui.key_input(UiKey::Home); demo.ui.key_input(UiKey::Down); demo.ui.key_input(UiKey::Right);
        capture(&mut demo, "cell-context-filter-submenu");
        for _ in 0..3 { demo.ui.key_input(UiKey::Down); }
        demo.ui.key_input(UiKey::Enter);
        assert_eq!(demo.ui.take_menu_action(), Some((demo.ids.context_menu, FIRST_FILTER + 3)));
        demo.take_browse_cell_target().unwrap(); demo.browse_cell_action(FIRST_FILTER + 3).unwrap();
        demo.rebuild = true; demo.do_rebuild();
        let now = std::time::Instant::now(); demo.advance_browse_filter(now); crate::browse_filter::tests::settle(&mut demo, now);
        assert_eq!(demo.state.browse().col_filters[1], ">3");
        assert_eq!(demo.state.browse().result.as_ref().unwrap().rows, [vec!["A_B", "4"], vec!["A%B", "5"]]);
        assert!(demo.state.browse().col_filters[0].is_empty()); assert!(!demo.state.dirty);
        demo.state.browse_mut().col_filters[1].clear();
        demo.state.conn.as_ref().unwrap().execute("UPDATE names SET name=x'4E554C4C' WHERE rowid=1", []).unwrap();
        demo.state.refresh_browse().unwrap(); demo.do_rebuild();
        let (x, y) = point(&demo, 0, 0); assert!(demo.open_browse_cell_menu(x, y));
        let items = demo.browse_cell_menu_items();
        assert!(items.iter().any(|item| item.id == EXACT && !item.enabled));
        let filters = items.iter().find(|item| item.id == FILTERS).unwrap();
        assert!(filters.children.iter().all(|item| !item.enabled));
    }

    #[test]
    fn browse_cell_context_preserves_ranges_and_returns_from_scoped_formats() {
        for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400, 860), (800, 600)] {
                let mut demo = fixture(width, height, theme); demo.state.write_changes().unwrap();
                demo.ui.set_table_selected_cells(demo.ids.browse_table, Some((0, 2)), Some((0, 1)));
                demo.poll_selection_changes(); demo.do_rebuild();
                let selection = Target::read(&demo).unwrap().selection;
                let (x, y) = point(&demo, 0, 0);
                assert!(demo.open_browse_cell_menu(x, y));
                assert_eq!(Target::read(&demo).unwrap().selection, selection);
                assert_eq!(demo.state.copy_browse_cells(false, false).unwrap().text, "alpha\t1\nalpine\t2\nbeta\t3");
                capture(&mut demo, &format!("cell-context-menu-{name}-{width}"));
                choose(&mut demo, FORMATS);
                assert_eq!(demo.state.overlay, Some(OverlayKind::CondFormat));
                assert_eq!(demo.state.cond_formats.as_ref().unwrap().column_scope.as_deref(), Some("amount"));
                capture(&mut demo, &format!("cell-context-formats-{name}-{width}"));
                demo.ui.key_input(UiKey::Escape); demo.handle_cond_format_dismiss(); demo.do_rebuild();
                assert!(demo.state.overlay.is_none());
                assert_eq!(demo.ui.focused(), Some(demo.ids.browse_table));
                assert_eq!(Target::read(&demo).unwrap().selection, selection);
                assert!(!demo.state.dirty);
                assert!(demo.handle_browse_cell_context_key(KeyCode::Function(10), Modifiers { shift: true, ..Default::default() }));
                demo.ui.close_popup();
                let (x, y) = point(&demo, 4, 0); assert!(demo.open_browse_cell_menu(x, y));
                assert_eq!(Target::read(&demo).unwrap().selection, [Some(4), Some(0), Some(4), Some(0)]);
                choose(&mut demo, NULL);
                assert!(demo.state.dirty);
                assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT name IS NULL, amount FROM names WHERE rowid=5", [], |row| Ok((row.get::<_, bool>(0)?, row.get::<_, i64>(1)?))).unwrap(), (true, 5));
                capture(&mut demo, &format!("cell-context-null-{name}-{width}"));
                demo.state.revert_changes().unwrap();
                assert_eq!(demo.state.conn.as_ref().unwrap().query_row("SELECT name FROM names WHERE rowid=5", [], |row| row.get::<_, String>(0)).unwrap(), "A%B");
            }
        }
    }

    #[test]
    fn browse_cell_context_rejects_changed_snapshots_selections_and_busy_sources() {
        let mut demo = fixture(1400, 860, Theme::dark()); demo.state.write_changes().unwrap();
        let open = |demo: &mut Demo| { let (x, y) = point(demo, 0, 0); assert!(demo.open_browse_cell_menu(x, y)); };
        open(&mut demo); demo.state.refresh_browse().unwrap();
        assert!(demo.take_browse_cell_target().unwrap_err().contains("source or selection changed"));
        open(&mut demo); demo.state.browse_mut().sel_col = Some(1); assert!(demo.take_browse_cell_target().is_err());
        demo.do_rebuild(); open(&mut demo); demo.state.sql_busy = true; assert!(demo.take_browse_cell_target().is_err()); demo.state.sql_busy = false;
        demo.state.read_only = true; demo.do_rebuild(); open(&mut demo);
        let items = demo.browse_cell_menu_items();
        for id in [NULL, CUT, PASTE] { assert!(items.iter().any(|item| item.id == id && !item.enabled)); }
        for id in [COPY, HEADERS, SQL, FORMATS] { assert!(items.iter().any(|item| item.id == id && item.enabled)); }
        choose(&mut demo, FORMATS); demo.handle_button(demo.ids.ov_btn_cancel); demo.do_rebuild();
        assert!(!demo.state.dirty);
        demo.state.read_only = false; demo.state.sync_edit_cell_from_selection();
        demo.state.set_edit_mode(crate::state::EditCellMode::Text).unwrap(); demo.state.edit_value = "unsaved".into();
        assert!(demo.state.has_cell_draft());
        let (x, y) = point(&demo, 0, 0); assert!(!demo.open_browse_cell_menu(x, y));
        demo.state.discard_cell_draft(); demo.state.tab = MainTab::ExecuteSql;
        assert!(!demo.open_browse_cell_menu(x, y));
    }
}
