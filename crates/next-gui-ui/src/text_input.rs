use crate::{DragState, Kind, Ui, UiKey, WidgetId};
use std::time::{Duration, Instant};

// Match Qt common spinbox mouse-repeat timing.
const INTEGER_REPEAT_DELAY: Duration = Duration::from_millis(500);
const INTEGER_REPEAT_INTERVAL: Duration = Duration::from_millis(150);

#[derive(Debug, Clone)]
pub(super) struct InputSnapshot {
    value: String,
    caret: usize,
    select_all: bool,
}

#[derive(Debug, Clone, Default)]
pub(super) struct InputEdit {
    undo: Vec<InputSnapshot>,
    redo: Vec<InputSnapshot>,
}

fn remember(stack: &mut Vec<InputSnapshot>, snapshot: InputSnapshot) {
    // shortcut: match TextArea's 100 edits / 8 MiB per stack; use deltas for large inputs.
    stack.push(snapshot);
    let mut bytes: usize = stack.iter().map(|s| s.value.len()).sum();
    while stack.len() > 1 && (stack.len() > 100 || bytes > 8 * 1024 * 1024) {
        bytes -= stack.remove(0).value.len();
    }
}

/// Opaque single-line editing session, including caret, selection, composition and history.
/// Restore only into the same document value and secret mode; does not set focus.
#[derive(Debug, Clone)]
pub struct TextInputState {
    snapshot: InputSnapshot,
    edit: InputEdit,
    secret: bool,
    preedit: String,
    preedit_cursor: Option<(usize, usize)>,
}

impl Ui {
    /// Add exact signed integer steppers to a TextInput, or remove them with None.
    /// Up/Down step by one, PageUp/PageDown by ten; changes share Undo/Redo.
    /// Draft text remains editable; hosts validate it before applying it.
    /// Invalid ranges, secret inputs and other widget kinds are rejected.
    pub fn set_text_input_integer_range(&mut self, id: WidgetId, range: Option<(i64, i64)>) -> bool {
        if range.is_some_and(|(min, max)| min > max) { return false; }
        if range.is_none() && self.focus == Some(id) { self.cancel_pointer_repeat(); }
        let Some(widget) = self.widgets.get_mut(&id) else { return false; };
        let Kind::TextInput { integer_range, secret, .. } = &mut widget.kind else { return false; };
        if *secret && range.is_some() { return false; }
        *integer_range = range;
        true
    }

    /// Configured integer bounds; None for ordinary inputs or other widget kinds.
    pub fn text_input_integer_range(&self, id: WidgetId) -> Option<(i64, i64)> {
        match &self.widgets.get(&id)?.kind { Kind::TextInput { integer_range, .. } => *integer_range, _ => None }
    }

    pub(super) fn integer_input_step_width(&self, id: WidgetId, width: f32) -> f32 {
        if self.text_input_integer_range(id).is_some() { 24.0f32.min(width * 0.5) } else { 0.0 }
    }

    pub(super) fn step_integer_input(&mut self, id: WidgetId, delta: i64) -> bool {
        let Some((min, max)) = self.text_input_integer_range(id) else { return false; };
        let before = self.input_snapshot(id);
        let Kind::TextInput { value, enabled, preedit, caret, select_all, .. } = &mut self.widgets.get_mut(&id).unwrap().kind else { return false; };
        if !*enabled || !preedit.is_empty() { return false; }
        let Ok(current) = value.trim().parse::<i64>() else { return true; };
        if !(min..=max).contains(&current) { return true; }
        *value = current.saturating_add(delta).clamp(min, max).to_string();
        *caret = value.len();
        *select_all = false;
        self.remember_input_change(id, before);
        true
    }

    pub(super) fn pointer_integer_input(
        &mut self,
        scene: &crate::Scene,
        id: WidgetId,
        x: f32,
        y: f32,
    ) -> bool {
        let Some(bounds) = self.widgets[&id]
            .node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
        else {
            return false;
        };
        let width = self.integer_input_step_width(id, bounds.width);
        if width == 0.0 || x < bounds.x + bounds.width - width {
            return false;
        }
        let delta = if y < bounds.y + bounds.height * 0.5 {
            1
        } else {
            -1
        };
        let before = self.text_input_value(id).unwrap().to_owned();
        self.step_integer_input(id, delta);
        if self.text_input_value(id) != Some(before.as_str()) {
            self.drag = Some(DragState::IntegerInput {
                input: id,
                delta,
                bounds: crate::Rect::new(
                    bounds.x + bounds.width - width,
                    bounds.y + if delta > 0 { 0.0 } else { bounds.height * 0.5 },
                    width,
                    bounds.height * 0.5,
                ),
                next_repeat: Some(Instant::now() + INTEGER_REPEAT_DELAY),
            });
        }
        true
    }

