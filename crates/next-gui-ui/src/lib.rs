//! Compact declarative UI API for next-gui.
//!
//! P1: retained widget tree (Column / Text / Button) built with a short builder,
//! synced into [`next_gui_scene::Scene`]. Not a third-party UI kernel; no GPU path.

#![deny(missing_docs)]

mod bitmap_font;

use std::collections::HashMap;

use next_gui_layout::{layout_flex, Axis, ChildMeasure, FlexParams};
use next_gui_scene::{Color, NodeId, Rect, Scene};

/// Stable widget identifier within a [`Ui`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetId(pub u64);

impl WidgetId {
    /// Raw id for debugging.
    pub fn raw(self) -> u64 {
        self.0
    }
}

/// Style for a column container.
#[derive(Debug, Clone, Copy)]
pub struct ColumnStyle {
    /// Outer x.
    pub x: f32,
    /// Outer y.
    pub y: f32,
    /// Max width available to the column.
    pub width: f32,
    /// Max height available.
    pub height: f32,
    /// Padding inside the column.
    pub padding: f32,
    /// Gap between children.
    pub gap: f32,
    /// Optional panel fill; `None` = host node stays invisible.
    pub background: Option<Color>,
}

impl Default for ColumnStyle {
    fn default() -> Self {
        Self {
            x: 24.0,
            y: 24.0,
            width: 400.0,
            height: 560.0,
            padding: 16.0,
            gap: 12.0,
            background: Some(Color::rgb(0x2a, 0x2a, 0x2e)),
        }
    }
}

#[derive(Debug)]
enum Kind {
    Column {
        style: ColumnStyle,
        children: Vec<WidgetId>,
    },
    Text {
        content: String,
        color: Color,
        scale: f32,
    },
    Button {
        label: String,
        bg: Color,
        fg: Color,
        scale: f32,
        min_width: f32,
        height: f32,
    },
}

#[derive(Debug)]
struct Widget {
    kind: Kind,
    /// Primary scene node (container / hit target).
    node: Option<NodeId>,
    /// Glyph pixel nodes for text/button labels.
    glyphs: Vec<NodeId>,
}

/// Retained UI tree that mounts into a [`Scene`].
#[derive(Debug, Default)]
pub struct Ui {
    next_id: u64,
    widgets: HashMap<WidgetId, Widget>,
    /// Top-level columns (mount roots).
    roots: Vec<WidgetId>,
    /// scene node → widget (buttons primarily).
    node_to_widget: HashMap<NodeId, WidgetId>,
}

impl Ui {
    /// Empty UI.
    pub fn new() -> Self {
        Self {
            next_id: 1,
            widgets: HashMap::new(),
            roots: Vec::new(),
            node_to_widget: HashMap::new(),
        }
    }

    fn alloc(&mut self) -> WidgetId {
        let id = WidgetId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Declare a column and build its children with a compact closure.
    pub fn column<F>(&mut self, style: ColumnStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut ColumnBuilder<'_>),
    {
        let id = self.alloc();
        self.widgets.insert(
            id,
            Widget {
                kind: Kind::Column {
                    style,
                    children: Vec::new(),
                },
                node: None,
                glyphs: Vec::new(),
            },
        );
        self.roots.push(id);
        {
            let mut builder = ColumnBuilder { ui: self, column: id };
            build(&mut builder);
        }
        id
    }

    /// Update text content; call [`Ui::sync`] afterwards to refresh scene glyphs/layout.
    pub fn set_text(&mut self, id: WidgetId, content: impl Into<String>) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Text { content: c, .. } = &mut w.kind {
                *c = content.into();
            }
        }
    }

