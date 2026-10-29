//! DB4S FilterLineEdit editing actions and expression helpers.
use crate::{state::MainTab, Demo, KeyCode, Modifiers, UiKey, WidgetId, WindowCtx};
use next_gui_ui::MenuItem;

const EXPRESSIONS: u64 = 200;
const UNDO: u64 = 201;
const REDO: u64 = 202;
const CUT: u64 = 203;
const COPY: u64 = 204;
const PASTE: u64 = 205;
const DELETE: u64 = 206;
const SELECT_ALL: u64 = 207;
const HELP: u64 = 208;
const FIRST_CONDITION: u64 = 210;
const CONDITIONS: [(&str, &str, Option<usize>); 13] = [
    ("Is NULL", "=NULL", None),
    ("Is not NULL", "<>NULL", None),
    ("Is empty", "=''", None),
    ("Is not empty", "<>''", None),
    ("Not containing…", "/^((?!?).)*$/", Some(6)),
    ("Equal to…", "=?", Some(1)),
    ("Not equal to…", "<>?", Some(2)),
    ("Greater than…", ">?", Some(1)),
    ("Less than…", "<?", Some(1)),
    ("Greater or equal…", ">=?", Some(2)),
    ("Less or equal…", "<=?", Some(2)),
    ("In range…", "?~?", Some(2)),
    ("Regular expression…", "/?/", Some(1)),
];

impl Demo {
    fn is_browse_filter(&self, id: WidgetId) -> bool {
        id.0 != 0
            && (id == self.ids.browse_global_filter || self.ids.browse_col_filters.contains(&id))
    }

    fn filter_menu_available(&self) -> bool {
        self.state.tab == MainTab::Browse
            && self.state.overlay.is_none()
            && !self.state.confirm_open
            && !self.state.sql_busy
            && self.ui.active_dialog().is_none()
    }

    fn filter_menu_items(&self, input: WidgetId) -> Vec<MenuItem> {
        let action = |id, label, enabled| {
            if enabled {
                MenuItem::new(id, label)
            } else {
                MenuItem::disabled(id, label)
            }
        };
        let selection = self.ui.text_input_selection(input).is_some();
        let mut expressions = vec![MenuItem::new(HELP, "What's This?"), MenuItem::separator()];
        for (index, (label, _, _)) in CONDITIONS.iter().enumerate() {
            if index == 4 {
                expressions.push(MenuItem::separator());
            }
            expressions.push(MenuItem::new(FIRST_CONDITION + index as u64, *label));
        }
        vec![
            action(UNDO, "Undo", self.ui.text_input_can_undo(input)),
            action(REDO, "Redo", self.ui.text_input_can_redo(input)),
            MenuItem::separator(),
            action(CUT, "Cut", selection),
            action(COPY, "Copy", selection),
            MenuItem::new(PASTE, "Paste"),
            action(DELETE, "Delete", selection),
            MenuItem::separator(),
            action(
                SELECT_ALL,
                "Select All",
                self.ui
                    .text_input_value(input)
                    .is_some_and(|value| !value.is_empty()),
            ),
            MenuItem::separator(),
            MenuItem::submenu(EXPRESSIONS, "Set Filter Expression", expressions),
        ]
    }

    pub(crate) fn open_filter_context_menu(&mut self, x: f32, y: f32) -> bool {
        if !self.filter_menu_available() {
            return false;
        }
        let Some(input) = self
            .ui
            .hit_widget(&self.scene, x, y)
            .filter(|id| self.is_browse_filter(*id))
        else {
            return false;
        };
        self.cancel_browse_filter();
        self.snapshot_inputs();
        self.browse_header_column = None;
        self.filter_context = Some(input);
        self.ui.set_focus(Some(input));
        self.ui
            .set_accessible_name(self.ids.context_menu, "Filter editing and expressions");
        self.ui
            .open_context_menu(self.ids.context_menu, self.filter_menu_items(input), x, y)
    }