    /// Cancel held integer tracking, including a pointer paused outside its arrow.
    /// Hosts should call this when the pointer leaves the native window.
    pub fn cancel_pointer_repeat(&mut self) -> bool {
        if matches!(self.drag, Some(DragState::IntegerInput { .. })) {
            self.drag = None;
            true
        } else {
            false
        }
    }

    pub(super) fn move_integer_repeat(&mut self, x: f32, y: f32) -> bool {
        let Some(DragState::IntegerInput {
            bounds,
            next_repeat,
            ..
        }) = &mut self.drag
        else {
            return false;
        };
        let inside = x.is_finite() && y.is_finite() && bounds.contains(x, y);
        if inside == next_repeat.is_some() {
            return false;
        }
        *next_repeat = inside.then(|| Instant::now() + INTEGER_REPEAT_DELAY);
        true
    }

    fn integer_repeat_allowed(&self, id: WidgetId) -> bool {
        self.focus == Some(id)
            && self.open_popup.is_none()
            && self
                .dialog_focus_scope()
                .is_none_or(|dialog| self.widget_contains(dialog, id))
            && matches!(self.widgets.get(&id).map(|w| &w.kind), Some(Kind::TextInput {enabled:true,preedit,integer_range:Some(_),..}) if preedit.is_empty())
    }

    /// Delay until a held integer arrow repeats. Schedule a host redraw after this delay.
    /// None means the pointer is released/paused or its editor is no longer active.
    pub fn pointer_repeat_delay(&self) -> Option<Duration> {
        let DragState::IntegerInput {
            input,
            next_repeat: Some(next),
            ..
        } = self.drag.as_ref()?
        else {
            return None;
        };
        self.integer_repeat_allowed(*input)
            .then(|| next.saturating_duration_since(Instant::now()))
    }

    /// Advance a held integer arrow; true means its value or tracking appearance changed.
    /// Call sync/repaint after true, including when stepping stops at a bound.
    /// Late redraws produce one step rather than a burst. Release via pointer_released.
    pub fn repeat_pointer(&mut self) -> bool {
        self.repeat_pointer_at(Instant::now())
    }

    fn repeat_pointer_at(&mut self, now: Instant) -> bool {
        let Some(DragState::IntegerInput {
            input,
            delta,
            next_repeat,
            ..
        }) = self.drag.clone()
        else {
            return false;
        };
        if !self.integer_repeat_allowed(input) {
            return self.cancel_pointer_repeat();
        }
        let Some(next) = next_repeat else {
            return false;
        };
        if now < next {
            return false;
        }
        let before = self.text_input_value(input).unwrap().to_owned();
        self.step_integer_input(input, delta);
        let changed = self.text_input_value(input) != Some(before.as_str());
        if !changed {
            return self.cancel_pointer_repeat();
        }
        if let Some(DragState::IntegerInput { next_repeat, .. }) = &mut self.drag {
            *next_repeat = Some(now + INTEGER_REPEAT_INTERVAL);
        }
        changed
    }

    pub(super) fn paint_integer_input_steps(&mut self, scene: &mut crate::Scene, id: WidgetId, bounds: crate::Rect) {
        let range = self.text_input_integer_range(id);
        let needed = if range.is_some() { 5 } else { 1 };
        let host = self.widgets[&id].node.unwrap();
        self.ensure_aux_nodes(scene, host, id, needed);
        let Some((min, max)) = range else { return; };
        let Kind::TextInput { style, value, enabled, preedit, .. } = &self.widgets[&id].kind else { return; };
        let (style, enabled) = (*style, *enabled && preedit.is_empty());
        let current = value.trim().parse::<i64>().ok();
        let width = self.integer_input_step_width(id, bounds.width);
        let aux = self.widgets[&id].aux.clone();
        for (slot, label) in [(0, "▴"), (1, "▾")] {
            let area = crate::Rect::new(bounds.x + bounds.width - width, bounds.y + slot as f32 * bounds.height * 0.5, width, bounds.height * 0.5);
            let background = aux[1 + slot * 2];
            let text = aux[2 + slot * 2];
            scene.set_bounds(background, area);
            let pressed = matches!(&self.drag, Some(DragState::IntegerInput {input,delta,next_repeat:Some(_),..}) if *input == id && (*delta > 0) == (slot == 0));
            scene.set_fill(background, if !enabled { self.theme.colors.muted } else if pressed { self.theme.colors.accent } else { self.theme.colors.strip_bg });
            scene.set_mask(background, None);
            scene.set_visible(background, true);
            let mask = self.font.rasterize_text(label, style.size.min(area.height), None).1;
            scene.set_bounds(text, crate::Rect::new(area.x + (area.width-mask.width as f32)*0.5, area.y + (area.height-mask.height as f32)*0.5, mask.width as f32, mask.height as f32));
            let can_step = enabled && current.is_some_and(|n| (min..=max).contains(&n) && if slot == 0 { n < max } else { n > min });
            scene.set_fill(text, if !can_step { self.theme.colors.disabled } else if pressed { self.theme.colors.on_accent } else { style.fg });
            scene.set_mask(text, Some(crate::AlphaMask::new(mask.width, mask.height, mask.pixels)));
            scene.set_visible(text, true);
        }
    }

