//! Windowing, input, and frame present abstraction for next-gui.
//!
//! OS backends (`winit` + softbuffer) live behind [`linux`], [`macos`], and [`windows`].
//! That is an OS API thin layer, not a third-party UI runtime.
//! [`run`] selects the native backend: Windows on `target_os = "windows"`, macOS on a
//! Mac, otherwise Linux with feature `linux`.
//!
//! Present backends:
//! - [`PresentBackend::Softbuffer`] — CPU pixels via softbuffer (Soft + GPU readback).
//! - [`PresentBackend::GpuSurface`] — no softbuffer; window is free for `wgpu::Surface`
//!   (Office native GPU present). Soft apps must not use this mode for `present()`.
//!
//! Clipboard: thin UTF-8 text API on [`WindowCtx`] (see [`clipboard`]).
//! Touch: [`Event::Touch`] is always forwarded; P6a [`touch_synth`] maps the primary
//! finger into [`Event::PointerMoved`] / [`Event::PointerButton`] for HMI.
//!
//! DPI (P6b): events / present / [`WindowCtx::size`] stay in **physical** pixels.
//! Read [`WindowCtx::scale_factor`] and use [`dpi`] helpers when layout wants logical
//! coordinates. Scale changes arrive as [`Event::ScaleFactorChanged`].
//!
//! Windows (P6c): Softbuffer Soft path is required; see `docs/platform-windows.md`.
//!
//! Soak (P6e): optional [`soak`] helpers + `NEXT_GUI_SOAK=1` periodic redraw wakes; see `docs/soak.md`.

#![deny(missing_docs)]

use std::fmt;

/// Opaque window handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

/// How the OS window presents frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PresentBackend {
    /// Attach softbuffer; [`WindowCtx::present`] uploads XRGB8888 CPU pixels.
    /// Soft renderer and GPU-readback both use this path.
    #[default]
    Softbuffer,
    /// Do **not** attach softbuffer. The window is reserved for a `wgpu::Surface`
    /// created by `next-gui-render-gpu`. CPU [`WindowCtx::present`] is unavailable.
    GpuSurface,
}

/// Attributes for window creation.
#[derive(Debug, Clone)]
pub struct WindowAttrs {
    /// Title string.
    pub title: String,
    /// Client width in **physical** pixels (buffer / framebuffer pixels).
    ///
    /// Passed to winit as [`winit::dpi::PhysicalSize`]. At OS scale 1.0 this matches
    /// logical size; on Retina / HiDPI the OS may still round the resulting buffer.
    /// Requesting a logical size via winit `LogicalSize` is not exposed here yet —
    /// apps that want logical window sizes can multiply by [`WindowCtx::scale_factor`]
    /// after create, or wait for a non-breaking attrs extension.
    pub width: u32,
    /// Client height in **physical** pixels (see [`Self::width`]).
    pub height: u32,
    /// Present backend (softbuffer vs GPU-first surface).
    pub present_backend: PresentBackend,
}

impl WindowAttrs {
    /// Convenience constructor (defaults to [`PresentBackend::Softbuffer`]).
    pub fn new(title: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            title: title.into(),
            width,
            height,
            present_backend: PresentBackend::Softbuffer,
        }
    }

    /// Builder: GPU-first window without softbuffer.
    pub fn with_gpu_surface(mut self) -> Self {
        self.present_backend = PresentBackend::GpuSurface;
        self
    }

    /// Builder: explicit present backend.
    pub fn with_present_backend(mut self, backend: PresentBackend) -> Self {
        self.present_backend = backend;
        self
    }
}

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    /// Primary.
    Left,
    /// Secondary.
    Right,
    /// Middle.
    Middle,
    /// Other.
    Other(u16),
}

/// Keyboard modifiers (bit flags for text editing shortcuts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Ctrl / Control.
    pub ctrl: bool,
    /// Shift.
    pub shift: bool,
    /// Alt / Option.
    pub alt: bool,
    /// Meta / Command / Super.
    pub meta: bool,
}

impl Modifiers {
    /// True when Ctrl or Meta is held (Select-All / shortcuts).
    pub fn commandish(self) -> bool {
        self.ctrl || self.meta
    }
}

