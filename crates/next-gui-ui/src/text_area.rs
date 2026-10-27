use super::*;
use std::ops::Range;

#[derive(Debug, Clone, Default)]
pub(super) struct TextAreaEdit {
    anchor: Option<usize>,
    goal_x: Option<f32>,
    undo: Vec<EditSnapshot>,
    redo: Vec<EditSnapshot>,
    pub(super) reveal: bool,
    highlights: Option<(Color, Vec<Range<usize>>)>,
    foregrounds: Vec<(Range<usize>, Color)>,
    pub(super) hint_dismissed: Option<(usize, String)>,
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
    font_size: f32,
    line_numbers: bool,
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
    nearest_on_row(positions, x, (y / line_height).floor().max(0.0) * line_height)
}

fn nearest_on_row(positions: &[(f32, f32)], x: f32, row: f32) -> usize {
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
            line_numbers,
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
            font_size: style.size,
            line_numbers: *line_numbers,
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
                    line_numbers,
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
        edit.reveal |= state.viewport != (style.width, style.height) || state.font_size != style.size || state.line_numbers != *line_numbers;
        if state.viewport.0 != style.width || state.font_size != style.size || state.line_numbers != *line_numbers {
            edit.goal_x = None;
        }
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
        edit.goal_x = None;
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

    /// Allow selection/copy/navigation while blocking edits and IME. Programmatic value setters still work.
    pub fn set_text_area_read_only(&mut self, id: WidgetId, read_only: bool) -> bool {
        let Some(Widget {
            kind: Kind::TextArea { read_only: current, preedit, preedit_cursor, .. },
            ..
        }) = self.widgets.get_mut(&id) else { return false; };
        *current = read_only;
        if read_only {
            preedit.clear();
            *preedit_cursor = None;
        }
        true
    }

    /// Show physical line numbers in a gutter. Keeps the document, selection and undo history.
    pub fn set_text_area_line_numbers(&mut self, id: WidgetId, enabled: bool) -> bool {
        let Some(Widget { kind: Kind::TextArea { line_numbers, edit, .. }, .. }) = self.widgets.get_mut(&id) else { return false; };
        if *line_numbers != enabled {
            *line_numbers = enabled;
            edit.goal_x = None;
            edit.reveal = true;
        }
        true
    }

    pub(super) fn text_area_gutter_width(&self, value: &str, style: TextAreaStyle, enabled: bool, width: f32) -> f32 {
        if !enabled { return 0.0; }
        let lines = value.bytes().filter(|b| *b == b'\n').count() + 1;
        let digits = lines.to_string().len() + 1;
        self.font.measure_line(&"0".repeat(digits), style.size).0
            .min((width - 2.0 * style.padding_x - 1.0).max(0.0))
    }

    /// Mark disjoint Unicode scalar ranges without changing selection or undo history.
    /// Empty ranges mark a position. Document edits clear the marks; sessions retain them.
    pub fn set_text_area_highlights(&mut self, id: WidgetId, mut ranges: Vec<Range<usize>>, color: Color) -> bool {
        let Some(Widget { kind: Kind::TextArea { value, edit, .. }, .. }) = self.widgets.get_mut(&id) else {
            return false;
        };
        let len = value.chars().count();
        if ranges.iter().any(|range| range.start > range.end || range.end > len) { return false; }
        ranges.sort_by_key(|range| (range.start, range.end));
        let mut merged: Vec<Range<usize>> = Vec::new();
        for range in ranges {
            if merged.last() == Some(&range) { continue; }
            if let Some(last) = merged.last_mut().filter(|last| last.start < last.end && range.start < range.end && range.start <= last.end) {
                last.end = last.end.max(range.end);
            } else { merged.push(range); }
        }
        edit.highlights = (!merged.is_empty()).then_some((color, merged));
        true
    }

    /// Color disjoint Unicode scalar ranges without changing document, selection or history.
    /// Overlapping/out-of-bounds ranges reject atomically; edits clear stale colors.
    pub fn set_text_area_foregrounds(&mut self, id: WidgetId, mut spans: Vec<(Range<usize>, Color)>) -> bool {
        let Some(Widget { kind: Kind::TextArea { value, edit, .. }, .. }) = self.widgets.get_mut(&id) else { return false; };
        let len = value.chars().count();
        if spans.iter().any(|(range, _)| range.start > range.end || range.end > len) { return false; }
        spans.retain(|(range, _)| !range.is_empty());
        spans.sort_by_key(|(range, _)| (range.start, range.end));
        if spans.windows(2).any(|pair| pair[0].0.end > pair[1].0.start) { return false; }
        edit.foregrounds = spans;
        true
    }

    /// Replace selection (or insert at caret) as one undoable edit. Read-only areas reject edits.
    pub fn replace_text_area_selection(&mut self, id: WidgetId, text: &str) -> bool {
        let Some(range) = self.text_area_selection(id) else { return false; };
        self.replace_text_area_range(id, range, text)
    }

    /// Replace Unicode scalar offsets as one edit, retaining the original selection for undo.
    /// Invalid ranges and read-only areas reject edits without changing the session.
    pub fn replace_text_area_range(&mut self, id: WidgetId, range: Range<usize>, text: &str) -> bool {
        let Some(Widget {
            kind:
                Kind::TextArea {
                    value,
                    caret,
                    edit,
                    preedit,
                    preedit_cursor,
                    read_only: false,
                    ..
                },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        if range.start > range.end || range.end > value.chars().count() { return false; }
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
            edit.highlights = None;
            edit.foregrounds.clear();
        }
        *caret = range.start + text.chars().count();
        edit.anchor = None;
        edit.goal_x = None;
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
            kind: Kind::TextArea { value, style, line_numbers, .. },
            ..
        }) = self.widgets.get(&id)
        else {
            return false;
        };
        let gutter = self.text_area_gutter_width(value, *style, *line_numbers, style.width);
        let layout = self.font.layout(
            value,
            style.size,
            Some((style.width - 2.0 * style.padding_x - gutter).max(1.0)),
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
                    line_numbers,
                    scroll_y,
                    ..
                },
            ..
        }) = self.widgets.get(&id)
        else {
            return false;
        };
        let gutter = self.text_area_gutter_width(value, *style, *line_numbers, bounds.width);
        let layout = self.font.layout(
            value,
            style.size,
            Some((bounds.width - 2.0 * style.padding_x - gutter).max(1.0)),
        );
        let points = layout.caret_positions(value, self.font.metrics(style.size).ascent);
        // shortcut: gutter clicks place the caret; add whole-line margin gestures with margin selection.
        let target = nearest(
            &points,
            x - bounds.x - style.padding_x - gutter,
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
        edit.goal_x = None;
        edit.reveal = true;
        preedit.clear();
        *preedit_cursor = None;
        true
    }

    pub(super) fn key_text_area(&mut self, id: WidgetId, key: UiKey) -> bool {
        if matches!(self.widgets.get(&id).map(|w| &w.kind), Some(Kind::TextArea { read_only: true, .. }))
            && matches!(key, UiKey::Char(_) | UiKey::Enter | UiKey::Backspace | UiKey::Delete | UiKey::Undo | UiKey::Redo)
        {
            return true;
        }
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
                    line_numbers,
                    scroll_y,
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
        let gutter = self.text_area_gutter_width(value, *style, *line_numbers, style.width);
        let layout = self.font.layout(
            value,
            style.size,
            Some((style.width - style.padding_x * 2.0 - gutter).max(1.0)),
        );
        let points = layout.caret_positions(value, self.font.metrics(style.size).ascent);
        let (x, y) = points[old.min(len)];
        let vertical = matches!(key, UiKey::Up | UiKey::Down | UiKey::SelectUp | UiKey::SelectDown
            | UiKey::PageUp | UiKey::PageDown | UiKey::SelectPageUp | UiKey::SelectPageDown);
        let target_x = edit.goal_x.unwrap_or(x);
        let page_height = (((style.height - 2.0 * style.padding_y) / layout.line_height).floor() - 1.0)
            .max(1.0) * layout.line_height;
        let page_delta = match key {
            UiKey::PageUp | UiKey::SelectPageUp => Some(-page_height),
            UiKey::PageDown | UiKey::SelectPageDown => Some(page_height),
            _ => None,
        };
        let page_scroll = page_delta.map(|delta| {
            (*scroll_y + delta).clamp(0.0,
                (layout.height + 2.0 * style.padding_y - style.height).max(0.0))
        });
        let extend = matches!(
            key,
            UiKey::SelectLeft
                | UiKey::SelectRight
                | UiKey::SelectUp
                | UiKey::SelectDown
                | UiKey::SelectHome
                | UiKey::SelectEnd
                | UiKey::SelectPageUp
                | UiKey::SelectPageDown
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
                    edit.highlights = None;
                    edit.foregrounds.clear();
                    *caret = snapshot.caret;
                    edit.anchor = snapshot.anchor;
                    edit.goal_x = None;
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
            UiKey::Up | UiKey::SelectUp => {
                nearest_on_row(&points, target_x, (y - layout.line_height).max(0.0))
            }
            UiKey::Down | UiKey::SelectDown => {
                nearest_on_row(&points, target_x, y + layout.line_height)
            }
            UiKey::PageUp | UiKey::SelectPageUp | UiKey::PageDown | UiKey::SelectPageDown => {
                nearest_on_row(&points, target_x, (y + page_delta.unwrap()).max(0.0))
            }
            UiKey::Home | UiKey::SelectHome => nearest_on_row(&points, 0.0, y),
            UiKey::End | UiKey::SelectEnd => {
                nearest_on_row(&points, layout.width + 1.0, y)
            }
            _ => return false,
        };
        let Kind::TextArea {
            caret,
            edit,
            preedit,
            preedit_cursor,
            scroll_y,
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
        edit.goal_x = vertical.then_some(target_x);
        if let Some(offset) = page_scroll {
            *scroll_y = offset;
        }
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
                    line_numbers,
                    edit,
                    ..
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
        let marks = if preedit.is_empty() { edit.highlights.clone() } else { None };
        let foregrounds = if preedit.is_empty() { edit.foregrounds.clone() } else { Vec::new() };
        let (value, caret) = display_text(value, *caret, edit, preedit, *preedit_cursor);
        let placeholder = placeholder.clone();
        let style = *style;
        let gutter = self.text_area_gutter_width(&value, style, *line_numbers, bounds.width);
        let (label, chrome) = (*label_node, *chrome_node);
        let layout = self.font.layout(
            &value,
            style.size,
            Some((bounds.width - style.padding_x * 2.0 - gutter).max(1.0)),
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
            bounds.x + style.padding_x + gutter,
            bounds.y + style.padding_y,
            (bounds.width - style.padding_x * 2.0 - gutter).max(1.0),
            view_h,
        );
        scene.set_fill(host, style.bg);
        scene.set_clip(host, Some(bounds));
        scene.set_visible(host, true);
        self.apply_corner_radius(scene, host, false);
        // Keep disjoint marks separate; selection is painted last over search highlights.
        let first = (scroll / layout.line_height).floor() as usize;
        let mut rectangles = Vec::new();
        let (mark_color, marks) = marks.unwrap_or((style.focus_border, Vec::new()));
        let ranges = marks.into_iter().map(|range| (range, mark_color))
            .chain((!range.is_empty()).then_some((range.clone(), style.focus_border)));
        for (range, color) in ranges {
            let mut segment: Option<(f32, f32, f32)> = None;
            let mut append = |x: f32, end: f32, y: f32| {
                if y + layout.line_height <= scroll || y >= scroll + view_h { return; }
                if let Some((_, right, _)) = segment.as_mut().filter(|(_, _, row)| *row == y) {
                    *right = (*right).max(end);
                } else if let Some(previous) = segment.replace((x, end, y)) {
                    rectangles.push((previous, color));
                }
            };
            if range.is_empty() {
                let (x, y) = points[range.start.min(points.len() - 1)];
                append(x, x + 2.0, y);
            } else {
                for i in range.start..range.end.min(points.len().saturating_sub(1)) {
                    let (x, y) = points[i];
                    let (nx, ny) = points[i + 1];
                    append(x, if ny > y { content.width } else { nx.max(x + 2.0) }, y);
                }
            }
            if let Some(previous) = segment { rectangles.push((previous, color)); }
        }
        let numbers: Vec<_> = if gutter > 0.0 { std::iter::once(0).chain(value.chars().enumerate()
            .filter_map(|(i, ch)| (ch == '\n').then_some(i + 1)))
            .enumerate().filter_map(|(i, start)| {
                let y = points[start].1;
                (y + layout.line_height > scroll && y < scroll + view_h).then_some((i + 1, y))
            }).collect() } else { Vec::new() };
        let placeholder_visible = value.is_empty() && !focused;
        let top = first as f32 * layout.line_height;
        let mut visible_layout = if placeholder_visible {
            self.font.layout(&placeholder, style.size, Some(content.width))
        } else { layout.clone() };
        let mut colored_layouts: Vec<(Color, next_gui_text::TextLayout)> = Vec::new();
        if !placeholder_visible && !foregrounds.is_empty() {
            visible_layout.glyphs.clear();
            let mut span = 0;
            for ((index, _), glyph) in value.chars().enumerate().filter(|(_, ch)| *ch != '\n').zip(&layout.glyphs) {
                while span < foregrounds.len() && foregrounds[span].0.end <= index { span += 1; }
                if glyph.baseline_y < top || glyph.baseline_y - layout.line_height >= scroll + view_h { continue; }
                let color = foregrounds.get(span).filter(|(r, _)| r.contains(&index) && !range.contains(&index)).map(|(_, c)| *c);
                let mut glyph = *glyph;
                glyph.baseline_y -= top;
                if let Some(color) = color.filter(|color| *color != style.fg) {
                    let group = if let Some(i) = colored_layouts.iter().position(|(c, _)| *c == color) { i } else {
                        let group = next_gui_text::TextLayout { width: layout.width,
                            height: view_h + layout.line_height, line_height: layout.line_height, glyphs: Vec::new() };
                        colored_layouts.push((color, group));
                        colored_layouts.len() - 1
                    };
                    colored_layouts[group].1.glyphs.push(glyph);
                } else { visible_layout.glyphs.push(glyph); }
            }
        } else {
            visible_layout.glyphs.retain(|g| g.baseline_y >= top && g.baseline_y - layout.line_height < scroll + view_h);
            for g in &mut visible_layout.glyphs { g.baseline_y -= top; }
        }
        visible_layout.height = view_h + layout.line_height;
        let marks_count = rectangles.len();
        let color_start = marks_count + if gutter > 0.0 { numbers.len() + 1 } else { 0 };
        self.ensure_aux_nodes(scene, host, id, color_start + colored_layouts.len());
        let aux = self.widgets.get(&id).unwrap().aux.clone();
        if gutter > 0.0 {
            let clip = Rect::new(bounds.x + style.padding_x, content.y, gutter, view_h);
            let bg = aux[marks_count];
            scene.set_bounds(bg, Rect::new(bounds.x, bounds.y, style.padding_x + gutter, bounds.height));
            scene.set_fill(bg, self.theme.colors.strip_bg);
            scene.set_mask(bg, None);
            scene.set_clip(bg, Some(bounds));
            scene.set_visible(bg, true);
            // Aux nodes change roles as marks/colors change; restore gutter paint order.
            scene.raise(bg);
            for (i, (number, y)) in numbers.into_iter().enumerate() {
                let (_, mask) = self.font.rasterize_text(&number.to_string(), style.size, None);
                let n = aux[marks_count + i + 1];
                scene.set_bounds(n, Rect::new(content.x - style.padding_x * 0.5 - mask.width as f32,
                    content.y + y - scroll, mask.width as f32, mask.height as f32));
                scene.set_fill(n, self.theme.colors.text_secondary);
                scene.set_clip(n, Some(clip));
                scene.set_mask(n, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
                scene.set_visible(n, true);
                scene.raise(n);
            }
        }
        for (i, ((a, b, y), color)) in rectangles.into_iter().enumerate() {
            let n = aux[i];
            let left = (content.x + a).floor();
            let top = (content.y + y - scroll).floor();
            let width = (content.x + b).ceil() - left;
            let height = layout.line_height.ceil();
            scene.set_visible(n, true);
            scene.set_bounds(n, Rect::new(left, top, width, height));
            scene.set_clip(n, Some(content));
            // Coverage gives both renderers the same translucent highlights.
            scene.set_mask(n, Some(AlphaMask::new(width as u32, height as u32,
                vec![100; width as usize * height as usize])));
            scene.set_fill(n, color);
        }
        if let Some(label) = label {
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
        for (i, (color, layout)) in colored_layouts.into_iter().enumerate() {
            let mask = self.font.rasterize_layout(&layout, style.size);
            let n = aux[color_start + i];
            scene.set_bounds(n, Rect::new(content.x, content.y + top - scroll, mask.width as f32, mask.height as f32));
            scene.set_clip(n, Some(content));
            scene.set_fill(n, color);
            scene.set_mask(n, Some(AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(n, true);
            scene.raise(n);
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
    fn text_foregrounds_validate_atomically_and_keep_document_selection_and_history() {
        let text = "SELECT 中文\nSELECT 2";
        let (mut ui, mut scene, area) = editor(text);
        let red = Color::rgb(240, 100, 100);
        let green = Color::rgb(100, 220, 120);
        ui.select_text_area_range(area, 0..0);
        assert!(!ui.set_text_area_foregrounds(WidgetId(0), vec![(0..1, red)]));
        assert!(ui.set_text_area_foregrounds(area, vec![(7..9, green), (0..6, red), (6..6, red)]));
        for spans in [vec![(0..7, red), (6..9, green)], vec![(Range { start: 9, end: 8 }, red)], vec![(0..99, red)]] {
            assert!(!ui.set_text_area_foregrounds(area, spans));
        }
        ui.set_text_area_line_numbers(area, true);
        ui.sync(&mut scene);
        let colored = |ui: &Ui, scene: &Scene, color| ui.widgets[&area].aux.iter()
            .filter(|id| { let n = scene.get(**id).unwrap(); n.visible && n.fill == color }).count();
        assert_eq!(colored(&ui, &scene, red), 1);
        assert_eq!(colored(&ui, &scene, green), 1, "Unicode scalar range colors Chinese text");
        ui.select_text_area_range(area, 0..6);
        ui.sync(&mut scene);
        assert_eq!(colored(&ui, &scene, red), 0, "selected text retains the normal readable foreground");
        assert_eq!(colored(&ui, &scene, green), 1);
        assert_eq!(ui.focused_text_for_copy().as_deref(), Some("SELECT"));
        let state = ui.text_area_state(area).unwrap();
        let (mut rebuilt, mut rebuilt_scene, other) = editor(text);
        assert!(rebuilt.restore_text_area_state(other, &state));
        rebuilt.sync(&mut rebuilt_scene);
        let Kind::TextArea { edit, .. } = &rebuilt.widgets[&other].kind else { unreachable!() };
        assert_eq!(edit.foregrounds.len(), 2);
        ui.replace_text_area_selection(area, "UPDATE");
        let Kind::TextArea { edit, .. } = &ui.widgets[&area].kind else { unreachable!() };
        assert!(edit.foregrounds.is_empty(), "edits discard stale offsets");
        assert!(ui.key_input(UiKey::Undo));
        assert_eq!(ui.text_area_value(area), Some(text));
        assert_eq!(ui.text_area_selection(area), Some(0..6));
        assert!(ui.set_text_area_foregrounds(area, Vec::new()));
        assert!(ui.key_input(UiKey::Redo));
        assert_eq!(ui.text_area_value(area), Some("UPDATE 中文\nSELECT 2"));
    }

    #[test]
    fn text_foregrounds_reused_nodes_keep_gutter_numbers_above_the_background() {
        let text = "abc def ghi jkl mno pqr stu vwx yz \n".repeat(12);
        let (mut ui, mut scene, area) = editor(&text);
        ui.set_text_area_line_numbers(area, true);
        ui.select_text_area_range(area, 0..0);
        for count in [18].into_iter().chain((0..18).flat_map(|count| [0, count])) {
            ui.set_text_area_highlights(area, (0..count).map(|i| i * 2..i * 2 + 1).collect(), Color::rgb(200, 180, 60));
            ui.set_text_area_foregrounds(area, vec![(0..text.chars().count(), Color::rgb(210, 80, 90))]);
            ui.sync(&mut scene);
            let widget = &ui.widgets[&area];
            let background = widget.aux.iter().filter_map(|id| scene.get(*id))
                .find(|n| n.visible && n.mask.is_none()).unwrap();
            let order = scene.iter_paint_order();
            let bg_index = order.iter().position(|n| n.id == background.id).unwrap();
            for number in widget.aux.iter().filter_map(|id| scene.get(*id)).filter(|n| {
                n.visible && n.mask.is_some() && n.clip.is_some_and(|c| c.x < background.bounds.x + background.bounds.width)
            }) {
                assert!(order.iter().position(|n| n.id == number.id).unwrap() > bg_index,
                    "reused number node must paint above gutter background, marks count {count}");
            }
        }
    }

    #[test]
    fn text_foregrounds_clip_visible_rows_and_suspend_during_ime_preedit() {
        let text = "SELECT 1;\n".repeat(80);
        let (mut ui, mut scene, area) = editor(&text);
        let red = Color::rgb(220, 80, 90);
        ui.select_text_area_range(area, 0..0);
        ui.set_text_area_read_only(area, true);
        assert!(ui.set_text_area_foregrounds(area, vec![(0..text.chars().count(), red)]));
        ui.set_text_area_scroll(area, 200.0);
        ui.sync(&mut scene);
        let n = ui.widgets[&area].aux.iter().filter_map(|id| scene.get(*id))
            .find(|n| n.visible && n.fill == red).unwrap();
        assert!(n.bounds.height < 120.0 && n.clip.is_some(), "masks are viewport-sized");
        ui.set_text_area_read_only(area, false);
        assert!(ui.ime_preedit("候选", None));
        ui.sync(&mut scene);
        assert!(!ui.widgets[&area].aux.iter().filter_map(|id| scene.get(*id)).any(|n| n.visible && n.fill == red));
        assert!(ui.ime_clear_preedit());
        ui.sync(&mut scene);
        assert!(ui.widgets[&area].aux.iter().filter_map(|id| scene.get(*id)).any(|n| n.visible && n.fill == red));
        assert_eq!(ui.text_area_value(area), Some(text.as_str()));
    }

    #[test]
    fn line_number_gutter_shares_wrap_hit_selection_scroll_and_ime_geometry() {
        let text = format!("{}\r\n中文 line\n\n", "W".repeat(60));
        let (mut ui, mut scene, area) = editor(&text);
        let host = ui.widgets[&area].node.unwrap();
        let bounds = scene.get(host).unwrap().bounds;
        let Kind::TextArea { style, .. } = ui.widgets[&area].kind else { unreachable!() };
        assert!(!ui.set_text_area_line_numbers(WidgetId(0), true));
        assert!(ui.set_text_area_line_numbers(area, true));
        ui.select_text_area_range(area, 0..0);
        ui.sync(&mut scene);
        let gutter = ui.text_area_gutter_width(&text, style, true, bounds.width);
        let content_x = bounds.x + style.padding_x + gutter;
        let label = ui.widgets[&area].label_node.unwrap();
        assert_eq!(scene.get(label).unwrap().bounds.x, content_x);
        let layout = ui.font.layout(&text, style.size, Some(bounds.width - 2.0 * style.padding_x - gutter));
        let points = layout.caret_positions(&text, ui.font.metrics(style.size).ascent);
        let index = 25;
        assert!(ui.pointer_text_area(area, bounds, content_x + points[index].0,
            bounds.y + style.padding_y + points[index].1 + 1.0, false));
        assert_eq!(ui.text_area_caret(area), Some(index));
        ui.sync(&mut scene);
        let chrome = scene.get(ui.widgets[&area].chrome_node.unwrap()).unwrap();
        let ime = ui.ime_cursor_area(&scene).unwrap();
        assert_eq!((ime.0, ime.1), (chrome.bounds.x, chrome.bounds.y));
        assert!(chrome.bounds.x >= content_x);
        assert!(ui.ime_preedit("候选\n", None));
        ui.sync(&mut scene);
        let chrome = scene.get(ui.widgets[&area].chrome_node.unwrap()).unwrap();
        let ime = ui.ime_cursor_area(&scene).unwrap();
        assert_eq!((ime.0, ime.1), (chrome.bounds.x, chrome.bounds.y));
        assert!(ui.ime_clear_preedit());
        assert_eq!(ui.text_area_value(area), Some(text.as_str()));
        ui.select_text_area_range(area, 0..text.chars().count());
        ui.sync(&mut scene);
        assert_eq!(ui.focused_text_for_copy().as_deref(), Some(text.as_str()));
        for node in &ui.widgets[&area].aux {
            let node = scene.get(*node).unwrap();
            if node.visible && node.fill == style.focus_border { assert!(node.clip.unwrap().x >= content_x); }
        }
        let bottom = (layout.height + 2.0 * style.padding_y - style.height).max(0.0);
        ui.set_text_area_scroll(area, f32::MAX);
        ui.sync(&mut scene);
        assert_eq!(ui.text_area_scroll(area), Some(bottom));
        assert_eq!(scene.get(label).unwrap().bounds.x, content_x);
        let before = ui.text_area_selection(area);
        assert!(ui.set_text_area_line_numbers(area, false));
        ui.sync(&mut scene);
        assert_eq!(ui.text_area_selection(area), before);
        assert_eq!(scene.get(label).unwrap().bounds.x, bounds.x + style.padding_x);
        ui.replace_text_area_selection(area, "replacement");
        assert!(ui.key_input(UiKey::Undo));
        assert_eq!(ui.text_area_value(area), Some(text.as_str()));
    }

    #[test]
    fn physical_line_numbers_handle_wrapping_empty_lines_digit_growth_and_session_modes() {
        let text = "long long long long long long long long long long long long long long\r\n\nlast\n";
        let (mut ui, mut scene, area) = editor(text);
        assert!(ui.set_text_area_line_numbers(area, true));
        ui.select_text_area_range(area, 0..0);
        let Kind::TextArea { style, .. } = &mut ui.widgets.get_mut(&area).unwrap().kind else { unreachable!() };
        style.height = 500.0;
        ui.sync(&mut scene);
        let widget = &ui.widgets[&area];
        let bounds = scene.get(widget.node.unwrap()).unwrap().bounds;
        let Kind::TextArea { style, .. } = widget.kind else { unreachable!() };
        let labels: Vec<_> = widget.aux.iter().filter_map(|id| {
            let n = scene.get(*id).unwrap();
            (n.visible && n.mask.is_some()).then_some(n)
        }).collect();
        assert_eq!(labels.len(), 4, "one number per LF/CRLF physical line, including the final empty line");
        for (i, n) in labels.iter().enumerate() {
            let (_, mask) = ui.font.rasterize_text(&(i + 1).to_string(), style.size, None);
            assert_eq!(n.mask.as_ref().unwrap().pixels.as_ref(), mask.pixels.as_ref());
        }
        assert!(labels[1].bounds.y - labels[0].bounds.y > 1.5 * ui.font.metrics(style.size).line_height,
            "wrapped continuations have no extra physical number");
        let nine = ui.text_area_gutter_width(&"x\n".repeat(8), style, true, bounds.width);
        let ten = ui.text_area_gutter_width(&"x\n".repeat(9), style, true, bounds.width);
        assert!(ten > nine);
        assert_eq!(ui.text_area_gutter_width(text, style, false, bounds.width), 0.0);
        assert_eq!(ui.text_area_gutter_width(text, style, true, 1.0), 0.0);
        ui.key_input(UiKey::Down);
        let state = ui.text_area_state(area).unwrap();
        assert!(state.line_numbers);
        assert!(state.edit.goal_x.is_some());
        ui.set_text_area_line_numbers(area, false);
        assert!(ui.restore_text_area_state(area, &state));
        let Kind::TextArea { line_numbers, edit, .. } = &ui.widgets[&area].kind else { unreachable!() };
        assert!(!line_numbers, "restoration keeps the destination presentation mode");
        assert!(edit.goal_x.is_none());
        assert!(edit.reveal);
        ui.set_text_area_value(area, "");
        ui.set_text_area_line_numbers(area, true);
        ui.sync(&mut scene);
        assert_eq!(ui.widgets[&area].aux.iter().filter(|id| {
            let n = scene.get(**id).unwrap(); n.visible && n.mask.is_some()
        }).count(), 1, "empty document has line 1");
    }

    #[test]
    fn vertical_keys_keep_the_desired_column_across_shorter_lines() {
        for key in [UiKey::Down, UiKey::PageDown] {
            let (mut ui, mut scene, area) = editor("abcdefghij\nx\nabcdefghij");
            let Kind::TextArea { style, .. } = &mut ui.widgets.get_mut(&area).unwrap().kind else { unreachable!() };
            style.height = 2.0 * ui.font.metrics(style.size).line_height + 2.0 * style.padding_y;
            ui.sync(&mut scene);
            ui.select_text_area_range(area, 8..8);
            assert!(ui.key_input(key));
            assert!(ui.key_input(key));
            assert_eq!(ui.text_area_caret(area), Some(21), "restore column 8 after the short middle line");
        }
    }

    #[test]
    fn desired_column_survives_rebuild_and_resets_after_explicit_placement_or_font_changes() {
        let original = "iiiiiiiiii\ni\niiiiiiiiii";
        let (mut ui, mut scene, area) = editor(original);
        ui.select_text_area_range(area, 8..8);
        ui.key_input(UiKey::Down);
        let session = ui.text_area_state(area).unwrap();
        let (mut rebuilt, _, next_area) = editor(original);
        assert!(rebuilt.restore_text_area_state(next_area, &session));
        rebuilt.key_input(UiKey::Down);
        assert_eq!(rebuilt.text_area_caret(next_area), Some(21));
        rebuilt.key_input(UiKey::SelectUp);
        assert_eq!(rebuilt.text_area_selection(next_area), Some(12..21));
        rebuilt.key_input(UiKey::SelectDown);
        assert_eq!(rebuilt.text_area_selection(next_area), Some(21..21));
        ui.key_input(UiKey::Left);
        ui.key_input(UiKey::Down);
        assert_eq!(ui.text_area_caret(area), Some(13), "horizontal key chooses a new column");
        for use_range_edit in [false, true] {
            ui.select_text_area_range(area, 8..8);
            ui.key_input(UiKey::Down);
            if use_range_edit { ui.replace_text_area_range(area, 12..12, ""); }
            else { ui.select_text_area_range(area, 12..12); }
            ui.key_input(UiKey::Down);
            assert_eq!(ui.text_area_caret(area), Some(14), "explicit placement resets the goal");
        }
        ui.select_text_area_range(area, 8..8);
        ui.key_input(UiKey::Down);
        ui.sync(&mut scene);
        let bounds = scene.get(ui.widgets[&area].node.unwrap()).unwrap().bounds;
        let Kind::TextArea { style, .. } = &ui.widgets[&area].kind else { unreachable!() };
        let (px, py) = (style.padding_x, style.padding_y);
        let line_height = ui.font.metrics(style.size).line_height;
        ui.pointer_text_area(area, bounds, bounds.x + px, bounds.y + py + 1.5 * line_height, false);
        ui.key_input(UiKey::Down);
        assert_eq!(ui.text_area_caret(area), Some(13), "pointer placement resets the goal");
        let Kind::TextArea { style, .. } = &mut rebuilt.widgets.get_mut(&next_area).unwrap().kind else { unreachable!() };
        style.size *= 2.0;
        assert!(rebuilt.restore_text_area_state(next_area, &session));
        rebuilt.key_input(UiKey::Down);
        assert_eq!(rebuilt.text_area_caret(next_area), Some(14), "font changes discard pixel goals");
        let Kind::TextArea { style, .. } = &mut rebuilt.widgets.get_mut(&next_area).unwrap().kind else { unreachable!() };
        style.size /= 2.0;
        style.width -= 10.0;
        assert!(rebuilt.restore_text_area_state(next_area, &session));
        rebuilt.key_input(UiKey::Down);
        assert_eq!(rebuilt.text_area_caret(next_area), Some(14), "width changes discard pixel goals");
        assert_eq!(rebuilt.text_area_value(next_area), Some(original));
    }

    #[test]
    fn fractional_line_heights_do_not_skip_rows_during_keyboard_navigation() {
        let line = "SELECT 01, '中文';\n";
        let line_len = line.chars().count();
        for size in [11.0, 12.0, 13.0, 14.0, 15.0] {
            let (mut ui, mut scene, area) = editor(&line.repeat(80));
            let Kind::TextArea { style, .. } = &mut ui.widgets.get_mut(&area).unwrap().kind else { unreachable!() };
            style.size = size;
            ui.sync(&mut scene);
            let origin = 3 * line_len + 2;
            ui.select_text_area_range(area, origin..origin);
            for row in 4..20 {
                assert!(ui.key_input(UiKey::Down));
                assert_eq!(ui.text_area_caret(area), Some(row * line_len + 2), "size {size}");
            }
            for row in (3..19).rev() {
                assert!(ui.key_input(UiKey::Up));
                assert_eq!(ui.text_area_caret(area), Some(row * line_len + 2), "size {size}");
            }
            ui.key_input(UiKey::PageDown);
            ui.key_input(UiKey::PageUp);
            assert_eq!(ui.text_area_caret(area), Some(origin), "size {size}");
        }
    }

    #[test]
    fn page_keys_move_and_extend_selection_in_long_read_only_documents() {
        let line = "SELECT '中文🙂';\n";
        let original = line.repeat(80);
        let line_len = line.chars().count();
        let (mut ui, mut scene, area) = editor(&original);
        ui.select_text_area_range(area, 2..2);
        ui.set_text_area_read_only(area, true);
        assert!(ui.key_input(UiKey::PageDown));
        let first = ui.text_area_caret(area).unwrap();
        assert!(first > line_len + 2, "PageDown must move more than one visual line");
        assert_eq!(first % line_len, 2, "keep the caret's horizontal position");
        assert_eq!(ui.text_area_selection(area), Some(first..first));
        ui.sync(&mut scene);
        assert!(ui.text_area_scroll(area).unwrap() > 0.0);
        assert!(ui.key_input(UiKey::SelectPageDown));
        let second = ui.text_area_caret(area).unwrap();
        assert!(second > first);
        assert_eq!(ui.text_area_selection(area), Some(first..second));
        assert!(ui.key_input(UiKey::SelectPageUp));
        assert_eq!(ui.text_area_caret(area), Some(first));
        assert_eq!(ui.text_area_selection(area), Some(first..first));
        assert!(ui.key_input(UiKey::PageUp));
        assert_eq!(ui.text_area_caret(area), Some(2));
        assert!(ui.key_input(UiKey::PageUp));
        assert_eq!(ui.text_area_caret(area), Some(2));
        for _ in 0..100 { assert!(ui.key_input(UiKey::PageDown)); }
        assert_eq!(ui.text_area_caret(area), Some(original.chars().count()));
        assert_eq!(ui.text_area_value(area), Some(original.as_str()));
        let session = ui.text_area_state(area).unwrap();
        let (mut next, _, next_area) = editor(&original);
        assert!(next.restore_text_area_state(next_area, &session));
        assert!(next.key_input(UiKey::SelectPageUp));
        assert!(next.text_area_selection(next_area).unwrap().start < original.chars().count());
        next.key_input(UiKey::Undo);
        assert_eq!(next.text_area_value(next_area), Some(original.as_str()));
    }

    #[test]
    fn page_keys_follow_wrapped_visual_lines_and_handle_tiny_or_empty_viewports() {
        let original = "中文🙂".repeat(180);
        let (mut ui, mut scene, area) = editor(&original);
        ui.select_text_area_range(area, 0..0);
        assert!(ui.key_input(UiKey::PageDown));
        let normal = ui.text_area_caret(area).unwrap();
        assert!(normal > 0 && normal < original.chars().count());
        assert!(ui.key_input(UiKey::PageUp));
        assert_eq!(ui.text_area_caret(area), Some(0));
        let Kind::TextArea { style, .. } = &mut ui.widgets.get_mut(&area).unwrap().kind else { unreachable!() };
        style.height = 1.0;
        ui.sync(&mut scene);
        assert!(ui.key_input(UiKey::SelectPageDown));
        let tiny = ui.text_area_caret(area).unwrap();
        assert!(tiny > 0 && tiny < normal, "a tiny viewport still advances one visual row");
        assert_eq!(ui.text_area_selection(area), Some(0..tiny));
        assert_eq!(ui.text_area_value(area), Some(original.as_str()));
        ui.set_text_area_value(area, "");
        for key in [UiKey::PageUp, UiKey::PageDown, UiKey::SelectPageUp, UiKey::SelectPageDown] {
            assert!(ui.key_input(key));
            assert_eq!(ui.text_area_caret(area), Some(0));
            assert_eq!(ui.text_area_value(area), Some(""));
        }
    }

    #[test]
    fn range_replacement_preserves_selection_direction_for_undo_and_rejects_invalid_edits() {
        let original = "中文🙂\r\nSELECT 1;";
        let (mut ui, _, area) = editor(original);
        ui.select_text_area_range(area, Range { start: 3, end: 1 });
        for range in [Range { start: 4, end: 2 }, 0..usize::MAX] {
            assert!(!ui.replace_text_area_range(area, range, "bad"));
            assert_eq!(ui.text_area_value(area), Some(original));
            assert_eq!(ui.text_area_caret(area), Some(1));
        }
        assert!(ui.replace_text_area_range(area, 0..5, "-- 中文🙂\r\n"));
        let changed = ui.text_area_value(area).unwrap().to_owned();
        assert_eq!(changed, "-- 中文🙂\r\nSELECT 1;");
        let session = ui.text_area_state(area).unwrap();
        let (mut rebuilt, _, next_area) = editor(&changed);
        assert!(rebuilt.restore_text_area_state(next_area, &session));
        assert!(rebuilt.key_input(UiKey::Undo));
        assert_eq!(rebuilt.text_area_value(next_area), Some(original));
        assert_eq!(rebuilt.text_area_selection(next_area), Some(1..3));
        assert_eq!(rebuilt.text_area_caret(next_area), Some(1));
        rebuilt.key_input(UiKey::Undo);
        assert_eq!(rebuilt.text_area_value(next_area), Some(original), "no second edit to undo");
        assert!(rebuilt.key_input(UiKey::Redo));
        assert_eq!(rebuilt.text_area_value(next_area), Some(changed.as_str()));
        rebuilt.set_text_area_read_only(next_area, true);
        assert!(!rebuilt.replace_text_area_range(next_area, 0..2, "bad"));
        assert!(!rebuilt.replace_text_area_range(WidgetId(0), 0..0, "bad"));
        assert_eq!(rebuilt.text_area_value(next_area), Some(changed.as_str()));
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
    fn read_only_keeps_selection_copy_scroll_and_blocks_all_user_edits() {
        let original = "SELECT '中文🙂';\n".repeat(30);
        let (mut ui, mut scene, area) = editor(&original);
        ui.key_input(UiKey::Char('x'));
        let edited = ui.text_area_value(area).unwrap().to_owned();
        ui.ime_preedit("東京", Some((6, 6)));
        assert!(ui.set_text_area_read_only(area, true));
        assert!(!ui.text_input_wants_ime());
        ui.key_input(UiKey::SelectAll);
        let range = ui.text_area_selection(area).unwrap();
        for key in [UiKey::Char('y'), UiKey::Enter, UiKey::Backspace, UiKey::Delete, UiKey::Undo, UiKey::Redo] {
            assert!(ui.key_input(key));
            assert_eq!(ui.text_area_value(area), Some(edited.as_str()));
            assert_eq!(ui.text_area_selection(area), Some(range.clone()));
        }
        assert!(!ui.paste_into_focused_text("changed"));
        assert!(!ui.ime_preedit("changed", None));
        assert!(!ui.ime_commit("changed"));
        assert!(!ui.replace_text_area_selection(area, "changed"));
        assert_eq!(ui.focused_text_for_copy().as_deref(), Some(edited.as_str()));
        ui.key_input(UiKey::End);
        ui.sync(&mut scene);
        assert!(ui.text_area_scroll(area).unwrap() > 0.0);
        assert!(ui.set_text_area_read_only(area, false));
        assert!(ui.text_input_wants_ime());
        ui.key_input(UiKey::Undo);
        assert_eq!(ui.text_area_value(area), Some(original.as_str()));
        ui.set_text_area_read_only(area, true);
        assert!(ui.set_text_area_value(area, "programmatic"));
        assert_eq!(ui.text_area_value(area), Some("programmatic"));
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
    fn search_highlights_are_disjoint_unicode_clipped_and_session_bound() {
        let (mut ui, mut scene, area) = editor("ab gap 中文 end");
        let color = Color::rgb(220, 160, 40);
        ui.select_text_area_range(area, 0..0);
        assert!(ui.set_text_area_highlights(area, vec![7..9, 0..2, 0..1, 13..13], color));
        assert_eq!(ui.text_area_selection(area), Some(0..0));
        let state = ui.text_area_state(area).unwrap();
        assert!(!ui.set_text_area_highlights(area, std::iter::once(0..14).collect(), color));
        ui.sync(&mut scene);
        let marks: Vec<_> = ui.widgets.get(&area).unwrap().aux.iter()
            .filter_map(|id| scene.get(*id).filter(|n| n.visible)).collect();
        assert_eq!(marks.len(), 3);
        assert!(marks[0].bounds.x + marks[0].bounds.width < marks[1].bounds.x);
        assert!((2.0..=3.0).contains(&marks[2].bounds.width));
        assert!(marks.iter().all(|n| n.fill == color && n.clip.is_some() && n.mask.is_some()));
        ui.replace_text_area_selection(area, "x");
        ui.sync(&mut scene);
        assert!(ui.widgets.get(&area).unwrap().aux.iter().all(|id| !scene.get(*id).unwrap().visible));
        assert!(!ui.restore_text_area_state(area, &state));
        ui.key_input(UiKey::Undo);
        assert!(ui.restore_text_area_state(area, &state));
        ui.sync(&mut scene);
        assert_eq!(ui.widgets.get(&area).unwrap().aux.iter().filter(|id| scene.get(**id).unwrap().visible).count(), 3);
        ui.set_text_area_highlights(area, vec![0..2, 2..2], color);
        ui.sync(&mut scene);
        assert_eq!(ui.widgets.get(&area).unwrap().aux.iter().filter(|id| scene.get(**id).unwrap().visible).count(), 2);
        ui.set_text_area_value(area, "short");
        ui.sync(&mut scene);
        assert!(ui.widgets.get(&area).unwrap().aux.iter().all(|id| !scene.get(*id).unwrap().visible));

        let text = (0..100).map(|_| "中文 abc\n").collect::<String>();
        ui.set_text_area_value(area, &text);
        assert!(ui.set_text_area_highlights(area, (0..100).map(|i| i * 7..i * 7 + 2).collect(), color));
        ui.set_text_area_scroll(area, 500.0);
        ui.sync(&mut scene);
        let visible: Vec<_> = ui.widgets.get(&area).unwrap().aux.iter()
            .filter_map(|id| scene.get(*id).filter(|n| n.visible)).collect();
        assert!(!visible.is_empty() && visible.len() < 10);
        assert!(visible.iter().all(|n| !n.bounds.intersect(&n.clip.unwrap()).is_empty()));
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
