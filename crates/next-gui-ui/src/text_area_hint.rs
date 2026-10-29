use super::*;

#[derive(Debug, Clone)]
pub(super) struct HintState {
    anchor: usize,
    prefix: String,
    preferred: usize,
    page: usize,
    scroll_y: f32,
    dismissed: bool,
}

#[derive(Debug)]
pub(super) struct TextAreaHint {
    pub(super) popup: WidgetId,
    owner: WidgetId,
    document: String,
    caret: usize,
    anchor: usize,
    dismissed: bool,
    items: Vec<String>,
    page: usize,
    scroll_y: f32,
    max_scroll: f32,
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
        self.set_text_area_hints(
            id,
            anchor,
            if text.is_empty() { vec![] } else { vec![text] },
            0,
        )
    }

    /// Show alternative hints with previous/next controls, keeping editor focus.
    /// `preferred` is a zero-based initial page, clamped to the supplied items.
    /// Manual choice/scroll survives refresh and matching editor sessions until the
    /// anchor/prefix or preferred page changes. Empty items clear the hint. Wheel
    /// scrolls clipped hint text; single hints otherwise pass pointer input through.
    pub fn set_text_area_hints(
        &mut self,
        id: WidgetId,
        anchor: usize,
        items: Vec<String>,
        preferred: usize,
    ) -> bool {
        if items.is_empty() || items.iter().any(String::is_empty) {
            if let Some(Widget {
                kind: Kind::TextArea { edit, .. },
                ..
            }) = self.widgets.get_mut(&id)
            {
                edit.hint = None;
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
        let previous = edit
            .hint
            .as_ref()
            .filter(|h| h.anchor == anchor && h.prefix == value[..prefix_end]);
        let dismissed = previous.is_some_and(|h| h.dismissed);
        let preferred = preferred.min(items.len() - 1);
        let retained = previous.filter(|h| h.preferred == preferred);
        let page = retained.map_or(preferred, |h| h.page.min(items.len() - 1));
        let scroll_y = retained.map_or(0.0, |h| h.scroll_y);
        let prefix = value[..prefix_end].to_owned();
        let document = value.clone();
        let caret = *caret;
        let editor_size = style.size;
        if let Kind::TextArea { edit, .. } = &mut self.widgets.get_mut(&id).unwrap().kind {
            edit.hint = Some(HintState {
                anchor,
                prefix,
                preferred,
                page,
                scroll_y,
                dismissed,
            });
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
                        content_owner: None,
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
        if let Kind::Tooltip {
            text: current,
            style,
            ..
        } = &mut self.widgets.get_mut(&popup).unwrap().kind
        {
            *current = items[page].clone();
            *style = hint_style;
        }
        self.text_area_hint = Some(TextAreaHint {
            popup,
            owner: id,
            document,
            caret,
            anchor,
            dismissed,
            items,
            page,
            scroll_y,
            max_scroll: 0.0,
        });
        self.update_text_area_hint_name();
        self.text_area_hint(id).is_some()
    }

    /// Current zero-based hint page and total count, if the editor's hint is valid.
    pub fn text_area_hint_page(&self, id: WidgetId) -> Option<(usize, usize)> {
        self.text_area_hint(id)?;
        let h = self.text_area_hint.as_ref()?;
        Some((h.page, h.items.len()))
    }

    /// Cycle alternatives without changing the document, selection, focus or Undo.
    /// Returns false for hidden/invalid hints or a single item. Call [`Ui::sync`].
    pub fn cycle_text_area_hint(&mut self, id: WidgetId, forward: bool) -> bool {
        let Some((page, count)) = self.text_area_hint_page(id).filter(|(_, n)| *n > 1) else {
            return false;
        };
        let page = if forward {
            (page + 1) % count
        } else {
            (page + count - 1) % count
        };
        let h = self.text_area_hint.as_mut().unwrap();
        h.page = page;
        h.scroll_y = 0.0;
        h.max_scroll = 0.0;
        if let Kind::Tooltip { text, .. } = &mut self.widgets.get_mut(&h.popup).unwrap().kind {
            *text = h.items[page].clone();
        }
        if let Kind::TextArea { edit, .. } = &mut self.widgets.get_mut(&id).unwrap().kind {
            if let Some(state) = &mut edit.hint {
                state.page = page;
                state.scroll_y = 0.0;
            }
        }
        self.update_text_area_hint_name();
        true
    }

    /// Scroll clipped hint text by pixels; returns false for an invalid or fitting
    /// hint/non-finite delta. Layout establishes the scroll bounds during [`Ui::sync`].
    pub fn scroll_text_area_hint(&mut self, id: WidgetId, delta: f32) -> bool {
        if !delta.is_finite() || self.text_area_hint(id).is_none() {
            return false;
        }
        let h = self.text_area_hint.as_mut().unwrap();
        if h.max_scroll <= 0.0 {
            return false;
        }
        h.scroll_y = (h.scroll_y + delta).clamp(0.0, h.max_scroll);
        if let Kind::TextArea { edit, .. } = &mut self.widgets.get_mut(&id).unwrap().kind {
            if let Some(state) = &mut edit.hint {
                state.scroll_y = h.scroll_y;
            }
        }
        true
    }

    fn update_text_area_hint_name(&mut self) {
        let h = self.text_area_hint.as_ref().unwrap();
        let text = if h.items.len() > 1 {
            format!(
                "Hint {} of {}. {}",
                h.page + 1,
                h.items.len(),
                h.items[h.page]
            )
        } else {
            h.items[h.page].clone()
        };
        self.set_accessible_name(h.popup, text);
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
            if let Some(state) = &mut edit.hint {
                state.dismissed = true;
            }
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

    pub(super) fn pointer_text_area_hint(
        &mut self,
        scene: &mut Scene,
        x: f32,
        y: f32,
        secondary: bool,
    ) -> bool {
        let Some(h) = &self.text_area_hint else {
            return false;
        };
        if self.text_area_hint(h.owner).is_none() {
            return false;
        }
        let owner = h.owner;
        let popup = h.popup;
        if !secondary && h.items.len() > 1 {
            for (slot, forward) in [(2, false), (4, true)] {
                if self.widgets[&popup]
                    .aux
                    .get(slot)
                    .and_then(|n| scene.get(*n))
                    .is_some_and(|n| n.visible && n.bounds.contains(x, y))
                {
                    return self.cycle_text_area_hint(owner, forward);
                }
            }
        }
        let inside = self.widgets[&popup]
            .aux
            .first()
            .and_then(|n| scene.get(*n))
            .is_some_and(|n| n.visible && n.bounds.contains(x, y));
        if inside {
            if !secondary && (h.items.len() > 1 || h.max_scroll > 0.0) {
                return true;
            }
            self.dismiss_text_area_hint();
        }
        // Passive overlays must not absorb the editor's pointer hit.
        self.hide_popup_aux_mut(scene, popup);
        false
    }

    pub(super) fn wheel_text_area_hint(&mut self, scene: &Scene, x: f32, y: f32, dy: f32) -> bool {
        let Some(h) = &self.text_area_hint else {
            return false;
        };
        let inside = self.widgets[&h.popup]
            .aux
            .first()
            .and_then(|n| scene.get(*n))
            .is_some_and(|n| n.visible && n.bounds.contains(x, y));
        inside && self.scroll_text_area_hint(h.owner, dy)
    }

    pub(super) fn text_area_hint_header_size(&self, id: WidgetId, size: f32) -> (f32, f32) {
        let Some(h) = self
            .text_area_hint
            .as_ref()
            .filter(|h| h.popup == id && h.items.len() > 1)
        else {
            return (0.0, 0.0);
        };
        let height = (28.0 * self.config.density.spacing_scale()).max(size + 8.0);
        let counter = format!("{} / {}", h.page + 1, h.items.len());
        (
            height,
            self.font.measure_line(&counter, size).0 + height * 2.0 + 16.0,
        )
    }

    pub(super) fn bound_text_area_hint_scroll(&mut self, id: WidgetId, max_scroll: f32) -> f32 {
        let Some(h) = self.text_area_hint.as_mut().filter(|h| h.popup == id) else {
            return 0.0;
        };
        // Floating layout subtraction can leave subpixel overflow on a fitting hint.
        let max_scroll = if max_scroll > 0.5 { max_scroll } else { 0.0 };
        h.max_scroll = max_scroll;
        h.scroll_y = h.scroll_y.min(max_scroll);
        if let Kind::TextArea { edit, .. } = &mut self.widgets.get_mut(&h.owner).unwrap().kind {
            if let Some(state) = &mut edit.hint {
                state.scroll_y = h.scroll_y;
            }
        }
        h.scroll_y
    }

    pub(super) fn paint_text_area_hint_controls(
        &mut self,
        scene: &mut Scene,
        id: WidgetId,
        tip: Rect,
        padding: f32,
        header: f32,
        text_height: f32,
    ) {
        let Some(h) = self.text_area_hint.as_ref().filter(|h| h.popup == id) else {
            return;
        };
        let (page, count, scroll_y, max_scroll) = (h.page, h.items.len(), h.scroll_y, h.max_scroll);
        self.ensure_overlay_aux(scene, id, 8);
        let aux = self.widgets[&id].aux.clone();
        for n in &aux[2..] {
            scene.set_visible(*n, false);
        }
        let size = match self.widgets[&id].kind {
            Kind::Tooltip { style, .. } => style.size,
            _ => return,
        };
        let colors = self.theme.colors;
        if count > 1 {
            let button_width = header.min(((tip.width - padding * 2.0) / 3.0).max(0.0));
            for (slot, x, label) in [
                (2, tip.x + padding, "‹"),
                (4, tip.x + tip.width - padding - button_width, "›"),
            ] {
                let bounds = Rect::new(x, tip.y + padding, button_width, header);
                scene.set_bounds(aux[slot], bounds);
                scene.set_fill(aux[slot], colors.button_bg);
                scene.set_mask(aux[slot], None);
                scene.set_clip(aux[slot], Some(tip));
                scene.set_visible(aux[slot], true);
                self.apply_corner_radius(scene, aux[slot], false);
                let (_, mask) = self.font.rasterize_text(label, size, None);
                scene.set_bounds(
                    aux[slot + 1],
                    Rect::new(
                        x + (button_width - mask.width as f32) * 0.5,
                        bounds.y + (header - mask.height as f32) * 0.5,
                        mask.width as f32,
                        mask.height as f32,
                    ),
                );
                scene.set_clip(aux[slot + 1], Some(bounds));
                scene.set_fill(aux[slot + 1], colors.button_fg);
                scene.set_mask(
                    aux[slot + 1],
                    Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                );
                scene.set_visible(aux[slot + 1], true);
            }
            let counter = format!("{} / {}", page + 1, count);
            let (_, mask) = self.font.rasterize_text(&counter, size, None);
            let bounds = Rect::new(
                tip.x + padding + button_width,
                tip.y + padding,
                (tip.width - padding * 2.0 - button_width * 2.0).max(0.0),
                header,
            );
            scene.set_bounds(
                aux[6],
                Rect::new(
                    bounds.x + (bounds.width - mask.width as f32) * 0.5,
                    bounds.y + (header - mask.height as f32) * 0.5,
                    mask.width as f32,
                    mask.height as f32,
                ),
            );
            scene.set_clip(aux[6], Some(bounds));
            scene.set_fill(aux[6], colors.text_secondary);
            scene.set_mask(
                aux[6],
                Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
            );
            scene.set_visible(aux[6], true);
        }
        if max_scroll > 0.0 {
            let height = (tip.height - padding * 2.0 - header).max(0.0);
            let thumb = (height * height / text_height).max(4.0).min(height);
            scene.set_bounds(
                aux[7],
                Rect::new(
                    (tip.x + tip.width - 3.0).max(tip.x),
                    tip.y + padding + header + (height - thumb) * scroll_y / max_scroll,
                    2.0_f32.min(tip.width),
                    thumb,
                ),
            );
            scene.set_fill(aux[7], colors.text_secondary);
            scene.set_mask(aux[7], None);
            scene.set_clip(aux[7], Some(tip));
            scene.set_visible(aux[7], true);
        }
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
        let (gutter,layout)=self.text_area_layout(&display,*style,*line_numbers,edit.no_wrap,bounds.width);
        let (x, y) = layout.caret_top_left(&display, caret, self.font.metrics(style.size).ascent);
        Some((
            (bounds.x + style.padding_x + gutter + x - edit.scroll_x).floor(),
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
    fn hint_pages_keep_manual_choice_through_refresh_and_editor_sessions() {
        let (mut ui, _, area) = editor("func(");
        let items = vec![
            "func(a)".to_owned(),
            "func(a, b)".to_owned(),
            "func(a, b, c)".to_owned(),
        ];
        assert!(ui.set_text_area_hints(area, 4, items.clone(), 1));
        assert_eq!(ui.text_area_hint_page(area), Some((1, 3)));
        assert!(ui.cycle_text_area_hint(area, true));
        assert_eq!(ui.text_area_hint(area), Some("func(a, b, c)"));
        let state = ui.text_area_state(area).unwrap();
        let (mut rebuilt, _, other) = editor("func(");
        assert!(rebuilt.restore_text_area_state(other, &state));
        rebuilt.set_text_area_hints(other, 4, items.clone(), 1);
        assert_eq!(rebuilt.text_area_hint_page(other), Some((2, 3)));
        assert!(rebuilt.cycle_text_area_hint(other, true));
        assert_eq!(rebuilt.text_area_hint_page(other), Some((0, 3)));
        assert!(rebuilt.cycle_text_area_hint(other, false));
        rebuilt.key_input(UiKey::Char('1'));
        rebuilt.set_text_area_hints(other, 4, items.clone(), 1);
        assert_eq!(rebuilt.text_area_hint_page(other), Some((2, 3)));
        rebuilt.set_text_area_hints(other, 4, items.clone(), 0);
        assert_eq!(rebuilt.text_area_hint_page(other), Some((0, 3)));
        rebuilt.key_input(UiKey::Undo);
        assert_eq!(rebuilt.text_area_value(other), Some("func("));
        rebuilt.set_text_area_hints(other, 4, items.clone(), 0);
        rebuilt.key_input(UiKey::Escape);
        assert!(!rebuilt.cycle_text_area_hint(other, true));
        assert!(!rebuilt.set_text_area_hints(other, 4, items, 2));
        assert_eq!(rebuilt.focused(), Some(other));
    }

    #[test]
    fn hint_arrow_hits_preserve_the_caret_and_disappear_for_single_or_hidden_hints() {
        let (mut ui, mut scene, area) = editor("func(");
        ui.set_text_area_hints(area, 4, vec!["first".into(), "second".into()], 0);
        ui.sync(&mut scene);
        let popup = ui.text_area_hint.as_ref().unwrap().popup;
        let next = scene.get(ui.widgets[&popup].aux[4]).unwrap().bounds;
        assert!(next.width >= 24.0 && next.height >= 24.0);
        assert!(ui.pointer_pressed(
            &mut scene,
            next.x + next.width * 0.5,
            next.y + next.height * 0.5
        ));
        assert_eq!(ui.text_area_hint_page(area), Some((1, 2)));
        assert_eq!(ui.text_area_selection(area), Some(5..5));
        assert_eq!(ui.focused(), Some(area));
        assert!(ui
            .accessible_name(popup)
            .unwrap()
            .contains("Hint 2 of 2. second"));
        assert_eq!(ui.take_button_activate(), None);
        let menu = ui.context_menu_popup(ui.context_menu_style());
        ui.open_context_menu(menu, vec![MenuItem::new(1, "Other")], 0.0, 0.0);
        assert!(!ui.cycle_text_area_hint(area, false));
        ui.sync(&mut scene);
        assert!(ui.widgets[&popup]
            .aux
            .iter()
            .all(|n| !scene.get(*n).unwrap().visible));
        ui.close_popup();
        ui.set_text_area_hint(area, 4, "single");
        ui.sync(&mut scene);
        assert!(ui.widgets[&popup].aux[2..]
            .iter()
            .all(|n| !scene.get(*n).unwrap().visible));
        assert!(!ui.cycle_text_area_hint(area, true));
        ui.set_text_area_hints(area, 4, vec![], 0);
        ui.sync(&mut scene);
        assert_eq!(ui.text_area_hint_page(area), None);
    }

    #[test]
    fn clipped_hint_text_scrolls_to_the_end_without_scrolling_or_editing_its_owner() {
        let (mut ui, mut scene, area) = editor("func(");
        let text = format!("{}last help line", "中文 long parameter help\n".repeat(40));
        ui.set_text_area_hint(area, 4, text.clone());
        ui.sync(&mut scene);
        let popup = ui.text_area_hint.as_ref().unwrap().popup;
        let aux = ui.widgets[&popup].aux.clone();
        let panel = scene.get(aux[0]).unwrap().bounds;
        let old_y = scene.get(aux[1]).unwrap().bounds.y;
        let old_scroll = ui.text_area_scroll(area);
        assert!(ui.pointer_scroll(&scene, panel.x + 10.0, panel.y + 10.0, 0.0, 50.0));
        ui.sync(&mut scene);
        assert_eq!(scene.get(aux[1]).unwrap().bounds.y, old_y - 50.0);
        assert!(!ui.scroll_text_area_hint(area, f32::INFINITY));
        assert!(ui.scroll_text_area_hint(area, f32::MAX));
        ui.sync(&mut scene);
        let label = scene.get(aux[1]).unwrap();
        let clip = label.clip.unwrap();
        assert!((label.bounds.y + label.bounds.height - clip.y - clip.height).abs() <= 2.0);
        assert!(scene.get(aux[7]).unwrap().visible);
        assert_eq!(ui.text_area_scroll(area), old_scroll);
        assert_eq!(ui.text_area_selection(area), Some(5..5));
        assert_eq!(ui.text_area_value(area), Some("func("));
        let state = ui.text_area_state(area).unwrap();
        let (mut rebuilt, mut other_scene, other) = editor("func(");
        rebuilt.restore_text_area_state(other, &state);
        rebuilt.set_text_area_hint(other, 4, text);
        rebuilt.sync(&mut other_scene);
        let h = rebuilt.text_area_hint.as_ref().unwrap();
        assert_eq!(h.scroll_y, h.max_scroll);
        rebuilt.set_text_area_hint(other, 4, "short");
        rebuilt.sync(&mut other_scene);
        assert!(!rebuilt.scroll_text_area_hint(other, 10.0));
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
