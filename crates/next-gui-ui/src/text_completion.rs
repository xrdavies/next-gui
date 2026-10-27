use super::*;
use std::ops::Range;

/// A displayed completion and the text inserted when it is accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextAreaCompletionItem {
    /// Single-line candidate label; may include a category or signature.
    pub label: String,
    /// Replacement for the application's supplied Unicode scalar range.
    pub text: String,
}

#[derive(Debug)]
pub(super) struct TextAreaCompletion {
    popup: WidgetId,
    owner: WidgetId,
    document: String,
    range: Range<usize>,
    caret: usize,
    items: Vec<TextAreaCompletionItem>,
    first: usize,
    rows: usize,
}

impl Ui {
    /// Show application-filtered completions at a focused, editable TextArea's caret.
    /// `range` uses Unicode scalar offsets and must contain the unselected caret.
    /// Rejects composition, invalid items/ranges and another open popup without mutation.
    /// Empty items dismiss this editor's completions. Call [`Ui::sync`] afterwards.
    /// Up/Down, PageUp/PageDown, Home/End and wheel browse all candidates; Enter/Tab
    /// or a pointer press accept one undoable replacement. Other input dismisses
    /// the list and continues to the editor. Supply refreshed candidates after edits.
    pub fn show_text_area_completions(
        &mut self,
        id: WidgetId,
        range: Range<usize>,
        items: Vec<TextAreaCompletionItem>,
    ) -> bool {
        if items.is_empty() {
            if self.text_area_completions_open(id) {
                self.close_popup_internal();
            }
            return false;
        }
        let Some(Widget {
            kind:
                Kind::TextArea {
                    value,
                    caret,
                    preedit,
                    read_only: false,
                    ..
                },
            node: Some(_),
            ..
        }) = self.widgets.get(&id)
        else {
            return false;
        };
        if self.focus != Some(id)
            || !preedit.is_empty()
            || range.start > range.end
            || range.start > *caret
            || range.end < *caret
            || range.end > value.chars().count()
            || self.text_area_selection(id) != Some(*caret..*caret)
            || items.iter().any(|item| {
                item.label.is_empty() || item.label.contains(['\n', '\r']) || item.text.is_empty()
            })
            || self.open_popup.is_some()
                && self.open_popup != self.text_completion.as_ref().map(|c| c.popup)
            || self.viewport.0 < 1.0
            || self.viewport.1 < 1.0
        {
            return false;
        }
        let document = value.clone();
        let caret = *caret;
        let mut style = self.context_menu_style();
        style.item_height = style.item_height.min(self.viewport.1);
        style.popup_width = items
            .iter()
            .map(|item| self.font.measure_line(&item.label, style.size).0 + 28.0)
            .fold(style.popup_width, f32::max)
            .min(self.viewport.0);
        let rows = ((self.viewport.1 / style.item_height).floor() as usize).clamp(1, 8);
        let popup = self
            .text_completion
            .as_ref()
            .map(|c| c.popup)
            .unwrap_or_else(|| self.context_menu_popup(style));
        if let Some(Widget {
            kind: Kind::ContextMenu { style: current, .. },
            ..
        }) = self.widgets.get_mut(&popup)
        {
            *current = style;
        }
        self.text_completion = Some(TextAreaCompletion {
            popup,
            owner: id,
            document,
            range,
            caret,
            items,
            first: 0,
            rows,
        });
        self.set_accessible_name(popup, "Text completions");
        self.open_context_menu_at(popup, 0.0, 0.0);
        self.update_text_completion_rows(0);
        true
    }

    /// Whether this TextArea currently owns the completion list.
    pub fn text_area_completions_open(&self, id: WidgetId) -> bool {
        self.text_completion
            .as_ref()
            .is_some_and(|c| c.owner == id && self.open_popup == Some(c.popup))
    }

    pub(super) fn text_completion_popup(&self) -> Option<WidgetId> {
        self.text_completion
            .as_ref()
            .map(|c| c.popup)
            .filter(|id| self.open_popup == Some(*id))
    }

    pub(super) fn text_completion_owner(&self, popup: WidgetId) -> Option<WidgetId> {
        self.text_completion.as_ref().filter(|completion| completion.popup == popup).map(|completion| completion.owner)
    }

    fn text_completion_valid(&self) -> bool {
        let Some(c) = &self.text_completion else {
            return false;
        };
        self.focus == Some(c.owner)
            && self.text_area_value(c.owner) == Some(c.document.as_str())
            && self.text_area_selection(c.owner) == Some(c.caret..c.caret)
            && matches!(self.widgets.get(&c.owner).map(|w| &w.kind),
                Some(Kind::TextArea { read_only: false, preedit, .. }) if preedit.is_empty())
    }

