//! Retained scene graph, dirty regions, and hit-testing.
//!
//! P0: absolute-coordinate rectangle nodes only. No layout engine.

#![deny(missing_docs)]

use std::collections::HashMap;
use std::sync::Arc;

/// Stable node identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(u64);

impl NodeId {
    /// Raw numeric id (for debugging).
    pub fn raw(self) -> u64 {
        self.0
    }
}

/// Axis-aligned rectangle in window/scene coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width (non-negative expected).
    pub width: f32,
    /// Height (non-negative expected).
    pub height: f32,
}

impl Rect {
    /// Create a rectangle.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Empty / invalid for union bootstrap.
    pub fn empty() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    /// Whether the rectangle has positive area.
    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// Point-in-rect test (half-open on right/bottom edges conceptually inclusive on left/top).
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x
            && py >= self.y
            && px < self.x + self.width
            && py < self.y + self.height
    }

    /// Intersection; empty if no overlap.
    pub fn intersect(&self, other: &Rect) -> Rect {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = (self.x + self.width).min(other.x + other.width);
        let y1 = (self.y + self.height).min(other.y + other.height);
        if x1 <= x0 || y1 <= y0 {
            Rect::empty()
        } else {
            Rect::new(x0, y0, x1 - x0, y1 - y0)
        }
    }

    /// Whether two rects overlap with positive area.
    pub fn intersects(&self, other: &Rect) -> bool {
        !self.intersect(other).is_empty()
    }

    /// Axis-aligned bounding union.
    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = (self.x + self.width).max(other.x + other.width);
        let y1 = (self.y + self.height).max(other.y + other.height);
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// Area in square pixels (float).
    pub fn area(&self) -> f32 {
        if self.is_empty() {
            0.0
        } else {
            self.width * self.height
        }
    }

    /// Clamp to integer pixel bounds inside a framebuffer.
    pub fn clip_to_buffer(&self, buf_w: u32, buf_h: u32) -> Option<(u32, u32, u32, u32)> {
        if self.is_empty() || buf_w == 0 || buf_h == 0 {
            return None;
        }
        let x0 = self.x.floor().max(0.0) as i32;
        let y0 = self.y.floor().max(0.0) as i32;
        let x1 = (self.x + self.width).ceil().min(buf_w as f32) as i32;
        let y1 = (self.y + self.height).ceil().min(buf_h as f32) as i32;
        if x1 <= x0 || y1 <= y0 {
            None
        } else {
            Some((x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32))
        }
    }
}

/// 8-bit RGBA color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha.
    pub a: u8,
}

impl Color {
    /// Opaque RGB.
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// RGBA.
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Pack as XRGB8888 (alpha forced opaque for softbuffer present).
    pub fn to_xrgb8888(self) -> u32 {
        // softbuffer expects 0x00RRGGBB on most targets
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }
}


/// 8-bit coverage mask tinted by [`Node::fill`] when encoded.
#[derive(Debug, Clone)]
pub struct AlphaMask {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row-major coverage 0–255.
    pub pixels: Arc<[u8]>,
}

impl AlphaMask {
    /// Create from owned pixel buffer.
    pub fn new(width: u32, height: u32, pixels: impl Into<Arc<[u8]>>) -> Self {
        Self {
            width,
            height,
            pixels: pixels.into(),
        }
    }

    /// Empty mask.
    pub fn empty() -> Self {
        Self {
            width: 0,
            height: 0,
            pixels: Arc::from([]),
        }
    }

    /// No drawable pixels.
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0 || self.pixels.is_empty()
    }
}

/// A retained rectangle node.
#[derive(Debug, Clone)]
pub struct Node {
    /// Id.
    pub id: NodeId,
    /// Parent, if any.
    pub parent: Option<NodeId>,
    /// Children in paint order (later = on top).
    pub children: Vec<NodeId>,
    /// Absolute bounds in window coordinates (P0).
    pub bounds: Rect,
    /// Fill color.
    pub fill: Color,
    /// Visibility.
    pub visible: bool,
    /// Optional alpha mask drawn at `bounds` origin, tinted by `fill`.
    /// When set, the solid `fill` rect is skipped in encode (mask-only paint).
    pub mask: Option<AlphaMask>,
}

