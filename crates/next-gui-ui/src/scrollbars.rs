use super::*;

const WIDTH: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
struct Scrollbar {
    axis: Axis,
    track: Rect,
    thumb: Rect,
    max: f32,
    viewport: f32,
}

impl Ui {
    pub(super) fn reveal_scroll_focus(&mut self, scene: &Scene) -> bool {
        if self.revealed_scroll_focus == self.focus {
            return false;
        }
        self.revealed_scroll_focus = self.focus;
        let Some(node) = self
            .focus
            .and_then(|id| self.widgets.get(&id))
            .and_then(|w| w.node)
            .and_then(|n| scene.get(n))
        else {
            return false;
        };
        let mut target = node.bounds;
        let mut parent = node.parent;
        let mut changed = false;
        while let Some(node) = parent.and_then(|n| scene.get(n)) {
            if let Some(id) = self
                .widget_from_node(node.id)
                .filter(|id| !self.table_filters.contains_key(id))
            {
                if let Some(Kind::ScrollArea {
                    style,
                    scroll_x,
                    scroll_y,
                    ..
                }) = self.widgets.get(&id).map(|w| &w.kind)
                {
                    let (sx, sy) = (*scroll_x, *scroll_y);
                    let viewport = node.bounds;
                    let delta = |start: f32, length: f32, view_start: f32, view_length: f32| {
                        if start < view_start || length > view_length {
                            start - view_start
                        } else {
                            (start + length - view_start - view_length).max(0.0)
                        }
                    };
                    let dx = delta(
                        target.x,
                        target.width,
                        viewport.x + style.padding,
                        (viewport.width - 2.0 * style.padding).max(1.0),
                    );
                    let dy = delta(
                        target.y,
                        target.height,
                        viewport.y + style.padding,
                        (viewport.height - 2.0 * style.padding).max(1.0),
                    );
                    if self.set_scroll_offset(id, sx + dx, sy + dy) {
                        let (nx, ny) = self.scroll_offset(id).unwrap();
                        target.x += sx - nx;
                        target.y += sy - ny;
                        changed = true;
                    }
                }
            }
            parent = node.parent;
        }
        changed
    }

    fn scrollbar_geometry(&self, scene: &Scene, id: WidgetId) -> Vec<Scrollbar> {
        let Some(widget) = self.widgets.get(&id) else {
            return Vec::new();
        };
        let Some(bounds) = widget.node.and_then(|n| scene.get(n)).map(|n| n.bounds) else {
            return Vec::new();
        };
        let Some((sx, sy)) = self.any_scroll_offset(id) else {
            return Vec::new();
        };
        let (mut view, max_x, max_y) = match &widget.kind {
            Kind::ScrollArea {
                content_w,
                content_h,
                ..
            } if !self.table_filters.contains_key(&id) => (
                bounds,
                (content_w - bounds.width).max(0.0),
                (content_h - bounds.height).max(0.0),
            ),
            Kind::Table {
                columns,
                frozen_cols,
                style,
                ..
            } => {
                let frozen: f32 = columns.iter().take(*frozen_cols).map(|c| c.width).sum();
                let width = (bounds.width - 2.0 * style.padding - frozen).max(1.0);
                let total: f32 = columns.iter().skip(*frozen_cols).map(|c| c.width).sum();
                (
                    Rect::new(
                        bounds.x + style.padding + frozen,
                        bounds.y + style.header_height,
                        width,
                        (bounds.height - style.header_height).max(0.0),
                    ),
                    (total - width).max(0.0),
                    self.data_max_scroll_y(id),
                )
            }
            Kind::TextArea { value, style, .. } => {
                let view = Rect::new(
                    bounds.x,
                    bounds.y + style.padding_y,
                    bounds.width,
                    (bounds.height - 2.0 * style.padding_y).max(1.0),
                );
                let layout = self.font.layout(
                    value,
                    style.size,
                    Some((bounds.width - 2.0 * style.padding_x).max(1.0)),
                );
                (view, 0.0, (layout.height - view.height).max(0.0))
            }
            Kind::List { .. }
            | Kind::Tree { .. }
            | Kind::AlarmList { .. }
            | Kind::PropertyGrid { .. }
            | Kind::TreeTable { .. } => (bounds, 0.0, self.data_max_scroll_y(id)),
            _ => return Vec::new(),
        };
        // The vertical bar stays at the viewport edge, even when every column is frozen.
        let vertical_right = bounds.x + bounds.width - 1.0;
        view.width = view.width.min((vertical_right - view.x).max(0.0));
        let mut bars = Vec::new();
        for (axis, max, scroll, viewport) in [
            (Axis::Horizontal, max_x, sx, view.width),
            (Axis::Vertical, max_y, sy, view.height),
        ] {
            if max <= 0.0 || !max.is_finite() {
                continue;
            }
            let track = if axis == Axis::Horizontal {
                Rect::new(
                    view.x,
                    bounds.y + bounds.height - WIDTH - 1.0,
                    (viewport - if max_y > 0.0 { WIDTH } else { 0.0 }).max(0.0),
                    WIDTH,
                )
            } else {
                Rect::new(
                    vertical_right - WIDTH,
                    view.y,
                    WIDTH,
                    (viewport - if max_x > 0.0 { WIDTH } else { 0.0 }).max(0.0),
                )
            };
            let length = if axis == Axis::Horizontal {
                track.width
            } else {
                track.height
            };
            if length < 4.0 {
                continue;
            }
            let size = (length * viewport / (viewport + max)).max(18.0).min(length);
            let offset = (length - size) * scroll.clamp(0.0, max) / max;
            let thumb = if axis == Axis::Horizontal {
                Rect::new(track.x + offset, track.y + 2.0, size, WIDTH - 4.0)
            } else {
                Rect::new(track.x + 2.0, track.y + offset, WIDTH - 4.0, size)
            };
            bars.push(Scrollbar {
                axis,
                track,
                thumb,
                max,
                viewport,
            });
        }
        bars
    }

