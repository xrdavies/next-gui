//! Source-bound, transactional editing of the existing conditional-format rules.
use crate::{
    db,
    state::{AppState, CondFormatRule, OverlayKind},
    Demo, WidgetId,
};
use next_gui_ui::{SelectState, TextInputState};

pub(crate) const OPERATORS: [&str; 6] = [
    "Equal to",
    "Not equal to",
    "Contains",
    "Greater than",
    "Less than",
    "Filter expression",
];

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Rule {
    id: usize,
    pub column: String,
    pub op: usize,
    pub value: String,
    pub color: String,
    pub style: crate::cond_style::Draft,
    native_format: Option<crate::state::NativeCondFormat>,
}

#[derive(Clone, Debug)]
pub(crate) struct Draft {
    pub schema: String,
    pub table: String,
    pub columns: Vec<String>,
    pub rules: Vec<Rule>,
    pub selected: Option<usize>,
    pub selected_column: Option<usize>,
    next_id: usize,
    original: Vec<CondFormatRule>,
    pub scroll: (f32, f32),
}

impl Draft {
    pub(crate) fn selected_id(&self) -> Option<usize> {
        self.selected
            .and_then(|row| self.rules.get(row))
            .map(|rule| rule.id)
    }
    pub(crate) fn rows(&self) -> Vec<Vec<String>> {
        self.rules
            .iter()
            .map(|rule| {
                vec![
                    rule.column.clone(),
                    format!(
                        "{} {}",
                        OPERATORS.get(rule.op).unwrap_or(&"Unknown operator"),
                        rule.value
                    ),
                    if rule.style.foreground.is_empty() { "Default".into() } else { rule.style.foreground.clone() },
                    rule.color.clone(),
                    if rule.style.family.is_empty() { "Default".into() } else { rule.style.family.clone() },
                    if rule.style.size.is_empty() { "Default".into() } else { rule.style.size.clone() },
                    rule.style.bold.to_string(),
                    rule.style.italic.to_string(),
                    rule.style.underline.to_string(),
                    crate::cond_style::ALIGNMENTS.get(rule.style.alignment).unwrap_or(&"Unknown").to_string(),
                ]
            })
            .collect()
    }
    pub(crate) fn checks(&self) -> Vec<Vec<Option<bool>>> {
        self.rules.iter().map(|rule| {
            let mut cells = vec![None; 10];
            cells[6] = Some(rule.style.bold);
            cells[7] = Some(rule.style.italic);
            cells[8] = Some(rule.style.underline);
            cells
        }).collect()
    }
    fn same_source(&self, state: &AppState) -> bool {
        self.schema == state.browse().schema && Some(&self.table) == state.browse().table.as_ref()
    }

    pub(crate) fn previews(&self, font: &next_gui_text::Font) -> Vec<Vec<Option<next_gui_ui::TableCellStyle>>> {
        self.rules.iter().map(|rule| {
            let style=rule.style.validate().ok().zip(u32::from_str_radix(rule.color.trim_start_matches('#'),16).ok());
            vec![None,style.map(|(style,color)|style.cell(color,font))]
        }).collect()
    }
}

impl AppState {
    pub(crate) fn begin_cond_formats(&mut self) -> Result<(), String> {
        self.guard_background_idle()?;
        if self.sql_busy {
            return Err("Wait for SQL to finish before editing formats".into());
        }
        let table = self.browse().table.clone().ok_or("Select a table first")?;
        let schema = self.browse().schema.clone();
        let columns = self
            .browse()
            .result
            .as_ref()
            .ok_or("Load the table before editing formats")?
            .columns
            .clone();
        let rules: Vec<_> = self
            .cond_rules
            .iter()
            .filter(|rule| rule.schema == schema && rule.table.as_ref() == Some(&table))
            .enumerate()
            .map(|(id, rule)| Rule {
                id,
                column: rule.column.clone(),
                op: rule.op,
                value: rule.value.clone(),
                color: format!("#{:06X}", rule.color_rgb),
                style: (&rule.style).into(),
                native_format: rule.native_format.clone(),
            })
            .collect();
        let selected = rules
            .iter()
            .position(|rule| rule.column == self.edit_col)
            .or_else(|| (!rules.is_empty()).then_some(0));
        let next_id = rules.len();
        self.cond_formats = Some(Draft {
            schema,
            table,
            columns,
            rules,
            selected,
            selected_column: None,
            next_id,
            original: self.cond_rules.clone(),
            scroll: (0.0, 0.0),
        });
        self.load_cond_format_fields();
        self.wizard_scroll = (0.0, 0.0);
        self.overlay = Some(OverlayKind::CondFormat);
        Ok(())
    }

