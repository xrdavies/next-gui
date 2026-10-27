use super::*;

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct TabStripState {
    offset: f32,
    pub(super) selected: Option<usize>,
    width: f32,
    size: f32,
}

/// Opaque tab-strip viewport for rebuilding the same titles and selected page.
#[derive(Debug, Clone, PartialEq)]
pub struct TabsState {
    titles: Vec<String>,
    selected: usize,
    scrollable: bool,
    strip: TabStripState,
}

struct StripGeometry {
    tabs: Vec<Rect>,
    viewport: Rect,
    arrows: Option<[Rect; 2]>,
    max_offset: f32,
}

impl Ui {
    /// Capture the current tab strip without changing its selected page.
    pub fn tabs_state(&self, id: WidgetId) -> Option<TabsState> {
        let Kind::Tabs {
            titles,
            selected,
            strip,
            style,
            ..
        } = &self.widgets.get(&id)?.kind
        else {
            return None;
        };
        Some(TabsState {
            titles: titles.clone(),
            selected: *selected,
            scrollable: style.scrollable,
            strip: strip.clone(),
        })
    }

    /// Restore the strip only for identical titles/selection and matching width/font size.
    /// A changed layout keeps its newly revealed selected page. Returns false for a different tab set.
    pub fn restore_tabs_state(&mut self, id: WidgetId, state: &TabsState) -> bool {
        let Some(Widget {
            kind:
                Kind::Tabs {
                    titles,
                    selected,
                    style,
                    strip,
                    ..
                },
            ..
        }) = self.widgets.get_mut(&id)
        else {
            return false;
        };
        if *titles != state.titles
            || *selected != state.selected
            || style.scrollable != state.scrollable
        {
            return false;
        }
        if style.width == state.strip.width && style.size == state.strip.size {
            *strip = state.strip.clone();
        }
        true
    }

    fn tab_strip_geometry(&self, id: WidgetId, bounds: Rect) -> Option<StripGeometry> {
        let Kind::Tabs {
            titles,
            style,
            strip,
            ..
        } = &self.widgets.get(&id)?.kind
        else {
            return None;
        };
        let widths: Vec<_> = titles
            .iter()
            .map(|title| {
                if style.scrollable {
                    (self.font.measure_line(title, style.size).0 + 20.0).max(40.0)
                } else {
                    (bounds.width / titles.len() as f32).max(40.0)
                }
            })
            .collect();
        let overflow = style.scrollable && widths.iter().sum::<f32>() > bounds.width;
        let button = if overflow {
            style.strip_height.min(bounds.width * 0.25).max(0.0)
        } else {
            0.0
        };
        let viewport = Rect::new(
            bounds.x,
            bounds.y,
            (bounds.width - 2.0 * button).max(0.0),
            style.strip_height,
        );
        let max_offset = (widths.iter().sum::<f32>() - viewport.width).max(0.0);
        let mut x = bounds.x
            - if style.scrollable {
                strip.offset.clamp(0.0, max_offset)
            } else {
                0.0
            };
        let tabs = widths
            .into_iter()
            .map(|width| {
                let rect = Rect::new(x, bounds.y, width, style.strip_height);
                x += width;
                rect
            })
            .collect();
        let arrows = overflow.then(|| {
            [
                Rect::new(
                    bounds.x + viewport.width,
                    bounds.y,
                    button,
                    style.strip_height,
                ),
                Rect::new(
                    bounds.x + viewport.width + button,
                    bounds.y,
                    button,
                    style.strip_height,
                ),
            ]
        });
        Some(StripGeometry {
            tabs,
            viewport,
            arrows,
            max_offset,
        })
    }

