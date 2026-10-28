use super::*;

/// An open modal's body position and active action, restored only to matching content.
#[derive(Clone, Debug, PartialEq)]
pub struct ModalState {
    title: String,
    body: String,
    primary: Option<String>,
    secondary: Option<String>,
    tertiary: Option<String>,
    scroll_y: f32,
    action_focus: usize,
}

impl Ui {
    /// Capture the active modal for a declarative rebuild.
    pub fn modal_state(&self, id: WidgetId) -> Option<ModalState> {
        if self.open_popup != Some(id) {
            return None;
        }
        let Kind::Modal {
            title,
            body,
            primary,
            secondary,
            tertiary,
            scroll_y,
            action_focus,
            open: true,
            ..
        } = &self.widgets.get(&id)?.kind
        else {
            return None;
        };
        Some(ModalState {
            title: title.clone(),
            body: body.clone(),
            primary: primary.clone(),
            secondary: secondary.clone(),
            tertiary: tertiary.clone(),
            scroll_y: *scroll_y,
            action_focus: *action_focus,
        })
    }

    /// Restore a matching active modal after layout; call sync afterwards.
    /// Changed questions keep their defaults.
    pub fn restore_modal_state(&mut self, id: WidgetId, state: &ModalState) -> bool {
        if self.open_popup != Some(id) {
            return false;
        }
        let Some(Widget {
            kind:
                Kind::Modal {
                    title,
                    body,
                    primary,
                    secondary,
                    tertiary,
                    action_focus,
                    open: true,
                    ..
                },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        if title != &state.title
            || body != &state.body
            || primary != &state.primary
            || secondary != &state.secondary
            || tertiary != &state.tertiary
        {
            return false;
        }
        *action_focus = state.action_focus;
        self.scroll_modal(id, state.scroll_y);
        true
    }

    pub(super) fn scroll_modal(&mut self, id: WidgetId, next: f32) -> bool {
        if !next.is_finite() {
            return false;
        }
        let Some(Widget {
            kind:
                Kind::Modal {
                    scroll_y,
                    body_view,
                    body_height,
                    open: true,
                    ..
                },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        let next = next.clamp(0.0, (*body_height - body_view.height).max(0.0));
        if (*scroll_y - next).abs() < 0.1 {
            return false;
        }
        *scroll_y = next;
        true
    }

    pub(super) fn key_modal_scroll(&mut self, id: WidgetId, key: UiKey) -> bool {
        let Some(Widget {
            kind:
                Kind::Modal {
                    scroll_y,
                    body_view,
                    body_height,
                    style,
                    open: true,
                    ..
                },
            ..
        }) = self.widgets.get(&id)
        else {
            return false;
        };
        let line = self.font.metrics(style.body_size).line_height;
        let next = match key {
            UiKey::Up => *scroll_y - line,
            UiKey::Down => *scroll_y + line,
            UiKey::PageUp => *scroll_y - body_view.height * 0.9,
            UiKey::PageDown => *scroll_y + body_view.height * 0.9,
            UiKey::Home => 0.0,
            UiKey::End => (*body_height - body_view.height).max(0.0),
            _ => return false,
        };
        self.scroll_modal(id, next);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modal_scrolls_by_wheel_keys_and_drag_keeps_actions_and_restores_matching_sessions() {
        for theme in [Theme::dark(), Theme::light()] {
            for (width, height) in [(640.0, 320.0), (400.0, 240.0)] {
                let mut ui = Ui::new();
                ui.set_theme(theme);
                let mut scene = Scene::new();
                ui.set_viewport(width, height);
                let background = ui.column(ColumnStyle::default(), |column| {
                    for _ in 0..30 {
                        column.button("Behind modal");
                    }
                });
                let body =
                    "Database path /archive/中文 'quoted' name and pending edits.\n".repeat(100);
                let modal = ui.modal(
                    "Save?",
                    body.clone(),
                    "Cancel",
                    Some("Save".into()),
                    ModalStyle::from_theme(&theme),
                );
                ui.set_modal_tertiary(modal, Some("Other".into()));
                ui.set_modal_open(modal, true);
                ui.sync(&mut scene);
                assert_eq!(ui.accessible_description(modal), Some(body.as_str()));
                assert_eq!(ui.accessible_description(background), None);
                let initial = ui.modal_state(modal).unwrap();
                assert_eq!(initial.scroll_y, 0.0);
                let aux = ui.widgets[&modal].aux.clone();
                let clip = scene.get(aux[5]).unwrap().clip.unwrap();
                assert!(ui.pointer_scroll(&scene, 2.0, 2.0, 0.0, 100.0));
                assert_eq!(ui.modal_state(modal).unwrap().scroll_y, 0.0);
                assert!(ui.pointer_scroll(&scene, clip.x + 8.0, clip.y + 8.0, 0.0, 80.0));
                assert!(ui.modal_state(modal).unwrap().scroll_y > 0.0);
                assert!(!ui.pointer_scroll(&scene, clip.x, clip.y, 0.0, f32::NAN));
                ui.key_input(UiKey::Home);
                assert_eq!(ui.modal_state(modal).unwrap().scroll_y, 0.0);
                ui.key_input(UiKey::Down);
                assert!(ui.modal_state(modal).unwrap().scroll_y > 0.0);
                ui.key_input(UiKey::PageDown);
                let page = ui.modal_state(modal).unwrap().scroll_y;
                ui.key_input(UiKey::PageUp);
                assert!(ui.modal_state(modal).unwrap().scroll_y < page);
                ui.key_input(UiKey::Tab);
                ui.key_input(UiKey::End);
                ui.sync(&mut scene);
                let end = ui.modal_state(modal).unwrap();
                assert!(end.scroll_y > page);
                assert_eq!(end.action_focus, 1);
                assert!(ui.take_modal_action().is_none());
                let footer = scene.get(aux[6]).unwrap().bounds;
                assert!(scene.get(aux[5]).unwrap().clip.unwrap().y + clip.height <= footer.y);
                let thumb = scene.get(aux[13]).unwrap().bounds;
                assert!(ui.pointer_pressed(
                    &mut scene,
                    thumb.x + thumb.width * 0.5,
                    thumb.y + thumb.height * 0.5
                ));
                assert!(ui.is_dragging());
                assert!(ui.pointer_moved(thumb.x, clip.y - 10000.0));
                ui.pointer_released();
                ui.sync(&mut scene);
                assert_eq!(ui.modal_state(modal).unwrap().scroll_y, 0.0);
                assert!(!ui.scroll_modal(modal, f32::INFINITY));
                assert!(ui.restore_modal_state(modal, &end));
                ui.sync(&mut scene);
                assert_eq!(ui.modal_state(modal).unwrap(), end);
                ui.set_viewport(1000.0, 3000.0);
                ui.sync(&mut scene);
                assert_eq!(ui.modal_state(modal).unwrap().scroll_y, 0.0);
                assert!(
                    !scene.get(aux[12]).unwrap().visible && !scene.get(aux[13]).unwrap().visible
                );
                ui.set_viewport(width, height);
                ui.sync(&mut scene);
                assert!(ui.restore_modal_state(modal, &end));
                ui.sync(&mut scene);
                assert_eq!(ui.modal_state(modal).unwrap(), end);
                if let Kind::Modal { body, .. } = &mut ui.widgets.get_mut(&modal).unwrap().kind {
                    *body = "A different question".into();
                }
                ui.set_modal_open(modal, false);
                ui.set_modal_open(modal, true);
                ui.sync(&mut scene);
                assert!(!ui.restore_modal_state(modal, &end));
                assert_eq!(ui.modal_state(modal).unwrap().action_focus, 0);
                ui.key_input(UiKey::Escape);
                ui.sync(&mut scene);
                assert_eq!(ui.take_modal_action(), Some((modal, ModalAction::Dismiss)));
                assert!(ui.modal_state(modal).is_none());
                assert!(!ui.restore_modal_state(modal, &initial));
            }
        }
    }
}