    pub(super) fn single_line_input_display(&self, id: WidgetId) -> Option<(String, usize, std::ops::Range<usize>)> {
        let (value,caret,select_all,preedit,preedit_cursor,secret)=match &self.widgets.get(&id)?.kind {
            Kind::TextInput {value,caret,select_all,preedit,preedit_cursor,secret,..} => (value,caret,select_all,preedit,preedit_cursor,*secret),
            Kind::Autocomplete {value,caret,select_all,preedit,preedit_cursor,..} => (value,caret,select_all,preedit,preedit_cursor,false),
            _ => return None,
        };
        let composing=self.focus==Some(id) && !preedit.is_empty();
        let (text,cursor,composition)=if composing {
            let before:String=value.chars().take(if *select_all {0} else {*caret}).collect();
            let after:String=if *select_all {String::new()} else {value.chars().skip(*caret).collect()};
            let start=before.chars().count();let end=start+preedit.chars().count();
            let cursor=preedit_cursor.and_then(|(byte,_)|preedit.get(..byte)).map_or(end,|prefix|start+prefix.chars().count());
            (format!("{before}{preedit}{after}"),cursor,start..end)
        } else {(value.clone(),*caret,0..0)};
        // Preserve the document and character positions while painting one physical line.
        let text=if secret {"*".repeat(text.chars().count())} else {text.replace(['\r','\n','\t']," ")};
        Some((text,cursor,composition))
    }

    pub(super) fn input_snapshot(&self, id: WidgetId) -> Option<InputSnapshot> {
        let (Kind::TextInput {
            value,
            caret,
            select_all,
            ..
        }
        | Kind::Autocomplete {
            value,
            caret,
            select_all,
            ..
        }) = &self.widgets.get(&id)?.kind
        else {
            return None;
        };
        Some(InputSnapshot {
            value: value.clone(),
            caret: *caret,
            select_all: *select_all,
        })
    }

    pub(super) fn remember_input_change(&mut self, id: WidgetId, before: Option<InputSnapshot>) {
        let Some(before) = before else {
            return;
        };
        if let Some(widget) = self.widgets.get_mut(&id) {
            if let Kind::TextInput { value, edit, .. } | Kind::Autocomplete { value, edit, .. } =
                &mut widget.kind
            {
                if *value != before.value {
                    remember(&mut edit.undo, before);
                    edit.redo.clear();
                }
            }
        }
    }

    pub(super) fn input_history(&mut self, id: WidgetId, key: UiKey) -> bool {
        let Some(widget) = self.widgets.get_mut(&id) else {
            return false;
        };
        let (Kind::TextInput {
            value,
            caret,
            select_all,
            edit,
            preedit,
            preedit_cursor,
            ..
        }
        | Kind::Autocomplete {
            value,
            caret,
            select_all,
            edit,
            preedit,
            preedit_cursor,
            ..
        }) = &mut widget.kind
        else {
            return false;
        };
        if !preedit.is_empty() {
            preedit.clear();
            *preedit_cursor = None;
            return true;
        }
        let (from, to) = if key == UiKey::Undo {
            (&mut edit.undo, &mut edit.redo)
        } else {
            (&mut edit.redo, &mut edit.undo)
        };
        let Some(snapshot) = from.pop() else {
            return false;
        };
        remember(
            to,
            InputSnapshot {
                value: std::mem::replace(value, snapshot.value),
                caret: *caret,
                select_all: *select_all,
            },
        );
        *caret = snapshot.caret.min(value.chars().count());
        *select_all = snapshot.select_all;
        *preedit_cursor = None;
        true
    }

    /// Capture a TextInput session for rebuilding the same field.
    pub fn text_input_state(&self, id: WidgetId) -> Option<TextInputState> {
        let Kind::TextInput {
            edit,
            secret,
            preedit,
            preedit_cursor,
            ..
        } = &self.widgets.get(&id)?.kind
        else {
            return None;
        };
        Some(TextInputState {
            snapshot: self.input_snapshot(id)?,
            edit: edit.clone(),
            secret: *secret,
            preedit: preedit.clone(),
            preedit_cursor: *preedit_cursor,
        })
    }