    pub(crate) fn handle_filter_context_key(&mut self, key: KeyCode, modifiers: Modifiers) -> bool {
        if key != KeyCode::Function(10)
            || !modifiers.shift
            || modifiers.commandish()
            || modifiers.alt
        {
            return false;
        }
        let Some(input) = self.ui.focused().filter(|id| self.is_browse_filter(*id)) else {
            return false;
        };
        let bounds = self
            .scene
            .iter_paint_order()
            .into_iter()
            .find(|node| {
                self.ui.widget_from_node(node.id) == Some(input) && !node.bounds.is_empty()
            })
            .map(|node| node.bounds);
        bounds.is_some_and(|bounds| {
            self.open_filter_context_menu(bounds.x + 4.0, bounds.y + bounds.height * 0.5)
        })
    }

    fn edit_filter_menu_action(&mut self, item: u64, pasted: Option<&str>) {
        let Some(input) = self.filter_context.filter(|id| self.is_browse_filter(*id)) else {
            return;
        };
        if !self.filter_menu_available() {
            return;
        }
        self.ui.set_focus(Some(input));
        match item {
            UNDO => {
                self.ui.key_input(UiKey::Undo);
            }
            REDO => {
                self.ui.key_input(UiKey::Redo);
            }
            SELECT_ALL => {
                self.ui.key_input(UiKey::SelectAll);
            }
            CUT | DELETE => {
                if self.ui.text_input_selection(input).is_some() {
                    self.ui.key_input(UiKey::Backspace);
                }
            }
            PASTE => {
                if let Some(text) = pasted {
                    self.ui.paste_into_focused_text(text);
                }
            }
            HELP => self
                .open_url("https://github.com/sqlitebrowser/sqlitebrowser/wiki/Using-the-Filters"),
            _ => {
                if let Some((_, expression, placeholder)) = item
                    .checked_sub(FIRST_CONDITION)
                    .and_then(|index| CONDITIONS.get(index as usize))
                {
                    self.ui.key_input(UiKey::SelectAll);
                    self.ui.paste_into_focused_text(expression);
                    if let Some(start) = placeholder {
                        self.ui.select_text_input_range(input, *start..*start + 1);
                    }
                    self.snapshot_inputs();
                    self.flush_browse_filter();
                }
            }
        }
        self.snapshot_inputs();
    }