/// Accumulated dirty rectangles.
#[derive(Debug, Clone, Default)]
pub struct DirtyRegion {
    /// Dirty rectangles (may overlap before simplify).
    pub rects: Vec<Rect>,
}

impl DirtyRegion {
    /// Empty dirty region.
    pub fn new() -> Self {
        Self { rects: Vec::new() }
    }

    /// No dirty area.
    pub fn is_empty(&self) -> bool {
        self.rects.iter().all(|r| r.is_empty()) || self.rects.is_empty()
    }

    /// Add a rectangle.
    pub fn add(&mut self, r: Rect) {
        if !r.is_empty() {
            self.rects.push(r);
        }
    }

    /// Collapse to a single bounding box (P0 simplify).
    pub fn simplify(&mut self) {
        let mut iter = self.rects.drain(..);
        let Some(first) = iter.next() else {
            return;
        };
        let union = iter.fold(first, |a, b| a.union(&b));
        if !union.is_empty() {
            self.rects.push(union);
        }
    }

    /// Total area after naive sum (overcounts overlaps); useful for logging.
    pub fn approx_area(&self) -> f32 {
        self.rects.iter().map(|r| r.area()).sum()
    }

    /// Whether any dirty rect intersects `r`.
    pub fn intersects_rect(&self, r: &Rect) -> bool {
        self.rects.iter().any(|d| d.intersects(r))
    }
}

/// Retained scene tree.
#[derive(Debug)]
pub struct Scene {
    next_id: u64,
    nodes: HashMap<NodeId, Node>,
    root: NodeId,
    dirty: DirtyRegion,
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene {
    /// Create a scene with an invisible full-window root (0×0 until sized).
    pub fn new() -> Self {
        let root = NodeId(1);
        let mut nodes = HashMap::new();
        nodes.insert(
            root,
            Node {
                id: root,
                parent: None,
                children: Vec::new(),
                bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
                fill: Color::rgba(0, 0, 0, 0),
                visible: false,
                mask: None,
            },
        );
        Self {
            next_id: 2,
            nodes,
            root,
            dirty: DirtyRegion::new(),
        }
    }

    /// Root node id.
    pub fn root(&self) -> NodeId {
        self.root
    }

    fn alloc_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Insert a child rectangle under `parent`.
    pub fn insert(&mut self, parent: NodeId, bounds: Rect, fill: Color) -> Option<NodeId> {
        if !self.nodes.contains_key(&parent) {
            return None;
        }
        let id = self.alloc_id();
        let node = Node {
            id,
            parent: Some(parent),
            children: Vec::new(),
            bounds,
            fill,
            visible: true,
            mask: None,
        };
        self.nodes.insert(id, node);
        if let Some(p) = self.nodes.get_mut(&parent) {
            p.children.push(id);
        }
        self.dirty.add(bounds);
        Some(id)
    }

    /// Set fill color; marks dirty.
    pub fn set_fill(&mut self, id: NodeId, fill: Color) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        if node.fill == fill {
            return true;
        }
        let bounds = node.bounds;
        node.fill = fill;
        self.dirty.add(bounds);
        true
    }

