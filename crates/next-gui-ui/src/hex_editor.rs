use super::{
    AlphaMask, FlexBuilder, Kind, Rect, Scene, TextAreaStyle, Ui, UiKey, Widget, WidgetId,
};
use std::ops::Range;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Position {
    nibble: usize,
    anchor: Option<usize>,
}

#[derive(Debug, Clone)]
struct Change {
    start: usize,
    removed: Arc<[u8]>,
    inserted: Arc<[u8]>,
    before: Position,
    after: Position,
}

/// Opaque binary editing session. Restore only against the same document bytes.
#[derive(Debug, Clone)]
pub struct HexEditorState {
    data: Arc<Vec<u8>>,
    position: Position,
    ascii: bool,
    overwrite: bool,
    scroll_x: f32,
    scroll_y: f32,
    reveal: bool,
    undo: Vec<Change>,
    redo: Vec<Change>,
    error: Option<String>,
}

impl HexEditorState {
    fn new(data: Arc<Vec<u8>>) -> Self {
        Self {
            data,
            position: Position::default(),
            ascii: false,
            overwrite: false,
            scroll_x: 0.0,
            scroll_y: 0.0,
            reveal: true,
            undo: Vec::new(),
            redo: Vec::new(),
            error: None,
        }
    }

    fn selection(&self) -> Range<usize> {
        let caret = self.position.nibble / 2;
        let anchor = self.position.anchor.unwrap_or(caret);
        anchor.min(caret)..anchor.max(caret)
    }

    fn move_to(&mut self, nibble: usize, extend: bool) {
        if extend {
            self.position.anchor.get_or_insert(self.position.nibble / 2);
        } else {
            self.position.anchor = None;
        }
        self.position.nibble = if extend {
            nibble.min(self.data.len().saturating_mul(2))
        } else {
            self.clamp_nibble(nibble, self.data.len())
        };
        if self.ascii {
            self.position.nibble &= !1;
        }
        self.reveal = true;
    }

    fn clamp_nibble(&self, nibble: usize, length: usize) -> usize {
        let end = length.saturating_mul(2);
        let last = if self.overwrite {
            end.saturating_sub(1)
        } else {
            end
        };
        let position = nibble.min(last);
        if self.ascii {
            position & !1
        } else {
            position
        }
    }

    fn remember(stack: &mut Vec<Change>, change: Change) {
        stack.push(change);
        let mut bytes = stack.iter().fold(0usize, |sum, item| {
            sum.saturating_add(item.removed.len())
                .saturating_add(item.inserted.len())
        });
        // shortcut: 100 deltas / 8 MiB per stack, retaining one oversized user edit.
        while stack.len() > 1 && (stack.len() > 100 || bytes > 8 * 1024 * 1024) {
            let first = stack.remove(0);
            bytes = bytes
                .saturating_sub(first.removed.len())
                .saturating_sub(first.inserted.len());
        }
    }

    fn replace(&mut self, range: Range<usize>, bytes: &[u8], nibble: usize) -> bool {
        let before = self.position;
        let after = Position {
            nibble: self.clamp_nibble(nibble, self.data.len() - range.len() + bytes.len()),
            anchor: None,
        };
        if self.data[range.clone()] == *bytes {
            let moved = self.position != after;
            self.position = after;
            self.error = None;
            self.reveal = true;
            return moved;
        }
        let change = Change {
            start: range.start,
            removed: Arc::from(&self.data[range.clone()]),
            inserted: Arc::from(bytes),
            before,
            after,
        };
        drop(Arc::make_mut(&mut self.data).splice(range, bytes.iter().copied()));
        self.position = after;
        Self::remember(&mut self.undo, change);
        self.redo.clear();
        self.error = None;
        self.reveal = true;
        true
    }

    fn history(&mut self, redo: bool) -> bool {
        let change = if redo {
            self.redo.pop()
        } else {
            self.undo.pop()
        };
        let Some(change) = change else {
            return false;
        };
        let (old, new, position) = if redo {
            (&change.removed, &change.inserted, change.after)
        } else {
            (&change.inserted, &change.removed, change.before)
        };
        drop(
            Arc::make_mut(&mut self.data)
                .splice(change.start..change.start + old.len(), new.iter().copied()),
        );
        self.position = position;
        self.position.nibble = if self.position.anchor.is_some() {
            self.position.nibble.min(self.data.len().saturating_mul(2))
        } else {
            self.clamp_nibble(self.position.nibble, self.data.len())
        };
        Self::remember(if redo { &mut self.undo } else { &mut self.redo }, change);
        self.error = None;
        self.reveal = true;
        true
    }