/// Keyboard key (Office input slice expands beyond Escape/Space).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    /// Escape.
    Escape,
    /// Space.
    Space,
    /// Backspace.
    Backspace,
    /// Delete (forward).
    Delete,
    /// Enter / Return.
    Enter,
    /// Tab.
    Tab,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Home.
    Home,
    /// End.
    End,
    /// Printable character from the keyboard path (typically Latin/ASCII).
    /// CJK composition arrives via [`Event::Ime`], not here.
    Char(char),
    /// Catch-all scancode / other.
    ///
    /// P6a reserves this as the extension hook for industrial / HMI function keys;
    /// a full field key map is out of scope for P6a.
    Other(u32),
}

/// Touch-screen contact phase (winit-compatible; primary finger also synthesizes pointer via P6a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TouchPhase {
    /// Finger (or stylus) contacted the surface.
    Started,
    /// Contact moved.
    Moved,
    /// Contact lifted.
    Ended,
    /// System cancelled tracking (focus loss, etc.).
    Cancelled,
}

/// One touch point. Primary id synthesizes pointer (P6a); multi-touch gestures remain later.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchEvent {
    /// Stable id for this contact until [`TouchPhase::Ended`] / [`TouchPhase::Cancelled`].
    pub id: u64,
    /// Phase.
    pub phase: TouchPhase,
    /// X in physical pixels.
    pub x: f64,
    /// Y in physical pixels.
    pub y: f64,
}

/// Platform / window events delivered to the app.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Event {
    /// User requested close.
    CloseRequested {
        /// Window.
        window: WindowId,
    },
    /// Window resized (**physical** pixels — framebuffer / Soft buffer size).
    ///
    /// Units match [`WindowCtx::size`] and [`WindowCtx::present`]. Does not change
    /// when only the OS scale factor changes; see [`Event::ScaleFactorChanged`].
    Resized {
        /// Window.
        window: WindowId,
        /// New width in physical pixels.
        width: u32,
        /// New height in physical pixels.
        height: u32,
    },
    /// OS display scale factor changed (winit `ScaleFactorChanged`).
    ///
    /// `scale_factor` is also readable via [`WindowCtx::scale_factor`]. Pointer/touch/
    /// resize events remain physical; use [`dpi`] to convert when doing logical layout.
    ScaleFactorChanged {
        /// Window.
        window: WindowId,
        /// New OS scale factor (typically `1.0`, `1.5`, `2.0`, …).
        scale_factor: f64,
    },
    /// Redraw requested by the OS or by [`WindowCtx::request_redraw`].
    RedrawRequested {
        /// Window.
        window: WindowId,
    },
    /// Pointer moved.
    PointerMoved {
        /// Window.
        window: WindowId,
        /// X in physical pixels.
        x: f64,
        /// Y in physical pixels.
        y: f64,
    },
    /// Pointer button.
    PointerButton {
        /// Window.
        window: WindowId,
        /// Button.
        button: PointerButton,
        /// Pressed or released.
        pressed: bool,
        /// X.
        x: f64,
        /// Y.
        y: f64,
    },
    /// Key event.
    Key {
        /// Window.
        window: WindowId,
        /// Key.
        key: KeyCode,
        /// Pressed or released.
        pressed: bool,
        /// Modifiers at event time.
        modifiers: Modifiers,
    },
    /// Mouse wheel / trackpad scroll.
    PointerScroll {
        /// Window.
        window: WindowId,
        /// Cursor X (physical px).
        x: f64,
        /// Cursor Y (physical px).
        y: f64,
        /// Horizontal delta (positive = right).
        delta_x: f64,
        /// Vertical delta (positive = scroll content down / finger up).
        delta_y: f64,
    },
    /// Input Method Editor (IME) composition / commit (CJK and other IMEs).
    Ime {
        /// Window.
        window: WindowId,
        /// IME payload.
        ime: ImeEvent,
    },
    /// Touch-screen contact. Always forwarded when the OS emits it.
    ///
    /// P6a also synthesizes primary-finger [`Event::PointerMoved`] /
    /// [`Event::PointerButton`] so retained UI works without handling Touch.
    /// Prefer consuming the synthesized pointer for HMI; listen here only for
    /// raw / multi-touch needs. macOS winit may not emit touch on all hardware.
    Touch {
        /// Window.
        window: WindowId,
        /// Touch payload.
        touch: TouchEvent,
    },
}

/// IME lifecycle and text from the OS input method (winit `WindowEvent::Ime`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeEvent {
    /// IME was enabled for this window (after [`WindowCtx::set_ime_allowed`]).
    Enabled,
    /// Composing text shown at the caret. Empty `text` clears preedit.
    /// `cursor` is an optional byte range into `text` (hide caret when `None`).
    Preedit {
        /// Preedit / composition string.
        text: String,
        /// Byte-range caret inside `text`, or `None` to hide.
        cursor: Option<(usize, usize)>,
    },
    /// Final text to insert at the caret (may be multi-character CJK).
    Commit {
        /// Committed string.
        text: String,
    },
    /// IME disabled; clear any pending preedit.
    Disabled,
}

