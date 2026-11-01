use super::*;

/// Document-tab requests. The owner can prompt before closing or renaming a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabAction {
    /// The close button was pressed for this page.
    Close(usize),
    /// A tab label was double-clicked.
    Rename(usize),
    /// Move a page to a display position, shifting intervening pages.
    Move {
        /// Original page index.
        from: usize,
        /// Destination page index after removing the original page.
        to: usize,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct TabStripState {
    offset: f32,
    pub(super) selected: Option<usize>,
    width: f32,
    size: f32,
    last_click: Option<(usize, std::time::Instant)>,
}

/// Opaque tab-strip viewport for rebuilding the same titles and selected page.
#[derive(Debug, Clone, PartialEq)]
pub struct TabsState {
    titles: Vec<String>,
    selected: usize,
    scrollable: bool,
    document_actions: (bool, bool, bool),
    strip: TabStripState,
}

struct StripGeometry {
    tabs: Vec<Rect>,
    viewport: Rect,
    arrows: Option<[Rect; 2]>,
    max_offset: f32,
}

impl Ui {
    /// Drain the last close, rename or drag request; page ownership stays with the caller.
    pub fn take_tab_action(&mut self) -> Option<(WidgetId, TabAction)> { self.pending_tab_action.take() }

    /// Change a label without rebuilding its page or losing the strip viewport.
    pub fn set_tab_title(&mut self, id: WidgetId, index: usize, title: impl Into<String>) -> bool {
        let Some(Widget { kind:Kind::Tabs { titles, .. }, .. }) = self.widgets.get_mut(&id) else { return false; };
        let Some(label) = titles.get_mut(index) else { return false; };
        *label = title.into(); true
    }

    /// Header hit test for application context menus, excluding arrows and clipped tabs.
    pub fn tab_at(&self, scene: &Scene, id: WidgetId, x: f32, y: f32) -> Option<usize> {
        let bounds = scene.get(self.widgets.get(&id)?.node?)?.bounds;
        let geometry = self.tab_strip_geometry(id,bounds)?;
        if !geometry.viewport.contains(x,y) { return None; }
        geometry.tabs.iter().position(|tab|tab.contains(x,y))
    }

    pub(super) fn move_tab(&mut self, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() { return false; }
        let Some(DragState::Tab { tabs, from, bounds, origin, moved, .. }) = self.drag else { return false; };
        if !moved && (x-origin.0).abs().max((y-origin.1).abs()) < 5.0 { return false; }
        let Some(g) = self.tab_strip_geometry(tabs,bounds) else { return false; };
        let mut to = from; let mut marker = None; let mut scrolling=false;
        if y >= g.viewport.y-24.0 && y <= g.viewport.y+g.viewport.height+24.0 {
            let pos = g.tabs.iter().position(|tab|x < tab.x+tab.width*0.5).unwrap_or(g.tabs.len());
            to = pos - usize::from(from < pos);
            marker = Some(g.tabs.get(pos).map_or_else(||g.tabs.last().unwrap().x+g.tabs.last().unwrap().width,|t|t.x));
            if let Some(Widget { kind:Kind::Tabs { strip, .. }, .. }) = self.widgets.get_mut(&tabs) {
                let previous=strip.offset;
                if x > g.viewport.x+g.viewport.width-12.0 { strip.offset = (strip.offset+20.0).min(g.max_offset); }
                else if x < g.viewport.x+12.0 { strip.offset = (strip.offset-20.0).max(0.0); }
                scrolling=strip.offset!=previous;
            }
        }
        if let Some(DragState::Tab { to: target, moved, marker: mark, next_repeat, .. }) = &mut self.drag {
            *target=to; *moved=true; *mark=marker;
            *next_repeat=scrolling.then(||std::time::Instant::now()+std::time::Duration::from_millis(33));
        }
        true
    }

    pub(super) fn release_tab(&mut self, tabs: WidgetId, from: usize, to: usize, moved: bool) {
        if moved {
            if from != to { self.pending_tab_action = Some((tabs,TabAction::Move {from,to})); }
            if let Some(Widget { kind:Kind::Tabs { strip, .. }, .. }) = self.widgets.get_mut(&tabs) { strip.last_click=None; }
            return;
        }
        self.set_tabs_selected(tabs,from);
        if let Some(Widget { kind:Kind::Tabs { strip, .. }, .. }) = self.widgets.get_mut(&tabs) {
            let now = std::time::Instant::now();
            if strip.last_click.is_some_and(|(i,time)|i==from && now.duration_since(time).as_millis()<500) {
                self.pending_tab_action = Some((tabs,TabAction::Rename(from))); strip.last_click=None;
            } else { strip.last_click=Some((from,now)); }
        }
    }

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
            document_actions: (style.enabled, style.closable, style.movable),
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
        if style.width == state.strip.width && style.size == state.strip.size
            && (style.enabled,style.closable,style.movable)==state.document_actions {
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
                    (self.font.measure_line(title, style.size).0 + 20.0 + if style.closable {24.0} else {0.0}).max(40.0)
                } else {
                    (bounds.width / titles.len() as f32).max(40.0)
                }
            })
            .collect();
        let strip_y = bounds.y
            + if style.strip_at_bottom {
                (bounds.height - style.strip_height).max(0.0)
            } else {
                0.0
            };
        let overflow = style.scrollable && widths.iter().sum::<f32>() > bounds.width;
        let button = if overflow {
            style.strip_height.min(bounds.width * 0.25).max(0.0)
        } else {
            0.0
        };
        let viewport = Rect::new(
            bounds.x,
            strip_y,
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
                let rect = Rect::new(x, strip_y, width, style.strip_height);
                x += width;
                rect
            })
            .collect();
        let arrows = overflow.then(|| {
            [
                Rect::new(
                    bounds.x + viewport.width,
                    strip_y,
                    button,
                    style.strip_height,
                ),
                Rect::new(
                    bounds.x + viewport.width + button,
                    strip_y,
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
            kind: Kind::Tabs { strip, style, .. },
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
        let offset = strip.offset;
        let style = *style;
        let Some(geometry) = self.tab_strip_geometry(id, bounds) else {
            return false;
        };
        if y < geometry.viewport.y || y >= geometry.viewport.y + geometry.viewport.height {
            return true;
        }
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
        let Some(index) = geometry.tabs.iter().position(|tab| tab.contains(x,y)) else { return false; };
        let tab = geometry.tabs[index];
        if style.closable && x >= tab.x+tab.width-24.0 {
            self.pending_tab_action = Some((id,TabAction::Close(index))); return true;
        }
        if style.movable {
            self.drag = Some(DragState::Tab { tabs:id,from:index,to:index,bounds,origin:(x,y),moved:false,marker:None,next_repeat:None }); true
        } else { self.set_tabs_selected(id,index) }
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
                last_click: previous.last_click,
            };
        }
        geometry = self.tab_strip_geometry(id, bounds).unwrap();
        let base = titles.len() * 2 + if geometry.arrows.is_some() { 4 } else { 0 };
        let needed = base + if style.closable { titles.len() } else { 0 } + usize::from(style.movable);
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
                    tab.y + ((style.strip_height - mask.height as f32) * 0.5).max(0.0),
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
            if style.closable {
                let close = aux[base+i];
                let mask = icon::rasterize_icon(IconName::Close,12.0);
                scene.set_bounds(close,Rect::new(tab.x+tab.width-18.0,tab.y+(tab.height-12.0)*0.5,12.0,12.0));
                scene.set_fill(close,if active {style.active_fg} else {style.fg});
                scene.set_mask(close,Some(mask)); scene.set_clip(close,Some(geometry.viewport.intersect(&tab))); scene.set_visible(close,true);
            }
        }
        if style.movable {
            let marker = match self.drag { Some(DragState::Tab { tabs, marker, .. }) if tabs==id => marker, _ =>None };
            let mark = aux[needed-1];
            scene.set_bounds(mark,Rect::new(marker.unwrap_or(bounds.x)-1.0,geometry.viewport.y,2.0,geometry.viewport.height));
            scene.set_fill(mark,style.active_fg); scene.set_mask(mark,None); scene.set_clip(mark,Some(geometry.viewport)); scene.set_visible(mark,marker.is_some());
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

    #[test]
    fn disabled_document_tabs_reject_pointer_keys_focus_and_actions() {
        let mut ui=Ui::new();let mut scene=Scene::new();let mut tabs=WidgetId(0);
        ui.column(ColumnStyle::default(),|p| {tabs=p.tabs(TabsStyle {enabled:false,movable:true,closable:true,scrollable:true,..Default::default()},|t| {t.page("First",|_|{});t.page("Second",|_|{});});});
        ui.sync(&mut scene);let b=scene.get(ui.widgets[&tabs].node.unwrap()).unwrap().bounds;
        assert!(!ui.focusable_widgets().contains(&tabs));ui.set_focus(Some(tabs));assert_eq!(ui.focused(),None);
        for x in [b.x+12.0,b.x+50.0,b.x+90.0] {
            assert!(!ui.pointer_pressed(&mut scene,x,b.y+12.0));ui.pointer_moved(x+100.0,b.y+12.0);ui.pointer_released();
        }
        assert!(!ui.key_tabs(tabs,UiKey::End));assert_eq!(ui.tabs_selected(tabs),Some(0));assert_eq!(ui.take_tab_action(),None);assert!(!ui.is_dragging());
    }

    #[test]
    fn document_strip_retains_double_click_across_rebuild() {
        let font=Ui::new().font().clone();
        let build=|| {
            let mut ui=Ui::with_font(font.clone());let mut scene=Scene::new();let mut tabs=WidgetId(0);
            ui.column(ColumnStyle {width:260.0,height:100.0,padding:0.0,..Default::default()},|p| {
                tabs=p.tabs(TabsStyle {width:500.0,height:80.0,scrollable:true,movable:true,closable:true,..Default::default()},|t| {t.page("Query",|_|{});});
            });ui.sync(&mut scene);(ui,scene,tabs)
        };
        let (mut ui,mut scene,tabs)=build();let b=scene.get(ui.widgets[&tabs].node.unwrap()).unwrap().bounds;
        ui.pointer_pressed(&mut scene,b.x+12.0,b.y+12.0);ui.pointer_released();let view=ui.tabs_state(tabs).unwrap();
        let (mut next,mut scene,tabs)=build();assert!(next.restore_tabs_state(tabs,&view));
        next.pointer_pressed(&mut scene,b.x+12.0,b.y+12.0);next.pointer_released();
        assert_eq!(next.take_tab_action(),Some((tabs,TabAction::Rename(0))));
    }

    #[test]
    fn held_document_drag_scrolls_overflow_and_cancel_keeps_pages() {
        let mut ui=Ui::new();let mut scene=Scene::new();let mut tabs=WidgetId(0);
        ui.column(ColumnStyle {width:260.0,height:120.0,padding:0.0,..Default::default()},|p| {
            tabs=p.tabs(TabsStyle {width:260.0,height:100.0,scrollable:true,movable:true,closable:true,..Default::default()},|t| {
                for i in 0..12 {t.page(format!("Document {i}"),|_|{});}
            });
        });ui.sync(&mut scene);
        let b=scene.get(ui.widgets[&tabs].node.unwrap()).unwrap().bounds;let g=ui.tab_strip_geometry(tabs,b).unwrap();
        ui.pointer_pressed(&mut scene,b.x+12.0,b.y+12.0);ui.pointer_moved(g.viewport.x+g.viewport.width-3.0,b.y+12.0);
        let before=ui.tabs_state(tabs).unwrap().strip.offset;assert!(ui.pointer_repeat_delay().is_some());
        std::thread::sleep(std::time::Duration::from_millis(40));assert!(ui.repeat_pointer());assert!(ui.tabs_state(tabs).unwrap().strip.offset>before);
        ui.cancel_pointer_capture();ui.pointer_released();assert_eq!(ui.take_tab_action(),None);assert!(ui.pointer_repeat_delay().is_none());assert_eq!(ui.tabs_selected(tabs),Some(0));
    }

    #[test]
    fn document_tabs_emit_source_bound_close_move_and_double_click_requests() {
        let mut ui=Ui::new();let mut scene=Scene::new();
        let mut tabs=WidgetId(0);
        ui.column(ColumnStyle {width:500.0,height:100.0,padding:0.0,..Default::default()},|p| {
            tabs=p.tabs(TabsStyle {width:500.0,height:80.0,scrollable:true,closable:true,movable:true,..Default::default()},|t| {
                for name in ["First SQL","Second SQL","中文 SQL"] {t.page(name,|_|{});}
            });
        });
        ui.sync(&mut scene);
        let bounds=scene.get(ui.widgets[&tabs].node.unwrap()).unwrap().bounds;
        let geometry=ui.tab_strip_geometry(tabs,bounds).unwrap();
        let (x,y)=(geometry.tabs[1].x+10.0,geometry.tabs[1].y+12.0);
        assert_eq!(ui.tab_at(&scene,tabs,x,y),Some(1));
        ui.pointer_pressed(&mut scene,x,y);assert_eq!(ui.tabs_selected(tabs),Some(0));
        ui.pointer_released();assert_eq!(ui.tabs_selected(tabs),Some(1));assert_eq!(ui.take_tab_action(),None);
        let view=ui.tabs_state(tabs).unwrap();assert!(ui.restore_tabs_state(tabs,&view));
        ui.pointer_pressed(&mut scene,x,y);ui.pointer_released();
        assert_eq!(ui.take_tab_action(),Some((tabs,TabAction::Rename(1))));
        let first=geometry.tabs[0];let last=geometry.tabs[2];
        ui.pointer_pressed(&mut scene,first.x+10.0,y);ui.pointer_moved(last.x+last.width-5.0,y);ui.sync(&mut scene);
        assert!(scene.get(*ui.widgets[&tabs].aux.last().unwrap()).unwrap().visible);
        ui.cancel_pointer_capture();ui.pointer_released();assert_eq!(ui.take_tab_action(),None);
        ui.pointer_pressed(&mut scene,first.x+10.0,y);ui.pointer_moved(last.x+last.width-5.0,y);ui.pointer_released();
        assert_eq!(ui.take_tab_action(),Some((tabs,TabAction::Move {from:0,to:2})));
        ui.pointer_pressed(&mut scene,last.x+last.width-12.0,y);ui.pointer_released();
        assert_eq!(ui.take_tab_action(),Some((tabs,TabAction::Close(2))));
        assert_eq!(ui.tabs_selected(tabs),Some(1));
    }

    #[test]
    fn bottom_strips_place_pages_above_labels_and_route_only_strip_pointer_input() {
        for bottom in [false, true] {
            let (mut ui, mut scene, tabs) = example(
                220.0,
                true,
                &[
                    "Edit Database Cell",
                    "SQL Log",
                    "Plot",
                    "DB Schema",
                    "Remote",
                ],
            );
            let Kind::Tabs { style, .. } = &mut ui.widgets.get_mut(&tabs).unwrap().kind else {
                unreachable!()
            };
            style.strip_at_bottom = bottom;
            ui.sync(&mut scene);
            let bounds = scene.get(ui.widgets[&tabs].node.unwrap()).unwrap().bounds;
            let strip = geometry(&ui, &scene, tabs);
            assert_eq!(
                strip.viewport.y,
                if bottom {
                    bounds.y + bounds.height - 32.0
                } else {
                    bounds.y
                }
            );
            let Kind::Tabs { children, .. } = &ui.widgets[&tabs].kind else {
                unreachable!()
            };
            let page = scene
                .get(ui.widgets[&children[0]].node.unwrap())
                .unwrap()
                .bounds;
            assert_eq!(page.y, if bottom { bounds.y } else { bounds.y + 32.0 });
            assert!(
                page.y + page.height
                    <= if bottom {
                        strip.viewport.y
                    } else {
                        bounds.y + bounds.height
                    }
            );
            let content_y = if bottom {
                bounds.y + 4.0
            } else {
                bounds.y + 36.0
            };
            assert!(ui.pointer_tabs(&scene, tabs, bounds.x + 170.0, content_y));
            assert_eq!(ui.tabs_selected(tabs), Some(0));
            click_arrow(&mut ui, &mut scene, tabs, 1);
            assert!(ui.key_input(UiKey::End));
            ui.sync(&mut scene);
            assert_eq!(ui.tabs_selected(tabs), Some(4));
            let geometry = geometry(&ui, &scene, tabs);
            let tab = geometry.tabs[4].intersect(&geometry.viewport);
            assert!(!tab.is_empty());
            assert!(ui.pointer_pressed(
                &mut scene,
                tab.x + tab.width * 0.5,
                tab.y + tab.height * 0.5
            ));
            ui.pointer_released();
            assert_eq!(ui.tabs_selected(tabs), Some(4));
            let aux = &ui.widgets[&tabs].aux;
            let label = scene.get(aux[9]).unwrap();
            assert!(
                label.visible
                    && label.bounds.y >= geometry.viewport.y
                    && label.bounds.y + label.bounds.height <= geometry.viewport.y + 32.0
            );
            let session = ui.tabs_state(tabs).unwrap();
            assert!(ui.restore_tabs_state(tabs, &session));
        }
    }

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