    /// Read text content if the widget is a Text.
    pub fn text_content(&self, id: WidgetId) -> Option<&str> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Text { content, .. }) => Some(content.as_str()),
            _ => None,
        }
    }

    /// Whether this id is a button.
    pub fn is_button(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Button { .. })
        )
    }

    /// Map a scene node to a widget (buttons are registered).
    pub fn widget_from_node(&self, node: NodeId) -> Option<WidgetId> {
        self.node_to_widget.get(&node).copied()
    }

    /// Hit-test widgets using the scene graph (glyph hits walk up to the button).
    pub fn hit_widget(&self, scene: &Scene, x: f32, y: f32) -> Option<WidgetId> {
        let node = scene.hit_test(x, y)?;
        if let Some(wid) = self.widget_from_node(node) {
            return Some(wid);
        }
        let mut cur = scene.get(node)?.parent;
        while let Some(pid) = cur {
            if let Some(wid) = self.widget_from_node(pid) {
                return Some(wid);
            }
            cur = scene.get(pid).and_then(|n| n.parent);
        }
        None
    }

    /// Ensure scene nodes exist, run layout, paint labels into the scene.
    pub fn sync(&mut self, scene: &mut Scene) {
        let parent = scene.root();
        for root in self.roots.clone() {
            self.ensure_mounted(scene, parent, root);
            self.layout_widget(scene, root);
            self.paint_widget(scene, root);
        }
    }

    fn ensure_mounted(&mut self, scene: &mut Scene, parent: NodeId, id: WidgetId) {
        let already = self.widgets.get(&id).and_then(|w| w.node).is_some();
        if !already {
            let (fill, visible) = match self.widgets.get(&id).map(|w| &w.kind) {
                Some(Kind::Column { style, .. }) => (
                    style.background.unwrap_or(Color::rgba(0, 0, 0, 0)),
                    style.background.is_some(),
                ),
                Some(Kind::Text { .. }) => (Color::rgba(0, 0, 0, 0), false),
                Some(Kind::Button { bg, .. }) => (*bg, true),
                None => return,
            };

            let node = scene
                .insert(parent, Rect::new(0.0, 0.0, 1.0, 1.0), fill)
                .expect("insert widget node");
            if !visible {
                scene.set_visible(node, false);
            }
            if let Some(w) = self.widgets.get_mut(&id) {
                w.node = Some(node);
            }
            if matches!(
                self.widgets.get(&id).map(|w| &w.kind),
                Some(Kind::Button { .. })
            ) {
                self.node_to_widget.insert(node, id);
            }
        }

        let (col_node, children) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Column { children, .. },
                node,
                ..
            }) => (*node, children.clone()),
            _ => return,
        };
        let host = col_node.unwrap_or(parent);
        for child in children {
            self.ensure_mounted(scene, host, child);
        }
    }

    fn measure_widget(&self, id: WidgetId) -> ChildMeasure {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Text {
                content, scale, ..
            }) => {
                let (w, h) = bitmap_font::measure(content, *scale);
                ChildMeasure::new(w, h + 4.0)
            }
            Some(Kind::Button {
                label,
                scale,
                min_width,
                height,
                ..
            }) => {
                let (tw, _) = bitmap_font::measure(label, *scale);
                let pad_x = 16.0;
                ChildMeasure::new((*min_width).max(tw + pad_x * 2.0), *height)
            }
            Some(Kind::Column {
                style, children, ..
            }) => {
                let measures: Vec<_> = children
                    .iter()
                    .map(|c| self.measure_widget(*c))
                    .collect();
                let params = FlexParams {
                    axis: Axis::Vertical,
                    x: 0.0,
                    y: 0.0,
                    max_width: style.width,
                    max_height: style.height,
                    padding: style.padding,
                    gap: style.gap,
                };
                next_gui_layout::measure_flex(&params, &measures)
            }
            None => ChildMeasure::new(0.0, 0.0),
        }
    }

    fn layout_widget(&mut self, scene: &mut Scene, id: WidgetId) {
        let (style, children, col_node) = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Column {
                    style, children, ..
                },
                node,
                ..
            }) => (*style, children.clone(), *node),
            _ => return,
        };

        let measures: Vec<_> = children
            .iter()
            .map(|c| self.measure_widget(*c))
            .collect();
        let params = FlexParams {
            axis: Axis::Vertical,
            x: style.x,
            y: style.y,
            max_width: style.width,
            max_height: style.height,
            padding: style.padding,
            gap: style.gap,
        };
        let total = next_gui_layout::measure_flex(&params, &measures);
        if let Some(node) = col_node {
            scene.set_bounds(
                node,
                Rect::new(
                    style.x,
                    style.y,
                    total.width.max(100.0),
                    total.height.max(40.0),
                ),
            );
            if style.background.is_some() {
                scene.set_visible(node, true);
            }
        }
        let rects = layout_flex(&params, &measures);
        for (child_id, rect) in children.iter().copied().zip(rects) {
            if let Some(w) = self.widgets.get(&child_id) {
                if let Some(node) = w.node {
                    scene.set_bounds(node, rect);
                    match &w.kind {
                        Kind::Button { .. } => {
                            scene.set_visible(node, true);
                        }
                        Kind::Text { .. } => {
                            scene.set_visible(node, false);
                        }
                        Kind::Column { .. } => {}
                    }
                }
            }
            if matches!(
                self.widgets.get(&child_id).map(|w| &w.kind),
                Some(Kind::Column { .. })
            ) {
                self.layout_widget(scene, child_id);
            }
        }
    }

    fn paint_widget(&mut self, scene: &mut Scene, id: WidgetId) {
        if let Some(Kind::Column { children, .. }) = self.widgets.get(&id).map(|w| &w.kind) {
            let children = children.clone();
            for child in children {
                self.paint_widget(scene, child);
            }
            return;
        }

        let Some(w) = self.widgets.get_mut(&id) else {
            return;
        };
        let Some(node) = w.node else {
            return;
        };
        let bounds = scene
            .get(node)
            .map(|n| n.bounds)
            .unwrap_or_else(Rect::empty);

        match &w.kind {
            Kind::Text {
                content,
                color,
                scale,
            } => {
                let content = content.clone();
                let color = *color;
                let scale = *scale;
                bitmap_font::sync_text(
                    scene,
                    node,
                    &content,
                    Rect::new(bounds.x, bounds.y + 2.0, bounds.width, bounds.height),
                    color,
                    scale,
                    &mut w.glyphs,
                );
            }
            Kind::Button {
                label,
                fg,
                scale,
                bg,
                ..
            } => {
                let label = label.clone();
                let fg = *fg;
                let scale = *scale;
                let bg = *bg;
                scene.set_fill(node, bg);
                let text_origin = Rect::new(
                    bounds.x + 16.0,
                    bounds.y + (bounds.height - 7.0 * scale) * 0.5,
                    bounds.width,
                    bounds.height,
                );
                bitmap_font::sync_text(
                    scene,
                    node,
                    &label,
                    text_origin,
                    fg,
                    scale,
                    &mut w.glyphs,
                );
            }
            Kind::Column { .. } => {}
        }
    }
}

