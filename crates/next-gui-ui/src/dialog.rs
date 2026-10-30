use super::*;

impl FlexBuilder<'_> {
    /// Build a closed modal root while composing a layout, without occupying its flow.
    /// Open and dismiss it through the same APIs as [`Ui::dialog`].
    pub fn dialog<B, F>(
        &mut self,
        title: impl Into<String>,
        style: ModalStyle,
        body: B,
        footer: F,
    ) -> WidgetId
    where
        B: FnOnce(&mut FlexBuilder<'_>),
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        self.ui.dialog(title, style, body, footer)
    }
}

struct DialogGeometry {
    panel: Rect,
    title: Rect,
    body: Rect,
    footer: Rect,
    close: Rect,
}

impl Ui {
    /// Build a closed modal dialog with a scrolling body and a fixed footer row.
    /// Children are standard widgets. Buttons keep their normal activation events;
    /// Escape, the close button and an enabled backdrop emit `take_dialog_dismiss`.
    pub fn dialog<B, F>(
        &mut self,
        title: impl Into<String>,
        style: ModalStyle,
        body: B,
        footer: F,
    ) -> WidgetId
    where
        B: FnOnce(&mut FlexBuilder<'_>),
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.alloc_container(Kind::Dialog {
            title: title.into(),
            children: Vec::new(),
            open: false,
            style,
        });
        self.roots.push(id);
        let mut scroll = ScrollAreaStyle::from_theme(&self.theme);
        scroll.width = 1.0;
        scroll.height = 1.0;
        scroll.padding = 0.0;
        scroll.background = None;
        let mut row = RowStyle::from_theme(&self.theme);
        row.width = 1.0;
        row.height = style.action_height;
        row.padding = 0.0;
        row.background = None;
        let mut builder = FlexBuilder {
            ui: self,
            parent: id,
        };
        builder.scroll_area(scroll, body);
        builder.row(row, footer);
        id
    }

    /// The scrolling body, for retaining its viewport across application rebuilds.
    pub fn dialog_body(&self, id: WidgetId) -> Option<WidgetId> {
        match &self.widgets.get(&id)?.kind {
            Kind::Dialog { children, .. } => children.first().copied(),
            _ => None,
        }
    }

    /// The most recently opened custom dialog. Transient child popups are separate.
    pub fn active_dialog(&self) -> Option<WidgetId> {
        self.dialogs.last().map(|entry| entry.0)
    }

    /// Open a dialog and focus its first control, or close it and restore prior focus.
    /// Closing a parent also closes dialogs opened above it. This emits no dismissal.
    pub fn set_dialog_open(&mut self, id: WidgetId, open: bool) -> bool {
        let Some(Widget {
            kind: Kind::Dialog { open: current, .. },
            ..
        }) = self.widgets.get(&id)
        else {
            return false;
        };
        if *current == open {
            return true;
        }
        if open {
            if self.legacy_modal_popup() {
                return false;
            }
            self.dismiss_text_area_hint();
            self.close_popup_internal();
            self.drag = None;
            for last in self.list_activation.values_mut() {
                *last = None;
            }
            self.dialogs.push((id, self.focus));
            if let Some(Widget {
                kind: Kind::Dialog { open, .. },
                ..
            }) = self.widgets.get_mut(&id)
            {
                *open = true;
            }
            self.focus = self.focusable_widgets().first().copied().or(Some(id));
        } else {
            let Some(position) = self.dialogs.iter().position(|entry| entry.0 == id) else {
                return false;
            };
            let closed = self.dialogs.split_off(position);
            if self.open_popup.is_some_and(|popup| {
                closed
                    .iter()
                    .any(|entry| self.widget_contains(entry.0, popup))
            }) {
                self.close_popup_internal();
            }
            for &(dialog, _) in &closed {
                if let Some(Widget {
                    kind: Kind::Dialog { open, .. },
                    ..
                }) = self.widgets.get_mut(&dialog)
                {
                    *open = false;
                }
            }
            if !self.legacy_modal_popup() {
                self.focus = closed[0].1.filter(|id| {
                    self.widgets.contains_key(id)
                        && self.button_enabled(*id) != Some(false)
                        && self.text_input_enabled(*id) != Some(false)
                        && self
                            .dialog_focus_scope()
                            .is_none_or(|dialog| self.widget_contains(dialog, *id))
                });
                if self.focus.is_none() && self.active_dialog().is_some() {
                    self.focus = self
                        .focusable_widgets()
                        .first()
                        .copied()
                        .or(self.active_dialog());
                }
            }
        }
        true
    }

