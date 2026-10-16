//! Tiny 5×7 bitmap font (ASCII subset) for P1 Text/Button labels.
//! Replaced by the `text` module in P2.

use next_gui_scene::{Color, NodeId, Rect, Scene};

const GLYPH_W: usize = 5;
const GLYPH_H: usize = 7;
const CELL_W: f32 = 6.0; // glyph + 1px gap
const CELL_H: f32 = 8.0;

/// Measure text in pixels (single line).
pub fn measure(text: &str, scale: f32) -> (f32, f32) {
    let n = text.chars().count() as f32;
    (n * CELL_W * scale, CELL_H * scale)
}

fn glyph_bits(ch: char) -> Option<[u8; GLYPH_H]> {
    // Each row: lower 5 bits, bit4 = leftmost.
    Some(match ch {
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        ':' => [0, 0b00100, 0, 0b00100, 0, 0b00100, 0],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        'A' | 'a' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' | 'b' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' | 'c' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' | 'd' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' | 'e' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' | 'f' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' | 'g' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
        'H' | 'h' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' | 'i' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'L' | 'l' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' | 'm' => [0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0b10001, 0b10001],
        'N' | 'n' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' | 'o' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' | 'p' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'R' | 'r' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' | 's' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' | 't' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' | 'u' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' | 'v' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'Y' | 'y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '+' => [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
        _ => return None,
    })
}

/// Remove previous glyph nodes and paint `text` as child rects under `parent`.
pub fn sync_text(
    scene: &mut Scene,
    parent: NodeId,
    text: &str,
    origin: Rect,
    color: Color,
    scale: f32,
    glyph_nodes: &mut Vec<NodeId>,
) {
    for id in glyph_nodes.drain(..) {
        scene.remove(id);
    }
    let px = scale.max(1.0);
    let mut cx = origin.x;
    let cy = origin.y;
    for ch in text.chars() {
        let bits = glyph_bits(ch).unwrap_or([0; GLYPH_H]);
        for (row, row_bits) in bits.iter().enumerate() {
            for col in 0..GLYPH_W {
                if (row_bits >> (GLYPH_W - 1 - col)) & 1 == 1 {
                    let r = Rect::new(
                        cx + col as f32 * px,
                        cy + row as f32 * px,
                        px,
                        px,
                    );
                    if let Some(id) = scene.insert(parent, r, color) {
                        glyph_nodes.push(id);
                    }
                }
            }
        }
        cx += CELL_W * px;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_text_keeps_glyph_columns_in_reading_order() {
        let mut scene = Scene::new();
        let parent = scene
            .insert(
                scene.root(),
                Rect::new(0.0, 0.0, 5.0, 7.0),
                Color::rgb(0, 0, 0),
            )
            .expect("parent");
        let mut glyphs = Vec::new();

        sync_text(
            &mut scene,
            parent,
            "L",
            Rect::new(0.0, 0.0, 5.0, 7.0),
            Color::rgb(255, 255, 255),
            1.0,
            &mut glyphs,
        );

        let bounds: Vec<_> = scene
            .iter_paint_order()
            .into_iter()
            .skip(1)
            .map(|node| node.bounds)
            .collect();
        assert!(bounds.contains(&Rect::new(0.0, 0.0, 1.0, 1.0)));
        assert!(!bounds.contains(&Rect::new(4.0, 0.0, 1.0, 1.0)));
        assert!(bounds.contains(&Rect::new(4.0, 6.0, 1.0, 1.0)));
    }
}
