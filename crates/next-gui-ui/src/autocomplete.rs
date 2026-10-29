use crate::{AlphaMask, Kind, Rect, Scene, Ui, WidgetId};
use std::ops::Range;

/// Autocomplete editing and suggestion session, independent of widget IDs.
#[derive(Debug, Clone)]
pub struct AutocompleteState {
    value: String,
    edit: crate::text_input::InputEdit,
    caret: usize,
    select_all: bool,
    preedit: String,
    preedit_cursor: Option<(usize, usize)>,
    open: bool,
    show_all: bool,
    highlighted: Option<String>,
}

/// A Select popup session, restored only while its selected value still matches.
#[derive(Debug, Clone)]
pub struct SelectState {
    value: Option<String>,
    open: bool,
}

impl Ui {
    /// Snapshot a Select popup without retaining widget IDs or option indices.
    pub fn select_state(&self,id:WidgetId)->Option<SelectState> {
        let Kind::Select {open,..}=&self.widgets.get(&id)?.kind else {return None;};
        Some(SelectState {value:self.select_value(id).map(str::to_owned),open:*open})
    }

    /// Restore popup visibility when the selected value still matches; does not focus or emit.
    pub fn restore_select_state(&mut self,id:WidgetId,state:&SelectState)->bool {
        if !self.is_select(id) || self.select_value(id)!=state.value.as_deref() {return false;}
        if state.open {self.close_popup_internal();}
        if let Some(crate::Widget {kind:Kind::Select {open,options,..},..})=self.widgets.get_mut(&id) {
            *open=state.open && !options.is_empty();
            if *open {self.open_popup=Some(id);} else if self.open_popup==Some(id) {self.open_popup=None;}
        }
        true
    }

    /// Snapshot text editing, IME composition and the highlighted suggestion.
    pub fn autocomplete_state(&self, id: WidgetId) -> Option<AutocompleteState> {
        let Kind::Autocomplete {
            value,
            edit,
            caret,
            select_all,
            preedit,
            preedit_cursor,
            open,
            show_all,
            highlight,
            ..
        } = &self.widgets.get(&id)?.kind
        else {
            return None;
        };
        Some(AutocompleteState {
            value: value.clone(),
            edit: edit.clone(),
            caret: *caret,
            select_all: *select_all,
            preedit: preedit.clone(),
            preedit_cursor: *preedit_cursor,
            open: *open,
            show_all: *show_all,
            highlighted: highlight.and_then(|i| {
                self.autocomplete_filtered(id)
                    .get(i)
                    .map(|(_, s)| s.clone())
            }),
        })
    }

    /// Restore a session only when its text still matches; does not change focus or emit.
    pub fn set_autocomplete_state(&mut self, id: WidgetId, state: &AutocompleteState) -> bool {
        if self.autocomplete_value(id) != Some(state.value.as_str()) {
            return false;
        }
        if let Some(crate::Widget {kind:Kind::Autocomplete {show_all,..},..})=self.widgets.get_mut(&id) {*show_all=state.show_all;}
        let filtered = self.autocomplete_filtered(id);
        let open = state.open && state.preedit.is_empty() && !filtered.is_empty();
        if open {
            self.close_popup_internal();
        }
        let Some(widget) = self.widgets.get_mut(&id) else {
            return false;
        };
        let Kind::Autocomplete {
            caret,
            edit,
            select_all,
            preedit,
            preedit_cursor,
            open: popup,
            highlight,
            ..
        } = &mut widget.kind
        else {
            return false;
        };
        *caret = state.caret.min(state.value.chars().count());
        *edit = state.edit.clone();
        *select_all = state.select_all;
        *preedit = state.preedit.clone();
        *preedit_cursor = state.preedit_cursor;
        *popup = open;
        *highlight = state
            .highlighted
            .as_ref()
            .and_then(|s| filtered.iter().position(|(_, label)| label == s));
        if open {
            self.open_popup = Some(id);
        } else if self.open_popup == Some(id) {
            self.open_popup = None;
        }
        true
    }

