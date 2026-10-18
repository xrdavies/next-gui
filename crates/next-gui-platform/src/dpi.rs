//! P6b DPI / scale helpers: physical ↔ logical coordinate conversion.
//!
//! # Coordinate spaces
//!
//! - **Physical pixels:** framebuffer / Soft buffer / GpuSurface size; pointer and
//!   touch `x`/`y`; [`crate::Event::Resized`] width/height; [`crate::WindowCtx::present`]
//!   and [`crate::WindowCtx::size`]; IME cursor area.
//! - **Logical pixels:** layout / widget sizes / hit-testing *when* an app applies
//!   [`scale_factor`](crate::WindowCtx::scale_factor) via these helpers.
//!
//! # API default (non-breaking)
//!
//! Events and present stay in **physical** pixels. Apps that want CSS-like logical
//! layout multiply sizes by `scale_factor` when rasterizing into Soft / Gpu buffers,
//! and divide pointer coords when hit-testing in logical space. Soft buffer size
//! remains physical ([`crate::WindowCtx::size`]).
//!
//! Touch synthesis ([`crate::touch_synth`]) and pointer hit must use the **same**
//! conversion when scale is applied — do not mix spaces on one path.
//!
//! `scale_factor` comes from the OS (`winit` `window.scale_factor()`), default `1.0`.
//! Changes arrive as [`crate::Event::ScaleFactorChanged`].

/// Convert one logical length to physical (`logical * scale`).
#[inline]
pub fn logical_to_physical(v: f64, scale: f64) -> f64 {
    v * scale
}

/// Convert one physical length to logical (`physical / scale`).
///
/// When `scale` is `0.0` or non-finite, returns `v` unchanged (defensive).
#[inline]
pub fn physical_to_logical(v: f64, scale: f64) -> f64 {
    if scale.is_finite() && scale != 0.0 {
        v / scale
    } else {
        v
    }
}

/// Convert a logical size `(w, h)` to physical `u32` pixels (rounded, min 1 when input > 0).
#[inline]
pub fn logical_size_to_physical(width: f64, height: f64, scale: f64) -> (u32, u32) {
    (
        round_dim(logical_to_physical(width, scale)),
        round_dim(logical_to_physical(height, scale)),
    )
}

/// Convert a physical size `(w, h)` to logical `f64`.
#[inline]
pub fn physical_size_to_logical(width: u32, height: u32, scale: f64) -> (f64, f64) {
    (
        physical_to_logical(f64::from(width), scale),
        physical_to_logical(f64::from(height), scale),
    )
}

/// Convert a logical point to physical.
#[inline]
pub fn logical_point_to_physical(x: f64, y: f64, scale: f64) -> (f64, f64) {
    (
        logical_to_physical(x, scale),
        logical_to_physical(y, scale),
    )
}

/// Convert a physical point to logical (e.g. pointer hit-test in logical layout).
#[inline]
pub fn physical_point_to_logical(x: f64, y: f64, scale: f64) -> (f64, f64) {
    (
        physical_to_logical(x, scale),
        physical_to_logical(y, scale),
    )
}

fn round_dim(v: f64) -> u32 {
    if !v.is_finite() || v <= 0.0 {
        return 0;
    }
    let r = v.round();
    if r < 1.0 {
        1
    } else if r > f64::from(u32::MAX) {
        u32::MAX
    } else {
        r as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_at_1x() {
        assert_eq!(logical_to_physical(100.0, 1.0), 100.0);
        assert_eq!(physical_to_logical(100.0, 1.0), 100.0);
        assert_eq!(logical_size_to_physical(800.0, 600.0, 1.0), (800, 600));
        assert_eq!(physical_size_to_logical(800, 600, 1.0), (800.0, 600.0));
        assert_eq!(physical_point_to_logical(12.5, 40.0, 1.0), (12.5, 40.0));
    }

    #[test]
    fn scale_2x() {
        assert_eq!(logical_to_physical(100.0, 2.0), 200.0);
        assert_eq!(physical_to_logical(200.0, 2.0), 100.0);
        assert_eq!(logical_size_to_physical(400.0, 300.0, 2.0), (800, 600));
        assert_eq!(physical_size_to_logical(800, 600, 2.0), (400.0, 300.0));
        assert_eq!(
            logical_point_to_physical(10.0, 20.0, 2.0),
            (20.0, 40.0)
        );
        assert_eq!(
            physical_point_to_logical(20.0, 40.0, 2.0),
            (10.0, 20.0)
        );
    }

    #[test]
    fn round_trip_2x() {
        let scale = 2.0;
        let logical = (120.0, 80.5);
        let phys = logical_point_to_physical(logical.0, logical.1, scale);
        let back = physical_point_to_logical(phys.0, phys.1, scale);
        assert!((back.0 - logical.0).abs() < 1e-9);
        assert!((back.1 - logical.1).abs() < 1e-9);

        let (pw, ph) = logical_size_to_physical(320.0, 240.0, scale);
        let (lw, lh) = physical_size_to_logical(pw, ph, scale);
        assert!((lw - 320.0).abs() < 1e-9);
        assert!((lh - 240.0).abs() < 1e-9);
    }

    #[test]
    fn zero_scale_is_defensive() {
        assert_eq!(physical_to_logical(50.0, 0.0), 50.0);
        assert_eq!(physical_to_logical(50.0, f64::NAN), 50.0);
    }
}
