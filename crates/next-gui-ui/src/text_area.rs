use super::*;
use std::ops::Range;

#[derive(Debug, Clone, Default)]
pub(super) struct TextAreaEdit {
    anchor: Option<usize>,
    undo: Vec<EditSnapshot>,
    redo: Vec<EditSnapshot>,
    pub(super) reveal: bool,
}

#[derive(Debug, Clone)]
struct EditSnapshot {
    value: Arc<str>,
    caret: usize,
    anchor: Option<usize>,
}

/// Opaque TextArea editing session, including selection, viewport and undo/redo.
/// Restore it only with the same document value; it is not a project file format.
#[derive(Debug, Clone)]
pub struct TextAreaState {
    snapshot: EditSnapshot,
    scroll_y: f32,
    viewport: (f32, f32),
    edit: TextAreaEdit,
}

fn selection(caret: usize, anchor: Option<usize>) -> Range<usize> {
    let anchor = anchor.unwrap_or(caret);
    anchor.min(caret)..anchor.max(caret)
}

fn byte_offset(value: &str, index: usize) -> usize {
    value
        .char_indices()
        .nth(index)
        .map_or(value.len(), |(i, _)| i)
}

fn remember(history: &mut Vec<EditSnapshot>, snapshot: EditSnapshot) {
    // shortcut: 100 edits / 8 MiB per stack (one oversized edit retained), use deltas for large documents.
    history.push(snapshot);
    let mut bytes: usize = history.iter().map(|s| s.value.len()).sum();
    while history.len() > 1 && (history.len() > 100 || bytes > 8 * 1024 * 1024) {
        bytes -= history.remove(0).value.len();
    }
}

pub(super) fn display_text(
    value: &str,
    caret: usize,
    edit: &TextAreaEdit,
    preedit: &str,
    cursor: Option<(usize, usize)>,
) -> (String, usize) {
    if preedit.is_empty() {
        return (value.to_owned(), caret);
    }
    let range = selection(caret, edit.anchor);
    let mut base = value.to_owned();
    base.replace_range(
        byte_offset(value, range.start)..byte_offset(value, range.end),
        "",
    );
    composed_text(&base, range.start, preedit, cursor)
}

fn nearest(positions: &[(f32, f32)], x: f32, y: f32, line_height: f32) -> usize {
    let row = (y / line_height).floor().max(0.0) * line_height;
    positions
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (a.1 - row)
                .abs()
                .total_cmp(&(b.1 - row).abs())
                .then_with(|| (a.0 - x).abs().total_cmp(&(b.0 - x).abs()))
        })
        .map_or(0, |(i, _)| i)
}

impl Ui {
    /// Capture selection, viewport and history for rebuilding a TextArea.
    pub fn text_area_state(&self, id: WidgetId) -> Option<TextAreaState> {
        let Kind::TextArea {
            value,
            caret,
            scroll_y,
            style,
            edit,
            ..
        } = &self.widgets.get(&id)?.kind
        else {
            return None;
        };
        Some(TextAreaState {
            snapshot: EditSnapshot {
                value: Arc::from(value.as_str()),
                caret: *caret,
                anchor: edit.anchor,
            },
            scroll_y: *scroll_y,
            viewport: (style.width, style.height),
            edit: edit.clone(),
        })
    }

