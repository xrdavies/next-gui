//! Optional **Immediate** viewport — per-frame code-described UI nested in a retained host.
//!
//! This crate is a **bypass** onto [`next_gui_scene::Scene`]: it does **not** replace
//! `next-gui-ui` as the default path. See `docs/p5-design.md` and architecture.md §7.
//!
//! Boundaries (MVP):
//! - **Clip**: draw + hit only inside [`ImmediateViewport::bounds`].
//! - **Focus**: pointer outside does not activate Immediate widgets; Esc / outside blurs.
//! - **Refresh**: apps mark only [`ImmediateViewport::bounds`] dirty for frame ticks;
//!   outer retained UI stays on its own dirty regions.

#![deny(missing_docs)]

use std::sync::Arc;

use next_gui_render::DrawCommand;
use next_gui_scene::{Color, NodeId, Rect, Scene};
use next_gui_text::Font;

/// Stable id for a widget declared inside one Immediate frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImmediateId(pub u32);

impl ImmediateId {
    /// Raw numeric id.
    pub fn raw(self) -> u32 {
        self.0
    }
}

/// Hit region recorded during [`ImmediateCtx`] building.
#[derive(Debug, Clone)]
pub struct HitRegion {
    /// Widget id for this frame.
    pub id: ImmediateId,
    /// Absolute window coordinates (already clipped to the viewport).
    pub rect: Rect,
    /// True when this region is a clickable button.
    pub clickable: bool,
}

/// Style for the retained host chrome (optional scene node).
#[derive(Debug, Clone, Copy)]
pub struct ImmediateViewportStyle {
    /// Host fill behind Immediate content.
    pub background: Color,
    /// Title bar height (0 = no title strip).
    pub title_height: f32,
    /// Title bar fill.
    pub title_bg: Color,
    /// Title text color.
    pub title_fg: Color,
    /// Title font size.
    pub title_size: f32,
}

impl Default for ImmediateViewportStyle {
    fn default() -> Self {
        Self {
            background: Color::rgb(0x24, 0x28, 0x30),
            title_height: 28.0,
            title_bg: Color::rgb(0x2e, 0x36, 0x42),
            title_fg: Color::rgb(0xd0, 0xd8, 0xe4),
            title_size: 14.0,
        }
    }
}

/// Local Immediate viewport nested in a retained window.
#[derive(Debug)]
pub struct ImmediateViewport {
    bounds: Rect,
    style: ImmediateViewportStyle,
    title: String,
    focused: bool,
    enabled: bool,
    host: Option<NodeId>,
    title_node: Option<NodeId>,
    cmds: Vec<DrawCommand>,
    hits: Vec<HitRegion>,
    next_id: u32,
    /// Button ids pressed on the previous pointer-down (same frame set cleared on release).
    pressed: Option<ImmediateId>,
    /// Button that completed a click (press+release inside) since last [`Self::take_clicked`].
    clicked: Option<ImmediateId>,
    frame: u64,
}

impl Default for ImmediateViewport {
    fn default() -> Self {
        Self::new(Rect::empty(), ImmediateViewportStyle::default())
    }
}

impl ImmediateViewport {
    /// Create a viewport with the given bounds and style (disabled until [`Self::set_enabled`]).
    pub fn new(bounds: Rect, style: ImmediateViewportStyle) -> Self {
        Self {
            bounds,
            style,
            title: "Immediate".into(),
            focused: false,
            enabled: true,
            host: None,
            title_node: None,
            cmds: Vec::new(),
            hits: Vec::new(),
            next_id: 1,
            pressed: None,
            clicked: None,
            frame: 0,
        }
    }

