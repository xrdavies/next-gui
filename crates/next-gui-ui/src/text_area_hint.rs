use super::*;

#[derive(Debug)]
pub(super) struct TextAreaHint {
    pub(super) popup: WidgetId,
    owner: WidgetId,
    document: String,
    caret: usize,
    anchor: usize,
    prefix_end: usize,
    dismissed: bool,
}

impl Ui {
    /// Set a passive, wrapping hint at a Unicode scalar offset in a focused TextArea.
    /// The editor keeps focus and accepts ordinary input. Menus/composition hide the
    /// hint. Refresh after edits; empty text clears it. Escape suppresses the same
    /// anchor while its document prefix remains unchanged. Call [`Ui::sync`] to paint.
    pub fn set_text_area_hint(
        &mut self,
        id: WidgetId,
        anchor: usize,
        text: impl Into<String>,
    ) -> bool {
        let text = text.into();
        if text.is_empty() {
            if let Some(Widget {
                kind: Kind::TextArea { edit, .. },
                ..
            }) = self.widgets.get_mut(&id)
            {
                edit.hint_dismissed = None;
            }
            if let Some(hint) = self.text_area_hint.as_mut().filter(|h| h.owner == id) {
                hint.anchor = usize::MAX;
                if let Kind::Tooltip { text, .. } =
                    &mut self.widgets.get_mut(&hint.popup).unwrap().kind
                {
                    text.clear();
                }
            }
            return false;
        }
        let Some(Widget {
            kind:
                Kind::TextArea {
                    value,
                    caret,
                    read_only: false,
                    preedit,
                    style,
                    edit,
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
            || anchor > *caret
            || self.text_area_selection(id) != Some(*caret..*caret)
        {
            return false;
        }
        let Some(prefix_end) = value
            .char_indices()
            .nth(anchor + 1)
            .map(|(i, _)| i)
            .or_else(|| (anchor <= value.chars().count()).then_some(value.len()))
        else {
            return false;
        };
        let dismissed = edit
            .hint_dismissed
            .as_ref()
            .is_some_and(|(offset, prefix)| *offset == anchor && prefix == &value[..prefix_end]);
        let document = value.clone();
        let caret = *caret;
        let editor_size = style.size;
        if !dismissed {
            if let Kind::TextArea { edit, .. } = &mut self.widgets.get_mut(&id).unwrap().kind {
                edit.hint_dismissed = None;
            }
        }
        let popup = if let Some(hint) = &self.text_area_hint {
            hint.popup
        } else {
            let popup = self.alloc();
            self.widgets.insert(
                popup,
                Widget {
                    kind: Kind::Tooltip {
                        text: String::new(),
                        style: self.tooltip_style(),
                        children: Vec::new(),
                    },
                    node: None,
                    label_node: None,
                    chrome_node: None,
                    aux: Vec::new(),
                    accessible_name: None,
                },
            );
            popup
        };
        let mut hint_style = self.tooltip_style();
        hint_style.size = hint_style.size.max(editor_size);
        self.set_accessible_name(popup, text.clone());
        if let Kind::Tooltip {
            text: current,
            style,
            ..
        } = &mut self.widgets.get_mut(&popup).unwrap().kind
        {
            *current = text;
            *style = hint_style;
        }
        self.text_area_hint = Some(TextAreaHint {
            popup,
            owner: id,
            document,
            caret,
            anchor,
            prefix_end,
            dismissed,
        });
        self.text_area_hint(id).is_some()
    }

    /// Current valid hint text for this editor, excluding dismissed hints and open menus.
    /// An offscreen anchor is additionally hidden during [`Ui::sync`].
    pub fn text_area_hint(&self, id: WidgetId) -> Option<&str> {
        let h = self.text_area_hint.as_ref()?;
        if h.owner != id
            || h.dismissed
            || self.focus != Some(id)
            || self.open_popup.is_some()
            || self.text_area_value(id) != Some(h.document.as_str())
            || self.text_area_selection(id) != Some(h.caret..h.caret)
            || !matches!(&self.widgets.get(&id)?.kind, Kind::TextArea { read_only: false, preedit, .. } if preedit.is_empty())
        {
            return None;
        }
        match &self.widgets.get(&h.popup)?.kind {
            Kind::Tooltip { text, .. } if !text.is_empty() => Some(text),
            _ => None,
        }
    }

    pub(super) fn dismiss_text_area_hint(&mut self) -> bool {
        let Some(h) = &self.text_area_hint else {
            return false;
        };
        if self.text_area_hint(h.owner).is_none() {
            return false;
        }
        if let Kind::TextArea { edit, .. } = &mut self.widgets.get_mut(&h.owner).unwrap().kind {
            edit.hint_dismissed = Some((h.anchor, h.document[..h.prefix_end].to_owned()));
        }
        self.text_area_hint.as_mut().unwrap().dismissed = true;
        true
    }

    pub(super) fn text_area_hint_anchor(&self, scene: &Scene) -> Option<(WidgetId, Rect)> {
        let h = self.text_area_hint.as_ref()?;
        self.text_area_hint(h.owner)?;
        let (x, y, w, height) = self.text_area_position_area(scene, h.owner, Some(h.anchor))?;
        let bounds = scene.get(self.widgets.get(&h.owner)?.node?)?.bounds;
        if !bounds.contains(x, y)
            || x < 0.0
            || x >= self.viewport.0
            || y < 0.0
            || y >= self.viewport.1
        {
            return None;
        }
        Some((h.popup, Rect::new(x, y, w, height)))
    }

    pub(super) fn pointer_text_area_hint(&mut self, scene: &mut Scene, x: f32, y: f32) {
        let Some(h) = &self.text_area_hint else {
            return;
        };
        let popup = h.popup;
        let inside = self.widgets[&popup]
            .aux
            .first()
            .and_then(|n| scene.get(*n))
            .is_some_and(|n| n.visible && n.bounds.contains(x, y));
        if inside {
            self.dismiss_text_area_hint();
        }
        // Passive overlays must not absorb the editor's pointer hit.
        self.hide_popup_aux_mut(scene, popup);
    }

    pub(super) fn text_area_position_area(
        &self,
        scene: &Scene,
        id: WidgetId,
        index: Option<usize>,
    ) -> Option<(f32, f32, f32, f32)> {
        let w = self.widgets.get(&id)?;
        let Kind::TextArea {
            value,
            caret,
            preedit,
            preedit_cursor,
            scroll_y,
            style,
            edit,
            line_numbers,
            ..
        } = &w.kind
        else {
            return None;
        };
        let bounds = scene.get(w.node?)?.bounds;
        let (display, caret) = if let Some(index) = index {
            (value.clone(), index)
        } else {
            text_area::display_text(value, *caret, edit, preedit, *preedit_cursor)
        };
        let gutter = self.text_area_gutter_width(&display, *style, *line_numbers, bounds.width);
        let layout = self.font.layout(
            &display,
            style.size,
            Some((bounds.width - style.padding_x * 2.0 - gutter).max(1.0)),
        );
        let (x, y) = layout.caret_top_left(&display, caret, self.font.metrics(style.size).ascent);
        Some((
            (bounds.x + style.padding_x + gutter + x).floor(),
            (bounds.y + style.padding_y - scroll_y + y).floor(),
            2.0,
            self.font.metrics(style.size).ascent.ceil(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(value: &str) -> (Ui, Scene, WidgetId) {
        let mut ui = Ui::new();
        ui.set_viewport(180.0, 120.0);
        let mut area = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 180.0,
                height: 120.0,
                padding: 0.0,
                ..ColumnStyle::default()
            },
            |c| {
                area = c.text_area(
                    value,
                    "SQL",
                    TextAreaStyle {
                        width: 180.0,
                        height: 120.0,
                        ..TextAreaStyle::default()
                    },
                );
            },
        );
        let mut scene = Scene::new();
        ui.set_focus(Some(area));
        let end = value.chars().count();
        ui.select_text_area_range(area, end..end);
        ui.sync(&mut scene);
        (ui, scene, area)
    }

    #[test]
    fn hints_wrap_clip_flip_and_pass_pointer_input_to_the_editor() {
        let (mut ui, mut scene, area) = editor("-- 中文\n\n\n\n\n\n\nsubstr(");
        let anchor = ui.text_area_caret(area).unwrap() - 1;
        let message = "substr(arg1, arg2, arg3)\nArgument 2 中文";
        assert!(ui.set_text_area_hint(area, anchor, message));
        ui.sync(&mut scene);
        let popup = ui.text_area_hint.as_ref().unwrap().popup;
        let aux = &ui.widgets[&popup].aux;
        let panel = scene.get(aux[0]).unwrap().bounds;
        let label = scene.get(aux[1]).unwrap();
        let clip = label.clip.unwrap();
        assert!(
            panel.x >= 0.0
                && panel.y >= 0.0
                && panel.x + panel.width <= 180.0
                && panel.y + panel.height <= 120.0
        );
        assert!(
            clip.x >= panel.x
                && clip.y >= panel.y
                && clip.x + clip.width <= panel.x + panel.width
                && clip.y + clip.height <= panel.y + panel.height
        );
        assert!(panel.height > ui.font.metrics(ui.tooltip_style().size).ascent * 2.0);
        let (_, y, _, _) = ui
            .text_area_position_area(&scene, area, Some(anchor))
            .unwrap();
        assert!(panel.y < y, "hint at bottom must flip above its anchor");
        assert!(ui.pointer_pressed(&mut scene, panel.x + 3.0, panel.y + 3.0));
        assert_eq!(ui.focused(), Some(area));
        assert_eq!(ui.text_area_hint(area), None);
        assert_eq!(
            ui.text_area_value(area),
            Some("-- 中文\n\n\n\n\n\n\nsubstr(")
        );
    }

    #[test]
    fn hint_escape_stays_dismissed_during_argument_edits_and_preserves_undo() {
        let (mut ui, mut scene, area) = editor("lower(");
        assert!(ui.set_text_area_hint(area, 5, "lower(arg1)"));
        assert!(ui.key_input(UiKey::Escape));
        assert_eq!(ui.focused(), Some(area));
        let state = ui.text_area_state(area).unwrap();
        let (mut rebuilt, _, other) = editor("lower(");
        rebuilt.restore_text_area_state(other, &state);
        assert!(!rebuilt.set_text_area_hint(other, 5, "lower(arg1)"));
        ui.key_input(UiKey::Char('a'));
        assert!(!ui.set_text_area_hint(area, 5, "lower(arg1)\nArgument 1"));
        assert_eq!(ui.text_area_hint(area), None);
        ui.key_input(UiKey::Undo);
        assert_eq!(ui.text_area_value(area), Some("lower("));
        assert!(!ui.set_text_area_hint(area, 5, "lower(arg1)"));
        ui.replace_text_area_range(area, 0..5, "upper");
        assert!(ui.set_text_area_hint(area, 5, "upper(arg1)"));
        ui.sync(&mut scene);
        assert_eq!(ui.text_area_hint(area), Some("upper(arg1)"));
        ui.set_text_area_hint(area, 0, "");
        assert_eq!(ui.text_area_hint(area), None);
        assert!(!ui.set_text_area_hint(area, usize::MAX, "bad"));
    }

    #[test]
    fn hints_defer_to_popups_and_hide_stale_selection_composition_and_read_only_state() {
        let (mut ui, mut scene, area) = editor("substr(");
        assert!(ui.set_text_area_hint(area, 6, "substr(arg1, arg2)"));
        let popup = ui.context_menu_popup(ui.context_menu_style());
        ui.open_context_menu(popup, vec![MenuItem::new(1, "Other")], 1.0, 1.0);
        assert_eq!(ui.text_area_hint(area), None);
        assert!(ui.key_input(UiKey::Escape));
        assert_eq!(ui.text_area_hint(area), Some("substr(arg1, arg2)"));
        ui.ime_preedit("中文", None);
        ui.sync(&mut scene);
        assert_eq!(ui.text_area_hint(area), None);
        assert!(!ui.set_text_area_hint(area, 6, "bad"));
        ui.ime_clear_preedit();
        ui.set_text_area_read_only(area, true);
        assert_eq!(ui.text_area_hint(area), None);
        ui.set_text_area_read_only(area, false);
        ui.select_text_area_range(area, 0..7);
        assert_eq!(ui.text_area_hint(area), None);
        ui.select_text_area_range(area, 7..7);
        ui.key_input(UiKey::Char('a'));
        assert_eq!(
            ui.text_area_hint(area),
            None,
            "document edits require refreshed context"
        );
        ui.set_focus(None);
        assert!(!ui.set_text_area_hint(area, 6, "bad"));
    }

    #[test]
    fn hint_anchor_uses_line_number_gutter_and_hides_when_scrolled_offscreen() {
        let value = format!("{}lower(", "-- line\n".repeat(30));
        let (mut ui, mut scene, area) = editor(&value);
        ui.set_text_area_line_numbers(area, true);
        let anchor = value.chars().count() - 1;
        ui.set_text_area_hint(area, anchor, "lower(arg1)");
        ui.sync(&mut scene);
        let (popup, bounds) = ui.text_area_hint_anchor(&scene).unwrap_or_else(|| {
            panic!(
                "area={:?}, caret={:?}, scroll={:?}",
                scene.get(ui.widgets[&area].node.unwrap()).unwrap().bounds,
                ui.ime_cursor_area(&scene),
                ui.text_area_scroll(area)
            )
        });
        let (x, y, _, _) = ui.ime_cursor_area(&scene).unwrap();
        assert_eq!(bounds.y, y);
        assert!(
            bounds.x < x
                && bounds.x
                    > ui.text_area_gutter_width(&value, TextAreaStyle::default(), true, 180.0)
        );
        let panel = scene.get(ui.widgets[&popup].aux[0]).unwrap().bounds;
        assert_eq!(
            ui.hit_widget(&scene, panel.x + 8.0, panel.y + 8.0),
            Some(area)
        );
        assert!(ui.pointer_scroll(&scene, panel.x + 8.0, panel.y + 8.0, 0.0, -20.0));
        ui.set_text_area_scroll(area, 0.0);
        ui.sync(&mut scene);
        assert!(ui.text_area_hint_anchor(&scene).is_none());
        assert!(ui.widgets[&popup]
            .aux
            .iter()
            .all(|n| !scene.get(*n).unwrap().visible));
    }
}