    fn update_text_completion_rows(&mut self, selected: usize) {
        let Some(c) = &mut self.text_completion else {
            return;
        };
        let selected = selected.min(c.items.len() - 1);
        if selected < c.first {
            c.first = selected;
        }
        if selected >= c.first + c.rows {
            c.first = selected + 1 - c.rows;
        }
        let items = c
            .items
            .iter()
            .enumerate()
            .skip(c.first)
            .take(c.rows)
            .map(|(i, item)| MenuItem::new(i as u64, item.label.clone()))
            .collect();
        if let Some(Widget {
            kind: Kind::ContextMenu { items: current, .. },
            ..
        }) = self.widgets.get_mut(&c.popup)
        {
            *current = items;
        }
        self.popup_menu_row = Some(selected - c.first);
    }

    pub(super) fn move_text_completion(&mut self, delta: isize) {
        let Some(c) = &self.text_completion else {
            return;
        };
        let current = c.first + self.popup_menu_row.unwrap_or(0);
        let next = (current as isize + delta).clamp(0, c.items.len() as isize - 1) as usize;
        self.update_text_completion_rows(next);
    }

    pub(super) fn commit_text_completion(&mut self, index: usize) {
        let valid = self.text_completion_valid();
        let edit = self.text_completion.as_ref().and_then(|c| {
            c.items
                .get(index)
                .map(|item| (c.owner, c.range.clone(), item.text.clone()))
        });
        self.close_popup_internal();
        if valid {
            if let Some((owner, range, text)) = edit {
                self.replace_text_area_range(owner, range, &text);
            }
        }
    }

    pub(super) fn key_text_completion(&mut self, key: UiKey) -> Option<bool> {
        self.text_completion_popup()?;
        if !self.text_completion_valid() {
            self.close_popup_internal();
            return None;
        }
        let c = self.text_completion.as_ref().unwrap();
        let index = c.first + self.popup_menu_row.unwrap_or(0);
        match key {
            UiKey::Enter | UiKey::Tab => self.commit_text_completion(index),
            UiKey::Escape => {
                self.close_popup_internal();
            }
            UiKey::Up => self.move_text_completion(-1),
            UiKey::Down => self.move_text_completion(1),
            UiKey::PageUp => self.move_text_completion(-(c.rows as isize)),
            UiKey::PageDown => self.move_text_completion(c.rows as isize),
            UiKey::Home => self.update_text_completion_rows(0),
            UiKey::End => self.update_text_completion_rows(c.items.len() - 1),
            _ => {
                self.close_popup_internal();
                return None;
            }
        }
        Some(true)
    }