    fn type_char(&mut self, ch: char) -> bool {
        let value = if self.ascii {
            if !(0x20..=0xff).contains(&u32::from(ch)) {
                return false;
            }
            ch as u8
        } else {
            if !ch.is_ascii_hexdigit() {
                return false;
            }
            ch.to_digit(16).unwrap() as u8
        };
        let selected = self.selection();
        let index = if selected.is_empty() {
            self.position.nibble / 2
        } else {
            selected.start
        };
        let high = self.position.nibble % 2 == 0 || !selected.is_empty();
        let insert = !self.overwrite && (high || self.ascii);
        if !insert && index >= self.data.len() {
            return false;
        }
        let byte = if self.ascii {
            value
        } else if high {
            (value << 4)
                | if insert || !selected.is_empty() {
                    0
                } else {
                    self.data[index] & 15
                }
        } else {
            (self.data[index] & 0xf0) | value
        };
        let range = if !selected.is_empty() {
            selected
        } else {
            index..index + usize::from(!insert)
        };
        let mut replacement = if self.overwrite && range.len() > 1 {
            vec![0; range.len()]
        } else {
            vec![byte]
        };
        replacement[0] = byte;
        let next = index * 2
            + if self.ascii {
                2
            } else if high {
                1
            } else {
                2
            };
        self.replace(range, &replacement, next)
    }

    fn delete(&mut self, backward: bool) -> bool {
        let selected = self.selection();
        let range = if !selected.is_empty() {
            selected
        } else {
            let mut index = self.position.nibble / 2;
            if backward {
                let Some(previous) = index.checked_sub(1) else {
                    return false;
                };
                index = previous;
            }
            if index >= self.data.len() {
                return false;
            }
            index..index + 1
        };
        let bytes = if self.overwrite {
            vec![0; range.len()]
        } else {
            Vec::new()
        };
        let nibble = range.start * 2;
        self.replace(range, &bytes, nibble)
    }

    fn paste_hex(&mut self, text: &str) -> bool {
        let mut high = None;
        let mut bytes = Vec::new();
        for ch in text.chars().filter(|ch| !ch.is_whitespace()) {
            if !ch.is_ascii_hexdigit() {
                self.error = Some("Paste hexadecimal byte pairs, for example 00 FF 41".into());
                return false;
            }
            let digit = ch.to_digit(16).unwrap() as u8;
            if let Some(first) = high.take() {
                bytes.push(first << 4 | digit);
            } else {
                high = Some(digit);
            }
        }
        if high.is_some() {
            self.error = Some("Hexadecimal paste needs complete byte pairs".into());
            return false;
        }
        if bytes.is_empty() {
            return false;
        }
        let selected = self.selection();
        let start = if selected.is_empty() {
            self.position.nibble / 2
        } else {
            selected.start
        };
        if self.overwrite {
            let count = bytes.len().min(self.data.len() - start);
            if count == 0 {
                return false;
            }
            self.replace(start..start + count, &bytes[..count], (start + count) * 2)
        } else {
            let end = if selected.is_empty() {
                start
            } else {
                selected.end
            };
            self.replace(start..end, &bytes, (start + bytes.len()) * 2)
        }
    }

    fn selected_hex(&self) -> Option<String> {
        let range = self.selection();
        if range.is_empty() {
            return None;
        }
        let mut text = String::new();
        for (index, byte) in self.data[range].iter().enumerate() {
            if index != 0 && index % 16 == 0 {
                text.push('\n');
            }
            use std::fmt::Write;
            write!(text, "{byte:02x}").unwrap();
        }
        Some(text)
    }
}

#[derive(Clone, Copy)]
pub(super) struct Geometry {
    pub view: Rect,
    status: Rect,
    pub max_x: f32,
    pub max_y: f32,
    cell: f32,
    line: f32,
    size: f32,
    digits: usize,
    hex: usize,
    ascii: usize,
}

impl Ui {
    fn hex_editor_font(&self) -> &next_gui_text::Font {
        static FONT: std::sync::OnceLock<Option<next_gui_text::Font>> = std::sync::OnceLock::new();
        FONT.get_or_init(|| next_gui_text::load_default_monospace_font().ok())
            .as_ref()
            .unwrap_or(&self.font)
    }
    /// Current bytes, shared without copying the buffer.
    pub fn hex_editor_bytes(&self, id: WidgetId) -> Option<Arc<Vec<u8>>> {
        let Kind::HexEditor { state, .. } = &self.widgets.get(&id)?.kind else {
            return None;
        };
        Some(state.data.clone())
    }

    /// Snapshot the cursor, selection, scrolling and byte-delta history.
    pub fn hex_editor_state(&self, id: WidgetId) -> Option<HexEditorState> {
        let Kind::HexEditor { state, .. } = &self.widgets.get(&id)?.kind else {
            return None;
        };
        Some(state.clone())
    }