    fn load_cond_format_fields(&mut self) {
        let Some(draft) = &self.cond_formats else {
            return;
        };
        if let Some(rule) = draft.selected.and_then(|row| draft.rules.get(row)) {
            self.cond_col = rule.column.clone();
            self.cond_op_idx = rule.op;
            self.cond_value = rule.value.clone();
            self.cond_color = rule.color.clone();
            self.cond_style = rule.style.clone();
        } else {
            self.cond_col = if draft.columns.contains(&self.edit_col) {
                self.edit_col.clone()
            } else {
                draft.columns.first().cloned().unwrap_or_default()
            };
            self.cond_op_idx = 0;
            self.cond_value.clear();
            self.cond_color = "#E67E22".into();
            self.cond_style = Default::default();
        }
    }

    pub(crate) fn snapshot_cond_format_fields(&mut self) -> bool {
        let Some(draft) = &mut self.cond_formats else {
            return false;
        };
        let Some(rule) = draft.selected.and_then(|row| draft.rules.get_mut(row)) else {
            return false;
        };
        let changed = rule.column != self.cond_col
            || rule.op != self.cond_op_idx
            || rule.value != self.cond_value
            || rule.color != self.cond_color
            || rule.style != self.cond_style;
        if changed {
            rule.column = self.cond_col.clone();
            rule.op = self.cond_op_idx;
            rule.value = self.cond_value.clone();
            rule.color = self.cond_color.clone();
            rule.style = self.cond_style.clone();
        }
        changed
    }