    /// Open every option without changing the text, for an editable combo's dropdown arrow.
    /// Typing returns to filtered suggestions; the session preserves this popup across rebuilds.
    pub fn open_autocomplete_options(&mut self, id: WidgetId) -> bool {
        if !matches!(self.widgets.get(&id).map(|w|&w.kind),Some(Kind::Autocomplete {options,preedit,..}) if !options.is_empty() && preedit.is_empty()) {return false;}
        self.close_popup_internal();
        if let Some(crate::Widget {kind:Kind::Autocomplete {open,show_all,highlight,options,value,..},..})=self.widgets.get_mut(&id) {
            *open=true;*show_all=true;*highlight=options.iter().position(|option|option==value).or(Some(0));
        }
        self.open_popup=Some(id);true
    }

    pub(super) fn refresh_autocomplete_popup(&mut self, id: WidgetId) {
        self.close_popup_internal();
        if let Some(crate::Widget {kind:Kind::Autocomplete {show_all,..},..})=self.widgets.get_mut(&id) {*show_all=false;}
        let matches = !self.autocomplete_filtered(id).is_empty();
        if let Some(widget) = self.widgets.get_mut(&id) {
            if let Kind::Autocomplete {
                open,
                highlight,
                preedit,
                ..
            } = &mut widget.kind
            {
                *open = matches && preedit.is_empty();
                *highlight = (*open).then_some(0);
                if *open {
                    self.open_popup = Some(id);
                }
            }
        }
    }

    pub(super) fn autocomplete_popup_window(
        &self,
        scene: &Scene,
        id: WidgetId,
    ) -> Option<(Rect, Range<usize>)> {
        let widget = self.widgets.get(&id)?;
        let Kind::Autocomplete {
            style,
            highlight,
            open: true,
            ..
        } = &widget.kind
        else {
            return None;
        };
        let host = scene.get(widget.node?)?.bounds;
        let count = self.autocomplete_filtered(id).len();
        self.choice_popup_window(host,count,*highlight,style.item_height,style.popup_max_height,style.width)
    }

    pub(super) fn select_popup_window(&self,scene:&Scene,id:WidgetId)->Option<(Rect,Range<usize>)> {
        let widget=self.widgets.get(&id)?;
        let Kind::Select {options,selected,style,open:true,..}=&widget.kind else {return None;};
        let host=scene.get(widget.node?)?.bounds;
        self.choice_popup_window(host,options.len(),*selected,style.item_height,style.popup_max_height,style.width)
    }

    fn choice_popup_window(&self,host:Rect,count:usize,selected:Option<usize>,item_height:f32,max_height:f32,min_width:f32)->Option<(Rect,Range<usize>)> {
        if count == 0 || item_height <= 0.0 {
            return None;
        }
        let desired = (count as f32 * item_height).min(max_height);
        let below = (self.viewport.1 - host.y - host.height).max(0.0);
        let above = host.y.max(0.0);
        let down = below >= desired || below >= above;
        let space = if down { below } else { above };
        let rows = (space.min(desired) / item_height).floor() as usize;
        if rows == 0 {
            return None;
        }
        let height = rows as f32 * item_height;
        let width = host.width.max(min_width).min(self.viewport.0);
        let first = selected
            .unwrap_or(0)
            .min(count - 1)
            .saturating_sub(rows - 1);
        let panel = Rect::new(
            host.x.clamp(0.0, self.viewport.0 - width),
            if down {
                host.y + host.height
            } else {
                host.y - height
            },
            width,
            height,
        );
        Some((panel, first..(first + rows).min(count)))
    }

    pub(super) fn autocomplete_cursor_area(
        &self,
        scene: &Scene,
        id: WidgetId,
    ) -> Option<(f32, f32, f32, f32)> {
        let widget = self.widgets.get(&id)?;
        let Kind::Autocomplete { style, .. } = &widget.kind else {
            return None;
        };
        let bounds = scene.get(widget.node?)?.bounds;
        let (text, caret, _) = self.single_line_input_display(id)?;
        let prefix: String = text.chars().take(caret).collect();
        let (width, _) = self.font.measure_line(&prefix, style.size);
        let visible = (bounds.width - 2.0 * style.padding - 2.0).max(0.0);
        Some((
            bounds.x + style.padding + width.min(visible),
            bounds.y + 4.0,
            2.0,
            (bounds.height - 8.0).max(8.0),
        ))
    }