    /// Restore a matching TextInput session without changing its value or focus.
    pub fn restore_text_input_state(&mut self, id: WidgetId, state: &TextInputState) -> bool {
        let Some(widget) = self.widgets.get_mut(&id) else {
            return false;
        };
        let Kind::TextInput {
            value,
            caret,
            select_all,
            edit,
            secret,
            preedit,
            preedit_cursor,
            ..
        } = &mut widget.kind
        else {
            return false;
        };
        if *value != state.snapshot.value || *secret != state.secret {
            return false;
        }
        *caret = state.snapshot.caret.min(value.chars().count());
        *select_all = state.snapshot.select_all;
        *edit = state.edit.clone();
        *preedit = state.preedit.clone();
        *preedit_cursor = state.preedit_cursor;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AutocompleteStyle, ColumnStyle, Scene, TextInputStyle};

    #[test]
    fn integer_input_pointer_press_arms_repeat_until_release() {
        let (mut ui, id) = field(false, "40");
        ui.set_text_input_integer_range(id, Some((0, 100)));
        ui.set_viewport(640.0, 480.0);
        let mut scene = Scene::new();
        ui.sync(&mut scene);
        let bounds = scene.get(ui.widgets[&id].node.unwrap()).unwrap().bounds;
        assert!(ui.pointer_pressed(
            &mut scene,
            bounds.x + bounds.width - 12.0,
            bounds.y + bounds.height * 0.25
        ));
        assert_eq!(ui.text_input_value(id), Some("41"));
        assert!(
            ui.is_dragging(),
            "A pressed integer arrow must retain pointer capture for repeat"
        );
        ui.pointer_released();
        assert!(!ui.is_dragging());
    }

    #[test]
    fn integer_repeat_waits_pauses_stops_and_preserves_exact_values_and_undo() {
        let (mut ui, id) = field(false, "9007199254740993");
        ui.set_text_input_integer_range(id, Some((0, i64::MAX)));
        ui.set_viewport(640.0, 480.0);
        let mut scene = Scene::new();
        ui.sync(&mut scene);
        let bounds = scene.get(ui.widgets[&id].node.unwrap()).unwrap().bounds;
        let (x, y) = (
            bounds.x + bounds.width - 12.0,
            bounds.y + bounds.height * 0.25,
        );
        let due = |ui: &Ui| match &ui.drag {
            Some(DragState::IntegerInput {
                next_repeat: Some(at),
                ..
            }) => *at,
            _ => panic!("expected armed integer arrow"),
        };
        ui.pointer_pressed(&mut scene, x, y);
        let first = due(&ui);
        assert!(ui.pointer_repeat_delay().unwrap() > Duration::from_millis(300));
        assert!(!ui.repeat_pointer_at(first - Duration::from_millis(1)));
        assert_eq!(ui.text_input_value(id), Some("9007199254740994"));
        assert!(ui.repeat_pointer_at(first));
        assert_eq!(ui.text_input_value(id), Some("9007199254740995"));
        assert!(ui.repeat_pointer_at(first + Duration::from_secs(5)));
        assert_eq!(ui.text_input_value(id), Some("9007199254740996"));
        assert!(ui.pointer_moved(bounds.x - 10.0, y));
        assert!(ui.is_dragging());
        assert_eq!(ui.pointer_repeat_delay(), None);
        assert!(!ui.repeat_pointer_at(first + Duration::from_secs(10)));
        assert!(ui.pointer_moved(x, y));
        assert!(due(&ui) >= Instant::now() + Duration::from_millis(300));
        assert!(ui.repeat_pointer_at(due(&ui)));
        ui.sync(&mut scene);
        assert_eq!(
            scene.get(ui.widgets[&id].aux[1]).unwrap().fill,
            ui.theme.colors.accent
        );
        ui.key_input(UiKey::Undo);
        assert!(!ui.is_dragging());
        assert_eq!(ui.text_input_value(id), Some("9007199254740996"));
        assert!(!ui.repeat_pointer_at(first + Duration::from_secs(60)));
        for stop in 0..10 {
            ui.set_viewport(640.0, 480.0);
            ui.set_text_input(id, "40");
            ui.set_text_input_integer_range(id, Some((0, i64::MAX)));
            ui.set_text_input_enabled(id, true);
            ui.pointer_pressed(&mut scene, x, y);
            let next = due(&ui);
            match stop {
                0 => ui.pointer_released(),
                1 => {
                    ui.set_focus(None);
                }
                2 => {
                    ui.ime_preedit("composing", None);
                }
                3 => {
                    ui.set_text_input_enabled(id, false);
                }
                4 => {
                    ui.set_text_input_integer_range(id, None);
                }
                5 => {
                    assert!(ui.cancel_pointer_repeat());
                }
                6 => {
                    ui.paste_into_focused_text("7");
                }
                7 => { ui.set_text_input(id, "70"); }
                8 => { assert!(ui.pointer_scroll(&scene, x, y, 0.0, 40.0)); }
                _ => { ui.set_viewport(800.0, 600.0); }
            }
            assert_eq!(ui.pointer_repeat_delay(), None, "stop {stop}");
            let value = ui.text_input_value(id).unwrap().to_owned();
            assert!(
                !ui.repeat_pointer_at(next + Duration::from_secs(5)),
                "stop {stop}"
            );
            assert_eq!(ui.text_input_value(id), Some(value.as_str()));
            assert!(!ui.is_dragging());
            ui.ime_clear_preedit();
        }
        ui.set_text_input(id, (i64::MAX - 2).to_string());
        ui.set_text_input_integer_range(id, Some((0, i64::MAX)));
        ui.pointer_pressed(&mut scene, x, y);
        let next = due(&ui);
        assert!(ui.repeat_pointer_at(next));
        assert_eq!(ui.text_input_value(id), Some("9223372036854775807"));
        assert!(ui.repeat_pointer_at(next + Duration::from_secs(30)));
        assert_eq!(ui.text_input_value(id), Some("9223372036854775807"));
        ui.sync(&mut scene);
        assert_eq!(scene.get(ui.widgets[&id].aux[1]).unwrap().fill, ui.theme.colors.strip_bg);
        assert_eq!(ui.pointer_repeat_delay(), None);
        for invalid in ["-", "9223372036854775808", "-1"] {
            ui.set_text_input(id, invalid);
            ui.pointer_pressed(&mut scene, x, y);
            assert!(!ui.is_dragging());
            assert_eq!(ui.text_input_value(id), Some(invalid));
        }
        ui.set_text_input(id, "40");
        ui.pointer_pressed(&mut scene, x, y);
        let next = due(&ui);
        let modal = ui.modal(
            "Question",
            "Explanation",
            "Cancel",
            None,
            crate::ModalStyle::default(),
        );
        ui.set_modal_open(modal, true);
        assert_eq!(ui.pointer_repeat_delay(), None);
        assert!(!ui.repeat_pointer_at(next));
        assert!(!ui.is_dragging());
    }

