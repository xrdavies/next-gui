use crate::{icon, AlphaMask, Color, IconName, Kind, MenuItem, Rect, Scene, Ui, UiKey, WidgetId};

fn separator(item: &MenuItem) -> bool {
    !item.enabled && item.label.is_empty() && item.children.is_empty()
}

fn row_height(item: &MenuItem, height: f32) -> f32 {
    if separator(item) {
        8.0
    } else {
        height
    }
}

impl Ui {
    pub(super) fn menu_height(items: &[MenuItem], height: f32) -> f32 {
        items.iter().map(|item| row_height(item, height)).sum()
    }

    fn menu_items_at(&self, id: WidgetId, path: &[usize]) -> Option<&[MenuItem]> {
        let mut items = match &self.widgets.get(&id)?.kind {
            Kind::Menu { items, .. } | Kind::ContextMenu { items, .. } => items.as_slice(),
            _ => return None,
        };
        for &row in path {
            items = &items.get(row)?.children;
        }
        Some(items)
    }

    fn menu_row_at(&self, id: WidgetId, x: f32, y: f32) -> Option<(Vec<usize>, usize)> {
        let height = match &self.widgets.get(&id)?.kind {
            Kind::Menu { style, .. } => style.item_height,
            Kind::ContextMenu { style, .. } => style.item_height,
            _ => return None,
        };
        for (panel, path) in self.menu_panels.iter().rev() {
            if !panel.contains(x, y) {
                continue;
            }
            let mut top = panel.y;
            for (row, item) in self.menu_items_at(id, path)?.iter().enumerate() {
                top += row_height(item, height);
                if y < top {
                    return Some((path.clone(), row));
                }
            }
        }
        None
    }

    pub(super) fn hover_menu_popup(&mut self, id: WidgetId, x: f32, y: f32) {
        if let Some((mut path, row)) = self.menu_row_at(id, x, y) {
            let item = &self.menu_items_at(id, &path).unwrap()[row];
            if item.enabled && !item.children.is_empty() {
                path.push(row);
                if self.popup_menu_path != path {
                    self.popup_menu_path = path;
                    self.popup_menu_row = None;
                }
            } else {
                let enabled = item.enabled;
                self.popup_menu_path = path;
                self.popup_menu_row = enabled.then_some(row);
            }
        } else if self.popup_menu_path.is_empty() {
            self.popup_menu_row = None;
        }
    }

    fn activate_menu_row(&mut self, id: WidgetId, row: usize, keyboard: bool) {
        let Some(item) = self
            .menu_items_at(id, &self.popup_menu_path)
            .and_then(|items| items.get(row))
        else {
            return;
        };
        if !item.enabled {
            return;
        }
        if !item.children.is_empty() {
            let first = item.children.iter().position(|child| child.enabled);
            self.popup_menu_path.push(row);
            self.popup_menu_row = if keyboard { first } else { None };
        } else {
            let action = item.id;
            if self.text_completion_popup() == Some(id) {
                self.commit_text_completion(action as usize);
            } else {
                self.pending_menu_action = Some((id, action));
            }
            self.close_popup_internal();
        }
    }

    pub(super) fn pointer_menu_popup(&mut self, id: WidgetId, x: f32, y: f32) -> bool {
        if let Some((path, row)) = self.menu_row_at(id, x, y) {
            self.popup_menu_path = path;
            self.popup_menu_row = Some(row);
            self.activate_menu_row(id, row, false);
        }
        true
    }