    /// Viewport rectangle in window coordinates.
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Update bounds (e.g. on resize). Does not remount; call [`Self::mount`] again if needed.
    pub fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    /// Title shown on the host chrome strip.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Whether the viewport participates in hit-testing / frame rebuild.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.focused = false;
            self.pressed = None;
            self.cmds.clear();
            self.hits.clear();
        }
    }

    /// Field Profile default: off. Demo / Office tool panels enable explicitly.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Input focus is inside this viewport.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus. Outside clicks / Esc should call with `false`.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.pressed = None;
        }
    }

    /// Point inside the viewport rectangle (half-open, same as [`Rect::contains`]).
    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.enabled && !self.bounds.is_empty() && self.bounds.contains(x, y)
    }

    /// Content rectangle below the title strip (where Immediate widgets layout).
    pub fn content_bounds(&self) -> Rect {
        let th = self.style.title_height.max(0.0);
        if th <= 0.0 || self.bounds.height <= th {
            return self.bounds;
        }
        Rect::new(
            self.bounds.x,
            self.bounds.y + th,
            self.bounds.width,
            (self.bounds.height - th).max(0.0),
        )
    }

    /// Ensure a clipped host (+ optional title) under `parent`. Returns the host node.
    pub fn mount(&mut self, scene: &mut Scene, parent: NodeId) -> Option<NodeId> {
        if self.bounds.is_empty() {
            return None;
        }
        let host = match self.host {
            Some(id) if scene.get(id).is_some() => id,
            _ => {
                let id = scene.insert(parent, self.bounds, self.style.background)?;
                self.host = Some(id);
                id
            }
        };
        scene.set_bounds(host, self.bounds);
        scene.set_fill(host, self.style.background);
        scene.set_visible(host, self.enabled);
        scene.set_clip(host, Some(self.bounds));
        scene.raise(host);

        let th = self.style.title_height;
        if th > 0.0 {
            let title_rect = Rect::new(self.bounds.x, self.bounds.y, self.bounds.width, th);
            let tn = match self.title_node {
                Some(id) if scene.get(id).is_some() => id,
                _ => {
                    let id = scene.insert(host, title_rect, self.style.title_bg)?;
                    self.title_node = Some(id);
                    id
                }
            };
            scene.set_bounds(tn, title_rect);
            scene.set_fill(tn, self.style.title_bg);
            scene.set_visible(tn, self.enabled);
            scene.set_clip(tn, Some(self.bounds));
        }
        Some(host)
    }

    /// Host scene node, if mounted.
    pub fn host_node(&self) -> Option<NodeId> {
        self.host
    }

    /// Begin a new Immediate frame. Clears prior draw list / hits for this viewport.
    pub fn begin_frame<'a>(&'a mut self, font: &'a Font) -> ImmediateCtx<'a> {
        self.frame = self.frame.wrapping_add(1);
        self.cmds.clear();
        self.hits.clear();
        self.next_id = 1;

        // Host chrome is painted via scene; Immediate draws content + title text.
        let content = self.content_bounds();
        ImmediateCtx {
            viewport: self,
            font,
            cursor_x: content.x + 10.0,
            cursor_y: content.y + 10.0,
            content,
        }
    }

    /// Draw commands produced by the last [`Self::begin_frame`] / builder.
    pub fn draw_commands(&self) -> &[DrawCommand] {
        &self.cmds
    }

    /// Hit regions for the last frame (clipped).
    pub fn hit_regions(&self) -> &[HitRegion] {
        &self.hits
    }

    /// Hit-test within the viewport only. `None` when disabled, outside, or miss.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<ImmediateId> {
        if !self.contains(x, y) {
            return None;
        }
        for h in self.hits.iter().rev() {
            if h.rect.contains(x, y) {
                return Some(h.id);
            }
        }
        // Click on empty viewport chrome still counts as "inside" for focus, but no widget.
        None
    }

    /// Pointer press. Returns true if the event was consumed (point inside viewport).
    pub fn pointer_pressed(&mut self, x: f32, y: f32) -> bool {
        if !self.contains(x, y) {
            self.set_focused(false);
            return false;
        }
        self.focused = true;
        self.pressed = None;
        if let Some(id) = self.hit_test(x, y) {
            if self
                .hits
                .iter()
                .any(|h| h.id == id && h.clickable && h.rect.contains(x, y))
            {
                self.pressed = Some(id);
            }
        }
        true
    }

    /// Pointer release. May record a click via [`Self::take_clicked`].
    pub fn pointer_released(&mut self, x: f32, y: f32) -> bool {
        if !self.enabled {
            return false;
        }
        let inside = self.contains(x, y);
        if let Some(pid) = self.pressed.take() {
            if inside {
                if let Some(id) = self.hit_test(x, y) {
                    if id == pid {
                        self.clicked = Some(id);
                    }
                }
            }
            return true;
        }
        inside
    }

    /// Take the button that completed a click since the last call.
    pub fn take_clicked(&mut self) -> Option<ImmediateId> {
        self.clicked.take()
    }

    /// Blur on Escape when focused. Returns true if focus was cleared (event consumed).
    pub fn handle_escape(&mut self) -> bool {
        if self.focused {
            self.set_focused(false);
            true
        } else {
            false
        }
    }

    /// Current frame counter (increments each [`Self::begin_frame`]).
    pub fn frame_index(&self) -> u64 {
        self.frame
    }
}