    #[test]
    fn integer_input_steps_exact_bounds_history_clipboard_and_composition() {
        let (mut ui, id) = field(false, "9223372036854775806");
        assert!(ui.set_text_input_integer_range(id, Some((i64::MIN, i64::MAX))));
        assert!(!ui.set_text_input_integer_range(id, Some((2, 1))));
        assert!(ui.key_input(UiKey::Up));
        assert_eq!(ui.text_input_value(id), Some("9223372036854775807"));
        assert!(ui.key_input(UiKey::PageUp));
        assert!(ui.key_input(UiKey::Undo));
        assert_eq!(ui.text_input_value(id), Some("9223372036854775806"));
        assert!(ui.key_input(UiKey::Redo));
        ui.set_text_input(id, "-9223372036854775808");
        ui.key_input(UiKey::Down);
        assert_eq!(ui.text_input_value(id), Some("-9223372036854775808"));
        ui.key_input(UiKey::PageUp);
        assert_eq!(ui.text_input_value(id), Some("-9223372036854775798"));
        ui.key_input(UiKey::SelectAll);
        ui.paste_into_focused_text("9007199254740993");
        ui.key_input(UiKey::Up);
        assert_eq!(ui.text_input_value(id), Some("9007199254740994"));
        ui.set_text_input(id, "-");
        ui.key_input(UiKey::Up);
        assert_eq!(ui.text_input_value(id), Some("-"));
        ui.set_text_input(id, "42");
        ui.ime_preedit("123", Some((3, 3)));
        assert!(!ui.key_input(UiKey::Up));
        assert_eq!(ui.text_input_value(id), Some("42"));
        assert!(matches!(&ui.widgets[&id].kind, Kind::TextInput { preedit, .. } if preedit == "123"));
        ui.key_input(UiKey::Escape);
        let session = ui.text_input_state(id).unwrap();
        let (mut rebuilt, fresh) = field(false, "42");
        rebuilt.set_text_input_integer_range(fresh, Some((0, 100)));
        assert!(rebuilt.restore_text_input_state(fresh, &session));
        rebuilt.key_input(UiKey::Down);
        assert_eq!(rebuilt.text_input_value(fresh), Some("41"));
        rebuilt.key_input(UiKey::Undo);
        assert_eq!(rebuilt.text_input_value(fresh), Some("42"));
        rebuilt.set_text_input(fresh, "101");
        rebuilt.key_input(UiKey::Up);
        rebuilt.key_input(UiKey::Down);
        assert_eq!(rebuilt.text_input_value(fresh), Some("101"));
        rebuilt.set_text_input(fresh, "42");
        rebuilt.set_text_input_enabled(fresh, false);
        assert!(!rebuilt.step_integer_input(fresh, 1));
        assert_eq!(rebuilt.text_input_value(fresh), Some("42"));
        rebuilt.set_text_input_secret(fresh, true);
        assert_eq!(rebuilt.text_input_integer_range(fresh), None);
        assert!(!rebuilt.set_text_input_integer_range(fresh, Some((0, 10))));
    }

