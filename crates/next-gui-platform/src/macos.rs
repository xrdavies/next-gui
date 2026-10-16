//! macOS window backend — **Office desktop placeholder**.
//!
//! Field (industrial) is **not** required on macOS. This module compiles and
//! exposes [`run`] with a clear unimplemented error until a real Cocoa/`winit`
//! backend lands.
//!
//! - Contract: same as Linux — [`crate::AppCallbacks`], [`crate::WindowCtx`], [`crate::Event`].
//! - Reference implementation: [`crate::linux`] (`crates/next-gui-platform/src/linux.rs`).
//! - Agent brief: `docs/platform-macos.md`.

use crate::{AppCallbacks, PlatformError, WindowAttrs};

/// Run a single-window macOS Office app until exit.
///
/// **Not implemented.** Returns [`PlatformError`] so callers fail loudly instead
/// of panicking mid-loop. Replace this body with a real event loop modeled on
/// [`crate::linux::run`].
pub fn run(_attrs: WindowAttrs, _app: impl AppCallbacks) -> Result<(), PlatformError> {
    Err(PlatformError::new(
        "macOS platform backend is not implemented yet (Office desktop placeholder). \
         See docs/platform-macos.md; use crates/next-gui-platform/src/linux.rs as the reference.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Event, WindowCtx};

    struct Noop;

    impl AppCallbacks for Noop {
        fn on_start(&mut self, _ctx: &mut WindowCtx<'_>) {}
        fn on_event(&mut self, _ctx: &mut WindowCtx<'_>, _event: Event) {}
        fn on_redraw(&mut self, _ctx: &mut WindowCtx<'_>) {}
    }

    #[test]
    fn run_is_unimplemented() {
        let err = run(WindowAttrs::new("mac stub", 320, 240), Noop).unwrap_err();
        assert!(
            err.to_string().contains("not implemented"),
            "unexpected message: {err}"
        );
    }
}