    /// Drain an automatic close request. The dialog is already closed in the Ui.
    pub fn take_dialog_dismiss(&mut self) -> Option<WidgetId> {
        self.pending_dialog_dismiss.take()
    }

    fn legacy_modal_popup(&self) -> bool {
        self.open_popup.is_some_and(|id| {
            matches!(
                self.widgets.get(&id).map(|w| &w.kind),
                Some(Kind::Modal { .. } | Kind::Drawer { .. })
            )
        })
    }

    pub(super) fn dialog_focus_scope(&self) -> Option<WidgetId> {
        if self.legacy_modal_popup() {
            None
        } else {
            self.active_dialog()
        }
    }

    pub(super) fn widget_contains(&self, root: WidgetId, target: WidgetId) -> bool {
        let target = self.text_completion_owner(target)
            .or_else(|| self.context_menu_owner.filter(|(popup, _)| *popup == target).map(|(_, owner)| owner))
            .unwrap_or(target);
        root == target
            || self.widgets.get(&root).is_some_and(|widget| {
                Self::widget_children_ids(&widget.kind)
                    .iter()
                    .any(|&child| self.widget_contains(child, target))
            })
    }

    pub(super) fn enforce_dialog_scope(&mut self) {
        let Some(dialog) = self.dialog_focus_scope() else {
            return;
        };
        if self
            .open_popup
            .is_some_and(|popup| !self.widget_contains(dialog, popup))
        {
            self.close_popup_internal();
        }
        if self
            .focus
            .is_some_and(|id| !self.widget_contains(dialog, id))
        {
            self.focus = self.focusable_widgets().first().copied().or(Some(dialog));
        }
    }

    pub(super) fn escape_dialog(&mut self) -> bool {
        let Some(dialog) = self.dialog_focus_scope() else {
            return false;
        };
        if self.focus.is_some_and(|id|matches!(self.widgets.get(&id).map(|w|&w.kind),Some(Kind::TextInput {preedit,..}|Kind::Autocomplete {preedit,..}|Kind::TextArea {preedit,..}) if !preedit.is_empty())) {
            self.ime_clear_preedit();return true;
        }
        self.set_dialog_open(dialog, false);
        self.pending_dialog_dismiss = Some(dialog);
        true
    }

    fn dialog_geometry(&self, id: WidgetId) -> Option<DialogGeometry> {
        let Kind::Dialog {
            style, children, ..
        } = &self.widgets.get(&id)?.kind
        else {
            return None;
        };
        let (vw, vh) = self.viewport;
        let width = style.width.max(0.0).min((vw - 40.0).max(vw * 0.75));
        let height = style.height.max(0.0).min((vh - 40.0).max(vh * 0.75));
        let panel = Rect::new((vw - width) * 0.5, (vh - height) * 0.5, width, height);
        let padding = style
            .padding
            .max(0.0)
            .min(width * 0.125)
            .min(height * 0.125);
        let title_height = style
            .title_height
            .max(0.0)
            .min((height - 2.0 * padding).max(0.0) / 3.0);
        let footer = children.get(1).and_then(|id| self.widgets.get(id));
        let footer_children =
            footer.map_or(&[][..], |widget| Self::widget_children_ids(&widget.kind));
        let footer_height = if footer_children.is_empty() {
            0.0
        } else {
            style
                .action_height
                .max(0.0)
                .min((height - title_height - 3.0 * padding).max(0.0) * 0.5)
        };
        let inner_width = (width - 2.0 * padding).max(0.0);
        let gap = footer
            .and_then(|widget| match widget.kind {
                Kind::Row { style, .. } => Some(style.gap),
                _ => None,
            })
            .unwrap_or(0.0);
        let footer_width = (footer_children
            .iter()
            .map(|&id| self.measure_widget(id).width)
            .sum::<f32>()
            + gap * footer_children.len().saturating_sub(1) as f32)
            .min(inner_width);
        let footer_y = panel.y + height - padding - footer_height;
        let body = Rect::new(
            panel.x + padding,
            panel.y + title_height + padding,
            inner_width,
            (footer_y - padding - (panel.y + title_height + padding)).max(0.0),
        );
        let close_size = 24.0_f32.min(title_height);
        Some(DialogGeometry {
            panel,
            title: Rect::new(panel.x, panel.y, width, title_height),
            body,
            footer: Rect::new(
                panel.x + width - padding - footer_width,
                footer_y,
                footer_width,
                footer_height,
            ),
            close: Rect::new(
                panel.x + width - padding - close_size,
                panel.y + (title_height - close_size) * 0.5,
                close_size,
                close_size,
            ),
        })
    }