    #[test]
    fn integer_input_pointer_steps_keep_editor_focus_and_reserve_text_space() {
        for theme in [crate::Theme::dark(), crate::Theme::light()] {
            let (mut ui, id) = field(false, "9007199254740993");
            ui.set_theme(theme);
            ui.set_text_input_integer_range(id, Some((0, i64::MAX)));
            ui.set_viewport(640.0, 480.0);
            let mut scene = Scene::new();
            ui.sync(&mut scene);
            let widget = &ui.widgets[&id];
            let bounds = scene.get(widget.node.unwrap()).unwrap().bounds;
            let x = bounds.x + bounds.width - 12.0;
            let top = bounds.y + bounds.height * 0.25;
            assert!(ui.pointer_press(&mut scene, x, top, false));
            assert_eq!(ui.focused(), Some(id));
            assert_eq!(ui.text_input_value(id), Some("9007199254740994"));
            assert!(ui.pointer_press(&mut scene, x, bounds.y + bounds.height * 0.75, false));
            assert_eq!(ui.text_input_value(id), Some("9007199254740993"));
            ui.sync(&mut scene);
            assert_eq!(scene.get(ui.widgets[&id].aux[3]).unwrap().fill, theme.colors.accent);
            assert_eq!(scene.get(ui.widgets[&id].aux[4]).unwrap().fill, theme.colors.on_accent);
            let label = scene.get(ui.widgets[&id].label_node.unwrap()).unwrap();
            assert!(label.clip.unwrap().x + label.clip.unwrap().width <= bounds.x + bounds.width - 24.0);
            let candidate = ui.ime_cursor_area(&scene).unwrap();
            assert!(candidate.0 + candidate.2 <= bounds.x + bounds.width - 24.0);
            assert_eq!(ui.widgets[&id].aux.len(), 5);
            assert!(ui.widgets[&id].aux[1..].iter().all(|n| scene.get(*n).unwrap().visible));
            ui.set_text_input_integer_range(id, Some((0, 100)));
            ui.set_text_input(id, "101");
            ui.sync(&mut scene);
            for n in [ui.widgets[&id].aux[2], ui.widgets[&id].aux[4]] {
                assert_eq!(scene.get(n).unwrap().fill, theme.colors.disabled);
            }
            ui.pointer_press(&mut scene, x, top, false);
            assert_eq!(ui.text_input_value(id), Some("101"));
            ui.set_text_input(id, "42");
            ui.ime_preedit("123", Some((3, 3)));
            ui.pointer_press(&mut scene, x, top, false);
            assert_eq!(ui.text_input_value(id), Some("42"));
            assert!(matches!(&ui.widgets[&id].kind, Kind::TextInput { preedit, .. } if preedit == "123"));
            ui.key_input(UiKey::Escape);
            ui.set_text_input_integer_range(id, None);
            ui.sync(&mut scene);
            assert!(ui.widgets[&id].aux[1..].iter().all(|n| !scene.get(*n).unwrap().visible));
        }
    }

    fn field(autocomplete: bool, value: &str) -> (Ui, WidgetId) {
        let mut ui = Ui::new();
        let mut id = WidgetId(0);
        ui.column(ColumnStyle::default(), |c| {
            id = if autocomplete {
                c.autocomplete(["INTEGER", "TEXT"], value, AutocompleteStyle::default())
            } else {
                c.text_input_styled(value, "name", TextInputStyle::default())
            };
        });
        ui.set_focus(Some(id));
        (ui, id)
    }
    fn value(ui: &Ui, id: WidgetId) -> &str {
        ui.text_input_value(id)
            .or_else(|| ui.autocomplete_value(id))
            .unwrap()
    }

