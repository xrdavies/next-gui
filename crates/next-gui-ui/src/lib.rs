//! Compact declarative UI API for next-gui.
//!
//! Retained widget tree built with a short builder or loaded from a compact `.ngui` file,
//! synced into [`next_gui_scene::Scene`]. Text uses [`next_gui_text`] (font metrics + CJK).
//!
//! Office P0 shell widgets: [`IconName`], Toolbar, Panel, StatusBar, Splitter.

#![deny(missing_docs)]

mod dsl;
mod icon;

pub use dsl::{load_ngui, load_ngui_path, DslError, LoadedUi};
pub use icon::IconName;

use std::collections::HashMap;
use std::sync::Arc;

use next_gui_layout::{layout_flex, Axis, ChildMeasure, CrossAlign, FlexParams};
use next_gui_scene::{AlphaMask, Color, NodeId, Rect, Scene};
use next_gui_text::{load_default_cjk_font, Font};

use icon::rasterize_icon;

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
    /// Cross-axis alignment (horizontal for a column).
    pub cross_align: CrossAlign,
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
            cross_align: CrossAlign::Start,
            background: Some(Color::rgb(0x2a, 0x2a, 0x2e)),
        }
    }
}

/// Style for a row container (toolbars, action strips).
#[derive(Debug, Clone, Copy)]
pub struct RowStyle {
    /// Outer x (ignored when nested; parent layout places the row).
    pub x: f32,
    /// Outer y (ignored when nested).
    pub y: f32,
    /// Max width available to the row.
    pub width: f32,
    /// Max height available.
    pub height: f32,
    /// Padding inside the row.
    pub padding: f32,
    /// Gap between children.
    pub gap: f32,
    /// Cross-axis alignment (vertical for a row).
    pub cross_align: CrossAlign,
    /// Optional strip fill.
    pub background: Option<Color>,
}

impl Default for RowStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 600.0,
            height: 48.0,
            padding: 8.0,
            gap: 8.0,
            cross_align: CrossAlign::Center,
            background: Some(Color::rgb(0x36, 0x36, 0x3a)),
        }
    }
}

/// Style for a first-class Toolbar strip.
#[derive(Debug, Clone, Copy)]
pub struct ToolbarStyle {
    /// Outer x (top-level only).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Width budget.
    pub width: f32,
    /// Height of the bar.
    pub height: f32,
    /// Inner padding.
    pub padding: f32,
    /// Gap between items.
    pub gap: f32,
    /// Vertical alignment of items.
    pub cross_align: CrossAlign,
    /// Bar fill.
    pub background: Option<Color>,
}

impl Default for ToolbarStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 40.0,
            padding: 6.0,
            gap: 6.0,
            cross_align: CrossAlign::Center,
            background: Some(Color::rgb(0x32, 0x32, 0x38)),
        }
    }
}

impl ToolbarStyle {
    fn to_row(self) -> RowStyle {
        RowStyle {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            padding: self.padding,
            gap: self.gap,
            cross_align: self.cross_align,
            background: self.background,
        }
    }
}

/// Style for a titled Panel.
#[derive(Debug, Clone, Copy)]
pub struct PanelStyle {
    /// Outer x (top-level only).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Outer width.
    pub width: f32,
    /// Outer height.
    pub height: f32,
    /// Content padding below the title bar.
    pub padding: f32,
    /// Gap between content children.
    pub gap: f32,
    /// Content cross-align.
    pub cross_align: CrossAlign,
    /// Panel body fill.
    pub background: Option<Color>,
    /// Title bar height.
    pub title_height: f32,
    /// Title bar fill.
    pub title_bg: Color,
    /// Title text color.
    pub title_fg: Color,
    /// Title font size (px).
    pub title_size: f32,
}

impl Default for PanelStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 320.0,
            height: 400.0,
            padding: 10.0,
            gap: 8.0,
            cross_align: CrossAlign::Start,
            background: Some(Color::rgb(0x28, 0x28, 0x2c)),
            title_height: 28.0,
            title_bg: Color::rgb(0x3a, 0x3a, 0x42),
            title_fg: Color::rgb(0xe8, 0xe8, 0xec),
            title_size: 14.0,
        }
    }
}

/// Style for a first-class StatusBar.
#[derive(Debug, Clone, Copy)]
pub struct StatusBarStyle {
    /// Outer x (top-level only).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Width budget.
    pub width: f32,
    /// Bar height.
    pub height: f32,
    /// Inner padding.
    pub padding: f32,
    /// Gap between sections.
    pub gap: f32,
    /// Vertical alignment.
    pub cross_align: CrossAlign,
    /// Bar fill.
    pub background: Option<Color>,
}

impl Default for StatusBarStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 26.0,
            padding: 6.0,
            gap: 12.0,
            cross_align: CrossAlign::Center,
            background: Some(Color::rgb(0x2c, 0x2c, 0x30)),
        }
    }
}

impl StatusBarStyle {
    fn to_row(self) -> RowStyle {
        RowStyle {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            padding: self.padding,
            gap: self.gap,
            cross_align: self.cross_align,
            background: self.background,
        }
    }
}

/// Style for a resizable Splitter.
#[derive(Debug, Clone, Copy)]
pub struct SplitterStyle {
    /// Outer x (top-level only).
    pub x: f32,
    /// Outer y (top-level only).
    pub y: f32,
    /// Outer width.
    pub width: f32,
    /// Outer height.
    pub height: f32,
    /// Split axis: Vertical = left|right; Horizontal = top|bottom.
    pub axis: Axis,
    /// Fraction of space for the first pane (clamped on drag to ~0.1..0.9).
    pub ratio: f32,
    /// Sash thickness in px.
    pub sash_size: f32,
    /// Sash fill.
    pub sash_color: Color,
    /// Optional outer fill behind panes.
    pub background: Option<Color>,
}

