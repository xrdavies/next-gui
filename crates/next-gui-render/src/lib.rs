//! Backend-agnostic draw commands and [`Renderer`] trait.

#![deny(missing_docs)]

use std::sync::Arc;

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
    /// Tint an 8-bit coverage mask with `color` at `(x, y)`.
    BlitAlpha {
        /// Left edge in window coordinates.
        x: i32,
        /// Top edge in window coordinates.
        y: i32,
        /// Mask width.
        width: u32,
        /// Mask height.
        height: u32,
        /// Row-major coverage 0–255.
        pixels: Arc<[u8]>,
        /// Tint color (RGB used; alpha scales coverage).
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
        if !node.visible {
            continue;
        }
        if let Some(mask) = node.mask.as_ref() {
            if mask.is_empty() {
                continue;
            }
            let mask_rect = Rect::new(
                node.bounds.x,
                node.bounds.y,
                mask.width as f32,
                mask.height as f32,
            );
            if !dirty.intersects_rect(&mask_rect) && !dirty.intersects_rect(&node.bounds) {
                continue;
            }
            cmds.push(DrawCommand::BlitAlpha {
                x: node.bounds.x.floor() as i32,
                y: node.bounds.y.floor() as i32,
                width: mask.width,
                height: mask.height,
                pixels: Arc::clone(&mask.pixels),
                color: node.fill,
            });
            continue;
        }
        if node.bounds.is_empty() {
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
    use next_gui_scene::{AlphaMask, Color};

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
            _ => panic!("expected FillRect"),
        }
    }

    #[test]
    fn encode_mask_node() {
        let mut scene = Scene::new();
        let id = scene
            .insert(
                scene.root(),
                Rect::new(5.0, 5.0, 4.0, 2.0),
                Color::rgb(255, 255, 255),
            )
            .unwrap();
        let pixels: Arc<[u8]> = Arc::from([255u8, 128, 64, 32, 0, 255, 128, 64]);
        scene.set_mask(id, Some(AlphaMask::new(4, 2, pixels)));
        scene.mark_full_dirty(64.0, 64.0);
        let dirty = scene.take_dirty();
        let cmds = encode_scene(&scene, &dirty);
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            DrawCommand::BlitAlpha {
                x, y, width, height, ..
            } => {
                assert_eq!((*x, *y, *width, *height), (5, 5, 4, 2));
            }
            _ => panic!("expected BlitAlpha"),
        }
    }
}