/// Ephemeral builder for one Immediate frame.
pub struct ImmediateCtx<'a> {
    viewport: &'a mut ImmediateViewport,
    font: &'a Font,
    cursor_x: f32,
    cursor_y: f32,
    content: Rect,
}

impl ImmediateCtx<'_> {
    fn alloc_id(&mut self) -> ImmediateId {
        let id = ImmediateId(self.viewport.next_id);
        self.viewport.next_id = self.viewport.next_id.wrapping_add(1).max(1);
        id
    }

    fn clip_rect(&self, r: Rect) -> Rect {
        r.intersect(&self.viewport.bounds)
    }

    fn push_fill(&mut self, rect: Rect, color: Color) {
        let rect = self.clip_rect(rect);
        if !rect.is_empty() {
            self.viewport
                .cmds
                .push(DrawCommand::FillRect { rect, color });
        }
    }

    fn push_text(&mut self, x: f32, y: f32, text: &str, px: f32, color: Color) {
        if text.is_empty() {
            return;
        }
        let (layout, mask) = self.font.rasterize_text(text, px, None);
        let _ = layout;
        if mask.is_empty() {
            return;
        }
        let ox = x.floor() as i32;
        let oy = y.floor() as i32;
        let mask_rect = Rect::new(ox as f32, oy as f32, mask.width as f32, mask.height as f32);
        let draw = self.clip_rect(mask_rect);
        if draw.is_empty() {
            return;
        }
        // Crop mask to clip (same approach as encode_scene).
        if let Some((bx, by, bw, bh, pixels)) =
            crop_alpha(ox, oy, mask.width, mask.height, &mask.pixels, draw)
        {
            self.viewport.cmds.push(DrawCommand::BlitAlpha {
                x: bx,
                y: by,
                width: bw,
                height: bh,
                pixels,
                color,
            });
        }
    }

    /// Draw the title string into the title strip (call once per frame if desired).
    pub fn paint_title(&mut self) {
        let th = self.viewport.style.title_height;
        if th <= 0.0 {
            return;
        }
        let style = self.viewport.style;
        let title = self.viewport.title.clone();
        let x = self.viewport.bounds.x + 10.0;
        let y = self.viewport.bounds.y + ((th - style.title_size) * 0.5).max(2.0);
        self.push_text(x, y, &title, style.title_size, style.title_fg);
        // Focus indicator
        if self.viewport.focused {
            let bar = Rect::new(
                self.viewport.bounds.x,
                self.viewport.bounds.y,
                3.0,
                th,
            );
            self.push_fill(bar, Color::rgb(0x4f, 0xc3, 0xf7));
        }
    }

    /// Filled rectangle in absolute window coordinates (clipped).
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.push_fill(rect, color);
    }

    /// Text at absolute position (clipped).
    pub fn text_at(&mut self, x: f32, y: f32, text: impl AsRef<str>, px: f32, color: Color) {
        self.push_text(x, y, text.as_ref(), px, color);
    }

    /// Flowing label under the automatic cursor; advances cursor.
    pub fn label(&mut self, text: impl AsRef<str>, px: f32, color: Color) -> ImmediateId {
        let id = self.alloc_id();
        let text = text.as_ref();
        let (tw, th) = self.font.measure_line(text, px);
        let rect = Rect::new(self.cursor_x, self.cursor_y, tw, th.max(px));
        self.push_text(self.cursor_x, self.cursor_y, text, px, color);
        self.viewport.hits.push(HitRegion {
            id,
            rect: self.clip_rect(rect),
            clickable: false,
        });
        self.cursor_y += th.max(px) + 6.0;
        id
    }

    /// Clickable button under the cursor; returns id for [`ImmediateViewport::take_clicked`].
    pub fn button(&mut self, label: impl AsRef<str>) -> ImmediateId {
        let id = self.alloc_id();
        let label = label.as_ref();
        let px = 14.0;
        let (tw, th) = self.font.measure_line(label, px);
        let pad_x = 12.0;
        let pad_y = 6.0;
        let bw = (tw + pad_x * 2.0).max(72.0);
        let bh = (th + pad_y * 2.0).max(28.0);
        let rect = Rect::new(self.cursor_x, self.cursor_y, bw, bh);
        let hot = self.viewport.pressed == Some(id);
        let bg = if hot {
            Color::rgb(0x3d, 0x7e, 0xc0)
        } else {
            Color::rgb(0x3a, 0x5f, 0x8a)
        };
        self.push_fill(rect, bg);
        let tx = self.cursor_x + pad_x;
        let ty = self.cursor_y + pad_y;
        self.push_text(tx, ty, label, px, Color::rgb(0xf0, 0xf4, 0xfa));
        self.viewport.hits.push(HitRegion {
            id,
            rect: self.clip_rect(rect),
            clickable: true,
        });
        self.cursor_y += bh + 8.0;
        id
    }

    /// Simple sparkline from samples (oldest → newest), absolute rect.
    pub fn sparkline(&mut self, rect: Rect, samples: &[f32], line: Color, fill: Color) {
        let rect = self.clip_rect(rect);
        if rect.is_empty() || samples.len() < 2 {
            self.push_fill(rect, fill);
            return;
        }
        self.push_fill(rect, fill);
        let min = samples.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = samples.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let span = (max - min).abs().max(1e-3);
        let n = samples.len();
        let mut prev: Option<(f32, f32)> = None;
        for (i, &v) in samples.iter().enumerate() {
            let t = i as f32 / (n - 1) as f32;
            let x = rect.x + t * rect.width;
            let y = rect.y + rect.height - ((v - min) / span) * rect.height;
            if let Some((px, py)) = prev {
                // Approximate line with thin rects
                let dx = x - px;
                let dy = y - py;
                let steps = ((dx.abs() + dy.abs()) as usize).max(1);
                for s in 0..=steps {
                    let u = s as f32 / steps as f32;
                    let lx = px + dx * u;
                    let ly = py + dy * u;
                    self.push_fill(Rect::new(lx - 0.5, ly - 0.5, 2.0, 2.0), line);
                }
            }
            prev = Some((x, y));
        }
    }

    /// Advance vertical cursor by `dy` pixels.
    pub fn spacer(&mut self, dy: f32) {
        self.cursor_y += dy;
    }

    /// Content cursor origin (top-left of flowing layout).
    pub fn cursor(&self) -> (f32, f32) {
        (self.cursor_x, self.cursor_y)
    }

    /// Set flowing cursor.
    pub fn set_cursor(&mut self, x: f32, y: f32) {
        self.cursor_x = x;
        self.cursor_y = y;
    }

    /// Content rect for this frame.
    pub fn content_bounds(&self) -> Rect {
        self.content
    }
}