    pub(crate) fn handle_filter_menu_action(&mut self, item: u64, ctx: &mut WindowCtx<'_>) -> bool {
        if !(UNDO..=HELP).contains(&item)
            && !(FIRST_CONDITION..FIRST_CONDITION + CONDITIONS.len() as u64).contains(&item)
        {
            return false;
        }
        if !self.filter_menu_available()
            || !self
                .filter_context
                .is_some_and(|id| self.is_browse_filter(id))
        {
            return true;
        }
        self.ui.set_focus(self.filter_context);
        if item == COPY || item == CUT {
            let Some(text) = self.ui.focused_text_for_copy().filter(|_| {
                self.ui
                    .text_input_selection(self.filter_context.unwrap())
                    .is_some()
            }) else {
                return true;
            };
            if let Err(error) = ctx.clipboard_set_text(&text) {
                self.set_error(error.to_string());
                return true;
            }
        }
        let pasted = if item == PASTE {
            match ctx.clipboard_get_text() {
                Ok(text) => text,
                Err(error) => {
                    self.set_error(error.to_string());
                    return true;
                }
            }
        } else {
            None
        };
        self.edit_filter_menu_action(item, pasted.as_deref());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browse_filter::tests::{capture, fixture, settle};
    use crate::state::OverlayKind;
    use next_gui_ui::Theme;
    use std::time::{Duration, Instant};

    fn open(demo: &mut Demo, input: WidgetId) {
        demo.ui.set_focus(Some(input));
        demo.ui.sync(&mut demo.scene);
        assert!(demo.handle_filter_context_key(
            KeyCode::Function(10),
            Modifiers {
                shift: true,
                ..Modifiers::default()
            }
        ));
        demo.ui.sync(&mut demo.scene);
    }

    fn choose(demo: &mut Demo, index: usize) {
        demo.ui.key_input(UiKey::End);
        demo.ui.key_input(UiKey::Right);
        for _ in 0..=index {
            demo.ui.key_input(UiKey::Down);
        }
        demo.ui.key_input(UiKey::Enter);
        let (owner, item) = demo.ui.take_menu_action().unwrap();
        assert_eq!(owner, demo.ids.context_menu);
        assert_eq!(item, FIRST_CONDITION + index as u64);
        demo.edit_filter_menu_action(item, None);
    }

    #[test]
    fn all_original_expression_helpers_select_the_placeholder_and_preserve_undo() {
        let mut demo = fixture(1400, 860, Theme::dark());
        for (index, (_, expression, placeholder)) in CONDITIONS.iter().enumerate() {
            let input = demo.ids.browse_col_filters[0];
            demo.ui.set_text_input(input, "previous 中文");
            open(&mut demo, input);
            choose(&mut demo, index);
            assert_eq!(demo.ui.text_input_value(input), Some(*expression));
            assert_eq!(
                demo.ui.text_input_selection(input),
                placeholder.map(|start| start..start + 1)
            );
            if placeholder.is_some() {
                assert_eq!(demo.ui.focused_text_for_copy().as_deref(), Some("?"));
            }
            demo.do_rebuild();
            let input = demo.ids.browse_col_filters[0];
            assert_eq!(
                demo.ui.text_input_selection(input),
                placeholder.map(|start| start..start + 1)
            );
            assert!(demo.ui.text_input_can_undo(input));
            assert!(!demo.ui.text_input_can_redo(input));
            demo.filter_context = Some(input);
            demo.edit_filter_menu_action(UNDO, None);
            assert_eq!(demo.ui.text_input_value(input), Some("previous 中文"));
            assert!(demo.ui.text_input_can_redo(input));
            demo.edit_filter_menu_action(REDO, None);
            assert_eq!(demo.ui.text_input_value(input), Some(*expression));
            assert_eq!(
                demo.ui.text_input_selection(input),
                placeholder.map(|start| start..start + 1)
            );
        }
    }

    #[test]
    fn expression_queries_flush_then_placeholder_edits_debounce_across_themes_and_sizes() {
        for (name, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
            for (width, height) in [(1400, 860), (800, 600)] {
                let mut demo = fixture(width, height, theme);
                demo.state.prefs.filter_delay_ms = 5000;
                let input = demo.ids.browse_col_filters[1];
                open(&mut demo, input);
                demo.ui.key_input(UiKey::End);
                demo.ui.key_input(UiKey::Right);
                demo.ui.key_input(UiKey::End);
                capture(&mut demo, &format!("menu-{name}-{width}"));
                demo.ui.key_input(UiKey::Left);
                choose(&mut demo, 9);
                let now = Instant::now();
                demo.advance_browse_filter(now);
                assert!(demo.state.browse_work.is_some());
                settle(&mut demo, now);
                let input = demo.ids.browse_col_filters[1];
                assert_eq!(demo.ui.focused(), Some(input));
                assert_eq!(demo.ui.text_input_selection(input), Some(2..3));
                capture(&mut demo, &format!("placeholder-{name}-{width}"));
                assert!(demo.ui.paste_into_focused_text("3"));
                assert_eq!(demo.ui.text_input_value(input), Some(">=3"));
                assert_eq!(
                    demo.advance_browse_filter(now + Duration::from_secs(1)),
                    Some(Duration::from_secs(5))
                );
                demo.advance_browse_filter(now + Duration::from_secs(6));
                settle(&mut demo, now + Duration::from_secs(6));
                assert_eq!(demo.state.browse().total_rows, Some(3));
                assert_eq!(demo.state.browse().col_filters[1], ">=3");
                capture(&mut demo, &format!("result-{name}-{width}"));
                let input = demo.ids.browse_col_filters[1];
                demo.filter_context = Some(input);
                demo.edit_filter_menu_action(UNDO, None);
                assert_eq!(demo.ui.text_input_value(input), Some(">=?"));
                assert_eq!(demo.ui.text_input_selection(input), Some(2..3));
                demo.edit_filter_menu_action(REDO, None);
                assert_eq!(demo.ui.text_input_value(input), Some(">=3"));
                demo.edit_filter_menu_action(SELECT_ALL, None);
                demo.edit_filter_menu_action(DELETE, None);
                demo.flush_browse_filter();
                demo.advance_browse_filter(Instant::now());
                settle(&mut demo, Instant::now());
                assert_eq!(demo.state.browse().total_rows, Some(5));
            }
        }
    }

    #[test]
    fn filter_menu_guards_drafts_and_keeps_raw_global_whitespace_history_and_selection() {
        let mut demo = fixture(1400, 860, Theme::dark());
        let input = demo.ids.browse_global_filter;
        demo.ui.set_focus(Some(input));
        assert!(demo.ui.paste_into_focused_text("alp   beta"));
        demo.ui.select_text_input_range(input, 0..3);
        demo.snapshot_inputs();
        assert_eq!(demo.state.browse().global_filters, vec!["alp", "beta"]);
        demo.do_rebuild();
        let input = demo.ids.browse_global_filter;
        assert_eq!(demo.ui.text_input_value(input), Some("alp   beta"));
        assert_eq!(demo.ui.text_input_selection(input), Some(0..3));
        capture(&mut demo, "global-whitespace-selection");
        demo.ui.ime_preedit("仮", None);
        open(&mut demo, input);
        demo.ui.key_input(UiKey::Escape);
        assert!(demo.ui.ime_is_composing());
        demo.ui.ime_preedit("", None);
        open(&mut demo, input);
        let items = demo.filter_menu_items(input);
        assert!(items.iter().find(|item| item.id == COPY).unwrap().enabled);
        demo.ui.key_input(UiKey::Escape);
        assert_eq!(demo.ui.text_input_value(input), Some("alp   beta"));
        demo.filter_context = Some(input);
        demo.edit_filter_menu_action(UNDO, None);
        assert_eq!(demo.ui.text_input_value(input), Some(""));
        demo.edit_filter_menu_action(REDO, None);
        assert_eq!(demo.ui.text_input_value(input), Some("alp   beta"));
        demo.state.overlay = Some(OverlayKind::Preferences);
        demo.edit_filter_menu_action(FIRST_CONDITION, None);
        assert_eq!(demo.ui.text_input_value(input), Some("alp   beta"));
        demo.state.overlay = None;
        demo.state.confirm_open = true;
        assert!(!demo.handle_filter_context_key(
            KeyCode::Function(10),
            Modifiers {
                shift: true,
                ..Modifiers::default()
            }
        ));
        demo.state.confirm_open = false;
        demo.state.browse_mut().sel_row = Some(0);
        demo.state.browse_mut().sel_col = Some(0);
        demo.state.sync_edit_cell_from_selection();
        demo.state.edit_value = "pending cell".into();
        assert!(demo.state.has_cell_draft());
        open(&mut demo, input);
        choose(&mut demo, 0);
        assert_eq!(demo.advance_browse_filter(Instant::now()), None);
        assert!(demo.state.browse_work.is_none());
        assert_eq!(demo.state.edit_value, "pending cell");
        demo.state.discard_cell_draft();
        demo.ui.set_text_input(input, "");
        demo.snapshot_inputs();
        demo.state.switch_browse_table_in("main", "other").unwrap();
        demo.do_rebuild();
        assert!(demo.filter_context.is_none());
        let other = demo.ids.browse_global_filter;
        demo.edit_filter_menu_action(FIRST_CONDITION, None);
        assert_eq!(demo.ui.text_input_value(other), Some(""));
        demo.state.tab = MainTab::Structure;
        assert!(!demo.handle_filter_context_key(
            KeyCode::Function(10),
            Modifiers {
                shift: true,
                ..Modifiers::default()
            }
        ));
    }
}