    pub(super) fn key_menu_popup(&mut self, id: WidgetId, key: UiKey) -> bool {
        let Some(items) = self.menu_items_at(id, &self.popup_menu_path) else {
            return false;
        };
        let enabled: Vec<_> = items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.enabled)
            .map(|(row, _)| row)
            .collect();
        let current = self
            .popup_menu_row
            .and_then(|row| enabled.iter().position(|i| *i == row));
        match key {
            UiKey::Up | UiKey::Down | UiKey::Home | UiKey::End if !enabled.is_empty() => {
                let next = match key {
                    UiKey::Home => 0,
                    UiKey::End => enabled.len() - 1,
                    UiKey::Up => current.map_or(enabled.len() - 1, |i| {
                        (i + enabled.len() - 1) % enabled.len()
                    }),
                    _ => current.map_or(0, |i| (i + 1) % enabled.len()),
                };
                self.popup_menu_row = Some(enabled[next]);
            }
            UiKey::Left if !self.popup_menu_path.is_empty() => {
                self.popup_menu_row = self.popup_menu_path.pop();
            }
            UiKey::Right => {
                if let Some(row) = self.popup_menu_row.or_else(|| enabled.first().copied()) {
                    if !items[row].children.is_empty() {
                        self.activate_menu_row(id, row, true);
                    }
                }
            }
            UiKey::Enter | UiKey::Space => {
                if let Some(row) = self.popup_menu_row.or_else(|| enabled.first().copied()) {
                    self.activate_menu_row(id, row, true);
                }
            }
            UiKey::Tab | UiKey::BackTab => {
                self.close_popup_internal();
                if key == UiKey::Tab {
                    self.focus_next();
                } else {
                    self.focus_prev();
                }
            }
            _ => {}
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_item_popup(
        &mut self,
        scene: &mut Scene,
        id: WidgetId,
        panel: Rect,
        items: &[MenuItem],
        item_height: f32,
        size: f32,
        popup_bg: Color,
        fg: Color,
        disabled_fg: Color,
        hover_bg: Color,
        hover_fg: Color,
    ) {
        let mut panels = vec![(panel, items, Vec::new())];
        let mut selected = items;
        let mut parent = panel;
        let mut path = Vec::new();
        for &row in &self.popup_menu_path {
            let Some(item) = selected
                .get(row)
                .filter(|item| item.enabled && !item.children.is_empty())
            else {
                break;
            };
            let children = item.children.as_slice();
            let checks = if children.iter().any(|item| item.checked.is_some()) {
                20.0
            } else {
                0.0
            };
            let arrows = if children.iter().any(|item| !item.children.is_empty()) {
                20.0
            } else {
                0.0
            };
            let width = children
                .iter()
                .map(|item| self.font.measure_line(&item.label, size).0)
                .fold(0.0f32, f32::max)
                + 28.0
                + checks
                + arrows;
            let width = width.min(self.viewport.0);
            let height = Self::menu_height(children, item_height);
            let x = if parent.x + parent.width + width <= self.viewport.0 {
                parent.x + parent.width
            } else {
                (parent.x - width).max(0.0)
            };
            let y = (parent.y + Self::menu_height(&selected[..row], item_height))
                .min((self.viewport.1 - height).max(0.0));
            parent = Rect::new(x, y, width, height);
            path.push(row);
            panels.push((parent, children, path.clone()));
            selected = children;
        }
        let needed = panels
            .iter()
            .map(|(_, items, _)| {
                1 + items.len()
                    * (2 + usize::from(items.iter().any(|item| item.checked.is_some()))
                        + usize::from(items.iter().any(|item| !item.children.is_empty())))
            })
            .sum();
        self.ensure_overlay_aux(scene, id, needed);
        let aux = self.widgets[&id].aux.clone();
        self.menu_panels.clear();
        let mut offset = 0;
        for (panel, items, path) in panels {
            self.menu_panels.push((panel, path.clone()));
            let check_space = if items.iter().any(|item| item.checked.is_some()) {
                20.0
            } else {
                0.0
            };
            let arrows = items.iter().any(|item| !item.children.is_empty());
            let bg = aux[offset];
            scene.set_bounds(bg, panel);
            scene.set_clip(bg, None);
            scene.set_fill(bg, popup_bg);
            scene.set_mask(bg, None);
            scene.set_visible(bg, true);
            self.apply_corner_radius(scene, bg, true);
            self.node_to_widget.insert(bg, id);
            let mut y = panel.y;
            for (row_index, item) in items.iter().enumerate() {
                let height = row_height(item, item_height);
                let row = Rect::new(panel.x, y, panel.width, height);
                y += height;
                let highlighted = item.enabled
                    && if path.len() < self.popup_menu_path.len() {
                        self.popup_menu_path.get(path.len()) == Some(&row_index)
                    } else {
                        self.popup_menu_row == Some(row_index)
                    };
                let color = if !item.enabled {
                    disabled_fg
                } else if highlighted {
                    hover_fg
                } else {
                    fg
                };
                let rbg = aux[offset + 1 + row_index * 2];
                scene.set_bounds(
                    rbg,
                    if separator(item) {
                        Rect::new(row.x + 8.0, row.y + 3.0, (row.width - 16.0).max(0.0), 1.0)
                    } else {
                        row
                    },
                );
                scene.set_clip(rbg, Some(panel));
                scene.set_fill(
                    rbg,
                    if separator(item) {
                        disabled_fg
                    } else if highlighted {
                        hover_bg
                    } else {
                        popup_bg
                    },
                );
                scene.set_mask(rbg, None);
                scene.set_corner_radius(rbg, 0.0);
                scene.set_visible(rbg, true);
                self.node_to_widget.insert(rbg, id);
                let lab = aux[offset + 2 + row_index * 2];
                scene.set_visible(lab, !separator(item));
                if !separator(item) {
                    let (_, mask) = self.font.rasterize_text(&item.label, size, None);
                    let x = row.x + 10.0 + check_space;
                    scene.set_bounds(
                        lab,
                        Rect::new(
                            x,
                            row.y + ((row.height - mask.height as f32) * 0.5).max(0.0),
                            mask.width as f32,
                            mask.height as f32,
                        ),
                    );
                    scene.set_clip(
                        lab,
                        Some(
                            Rect::new(
                                x,
                                row.y,
                                (row.width - 20.0 - check_space - if arrows { 20.0 } else { 0.0 })
                                    .max(0.0),
                                row.height,
                            )
                            .intersect(&panel),
                        ),
                    );
                    scene.set_fill(lab, color);
                    scene.set_mask(
                        lab,
                        Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                    );
                }
                let marks = offset + 1 + items.len() * 2;
                if check_space > 0.0 {
                    let mark = aux[marks + row_index];
                    scene.set_visible(mark, item.checked == Some(true));
                    if item.checked == Some(true) {
                        let mask = icon::rasterize_icon(
                            IconName::Check,
                            (size + 1.0).min(item_height - 8.0).max(1.0),
                        );
                        scene.set_bounds(
                            mark,
                            Rect::new(
                                row.x + 8.0,
                                row.y + (row.height - mask.height as f32) * 0.5,
                                mask.width as f32,
                                mask.height as f32,
                            ),
                        );
                        scene.set_clip(mark, Some(row.intersect(&panel)));
                        scene.set_fill(mark, color);
                        scene.set_mask(mark, Some(mask));
                        scene.set_corner_radius(mark, 0.0);
                        self.node_to_widget.insert(mark, id);
                    }
                }
                if arrows {
                    let arrow =
                        aux[marks + if check_space > 0.0 { items.len() } else { 0 } + row_index];
                    scene.set_visible(arrow, !item.children.is_empty());
                    if !item.children.is_empty() {
                        let mask = icon::rasterize_icon(IconName::Play, 8.0);
                        scene.set_bounds(
                            arrow,
                            Rect::new(
                                row.x + row.width - 16.0,
                                row.y + (row.height - 8.0) * 0.5,
                                8.0,
                                8.0,
                            ),
                        );
                        scene.set_clip(arrow, Some(row.intersect(&panel)));
                        scene.set_fill(arrow, color);
                        scene.set_mask(arrow, Some(mask));
                        scene.set_corner_radius(arrow, 0.0);
                        self.node_to_widget.insert(arrow, id);
                    }
                }
            }
            offset += 1 + items.len() * (2 + usize::from(check_space > 0.0) + usize::from(arrows));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ColumnStyle, ContextMenuStyle, MenuStyle, Theme};

    fn items() -> Vec<MenuItem> {
        vec![
            MenuItem::disabled(9, "Unavailable"),
            MenuItem::separator(),
            MenuItem::submenu(
                10,
                "Expressions",
                vec![
                    MenuItem::disabled(11, "Disabled"),
                    MenuItem::new(12, "Is NULL").checked(true),
                    MenuItem::separator(),
                    MenuItem::submenu(13, "More", vec![MenuItem::new(14, "Unicode 中文")]),
                ],
            ),
            MenuItem::new(15, "Clear"),
        ]
    }

    #[test]
    fn nested_menus_keep_owner_focus_skip_disabled_and_return_from_each_level() {
        for dropdown in [false, true] {
            let mut ui = Ui::new();
            let mut scene = Scene::new();
            ui.set_viewport(640.0, 480.0);
            let mut input = WidgetId(0);
            let mut menu = WidgetId(0);
            ui.column(ColumnStyle::default(), |c| {
                input = c.text_input("draft");
                if dropdown {
                    menu = c.menu("Edit", items(), MenuStyle::default());
                }
            });
            ui.sync(&mut scene);
            ui.set_focus(Some(input));
            if dropdown {
                ui.set_focus(Some(menu));
                ui.key_input(UiKey::Down);
            } else {
                menu = ui.context_menu_popup(ContextMenuStyle::default());
                ui.open_context_menu(menu, items(), 20.0, 20.0);
                ui.key_input(UiKey::Home);
            }
            let focus = ui.focused();
            assert_eq!(ui.popup_menu_row, Some(2));
            ui.key_input(UiKey::Right);
            assert_eq!(ui.popup_menu_path, vec![2]);
            assert_eq!(ui.popup_menu_row, Some(1));
            ui.key_input(UiKey::End);
            ui.key_input(UiKey::Enter);
            assert_eq!(ui.popup_menu_path, vec![2, 3]);
            ui.key_input(UiKey::Escape);
            assert_eq!(ui.popup_menu_path, vec![2]);
            assert_eq!(ui.popup_menu_row, Some(3));
            ui.key_input(UiKey::Left);
            assert!(ui.popup_menu_path.is_empty());
            assert_eq!(ui.popup_menu_row, Some(2));
            ui.key_input(UiKey::Right);
            ui.key_input(UiKey::Enter);
            assert_eq!(ui.take_menu_action(), Some((menu, 12)));
            assert_eq!(ui.focused(), focus);
            ui.sync(&mut scene);
            assert!(ui.menu_panels.is_empty());
            assert!(ui.widgets[&menu]
                .aux
                .iter()
                .all(|node| !scene.get(*node).unwrap().visible));
        }
    }

    #[test]
    fn nested_pointer_hover_clamps_flips_left_and_reuses_nodes_without_ghosts() {
        for theme in [Theme::dark(), Theme::light()] {
            let mut ui = Ui::new();
            ui.set_theme(theme);
            ui.set_viewport(420.0, 300.0);
            let mut scene = Scene::new();
            let menu = ui.context_menu_popup(ContextMenuStyle::from_theme(&theme));
            ui.open_context_menu(menu, items(), 415.0, 295.0);
            ui.sync(&mut scene);
            let (root, _) = ui.menu_panels[0].clone();
            let height = ContextMenuStyle::from_theme(&theme).item_height;
            let y = root.y + height + 8.0 + height * 0.5;
            assert!(ui.pointer_hover(&scene, root.x + root.width - 10.0, y));
            ui.sync(&mut scene);
            assert_eq!(ui.menu_panels.len(), 2);
            let (child, _) = ui.menu_panels[1].clone();
            assert!(child.x < root.x);
            assert!(
                child.x >= 0.0
                    && child.y >= 0.0
                    && child.x + child.width <= 420.0
                    && child.y + child.height <= 300.0
            );
            assert!(ui.popup_contains(&scene, menu, child.x + 5.0, child.y + height * 1.5));
            ui.pointer_hover(&scene, child.x + 5.0, child.y + height * 0.5);
            ui.sync(&mut scene);
            assert!(ui.pointer_pressed(&mut scene, child.x + 5.0, child.y + height * 0.5));
            assert_eq!(ui.take_menu_action(), None);
            assert!(ui.has_open_popup());
            assert!(ui.pointer_pressed(&mut scene, child.x + 5.0, child.y + height * 1.5));
            assert_eq!(ui.take_menu_action(), Some((menu, 12)));
            ui.sync(&mut scene);
            ui.open_context_menu(menu, vec![MenuItem::new(16, "Plain")], 0.0, 0.0);
            ui.sync(&mut scene);
            assert_eq!(ui.menu_panels.len(), 1);
            assert!(ui.widgets[&menu].aux[3..]
                .iter()
                .all(|node| !scene.get(*node).unwrap().visible));
            ui.pointer_pressed(&mut scene, 419.0, 299.0);
            assert!(!ui.has_open_popup());
        }
    }
}