/// Platform error.
#[derive(Debug)]
pub struct PlatformError {
    message: String,
}

impl PlatformError {
    /// Create from message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PlatformError {}

/// Mutable access to the live window during callbacks.
pub struct WindowCtx<'a> {
    pub(crate) inner: &'a mut dyn WindowHost,
}

impl WindowCtx<'_> {
    /// Logical next-gui window id (single-window P0: always 1).
    pub fn window_id(&self) -> WindowId {
        self.inner.window_id()
    }

    /// Current buffer / client size in **physical** pixels.
    pub fn size(&self) -> (u32, u32) {
        self.inner.size()
    }

    /// OS scale factor (`winit` `window.scale_factor()`), default `1.0`.
    ///
    /// Updated on create and on [`Event::ScaleFactorChanged`]. See [`dpi`].
    pub fn scale_factor(&self) -> f64 {
        self.inner.scale_factor()
    }

    /// Present backend chosen at window creation.
    pub fn present_backend(&self) -> PresentBackend {
        self.inner.present_backend()
    }

    /// Update title.
    pub fn set_title(&mut self, title: &str) {
        self.inner.set_title(title);
    }

    /// Ask the OS for a redraw.
    pub fn request_redraw(&mut self) {
        self.inner.request_redraw();
    }

    /// Ask the platform to leave the event loop after this callback returns.
    ///
    /// Honored after any app callback (`on_start`, `on_event`, `on_redraw`), not only
    /// keyboard handlers — so a Quit button or startup shutdown works without waiting
    /// for an unrelated key. Escape no longer exits by itself; apps call this when they
    /// want to quit (for example after a popup has already consumed Escape).
    pub fn request_exit(&mut self) {
        self.inner.request_exit();
    }

    /// Present packed XRGB8888 pixels (`width * height` length).
    ///
    /// Only valid for [`PresentBackend::Softbuffer`]. On [`PresentBackend::GpuSurface`]
    /// this returns an error — use `GpuRenderer::try_new_native` + its present path.
    pub fn present(
        &mut self,
        width: u32,
        height: u32,
        pixels: &[u32],
    ) -> Result<(), PlatformError> {
        self.inner.present(width, height, pixels)
    }

    /// Allow or deny IME composition for this window.
    ///
    /// Enable while a text field is focused so the OS IME can deliver
    /// [`Event::Ime`] preedit/commit. Disable otherwise so Space/etc. stay
    /// normal key events.
    pub fn set_ime_allowed(&mut self, allowed: bool) {
        self.inner.set_ime_allowed(allowed);
    }

    /// Tell the IME where the caret is (physical pixels), for candidate UI placement.
    pub fn set_ime_cursor_area(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.inner.set_ime_cursor_area(x, y, width, height);
    }

    /// Read UTF-8 text from the OS clipboard.
    ///
    /// Returns `Ok(None)` when empty or non-text. See [`clipboard`] for OS limits.
    pub fn clipboard_get_text(&mut self) -> Result<Option<String>, PlatformError> {
        self.inner.clipboard_get_text()
    }

    /// Write UTF-8 text to the OS clipboard.
    pub fn clipboard_set_text(&mut self, text: &str) -> Result<(), PlatformError> {
        self.inner.clipboard_set_text(text)
    }

    /// Clone of the underlying winit window (for creating a `wgpu::Surface`).
    ///
    /// Available on Linux / macOS / Windows backends built with winit.
    #[cfg(any(feature = "linux", feature = "macos", feature = "windows", target_os = "macos", target_os = "windows"))]
    pub fn winit_window(&self) -> std::sync::Arc<winit::window::Window> {
        self.inner.winit_window()
    }
}

pub(crate) trait WindowHost {
    fn window_id(&self) -> WindowId;
    fn size(&self) -> (u32, u32);
    fn scale_factor(&self) -> f64;
    fn present_backend(&self) -> PresentBackend;
    fn set_title(&mut self, title: &str);
    fn request_redraw(&mut self);
    fn request_exit(&mut self);
    fn present(&mut self, width: u32, height: u32, pixels: &[u32]) -> Result<(), PlatformError>;
    fn set_ime_allowed(&mut self, allowed: bool);
    fn set_ime_cursor_area(&mut self, x: f64, y: f64, width: f64, height: f64);
    fn clipboard_get_text(&mut self) -> Result<Option<String>, PlatformError>;
    fn clipboard_set_text(&mut self, text: &str) -> Result<(), PlatformError>;
    #[cfg(any(feature = "linux", feature = "macos", feature = "windows", target_os = "macos", target_os = "windows"))]
    fn winit_window(&self) -> std::sync::Arc<winit::window::Window>;
}

