use crate::{Kind, Ui, UiKey, WidgetId};

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