    pub(super) fn paint_scrollbars(&mut self, scene: &mut Scene, id: WidgetId) {
        let bars = self.scrollbar_geometry(scene, id);
        if bars.is_empty() && !self.scrollbars.contains_key(&id) {
            return;
        }
        let Some(host) = self.widgets.get(&id).and_then(|w| w.node) else {
            return;
        };
        let nodes = *self.scrollbars.entry(id).or_insert_with(|| {
            std::array::from_fn(|_| {
                let node = scene
                    .insert(host, Rect::empty(), self.theme.colors.track)
                    .expect("scrollbar node");
                self.node_to_widget.insert(node, id);
                node
            })
        });
        for (index, node) in nodes.iter().enumerate() {
            let axis = if index < 2 {
                Axis::Horizontal
            } else {
                Axis::Vertical
            };
            scene.set_visible(*node, bars.iter().any(|bar| bar.axis == axis));
        }
        if !scene
            .get(host)
            .is_some_and(|host| host.children.ends_with(&nodes))
        {
            for node in nodes {
                scene.raise(node);
            }
        }
        for bar in bars {
            let index = if bar.axis == Axis::Horizontal { 0 } else { 2 };
            for (node, bounds, fill) in [
                (nodes[index], bar.track, self.theme.colors.track),
                (
                    nodes[index + 1],
                    bar.thumb,
                    self.theme.colors.text_secondary,
                ),
            ] {
                scene.set_bounds(node, bounds);
                scene.set_fill(node, fill);
                scene.set_corner_radius(node, 2.0);
                scene.set_visible(node, true);
            }
        }
    }