/// Cropped alpha blit: x, y, width, height, pixels.
type CroppedAlpha = (i32, i32, u32, u32, Arc<[u8]>);
fn crop_alpha(
    ox: i32,
    oy: i32,
    mw: u32,
    mh: u32,
    pixels: &[u8],
    clip: Rect,
) -> Option<CroppedAlpha> {
    let area = Rect::new(ox as f32, oy as f32, mw as f32, mh as f32).intersect(&clip);
    if area.is_empty() {
        return None;
    }
    let x0 = (area.x.floor() as i32).max(ox);
    let y0 = (area.y.floor() as i32).max(oy);
    let x1 = ((area.x + area.width).ceil() as i32).min(ox + mw as i32);
    let y1 = ((area.y + area.height).ceil() as i32).min(oy + mh as i32);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let bw = (x1 - x0) as u32;
    let bh = (y1 - y0) as u32;
    let mut out = vec![0u8; (bw * bh) as usize];
    for row in 0..bh {
        let src_y = (y0 - oy) as u32 + row;
        let src_off = (src_y * mw + (x0 - ox) as u32) as usize;
        let dst_off = (row * bw) as usize;
        out[dst_off..dst_off + bw as usize]
            .copy_from_slice(&pixels[src_off..src_off + bw as usize]);
    }
    Some((x0, y0, bw, bh, Arc::from(out)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use next_gui_text::load_default_cjk_font;

    fn font() -> Font {
        load_default_cjk_font().expect("font")
    }

    #[test]
    fn contains_and_clip_hit() {
        let bounds = Rect::new(100.0, 50.0, 200.0, 300.0);
        let mut vp = ImmediateViewport::new(bounds, ImmediateViewportStyle::default());
        assert!(vp.contains(150.0, 100.0));
        assert!(!vp.contains(50.0, 100.0));
        assert!(!vp.contains(100.0 + 200.0, 100.0));

        let font = font();
        {
            let mut ctx = vp.begin_frame(&font);
            // Intentionally request a rect that extends past the right edge.
            ctx.fill_rect(Rect::new(280.0, 80.0, 80.0, 40.0), Color::rgb(255, 0, 0));
            ctx.button("Go");
        }
        for cmd in vp.draw_commands() {
            if let DrawCommand::FillRect { rect, .. } = cmd {
                assert!(
                    rect.x >= bounds.x - 0.01,
                    "draw leaked left: {rect:?} vs {bounds:?}"
                );
                assert!(
                    rect.x + rect.width <= bounds.x + bounds.width + 0.01,
                    "draw leaked right: {rect:?}"
                );
            }
        }
        for h in vp.hit_regions() {
            assert!(bounds.contains(h.rect.x, h.rect.y) || h.rect.intersects(&bounds));
            let clipped = h.rect.intersect(&bounds);
            assert_eq!(h.rect, clipped, "hit region must be clipped");
        }
    }

    #[test]
    fn outside_pointer_does_not_focus_or_click() {
        let mut vp =
            ImmediateViewport::new(Rect::new(400.0, 0.0, 300.0, 400.0), Default::default());
        let font = font();
        let btn;
        {
            let mut ctx = vp.begin_frame(&font);
            ctx.paint_title();
            btn = ctx.button("Tick");
        }
        let hit = vp
            .hit_regions()
            .iter()
            .find(|h| h.id == btn && h.clickable)
            .expect("button hit region");
        let cx = hit.rect.x + hit.rect.width * 0.5;
        let cy = hit.rect.y + hit.rect.height * 0.5;

        assert!(!vp.pointer_pressed(10.0, 10.0));
        assert!(!vp.is_focused());
        assert!(vp.pointer_pressed(cx, cy));
        assert!(vp.is_focused());
        vp.pointer_released(cx, cy);
        assert_eq!(vp.take_clicked(), Some(btn));

        // Press inside, release outside → no click.
        {
            let mut ctx = vp.begin_frame(&font);
            let btn2 = ctx.button("Tick");
            let _ = btn2;
        }
        let hit = vp.hit_regions().iter().find(|h| h.clickable).unwrap();
        let cx = hit.rect.x + hit.rect.width * 0.5;
        let cy = hit.rect.y + hit.rect.height * 0.5;
        assert!(vp.pointer_pressed(cx, cy));
        vp.pointer_released(10.0, 10.0);
        assert!(vp.take_clicked().is_none());
    }

    #[test]
    fn escape_clears_focus_only_when_focused() {
        let mut vp =
            ImmediateViewport::new(Rect::new(0.0, 0.0, 100.0, 100.0), Default::default());
        assert!(!vp.handle_escape());
        vp.set_focused(true);
        assert!(vp.handle_escape());
        assert!(!vp.is_focused());
        assert!(!vp.handle_escape());
    }

    #[test]
    fn disabled_viewport_ignores_hits() {
        let mut vp =
            ImmediateViewport::new(Rect::new(0.0, 0.0, 200.0, 200.0), Default::default());
        vp.set_enabled(false);
        assert!(!vp.contains(50.0, 50.0));
        assert!(!vp.pointer_pressed(50.0, 50.0));
    }

    #[test]
    fn mount_sets_clip_on_host() {
        let mut scene = Scene::new();
        let mut vp =
            ImmediateViewport::new(Rect::new(10.0, 20.0, 100.0, 80.0), Default::default());
        let root = scene.root();
        let host = vp.mount(&mut scene, root).expect("host");
        let node = scene.get(host).expect("node");
        assert_eq!(node.clip, Some(vp.bounds()));
        assert_eq!(node.bounds, vp.bounds());
    }

    #[test]
    fn mark_dirty_is_viewport_local() {
        let mut scene = Scene::new();
        scene.mark_dirty(Rect::new(50.0, 50.0, 40.0, 40.0));
        let d = scene.dirty_peek();
        assert!(!d.is_empty());
        assert!(d.intersects_rect(&Rect::new(50.0, 50.0, 40.0, 40.0)));
        assert!(!d.intersects_rect(&Rect::new(0.0, 0.0, 10.0, 10.0)));
    }
}