/// Compact builder for column children.
pub struct ColumnBuilder<'a> {
    ui: &'a mut Ui,
    column: WidgetId,
}

impl ColumnBuilder<'_> {
    /// Add a text label; returns its [`WidgetId`].
    pub fn text(&mut self, content: impl Into<String>) -> WidgetId {
        self.text_styled(content, Color::rgb(0xe5, 0xe5, 0xe5), 2.0)
    }

    /// Add a text label with explicit color/scale.
    pub fn text_styled(
        &mut self,
        content: impl Into<String>,
        color: Color,
        scale: f32,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Text {
                    content: content.into(),
                    color,
                    scale,
                },
                node: None,
                glyphs: Vec::new(),
            },
        );
        self.push_child(id);
        id
    }

    /// Add a button; returns its [`WidgetId`] for hit-testing.
    pub fn button(&mut self, label: impl Into<String>) -> WidgetId {
        self.button_styled(
            label,
            Color::rgb(0x3b, 0x82, 0xf6),
            Color::rgb(0xff, 0xff, 0xff),
        )
    }

    /// Button with custom colors.
    pub fn button_styled(
        &mut self,
        label: impl Into<String>,
        bg: Color,
        fg: Color,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Button {
                    label: label.into(),
                    bg,
                    fg,
                    scale: 2.0,
                    min_width: 160.0,
                    height: 40.0,
                },
                node: None,
                glyphs: Vec::new(),
            },
        );
        self.push_child(id);
        id
    }

    fn push_child(&mut self, child: WidgetId) {
        if let Some(w) = self.ui.widgets.get_mut(&self.column) {
            if let Kind::Column { children, .. } = &mut w.kind {
                children.push(child);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_column_and_hit_button() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut label = WidgetId(0);
        let mut inc = WidgetId(0);
        ui.column(ColumnStyle::default(), |col| {
            label = col.text("Count: 0");
            inc = col.button("Increment");
        });
        ui.sync(&mut scene);
        let btn_node = ui.widgets.get(&inc).unwrap().node.unwrap();
        let b = scene.get(btn_node).unwrap().bounds;
        let hit = ui
            .hit_widget(&scene, b.x + 5.0, b.y + 5.0)
            .expect("hit");
        assert_eq!(hit, inc);
        assert!(ui.is_button(inc));
        ui.set_text(label, "Count: 1");
        ui.sync(&mut scene);
        assert_eq!(ui.text_content(label), Some("Count: 1"));
    }
}