    pub(super) fn pointer_scrollbar(
        &mut self,
        scene: &Scene,
        id: WidgetId,
        x: f32,
        y: f32,
    ) -> bool {
        for bar in self.scrollbar_geometry(scene, id) {
            if !bar.track.contains(x, y) {
                continue;
            }
            let (sx, sy) = self.any_scroll_offset(id).unwrap_or_default();
            let horizontal = bar.axis == Axis::Horizontal;
            let pos = if horizontal { x } else { y };
            let start = if horizontal { bar.thumb.x } else { bar.thumb.y };
            let end = start
                + if horizontal {
                    bar.thumb.width
                } else {
                    bar.thumb.height
                };
            let scroll = if horizontal { sx } else { sy };
            if pos >= start && pos < end {
                let travel = if horizontal {
                    bar.track.width - bar.thumb.width
                } else {
                    bar.track.height - bar.thumb.height
                };
                self.drag = Some(DragState::Scrollbar {
                    area: id,
                    axis: bar.axis,
                    start_pos: pos,
                    start_scroll: scroll,
                    scroll_per_pixel: bar.max / travel.max(1.0),
                });
            } else {
                let next = scroll + bar.viewport * if pos < start { -0.9 } else { 0.9 };
                if horizontal {
                    self.set_any_scroll(id, next, sy);
                } else {
                    self.set_any_scroll(id, sx, next);
                }
            }
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_bars_drag_both_axes_page_without_selecting_and_follow_frozen_columns() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut table = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 240.0,
                padding: 0.0,
                ..ColumnStyle::default()
            },
            |c| {
                table = c.table(
                    (0..4)
                        .map(|i| TableColumn::new(format!("Column {i}"), 100.0))
                        .collect(),
                    (0..50).map(|i| vec![i.to_string(); 4]).collect(),
                    TableStyle {
                        width: 240.0,
                        height: 160.0,
                        ..TableStyle::default()
                    },
                );
            },
        );
        ui.set_table_frozen_cols(table, 1);
        ui.set_table_selected_row(table, Some(3));
        ui.sync(&mut scene);
        let bars = ui.scrollbar_geometry(&scene, table);
        assert_eq!(bars.len(), 2);
        scene.take_dirty();
        ui.paint_scrollbars(&mut scene, table);
        assert!(
            scene.dirty_peek().is_empty(),
            "Unchanged bars must not invalidate the scene"
        );
        let bounds = scene.get(ui.widgets[&table].node.unwrap()).unwrap().bounds;
        assert!(bars[0].track.x >= bounds.x + 100.0);
        for bar in bars {
            let x = bar.thumb.x + bar.thumb.width * 0.5;
            let y = bar.thumb.y + bar.thumb.height * 0.5;
            assert_eq!(ui.hit_widget(&scene, x, y), Some(table));
            assert!(ui.pointer_pressed(&mut scene, x, y));
            assert!(ui.is_dragging());
            assert!(!ui.pointer_moved(f32::NAN, f32::NAN));
            assert!(ui.pointer_moved(x + 10000.0, y + 10000.0));
            ui.pointer_released();
            ui.sync(&mut scene);
            let offset = ui.table_scroll_offset(table).unwrap();
            let actual = if bar.axis == Axis::Horizontal {
                offset.0
            } else {
                offset.1
            };
            assert!((actual - bar.max).abs() < 0.1);
            assert_eq!(ui.table_selected_row(table), Some(3));
            assert!(ui.take_table_header_click().is_none());
        }
        let vertical = ui
            .scrollbar_geometry(&scene, table)
            .into_iter()
            .find(|b| b.axis == Axis::Vertical)
            .unwrap();
        let before = ui.table_scroll_offset(table).unwrap();
        assert!(ui.pointer_pressed(&mut scene, vertical.track.x + 4.0, vertical.track.y + 1.0));
        let after = ui.table_scroll_offset(table).unwrap();
        assert!(after.1 < before.1);
        assert_eq!(after.0, before.0);
        ui.set_table_rows(table, Vec::new());
        ui.sync(&mut scene);
        let nodes = ui.scrollbars[&table];
        assert!(!scene.get(nodes[2]).unwrap().visible);
        assert!(!scene.get(nodes[3]).unwrap().visible);
        assert!(scene.get(nodes[0]).unwrap().visible);
    }

    #[test]
    fn scroll_area_bar_takes_priority_over_children_and_text_bar_keeps_caret() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut area = WidgetId(0);
        let mut editor = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 220.0,
                height: 500.0,
                padding: 0.0,
                ..ColumnStyle::default()
            },
            |c| {
                area = c.scroll_area(
                    ScrollAreaStyle {
                        width: 220.0,
                        height: 100.0,
                        padding: 0.0,
                        ..ScrollAreaStyle::default()
                    },
                    |c| {
                        for _ in 0..6 {
                            c.button("Wide button ".repeat(10));
                        }
                    },
                );
                editor = c.text_area(
                    "SELECT 1;\n".repeat(100),
                    "SQL",
                    TextAreaStyle {
                        width: 220.0,
                        height: 100.0,
                        ..TextAreaStyle::default()
                    },
                );
            },
        );
        ui.sync(&mut scene);
        let bar = ui
            .scrollbar_geometry(&scene, area)
            .into_iter()
            .find(|b| b.axis == Axis::Vertical)
            .unwrap();
        let x = bar.thumb.x + 1.0;
        let y = bar.thumb.y + 1.0;
        assert_eq!(ui.hit_widget(&scene, x, y), Some(area));
        assert!(ui.pointer_pressed(&mut scene, x, y));
        assert!(ui.pointer_moved(x, y + 1000.0));
        ui.pointer_released();
        assert!((ui.scroll_offset(area).unwrap().1 - bar.max).abs() < 0.1);
        assert!(ui.take_button_activate().is_none());
        let bar = ui.scrollbar_geometry(&scene, editor)[0];
        let caret = ui.text_area_caret(editor);
        let x = bar.thumb.x + 1.0;
        let y = bar.thumb.y + 1.0;
        ui.pointer_pressed(&mut scene, x, y);
        assert!(ui.pointer_moved(x, y + 1000.0));
        ui.pointer_released();
        ui.sync(&mut scene);
        assert!((ui.text_area_scroll(editor).unwrap() - bar.max).abs() < 0.1);
        assert_eq!(ui.text_area_caret(editor), caret);
        assert!(ui.text_area_value(editor).unwrap().starts_with("SELECT 1;"));
    }

    #[test]
    fn keyboard_focus_reveals_nested_scroll_children_without_undoing_wheel_scroll() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut outer = WidgetId(0);
        let mut inner = WidgetId(0);
        let mut target = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 220.0,
                padding: 0.0,
                ..ColumnStyle::default()
            },
            |c| {
                outer = c.scroll_area(
                    ScrollAreaStyle {
                        width: 220.0,
                        height: 100.0,
                        padding: 0.0,
                        ..ScrollAreaStyle::default()
                    },
                    |c| {
                        for _ in 0..8 {
                            c.button("Earlier action");
                        }
                        inner = c.scroll_row(
                            RowStyle {
                                width: 220.0,
                                height: 40.0,
                                padding: 0.0,
                                ..RowStyle::default()
                            },
                            |c| {
                                for i in 0..8 {
                                    target = c.button(format!("Action {i}"));
                                }
                            },
                        );
                    },
                );
            },
        );
        ui.sync(&mut scene);
        ui.set_focus(Some(target));
        ui.sync(&mut scene);
        assert!(ui.scroll_offset(outer).unwrap().1 > 0.0);
        assert!(ui.scroll_offset(inner).unwrap().0 > 0.0);
        let target_bounds = scene.get(ui.widgets[&target].node.unwrap()).unwrap().bounds;
        let x = target_bounds.x + target_bounds.width * 0.5;
        let y = target_bounds.y + target_bounds.height * 0.5;
        assert_eq!(ui.hit_widget(&scene, x, y), Some(target));
        ui.set_scroll_offset(outer, 0.0, 0.0);
        ui.set_scroll_offset(inner, 0.0, 0.0);
        ui.sync(&mut scene);
        assert_eq!(ui.scroll_offset(outer), Some((0.0, 0.0)));
        assert_eq!(ui.scroll_offset(inner), Some((0.0, 0.0)));
    }

    #[test]
    fn inactive_tab_bars_hide_and_restore_without_stale_hit_targets() {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut tabs = WidgetId(0);
        let mut list = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 220.0,
                padding: 0.0,
                ..ColumnStyle::default()
            },
            |c| {
                tabs = c.tabs(
                    TabsStyle {
                        width: 220.0,
                        height: 200.0,
                        ..TabsStyle::default()
                    },
                    |tabs| {
                        tabs.page("Long list", |c| {
                            list = c.list(
                                (0..100).map(|i| i.to_string()),
                                ListStyle {
                                    width: 210.0,
                                    height: 100.0,
                                    ..ListStyle::default()
                                },
                            );
                        });
                        tabs.page("Other", |c| {
                            c.text("Other page");
                        });
                    },
                );
            },
        );
        ui.sync(&mut scene);
        let bar = ui.scrollbar_geometry(&scene, list)[0];
        let point = (bar.thumb.x + 1.0, bar.thumb.y + 1.0);
        assert_eq!(ui.hit_widget(&scene, point.0, point.1), Some(list));
        for _ in 0..3 {
            assert!(ui.set_tabs_selected(tabs, 1));
            ui.sync(&mut scene);
            for node in ui.scrollbars[&list] {
                assert!(!scene.get(node).unwrap().visible);
            }
            assert_ne!(ui.hit_widget(&scene, point.0, point.1), Some(list));
            assert!(ui.set_tabs_selected(tabs, 0));
            ui.sync(&mut scene);
            assert_eq!(ui.hit_widget(&scene, point.0, point.1), Some(list));
        }
        ui.set_focus(Some(list));
        assert!(ui.key_input(UiKey::End));
        ui.sync(&mut scene);
        let bar = ui.scrollbar_geometry(&scene, list)[0];
        assert!((bar.thumb.y + bar.thumb.height - bar.track.y - bar.track.height).abs() < 1.0);
    }
}
