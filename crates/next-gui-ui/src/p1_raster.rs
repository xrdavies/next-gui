//! Soft raster helpers for Office Gauge / Trend / Chart (AlphaMask + cheap AA).

use std::sync::Arc;

use next_gui_scene::AlphaMask;

/// Rasterize a simple dial gauge into an alpha mask (arc track/fill + needle).
///
/// Sweep is 270° clockwise from 135° (lower-left) through the top to 45° (lower-right),
/// matching a typical HMI dial. Angles are measured clockwise from +X in screen space
/// (Y grows downward).
pub(crate) fn rasterize_gauge(
    size: u32,
    value: f32,
    min: f32,
    max: f32,
    track_coverage: u8,
    fill_coverage: u8,
    needle_coverage: u8,
) -> AlphaMask {
    let size = size.max(16);
    let mut px = vec![0u8; (size * size) as usize];
    let cx = (size as f32 - 1.0) * 0.5;
    let cy = (size as f32 - 1.0) * 0.5;
    let outer = size as f32 * 0.46;
    let inner = outer - size as f32 * 0.10;
    let start_deg = 135.0_f32;
    let sweep_deg = 270.0_f32;
    let t = if (max - min).abs() < 1e-9 {
        0.0
    } else {
        ((value - min) / (max - min)).clamp(0.0, 1.0)
    };

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let r = (dx * dx + dy * dy).sqrt();
            if r < inner - 1.0 || r > outer + 1.0 {
                continue;
            }
            // Clockwise degrees from +X (screen Y-down): atan2(y, x) then remap.
            let mut deg = dy.atan2(dx).to_degrees();
            if deg < 0.0 {
                deg += 360.0;
            }
            let mut rel = deg - start_deg;
            if rel < 0.0 {
                rel += 360.0;
            }
            if rel > sweep_deg {
                continue;
            }
            let edge = if r < inner {
                (1.0 - (inner - r)).clamp(0.0, 1.0)
            } else if r > outer {
                (1.0 - (r - outer)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            if edge <= 0.0 {
                continue;
            }
            let filled = rel <= sweep_deg * t;
            let base = if filled {
                fill_coverage
            } else {
                track_coverage
            };
            let cov = (f32::from(base) * edge) as u8;
            let idx = (y * size + x) as usize;
            px[idx] = px[idx].max(cov);
        }
    }

    let needle_deg = start_deg + sweep_deg * t;
    let rad = needle_deg.to_radians();
    let nx = rad.cos();
    let ny = rad.sin();
    let needle_len = outer * 0.92;
    let steps = (needle_len as i32).max(8);
    for i in 0..=steps {
        let f = i as f32 / steps as f32;
        stamp_disc(&mut px, size, size, cx + nx * needle_len * f, cy + ny * needle_len * f, 1.25, needle_coverage);
    }
    stamp_disc(&mut px, size, size, cx, cy, size as f32 * 0.045, needle_coverage);

    AlphaMask::new(size, size, Arc::<[u8]>::from(px))
}

fn stamp_disc(px: &mut [u8], w: u32, h: u32, cx: f32, cy: f32, radius: f32, cov: u8) {
    // Soft edge: coverage falls off across ~1px beyond the hard radius (cheap AA).
    let soft = radius + 1.0;
    let r = soft.ceil() as i32 + 1;
    let ix = cx as i32;
    let iy = cy as i32;
    for dy in -r..=r {
        for dx in -r..=r {
            let x = ix + dx;
            let y = iy + dy;
            if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
                continue;
            }
            let fx = x as f32 + 0.5 - cx;
            let fy = y as f32 + 0.5 - cy;
            let d = (fx * fx + fy * fy).sqrt();
            if d > soft {
                continue;
            }
            let edge = if d <= radius {
                1.0
            } else {
                (1.0 - (d - radius) / (soft - radius).max(1e-3)).clamp(0.0, 1.0)
            };
            let c = (f32::from(cov) * edge) as u8;
            let idx = (y as u32 * w + x as u32) as usize;
            px[idx] = px[idx].max(c);
        }
    }
}

/// Rasterize a sparkline / trend polyline into an alpha mask.
pub(crate) fn rasterize_trend(
    width: u32,
    height: u32,
    points: &[f32],
    line_coverage: u8,
    fill_coverage: u8,
) -> AlphaMask {
    let width = width.max(8);
    let height = height.max(8);
    let mut px = vec![0u8; (width * height) as usize];
    if points.len() < 2 {
        return AlphaMask::new(width, height, Arc::<[u8]>::from(px));
    }
    let mut vmin = points[0];
    let mut vmax = points[0];
    for &p in points {
        vmin = vmin.min(p);
        vmax = vmax.max(p);
    }
    if (vmax - vmin).abs() < 1e-6 {
        vmax = vmin + 1.0;
    }
    let pad = 2.0;
    let n = points.len();
    let mut coords: Vec<(f32, f32)> = Vec::with_capacity(n);
    for (i, &v) in points.iter().enumerate() {
        let x = pad + (width as f32 - pad * 2.0) * (i as f32) / (n - 1) as f32;
        let t = (v - vmin) / (vmax - vmin);
        let y = (height as f32 - pad) - t * (height as f32 - pad * 2.0);
        coords.push((x, y));
    }
    if fill_coverage > 0 {
        for i in 0..n - 1 {
            let (x0, y0) = coords[i];
            let (x1, y1) = coords[i + 1];
            let steps = ((x1 - x0).abs().ceil() as i32).max(1);
            for s in 0..=steps {
                let f = s as f32 / steps as f32;
                let x = x0 + (x1 - x0) * f;
                let y = y0 + (y1 - y0) * f;
                let xi = x.round() as i32;
                if xi < 0 || xi >= width as i32 {
                    continue;
                }
                let y_top = y.round() as i32;
                for yy in y_top.max(0)..(height as i32) {
                    let idx = (yy as u32 * width + xi as u32) as usize;
                    px[idx] = px[idx].max(fill_coverage);
                }
            }
        }
    }
    for i in 0..n - 1 {
        draw_line(
            &mut px,
            width,
            height,
            coords[i].0,
            coords[i].1,
            coords[i + 1].0,
            coords[i + 1].1,
            line_coverage,
        );
    }
    AlphaMask::new(width, height, Arc::<[u8]>::from(px))
}

#[allow(clippy::too_many_arguments)]
fn draw_line(px: &mut [u8], w: u32, h: u32, x0: f32, y0: f32, x1: f32, y1: f32, cov: u8) {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let steps = (dx.abs().max(dy.abs()).ceil() as i32).max(1);
    for s in 0..=steps {
        let f = s as f32 / steps as f32;
        stamp_disc(px, w, h, x0 + dx * f, y0 + dy * f, 1.35, cov);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauge_not_empty() {
        let m = rasterize_gauge(64, 50.0, 0.0, 100.0, 40, 200, 255);
        assert_eq!(m.width, 64);
        assert!(m.pixels.iter().any(|&p| p > 0));
    }

    #[test]
    fn trend_not_empty() {
        let pts = vec![1.0, 3.0, 2.0, 5.0, 4.0];
        let m = rasterize_trend(120, 40, &pts, 255, 40);
        assert_eq!(m.width, 120);
        assert!(m.pixels.iter().any(|&p| p > 0));
    }
}