    pub(super) fn pointer_dialog(
        &mut self,
        scene: &mut Scene,
        id: WidgetId,
        x: f32,
        y: f32,
        secondary: bool,
    ) -> bool {
        if secondary {
            return true;
        }
        let Some(geometry) = self.dialog_geometry(id) else {
            return false;
        };
        let backdrop = matches!(self.widgets.get(&id).map(|w|&w.kind),Some(Kind::Dialog {style,..}) if style.close_on_backdrop);
        if geometry.close.contains(x, y) || (backdrop && !geometry.panel.contains(x, y)) {
            self.set_dialog_open(id, false);
            self.hide_widget_subtree(scene, id);
            self.pending_dialog_dismiss = Some(id);
        }
        true
    }

    pub(super) fn paint_dialogs(&mut self, scene: &mut Scene) {
        let dialogs: Vec<_> = self.dialogs.iter().map(|entry| entry.0).collect();
        for &dialog in &dialogs {
            self.paint_dialog(scene, dialog);
        }
        if !dialogs.is_empty() && self.reveal_scroll_focus(scene) {
            for dialog in dialogs {
                self.paint_dialog(scene, dialog);
            }
        }
    }

    fn paint_dialog(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(geometry) = self.dialog_geometry(id) else {
            return;
        };
        let Some(Widget {
            kind:
                Kind::Dialog {
                    title,
                    style,
                    children,
                    open: true,
                },
            node: Some(node),
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let (title, style, children, node) = (title.clone(), *style, children.clone(), *node);
        scene.set_bounds(node, Rect::new(0.0, 0.0, self.viewport.0, self.viewport.1));
        scene.set_fill(node, style.scrim);
        scene.set_mask(node, None);
        scene.set_visible(node, true);
        scene.raise(node);
        self.ensure_aux_nodes(scene, node, id, 5);
        let aux = self.widgets[&id].aux.clone();
        for (index, bounds, color) in [
            (0, geometry.panel, style.panel_bg),
            (1, geometry.title, style.title_bg),
            (3, geometry.close, style.secondary_bg),
        ] {
            scene.set_bounds(aux[index], bounds);
            scene.set_fill(aux[index], color);
            scene.set_mask(aux[index], None);
            scene.set_clip(aux[index], Some(geometry.panel));
            scene.set_visible(aux[index], true);
        }
        self.apply_corner_radius(scene, aux[0], true);
        self.apply_corner_radius(scene, aux[3], false);
        let title_clip = Rect::new(
            geometry.panel.x + style.padding.max(0.0),
            geometry.title.y,
            (geometry.close.x - geometry.panel.x - 2.0 * style.padding.max(0.0)).max(0.0),
            geometry.title.height,
        );
        for (index, text, rect, color) in [
            (
                2,
                title.replace(['\r', '\n', '\t'], " "),
                title_clip,
                style.title_fg,
            ),
            (4, "×".to_string(), geometry.close, style.title_fg),
        ] {
            let (_, mask) = self.font.rasterize_text(&text, style.title_size, None);
            let x = if index == 4 {
                rect.x + ((rect.width - mask.width as f32) * 0.5).max(0.0)
            } else {
                rect.x
            };
            scene.set_bounds(
                aux[index],
                Rect::new(
                    x,
                    rect.y + ((rect.height - mask.height as f32) * 0.5).max(0.0),
                    mask.width as f32,
                    mask.height as f32,
                ),
            );
            scene.set_fill(aux[index], color);
            scene.set_mask(
                aux[index],
                Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
            );
            scene.set_clip(aux[index], Some(rect));
            scene.set_visible(aux[index], true);
        }
        for (child, bounds) in children.into_iter().zip([geometry.body, geometry.footer]) {
            if let Some(widget) = self.widgets.get_mut(&child) {
                match &mut widget.kind {
                    Kind::ScrollArea { style, .. } => {
                        style.width = bounds.width;
                        style.height = bounds.height;
                    }
                    Kind::Row { style, .. } => {
                        style.width = bounds.width;
                        style.height = bounds.height;
                    }
                    _ => {}
                }
            }
            self.place_child(scene, child, bounds);
            self.layout_widget(scene, child);
            self.paint_widget(scene, child);
            if let Some(node) = self.widgets.get(&child).and_then(|widget| widget.node) {
                scene.set_clip(node, Some(bounds));
                scene.raise(node);
            }
        }
        if let Some(host) = self.overlay_host {
            scene.raise(host);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_text_completions_follow_the_owner_scope_and_resized_caret() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        ui.set_viewport(800.0, 600.0);
        let mut area = WidgetId(0);
        let dialog = ui.dialog(
            "SQL",
            ModalStyle::default(),
            |c| {
                area = c.text_area(
                    "sel",
                    "",
                    TextAreaStyle {
                        width: 240.0,
                        height: 100.0,
                        ..Default::default()
                    },
                );
            },
            |_| {},
        );
        ui.set_dialog_open(dialog, true);
        ui.sync(&mut scene);
        ui.select_text_area_range(area, 3..3);
        let items = || {
            vec![
                TextAreaCompletionItem {
                    label: "SELECT".into(),
                    text: "SELECT".into(),
                },
                TextAreaCompletionItem {
                    label: "select".into(),
                    text: "select".into(),
                },
            ]
        };
        assert!(ui.show_text_area_completions(area, 0..3, items()));
        for (width, height) in [(800.0, 600.0), (600.0, 480.0)] {
            ui.set_viewport(width, height);
            ui.sync(&mut scene);
            assert!(ui.text_area_completions_open(area));
            let popup = ui.text_completion_popup().unwrap();
            let panel = scene.get(ui.widgets[&popup].aux[0]).unwrap().bounds;
            let (x, y, _, height) = ui.ime_cursor_area(&scene).unwrap();
            assert!(
                (panel.x - x).abs() < 1.0 && (panel.y - y - height - 4.0).abs() < 1.0,
                "{panel:?} {x} {y} {height}"
            );
            assert_eq!(
                ui.hit_widget(&scene, panel.x + panel.width - 5.0, panel.y + 5.0),
                Some(popup)
            );
        }
        assert!(ui.key_input(UiKey::Escape));
        assert!(!ui.has_open_popup());
        assert_eq!(ui.active_dialog(), Some(dialog));
        assert!(ui.show_text_area_completions(area, 0..3, items()));
        ui.sync(&mut scene);
        assert!(ui.key_input(UiKey::Enter));
        assert_eq!(ui.text_area_value(area), Some("SELECT"));
        ui.select_text_area_range(area, 6..6);
        assert!(ui.show_text_area_completions(area, 0..6, items()));
        assert!(ui.set_dialog_open(dialog, false));
        assert!(!ui.has_open_popup());
    }

    #[test]
    fn list_activation_is_opt_in_and_checks_rows_timing_scope_and_keyboard() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        ui.set_viewport(800.0, 600.0);
        let mut list = WidgetId(0);
        ui.column(ColumnStyle::default(), |c| {
            list = c.list(["first", "second", "third"], ListStyle::default());
        });
        ui.sync(&mut scene);
        ui.set_focus(Some(list));
        ui.set_list_selected(list, Some(0));
        assert!(!ui.key_input(UiKey::Enter));
        assert!(ui.set_list_activatable(list, true));
        assert!(!ui.set_list_activatable(WidgetId(9999), true));
        assert!(ui.key_input(UiKey::Enter));
        assert_eq!(ui.take_list_activate(), Some((list, 0)));
        let bounds = bounds(&ui, &scene, list);
        let click = |ui: &mut Ui, scene: &mut Scene, row| {
            assert!(ui.pointer_pressed(
                scene,
                bounds.x + 150.0,
                bounds.y
                    + ListStyle::default().padding
                    + (row as f32 + 0.5) * ListStyle::default().item_height
            ));
            ui.pointer_released();
        };
        click(&mut ui, &mut scene, 0);
        assert_eq!(ui.take_list_activate(), None);
        click(&mut ui, &mut scene, 1);
        assert_eq!(ui.take_list_activate(), None);
        ui.list_activation.insert(
            list,
            Some((
                1,
                std::time::Instant::now() - std::time::Duration::from_secs(1),
            )),
        );
        click(&mut ui, &mut scene, 1);
        assert_eq!(ui.take_list_activate(), None);
        click(&mut ui, &mut scene, 1);
        assert_eq!(ui.take_list_activate(), Some((list, 1)));
        click(&mut ui, &mut scene, 1);
        ui.key_input(UiKey::Home);
        click(&mut ui, &mut scene, 1);
        assert_eq!(ui.take_list_activate(), None);
        assert!(ui.key_input(UiKey::Enter));
        ui.set_list_items(list, ["only"]);
        assert_eq!(ui.take_list_activate(), None);
        ui.set_list_selected(list, Some(0));
        ui.key_input(UiKey::Enter);
        let dialog = ui.dialog(
            "Scope",
            ModalStyle::default(),
            |c| {
                c.text_input("inside");
            },
            |_| {},
        );
        ui.set_dialog_open(dialog, true);
        assert_eq!(ui.take_list_activate(), None);
        ui.set_dialog_open(dialog, false);
        ui.set_focus(Some(list));
        assert!(ui.set_list_activatable(list, false));
        assert!(!ui.key_input(UiKey::Enter));
        ui.set_list_activatable(list, true);
        ui.set_list_items(list, Vec::<String>::new());
        assert!(!ui.key_input(UiKey::Enter));
        assert_eq!(ui.take_list_activate(), None);
    }

    fn bounds(ui: &Ui, scene: &Scene, id: WidgetId) -> Rect {
        scene.get(ui.widgets[&id].node.unwrap()).unwrap().bounds
    }

    #[test]
    fn dialogs_trap_focus_block_background_and_keep_child_popups_above_reused_content() {
        for theme in [Theme::dark(), Theme::light()] {
            let mut ui = Ui::new();
            ui.set_theme(theme);
            let mut scene = Scene::new();
            ui.set_viewport(800.0, 600.0);
            let mut background = WidgetId(0);
            let mut button = WidgetId(0);
            ui.column(ColumnStyle::default(), |c| {
                c.tooltip("Background tooltip", |c| {
                    background = c.text_input("background");
                    button = c.tool_button("Background action");
                });
            });
            ui.set_focus(Some(background));
            let mut input = WidgetId(0);
            let mut select = WidgetId(0);
            let mut apply = WidgetId(0);
            let mut cancel = WidgetId(0);
            let mut inside_tip = WidgetId(0);
            let mut style = ModalStyle::from_theme(&theme);
            style.width = 480.0;
            style.height = 300.0;
            style.close_on_backdrop = false;
            let dialog = ui.dialog(
                "Select columns",
                style,
                |c| {
                    inside_tip = c.tooltip("Dialog tooltip", |c| {
                        input =
                            c.text_input_styled("draft", "", TextInputStyle::from_theme(&theme));
                    });
                    select = c.select(
                        ["First", "Second"],
                        Some(0),
                        "",
                        SelectStyle::from_theme(&theme),
                    );
                },
                |r| {
                    apply = r.tool_button("Apply");
                    cancel = r.tool_button("Cancel");
                },
            );
            ui.sync(&mut scene);
            assert!(!ui.focusable_widgets().contains(&input));
            assert_eq!(ui.active_dialog(), None);
            assert!(ui.set_dialog_open(dialog, true));
            ui.sync(&mut scene);
            assert_eq!(ui.focused(), Some(input));
            let rect = bounds(&ui, &scene, input);
            ui.pointer_moved(rect.x + 5.0, rect.y + 5.0);
            ui.sync(&mut scene);
            assert_eq!(ui.tooltip_hover, Some(inside_tip));
            let rect = bounds(&ui, &scene, button);
            ui.pointer_moved(rect.x + 5.0, rect.y + 5.0);
            ui.sync(&mut scene);
            assert_eq!(ui.tooltip_hover, None);
            assert_eq!(ui.focusable_widgets(), vec![input, select, apply, cancel]);
            assert_eq!(
                ui.accessible_name(dialog).as_deref(),
                Some("Select columns")
            );
            for target in [select, apply, cancel, input] {
                assert!(ui.key_input(UiKey::Tab));
                assert_eq!(ui.focused(), Some(target));
            }
            ui.set_focus(Some(background));
            assert_eq!(ui.focused(), Some(input));
            ui.key_input(UiKey::Char('x'));
            assert_eq!(ui.text_input_value(background), Some(""));
            assert_eq!(ui.text_input_value(input), Some("draftx"));
            let old = bounds(&ui, &scene, button);
            assert_eq!(
                ui.hit_widget(&scene, old.x + 2.0, old.y + 2.0),
                Some(dialog)
            );
            assert!(ui.pointer_pressed(&mut scene, old.x + 2.0, old.y + 2.0));
            ui.pointer_released();
            assert_eq!(ui.take_button_activate(), None);
            assert_eq!(ui.active_dialog(), Some(dialog));
            assert!(ui.pointer_scroll(&scene, 1.0, 1.0, 0.0, 100.0));
            for _ in 0..2 {
                ui.set_focus(Some(select));
                assert!(ui.key_input(UiKey::Space));
                ui.sync(&mut scene);
                let popup = scene.get(ui.widgets[&select].aux[0]).unwrap().bounds;
                assert_eq!(
                    ui.hit_widget(&scene, popup.x + popup.width - 8.0, popup.y + 8.0),
                    Some(select)
                );
                assert!(ui.key_input(UiKey::Escape));
                assert_eq!(ui.focused(), Some(select));
                assert_eq!(ui.take_dialog_dismiss(), None);
                ui.set_dialog_open(dialog, false);
                ui.sync(&mut scene);
                assert_eq!(ui.focused(), Some(background));
                ui.set_dialog_open(dialog, true);
                ui.sync(&mut scene);
            }
            ui.set_focus(Some(input));
            ui.key_input(UiKey::SelectAll);
            ui.ime_preedit("候選", Some((3, 3)));
            assert!(ui.key_input(UiKey::Escape));
            assert_eq!(ui.active_dialog(), Some(dialog));
            assert_eq!(ui.text_input_value(input), Some("draftx"));
            assert_eq!(ui.take_dialog_dismiss(), None);
            assert!(ui.key_input(UiKey::Escape));
            assert_eq!(ui.take_dialog_dismiss(), Some(dialog));
            assert_eq!(ui.active_dialog(), None);
            assert_eq!(ui.focused(), Some(background));
            ui.sync(&mut scene);
            assert!(!ui.focusable_widgets().contains(&input));
        }
    }

    #[test]
    fn detached_context_menus_follow_their_dialog_owner_and_reject_background_actions() {
        for theme in [Theme::dark(), Theme::light()] {
            let mut ui = Ui::new();
            ui.set_theme(theme);
            ui.set_viewport(800.0, 600.0);
            let mut scene = Scene::new();
            let mut background = WidgetId(0);
            ui.column(ColumnStyle::default(), |body| { background = body.text_input("background"); });
            let mut input = WidgetId(0);
            let parent = ui.dialog("Formats", ModalStyle::default(), |body| {
                input = body.text_input("condition");
            }, |_| {});
            let child = ui.dialog("Reset?", ModalStyle::default(), |body| { body.text_input("child"); }, |_| {});
            let popup = ui.context_menu_popup(ContextMenuStyle::from_theme(&theme));
            ui.set_dialog_open(parent, true);
            ui.sync(&mut scene);
            let items = vec![MenuItem::submenu(1, "Expressions", vec![MenuItem::new(2, "Is NULL")])];
            for pointer in [false, true] {
                assert!(ui.open_context_menu_for(popup, input, items.clone(), 790.0, 590.0));
                ui.sync(&mut scene);
                assert_eq!(ui.popup_owner(), Some(popup));
                assert_eq!(ui.focused(), Some(input));
                assert!(!ui.open_context_menu_for(popup, background, vec![MenuItem::new(9, "blocked")], 2.0, 2.0));
                assert!(!ui.open_context_menu_for(popup, WidgetId(u64::MAX), items.clone(), 2.0, 2.0));
                assert!(!ui.open_context_menu_for(popup, popup, items.clone(), 2.0, 2.0));
                assert!(!ui.open_context_menu_for(popup, input, items.clone(), f32::NAN, 2.0));
                assert_eq!(ui.popup_owner(), Some(popup));
                ui.key_input(UiKey::Down);
                ui.key_input(UiKey::Right);
                ui.sync(&mut scene);
                assert_eq!(ui.menu_panels.len(), 2);
                if pointer {
                    let panel = ui.menu_panels[1].0;
                    assert_eq!(ui.hit_widget(&scene, panel.x + 8.0, panel.y + 8.0), Some(popup));
                    assert!(ui.pointer_pressed(&mut scene, panel.x + 8.0, panel.y + 8.0));
                    ui.pointer_released();
                } else { ui.key_input(UiKey::Enter); }
                assert_eq!(ui.take_menu_action(), Some((popup, 2)));
                assert_eq!(ui.active_dialog(), Some(parent));
                assert_eq!(ui.focused(), Some(input));
            }
            assert!(ui.open_context_menu_for(popup, input, items.clone(), 20.0, 20.0));
            ui.sync(&mut scene);
            assert!(ui.key_input(UiKey::Escape));
            assert!(!ui.has_open_popup());
            assert_eq!(ui.take_dialog_dismiss(), None);
            assert_eq!(ui.active_dialog(), Some(parent));
            assert!(ui.open_context_menu_for(popup, input, items.clone(), 20.0, 20.0));
            ui.set_dialog_open(child, true);
            ui.sync(&mut scene);
            assert!(!ui.has_open_popup());
            assert!(!ui.open_context_menu_for(popup, input, items.clone(), 20.0, 20.0));
            ui.set_dialog_open(child, false);
            assert!(ui.open_context_menu_for(popup, input, items, 20.0, 20.0));
            ui.set_dialog_open(parent, false);
            ui.sync(&mut scene);
            assert!(!ui.has_open_popup());
            assert_eq!(ui.context_menu_owner, None);
        }
    }

    #[test]
    fn nested_dialogs_restore_focus_and_respect_close_backdrop_and_legacy_decisions() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        ui.set_viewport(800.0, 600.0);
        let mut background = WidgetId(0);
        ui.column(ColumnStyle::default(), |c| {
            background = c.tool_button("Open");
        });
        ui.set_focus(Some(background));
        let mut parent_input = WidgetId(0);
        let parent = ui.dialog(
            "Parent",
            ModalStyle::default(),
            |c| {
                parent_input = c.text_input("Parent input");
            },
            |_| {},
        );
        let mut child_input = WidgetId(0);
        let mut child_button = WidgetId(0);
        let style = ModalStyle {
            close_on_backdrop: true,
            ..Default::default()
        };
        let child = ui.dialog(
            "Child",
            style,
            |c| {
                child_input = c.text_input("Child input");
            },
            |r| {
                child_button = r.tool_button("Apply");
            },
        );
        let legacy = ui.modal(
            "Pending decision",
            "Keep this decision",
            "Save",
            Some("Cancel".into()),
            ModalStyle::default(),
        );
        ui.set_modal_open(legacy, true);
        assert!(!ui.set_dialog_open(parent, true));
        assert_eq!(ui.modal_open(legacy), Some(true));
        assert_eq!(ui.focused(), Some(legacy));
        ui.set_modal_open(legacy, false);
        ui.set_focus(Some(background));
        ui.set_dialog_open(parent, true);
        ui.set_focus(Some(parent_input));
        ui.set_dialog_open(child, true);
        ui.sync(&mut scene);
        assert_eq!(ui.focusable_widgets(), vec![child_input, child_button]);
        assert_eq!(ui.focused(), Some(child_input));
        assert!(ui.pointer_press(&mut scene, 2.0, 2.0, true));
        assert_eq!(ui.active_dialog(), Some(child));
        assert!(ui.pointer_pressed(&mut scene, 2.0, 2.0));
        ui.pointer_released();
        assert_eq!(ui.take_dialog_dismiss(), Some(child));
        assert_eq!(ui.active_dialog(), Some(parent));
        assert_eq!(ui.focused(), Some(parent_input));
        ui.sync(&mut scene);
        ui.set_dialog_open(child, true);
        ui.sync(&mut scene);
        let close = ui.dialog_geometry(child).unwrap().close;
        assert!(ui.pointer_pressed(
            &mut scene,
            close.x + close.width * 0.5,
            close.y + close.height * 0.5
        ));
        ui.pointer_released();
        assert_eq!(ui.take_dialog_dismiss(), Some(child));
        assert_eq!(ui.focused(), Some(parent_input));
        ui.set_dialog_open(child, true);
        assert!(ui.set_dialog_open(parent, false));
        assert_eq!(ui.active_dialog(), None);
        assert_eq!(ui.focused(), Some(background));
        ui.sync(&mut scene);
        assert_eq!(ui.focusable_widgets(), vec![background]);
        assert!(!ui.set_dialog_open(WidgetId(u64::MAX), true));
        assert_eq!(ui.dialog_body(WidgetId(u64::MAX)), None);
    }

    #[test]
    fn dialog_body_scroll_reveals_focus_keeps_footer_hits_and_cell_escape_stays_local() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        ui.set_viewport(480.0, 320.0);
        let mut fields = Vec::new();
        let mut apply = WidgetId(0);
        let mut cancel = WidgetId(0);
        let style = ModalStyle {
            width: 400.0,
            height: 260.0,
            ..Default::default()
        };
        let dialog = ui.dialog(
            "Long form",
            style,
            |c| {
                for index in 0..20 {
                    fields.push(c.text_input_styled(
                        index.to_string(),
                        "",
                        TextInputStyle {
                            width: 240.0,
                            ..Default::default()
                        },
                    ));
                }
            },
            |r| {
                apply = r.tool_button("Apply");
                cancel = r.tool_button("Cancel");
            },
        );
        ui.set_dialog_open(dialog, true);
        ui.sync(&mut scene);
        let footer = bounds(&ui, &scene, apply);
        ui.set_focus(fields.last().copied());
        ui.sync(&mut scene);
        let body = ui.dialog_body(dialog).unwrap();
        let body_bounds = bounds(&ui, &scene, body);
        let last = bounds(&ui, &scene, *fields.last().unwrap());
        assert!(ui.scroll_offset(body).unwrap().1 > 0.0);
        assert!(!last.intersect(&body_bounds).is_empty());
        assert_eq!(bounds(&ui, &scene, apply), footer);
        assert_eq!(
            ui.hit_widget(&scene, footer.x + 5.0, footer.y + 5.0),
            Some(apply)
        );
        assert!(!ui.pointer_pressed(&mut scene, footer.x + 5.0, footer.y + 5.0));
        ui.pointer_released();
        assert_eq!(ui.focused(), Some(apply));
        assert!(ui.key_input(UiKey::Space));
        assert_eq!(ui.take_button_activate(), Some(apply));
        assert_eq!(ui.active_dialog(), Some(dialog));
        ui.set_viewport(320.0, 240.0);
        ui.sync(&mut scene);
        for id in [apply, cancel] {
            let rect = bounds(&ui, &scene, id);
            assert!(
                rect.y + rect.height <= 240.0 && rect.x + rect.width <= 320.0,
                "{rect:?}"
            );
        }
        ui.set_dialog_open(dialog, false);
        let mut table = WidgetId(0);
        let mut editor = WidgetId(0);
        let cell_dialog = ui.dialog(
            "Cell",
            style,
            |c| {
                table = c.table(
                    vec![TableColumn::new("Name", 200.0)],
                    vec![vec!["draft".into()]],
                    TableStyle {
                        width: 250.0,
                        height: 100.0,
                        ..Default::default()
                    },
                );
                editor = c
                    .table_cell_editor(table, 0, 0, |cell| {
                        cell.text_input_styled("draft", "", TextInputStyle::default())
                    })
                    .unwrap();
            },
            |_| {},
        );
        ui.set_dialog_open(cell_dialog, true);
        ui.set_focus(Some(editor));
        ui.sync(&mut scene);
        ui.ime_preedit("候選", Some((3, 3)));
        ui.key_input(UiKey::Escape);
        assert_eq!(ui.active_dialog(), Some(cell_dialog));
        assert_eq!(ui.take_table_cell_edit(), None);
        ui.key_input(UiKey::Escape);
        assert_eq!(
            ui.take_table_cell_edit(),
            Some((table, TableCellEditEvent::End { cancel: true }))
        );
        assert_eq!(ui.active_dialog(), Some(cell_dialog));
        ui.key_input(UiKey::Escape);
        assert_eq!(ui.take_dialog_dismiss(), Some(cell_dialog));
    }
}