    #[test]
    fn single_line_paint_keeps_multiline_documents_and_matches_ime_scroll_and_secret_geometry() {
        let original=format!("REFERENCES parent(id)\r\nON UPDATE CASCADE\t中文{}"," long".repeat(30));
        for autocomplete in [false,true] {
            let (mut ui,id)=field(autocomplete,"");let mut scene=Scene::new();
            assert!(ui.paste_into_focused_text(&original));ui.sync(&mut scene);
            let (display,caret,composition)=ui.single_line_input_display(id).unwrap();
            assert_eq!(display,original.replace(['\r','\n','\t']," "));assert_eq!(caret,original.chars().count());assert!(composition.is_empty());assert_eq!(value(&ui,id),original);
            let w=&ui.widgets[&id];let (size,padding)=match &w.kind {Kind::TextInput {style,..}=>(style.size,style.padding_x),Kind::Autocomplete {style,..}=>(style.size,style.padding),_=>unreachable!()};
            let (_,expected)=ui.font.rasterize_text(&display,size,None);let mask=scene.get(w.label_node.unwrap()).unwrap().mask.as_ref().unwrap();assert_eq!((mask.width,mask.height,&mask.pixels),(expected.width,expected.height,&expected.pixels));
            let bounds=scene.get(w.node.unwrap()).unwrap().bounds;let at=ui.ime_cursor_area(&scene).unwrap();assert!((at.0-(bounds.x+bounds.width-padding-2.0)).abs()<0.01);
            if !autocomplete {assert_eq!(at.0,scene.get(w.chrome_node.unwrap()).unwrap().bounds.x);}
            ui.key_input(UiKey::SelectAll);assert_eq!(ui.focused_text_for_copy().as_deref(),Some(original.as_str()));ui.ime_preedit("候\n選",Some((4,4)));ui.sync(&mut scene);
            assert_eq!(ui.single_line_input_display(id).unwrap(),("候 選".into(),2,0..3));let at=ui.ime_cursor_area(&scene).unwrap();let (width,_)=ui.font.measure_line("候 ",size);assert!((at.0-(bounds.x+padding+width)).abs()<0.01);
            ui.ime_commit("候選");ui.key_input(UiKey::Undo);assert_eq!(value(&ui,id),original);ui.key_input(UiKey::Redo);assert_eq!(value(&ui,id),"候選");
            if !autocomplete {
                ui.set_text_input(id,&original);ui.set_text_input_secret(id,true);ui.key_input(UiKey::End);ui.sync(&mut scene);let (masked,_,_)=ui.single_line_input_display(id).unwrap();assert_eq!(masked,"*".repeat(original.chars().count()));assert_eq!(ui.focused_text_for_copy(),None);
                let at=ui.ime_cursor_area(&scene).unwrap();let w=&ui.widgets[&id];assert_eq!(at.0,scene.get(w.chrome_node.unwrap()).unwrap().bounds.x);
            }
        }
    }

    #[test]
    fn input_selection_contrasts_in_both_themes_and_long_carets_and_ime_stay_visible() {
        for theme in [crate::Theme::dark(),crate::Theme::light()] {
            let mut ui=Ui::new();ui.set_theme(theme);let mut scene=Scene::new();let mut id=WidgetId(0);
            let mut style=TextInputStyle::from_theme(&theme);style.width=80.0;
            let original="very_long_列名称_for_cell_editing";
            ui.column(ColumnStyle::default(),|c|{id=c.text_input_styled(original,"",style);});ui.set_focus(Some(id));ui.key_input(UiKey::SelectAll);ui.sync(&mut scene);
            let w=&ui.widgets[&id];let (host,label,caret)=(w.node.unwrap(),w.label_node.unwrap(),w.chrome_node.unwrap());let bounds=scene.get(host).unwrap().bounds;
            assert_eq!(scene.get(host).unwrap().fill,style.focus_border);assert_eq!(scene.get(label).unwrap().fill,crate::Color::rgb(255,255,255));assert_eq!(scene.get(host).unwrap().clip,Some(bounds));
            ui.set_focus(None);ui.sync(&mut scene);assert_eq!(scene.get(host).unwrap().fill,style.bg);assert_eq!(scene.get(label).unwrap().fill,style.fg);
            ui.set_focus(Some(id));ui.key_input(UiKey::Right);ui.key_input(UiKey::Char('x'));ui.sync(&mut scene);let caret_bounds=scene.get(caret).unwrap().bounds;
            assert!(caret_bounds.x>=bounds.x && caret_bounds.x+caret_bounds.width<=bounds.x+bounds.width);assert!(scene.get(label).unwrap().bounds.x<bounds.x);
            ui.key_input(UiKey::SelectAll);ui.ime_preedit("候選",Some((3,3)));ui.sync(&mut scene);assert_eq!(scene.get(host).unwrap().fill,style.bg);
            let (_,mask)=ui.font.rasterize_text("候選",style.size,None);assert_eq!(scene.get(label).unwrap().bounds.width,mask.width as f32);assert_eq!(ui.text_input_value(id),Some(format!("{original}x").as_str()));
            ui.key_input(UiKey::Escape);assert!(ui.ime_commit("中文"));assert_eq!(ui.text_input_value(id),Some("中文"));ui.key_input(UiKey::Undo);assert_eq!(ui.text_input_value(id),Some(format!("{original}x").as_str()));
        }
    }