/// Application callbacks driven by the platform event loop.
pub trait AppCallbacks {
    /// Called once after the window exists.
    fn on_start(&mut self, ctx: &mut WindowCtx<'_>);
    /// Input / lifecycle event.
    fn on_event(&mut self, ctx: &mut WindowCtx<'_>, event: Event);
    /// Redraw: app should render and present (softbuffer or GPU surface).
    fn on_redraw(&mut self, ctx: &mut WindowCtx<'_>);
}

/// P6a primary-finger touch → pointer synthesis (shared by OS backends).
pub mod touch_synth;

/// P6b physical ↔ logical DPI helpers ([`dpi`]).
pub mod dpi;

/// P6e soak / leak probe env helpers ([`soak`]); backends honor `NEXT_GUI_SOAK`.
pub mod soak;

/// OS text clipboard helpers (also used by [`WindowCtx`]).
#[cfg(any(feature = "linux", feature = "macos", feature = "windows"))]
pub mod clipboard;

#[cfg(feature = "linux")]
pub mod linux;

#[cfg(all(feature = "linux", not(target_os = "macos"), not(target_os = "windows")))]
pub use linux::run;

#[cfg(target_os = "macos")]
pub use macos::run;

#[cfg(target_os = "windows")]
pub use windows::run;

/// macOS Office-desktop backend.
///
/// Compiled on `target_os = "macos"` or with `--features macos` (cross-platform typecheck).
/// See `docs/platform-macos.md`. Not a Field target.
#[cfg(any(target_os = "macos", feature = "macos"))]
pub mod macos;

/// Windows Field/Office backend (P6c Soft skeleton).
///
/// Compiled on `target_os = "windows"` or with `--features windows` (cross-platform typecheck).
/// Softbuffer Soft present is required; GPU Surface optional later.
/// See `docs/platform-windows.md`.
#[cfg(any(target_os = "windows", feature = "windows"))]
pub mod windows;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_factor_changed_constructs() {
        let ev = Event::ScaleFactorChanged {
            window: WindowId(1),
            scale_factor: 2.0,
        };
        match ev {
            Event::ScaleFactorChanged {
                window,
                scale_factor,
            } => {
                assert_eq!(window, WindowId(1));
                assert_eq!(scale_factor, 2.0);
            }
            _ => panic!("expected ScaleFactorChanged"),
        }
    }

    #[test]
    fn touch_event_stub_constructs() {
        let touch = TouchEvent {
            id: 7,
            phase: TouchPhase::Started,
            x: 12.5,
            y: 40.0,
        };
        let ev = Event::Touch {
            window: WindowId(1),
            touch,
        };
        match ev {
            Event::Touch { window, touch } => {
                assert_eq!(window, WindowId(1));
                assert_eq!(touch.id, 7);
                assert_eq!(touch.phase, TouchPhase::Started);
                assert_eq!(touch.x, 12.5);
                assert_eq!(touch.y, 40.0);
            }
            _ => panic!("expected Touch"),
        }
        assert_eq!(TouchPhase::Moved, TouchPhase::Moved);
        assert_eq!(TouchPhase::Ended, TouchPhase::Ended);
        assert_eq!(TouchPhase::Cancelled, TouchPhase::Cancelled);
    }

    #[test]
    #[cfg(any(feature = "linux", feature = "macos", feature = "windows"))]
    fn clipboard_roundtrip_smoke() {
        // May fail without a display / clipboard provider — treat that as skip.
        let Ok(mut cb) = clipboard::Clipboard::open() else {
            eprintln!("clipboard_roundtrip_smoke: open failed (no display?); skip");
            return;
        };
        let marker = "next-gui-clipboard-smoke-7c9427c";
        if let Err(e) = cb.set_text(marker) {
            eprintln!("clipboard_roundtrip_smoke: set failed: {e}; skip");
            return;
        }
        match cb.get_text() {
            Ok(Some(s)) => assert_eq!(s, marker),
            Ok(None) => panic!("expected text after set"),
            Err(e) => panic!("get failed: {e}"),
        }
    }
}