    /// Restore a session. Returns false if the widget's document differs.
    pub fn restore_text_area_state(&mut self, id: WidgetId, state: &TextAreaState) -> bool {
        let Some(Widget {
            kind:
                Kind::TextArea {
                    value,
                    caret,
                    scroll_y,
                    style,
                    edit,
                    ..
                },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        if value.as_str() != state.snapshot.value.as_ref() {
            return false;
        }
        *caret = state.snapshot.caret;
        *scroll_y = state.scroll_y;
        *edit = state.edit.clone();
        edit.reveal |= state.viewport != (style.width, style.height);
        true
    }

    /// Selected Unicode scalar range (empty at the caret when nothing is selected).
    pub fn text_area_selection(&self, id: WidgetId) -> Option<Range<usize>> {
        let Kind::TextArea { caret, edit, .. } = &self.widgets.get(&id)?.kind else {
            return None;
        };
        Some(selection(*caret, edit.anchor))
    }

    /// Select Unicode scalar offsets, clamp to the document, and reveal the caret.
    pub fn select_text_area_range(&mut self, id: WidgetId, range: Range<usize>) -> bool {
        let Some(Widget {
            kind:
                Kind::TextArea {
                    value,
                    caret,
                    edit,
                    preedit,
                    preedit_cursor,
                    ..
                },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        let len = value.chars().count();
        edit.anchor = Some(range.start.min(len));
        *caret = range.end.min(len);
        edit.reveal = true;
        preedit.clear();
        *preedit_cursor = None;
        true
    }

    /// Selected text, or None for an empty selection or another widget kind.
    pub fn text_area_selected_text(&self, id: WidgetId) -> Option<String> {
        let range = self.text_area_selection(id)?;
        if range.is_empty() {
            return None;
        }
        Some(
            self.text_area_value(id)?
                .chars()
                .skip(range.start)
                .take(range.len())
                .collect(),
        )
    }

    /// Replace selection (or insert at caret) as one undoable edit. Empty text deletes selection.
    pub fn replace_text_area_selection(&mut self, id: WidgetId, text: &str) -> bool {
        let Some(Widget {
            kind:
                Kind::TextArea {
                    value,
                    caret,
                    edit,
                    preedit,
                    preedit_cursor,
                    ..
                },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        let range = selection(*caret, edit.anchor);
        let start = byte_offset(value, range.start);
        let end = byte_offset(value, range.end);
        if value[start..end] != *text {
            remember(
                &mut edit.undo,
                EditSnapshot {
                    value: Arc::from(value.as_str()),
                    caret: *caret,
                    anchor: edit.anchor,
                },
            );
            edit.redo.clear();
            value.replace_range(start..end, text);
        }
        *caret = range.start + text.chars().count();
        edit.anchor = None;
        edit.reveal = true;
        preedit.clear();
        *preedit_cursor = None;
        true
    }

    /// Find the next literal match after the current selection/caret, wrapping once.
    /// Offsets and selection remain Unicode scalar based; search is case sensitive.
    pub fn find_in_text_area(&mut self, id: WidgetId, needle: &str) -> bool {
        if needle.is_empty() {
            return false;
        }
        let Some(value) = self.text_area_value(id) else {
            return false;
        };
        let start = byte_offset(value, self.text_area_selection(id).unwrap().end);
        let found = value[start..]
            .find(needle)
            .map(|i| start + i)
            .or_else(|| value.find(needle));
        let Some(found) = found else { return false };
        let start = value[..found].chars().count();
        self.select_text_area_range(id, start..start + needle.chars().count())
    }

    /// TextArea vertical viewport offset in pixels.
    pub fn text_area_scroll(&self, id: WidgetId) -> Option<f32> {
        let Kind::TextArea { scroll_y, .. } = &self.widgets.get(&id)?.kind else {
            return None;
        };
        Some(*scroll_y)
    }

    /// Scroll without moving the caret, clamping to wrapped document height.
    pub fn set_text_area_scroll(&mut self, id: WidgetId, offset: f32) -> bool {
        if !offset.is_finite() {
            return false;
        }
        let Some(Widget {
            kind: Kind::TextArea { value, style, .. },
            ..
        }) = self.widgets.get(&id)
        else {
            return false;
        };
        let layout = self.font.layout(
            value,
            style.size,
            Some((style.width - 2.0 * style.padding_x).max(1.0)),
        );
        let offset = offset.clamp(
            0.0,
            (layout.height + 2.0 * style.padding_y - style.height).max(0.0),
        );
        let Kind::TextArea { scroll_y, edit, .. } = &mut self.widgets.get_mut(&id).unwrap().kind
        else {
            unreachable!()
        };
        edit.reveal = false;
        let changed = *scroll_y != offset;
        *scroll_y = offset;
        changed
    }

    pub(super) fn pointer_text_area(
        &mut self,
        id: WidgetId,
        bounds: Rect,
        x: f32,
        y: f32,
        extend: bool,
    ) -> bool {
        let Some(Widget {
            kind:
                Kind::TextArea {
                    value,
                    style,
                    scroll_y,
                    ..
                },
            ..
        }) = self.widgets.get(&id)
        else {
            return false;
        };
        let layout = self.font.layout(
            value,
            style.size,
            Some((bounds.width - 2.0 * style.padding_x).max(1.0)),
        );
        let points = layout.caret_positions(value, self.font.metrics(style.size).ascent);
        let target = nearest(
            &points,
            x - bounds.x - style.padding_x,
            y - bounds.y - style.padding_y + scroll_y,
            layout.line_height,
        );
        let Kind::TextArea {
            caret,
            edit,
            preedit,
            preedit_cursor,
            ..
        } = &mut self.widgets.get_mut(&id).unwrap().kind
        else {
            unreachable!()
        };
        if !extend {
            edit.anchor = Some(target);
        }
        *caret = target;
        edit.reveal = true;
        preedit.clear();
        *preedit_cursor = None;
        true
    }

    pub(super) fn key_text_area(&mut self, id: WidgetId, key: UiKey) -> bool {
        if let UiKey::Char(c) = key {
            return self.replace_text_area_selection(id, &c.to_string());
        }
        if key == UiKey::Enter {
            return self.replace_text_area_selection(id, "\n");
        }
        let Some(Widget {
            kind:
                Kind::TextArea {
                    value,
                    caret,
                    edit,
                    style,
                    ..
                },
            ..
        }) = self.widgets.get(&id)
        else {
            return false;
        };
        let len = value.chars().count();
        let range = selection(*caret, edit.anchor);
        let old = *caret;
        let layout = self.font.layout(
            value,
            style.size,
            Some((style.width - style.padding_x * 2.0).max(1.0)),
        );
        let points = layout.caret_positions(value, self.font.metrics(style.size).ascent);
        let (x, y) = points[old.min(len)];
        let extend = matches!(
            key,
            UiKey::SelectLeft
                | UiKey::SelectRight
                | UiKey::SelectUp
                | UiKey::SelectDown
                | UiKey::SelectHome
                | UiKey::SelectEnd
        );
        let target = match key {
            UiKey::SelectAll => return self.select_text_area_range(id, 0..len),
            UiKey::Backspace | UiKey::Delete => {
                if range.is_empty() {
                    let range = if key == UiKey::Backspace {
                        old.saturating_sub(1)..old
                    } else {
                        old..(old + 1).min(len)
                    };
                    self.select_text_area_range(id, range);
                }
                return self.replace_text_area_selection(id, "");
            }
            UiKey::Undo | UiKey::Redo => {
                let Kind::TextArea {
                    value,
                    caret,
                    edit,
                    preedit,
                    preedit_cursor,
                    ..
                } = &mut self.widgets.get_mut(&id).unwrap().kind
                else {
                    unreachable!()
                };
                let (from, to) = if key == UiKey::Undo {
                    (&mut edit.undo, &mut edit.redo)
                } else {
                    (&mut edit.redo, &mut edit.undo)
                };
                if let Some(snapshot) = from.pop() {
                    remember(
                        to,
                        EditSnapshot {
                            value: Arc::from(value.as_str()),
                            caret: *caret,
                            anchor: edit.anchor,
                        },
                    );
                    *value = snapshot.value.to_string();
                    *caret = snapshot.caret;
                    edit.anchor = snapshot.anchor;
                    edit.reveal = true;
                    preedit.clear();
                    *preedit_cursor = None;
                }
                return true;
            }
            UiKey::Left if !range.is_empty() => range.start,
            UiKey::Right if !range.is_empty() => range.end,
            UiKey::Left | UiKey::SelectLeft => old.saturating_sub(1),
            UiKey::Right | UiKey::SelectRight => (old + 1).min(len),
            UiKey::Up | UiKey::SelectUp => nearest(
                &points,
                x,
                (y - layout.line_height).max(0.0),
                layout.line_height,
            ),
            UiKey::Down | UiKey::SelectDown => {
                nearest(&points, x, y + layout.line_height, layout.line_height)
            }
            UiKey::Home | UiKey::SelectHome => nearest(&points, 0.0, y, layout.line_height),
            UiKey::End | UiKey::SelectEnd => {
                nearest(&points, layout.width + 1.0, y, layout.line_height)
            }
            _ => return false,
        };
        let Kind::TextArea {
            caret,
            edit,
            preedit,
            preedit_cursor,
            ..
        } = &mut self.widgets.get_mut(&id).unwrap().kind
        else {
            unreachable!()
        };
        if extend {
            edit.anchor.get_or_insert(old);
        } else {
            edit.anchor = None;
        }
        *caret = target;
        edit.reveal = true;
        preedit.clear();
        *preedit_cursor = None;
        true
    }

    pub(super) fn paint_text_area(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::TextArea {
                    value,
                    placeholder,
                    caret,
                    preedit,
                    preedit_cursor,
                    scroll_y,
                    style,
                    edit,
                },
            node,
            label_node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let Some(host) = *node else { return };
        let bounds = scene
            .get(host)
            .map(|n| n.bounds)
            .unwrap_or_else(Rect::empty);
        let focused = self.focus == Some(id);
        let range = if preedit.is_empty() {
            selection(*caret, edit.anchor)
        } else {
            0..0
        };
        let (value, caret) = display_text(value, *caret, edit, preedit, *preedit_cursor);
        let placeholder = placeholder.clone();
        let style = *style;
        let (label, chrome) = (*label_node, *chrome_node);
        let layout = self.font.layout(
            &value,
            style.size,
            Some((bounds.width - style.padding_x * 2.0).max(1.0)),
        );
        let points = layout.caret_positions(&value, self.font.metrics(style.size).ascent);
        let (_, cy) = points[caret.min(points.len() - 1)];
        let view_h = (bounds.height - 2.0 * style.padding_y).max(1.0);
        let max_scroll = (layout.height - view_h).max(0.0);
        let mut scroll = scroll_y.clamp(0.0, max_scroll);
        if edit.reveal {
            if cy < scroll {
                scroll = cy;
            }
            if cy + layout.line_height > scroll + view_h {
                scroll = (cy + layout.line_height - view_h).min(max_scroll);
            }
        }
        if let Kind::TextArea { scroll_y, edit, .. } = &mut self.widgets.get_mut(&id).unwrap().kind
        {
            *scroll_y = scroll;
            edit.reveal = false;
        }
        let content = Rect::new(
            bounds.x + style.padding_x,
            bounds.y + style.padding_y,
            (bounds.width - style.padding_x * 2.0).max(1.0),
            view_h,
        );
        scene.set_fill(host, style.bg);
        scene.set_clip(host, Some(bounds));
        scene.set_visible(host, true);
        self.apply_corner_radius(scene, host, false);
        // Allocate only visible selection rows; text uses the same wrapped layout as hit testing.
        let first = (scroll / layout.line_height).floor() as usize;
        let last = ((scroll + view_h) / layout.line_height).ceil() as usize;
        let mut highlights: Vec<Option<(f32, f32)>> = vec![None; last - first + 1];
        for i in range.start..range.end.min(points.len().saturating_sub(1)) {
            let (x, y) = points[i];
            let (nx, ny) = points[i + 1];
            let row = (y / layout.line_height).round() as usize;
            if row < first || row > last {
                continue;
            }
            let end = if ny > y {
                content.width
            } else {
                nx.max(x + 2.0)
            };
            let entry = &mut highlights[row - first];
            *entry = Some(entry.map_or((x, end), |(a, b)| (a.min(x), b.max(end))));
        }
        self.ensure_aux_nodes(scene, host, id, highlights.len());
        let aux = self.widgets.get(&id).unwrap().aux.clone();
        for (i, extent) in highlights.iter().enumerate() {
            let n = aux[i];
            scene.set_visible(n, extent.is_some());
            if let Some((a, b)) = extent {
                let left = (content.x + a).floor();
                let top = (content.y + (first + i) as f32 * layout.line_height - scroll).floor();
                let width = (content.x + b).ceil() - left;
                let height = layout.line_height.ceil();
                scene.set_bounds(n, Rect::new(left, top, width, height));
                scene.set_clip(n, Some(content));
                // Solid fills are opaque; coverage gives both renderers the same translucent selection.
                scene.set_mask(
                    n,
                    Some(AlphaMask::new(
                        width as u32,
                        height as u32,
                        vec![100; width as usize * height as usize],
                    )),
                );
                scene.set_fill(n, style.focus_border);
            }
        }
        if let Some(label) = label {
            let placeholder_visible = value.is_empty() && !focused;
            let mut visible_layout = if placeholder_visible {
                self.font
                    .layout(&placeholder, style.size, Some(content.width))
            } else {
                layout.clone()
            };
            let top = first as f32 * layout.line_height;
            visible_layout.glyphs.retain(|g| {
                g.baseline_y >= top && g.baseline_y - layout.line_height < scroll + view_h
            });
            for g in &mut visible_layout.glyphs {
                g.baseline_y -= top;
            }
            visible_layout.height = view_h + layout.line_height;
            let mask = self.font.rasterize_layout(&visible_layout, style.size);
            scene.set_bounds(
                label,
                Rect::new(
                    content.x,
                    content.y + top - scroll,
                    mask.width as f32,
                    mask.height as f32,
                ),
            );
            scene.set_clip(label, Some(content));
            scene.set_fill(
                label,
                if placeholder_visible {
                    style.placeholder_fg
                } else {
                    style.fg
                },
            );
            scene.set_mask(
                label,
                Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
            );
            scene.set_visible(label, true);
            scene.raise(label);
        }
        if let Some(chrome) = chrome {
            let (x, y) = points[caret.min(points.len() - 1)];
            scene.set_bounds(
                chrome,
                Rect::new(
                    (content.x + x).floor(),
                    (content.y + y - scroll).floor(),
                    2.0,
                    self.font.metrics(style.size).ascent.ceil(),
                ),
            );
            scene.set_clip(chrome, Some(content));
            scene.set_fill(chrome, style.focus_border);
            scene.set_visible(chrome, focused);
            scene.raise(chrome);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(text: &str) -> (Ui, Scene, WidgetId) {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut area = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 400.0,
                height: 220.0,
                ..ColumnStyle::default()
            },
            |c| {
                area = c.text_area(
                    text,
                    "SQL",
                    TextAreaStyle {
                        width: 320.0,
                        height: 90.0,
                        ..TextAreaStyle::default()
                    },
                );
            },
        );
        ui.set_focus(Some(area));
        ui.sync(&mut scene);
        (ui, scene, area)
    }

    #[test]
    fn unicode_selection_clipboard_ime_and_history_survive_rebuild() {
        let original = "SELECT '中文🙂';\nSELECT 2;";
        let (mut ui, mut scene, area) = editor(original);
        assert_eq!(ui.focused_text_for_copy(), None);
        ui.select_text_area_range(area, 8..11);
        assert_eq!(ui.focused_text_for_copy().as_deref(), Some("中文🙂"));
        ui.ime_preedit("東京", Some((6, 6)));
        ui.sync(&mut scene);
        assert_eq!(ui.text_area_value(area), Some(original));
        let preview_cursor = ui.ime_cursor_area(&scene).unwrap();
        let chrome = ui.widgets[&area].chrome_node.unwrap();
        let drawn = scene.get(chrome).unwrap().bounds;
        assert!((drawn.x - preview_cursor.0).abs() < 0.5);
        assert!((drawn.y - preview_cursor.1).abs() < 0.5);
        ui.ime_commit("東京");
        assert_eq!(ui.text_area_value(area), Some("SELECT '東京';\nSELECT 2;"));
        assert!(ui.key_input(UiKey::Undo));
        assert_eq!(ui.text_area_value(area), Some(original));
        assert_eq!(ui.text_area_selected_text(area).as_deref(), Some("中文🙂"));
        ui.key_input(UiKey::Redo);
        let state = ui.text_area_state(area).unwrap();
        let value = ui.text_area_value(area).unwrap().to_owned();
        let (mut next, _, next_area) = editor(&value);
        assert!(next.restore_text_area_state(next_area, &state));
        next.key_input(UiKey::Undo);
        assert_eq!(next.text_area_value(next_area), Some(original));
        next.key_input(UiKey::SelectAll);
        next.paste_into_focused_text("再开\n");
        assert_eq!(next.text_area_value(next_area), Some("再开\n"));
        next.key_input(UiKey::Redo);
        assert_eq!(
            next.text_area_value(next_area),
            Some("再开\n"),
            "new edits invalidate redo"
        );
        assert!(
            !next.restore_text_area_state(next_area, &state),
            "never restore history from another document"
        );
    }

    #[test]
    fn pointer_drag_wrap_navigation_scroll_and_clip_use_same_layout() {
        let text = format!("中文 {}\n{}", "W".repeat(55), "SELECT 42;\n".repeat(80));
        let (mut ui, mut scene, area) = editor(&text);
        let host = ui.widgets[&area].node.unwrap();
        let bounds = scene.get(host).unwrap().bounds;
        let Kind::TextArea { style, .. } = ui.widgets[&area].kind else {
            panic!()
        };
        let layout = ui.font.layout(
            &text,
            style.size,
            Some(bounds.width - style.padding_x * 2.0),
        );
        let points = layout.caret_positions(&text, ui.font.metrics(style.size).ascent);
        let (x, y) = (bounds.x + style.padding_x, bounds.y + style.padding_y);
        assert!(ui.pointer_pressed(&mut scene, x + 0.1, y + 2.0));
        assert_eq!(ui.text_area_caret(area), Some(0));
        ui.pointer_moved(x + points[8].0, y + points[8].1 + 2.0);
        ui.pointer_released();
        assert_eq!(ui.text_area_selection(area), Some(0..8));
        ui.key_input(UiKey::Right);
        assert_eq!(ui.text_area_selection(area), Some(8..8));
        ui.key_input(UiKey::SelectDown);
        assert!(!ui.text_area_selection(area).unwrap().is_empty());
        assert!(points[ui.text_area_caret(area).unwrap()].1 > points[8].1);
        ui.sync(&mut scene);
        assert!(ui.pointer_scroll(&scene, x + 5.0, y + 5.0, 0.0, 200.0));
        ui.sync(&mut scene);
        assert_eq!(
            ui.text_area_scroll(area),
            Some(200.0),
            "paint must not undo wheel scrolling"
        );
        let caret_before = ui.text_area_caret(area);
        ui.pointer_pressed(&mut scene, x + 0.1, y + 2.0);
        ui.pointer_released();
        assert!(ui.text_area_caret(area) > caret_before);
        ui.select_text_area_range(area, text.chars().count()..text.chars().count());
        ui.sync(&mut scene);
        let widget = &ui.widgets[&area];
        let drawn = scene.get(widget.chrome_node.unwrap()).unwrap().bounds;
        assert!(drawn.y >= bounds.y && drawn.y + drawn.height <= bounds.y + bounds.height);
        let mask = scene
            .get(widget.label_node.unwrap())
            .unwrap()
            .mask
            .as_ref()
            .unwrap();
        assert!(
            (mask.height as f32) < style.height + layout.line_height * 3.0,
            "rasterize viewport, not all 80 lines"
        );
        assert!(!ui.set_text_area_scroll(area, f32::NAN));
        assert!(ui.set_text_area_scroll(area, 0.0));
        ui.key_input(UiKey::SelectAll);
        ui.sync(&mut scene);
        assert!(
            ui.widgets[&area].aux.len() < 15,
            "selection nodes are viewport bounded"
        );
    }

    #[test]
    fn literal_find_selects_unicode_match_and_replace_undoes_exact_match() {
        let (mut ui, _, area) = editor("中文 one 中文 two");
        ui.select_text_area_range(area, 0..0);
        assert!(ui.find_in_text_area(area, "中文"));
        assert_eq!(ui.text_area_selection(area), Some(0..2));
        assert!(ui.find_in_text_area(area, "中文"));
        assert_eq!(ui.text_area_selection(area), Some(7..9));
        ui.replace_text_area_selection(area, "東京");
        assert_eq!(ui.text_area_value(area), Some("中文 one 東京 two"));
        ui.key_input(UiKey::Undo);
        assert_eq!(ui.text_area_value(area), Some("中文 one 中文 two"));
        assert!(ui.find_in_text_area(area, "中文"));
        assert_eq!(ui.text_area_selection(area), Some(0..2));
        assert!(!ui.find_in_text_area(area, ""));
        assert!(!ui.find_in_text_area(area, "absent"));
    }
}