    #[test]
    fn input_history_covers_native_edit_routes_and_invalidates_redo_only_for_changes() {
        for autocomplete in [false, true] {
            let (mut ui, id) = field(autocomplete, "中文");
            ui.key_input(UiKey::SelectAll);
            assert!(ui.paste_into_focused_text("customer_name"));
            assert!(ui.key_input(UiKey::Undo));
            assert_eq!(value(&ui, id), "中文");
            assert!(ui.key_input(UiKey::Redo));
            assert_eq!(value(&ui, id), "customer_name");
            assert!(ui.delete_focused_text_selection());
            assert_eq!(value(&ui, id), "");
            assert!(ui.key_input(UiKey::Undo));
            assert_eq!(value(&ui, id), "customer_name");
            ui.key_input(UiKey::Home);
            assert!(ui.key_input(UiKey::Delete));
            assert!(ui.key_input(UiKey::Undo));
            assert_eq!(value(&ui, id), "customer_name");
            ui.key_input(UiKey::SelectAll);
            ui.ime_preedit("列", Some((3, 3)));
            assert!(!ui.key_input(UiKey::Char('x')));
            assert!(ui.ime_commit("列名"));
            assert!(ui.key_input(UiKey::Undo));
            assert_eq!(value(&ui, id), "customer_name");
            assert!(ui.key_input(UiKey::Redo));
            assert_eq!(value(&ui, id), "列名");
            ui.key_input(UiKey::End);
            assert!(ui.key_input(UiKey::Char('!')));
            assert!(ui.key_input(UiKey::Undo));
            assert_eq!(value(&ui, id), "列名");
            ui.key_input(UiKey::Home);
            assert!(!ui.key_input(UiKey::Backspace));
            assert!(ui.key_input(UiKey::Redo));
            assert_eq!(value(&ui, id), "列名!");
            ui.key_input(UiKey::SelectAll);
            assert!(ui.paste_into_focused_text("same"));
            ui.key_input(UiKey::SelectAll);
            assert!(ui.paste_into_focused_text("same"));
            assert!(ui.key_input(UiKey::Undo));
            assert_eq!(value(&ui, id), "列名!");
            ui.key_input(UiKey::End);
            ui.key_input(UiKey::Backspace);
            assert!(!ui.key_input(UiKey::Redo));
            assert_eq!(value(&ui, id), "列名");
            ui.ime_preedit("候", Some((3, 3)));
            assert!(ui.key_input(UiKey::Undo));
            assert_eq!(value(&ui, id), "列名");
            assert!(ui.key_input(UiKey::Undo));
            assert_eq!(value(&ui, id), "列名!");
            if autocomplete {
                ui.set_autocomplete_value(id, "TEXT");
            } else {
                ui.set_text_input(id, "TEXT");
            }
            assert!(!ui.key_input(UiKey::Undo));
            assert!(!ui.key_input(UiKey::Redo));
        }
    }

    #[test]
    fn input_sessions_and_autocomplete_acceptance_keep_bounded_history_and_composition() {
        let (mut ui, id) = field(false, "name");
        ui.key_input(UiKey::SelectAll);
        ui.paste_into_focused_text("new_name");
        ui.key_input(UiKey::Home);
        ui.ime_preedit("新", Some((3, 3)));
        let session = ui.text_input_state(id).unwrap();
        let (mut rebuilt, next) = field(false, "new_name");
        assert!(rebuilt.restore_text_input_state(next, &session));
        let mut scene = Scene::new();
        rebuilt.sync(&mut scene);
        assert!(rebuilt.ime_cursor_area(&scene).is_some());
        rebuilt.ime_commit("新");
        assert_eq!(value(&rebuilt, next), "新new_name");
        rebuilt.key_input(UiKey::Undo);
        assert_eq!(value(&rebuilt, next), "new_name");
        rebuilt.key_input(UiKey::Undo);
        assert_eq!(value(&rebuilt, next), "name");
        assert!(!rebuilt.restore_text_input_state(next, &session));
        rebuilt.set_text_input(next, "new_name");
        rebuilt.set_text_input_secret(next, true);
        assert!(!rebuilt.restore_text_input_state(next, &session));
        rebuilt.set_text_input_secret(next, false);
        for _ in 0..105 {
            rebuilt.key_input(UiKey::Char('a'));
        }
        for _ in 0..100 {
            assert!(rebuilt.key_input(UiKey::Undo));
        }
        assert!(!rebuilt.key_input(UiKey::Undo));
        assert_eq!(value(&rebuilt, next), "new_nameaaaaa");
        let (mut ui, id) = field(true, "");
        ui.paste_into_focused_text("EX");
        ui.key_input(UiKey::Enter);
        assert_eq!(ui.take_autocomplete_change(), Some((id, "TEXT".into())));
        assert_eq!(value(&ui, id), "TEXT");
        let session = ui.autocomplete_state(id).unwrap();
        let (mut rebuilt, next) = field(true, "TEXT");
        assert!(rebuilt.set_autocomplete_state(next, &session));
        assert!(rebuilt.key_input(UiKey::Undo));
        assert_eq!(value(&rebuilt, next), "EX");
        assert!(rebuilt.key_input(UiKey::Undo));
        assert_eq!(value(&rebuilt, next), "");
        assert!(rebuilt.key_input(UiKey::Redo));
        assert!(rebuilt.key_input(UiKey::Redo));
        assert_eq!(value(&rebuilt, next), "TEXT");
    }
}