    pub(crate) fn save_cond_formats(&mut self) -> Result<(), String> {
        self.guard_background_idle()?;
        if self.sql_busy {
            return Err("Wait for SQL to finish before saving formats".into());
        }
        self.snapshot_cond_format_fields();
        let draft = self
            .cond_formats
            .as_ref()
            .ok_or("Open the format manager first")?;
        if !draft.same_source(self) || self.cond_rules != draft.original {
            return Err("The source or formats changed. Reopen the format manager".into());
        }
        let current = self
            .browse()
            .result
            .as_ref()
            .ok_or("The source is no longer loaded")?;
        let rules = draft
            .rules
            .iter()
            .enumerate()
            .map(|(row, rule)| {
                let error = |message: &str| format!("Rule {}: {message}", row + 1);
                if !current.columns.contains(&rule.column) {
                    return Err(error("choose an existing column"));
                }
                if rule.op >= OPERATORS.len() {
                    return Err(error("choose a supported operator"));
                }
                if matches!(rule.op, 3 | 4)
                    && !rule
                        .value
                        .trim()
                        .parse::<f64>()
                        .is_ok_and(|value| value.is_finite())
                {
                    return Err(error("comparison value must be a finite number"));
                }
                if rule.op == 5 {
                    crate::cond_match::validate(
                        self.conn.as_ref().ok_or("No idle database available")?,
                        &rule.value,
                        &self.prefs.filter_escape,
                    )
                    .map_err(|message| error(&message))?;
                }
                let color = rule.color.strip_prefix('#').unwrap_or(&rule.color);
                if color.len() != 6 || !color.bytes().all(|ch| ch.is_ascii_hexdigit()) {
                    return Err(error("background color must be #RRGGBB"));
                }
                Ok(CondFormatRule {
                    schema: draft.schema.clone(),
                    table: Some(draft.table.clone()),
                    column: rule.column.clone(),
                    op: rule.op,
                    value: if matches!(rule.op, 3 | 4) {
                        rule.value.trim().to_string()
                    } else {
                        rule.value.clone()
                    },
                    color_rgb: u32::from_str_radix(color, 16).unwrap(),
                    style: rule.style.validate().map_err(|message| error(&message))?,
                    native_format: rule.native_format.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let (schema, table) = (draft.schema.clone(), draft.table.clone());
        self.cond_rules
            .retain(|rule| rule.schema != schema || rule.table.as_ref() != Some(&table));
        self.cond_rules.extend(rules);
        self.cond_formats = None;
        self.overlay = None;
        self.status_msg = format!(
            "Saved conditional formats for {}",
            db::qualified_name(&schema, &table)
        );
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Controls {
    pub row: Option<usize>,
    pub table: WidgetId,
    pub add: WidgetId,
    pub remove: WidgetId,
    pub up: WidgetId,
    pub down: WidgetId,
    pub foreground: WidgetId,
    pub family: WidgetId,
    pub size: WidgetId,
    pub bold: WidgetId,
    pub italic: WidgetId,
    pub underline: WidgetId,
    pub alignment: WidgetId,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            row: None,
            table: WidgetId(0),
            add: WidgetId(0),
            remove: WidgetId(0),
            up: WidgetId(0),
            down: WidgetId(0),
            foreground: WidgetId(0), family: WidgetId(0), size: WidgetId(0), bold: WidgetId(0),
            italic: WidgetId(0), underline: WidgetId(0), alignment: WidgetId(0),
        }
    }
}

fn fields(demo: &Demo) -> [WidgetId; 16] {
    let c = demo.ids.cond_formats;
    [
        c.table,
        c.add,
        c.remove,
        c.up,
        c.down,
        demo.ids.ov_cond_col,
        demo.ids.ov_cond_op,
        demo.ids.ov_cond_val,
        demo.ids.ov_cond_color,
        c.foreground,c.family,c.size,c.bold,c.italic,c.underline,c.alignment,
    ]
}

pub(crate) struct ViewSession {
    source: (String, String),
    row: Option<usize>,
    focus: usize,
    input: Option<TextInputState>,
    select: Option<SelectState>,
    value: Option<String>,
}
impl ViewSession {
    pub(crate) fn capture(demo: &Demo) -> Option<Self> {
        if demo.state.overlay != Some(OverlayKind::CondFormat) {
            return None;
        }
        let draft = demo.state.cond_formats.as_ref()?;
        let focus = fields(demo)
            .iter()
            .position(|id| id.0 != 0 && demo.ui.focused() == Some(*id))?;
        let row = demo.ids.cond_formats.row;
        if focus > 4 && row != draft.selected_id() {
            return None;
        }
        let id = fields(demo)[focus];
        Some(Self {
            source: (draft.schema.clone(), draft.table.clone()),
            row,
            focus,
            input: demo.ui.text_input_state(id),
            select: demo.ui.select_state(id),
            value: demo.ui.text_input_value(id).map(str::to_owned),
        })
    }
    pub(crate) fn restore(self, demo: &mut Demo) {
        let Some(draft) = demo
            .state
            .cond_formats
            .as_ref()
            .filter(|_| demo.state.overlay == Some(OverlayKind::CondFormat))
        else {
            return;
        };
        if self.source != (draft.schema.clone(), draft.table.clone())
            || self.focus > 4 && self.row != draft.selected_id()
        {
            return;
        }
        let id = fields(demo)[self.focus];
        if let Some(input) = self.input {
            if demo.ui.text_input_value(id) != self.value.as_deref() {
                return;
            }
            demo.ui.restore_text_input_state(id, &input);
        }
        if let Some(select) = self.select {
            demo.ui.restore_select_state(id, &select);
        }
        demo.ui.set_focus(Some(id));
    }
}

impl Demo {
    pub(crate) fn snapshot_cond_formats(&mut self) {
        if self.state.overlay != Some(OverlayKind::CondFormat) {
            return;
        }
        let changed = self.state.snapshot_cond_format_fields();
        let Some(draft) = &mut self.state.cond_formats else {
            return;
        };
        draft.selected_column = self.ui.table_selected_col(self.ids.cond_formats.table);
        if let Some(scroll) = self.ui.table_scroll_offset(self.ids.cond_formats.table) {
            draft.scroll = scroll;
        }
        if !changed {
            return;
        }
        let rows = draft.rows();
        let selected = self.ui.table_selected_row(self.ids.cond_formats.table);
        self.ui.set_table_rows(self.ids.cond_formats.table, rows);
        self.ui.set_table_check_cells(self.ids.cond_formats.table, draft.checks());
        let previews=draft.previews(self.ui.font());
        self.ui.set_table_cell_styles(self.ids.cond_formats.table,previews);
        self.ui
            .set_table_selected_row(self.ids.cond_formats.table, selected);
        self.ui.set_table_selected_col(self.ids.cond_formats.table, draft.selected_column);
        self.ui.set_table_scroll_offset(
            self.ids.cond_formats.table,
            draft.scroll.0,
            draft.scroll.1,
        );
    }

    pub(crate) fn handle_cond_format_check(
        &mut self, table: WidgetId, row: usize, column: usize, checked: bool,
    ) -> bool {
        if table.0 == 0 || table != self.ids.cond_formats.table
            || self.state.overlay != Some(OverlayKind::CondFormat) {
            return false;
        }
        if !(6..=8).contains(&column)
            || self.state.cond_formats.as_ref().is_none_or(|draft| row >= draft.rules.len()) {
            return true;
        }
        // Flush the old row's controls before a checkbox selects a different rule.
        self.snapshot_inputs();
        let draft = self.state.cond_formats.as_mut().unwrap();
        let style = &mut draft.rules[row].style;
        match column {
            6 => style.bold = checked,
            7 => style.italic = checked,
            8 => style.underline = checked,
            _ => unreachable!(),
        }
        draft.selected = Some(row);
        draft.selected_column = Some(column);
        self.state.load_cond_format_fields();
        self.do_rebuild();
        true
    }

    pub(crate) fn poll_cond_formats(&mut self) -> bool {
        if self.state.overlay != Some(OverlayKind::CondFormat) {
            return false;
        }
        let Some(draft) = &mut self.state.cond_formats else {
            return false;
        };
        let selected = self
            .ui
            .table_selected_row(self.ids.cond_formats.table)
            .filter(|row| *row < draft.rules.len());
        if selected != draft.selected {
            draft.selected = selected;
            self.state.load_cond_format_fields();
            self.rebuild = true;
            return true;
        }
        false
    }

    pub(crate) fn handle_cond_format_button(&mut self, id: WidgetId) -> bool {
        let controls = self.ids.cond_formats;
        if id.0 == 0
            || self.state.overlay != Some(OverlayKind::CondFormat)
            || ![controls.add, controls.remove, controls.up, controls.down].contains(&id)
        {
            return false;
        }
        if self.ui.button_enabled(id) == Some(false) {
            return true;
        }
        self.snapshot_inputs();
        let Some(draft) = &mut self.state.cond_formats else {
            return true;
        };
        if id == controls.add {
            draft.rules.push(Rule {
                id: draft.next_id,
                column: self.state.cond_col.clone(),
                op: 0,
                value: String::new(),
                color: "#E67E22".into(),
                style: Default::default(),
                native_format: None,
            });
            draft.next_id += 1;
            draft.selected = draft.rules.len().checked_sub(1);
        } else if let Some(row) = draft.selected.filter(|row| *row < draft.rules.len()) {
            if id == controls.remove {
                draft.rules.remove(row);
                draft.selected = (!draft.rules.is_empty())
                    .then_some(row.min(draft.rules.len().saturating_sub(1)));
            } else {
                let target = if id == controls.up {
                    row.checked_sub(1)
                } else {
                    row.checked_add(1)
                };
                if let Some(target) = target.filter(|target| *target < draft.rules.len()) {
                    draft.rules.swap(row, target);
                    draft.selected = Some(target);
                }
            }
        }
        self.state.load_cond_format_fields();
        self.rebuild = true;
        true
    }
}

pub(crate) fn prepare_qa(state: &mut AppState) {
    state.close_db();
    state.new_memory_db().unwrap();
    state.conn.as_ref().unwrap().execute_batch("CREATE TABLE names(name TEXT,amount INTEGER); INSERT INTO names VALUES('alpha',1),('alpine',2),('beta',3),('A_B',4),('A%B',5);").unwrap();
    state.dirty = true;
    state.refresh_schema().unwrap();
    state.switch_browse_table_in("main", "names").unwrap();
    state.cond_rules = [
        ("amount", 3, "2", 0xBF5266),
        ("amount", 4, "5", 0x416FA8),
        ("name", 2, "alp", 0x288976),
    ]
    .into_iter()
    .map(|(column, op, value, color_rgb)| CondFormatRule {
        schema: "main".into(),
        table: Some("names".into()),
        column: column.into(),
        op,
        value: value.into(),
        color_rgb,
        style: Default::default(),
        native_format: None,
    })
    .collect();
    state.edit_col = "amount".into();
    state.begin_cond_formats().unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browse_filter::tests::{capture, fixture};
    use next_gui_ui::{Theme, UiKey};

    fn manager(width: u32, height: u32, theme: Theme) -> Demo {
        let mut demo = fixture(width, height, theme);
        prepare_qa(&mut demo.state);
        demo.do_rebuild();
        demo
    }
    fn bounds(demo: &Demo, id: WidgetId) -> next_gui_scene::Rect {
        demo.scene
            .iter_paint_order()
            .into_iter()
            .find(|node| demo.ui.widget_from_node(node.id) == Some(id) && !node.bounds.is_empty())
            .unwrap()
            .bounds
    }
    fn click(id: WidgetId, demo: &mut Demo) {
        demo.ui.sync(&mut demo.scene);
        let rect = bounds(demo, id);
        let (x, y) = (rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
        assert_eq!(demo.ui.hit_widget(&demo.scene, x, y), Some(id));
        assert_eq!(demo.ui.button_enabled(id), Some(true));
        assert!(!demo.ui.pointer_pressed(&mut demo.scene, x, y));
        assert_eq!(demo.ui.focused(), Some(id));
        demo.ui.pointer_released();
        demo.handle_button(id);
        demo.do_rebuild();
    }

    #[test]
    fn save_is_atomic_qualified_and_does_not_touch_database_transactions() {
        let mut demo = manager(1400, 860, Theme::dark());
        let state = &mut demo.state;
        state.conn.as_ref().unwrap().execute_batch("CREATE TEMP TABLE names(name,amount); ATTACH ':memory:' AS \"aux space\"; CREATE TABLE \"aux space\".names(name,amount); INSERT INTO main.names VALUES('pending',6);").unwrap();
        state.refresh_schema().unwrap();
        let original = state.cond_rules.clone();
        for schema in ["temp", "aux space"] {
            let mut rule = original[0].clone();
            rule.schema = schema.into();
            state.cond_rules.push(rule);
        }
        state.begin_cond_formats().unwrap();
        state.cond_value = " 3 ".into();
        state.cond_color = "aBcD09".into();
        state.read_only = true;
        state.save_cond_formats().unwrap();
        assert!(state.dirty);
        assert!(!state.conn.as_ref().unwrap().is_autocommit());
        assert_eq!(
            state
                .conn
                .as_ref()
                .unwrap()
                .query_row(
                    "SELECT amount FROM main.names WHERE name='pending'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            6
        );
        assert_eq!(
            state
                .cond_rules
                .iter()
                .filter(|r| r.schema != "main")
                .count(),
            2
        );
        let changed = state
            .cond_rules
            .iter()
            .find(|r| r.schema == "main")
            .unwrap();
        assert_eq!((changed.value.as_str(), changed.color_rgb), ("3", 0xABCD09));
        assert!(state.cond_formats.is_none());
        assert!(state.overlay.is_none());
        let project = crate::project::ProjectFile::from_state(state);
        let json = serde_json::to_string(&project).unwrap();
        let loaded: crate::project::ProjectFile = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.cond_rules, state.cond_rules);
        assert!(!json.contains("next_id"));
        for schema in ["temp", "aux space"] {
            state.switch_browse_table_in(schema, "names").unwrap();
            state.begin_cond_formats().unwrap();
            assert_eq!(state.cond_formats.as_ref().unwrap().rules.len(), 1);
            state.cond_value = "7".into();
            state.save_cond_formats().unwrap();
        }
        assert_eq!(
            state
                .cond_rules
                .iter()
                .find(|r| r.schema == "main")
                .unwrap()
                .value,
            "3"
        );
    }

    #[test]
    fn invalid_and_stale_drafts_preserve_committed_rules_and_pending_inputs() {
        let mut demo = manager(1400, 860, Theme::dark());
        let state = &mut demo.state;
        let original = state.cond_rules.clone();
        for color in ["#fff", "#1234567", "#GG0000", "💚abcd"] {
            state.cond_color = color.into();
            assert!(state.save_cond_formats().unwrap_err().contains("#RRGGBB"));
            assert_eq!(state.cond_color, color);
            assert_eq!(state.cond_rules, original);
        }
        state.cond_color = "#123456".into();
        for number in ["NaN", "inf", "1e999", "not numeric"] {
            state.cond_value = number.into();
            assert!(state
                .save_cond_formats()
                .unwrap_err()
                .contains("finite number"));
            assert_eq!(state.cond_rules, original);
        }
        state.cond_value = "2".into();
        state.cond_col = "missing".into();
        assert!(state
            .save_cond_formats()
            .unwrap_err()
            .contains("existing column"));
        state.cond_col = "amount".into();
        state.cond_op_idx = 99;
        assert!(state
            .save_cond_formats()
            .unwrap_err()
            .contains("supported operator"));
        assert_eq!(state.cond_rules, original);
        state.begin_cond_formats().unwrap();
        state.sql_busy = true;
        assert!(state.save_cond_formats().unwrap_err().contains("SQL"));
        state.sql_busy = false;
        state.cond_rules[0].value = "external change".into();
        assert!(state.save_cond_formats().unwrap_err().contains("changed"));
        assert_eq!(state.cond_rules[0].value, "external change");
        state.begin_cond_formats().unwrap();
        state.browse_mut().schema = "temp".into();
        assert!(state.save_cond_formats().unwrap_err().contains("changed"));
        state.close_db();
        assert!(state.cond_formats.is_none());
        assert!(state.overlay.is_none());
    }

    #[test]
    fn conditional_style_grid_checks_keep_rule_identity_and_save_cancel() {
        for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400, 860), (800, 600)] {
                let mut demo = fixture(width, height, theme);
                crate::cond_style::prepare_qa(&mut demo.state);
                if width == 800 { demo.state.prefs.font_size = 18.0; }
                demo.do_rebuild();
                let original = demo.state.cond_rules.clone();
                let table = demo.ids.cond_formats.table;
                for column in 6..=8 {
                    assert_eq!(demo.ui.table_cell_checked(table, 0, column), Some(true));
                }
                capture(&mut demo, &format!("checks-manager-{name}-{width}"));

                // A click on another rule must flush edits into the previous rule only.
                demo.ui.set_text_input(demo.ids.ov_cond_val, ">=2");
                demo.ui.set_table_selected_col(table, Some(6));
                demo.ui.reveal_table_selection(table);
                demo.ui.sync(&mut demo.scene);
                let grid = bounds(&demo, table);
                let style = next_gui_ui::TableStyle::from_theme(&demo.theme);
                let x = grid.x + style.padding + 810.0 + 16.0
                    - demo.ui.table_scroll_offset(table).unwrap().0;
                let y = grid.y + style.header_height + style.row_height * 1.5;
                assert_eq!(demo.ui.hit_widget(&demo.scene, x, y), Some(table));
                assert!(demo.ui.pointer_pressed(&mut demo.scene, x, y));
                demo.ui.pointer_released();
                demo.snapshot_inputs();
                demo.handle_field_check_change();
                demo.snapshot_inputs();
                let draft = demo.state.cond_formats.as_ref().unwrap();
                assert_eq!(draft.rules[0].value, ">=2");
                assert_eq!(draft.rules[1].value, "5");
                assert!(draft.rules[1].style.bold);
                assert_eq!(draft.selected, Some(1));
                assert_eq!(demo.ui.is_checked(demo.ids.cond_formats.bold), Some(true));
                assert_eq!(demo.ui.table_selected_col(demo.ids.cond_formats.table), Some(6));
                assert_eq!(demo.ui.focused(), Some(demo.ids.cond_formats.table));
                for column in 7..=8 {
                    assert!(demo.ui.key_input(UiKey::Right));
                    assert_eq!(demo.ui.table_selected_col(demo.ids.cond_formats.table), Some(column));
                    assert!(demo.ui.key_input(UiKey::Space));
                    demo.handle_field_check_change();
                    demo.snapshot_inputs();
                    assert_eq!(demo.ui.table_cell_checked(demo.ids.cond_formats.table, 1, column), Some(true));
                }
                capture(&mut demo, &format!("checks-pointer-{name}-{width}"));
                click(demo.ids.cond_formats.up, &mut demo);
                assert_eq!(demo.state.cond_formats.as_ref().unwrap().rules[0].op, 4);
                for column in 6..=8 {
                    assert_eq!(demo.ui.table_cell_checked(demo.ids.cond_formats.table, 0, column), Some(true));
                }
                capture(&mut demo, &format!("checks-reordered-{name}-{width}"));
                click(demo.ids.ov_btn_cancel, &mut demo);
                assert_eq!(demo.state.cond_rules, original);

                demo.state.begin_cond_formats().unwrap();
                demo.do_rebuild();
                demo.ui.set_focus(Some(demo.ids.ov_cond_val));
                for _ in 0..3 { demo.ui.key_input(UiKey::BackTab); }
                assert_eq!(demo.ui.focused(), Some(demo.ids.cond_formats.table));
                for _ in 0..7 { demo.ui.key_input(UiKey::Right); }
                for column in 6..=8 {
                    assert_eq!(demo.ui.table_selected_col(demo.ids.cond_formats.table), Some(column));
                    assert!(demo.ui.key_input(UiKey::Space));
                    demo.handle_field_check_change();
                    demo.snapshot_inputs();
                    demo.do_rebuild();
                    assert_eq!(demo.ui.table_selected_col(demo.ids.cond_formats.table), Some(column));
                    assert_eq!(demo.ui.table_cell_checked(demo.ids.cond_formats.table, 0, column), Some(false));
                    if column < 8 { demo.ui.key_input(UiKey::Right); }
                }
                capture(&mut demo, &format!("checks-keyboard-{name}-{width}"));
                click(demo.ids.ov_btn_ok, &mut demo);
                assert!(demo.state.overlay.is_none(), "{:?}", demo.state.error_msg);
                let style = &demo.state.cond_rules[0].style;
                assert!(!style.bold && !style.italic && !style.underline);
                assert_eq!(demo.state.cond_rules[0].value, "2~4");
                assert_eq!(demo.state.cond_rules[1..], original[1..]);
                assert_eq!(demo.state.conn.as_ref().unwrap().query_row(
                    "SELECT count(*) FROM names", [], |row| row.get::<_, i64>(0)
                ).unwrap(), 5);
                assert!(!demo.state.conn.as_ref().unwrap().is_autocommit());
                capture(&mut demo, &format!("checks-saved-{name}-{width}"));
            }
        }
    }

    #[test]
    fn priority_add_remove_save_and_cancel_work_across_themes_and_sizes() {
        for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400, 860), (800, 600)] {
                let mut demo = manager(width, height, theme);
                capture(&mut demo, &format!("manager-{name}-{width}"));
                let original = demo.state.cond_rules.clone();
                demo.ui.set_focus(Some(demo.ids.ov_cond_val));
                for _ in 0..4 {
                    demo.ui.key_input(UiKey::BackTab);
                }
                assert_eq!(demo.ui.focused(), Some(demo.ids.cond_formats.down));
                for _ in 0..13 {
                    demo.ui.key_input(UiKey::Tab);
                }
                assert_eq!(demo.ui.focused(), Some(demo.ids.ov_btn_ok));
                demo.ui.key_input(UiKey::Tab);
                assert_eq!(demo.ui.focused(), Some(demo.ids.ov_btn_cancel));
                assert_eq!(
                    demo.ui.button_enabled(demo.ids.cond_formats.up),
                    Some(false)
                );
                let grid = bounds(&demo, demo.ids.cond_formats.table);
                let style = next_gui_ui::TableStyle::from_theme(&demo.theme);
                assert!(demo.ui.pointer_pressed(
                    &mut demo.scene,
                    grid.x + 30.0,
                    grid.y + style.header_height + style.row_height * 1.5
                ));
                demo.ui.pointer_released();
                demo.snapshot_inputs();
                demo.poll_selection_changes();
                demo.do_rebuild();
                assert_eq!(demo.state.cond_op_idx, 4);
                assert_eq!(demo.ui.focused(), Some(demo.ids.cond_formats.table));
                click(demo.ids.cond_formats.up, &mut demo);
                assert_eq!(demo.state.cond_formats.as_ref().unwrap().rules[0].op, 4);
                click(demo.ids.cond_formats.add, &mut demo);
                demo.ui.set_text_input(demo.ids.ov_cond_val, "uncommitted");
                demo.snapshot_inputs();
                assert_eq!(demo.state.cond_rules, original);
                click(demo.ids.cond_formats.remove, &mut demo);
                assert_eq!(demo.state.cond_formats.as_ref().unwrap().rules.len(), 3);
                capture(&mut demo, &format!("reordered-{name}-{width}"));
                click(demo.ids.ov_btn_cancel, &mut demo);
                assert_eq!(demo.state.cond_rules, original);
                assert!(demo.state.cond_formats.is_none());
                demo.state.begin_cond_formats().unwrap();
                demo.do_rebuild();
                click(demo.ids.cond_formats.down, &mut demo);
                click(demo.ids.ov_btn_ok, &mut demo);
                assert!(demo.state.error_msg.is_none(), "{:?}", demo.state.error_msg);
                assert!(demo.state.overlay.is_none());
                assert_eq!(demo.state.cond_rules[0].op, 4);
                assert_eq!(
                    demo.state.cell_format_colors()[2][1],
                    Some(next_gui_scene::Color::rgb(0x41, 0x6F, 0xA8))
                );
                capture(&mut demo, &format!("saved-{name}-{width}"));
                demo.state.begin_cond_formats().unwrap();
                demo.do_rebuild();
                for _ in 0..3 {
                    click(demo.ids.cond_formats.remove, &mut demo);
                }
                assert_eq!(
                    demo.ui.button_enabled(demo.ids.cond_formats.remove),
                    Some(false)
                );
                capture(&mut demo, &format!("empty-{name}-{width}"));
                click(demo.ids.ov_btn_ok, &mut demo);
                assert!(demo.state.cond_rules.is_empty());
            }
        }
    }

    #[test]
    fn same_rule_history_selection_composition_and_scroll_survive_rebuilds() {
        let mut demo = manager(1400, 860, Theme::dark());
        let input = demo.ids.ov_cond_val;
        demo.ui.set_focus(Some(input));
        demo.ui.key_input(UiKey::End);
        demo.ui.paste_into_focused_text("345");
        demo.ui.select_text_input_range(input, 1..3);
        demo.snapshot_inputs();
        demo.win_w = 800;
        demo.win_h = 600;
        demo.do_rebuild();
        let input = demo.ids.ov_cond_val;
        assert_eq!(demo.ui.focused(), Some(input));
        assert_eq!(demo.ui.text_input_value(input), Some("2345"));
        assert_eq!(demo.ui.text_input_selection(input), Some(1..3));
        demo.ui.key_input(UiKey::Undo);
        assert_eq!(demo.ui.text_input_value(input), Some("2"));
        demo.ui.key_input(UiKey::Redo);
        assert_eq!(demo.ui.text_input_value(input), Some("2345"));
        demo.ui.ime_preedit("三", None);
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert!(demo.ui.ime_is_composing());
        assert!(demo.ui.ime_commit("三"));
        assert_eq!(demo.ui.text_input_value(demo.ids.ov_cond_val), Some("2三5"));
        demo.ui.key_input(UiKey::Undo);
        demo.snapshot_inputs();
        demo.ui
            .set_table_selected_row(demo.ids.cond_formats.table, Some(1));
        demo.snapshot_inputs();
        demo.poll_selection_changes();
        demo.do_rebuild();
        assert_eq!(demo.state.cond_value, "5");
        assert!(!demo.ui.text_input_can_undo(demo.ids.ov_cond_val));
        demo.ui.set_focus(Some(demo.ids.ov_cond_op));
        demo.ui.key_input(UiKey::Enter);
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert_eq!(demo.ui.focused(), Some(demo.ids.ov_cond_op));
        assert_eq!(demo.ui.popup_owner(), Some(demo.ids.ov_cond_op));
        demo.ui.key_input(UiKey::Escape);
        let scroll = demo.ui.scroll_offset(demo.ids.ov_scroll).unwrap();
        demo.snapshot_inputs();
        demo.do_rebuild();
        assert_eq!(demo.ui.scroll_offset(demo.ids.ov_scroll), Some(scroll));
        capture(&mut demo, "history-resized");
    }
}