    /// Restore matching bytes/history while retaining the destination's style and read-only policy.
    pub fn restore_hex_editor_state(&mut self, id: WidgetId, saved: &HexEditorState) -> bool {
        let Some(Widget {
            kind: Kind::HexEditor { state, .. },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        if !Arc::ptr_eq(&state.data, &saved.data) && state.data != saved.data {
            return false;
        }
        *state = saved.clone();
        true
    }

    /// Load a new document, clearing its selection, scroll offsets and history.
    pub fn set_hex_editor_bytes(&mut self, id: WidgetId, data: Arc<Vec<u8>>) -> bool {
        let Some(Widget {
            kind: Kind::HexEditor { state, .. },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        *state = HexEditorState::new(data);
        true
    }

    /// Disable byte mutations while retaining navigation, selection and copying.
    pub fn set_hex_editor_read_only(&mut self, id: WidgetId, value: bool) -> bool {
        let Some(Widget {
            kind: Kind::HexEditor { read_only, .. },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        *read_only = value;
        true
    }

    /// Byte offset and whether the caret is on its low nibble.
    pub fn hex_editor_caret(&self, id: WidgetId) -> Option<(usize, bool)> {
        let Kind::HexEditor { state, .. } = &self.widgets.get(&id)?.kind else {
            return None;
        };
        Some((state.position.nibble / 2, state.position.nibble % 2 != 0))
    }

    /// Selected bytes, as a half-open range.
    pub fn hex_editor_selection(&self, id: WidgetId) -> Option<Range<usize>> {
        let Kind::HexEditor { state, .. } = &self.widgets.get(&id)?.kind else {
            return None;
        };
        Some(state.selection())
    }

    /// Select a valid byte range and reveal its end; reject out-of-bounds ranges.
    pub fn select_hex_editor_range(&mut self, id: WidgetId, range: Range<usize>) -> bool {
        let Some(Widget {
            kind: Kind::HexEditor { state, .. },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        if range.start > range.end || range.end > state.data.len() {
            return false;
        }
        state.position = Position {
            nibble: range.end * 2,
            anchor: (!range.is_empty()).then_some(range.start),
        };
        state.reveal = true;
        true
    }

    /// Whether typing replaces bytes instead of inserting them.
    pub fn hex_editor_overwrite(&self, id: WidgetId) -> Option<bool> {
        let Kind::HexEditor { state, .. } = &self.widgets.get(&id)?.kind else {
            return None;
        };
        Some(state.overwrite)
    }

    /// Choose overwrite or insert mode without changing the document.
    pub fn set_hex_editor_overwrite(&mut self, id: WidgetId, value: bool) -> bool {
        let Some(Widget {
            kind: Kind::HexEditor { state, .. },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        state.overwrite = value;
        if state.selection().is_empty() {
            state.move_to(state.position.nibble, false);
        }
        true
    }

    /// Drain the last rejected hexadecimal paste diagnostic.
    pub fn take_hex_editor_error(&mut self, id: WidgetId) -> Option<String> {
        let Kind::HexEditor { state, .. } = &mut self.widgets.get_mut(&id)?.kind else {
            return None;
        };
        state.error.take()
    }

    /// Format a valid byte range as addressed 16-byte Hex/ASCII rows, starting at zero.
    pub fn hex_editor_readable(&self, id: WidgetId, range: Range<usize>) -> Option<String> {
        let document = self.hex_editor_bytes(id)?;
        let bytes = document.get(range)?;
        let digits = format!("{:x}", document.len()).len().max(4);
        let mut text = String::new();
        use std::fmt::Write;
        for (row, bytes) in bytes.chunks(16).enumerate() {
            write!(text, "{:0digits$x} ", row * 16).unwrap();
            for index in 0..16 {
                if let Some(byte) = bytes.get(index) {
                    write!(text, " {byte:02x}").unwrap();
                } else {
                    text.push_str("   ");
                }
            }
            text.push_str("  ");
            for byte in bytes {
                text.push(if (0x20..=0x7e).contains(byte) {
                    *byte as char
                } else {
                    '.'
                });
            }
            for _ in bytes.len()..17 {
                text.push(' ');
            }
            text.push('\n');
        }
        Some(text)
    }

    /// Current horizontal and vertical viewport offsets.
    pub fn hex_editor_scroll_offsets(&self, id: WidgetId) -> Option<(f32, f32)> {
        let Kind::HexEditor { state, .. } = &self.widgets.get(&id)?.kind else {
            return None;
        };
        Some((state.scroll_x, state.scroll_y))
    }

    /// Clamp finite viewport offsets to the current byte document.
    pub fn set_hex_editor_scroll_offsets(&mut self, id: WidgetId, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let Some(Widget {
            kind: Kind::HexEditor { style, .. },
            ..
        }) = self.widgets.get(&id)
        else {
            return false;
        };
        let Some(g) = self.hex_editor_geometry(id, Rect::new(0.0, 0.0, style.width, style.height))
        else {
            return false;
        };
        let Kind::HexEditor { state, .. } = &mut self.widgets.get_mut(&id).unwrap().kind else {
            return false;
        };
        let next = (x.clamp(0.0, g.max_x), y.clamp(0.0, g.max_y));
        let changed = next != (state.scroll_x, state.scroll_y);
        (state.scroll_x, state.scroll_y) = next;
        state.reveal = false;
        changed
    }

    pub(super) fn hex_editor_selected_text(&self, id: WidgetId) -> Option<String> {
        let Kind::HexEditor { state, .. } = &self.widgets.get(&id)?.kind else {
            return None;
        };
        state.selected_hex()
    }

    pub(super) fn paste_hex_editor(&mut self, id: WidgetId, text: &str) -> bool {
        let Some(Widget {
            kind:
                Kind::HexEditor {
                    state,
                    read_only: false,
                    ..
                },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        state.paste_hex(text)
    }

    pub(super) fn cut_hex_editor(&mut self, id: WidgetId) -> bool {
        let Some(Widget {
            kind:
                Kind::HexEditor {
                    state,
                    read_only: false,
                    ..
                },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        !state.selection().is_empty() && state.delete(false)
    }

    pub(super) fn hex_editor_geometry(&self, id: WidgetId, bounds: Rect) -> Option<Geometry> {
        let Kind::HexEditor { state, style, .. } = &self.widgets.get(&id)?.kind else {
            return None;
        };
        let size = if style.size.is_finite() {
            style.size.clamp(1.0, 512.0)
        } else {
            13.0
        };
        let cell = ['M', 'W', '0', '@']
            .iter()
            .map(|ch| self.hex_editor_font().measure_line(&ch.to_string(), size).0)
            .fold(1.0f32, f32::max);
        let line = self
            .hex_editor_font()
            .measure_line("Ag", size)
            .1
            .max(style.line_height)
            .clamp(1.0, 1024.0);
        let px = if style.padding_x.is_finite() {
            style.padding_x.max(0.0)
        } else {
            0.0
        };
        let py = if style.padding_y.is_finite() {
            style.padding_y.max(0.0)
        } else {
            0.0
        };
        let view = Rect::new(
            bounds.x + px,
            bounds.y + py,
            (bounds.width - 2.0 * px).max(1.0),
            (bounds.height - 2.0 * py - line - 4.0).max(1.0),
        );
        let status = Rect::new(view.x, view.y + view.height + 4.0, view.width, line);
        let digits = format!("{:x}", state.data.len()).len().max(4);
        let hex = digits + 2;
        let ascii = hex + 48 + 1;
        Some(Geometry {
            view,
            status,
            cell,
            line,
            size,
            digits,
            hex,
            ascii,
            max_x: ((ascii + 16) as f32 * cell - view.width).max(0.0),
            max_y: ((state.data.len() / 16 + 1) as f32 * line - view.height).max(0.0),
        })
    }

    pub(super) fn pointer_hex_editor(
        &mut self,
        id: WidgetId,
        bounds: Rect,
        x: f32,
        y: f32,
        extend: bool,
    ) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let Some(g) = self.hex_editor_geometry(id, bounds) else {
            return false;
        };
        if !extend && y >= g.status.y {
            return true;
        }
        let Some(Widget {
            kind: Kind::HexEditor { state, .. },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        if extend {
            state.scroll_y = (state.scroll_y
                + (y - g.view.y).min(0.0)
                + (y - g.view.y - g.view.height).max(0.0))
            .clamp(0.0, g.max_y);
            state.scroll_x =
                (state.scroll_x + (x - g.view.x).min(0.0) + (x - g.view.x - g.view.width).max(0.0))
                    .clamp(0.0, g.max_x);
        }
        let row = ((y - g.view.y + state.scroll_y).max(0.0) / g.line).floor() as usize;
        let column = (x - g.view.x + state.scroll_x) / g.cell;
        if !extend {
            state.ascii = column >= g.ascii as f32;
        }
        let nibble = if state.ascii {
            (column - g.ascii as f32).floor().clamp(0.0, 16.0) as usize * 2
        } else {
            let cell = (column - g.hex as f32).max(0.0);
            let byte = (cell / 3.0).floor().min(16.0) as usize;
            byte * 2 + usize::from(byte < 16 && cell % 3.0 >= 1.0)
        };
        state.move_to(row.saturating_mul(32).saturating_add(nibble), extend);
        true
    }

    pub(super) fn key_hex_editor(&mut self, id: WidgetId, key: UiKey) -> bool {
        let Some(Widget {
            kind: Kind::HexEditor { style, .. },
            ..
        }) = self.widgets.get(&id)
        else {
            return false;
        };
        let Some(g) = self.hex_editor_geometry(id, Rect::new(0.0, 0.0, style.width, style.height))
        else {
            return false;
        };
        let Some(Widget {
            kind: Kind::HexEditor {
                state, read_only, ..
            },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        if key == UiKey::Tab && !state.ascii {
            state.ascii = true;
            state.position.nibble &= !1;
            state.reveal = true;
            return true;
        }
        if key == UiKey::BackTab && state.ascii {
            state.ascii = false;
            state.reveal = true;
            return true;
        }
        let step = if state.ascii { 2 } else { 1 };
        let caret = state.position.nibble;
        let page = (g.view.height / g.line).floor().max(2.0) as usize - 1;
        let movement = match key {
            UiKey::DocumentHome | UiKey::SelectDocumentHome => {
                Some((0, key == UiKey::SelectDocumentHome))
            }
            UiKey::DocumentEnd | UiKey::SelectDocumentEnd => Some((
                state.data.len().saturating_mul(2),
                key == UiKey::SelectDocumentEnd,
            )),
            UiKey::Left | UiKey::SelectLeft => {
                Some((caret.saturating_sub(step), key == UiKey::SelectLeft))
            }
            UiKey::Right | UiKey::SelectRight => {
                Some((caret.saturating_add(step), key == UiKey::SelectRight))
            }
            UiKey::Up | UiKey::SelectUp => Some((caret.saturating_sub(32), key == UiKey::SelectUp)),
            UiKey::Down | UiKey::SelectDown => {
                Some((caret.saturating_add(32), key == UiKey::SelectDown))
            }
            UiKey::Home | UiKey::SelectHome => Some((caret / 32 * 32, key == UiKey::SelectHome)),
            UiKey::End | UiKey::SelectEnd => Some((caret / 32 * 32 + 31, key == UiKey::SelectEnd)),
            UiKey::PageUp | UiKey::SelectPageUp => Some((
                caret.saturating_sub(page.saturating_mul(32)),
                key == UiKey::SelectPageUp,
            )),
            UiKey::PageDown | UiKey::SelectPageDown => Some((
                caret.saturating_add(page.saturating_mul(32)),
                key == UiKey::SelectPageDown,
            )),
            _ => None,
        };
        if let Some((target, extend)) = movement {
            state.move_to(target, extend);
            return true;
        }
        match key {
            UiKey::SelectAll => {
                state.position = Position {
                    nibble: state.data.len() * 2,
                    anchor: Some(0),
                };
                state.reveal = true;
                true
            }
            UiKey::Insert => {
                state.overwrite = !state.overwrite;
                if state.selection().is_empty() {
                    state.move_to(caret, false);
                }
                true
            }
            UiKey::Char(ch) if !*read_only => state.type_char(ch),
            UiKey::Backspace | UiKey::Delete if !*read_only => {
                state.delete(key == UiKey::Backspace)
            }
            UiKey::Undo | UiKey::Redo if !*read_only => state.history(key == UiKey::Redo),
            _ => false,
        }
    }

    pub(super) fn paint_hex_editor(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(widget) = self.widgets.get(&id) else {
            return;
        };
        let Some(host) = widget.node else {
            return;
        };
        let Some(bounds) = scene.get(host).map(|node| node.bounds) else {
            return;
        };
        let Some(g) = self.hex_editor_geometry(id, bounds) else {
            return;
        };
        let Kind::HexEditor { state, style, .. } = &widget.kind else {
            return;
        };
        let (data, position, ascii, selected, style) = (
            state.data.clone(),
            state.position,
            state.ascii,
            state.selection(),
            *style,
        );
        let overwrite = state.overwrite;
        let (label, caret_node) = (widget.label_node, widget.chrome_node);
        let mut sx = state.scroll_x.clamp(0.0, g.max_x);
        let mut sy = state.scroll_y.clamp(0.0, g.max_y);
        let byte = position.nibble / 2;
        let column = if ascii {
            g.ascii + byte % 16
        } else {
            g.hex + byte % 16 * 3 + position.nibble % 2
        };
        let (cx, cy) = (column as f32 * g.cell, (byte / 16) as f32 * g.line);
        if state.reveal {
            if cx < sx {
                sx = cx;
            }
            if cx + g.cell > sx + g.view.width {
                sx = (cx + g.cell - g.view.width).min(g.max_x);
            }
            if cy < sy {
                sy = cy;
            }
            if cy + g.line > sy + g.view.height {
                sy = (cy + g.line - g.view.height).min(g.max_y);
            }
        }
        let Kind::HexEditor { state, .. } = &mut self.widgets.get_mut(&id).unwrap().kind else {
            return;
        };
        (state.scroll_x, state.scroll_y, state.reveal) = (sx, sy, false);
        scene.set_fill(host, style.bg);
        scene.set_clip(host, Some(bounds));
        scene.set_visible(host, true);
        self.apply_corner_radius(scene, host, false);
        if let Some(label) = label {
            let mode = if overwrite { "Overwrite" } else { "Insert" };
            let text = format!("{byte:#06x} · {mode} · {} selected", selected.len());
            let layout = self.font.layout(&text, g.size, None);
            let mask = self.font.rasterize_layout_clipped(
                &layout,
                g.size,
                g.status.width.ceil() as u32,
                g.line.ceil() as u32,
            );
            scene.set_bounds(
                label,
                Rect::new(
                    g.status.x,
                    g.status.y,
                    mask.width as f32,
                    mask.height as f32,
                ),
            );
            scene.set_fill(label, self.theme.colors.text_secondary);
            scene.set_clip(label, Some(bounds));
            scene.set_mask(
                label,
                Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
            );
            scene.set_visible(label, true);
            scene.raise(label);
        }
        let first = (sy / g.line).floor() as usize;
        let end = (first + (g.view.height / g.line).ceil() as usize + 1).min(data.len() / 16 + 1);
        self.ensure_aux_nodes(scene, host, id, (end - first) * 4);
        let aux = self.widgets[&id].aux.clone();
        for (slot, row) in (first..end).enumerate() {
            let offset = row * 16;
            let y = g.view.y + row as f32 * g.line - sy;
            let nodes = &aux[slot * 4..slot * 4 + 4];
            let range = selected.start.max(offset)..selected.end.min(offset + 16).min(data.len());
            for (node, ascii_area) in [(nodes[0], false), (nodes[1], true)] {
                if range.start >= range.end {
                    scene.set_visible(node, false);
                    continue;
                }
                let start = range.start - offset;
                let count = range.len();
                let (x, width) = if ascii_area {
                    ((g.ascii + start) as f32 * g.cell, count as f32 * g.cell)
                } else {
                    (
                        (g.hex + start * 3) as f32 * g.cell,
                        (count * 3 - 1) as f32 * g.cell,
                    )
                };
                let rect = Rect::new(g.view.x + x - sx, y, width, g.line).intersect(&g.view);
                scene.set_bounds(node, rect);
                scene.set_clip(node, Some(g.view));
                scene.set_fill(node, style.focus_border);
                let (width, height) = (rect.width.ceil() as u32, rect.height.ceil() as u32);
                scene.set_mask(
                    node,
                    Some(AlphaMask::new(
                        width,
                        height,
                        vec![90; width as usize * height as usize],
                    )),
                );
                scene.set_visible(node, !rect.is_empty());
                scene.raise(node);
            }
            let address = format!("{offset:0width$x}", width = g.digits);
            let mut text = vec![b' '; g.ascii + 16];
            const HEX: &[u8; 16] = b"0123456789abcdef";
            for (index, byte) in data[offset..(offset + 16).min(data.len())]
                .iter()
                .enumerate()
            {
                text[g.hex + index * 3] = HEX[(byte >> 4) as usize];
                text[g.hex + index * 3 + 1] = HEX[(byte & 15) as usize];
                text[g.ascii + index] = if (0x20..=0x7e).contains(byte) {
                    *byte
                } else {
                    b'.'
                };
            }
            let content = String::from_utf8(text).unwrap();
            for (node, text, color) in [
                (nodes[2], address.as_str(), self.theme.colors.text_secondary),
                (nodes[3], content.as_str(), style.fg),
            ] {
                let mut layout = self.hex_editor_font().layout(text, g.size, None);
                for (index, glyph) in layout.glyphs.iter_mut().enumerate() {
                    glyph.x = index as f32 * g.cell + (g.cell - glyph.advance).max(0.0) * 0.5 - sx;
                    glyph.advance = g.cell;
                }
                layout.width = text.len() as f32 * g.cell;
                layout.height = g.line;
                let mask = self.hex_editor_font().rasterize_layout_clipped(
                    &layout,
                    g.size,
                    g.view.width.ceil() as u32,
                    g.line.ceil() as u32,
                );
                scene.set_bounds(
                    node,
                    Rect::new(g.view.x, y, mask.width as f32, mask.height as f32),
                );
                scene.set_fill(node, color);
                scene.set_clip(node, Some(g.view));
                scene.set_mask(
                    node,
                    Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                );
                scene.set_visible(node, true);
                scene.raise(node);
            }
        }
        if let Some(caret) = caret_node {
            scene.set_bounds(
                caret,
                Rect::new(
                    (g.view.x + cx - sx).floor(),
                    (g.view.y + cy - sy).floor(),
                    2.0,
                    g.line,
                ),
            );
            scene.set_mask(caret, None);
            scene.set_fill(caret, style.focus_border);
            scene.set_clip(caret, Some(g.view));
            scene.set_visible(caret, self.focus == Some(id));
            scene.raise(caret);
        }
    }
}

impl FlexBuilder<'_> {
    /// Virtualized address/Hex/ASCII editor, using the shared multiline input skin.
    pub fn hex_editor(&mut self, data: Arc<Vec<u8>>, style: TextAreaStyle) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::HexEditor {
                    state: HexEditorState::new(data),
                    read_only: false,
                    style,
                },
                node: None,
                label_node: None,
                chrome_node: None,
                aux: Vec::new(),
                accessible_name: None,
            },
        );
        self.push_child(id);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_virtualizes_rows_and_keeps_pointer_clipboard_scroll_and_read_only_sessions() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut id = WidgetId(0);
        let mut next = WidgetId(0);
        let bytes = Arc::new((0..1_000_003).map(|index| (index % 256) as u8).collect());
        ui.column(
            super::super::ColumnStyle {
                width: 360.0,
                padding: 0.0,
                ..Default::default()
            },
            |c| {
                id = c.hex_editor(
                    bytes,
                    TextAreaStyle {
                        width: 360.0,
                        height: 180.0,
                        ..Default::default()
                    },
                );
                next = c.button("Next");
            },
        );
        ui.set_focus(Some(id));
        ui.sync(&mut scene);
        let bounds = scene.get(ui.widgets[&id].node.unwrap()).unwrap().bounds;
        let g = ui.hex_editor_geometry(id, bounds).unwrap();
        assert!(
            ui.widgets[&id].aux.len() < 80,
            "Only visible rows should have nodes"
        );
        assert!(ui.widgets[&id]
            .aux
            .iter()
            .filter_map(|id| scene.get(*id))
            .filter_map(|node| node.mask.as_ref())
            .all(|mask| mask.width <= 360 && mask.height < 100));
        assert!(!ui.text_input_wants_ime());
        assert!(!ui.ime_commit("中文"));
        assert!(ui.pointer_pressed(
            &mut scene,
            g.view.x + (g.hex as f32 + 1.4) * g.cell,
            g.view.y + g.line * 0.5
        ));
        ui.pointer_released();
        assert_eq!(ui.hex_editor_caret(id), Some((0, true)));
        assert!(ui.key_input(UiKey::SelectRight));
        assert_eq!(ui.hex_editor_selection(id), Some(0..1));
        assert_eq!(ui.focused_text_for_copy().as_deref(), Some("00"));
        let original = ui.hex_editor_bytes(id).unwrap();
        assert!(!ui.paste_into_focused_text("12 zz"));
        assert!(Arc::ptr_eq(&original, &ui.hex_editor_bytes(id).unwrap()));
        assert!(ui.take_hex_editor_error(id).is_some());
        assert!(ui.paste_into_focused_text("41 42"));
        assert_eq!(&ui.hex_editor_bytes(id).unwrap()[..3], b"AB\x01");
        let saved = ui.hex_editor_state(id).unwrap();
        ui.set_hex_editor_bytes(id, ui.hex_editor_bytes(id).unwrap());
        assert!(ui.restore_hex_editor_state(id, &saved));
        assert!(ui.key_input(UiKey::Undo));
        assert_eq!(*ui.hex_editor_bytes(id).unwrap(), *original);
        assert!(ui.key_input(UiKey::Redo));
        ui.select_hex_editor_range(id, 0..2);
        let readable = ui.hex_editor_readable(id, 0..2).unwrap();
        assert!(readable.starts_with("00000  41 42"));
        assert!(readable.contains("  AB"));
        assert!(readable.ends_with('\n'));
        assert!(ui.hex_editor_readable(id, 2..usize::MAX).is_none());
        ui.set_hex_editor_read_only(id, true);
        for key in [
            UiKey::Char('f'),
            UiKey::Backspace,
            UiKey::Delete,
            UiKey::Undo,
            UiKey::Redo,
        ] {
            assert!(!ui.key_input(key));
        }
        assert!(!ui.paste_into_focused_text("ff"));
        assert!(!ui.delete_focused_text_selection());
        assert_eq!(ui.focused_text_for_copy().as_deref(), Some("4142"));
        assert!(ui.key_input(UiKey::Tab));
        assert_eq!(ui.focused(), Some(id));
        assert!(ui.key_input(UiKey::Tab));
        assert_eq!(ui.focused(), Some(next));
        ui.set_focus(Some(id));
        assert!(ui.key_input(UiKey::BackTab));
        assert_eq!(ui.focused(), Some(id));
        assert!(ui.key_input(UiKey::DocumentEnd));
        ui.sync(&mut scene);
        assert!(ui.hex_editor_scroll_offsets(id).unwrap().1 > 100_000.0);
        assert!(ui.key_input(UiKey::DocumentHome));
        ui.sync(&mut scene);
        assert_eq!(ui.hex_editor_scroll_offsets(id).unwrap().1, 0.0);
        assert!(ui.pointer_scroll(&scene, g.view.x + 10.0, g.view.y + 10.0, 30.0, 100.0));
        assert_eq!(ui.hex_editor_scroll_offsets(id), Some((30.0, 100.0)));
        let caret = ui.hex_editor_caret(id);
        ui.sync(&mut scene);
        for index in [1, 3] {
            let thumb = scene.get(ui.scrollbars[&id][index]).unwrap().bounds;
            let (x, y) = (thumb.x + thumb.width * 0.5, thumb.y + thumb.height * 0.5);
            assert!(ui.pointer_pressed(&mut scene, x, y));
            assert!(ui.pointer_moved(x + 1_000_000.0, y + 1_000_000.0));
            ui.pointer_released();
            ui.sync(&mut scene);
        }
        assert_eq!(ui.hex_editor_caret(id), caret);
        assert!(!ui.set_hex_editor_scroll_offsets(id, f32::NAN, 0.0));
        ui.set_hex_editor_read_only(id, false);
        ui.key_input(UiKey::DocumentHome);
        ui.key_input(UiKey::Tab);
        assert!(ui.key_input(UiKey::Char('Z')));
        assert_eq!(ui.hex_editor_bytes(id).unwrap()[0], b'Z');
        let different = ui.hex_editor_state(id).unwrap();
        ui.set_hex_editor_bytes(id, Arc::new(vec![]));
        assert!(!ui.restore_hex_editor_state(id, &different));
    }

    #[test]
    fn unchanged_overwrites_still_report_cursor_and_selection_changes_without_history() {
        let mut edit = HexEditorState::new(Arc::new(vec![0, 0]));
        edit.overwrite = true;
        assert!(edit.type_char('0'));
        assert_eq!(edit.position.nibble, 1);
        assert!(edit.undo.is_empty());
        assert!(edit.type_char('0'));
        assert_eq!(edit.position.nibble, 2);
        assert!(edit.undo.is_empty());
        edit.move_to(0, false);
        edit.move_to(4, true);
        assert!(edit.delete(false));
        assert!(edit.selection().is_empty());
        assert!(edit.undo.is_empty());
        assert_eq!(*edit.data, [0, 0]);
    }

    #[test]
    fn byte_edits_keep_nibbles_selections_modes_and_delta_history() {
        let source = Arc::new(vec![0x12, 0x34, 0x56]);
        let mut edit = HexEditorState::new(source.clone());
        assert!(edit.type_char('a'));
        assert_eq!(*edit.data, [0xa0, 0x12, 0x34, 0x56]);
        assert!(edit.type_char('b'));
        assert_eq!(*edit.data, [0xab, 0x12, 0x34, 0x56]);
        assert_eq!(edit.position.nibble, 2);
        assert_eq!(*source, [0x12, 0x34, 0x56]);
        assert!(edit.history(false));
        assert_eq!(*edit.data, [0xa0, 0x12, 0x34, 0x56]);
        assert!(edit.history(false));
        assert_eq!(*edit.data, *source);
        assert!(edit.history(true));
        assert!(edit.history(true));
        assert_eq!(*edit.data, [0xab, 0x12, 0x34, 0x56]);
        edit.move_to(2, false);
        edit.move_to(6, true);
        assert_eq!(edit.selection(), 1..3);
        assert_eq!(edit.selected_hex().as_deref(), Some("1234"));
        assert!(edit.type_char('f'));
        assert_eq!(*edit.data, [0xab, 0xf0, 0x56]);
        assert!(edit.history(false));
        assert_eq!(edit.selection(), 1..3);
        assert_eq!(*edit.data, [0xab, 0x12, 0x34, 0x56]);
        edit.overwrite = true;
        assert!(edit.delete(false));
        assert_eq!(*edit.data, [0xab, 0, 0, 0x56]);
        assert!(edit.history(false));
        edit.ascii = true;
        assert!(edit.type_char('Z'));
        assert_eq!(*edit.data, [0xab, b'Z', 0, 0x56]);
        assert!(!edit.type_char('中'));
        assert!(!edit.type_char('\n'));
        edit.move_to(usize::MAX, false);
        assert_eq!(edit.position.nibble, 6);
        assert!(edit.type_char('é'));
        assert_eq!(*edit.data, [0xab, b'Z', 0, 0xe9]);
        assert_eq!(edit.position.nibble, 6);
        edit.overwrite = false;
        edit.move_to(usize::MAX, false);
        assert!(edit.type_char('!'));
        assert_eq!(edit.data.last(), Some(&b'!'));
        assert!(edit.delete(true));
        assert_eq!(edit.data.last(), Some(&0xe9));
        assert!(!edit.history(true));
    }

    #[test]
    fn paste_validates_before_changes_and_overwrite_never_grows_the_buffer() {
        let mut edit = HexEditorState::new(Arc::new(vec![0x12, 0x34, 0x56]));
        for text in ["0", "0x12", "+1", "FF 中", "ＡＡ"] {
            assert!(!edit.paste_hex(text));
            assert_eq!(*edit.data, [0x12, 0x34, 0x56]);
            assert!(edit.undo.is_empty());
        }
        assert!(edit.paste_hex("00\u{2003}Ff"));
        assert_eq!(*edit.data, [0, 255, 0x12, 0x34, 0x56]);
        assert!(edit.error.is_none());
        assert!(edit.history(false));
        edit.overwrite = true;
        edit.move_to(4, false);
        assert!(edit.paste_hex("aa bb cc"));
        assert_eq!(*edit.data, [0x12, 0x34, 0xaa]);
        assert!(edit.history(false));
        assert_eq!(edit.position.nibble, 4);
        edit.overwrite = false;
        edit.move_to(0, false);
        edit.move_to(6, true);
        assert!(edit.paste_hex("41"));
        assert_eq!(*edit.data, [b'A']);
        assert_eq!(edit.position.nibble, 2);
        assert!(edit.history(false));
        assert_eq!(edit.selection(), 0..3);
        assert!(edit.delete(false));
        assert!(edit.data.is_empty());
        assert!(edit.history(false));
        assert_eq!(edit.selection(), 0..3);
    }

    #[test]
    fn nibble_selection_and_history_are_bounded_without_copying_whole_snapshots() {
        let mut edit = HexEditorState::new(Arc::new(vec![0; 1_000_000]));
        edit.move_to(1, true);
        assert!(edit.selection().is_empty());
        edit.move_to(2, true);
        assert_eq!(edit.selection(), 0..1);
        edit.move_to(0, false);
        for _ in 0..120 {
            assert!(edit.type_char('f'));
        }
        assert_eq!(edit.undo.len(), 100);
        assert!(edit
            .undo
            .iter()
            .all(|change| change.removed.len() <= 1 && change.inserted.len() == 1));
        assert_eq!(
            edit.undo
                .iter()
                .map(|change| change.removed.len() + change.inserted.len())
                .sum::<usize>(),
            150
        );
        edit.move_to(0, false);
        edit.move_to(2 * 33, true);
        let copied = edit.selected_hex().unwrap();
        assert_eq!(
            copied.lines().map(str::len).collect::<Vec<_>>(),
            [32, 32, 2]
        );
        assert_eq!((edit.scroll_x, edit.scroll_y), (0.0, 0.0));
    }
}
