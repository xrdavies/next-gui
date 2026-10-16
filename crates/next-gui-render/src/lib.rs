//! Backend-agnostic draw commands and [`Renderer`] trait.

#![deny(missing_docs)]

use next_gui_scene::{Color, DirtyRegion, Rect, Scene};

/// A single draw operation. Render backends must not interpret widget semantics.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    /// Fill an axis-aligned rectangle.
    FillRect {
        /// Rectangle in scene/window coordinates.
        rect: Rect,
        /// Fill color.
        color: Color,
    },
}

/// Encode visible scene nodes that intersect `dirty` into draw commands (paint order).
pub fn encode_scene(scene: &Scene, dirty: &DirtyRegion) -> Vec<DrawCommand> {
    if dirty.is_empty() {
        return Vec::new();
    }
    let mut cmds = Vec::new();
    for node in scene.iter_paint_order() {
        if !node.visible || node.bounds.is_empty() {
            continue;
        }
        if !dirty.intersects_rect(&node.bounds) {
            continue;
        }
        cmds.push(DrawCommand::FillRect {
            rect: node.bounds,
            color: node.fill,
        });
    }
    cmds
}

/// Raster / GPU backend interface.
pub trait Renderer {
    /// Resize the backing store; typically marks full clear needed by caller.
    fn resize(&mut self, width: u32, height: u32);

    /// Rasterize `commands` clipped to `dirty` into the backing store.
    fn render(&mut self, commands: &[DrawCommand], dirty: &DirtyRegion);

    /// Packed XRGB8888 pixels, width, height — if the backend keeps a CPU buffer.
    fn frame_pixels(&self) -> Option<(&[u32], u32, u32)>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use next_gui_scene::Color;

    #[test]
    fn encode_skips_clean_nodes() {
        let mut scene = Scene::new();
        let _a = scene
            .insert(
                scene.root(),
                Rect::new(0.0, 0.0, 10.0, 10.0),
                Color::rgb(255, 0, 0),
            )
            .unwrap();
        let b = scene
            .insert(
                scene.root(),
                Rect::new(100.0, 100.0, 10.0, 10.0),
                Color::rgb(0, 255, 0),
            )
            .unwrap();
        let _ = scene.take_dirty();
        scene.set_fill(b, Color::rgb(0, 0, 255));
        let dirty = scene.take_dirty();
        let cmds = encode_scene(&scene, &dirty);
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            DrawCommand::FillRect { rect, .. } => {
                assert_eq!(*rect, Rect::new(100.0, 100.0, 10.0, 10.0));
            }
        }
    }
}