impl Default for SplitterStyle {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 480.0,
            axis: Axis::Vertical,
            ratio: 0.3,
            sash_size: 5.0,
            sash_color: Color::rgb(0x4a, 0x4a, 0x52),
            background: None,
        }
    }
}

#[derive(Debug)]
enum Kind {
    Column {
        style: ColumnStyle,
        children: Vec<WidgetId>,
    },
    Row {
        style: RowStyle,
        children: Vec<WidgetId>,
    },
    Toolbar {
        style: ToolbarStyle,
        children: Vec<WidgetId>,
    },
    Panel {
        title: String,
        style: PanelStyle,
        children: Vec<WidgetId>,
    },
    StatusBar {
        style: StatusBarStyle,
        children: Vec<WidgetId>,
    },
    Splitter {
        style: SplitterStyle,
        children: Vec<WidgetId>,
    },
    Text {
        content: String,
        color: Color,
        size: f32,
    },
    Button {
        label: String,
        bg: Color,
        fg: Color,
        size: f32,
        min_width: f32,
        height: f32,
    },
    Icon {
        name: IconName,
        size: f32,
        color: Color,
    },
    Spacer {
        width: f32,
        height: f32,
    },
    Separator {
        thickness: f32,
        length: f32,
        horizontal: bool,
        color: Color,
    },
}

#[derive(Debug)]
struct Widget {
    kind: Kind,
    /// Primary scene node (container / hit target / text mask host).
    node: Option<NodeId>,
    /// Label mask node for buttons / panel titles.
    label_node: Option<NodeId>,
    /// Extra chrome: panel title bar bg, splitter sash.
    chrome_node: Option<NodeId>,
}

#[derive(Debug, Clone)]
struct DragState {
    splitter: WidgetId,
    /// Pointer position along the split axis at press.
    start_pos: f32,
    start_ratio: f32,
    axis: Axis,
    /// Splitter outer bounds at press.
    bounds: Rect,
    sash_size: f32,
}

/// Retained UI tree that mounts into a [`Scene`].
#[derive(Debug)]
pub struct Ui {
    next_id: u64,
    widgets: HashMap<WidgetId, Widget>,
    /// Top-level columns/rows/shell widgets (mount roots).
    roots: Vec<WidgetId>,
    /// scene node → widget (buttons, splitter sashes).
    node_to_widget: HashMap<NodeId, WidgetId>,
    font: Arc<Font>,
    drag: Option<DragState>,
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    /// Empty UI using the default system CJK-capable font.
    pub fn new() -> Self {
        let font = load_default_cjk_font().unwrap_or_else(|e| {
            panic!("next-gui-ui: failed to load default CJK font: {e}");
        });
        Self::with_font(font)
    }

    /// UI with an explicit font face.
    pub fn with_font(font: Font) -> Self {
        Self {
            next_id: 1,
            widgets: HashMap::new(),
            roots: Vec::new(),
            node_to_widget: HashMap::new(),
            font: Arc::new(font),
            drag: None,
        }
    }

    /// Borrow the active font.
    pub fn font(&self) -> &Font {
        &self.font
    }