    pub(super) fn sync_text_completion(&mut self, scene: &Scene) {
        let Some(popup) = self.text_completion_popup() else {
            return;
        };
        if !self.text_completion_valid() {
            self.close_popup_internal();
            return;
        }
        if let Some((x, y, _, height)) = self.ime_cursor_area(scene) {
            if x < 0.0 || x >= self.viewport.0 || y + height <= 0.0 || y >= self.viewport.1 {
                self.close_popup_internal();
                return;
            }
            let item_height = self.context_menu_style().item_height.min(self.viewport.1);
            match self.widgets.get_mut(&popup).map(|w| &mut w.kind) {
                Some(Kind::ContextMenu { style, .. }) => style.item_height = item_height,
                _ => return,
            }
            let rows = ((self.viewport.1 / item_height).floor() as usize).clamp(1, 8);
            let c = self.text_completion.as_mut().unwrap();
            let selected = c.first + self.popup_menu_row.unwrap_or(0);
            c.rows = rows;
            self.update_text_completion_rows(selected);
            if let Some(Widget {
                kind:
                    Kind::ContextMenu {
                        anchor_x, anchor_y, ..
                    },
                ..
            }) = self.widgets.get_mut(&popup)
            {
                *anchor_x = x;
                *anchor_y = y + height + 4.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> (Ui, Scene, WidgetId) {
        let mut ui = Ui::new();
        ui.set_viewport(500.0, 300.0);
        let mut area = WidgetId(0);
        ui.column(ColumnStyle::default(), |c| {
            area = c.text_area("中文\nsel", "SQL", TextAreaStyle::default());
        });
        let mut scene = Scene::new();
        ui.set_focus(Some(area));
        ui.sync(&mut scene);
        ui.select_text_area_range(area, 6..6);
        (ui, scene, area)
    }

    fn items() -> Vec<TextAreaCompletionItem> {
        (0..24)
            .map(|i| TextAreaCompletionItem {
                label: format!("candidate {i}"),
                text: format!("SELECT_{i}"),
            })
            .collect()
    }

    #[test]
    fn text_completions_page_accept_once_and_keep_editor_history_and_focus() {
        let (mut ui, mut scene, area) = editor();
        ui.set_text_area_line_numbers(area, true);
        assert!(ui.show_text_area_completions(area, 3..6, items()));
        ui.sync(&mut scene);
        let popup = ui.text_completion_popup().unwrap();
        let panel = scene.get(ui.widgets[&popup].aux[0]).unwrap().bounds;
        assert!(panel.x >= 0.0 && panel.y >= 0.0 && panel.y + panel.height <= 300.0);
        assert!(ui.key_input(UiKey::End));
        ui.sync(&mut scene);
        assert_eq!(ui.popup_menu_row, Some(7));
        assert!(ui.key_input(UiKey::PageUp));
        assert!(ui.key_input(UiKey::Down));
        assert!(ui.key_input(UiKey::Tab));
        assert_eq!(ui.text_area_value(area), Some("中文\nSELECT_16"));
        assert_eq!(ui.focused(), Some(area));
        assert!(!ui.has_open_popup());
        assert_eq!(ui.take_menu_action(), None);
        ui.key_input(UiKey::Undo);
        assert_eq!(ui.text_area_value(area), Some("中文\nsel"));
        assert_eq!(ui.text_area_caret(area), Some(6));
        ui.key_input(UiKey::Redo);
        assert_eq!(ui.text_area_value(area), Some("中文\nSELECT_16"));
    }

    #[test]
    fn text_completions_pointer_wheel_and_typing_share_the_visible_candidates() {
        let (mut ui, mut scene, area) = editor();
        ui.show_text_area_completions(area, 3..6, items());
        ui.key_input(UiKey::End);
        ui.sync(&mut scene);
        let popup = ui.text_completion_popup().unwrap();
        let panel = scene.get(ui.widgets[&popup].aux[0]).unwrap().bounds;
        assert!(ui.pointer_scroll(&scene, panel.x + 2.0, panel.y + 2.0, 0.0, -30.0));
        ui.sync(&mut scene);
        assert_eq!(ui.popup_menu_row, Some(6));
        ui.pointer_hover(&scene, 490.0, 290.0);
        assert_eq!(
            ui.popup_menu_row,
            Some(6),
            "leaving the list retains its active candidate"
        );
        assert!(ui.pointer_pressed(&mut scene, panel.x + 10.0, panel.y + 5.0));
        assert_eq!(ui.text_area_value(area), Some("中文\nSELECT_16"));
        assert_eq!(ui.take_menu_action(), None);
        ui.key_input(UiKey::Undo);
        ui.show_text_area_completions(area, 3..6, items());
        ui.key_input(UiKey::Char('e'));
        assert!(!ui.has_open_popup());
        assert_eq!(ui.text_area_value(area), Some("中文\nsele"));
        ui.show_text_area_completions(area, 3..7, items());
        ui.key_input(UiKey::Escape);
        assert_eq!(ui.text_area_value(area), Some("中文\nsele"));
    }

    #[test]
    fn text_completions_reject_stale_document_selection_composition_and_other_popups() {
        let (mut ui, mut scene, area) = editor();
        for range in [Range { start: 7, end: 6 }, 0..usize::MAX, 0..5] {
            assert!(!ui.show_text_area_completions(area, range, items()));
        }
        for (label, text) in [("", "text"), ("label", ""), ("multi\nline", "text")] {
            assert!(!ui.show_text_area_completions(
                area,
                3..6,
                vec![TextAreaCompletionItem {
                    label: label.into(),
                    text: text.into()
                }]
            ));
        }
        ui.select_text_area_range(area, 3..6);
        assert!(!ui.show_text_area_completions(area, 3..6, items()));
        ui.select_text_area_range(area, 6..6);
        ui.ime_preedit("选", None);
        assert!(!ui.show_text_area_completions(area, 3..6, items()));
        ui.ime_clear_preedit();
        ui.set_text_area_read_only(area, true);
        assert!(!ui.show_text_area_completions(area, 3..6, items()));
        ui.set_text_area_read_only(area, false);
        let other = ui.context_menu_popup(ui.context_menu_style());
        ui.open_context_menu(other, vec![MenuItem::new(99, "Other")], 1.0, 1.0);
        assert!(!ui.show_text_area_completions(area, 3..6, items()));
        assert_eq!(ui.open_popup, Some(other));
        ui.close_popup();
        ui.show_text_area_completions(area, 3..6, items());
        ui.replace_text_area_range(area, 0..0, "--");
        ui.commit_text_completion(0);
        assert_eq!(ui.text_area_value(area), Some("--中文\nsel"));
        ui.select_text_area_range(area, 8..8);
        ui.show_text_area_completions(area, 5..8, items());
        ui.set_focus(None);
        ui.sync(&mut scene);
        assert!(!ui.has_open_popup());
    }

    #[test]
    fn text_completions_clip_long_labels_inside_a_narrow_window() {
        let (mut ui, mut scene, area) = editor();
        ui.set_viewport(180.0, 120.0);
        let mut candidates = items();
        candidates[0].label = "中文 long identifier ".repeat(40);
        assert!(ui.show_text_area_completions(area, 3..6, candidates));
        ui.sync(&mut scene);
        let popup = ui.text_completion_popup().unwrap();
        let aux = &ui.widgets[&popup].aux;
        let panel = scene.get(aux[0]).unwrap().bounds;
        assert!(panel.x + panel.width <= 180.0 && panel.y + panel.height <= 120.0);
        let label = scene.get(aux[2]).unwrap();
        let clip = label
            .clip
            .expect("long menu labels must be clipped to their row");
        assert!(clip.x >= panel.x && clip.x + clip.width <= panel.x + panel.width);
        ui.key_input(UiKey::Tab);
        assert_eq!(ui.text_area_value(area), Some("中文\nSELECT_0"));
    }
}