    pub(super) fn pointer_tabs(&mut self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> bool {
        let Some(Widget {
            kind: Kind::Tabs { style, strip, .. },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return false;
        };
        let bounds = node
            .and_then(|node| scene.get(node).map(|node| node.bounds))
            .unwrap_or_else(Rect::empty);
        if !x.is_finite() || !y.is_finite() || !bounds.contains(x, y) {
            return false;
        }
        if y >= bounds.y + style.strip_height {
            return true;
        }
        let offset = strip.offset;
        let Some(geometry) = self.tab_strip_geometry(id, bounds) else {
            return false;
        };
        if let Some(arrows) = geometry.arrows {
            for (direction, arrow) in arrows.iter().enumerate() {
                if arrow.contains(x, y) {
                    let next = if direction == 0 {
                        geometry
                            .tabs
                            .iter()
                            .rev()
                            .find(|tab| tab.x < bounds.x - 0.5)
                            .map_or(0.0, |tab| offset + tab.x - bounds.x)
                    } else {
                        geometry
                            .tabs
                            .iter()
                            .find(|tab| tab.x > bounds.x + 0.5)
                            .map_or(geometry.max_offset, |tab| offset + tab.x - bounds.x)
                    }
                    .clamp(0.0, geometry.max_offset);
                    if let Kind::Tabs { strip, .. } = &mut self.widgets.get_mut(&id).unwrap().kind {
                        strip.offset = next;
                    }
                    return true;
                }
            }
        }
        if !geometry.viewport.contains(x, y) {
            return false;
        }
        geometry
            .tabs
            .iter()
            .position(|tab| tab.contains(x, y))
            .is_some_and(|index| self.set_tabs_selected(id, index))
    }

    pub(super) fn paint_tabs_strip(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind:
                Kind::Tabs {
                    titles,
                    selected,
                    style,
                    strip,
                    ..
                },
            node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let (titles, selected, style, previous) =
            (titles.clone(), *selected, *style, strip.clone());
        let Some(host) = *node else {
            return;
        };
        let bounds = scene
            .get(host)
            .map(|node| node.bounds)
            .unwrap_or_else(Rect::empty);
        let Some(mut geometry) = self.tab_strip_geometry(id, bounds) else {
            return;
        };
        let mut offset = previous.offset.clamp(0.0, geometry.max_offset);
        if style.scrollable
            && (previous.selected != Some(selected)
                || previous.width != bounds.width
                || previous.size != style.size)
        {
            if let Some(tab) = geometry.tabs.get(selected) {
                if tab.width > geometry.viewport.width || tab.x < geometry.viewport.x {
                    offset += tab.x - geometry.viewport.x;
                } else if tab.x + tab.width > geometry.viewport.x + geometry.viewport.width {
                    offset += tab.x + tab.width - geometry.viewport.x - geometry.viewport.width;
                }
            }
        }
        offset = offset.clamp(0.0, geometry.max_offset);
        if let Kind::Tabs { strip, .. } = &mut self.widgets.get_mut(&id).unwrap().kind {
            *strip = TabStripState {
                offset,
                selected: Some(selected),
                width: bounds.width,
                size: style.size,
            };
        }
        geometry = self.tab_strip_geometry(id, bounds).unwrap();
        let needed = titles.len() * 2 + if geometry.arrows.is_some() { 4 } else { 0 };
        self.ensure_aux_nodes(scene, host, id, needed);
        let aux = self.widgets[&id].aux.clone();
        for &node in aux.iter().skip(needed) {
            scene.set_visible(node, false);
        }
        for (i, title) in titles.iter().enumerate() {
            let tab = geometry.tabs[i];
            let active = i == selected;
            let clip = style.scrollable.then_some(geometry.viewport);
            let bg = aux[i * 2];
            scene.set_bounds(bg, tab);
            scene.set_fill(
                bg,
                if active {
                    style.active_bg
                } else {
                    style.strip_bg
                },
            );
            scene.set_mask(bg, None);
            scene.set_clip(bg, clip);
            scene.set_visible(bg, true);
            self.apply_corner_radius(scene, bg, false);
            let (_, mask) = self.font.rasterize_text(title, style.size, None);
            let lab = aux[i * 2 + 1];
            scene.set_bounds(
                lab,
                Rect::new(
                    tab.x + 10.0,
                    bounds.y + ((style.strip_height - mask.height as f32) * 0.5).max(0.0),
                    mask.width as f32,
                    mask.height as f32,
                ),
            );
            scene.set_fill(lab, if active { style.active_fg } else { style.fg });
            scene.set_mask(
                lab,
                Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
            );
            scene.set_clip(lab, clip);
            scene.set_visible(lab, true);
        }
        if let Some(arrows) = geometry.arrows {
            for (i, rect) in arrows.iter().enumerate() {
                let enabled = if i == 0 {
                    offset > 0.0
                } else {
                    offset < geometry.max_offset
                };
                let bg = aux[titles.len() * 2 + i * 2];
                scene.set_bounds(bg, *rect);
                scene.set_fill(bg, style.strip_bg);
                scene.set_mask(bg, None);
                scene.set_clip(bg, Some(*rect));
                scene.set_visible(bg, true);
                let (_, mask) = self.font.rasterize_text(
                    if i == 0 { "‹" } else { "›" },
                    style.size + 4.0,
                    None,
                );
                let lab = aux[titles.len() * 2 + i * 2 + 1];
                scene.set_bounds(
                    lab,
                    Rect::new(
                        rect.x + (rect.width - mask.width as f32) * 0.5,
                        rect.y + (rect.height - mask.height as f32) * 0.5,
                        mask.width as f32,
                        mask.height as f32,
                    ),
                );
                scene.set_fill(
                    lab,
                    if enabled {
                        style.fg
                    } else {
                        self.theme.colors.disabled
                    },
                );
                scene.set_mask(
                    lab,
                    Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                );
                scene.set_clip(lab, Some(*rect));
                scene.set_visible(lab, true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example(width: f32, scrollable: bool, titles: &[&str]) -> (Ui, Scene, WidgetId) {
        let mut ui = Ui::new();
        let mut scene = Scene::new();
        let mut tabs = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 20.0,
                y: 10.0,
                width,
                height: 160.0,
                padding: 0.0,
                ..ColumnStyle::default()
            },
            |c| {
                tabs = c.tabs(
                    TabsStyle {
                        width,
                        height: 160.0,
                        scrollable,
                        ..TabsStyle::default()
                    },
                    |t| {
                        for title in titles {
                            t.page(*title, |page| {
                                page.text(*title);
                            });
                        }
                    },
                );
            },
        );
        ui.sync(&mut scene);
        ui.set_focus(Some(tabs));
        (ui, scene, tabs)
    }

    fn geometry(ui: &Ui, scene: &Scene, tabs: WidgetId) -> StripGeometry {
        let bounds = scene.get(ui.widgets[&tabs].node.unwrap()).unwrap().bounds;
        ui.tab_strip_geometry(tabs, bounds).unwrap()
    }

    fn click_arrow(ui: &mut Ui, scene: &mut Scene, tabs: WidgetId, direction: usize) {
        let arrow = geometry(ui, scene, tabs).arrows.unwrap()[direction];
        assert!(ui.pointer_tabs(
            scene,
            tabs,
            arrow.x + arrow.width * 0.5,
            arrow.y + arrow.height * 0.5
        ));
        ui.sync(scene);
    }

    #[test]
    fn scrollable_tabs_keep_title_widths_and_reveal_keyboard_selection() {
        let (mut ui, mut scene, tabs) = example(
            220.0,
            true,
            &["Edit Cell", "Database Schema", "SQL Log", "Plot", "Remote"],
        );
        let initial = geometry(&ui, &scene, tabs);
        assert!(initial.arrows.is_some());
        for (i, tab) in initial.tabs.iter().enumerate() {
            let label = scene.get(ui.widgets[&tabs].aux[i * 2 + 1]).unwrap();
            assert!(label.bounds.x + label.bounds.width <= tab.x + tab.width + 0.5);
            assert_eq!(label.clip, Some(initial.viewport));
        }
        click_arrow(&mut ui, &mut scene, tabs, 1);
        assert_eq!(
            ui.tabs_selected(tabs),
            Some(0),
            "scrolling does not change pages"
        );
        assert!(geometry(&ui, &scene, tabs).tabs[0].x < initial.tabs[0].x);
        assert!(ui.key_input(UiKey::End));
        ui.sync(&mut scene);
        let end = geometry(&ui, &scene, tabs);
        let last = end.tabs.last().unwrap();
        assert!(
            last.x >= end.viewport.x
                && last.x + last.width <= end.viewport.x + end.viewport.width + 0.5
        );
        assert_eq!(ui.accessible_name(tabs).as_deref(), Some("Remote"));
        let selected = end.tabs[3];
        let x = (selected.x + selected.width * 0.5).max(end.viewport.x + 1.0);
        assert!(ui.pointer_tabs(&scene, tabs, x, selected.y + selected.height * 0.5));
        assert_eq!(ui.tabs_selected(tabs), Some(3));
        assert!(ui.key_input(UiKey::Home));
        ui.sync(&mut scene);
        assert_eq!(geometry(&ui, &scene, tabs).tabs[0].x, initial.viewport.x);
        assert!(!ui.pointer_tabs(&scene, tabs, f32::NAN, 20.0));
        assert!(!ui.pointer_tabs(&scene, tabs, initial.viewport.x - 1.0, 20.0));
    }

    #[test]
    fn strip_sessions_retain_manual_scroll_but_reject_different_pages_and_layouts() {
        let titles = ["Edit Cell", "Database Schema", "SQL Log", "Plot", "Remote"];
        let (mut ui, mut scene, tabs) = example(220.0, true, &titles);
        click_arrow(&mut ui, &mut scene, tabs, 1);
        let expected = geometry(&ui, &scene, tabs).tabs[0].x;
        let state = ui.tabs_state(tabs).unwrap();
        let (mut next, mut next_scene, next_tabs) = example(220.0, true, &titles);
        assert!(next.restore_tabs_state(next_tabs, &state));
        next.sync(&mut next_scene);
        assert_eq!(geometry(&next, &next_scene, next_tabs).tabs[0].x, expected);
        next.set_tabs_selected(next_tabs, 4);
        next.sync(&mut next_scene);
        assert!(!next.restore_tabs_state(next_tabs, &state));
        let (mut wide, mut wide_scene, wide_tabs) = example(500.0, true, &titles);
        assert!(wide.restore_tabs_state(wide_tabs, &state));
        wide.sync(&mut wide_scene);
        assert_eq!(geometry(&wide, &wide_scene, wide_tabs).tabs[0].x, 20.0);
        assert!(geometry(&wide, &wide_scene, wide_tabs).arrows.is_none());
        let (mut other, _, other_tabs) = example(220.0, true, &["Different"]);
        assert!(!other.restore_tabs_state(other_tabs, &state));
        assert!(!other.restore_tabs_state(WidgetId(0), &state));
        assert!(ui.tabs_state(WidgetId(0)).is_none());
    }

    #[test]
    fn legacy_equal_tabs_empty_strips_and_oversized_titles_remain_usable() {
        let (ui, scene, tabs) = example(240.0, false, &["General", "Alarms", "About"]);
        let strip = geometry(&ui, &scene, tabs);
        assert!(strip.arrows.is_none());
        assert!(strip.tabs.iter().all(|tab| tab.width == 80.0));
        let (mut ui, scene, tabs) = example(160.0, true, &[]);
        assert!(geometry(&ui, &scene, tabs).tabs.is_empty());
        assert!(!ui.pointer_tabs(&scene, tabs, 30.0, 20.0));
        let title = "A title longer than the entire narrow strip";
        let (mut ui, mut scene, tabs) = example(160.0, true, &[title]);
        assert_eq!(
            geometry(&ui, &scene, tabs).tabs[0].x,
            20.0,
            "initially show the title's start"
        );
        click_arrow(&mut ui, &mut scene, tabs, 1);
        assert!(geometry(&ui, &scene, tabs).tabs[0].x < 20.0);
        assert!(ui.key_input(UiKey::Home));
        ui.sync(&mut scene);
        assert_eq!(geometry(&ui, &scene, tabs).tabs[0].x, 20.0);
        assert_eq!(ui.accessible_name(tabs).as_deref(), Some(title));
    }
}