    pub(super) fn paint_autocomplete_field(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(widget) = self.widgets.get(&id) else {
            return;
        };
        let Kind::Autocomplete {
            value,
            select_all,
            preedit_cursor,
            placeholder,
            style,
            ..
        } = &widget.kind
        else {
            return;
        };
        let Some(host) = widget.node else {
            return;
        };
        let label = widget.label_node;
        let chrome = widget.chrome_node;
        let style = *style;
        let focused = self.focus == Some(id);
        let select_all = focused && (*select_all || self.text_input_selection(id).is_some_and(|r|r.start==0 && r.end==value.chars().count())) && !value.is_empty();
        let cursor_visible = preedit_cursor.is_some();
        let placeholder = placeholder.replace(['\r','\n','\t']," ");
        let (text, caret, composition) = self.single_line_input_display(id).unwrap();
        let composing = composition.start != composition.end;
        let selected = select_all && !composing;
        let bounds = scene.get(host).map_or_else(Rect::empty, |n| n.bounds);
        scene.set_clip(host, Some(bounds));
        scene.set_fill(
            host,
            if selected {
                style.selected_bg
            } else {
                style.bg
            },
        );
        scene.set_mask(host, None);
        scene.set_visible(host, true);
        self.apply_corner_radius(scene, host, false);
        let prefix: String = text.chars().take(caret).collect();
        let (cw, _) = self.font.measure_line(&prefix, style.size);
        let visible = (bounds.width - 2.0 * style.padding - 2.0).max(0.0);
        let scroll = if focused {
            (cw - visible).max(0.0)
        } else {
            0.0
        };
        let lx = bounds.x + style.padding - scroll;
        let fg = if text.is_empty() {
            style.placeholder_fg
        } else if selected {
            style.selected_fg
        } else {
            style.fg
        };
        let display = if text.is_empty() { &placeholder } else { &text };
        let (_, mask) = self.font.rasterize_text(display, style.size, None);
        let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
        if let Some(label) = label {
            scene.set_bounds(
                label,
                Rect::new(lx, ly, mask.width as f32, mask.height as f32),
            );
            scene.set_fill(label, fg);
            scene.set_mask(
                label,
                Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
            );
            scene.set_visible(label, true);
        }
        if let Some(chrome) = chrome {
            scene.set_visible(chrome, focused);
            if focused {
                let width = bounds.width.ceil().max(1.0) as u32;
                let height = bounds.height.ceil().max(1.0) as u32;
                let mut pixels = vec![0u8; width as usize * height as usize];
                let mut stroke = |x: f32, y: f32, w: f32, h: f32| {
                    for py in y.max(0.0) as u32..(y + h).ceil().max(0.0).min(height as f32) as u32 {
                        for px in
                            x.max(0.0) as u32..(x + w).ceil().max(0.0).min(width as f32) as u32
                        {
                            pixels[(py * width + px) as usize] = 255;
                        }
                    }
                };
                if !composing || cursor_visible {
                    stroke(
                        style.padding + cw - scroll,
                        4.0,
                        2.0,
                        (bounds.height - 8.0).max(8.0),
                    );
                }
                if composing {
                    let before: String = text.chars().take(composition.start).collect();
                    let preedit: String = text
                        .chars()
                        .skip(composition.start)
                        .take(composition.end - composition.start)
                        .collect();
                    let (bw, _) = self.font.measure_line(&before, style.size);
                    let (pw, _) = self.font.measure_line(&preedit, style.size);
                    stroke(
                        style.padding + bw - scroll,
                        bounds.height - 3.0,
                        pw.max(1.0),
                        2.0,
                    );
                }
                scene.set_bounds(chrome, bounds);
                scene.set_fill(chrome, style.focus_border);
                scene.set_mask(chrome, Some(AlphaMask::new(width, height, pixels)));
            }
        }
        self.paint_input_selection(scene,id,style.size,(lx,ly),bounds,(style.selected_bg,style.selected_fg));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn select_sessions_keep_popup_by_value_without_reopening_changed_values_or_emitting() {
        let mut ui=crate::Ui::new();let mut id=crate::WidgetId(0);let mut other=crate::WidgetId(0);
        ui.column(crate::ColumnStyle::default(),|c|{id=c.select(["A","B"],Some(1),"",crate::SelectStyle::default());other=c.select(["Other"],None,"",crate::SelectStyle::default());});
        ui.set_focus(Some(id));ui.key_input(crate::UiKey::Space);let session=ui.select_state(id).unwrap();
        ui.set_focus(Some(other));ui.key_input(crate::UiKey::Space);assert!(ui.restore_select_state(id,&session));assert_eq!(ui.focused(),Some(other));assert!(matches!(ui.widgets[&other].kind,crate::Kind::Select {open:false,..}));
        let mut rebuilt=crate::Ui::new();let mut next=crate::WidgetId(0);rebuilt.column(crate::ColumnStyle::default(),|c|{next=c.select(["B","A"],Some(0),"",crate::SelectStyle::default());});
        assert!(rebuilt.restore_select_state(next,&session));assert_eq!(rebuilt.select_value(next),Some("B"));assert!(rebuilt.has_open_popup());assert_eq!(rebuilt.focused(),None);
        rebuilt.set_focus(Some(next));rebuilt.key_input(crate::UiKey::Escape);let closed=rebuilt.select_state(next).unwrap();rebuilt.key_input(crate::UiKey::Space);assert!(rebuilt.restore_select_state(next,&closed));assert!(!rebuilt.has_open_popup());
        rebuilt.set_select_selected(next,Some(1));assert!(!rebuilt.restore_select_state(next,&session));assert!(!rebuilt.has_open_popup());assert!(!rebuilt.restore_select_state(crate::WidgetId(0),&session));
    }

    #[test]
    fn select_popup_fits_viewport_reveals_selection_and_hits_the_visible_option() {
        for theme in [crate::Theme::dark(),crate::Theme::light()] {
            let mut ui=crate::Ui::new();ui.set_theme(theme);ui.set_viewport(400.0,300.0);let mut scene=crate::Scene::new();let mut id=crate::WidgetId(0);
            let style=crate::SelectStyle::from_theme(&theme);
            ui.column(crate::ColumnStyle::default(),|c|{id=c.select((0..100).map(|i|format!("Option {i}")),Some(95),"",style);});
            ui.sync(&mut scene);let host=ui.widgets[&id].node.unwrap();scene.set_bounds(host,crate::Rect::new(350.0,260.0,48.0,30.0));ui.set_focus(Some(id));assert!(ui.key_input(crate::UiKey::Space));
            ui.paint_select_field(&mut scene,id);ui.paint_select_popup(&mut scene,id);
            let (panel,visible)=ui.select_popup_window(&scene,id).unwrap();assert!(panel.x>=0.0 && panel.x+panel.width<=400.0);assert!(panel.y>=0.0 && panel.y+panel.height<=260.0);assert!(visible.contains(&95));
            let aux=&ui.widgets[&id].aux;assert!(aux.len()<20);let selected_label=scene.get(aux[2+(95-visible.start)*2]).unwrap();assert_eq!(selected_label.fill,style.selected_fg);assert_eq!(scene.get(ui.widgets[&id].label_node.unwrap()).unwrap().fill,style.selected_fg);
            assert!(ui.pointer_popup_item(&mut scene,id,panel.x+10.0,panel.y+style.item_height*0.5));assert_eq!(ui.select_selected(id),Some(visible.start));assert!(!ui.has_open_popup());
            ui.set_select_selected(id,Some(99));ui.key_input(crate::UiKey::Space);ui.paint_select_popup(&mut scene,id);let (_,visible)=ui.select_popup_window(&scene,id).unwrap();assert!(visible.contains(&99));
            let (panel,_)=ui.select_popup_window(&scene,id).unwrap();assert!(ui.pointer_scroll(&scene,panel.x+5.0,panel.y+5.0,0.0,-26.0));assert_eq!(ui.select_selected(id),Some(98));
            scene.set_bounds(host,crate::Rect::new(0.0,0.0,48.0,30.0));ui.paint_select_popup(&mut scene,id);let (panel,_)=ui.select_popup_window(&scene,id).unwrap();assert!(panel.y>=30.0 && panel.y+panel.height<=300.0);
        }
    }

    #[test]
    fn all_options_dropdown_keeps_value_history_session_and_resumes_filtering() {
        let mut ui=crate::Ui::new();let mut id=crate::WidgetId(0);
        ui.column(crate::ColumnStyle::default(),|c|{id=c.autocomplete(["INTEGER","TEXT","REAL"],"TEXT",crate::AutocompleteStyle::default());});
        assert!(ui.open_autocomplete_options(id));assert_eq!(ui.autocomplete_value(id),Some("TEXT"));assert_eq!(ui.autocomplete_filtered(id).len(),3);
        ui.set_focus(Some(id));assert!(ui.key_input(crate::UiKey::Down));
        let session=ui.autocomplete_state(id).unwrap();
        let mut rebuilt=crate::Ui::new();let mut next=crate::WidgetId(0);rebuilt.column(crate::ColumnStyle::default(),|c|{next=c.autocomplete(["INTEGER","TEXT","REAL"],"TEXT",crate::AutocompleteStyle::default());});
        assert!(rebuilt.set_autocomplete_state(next,&session));rebuilt.set_focus(Some(next));assert!(rebuilt.has_open_popup());
        assert!(rebuilt.key_input(crate::UiKey::Enter));assert_eq!(rebuilt.autocomplete_value(next),Some("REAL"));
        assert!(rebuilt.key_input(crate::UiKey::Undo));assert_eq!(rebuilt.autocomplete_value(next),Some("TEXT"));
        rebuilt.key_input(crate::UiKey::SelectAll);rebuilt.paste_into_focused_text("IN");assert_eq!(rebuilt.autocomplete_filtered(next),vec![(0,"INTEGER".into())]);
        rebuilt.ime_preedit("候選",None);assert!(!rebuilt.open_autocomplete_options(next));assert!(!rebuilt.open_autocomplete_options(crate::WidgetId(0)));
    }

    use super::*;
    use crate::{AutocompleteStyle, ColumnStyle, UiKey};

    fn field(options: &[&str], value: &str) -> (Ui, Scene, WidgetId) {
        let mut ui = Ui::new();
        ui.set_viewport(400.0, 300.0);
        let mut id = WidgetId(0);
        ui.column(ColumnStyle::default(), |col| {
            id = col.autocomplete(
                options.iter().copied(),
                value,
                AutocompleteStyle {
                    width: 120.0,
                    ..AutocompleteStyle::default()
                },
            );
        });
        ui.set_focus(Some(id));
        let mut scene = Scene::new();
        ui.sync(&mut scene);
        (ui, scene, id)
    }

    #[test]
    fn session_restores_named_suggestion_selection_and_composition_geometry() {
        let (mut ui, mut scene, id) = field(&["TEXT", "INTEGER"], "");
        assert!(ui.key_input(UiKey::Down));
        assert!(ui.key_input(UiKey::Down));
        let session = ui.autocomplete_state(id).unwrap();
        let (mut rebuilt, _, new_id) = field(&["INTEGER", "TEXT"], "");
        assert!(rebuilt.set_autocomplete_state(new_id, &session));
        assert!(rebuilt.key_input(UiKey::Enter));
        assert_eq!(rebuilt.autocomplete_value(new_id), Some("INTEGER"));
        assert!(!rebuilt.set_autocomplete_state(new_id, &session));
        assert!(!rebuilt.set_autocomplete_state(WidgetId(0), &session));

        ui.set_autocomplete_value(id, "VARYING CHARACTER(123456789)");
        ui.key_input(UiKey::Home);
        ui.key_input(UiKey::Right);
        ui.ime_preedit("中文", Some((3, 3)));
        ui.sync(&mut scene);
        assert!(!ui.has_open_popup());
        assert_eq!(
            ui.single_line_input_display(id).unwrap(),
            ("V中文ARYING CHARACTER(123456789)".into(), 2, 1..3)
        );
        let at = ui.ime_cursor_area(&scene).unwrap();
        let session = ui.autocomplete_state(id).unwrap();
        let (mut rebuilt, mut next_scene, new_id) = field(&[], "VARYING CHARACTER(123456789)");
        assert!(rebuilt.set_autocomplete_state(new_id, &session));
        rebuilt.sync(&mut next_scene);
        assert_eq!(rebuilt.ime_cursor_area(&next_scene), Some(at));
        assert!(rebuilt.ime_commit("中"));
        assert_eq!(
            rebuilt.autocomplete_value(new_id),
            Some("V中ARYING CHARACTER(123456789)")
        );
        rebuilt.key_input(UiKey::End);
        rebuilt.sync(&mut next_scene);
        let host = next_scene
            .get(rebuilt.widgets[&new_id].node.unwrap())
            .unwrap()
            .bounds;
        let caret = rebuilt.ime_cursor_area(&next_scene).unwrap();
        assert!(caret.0 >= host.x && caret.0 + caret.2 <= host.x + host.width);

        rebuilt.key_input(UiKey::SelectAll);
        let selection = rebuilt.autocomplete_state(new_id).unwrap();
        assert!(rebuilt.set_autocomplete_state(new_id, &selection));
        assert!(rebuilt.ime_preedit("型", None));
        assert_eq!(rebuilt.single_line_input_display(new_id).unwrap().0, "型");
        rebuilt.set_focus(None);
        rebuilt.sync(&mut next_scene);
        assert!(rebuilt
            .autocomplete_state(new_id)
            .unwrap()
            .preedit
            .is_empty());
        rebuilt.set_focus(Some(new_id));
        assert!(rebuilt.delete_focused_text_selection());
        assert_eq!(rebuilt.autocomplete_value(new_id), Some(""));
        assert!(!rebuilt.delete_focused_text_selection());
    }

    #[test]
    fn popup_stays_in_viewport_and_pointer_selects_the_scrolled_suggestion() {
        let mut ui = Ui::new();
        ui.set_viewport(400.0, 300.0);
        let options: Vec<_> = (0..20).map(|i| format!("Type {i}")).collect();
        let mut id = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 350.0,
                y: 240.0,
                ..ColumnStyle::default()
            },
            |col| {
                id = col.autocomplete(
                    options,
                    "",
                    AutocompleteStyle {
                        width: 120.0,
                        ..AutocompleteStyle::default()
                    },
                );
            },
        );
        ui.set_focus(Some(id));
        ui.key_input(UiKey::Down);
        let mut scene = Scene::new();
        ui.sync(&mut scene);
        let (panel, visible) = ui.autocomplete_popup_window(&scene, id).unwrap();
        let host = scene.get(ui.widgets[&id].node.unwrap()).unwrap().bounds;
        assert!(panel.x >= 0.0 && panel.x + panel.width <= 400.0);
        assert!(panel.y >= 0.0 && panel.y + panel.height <= host.y);
        assert_eq!(visible.start, 0);
        assert!(visible.end < 20);
        for _ in 0..19 {
            ui.key_input(UiKey::Down);
        }
        ui.sync(&mut scene);
        let (panel, visible) = ui.autocomplete_popup_window(&scene, id).unwrap();
        assert_eq!(visible.end, 20);
        assert!(visible.start > 0);
        assert!(ui.pointer_scroll(&scene, panel.x + 2.0, panel.y + 2.0, 0.0, -26.0));
        ui.sync(&mut scene);
        let (panel, visible) = ui.autocomplete_popup_window(&scene, id).unwrap();
        assert_eq!(visible.end, 19);
        let expected = format!("Type {}", visible.start);
        assert!(ui.pointer_pressed(&mut scene, panel.x + 4.0, panel.y + 4.0));
        assert_eq!(ui.autocomplete_value(id), Some(expected.as_str()));
        assert_eq!(ui.take_autocomplete_change(), Some((id, expected)));
        assert!(!ui.has_open_popup());
        ui.set_autocomplete_value(id, "");
        ui.key_input(UiKey::Down);
        ui.set_focus(None);
        ui.sync(&mut scene);
        assert!(!ui.has_open_popup());
    }
}