    fn alloc(&mut self) -> WidgetId {
        let id = WidgetId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Declare a top-level column and build its children.
    pub fn column<F>(&mut self, style: ColumnStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.alloc_container(Kind::Column {
            style,
            children: Vec::new(),
        });
        self.roots.push(id);
        {
            let mut builder = FlexBuilder { ui: self, parent: id };
            build(&mut builder);
        }
        id
    }

    /// Declare a top-level row (e.g. window toolbar).
    pub fn row<F>(&mut self, style: RowStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.alloc_container(Kind::Row {
            style,
            children: Vec::new(),
        });
        self.roots.push(id);
        {
            let mut builder = FlexBuilder { ui: self, parent: id };
            build(&mut builder);
        }
        id
    }

    /// Declare a top-level toolbar.
    pub fn toolbar<F>(&mut self, style: ToolbarStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.alloc_container(Kind::Toolbar {
            style,
            children: Vec::new(),
        });
        self.roots.push(id);
        {
            let mut builder = FlexBuilder { ui: self, parent: id };
            build(&mut builder);
        }
        id
    }

    /// Declare a top-level titled panel.
    pub fn panel<F>(&mut self, title: impl Into<String>, style: PanelStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.alloc_container(Kind::Panel {
            title: title.into(),
            style,
            children: Vec::new(),
        });
        self.roots.push(id);
        {
            let mut builder = FlexBuilder { ui: self, parent: id };
            build(&mut builder);
        }
        id
    }

    /// Declare a top-level status bar.
    pub fn status_bar<F>(&mut self, style: StatusBarStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.alloc_container(Kind::StatusBar {
            style,
            children: Vec::new(),
        });
        self.roots.push(id);
        {
            let mut builder = FlexBuilder { ui: self, parent: id };
            build(&mut builder);
        }
        id
    }

    /// Declare a top-level splitter; each closure should add one pane child (e.g. a panel).
    pub fn splitter<A, B>(&mut self, style: SplitterStyle, build_a: A, build_b: B) -> WidgetId
    where
        A: FnOnce(&mut FlexBuilder<'_>),
        B: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.alloc_container(Kind::Splitter {
            style,
            children: Vec::new(),
        });
        self.roots.push(id);
        {
            let mut builder = FlexBuilder { ui: self, parent: id };
            build_a(&mut builder);
        }
        {
            let mut builder = FlexBuilder { ui: self, parent: id };
            build_b(&mut builder);
        }
        id
    }

    fn alloc_container(&mut self, kind: Kind) -> WidgetId {
        let id = self.alloc();
        self.widgets.insert(
            id,
            Widget {
                kind,
                node: None,
                label_node: None,
                chrome_node: None,
            },
        );
        id
    }

    /// Update text content; call [`Ui::sync`] afterwards.
    pub fn set_text(&mut self, id: WidgetId, content: impl Into<String>) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Text { content: c, .. } = &mut w.kind {
                *c = content.into();
            }
        }
    }

    /// Update a panel title; call [`Ui::sync`] afterwards.
    pub fn set_panel_title(&mut self, id: WidgetId, title: impl Into<String>) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Panel { title: t, .. } = &mut w.kind {
                *t = title.into();
            }
        }
    }

    /// Set splitter ratio (0..1); call [`Ui::sync`] afterwards.
    pub fn set_splitter_ratio(&mut self, id: WidgetId, ratio: f32) {
        if let Some(w) = self.widgets.get_mut(&id) {
            if let Kind::Splitter { style, .. } = &mut w.kind {
                style.ratio = ratio.clamp(0.05, 0.95);
            }
        }
    }

    /// Current splitter ratio, if the widget is a splitter.
    pub fn splitter_ratio(&self, id: WidgetId) -> Option<f32> {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Splitter { style, .. }) => Some(style.ratio),
            _ => None,
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

    /// Whether this id is a splitter.
    pub fn is_splitter(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Splitter { .. })
        )
    }

    /// Map a scene node to a widget (buttons and splitter sashes are registered).
    pub fn widget_from_node(&self, node: NodeId) -> Option<WidgetId> {
        self.node_to_widget.get(&node).copied()
    }

    /// Hit-test widgets using the scene graph.
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

    /// Begin a pointer press; returns true if a splitter sash started dragging.
    pub fn pointer_pressed(&mut self, scene: &Scene, x: f32, y: f32) -> bool {
        let Some(id) = self.hit_widget(scene, x, y) else {
            return false;
        };
        if !self.is_splitter(id) {
            return false;
        }
        let Some(w) = self.widgets.get(&id) else {
            return false;
        };
        let Kind::Splitter { style, .. } = &w.kind else {
            return false;
        };
        let bounds = w
            .node
            .and_then(|n| scene.get(n).map(|n| n.bounds))
            .unwrap_or_else(Rect::empty);
        let start_pos = match style.axis {
            Axis::Vertical => x,
            Axis::Horizontal => y,
        };
        self.drag = Some(DragState {
            splitter: id,
            start_pos,
            start_ratio: style.ratio,
            axis: style.axis,
            bounds,
            sash_size: style.sash_size,
        });
        true
    }

    /// Continue a splitter drag; returns true if ratio changed (caller should sync).
    pub fn pointer_moved(&mut self, x: f32, y: f32) -> bool {
        let Some(drag) = self.drag.clone() else {
            return false;
        };
        let span = match drag.axis {
            Axis::Vertical => (drag.bounds.width - drag.sash_size).max(1.0),
            Axis::Horizontal => (drag.bounds.height - drag.sash_size).max(1.0),
        };
        let cur = match drag.axis {
            Axis::Vertical => x,
            Axis::Horizontal => y,
        };
        let delta = cur - drag.start_pos;
        let new_ratio = (drag.start_ratio + delta / span).clamp(0.1, 0.9);
        if let Some(w) = self.widgets.get_mut(&drag.splitter) {
            if let Kind::Splitter { style, .. } = &mut w.kind {
                if (style.ratio - new_ratio).abs() < 0.0005 {
                    return false;
                }
                style.ratio = new_ratio;
                return true;
            }
        }
        false
    }

    /// End an active splitter drag.
    pub fn pointer_released(&mut self) {
        self.drag = None;
    }

    /// Whether a splitter sash drag is active.
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
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
                Some(Kind::Row { style, .. }) => (
                    style.background.unwrap_or(Color::rgba(0, 0, 0, 0)),
                    style.background.is_some(),
                ),
                Some(Kind::Toolbar { style, .. }) => (
                    style.background.unwrap_or(Color::rgba(0, 0, 0, 0)),
                    style.background.is_some(),
                ),
                Some(Kind::Panel { style, .. }) => (
                    style.background.unwrap_or(Color::rgba(0, 0, 0, 0)),
                    style.background.is_some(),
                ),
                Some(Kind::StatusBar { style, .. }) => (
                    style.background.unwrap_or(Color::rgba(0, 0, 0, 0)),
                    style.background.is_some(),
                ),
                Some(Kind::Splitter { style, .. }) => (
                    style.background.unwrap_or(Color::rgba(0, 0, 0, 0)),
                    style.background.is_some(),
                ),
                Some(Kind::Text { color, .. }) => (*color, true),
                Some(Kind::Button { bg, .. }) => (*bg, true),
                Some(Kind::Icon { color, .. }) => (*color, true),
                Some(Kind::Spacer { .. }) => (Color::rgba(0, 0, 0, 0), false),
                Some(Kind::Separator { color, .. }) => (*color, true),
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

            match self.widgets.get(&id).map(|w| &w.kind) {
                Some(Kind::Button { .. }) => {
                    self.node_to_widget.insert(node, id);
                    let label = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert label node");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.label_node = Some(label);
                    }
                }
                Some(Kind::Panel { style, .. }) => {
                    let title_bg = style.title_bg;
                    let chrome = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), title_bg)
                        .expect("insert panel title bar");
                    let label = scene
                        .insert(chrome, Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgb(255, 255, 255))
                        .expect("insert panel title label");
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.chrome_node = Some(chrome);
                        w.label_node = Some(label);
                    }
                }
                Some(Kind::Splitter { style, .. }) => {
                    let sash = scene
                        .insert(node, Rect::new(0.0, 0.0, 1.0, 1.0), style.sash_color)
                        .expect("insert splitter sash");
                    self.node_to_widget.insert(sash, id);
                    if let Some(w) = self.widgets.get_mut(&id) {
                        w.chrome_node = Some(sash);
                    }
                }
                _ => {}
            }
        }

        let (host, children) = match self.widgets.get(&id) {
            Some(Widget {
                kind:
                    Kind::Column { children, .. }
                    | Kind::Row { children, .. }
                    | Kind::Toolbar { children, .. }
                    | Kind::Panel { children, .. }
                    | Kind::StatusBar { children, .. }
                    | Kind::Splitter { children, .. },
                node,
                ..
            }) => (node.unwrap_or(parent), children.clone()),
            _ => return,
        };
        for child in children {
            self.ensure_mounted(scene, host, child);
        }
    }

    fn measure_widget(&self, id: WidgetId) -> ChildMeasure {
        match self.widgets.get(&id).map(|w| &w.kind) {
            Some(Kind::Text { content, size, .. }) => {
                let (w, h) = self.font.measure_line(content, *size);
                ChildMeasure::new(w, h + 4.0)
            }
            Some(Kind::Button {
                label,
                size,
                min_width,
                height,
                ..
            }) => {
                let (tw, _) = self.font.measure_line(label, *size);
                let pad_x = 16.0;
                ChildMeasure::new((*min_width).max(tw + pad_x * 2.0), *height)
            }
            Some(Kind::Icon { size, .. }) => ChildMeasure::new(*size, *size),
            Some(Kind::Spacer { width, height }) => ChildMeasure::new(*width, *height),
            Some(Kind::Separator {
                thickness,
                length,
                horizontal,
                ..
            }) => {
                if *horizontal {
                    ChildMeasure::new(*length, *thickness)
                } else {
                    ChildMeasure::new(*thickness, *length)
                }
            }
            Some(Kind::Column {
                style, children, ..
            }) => self.measure_flex_axis(
                Axis::Vertical,
                style.width,
                style.height,
                style.padding,
                style.gap,
                style.cross_align,
                children,
            ),
            Some(Kind::Row {
                style, children, ..
            }) => self.measure_flex_axis(
                Axis::Horizontal,
                style.width,
                style.height,
                style.padding,
                style.gap,
                style.cross_align,
                children,
            ),
            Some(Kind::Toolbar {
                style, children, ..
            }) => {
                let m = self.measure_flex_axis(
                    Axis::Horizontal,
                    style.width,
                    style.height,
                    style.padding,
                    style.gap,
                    style.cross_align,
                    children,
                );
                ChildMeasure::new(m.width.max(style.width * 0.25), style.height.max(m.height))
            }
            Some(Kind::StatusBar {
                style, children, ..
            }) => {
                let m = self.measure_flex_axis(
                    Axis::Horizontal,
                    style.width,
                    style.height,
                    style.padding,
                    style.gap,
                    style.cross_align,
                    children,
                );
                ChildMeasure::new(m.width.max(style.width * 0.25), style.height.max(m.height))
            }
            Some(Kind::Panel {
                style, children, ..
            }) => {
                let content = self.measure_flex_axis(
                    Axis::Vertical,
                    (style.width - style.padding * 2.0).max(0.0),
                    (style.height - style.title_height - style.padding * 2.0).max(0.0),
                    0.0,
                    style.gap,
                    style.cross_align,
                    children,
                );
                ChildMeasure::new(
                    style.width.max(content.width + style.padding * 2.0),
                    (style.title_height + content.height + style.padding * 2.0)
                        .max(style.height * 0.5),
                )
            }
            Some(Kind::Splitter { style, .. }) => ChildMeasure::new(style.width, style.height),
            None => ChildMeasure::new(0.0, 0.0),
        }
    }

    fn measure_flex_axis(
        &self,
        axis: Axis,
        max_width: f32,
        max_height: f32,
        padding: f32,
        gap: f32,
        cross_align: CrossAlign,
        children: &[WidgetId],
    ) -> ChildMeasure {
        let measures: Vec<_> = children.iter().map(|c| self.measure_widget(*c)).collect();
        let params = FlexParams {
            axis,
            x: 0.0,
            y: 0.0,
            max_width,
            max_height,
            padding,
            gap,
            cross_align,
        };
        next_gui_layout::measure_flex(&params, &measures)
    }

    fn layout_widget(&mut self, scene: &mut Scene, id: WidgetId) {
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Splitter { .. })
        ) {
            self.layout_splitter(scene, id);
            return;
        }
        if matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(Kind::Panel { .. })
        ) {
            self.layout_panel(scene, id);
            return;
        }

        let flex = match self.widgets.get(&id) {
            Some(Widget {
                kind: Kind::Column {
                    style, children, ..
                },
                node,
                ..
            }) => Some((
                FlexParams {
                    axis: Axis::Vertical,
                    x: style.x,
                    y: style.y,
                    max_width: style.width,
                    max_height: style.height,
                    padding: style.padding,
                    gap: style.gap,
                    cross_align: style.cross_align,
                },
                style.background.is_some(),
                children.clone(),
                *node,
            )),
            Some(Widget {
                kind: Kind::Row {
                    style, children, ..
                },
                node,
                ..
            }) => Some((
                FlexParams {
                    axis: Axis::Horizontal,
                    x: style.x,
                    y: style.y,
                    max_width: style.width,
                    max_height: style.height,
                    padding: style.padding,
                    gap: style.gap,
                    cross_align: style.cross_align,
                },
                style.background.is_some(),
                children.clone(),
                *node,
            )),
            Some(Widget {
                kind: Kind::Toolbar {
                    style, children, ..
                },
                node,
                ..
            }) => {
                let row = style.to_row();
                Some((
                    FlexParams {
                        axis: Axis::Horizontal,
                        x: row.x,
                        y: row.y,
                        max_width: row.width,
                        max_height: row.height,
                        padding: row.padding,
                        gap: row.gap,
                        cross_align: row.cross_align,
                    },
                    row.background.is_some(),
                    children.clone(),
                    *node,
                ))
            }
            Some(Widget {
                kind: Kind::StatusBar {
                    style, children, ..
                },
                node,
                ..
            }) => {
                let row = style.to_row();
                Some((
                    FlexParams {
                        axis: Axis::Horizontal,
                        x: row.x,
                        y: row.y,
                        max_width: row.width,
                        max_height: row.height,
                        padding: row.padding,
                        gap: row.gap,
                        cross_align: row.cross_align,
                    },
                    row.background.is_some(),
                    children.clone(),
                    *node,
                ))
            }
            _ => None,
        };

        let Some((params, has_bg, children, col_node)) = flex else {
            return;
        };

        let measures: Vec<_> = children
            .iter()
            .map(|c| self.measure_widget(*c))
            .collect();
        let total = next_gui_layout::measure_flex(&params, &measures);
        if let Some(node) = col_node {
            let (ox, oy) = scene
                .get(node)
                .map(|n| {
                    if self.roots.contains(&id) {
                        (params.x, params.y)
                    } else {
                        (n.bounds.x, n.bounds.y)
                    }
                })
                .unwrap_or((params.x, params.y));
            let mut placed = params;
            placed.x = ox;
            placed.y = oy;
            let out_w = match self.widgets.get(&id).map(|w| &w.kind) {
                Some(Kind::Toolbar { style, .. }) => style.width.max(total.width),
                Some(Kind::StatusBar { style, .. }) => style.width.max(total.width),
                _ => total.width.max(40.0),
            };
            let out_h = match self.widgets.get(&id).map(|w| &w.kind) {
                Some(Kind::Toolbar { style, .. }) => style.height.max(total.height),
                Some(Kind::StatusBar { style, .. }) => style.height.max(total.height),
                Some(Kind::Row { style, .. }) => style.height.max(total.height),
                _ => total.height.max(24.0),
            };
            placed.max_width = out_w;
            placed.max_height = out_h;
            scene.set_bounds(node, Rect::new(ox, oy, out_w, out_h));
            if has_bg {
                scene.set_visible(node, true);
            }
            let rects = layout_flex(&placed, &measures);
            for (child_id, rect) in children.iter().copied().zip(rects) {
                self.place_child(scene, child_id, rect);
            }
        }
    }

    fn place_child(&mut self, scene: &mut Scene, child_id: WidgetId, rect: Rect) {
        if let Some(w) = self.widgets.get(&child_id) {
            if let Some(node) = w.node {
                scene.set_bounds(node, rect);
                let visible = !matches!(w.kind, Kind::Spacer { .. });
                scene.set_visible(node, visible);
            }
        }
        if self.is_container(child_id) {
            self.rebase_container_origin(child_id, rect.x, rect.y);
            self.rebase_container_size(child_id, rect.width, rect.height);
            self.layout_widget(scene, child_id);
        }
    }

    fn is_container(&self, id: WidgetId) -> bool {
        matches!(
            self.widgets.get(&id).map(|w| &w.kind),
            Some(
                Kind::Column { .. }
                    | Kind::Row { .. }
                    | Kind::Toolbar { .. }
                    | Kind::Panel { .. }
                    | Kind::StatusBar { .. }
                    | Kind::Splitter { .. }
            )
        )
    }

    fn layout_panel(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Panel {
                style, children, ..
            },
            node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let style = *style;
        let children = children.clone();
        let col_node = *node;
        let chrome = *chrome_node;

        let (ox, oy) = if let Some(node) = col_node {
            if self.roots.contains(&id) {
                (style.x, style.y)
            } else {
                scene
                    .get(node)
                    .map(|n| (n.bounds.x, n.bounds.y))
                    .unwrap_or((style.x, style.y))
            }
        } else {
            (style.x, style.y)
        };

        let (w, h) = if let Some(node) = col_node {
            if self.roots.contains(&id) {
                (style.width, style.height)
            } else {
                scene
                    .get(node)
                    .map(|n| {
                        (
                            n.bounds.width.max(style.width * 0.5),
                            n.bounds.height.max(style.height * 0.5),
                        )
                    })
                    .unwrap_or((style.width, style.height))
            }
        } else {
            (style.width, style.height)
        };

        if let Some(node) = col_node {
            scene.set_bounds(node, Rect::new(ox, oy, w, h));
            if style.background.is_some() {
                scene.set_visible(node, true);
            }
        }
        if let Some(chrome) = chrome {
            scene.set_bounds(chrome, Rect::new(ox, oy, w, style.title_height));
            scene.set_fill(chrome, style.title_bg);
            scene.set_visible(chrome, true);
        }

        let content_y = oy + style.title_height;
        let content_h = (h - style.title_height).max(0.0);
        let params = FlexParams {
            axis: Axis::Vertical,
            x: ox,
            y: content_y,
            max_width: w,
            max_height: content_h,
            padding: style.padding,
            gap: style.gap,
            cross_align: style.cross_align,
        };
        let measures: Vec<_> = children.iter().map(|c| self.measure_widget(*c)).collect();
        let rects = layout_flex(&params, &measures);
        for (child_id, rect) in children.iter().copied().zip(rects) {
            self.place_child(scene, child_id, rect);
        }
    }

    fn layout_splitter(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(Widget {
            kind: Kind::Splitter { style, children },
            node,
            chrome_node,
            ..
        }) = self.widgets.get(&id)
        else {
            return;
        };
        let style = *style;
        let children = children.clone();
        let col_node = *node;
        let sash = *chrome_node;

        let (ox, oy) = if let Some(node) = col_node {
            if self.roots.contains(&id) {
                (style.x, style.y)
            } else {
                scene
                    .get(node)
                    .map(|n| (n.bounds.x, n.bounds.y))
                    .unwrap_or((style.x, style.y))
            }
        } else {
            (style.x, style.y)
        };
        let (w, h) = if let Some(node) = col_node {
            if self.roots.contains(&id) {
                (style.width, style.height)
            } else {
                scene
                    .get(node)
                    .map(|n| {
                        (
                            if n.bounds.width > 1.0 {
                                n.bounds.width
                            } else {
                                style.width
                            },
                            if n.bounds.height > 1.0 {
                                n.bounds.height
                            } else {
                                style.height
                            },
                        )
                    })
                    .unwrap_or((style.width, style.height))
            }
        } else {
            (style.width, style.height)
        };

        if let Some(node) = col_node {
            scene.set_bounds(node, Rect::new(ox, oy, w, h));
            if style.background.is_some() {
                scene.set_visible(node, true);
            } else {
                scene.set_visible(node, false);
            }
        }

        let ratio = style.ratio.clamp(0.05, 0.95);
        let sash_size = style.sash_size.max(2.0);

        let (first_rect, sash_rect, second_rect) = match style.axis {
            Axis::Vertical => {
                let avail = (w - sash_size).max(0.0);
                let first_w = (avail * ratio).round();
                let second_w = (avail - first_w).max(0.0);
                (
                    Rect::new(ox, oy, first_w, h),
                    Rect::new(ox + first_w, oy, sash_size, h),
                    Rect::new(ox + first_w + sash_size, oy, second_w, h),
                )
            }
            Axis::Horizontal => {
                let avail = (h - sash_size).max(0.0);
                let first_h = (avail * ratio).round();
                let second_h = (avail - first_h).max(0.0);
                (
                    Rect::new(ox, oy, w, first_h),
                    Rect::new(ox, oy + first_h, w, sash_size),
                    Rect::new(ox, oy + first_h + sash_size, w, second_h),
                )
            }
        };

        if let Some(sash) = sash {
            scene.set_bounds(sash, sash_rect);
            scene.set_fill(sash, style.sash_color);
            scene.set_visible(sash, true);
        }

        if let Some(c0) = children.first().copied() {
            self.place_child(scene, c0, first_rect);
        }
        if let Some(c1) = children.get(1).copied() {
            self.place_child(scene, c1, second_rect);
        }
    }

    fn rebase_container_origin(&mut self, id: WidgetId, x: f32, y: f32) {
        if let Some(w) = self.widgets.get_mut(&id) {
            match &mut w.kind {
                Kind::Column { style, .. } => {
                    style.x = x;
                    style.y = y;
                }
                Kind::Row { style, .. } => {
                    style.x = x;
                    style.y = y;
                }
                Kind::Toolbar { style, .. } => {
                    style.x = x;
                    style.y = y;
                }
                Kind::Panel { style, .. } => {
                    style.x = x;
                    style.y = y;
                }
                Kind::StatusBar { style, .. } => {
                    style.x = x;
                    style.y = y;
                }
                Kind::Splitter { style, .. } => {
                    style.x = x;
                    style.y = y;
                }
                _ => {}
            }
        }
    }

    fn rebase_container_size(&mut self, id: WidgetId, width: f32, height: f32) {
        if let Some(w) = self.widgets.get_mut(&id) {
            match &mut w.kind {
                Kind::Column { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::Row { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::Toolbar { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::Panel { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::StatusBar { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                Kind::Splitter { style, .. } => {
                    style.width = width;
                    style.height = height;
                }
                _ => {}
            }
        }
    }

    fn paint_widget(&mut self, scene: &mut Scene, id: WidgetId) {
        let container_children = match self.widgets.get(&id).map(|w| &w.kind) {
            Some(
                Kind::Column { children, .. }
                | Kind::Row { children, .. }
                | Kind::Toolbar { children, .. }
                | Kind::Panel { children, .. }
                | Kind::StatusBar { children, .. }
                | Kind::Splitter { children, .. },
            ) => Some(children.clone()),
            _ => None,
        };
        if let Some(children) = container_children {
            if matches!(
                self.widgets.get(&id).map(|w| &w.kind),
                Some(Kind::Panel { .. })
            ) {
                self.paint_panel_title(scene, id);
            }
            for child in children {
                self.paint_widget(scene, child);
            }
            return;
        }

        let Some(w) = self.widgets.get(&id) else {
            return;
        };
        let Some(node) = w.node else {
            return;
        };
        let label_node = w.label_node;
        let bounds = scene
            .get(node)
            .map(|n| n.bounds)
            .unwrap_or_else(Rect::empty);

        match &w.kind {
            Kind::Text {
                content,
                color,
                size,
            } => {
                let content = content.clone();
                let color = *color;
                let size = *size;
                let (_, mask) = self.font.rasterize_text(&content, size, None);
                scene.set_fill(node, color);
                scene.set_mask(
                    node,
                    Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                );
                let bw = bounds.width.max(mask.width as f32);
                let bh = bounds.height.max(mask.height as f32);
                scene.set_bounds(node, Rect::new(bounds.x, bounds.y, bw, bh));
            }
            Kind::Button {
                label,
                fg,
                size,
                bg,
                ..
            } => {
                let label = label.clone();
                let fg = *fg;
                let size = *size;
                let bg = *bg;
                scene.set_fill(node, bg);
                scene.set_mask(node, None);
                let (_, mask) = self.font.rasterize_text(&label, size, None);
                if let Some(lid) = label_node {
                    let lx = bounds.x + 16.0;
                    let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
                    scene.set_bounds(
                        lid,
                        Rect::new(lx, ly, mask.width as f32, mask.height as f32),
                    );
                    scene.set_fill(lid, fg);
                    scene.set_mask(
                        lid,
                        Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
                    );
                    scene.set_visible(lid, true);
                }
            }
            Kind::Icon { name, size, color } => {
                let name = *name;
                let size = *size;
                let color = *color;
                let mask = rasterize_icon(name, size);
                scene.set_fill(node, color);
                scene.set_mask(node, Some(mask.clone()));
                scene.set_bounds(
                    node,
                    Rect::new(bounds.x, bounds.y, mask.width as f32, mask.height as f32),
                );
                scene.set_visible(node, true);
            }
            Kind::Separator { color, .. } => {
                scene.set_fill(node, *color);
                scene.set_mask(node, None);
                scene.set_visible(node, true);
            }
            Kind::Spacer { .. }
            | Kind::Column { .. }
            | Kind::Row { .. }
            | Kind::Toolbar { .. }
            | Kind::Panel { .. }
            | Kind::StatusBar { .. }
            | Kind::Splitter { .. } => {}
        }
    }

    fn paint_panel_title(&mut self, scene: &mut Scene, id: WidgetId) {
        let Some(w) = self.widgets.get(&id) else {
            return;
        };
        let Kind::Panel { title, style, .. } = &w.kind else {
            return;
        };
        let title = title.clone();
        let title_fg = style.title_fg;
        let title_size = style.title_size;
        let Some(chrome) = w.chrome_node else {
            return;
        };
        let Some(label) = w.label_node else {
            return;
        };
        let bounds = scene
            .get(chrome)
            .map(|n| n.bounds)
            .unwrap_or_else(Rect::empty);
        let (_, mask) = self.font.rasterize_text(&title, title_size, None);
        let lx = bounds.x + 10.0;
        let ly = bounds.y + ((bounds.height - mask.height as f32) * 0.5).max(0.0);
        scene.set_bounds(
            label,
            Rect::new(lx, ly, mask.width as f32, mask.height as f32),
        );
        scene.set_fill(label, title_fg);
        scene.set_mask(
            label,
            Some(AlphaMask::new(mask.width, mask.height, mask.pixels)),
        );
        scene.set_visible(label, true);
    }
}

/// Compact builder for column/row/shell children.
pub struct FlexBuilder<'a> {
    ui: &'a mut Ui,
    parent: WidgetId,
}

impl FlexBuilder<'_> {
    /// Add a text label; returns its [`WidgetId`].
    pub fn text(&mut self, content: impl Into<String>) -> WidgetId {
        self.text_styled(content, Color::rgb(0xe5, 0xe5, 0xe5), 22.0)
    }

    /// Add a text label with explicit color/size (px).
    pub fn text_styled(
        &mut self,
        content: impl Into<String>,
        color: Color,
        size: f32,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Text {
                    content: content.into(),
                    color,
                    size,
                },
                node: None,
                label_node: None,
                chrome_node: None,
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
                    size: 18.0,
                    min_width: 88.0,
                    height: 36.0,
                },
                node: None,
                label_node: None,
                chrome_node: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Compact toolbar button (smaller height).
    pub fn tool_button(&mut self, label: impl Into<String>) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Button {
                    label: label.into(),
                    bg: Color::rgb(0x3b, 0x82, 0xf6),
                    fg: Color::rgb(0xff, 0xff, 0xff),
                    size: 14.0,
                    min_width: 64.0,
                    height: 28.0,
                },
                node: None,
                label_node: None,
                chrome_node: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Built-in icon glyph.
    pub fn icon(&mut self, name: IconName) -> WidgetId {
        self.icon_styled(name, 18.0, Color::rgb(0xd0, 0xd4, 0xdc))
    }

    /// Icon from a stable name string (`"save"`, …).
    pub fn icon_named(&mut self, name: &str) -> WidgetId {
        self.icon(IconName::from_name(name))
    }

    /// Icon with size and color.
    pub fn icon_styled(&mut self, name: IconName, size: f32, color: Color) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Icon { name, size, color },
                node: None,
                label_node: None,
                chrome_node: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Fixed-size spacer (invisible).
    pub fn spacer(&mut self, width: f32, height: f32) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Spacer { width, height },
                node: None,
                label_node: None,
                chrome_node: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Horizontal separator (thin bar along the main cross of a column).
    pub fn separator(&mut self) -> WidgetId {
        self.separator_styled(1.0, 200.0, true, Color::rgb(0x55, 0x55, 0x5a))
    }

    /// Vertical separator for rows (thin upright bar).
    pub fn separator_vertical(&mut self, length: f32) -> WidgetId {
        self.separator_styled(1.0, length, false, Color::rgb(0x55, 0x55, 0x5a))
    }

    /// Separator with explicit geometry.
    pub fn separator_styled(
        &mut self,
        thickness: f32,
        length: f32,
        horizontal: bool,
        color: Color,
    ) -> WidgetId {
        let id = self.ui.alloc();
        self.ui.widgets.insert(
            id,
            Widget {
                kind: Kind::Separator {
                    thickness,
                    length,
                    horizontal,
                    color,
                },
                node: None,
                label_node: None,
                chrome_node: None,
            },
        );
        self.push_child(id);
        id
    }

    /// Nest a column.
    pub fn column<F>(&mut self, style: ColumnStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc_container(Kind::Column {
            style,
            children: Vec::new(),
        });
        self.push_child(id);
        {
            let mut builder = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut builder);
        }
        id
    }

    /// Nest a row (toolbar strip).
    pub fn row<F>(&mut self, style: RowStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc_container(Kind::Row {
            style,
            children: Vec::new(),
        });
        self.push_child(id);
        {
            let mut builder = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut builder);
        }
        id
    }

    /// Nest a first-class toolbar.
    pub fn toolbar<F>(&mut self, style: ToolbarStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc_container(Kind::Toolbar {
            style,
            children: Vec::new(),
        });
        self.push_child(id);
        {
            let mut builder = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut builder);
        }
        id
    }

    /// Nest a titled panel.
    pub fn panel<F>(&mut self, title: impl Into<String>, style: PanelStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc_container(Kind::Panel {
            title: title.into(),
            style,
            children: Vec::new(),
        });
        self.push_child(id);
        {
            let mut builder = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut builder);
        }
        id
    }

    /// Nest a status bar.
    pub fn status_bar<F>(&mut self, style: StatusBarStyle, build: F) -> WidgetId
    where
        F: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc_container(Kind::StatusBar {
            style,
            children: Vec::new(),
        });
        self.push_child(id);
        {
            let mut builder = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build(&mut builder);
        }
        id
    }

    /// Nest a splitter; each closure should add one pane (panel/column).
    pub fn splitter<A, B>(&mut self, style: SplitterStyle, build_a: A, build_b: B) -> WidgetId
    where
        A: FnOnce(&mut FlexBuilder<'_>),
        B: FnOnce(&mut FlexBuilder<'_>),
    {
        let id = self.ui.alloc_container(Kind::Splitter {
            style,
            children: Vec::new(),
        });
        self.push_child(id);
        {
            let mut builder = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build_a(&mut builder);
        }
        {
            let mut builder = FlexBuilder {
                ui: self.ui,
                parent: id,
            };
            build_b(&mut builder);
        }
        id
    }

    fn push_child(&mut self, child: WidgetId) {
        if let Some(w) = self.ui.widgets.get_mut(&self.parent) {
            match &mut w.kind {
                Kind::Column { children, .. }
                | Kind::Row { children, .. }
                | Kind::Toolbar { children, .. }
                | Kind::Panel { children, .. }
                | Kind::StatusBar { children, .. }
                | Kind::Splitter { children, .. } => {
                    children.push(child);
                }
                _ => {}
            }
        }
    }
}

/// Alias kept for call sites that used the P1 name.
pub type ColumnBuilder<'a> = FlexBuilder<'a>;

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

    #[test]
    fn row_toolbar_lays_out_buttons() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut open = WidgetId(0);
        let mut save = WidgetId(0);
        ui.column(
            ColumnStyle {
                width: 640.0,
                height: 400.0,
                padding: 12.0,
                gap: 10.0,
                ..ColumnStyle::default()
            },
            |col| {
                col.row(
                    RowStyle {
                        width: 600.0,
                        height: 44.0,
                        ..RowStyle::default()
                    },
                    |row| {
                        open = row.button("Open");
                        row.spacer(12.0, 1.0);
                        row.separator_vertical(24.0);
                        row.spacer(12.0, 1.0);
                        save = row.button("Save");
                    },
                );
                col.text("Workspace");
            },
        );
        ui.sync(&mut scene);
        let a = scene
            .get(ui.widgets.get(&open).unwrap().node.unwrap())
            .unwrap()
            .bounds;
        let b = scene
            .get(ui.widgets.get(&save).unwrap().node.unwrap())
            .unwrap()
            .bounds;
        assert!(b.x > a.x + a.width, "Save should sit to the right of Open");
        assert!(ui.is_button(open) && ui.is_button(save));
    }

    #[test]
    fn cjk_text_gets_mask() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut label = WidgetId(0);
        ui.column(ColumnStyle::default(), |col| {
            label = col.text("你好工业 Hello");
        });
        ui.sync(&mut scene);
        let node = ui.widgets.get(&label).unwrap().node.unwrap();
        let n = scene.get(node).unwrap();
        assert!(n.mask.as_ref().is_some_and(|m| !m.is_empty()));
    }

    #[test]
    fn office_shell_panel_toolbar_splitter_statusbar() {
        let mut scene = Scene::new();
        let mut ui = Ui::new();
        let mut splitter = WidgetId(0);
        let mut status = WidgetId(0);
        ui.column(
            ColumnStyle {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 560.0,
                padding: 8.0,
                gap: 6.0,
                background: Some(Color::rgb(0x1a, 0x1a, 0x1c)),
                ..ColumnStyle::default()
            },
            |col| {
                col.toolbar(
                    ToolbarStyle {
                        width: 780.0,
                        ..ToolbarStyle::default()
                    },
                    |tb| {
                        tb.icon(IconName::Menu);
                        tb.spacer(8.0, 1.0);
                        tb.tool_button("Open");
                        tb.icon(IconName::Save);
                        tb.tool_button("Save");
                    },
                );
                splitter = col.splitter(
                    SplitterStyle {
                        width: 780.0,
                        height: 460.0,
                        axis: Axis::Vertical,
                        ratio: 0.28,
                        ..SplitterStyle::default()
                    },
                    |left| {
                        left.panel(
                            "Navigator",
                            PanelStyle {
                                width: 200.0,
                                height: 450.0,
                                ..PanelStyle::default()
                            },
                            |p| {
                                p.text_styled("Plant / Bay", Color::rgb(0xc0, 0xc4, 0xcc), 14.0);
                            },
                        );
                    },
                    |right| {
                        right.panel(
                            "Workspace",
                            PanelStyle {
                                width: 500.0,
                                height: 450.0,
                                ..PanelStyle::default()
                            },
                            |p| {
                                p.text_styled(
                                    "Document body",
                                    Color::rgb(0xc0, 0xc4, 0xcc),
                                    14.0,
                                );
                            },
                        );
                    },
                );
                col.status_bar(
                    StatusBarStyle {
                        width: 780.0,
                        ..StatusBarStyle::default()
                    },
                    |sb| {
                        status = sb.text_styled("Ready", Color::rgb(0x8a, 0xe6, 0x9a), 12.0);
                        sb.spacer(24.0, 1.0);
                        sb.text_styled("Soft", Color::rgb(0xa0, 0xa8, 0xb4), 12.0);
                    },
                );
            },
        );
        ui.sync(&mut scene);
        assert!(ui.is_splitter(splitter));
        assert_eq!(ui.text_content(status), Some("Ready"));

        let sash = ui.widgets.get(&splitter).unwrap().chrome_node.unwrap();
        let sb = scene.get(sash).unwrap().bounds;
        let cx = sb.x + sb.width * 0.5;
        let cy = sb.y + sb.height * 0.5;
        assert!(ui.pointer_pressed(&scene, cx, cy));
        assert!(ui.pointer_moved(cx + 40.0, cy));
        ui.pointer_released();
        ui.sync(&mut scene);
        let ratio = ui.splitter_ratio(splitter).unwrap();
        assert!(
            ratio > 0.28,
            "drag right should grow first pane, got {ratio}"
        );
    }
}