    /// Set bounds; marks old ∪ new dirty.
    pub fn set_bounds(&mut self, id: NodeId, bounds: Rect) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        let old = node.bounds;
        if old == bounds {
            return true;
        }
        node.bounds = bounds;
        self.dirty.add(old);
        self.dirty.add(bounds);
        true
    }

    /// Set visibility; marks dirty.
    pub fn set_visible(&mut self, id: NodeId, visible: bool) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        if node.visible == visible {
            return true;
        }
        let bounds = node.bounds;
        node.visible = visible;
        self.dirty.add(bounds);
        true
    }


    /// Set or clear an alpha mask; marks dirty using current bounds ∪ mask size.
    pub fn set_mask(&mut self, id: NodeId, mask: Option<AlphaMask>) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        let old = node.bounds;
        node.mask = mask;
        let mut dirty_bounds = old;
        if let Some(m) = node.mask.as_ref() {
            if !m.is_empty() {
                dirty_bounds = dirty_bounds.union(&Rect::new(
                    old.x,
                    old.y,
                    m.width as f32,
                    m.height as f32,
                ));
            }
        }
        self.dirty.add(dirty_bounds);
        true
    }

    /// Remove node and descendants; marks dirty.
    pub fn remove(&mut self, id: NodeId) -> bool {
        if id == self.root {
            return false;
        }
        let Some(node) = self.nodes.get(&id).cloned() else {
            return false;
        };
        self.dirty.add(node.bounds);
        let children: Vec<NodeId> = node.children.clone();
        for child in children {
            self.remove(child);
        }
        if let Some(parent) = node.parent {
            if let Some(p) = self.nodes.get_mut(&parent) {
                p.children.retain(|c| *c != id);
            }
        }
        self.nodes.remove(&id);
        true
    }

    /// Take and clear dirty region (simplified to one bbox).
    pub fn take_dirty(&mut self) -> DirtyRegion {
        let mut d = std::mem::take(&mut self.dirty);
        d.simplify();
        d
    }

    /// Mark the entire window dirty.
    pub fn mark_full_dirty(&mut self, width: f32, height: f32) {
        self.dirty = DirtyRegion::new();
        self.dirty.add(Rect::new(0.0, 0.0, width, height));
    }

    /// Peek without clearing (for tests).
    pub fn dirty_peek(&self) -> &DirtyRegion {
        &self.dirty
    }

    /// Hit-test: top-most visible node under (x,y), excluding invisible root fill.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<NodeId> {
        self.hit_test_node(self.root, x, y)
    }

    fn hit_test_node(&self, id: NodeId, x: f32, y: f32) -> Option<NodeId> {
        let node = self.nodes.get(&id)?;
        // Children front-to-back (reverse paint order)
        for child_id in node.children.iter().rev() {
            if let Some(hit) = self.hit_test_node(*child_id, x, y) {
                return Some(hit);
            }
        }
        if id != self.root && node.visible && node.bounds.contains(x, y) {
            Some(id)
        } else {
            None
        }
    }

    /// Borrow a node.
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    /// Paint order: depth-first, parent then children (painter's algorithm).
    pub fn iter_paint_order(&self) -> Vec<&Node> {
        let mut out = Vec::new();
        self.collect_paint(self.root, &mut out);
        out
    }

    fn collect_paint<'a>(&'a self, id: NodeId, out: &mut Vec<&'a Node>) {
        let Some(node) = self.nodes.get(&id) else {
            return;
        };
        if id != self.root {
            out.push(node);
        }
        for child in &node.children {
            self.collect_paint(*child, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_on_fill_change() {
        let mut scene = Scene::new();
        let id = scene
            .insert(scene.root(), Rect::new(10.0, 10.0, 100.0, 40.0), Color::rgb(255, 0, 0))
            .unwrap();
        let _ = scene.take_dirty();
        scene.set_fill(id, Color::rgb(0, 255, 0));
        let dirty = scene.take_dirty();
        assert!(!dirty.is_empty());
        assert!(dirty.approx_area() >= 100.0 * 40.0 - 1.0);
    }

    #[test]
    fn hit_test_topmost() {
        let mut scene = Scene::new();
        let a = scene
            .insert(scene.root(), Rect::new(0.0, 0.0, 100.0, 100.0), Color::rgb(1, 0, 0))
            .unwrap();
        let b = scene
            .insert(scene.root(), Rect::new(50.0, 50.0, 100.0, 100.0), Color::rgb(0, 1, 0))
            .unwrap();
        assert_eq!(scene.hit_test(60.0, 60.0), Some(b));
        assert_eq!(scene.hit_test(10.0, 10.0), Some(a));
        assert_eq!(scene.hit_test(200.0, 200.0), None);
    }
}
