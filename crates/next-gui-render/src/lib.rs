//! Backend-agnostic draw commands and [`Renderer`] trait.

#![deny(missing_docs)]

use std::sync::Arc;

use next_gui_scene::{Color, DirtyRegion, Rect, RgbaImage, Scene};

/// A single draw operation. Render backends must not interpret widget semantics.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    /// Scale a straight-alpha RGBA8 image with nearest-neighbor sampling.
    BlitRgba {
        /// Original destination geometry, retained when clipping to avoid stretching.
        rect: Rect,
        /// Visible destination region in window coordinates.
        clip: Rect,
        /// Shared, validated source pixels.
        image: RgbaImage,
    },
    /// Fill an axis-aligned rectangle at covered pixel centers, with straight-alpha blending.
    FillRect {
        /// Rectangle in scene/window coordinates.
        rect: Rect,
        /// Fill color.
        color: Color,
    },
    /// Fill a rectangle with uniform corner radius and straight-alpha blending.
    /// Radii below 0.5px use sharp edges; otherwise signed-distance coverage spans 1px.
    FillRoundedRect {
        /// Original rectangle in scene/window coordinates, retained when clipped.
        rect: Rect,
        /// Visible destination region in window coordinates.
        clip: Rect,
        /// Fill color.
        color: Color,
        /// Corner radius in px (clamped to half min side).
        radius: f32,
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
/// Honors [`next_gui_scene::Node::clip`] by intersecting fills and cropping alpha blits.
pub fn encode_scene(scene: &Scene, dirty: &DirtyRegion) -> Vec<DrawCommand> {
    if dirty.is_empty() {
        return Vec::new();
    }
    let mut cmds = Vec::new();
    encode_node(scene, scene.root(), dirty, None, &mut cmds);
    cmds
}

fn encode_node(
    scene: &Scene,
    id: next_gui_scene::NodeId,
    dirty: &DirtyRegion,
    parent_clip: Option<Rect>,
    cmds: &mut Vec<DrawCommand>,
) {
    let Some(node) = scene.get(id) else {
        return;
    };
    let clip = match (parent_clip, node.clip) {
        (Some(a), Some(b)) => {
            let i = a.intersect(&b);
            if i.is_empty() {
                return;
            }
            Some(i)
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };

    // Skip root chrome; paint children.
    if id != scene.root() && node.visible {
        if let Some(image) = node.image.as_ref() {
            let rect = node.bounds;
            if [rect.x, rect.y, rect.width, rect.height]
                .iter()
                .all(|v| v.is_finite())
                && !rect.is_empty()
            {
                let clip = clip.map_or(rect, |c| rect.intersect(&c));
                if !clip.is_empty() && dirty.intersects_rect(&clip) {
                    cmds.push(DrawCommand::BlitRgba {
                        rect,
                        clip,
                        image: image.clone(),
                    });
                }
            }
        } else if let Some(mask) = node.mask.as_ref() {
            if !mask.is_empty() {
                let ox = node.bounds.x.floor() as i32;
                let oy = node.bounds.y.floor() as i32;
                let mask_rect = Rect::new(
                    ox as f32,
                    oy as f32,
                    mask.width as f32,
                    mask.height as f32,
                );
                let draw_rect = match clip {
                    Some(c) => mask_rect.intersect(&c),
                    None => mask_rect,
                };
                if !draw_rect.is_empty()
                    && (dirty.intersects_rect(&draw_rect) || dirty.intersects_rect(&node.bounds))
                {
                    if let Some((bx, by, bw, bh, pixels)) =
                        crop_alpha_blit(ox, oy, mask.width, mask.height, &mask.pixels, draw_rect)
                    {
                        cmds.push(DrawCommand::BlitAlpha {
                            x: bx,
                            y: by,
                            width: bw,
                            height: bh,
                            pixels,
                            color: node.fill,
                        });
                    }
                }
            }
        } else if !node.bounds.is_empty() {
            let rect = match clip {
                Some(c) => node.bounds.intersect(&c),
                None => node.bounds,
            };
            if !rect.is_empty() && dirty.intersects_rect(&rect) {
                if node.corner_radius > 0.0 {
                    cmds.push(DrawCommand::FillRoundedRect {
                        rect: node.bounds,
                        clip: rect,
                        color: node.fill,
                        radius: node.corner_radius,
                    });
                } else {
                    cmds.push(DrawCommand::FillRect {
                        rect,
                        color: node.fill,
                    });
                }
            }
        }
    }

    for child in &node.children {
        encode_node(scene, *child, dirty, clip, cmds);
    }
}

/// Cropped alpha blit: x, y, width, height, pixels.
type CroppedAlpha = (i32, i32, u32, u32, Arc<[u8]>);

fn crop_alpha_blit(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pixels: &Arc<[u8]>,
    clip: Rect,
) -> Option<CroppedAlpha> {
    let src = Rect::new(x as f32, y as f32, width as f32, height as f32);
    let area = src.intersect(&clip);
    if area.is_empty() {
        return None;
    }
    let x0 = area.x.floor() as i32;
    let y0 = area.y.floor() as i32;
    let x1 = (area.x + area.width).ceil() as i32;
    let y1 = (area.y + area.height).ceil() as i32;
    let nx0 = x0.max(x);
    let ny0 = y0.max(y);
    let nx1 = x1.min(x + width as i32);
    let ny1 = y1.min(y + height as i32);
    if nx1 <= nx0 || ny1 <= ny0 {
        return None;
    }
    let nw = (nx1 - nx0) as u32;
    let nh = (ny1 - ny0) as u32;
    // Full overlap — reuse buffer.
    if nx0 == x && ny0 == y && nw == width && nh == height {
        return Some((x, y, width, height, Arc::clone(pixels)));
    }
    let mut out = vec![0u8; (nw as usize) * (nh as usize)];
    for row in 0..nh as i32 {
        let src_y = (ny0 - y + row) as u32;
        let src_off = (src_y * width + (nx0 - x) as u32) as usize;
        let dst_off = (row as u32 * nw) as usize;
        let end = src_off + nw as usize;
        if end > pixels.len() {
            return None;
        }
        out[dst_off..dst_off + nw as usize].copy_from_slice(&pixels[src_off..end]);
    }
    Some((nx0, ny0, nw, nh, Arc::from(out)))
}

/// Integer pixels whose centers lie inside a destination and clip.
/// Shared by Soft rasterization and GPU scissors, including fractional clips.
pub fn image_pixel_bounds(
    rect: &Rect,
    clip: &Rect,
    width: u32,
    height: u32,
) -> Option<(u32, u32, u32, u32)> {
    if ![
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        clip.x,
        clip.y,
        clip.width,
        clip.height,
    ]
    .iter()
    .all(|v| v.is_finite())
        || rect.is_empty()
        || clip.is_empty()
    {
        return None;
    }
    let area = rect.intersect(clip);
    let x0 = (area.x - 0.5).ceil().max(0.0).min(width as f32) as u32;
    let y0 = (area.y - 0.5).ceil().max(0.0).min(height as f32) as u32;
    let x1 = (area.x + area.width - 0.5)
        .ceil()
        .max(0.0)
        .min(width as f32) as u32;
    let y1 = (area.y + area.height - 0.5)
        .ceil()
        .max(0.0)
        .min(height as f32) as u32;
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)))
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
    fn rgba_validation_clipping_and_dirty_keep_source_geometry() {
        assert!(RgbaImage::new(0, 1, vec![]).is_none());
        assert!(RgbaImage::new(2, 1, vec![0; 7]).is_none());
        assert!(RgbaImage::new(u32::MAX, u32::MAX, vec![]).is_none());
        let image = RgbaImage::new(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 128]).unwrap();
        let mut scene = Scene::new();
        let parent = scene
            .insert(scene.root(), Rect::empty(), Color::rgb(0, 0, 0))
            .unwrap();
        scene.set_clip(parent, Some(Rect::new(4.0, 2.0, 4.0, 3.0)));
        let rect = Rect::new(0.0, 0.0, 8.0, 8.0);
        let node = scene.insert(parent, rect, Color::rgb(0, 0, 0)).unwrap();
        scene.take_dirty();
        scene.set_image(node, Some(image.clone()));
        let dirty = scene.take_dirty();
        assert!(dirty.intersects_rect(&rect));
        let cmds = encode_scene(&scene, &dirty);
        let [DrawCommand::BlitRgba {
            rect: destination,
            clip,
            image: encoded,
        }] = cmds.as_slice()
        else {
            panic!("expected one image command: {cmds:?}");
        };
        assert_eq!(*destination, rect);
        assert_eq!(*clip, Rect::new(4.0, 2.0, 4.0, 3.0));
        assert!(Arc::ptr_eq(image.pixels(), encoded.pixels()));
        scene.set_visible(node, false);
        let dirty = scene.take_dirty();
        assert!(encode_scene(&scene, &dirty).is_empty());
        assert_eq!(
            image_pixel_bounds(&rect, &Rect::new(1.6, 1.6, 2.8, 2.8), 8, 8),
            Some((2, 2, 2, 2))
        );
        assert!(image_pixel_bounds(&Rect::new(f32::NAN, 0.0, 1.0, 1.0), &rect, 8, 8).is_none());
    }

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
    fn encode_rounded_rect_when_radius_set() {
        let mut scene = Scene::new();
        let id = scene
            .insert(
                scene.root(),
                Rect::new(10.0, 10.0, 40.0, 20.0),
                Color::rgb(255, 0, 0),
            )
            .unwrap();
        scene.set_corner_radius(id, 4.0);
        scene.mark_full_dirty(64.0, 64.0);
        let dirty = scene.take_dirty();
        let cmds = encode_scene(&scene, &dirty);
        assert!(
            cmds.iter().any(|c| matches!(
                c,
                DrawCommand::FillRoundedRect { radius, .. } if (*radius - 4.0).abs() < f32::EPSILON
            )),
            "expected FillRoundedRect, got {cmds:?}"
        );
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
