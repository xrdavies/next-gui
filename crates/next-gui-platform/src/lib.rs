//! Windowing, input, and frame present abstraction for next-gui.
//!
//! P0 ships a Linux backend (`winit` + softbuffer) behind [`linux`]. That is an OS
//! API thin layer, not a third-party UI runtime.
//!
//! Present backends:
//! - [`PresentBackend::Softbuffer`] — CPU pixels via softbuffer (Soft + GPU readback).
//! - [`PresentBackend::GpuSurface`] — no softbuffer; window is free for `wgpu::Surface`
//!   (Office native GPU present). Soft apps must not use this mode for `present()`.

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
    /// Client width in physical pixels (P0: treat as buffer pixels).
    pub width: u32,
    /// Client height in physical pixels.
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

/// Keyboard key (P0 minimal set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    /// Escape.
    Escape,
    /// Space.
    Space,
    /// Catch-all.
    Other(u32),
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
    /// Window resized (physical pixels).
    Resized {
        /// Window.
        window: WindowId,
        /// New width.
        width: u32,
        /// New height.
        height: u32,
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
    },
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

    /// Current buffer / client size.
    pub fn size(&self) -> (u32, u32) {
        self.inner.size()
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

    /// Present packed XRGB8888 pixels (`width * height` length).
    ///
    /// Only valid for [`PresentBackend::Softbuffer`]. On [`PresentBackend::GpuSurface`]
    /// this returns an error — use `GpuRenderer::try_new_native` + its present path.
    pub fn present(&mut self, width: u32, height: u32, pixels: &[u32]) -> Result<(), PlatformError> {
        self.inner.present(width, height, pixels)
    }

    /// Clone of the underlying winit window (for creating a `wgpu::Surface`).
    ///
    /// Available on Linux / macOS backends built with winit.
    #[cfg(any(feature = "linux", feature = "macos", target_os = "macos"))]
    pub fn winit_window(&self) -> std::sync::Arc<winit::window::Window> {
        self.inner.winit_window()
    }
}

pub(crate) trait WindowHost {
    fn window_id(&self) -> WindowId;
    fn size(&self) -> (u32, u32);
    fn present_backend(&self) -> PresentBackend;
    fn set_title(&mut self, title: &str);
    fn request_redraw(&mut self);
    fn present(&mut self, width: u32, height: u32, pixels: &[u32]) -> Result<(), PlatformError>;
    #[cfg(any(feature = "linux", feature = "macos", target_os = "macos"))]
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

#[cfg(feature = "linux")]
pub mod linux;

#[cfg(feature = "linux")]
pub use linux::run;

/// macOS Office-desktop backend.
///
/// Compiled on `target_os = "macos"` or with `--features macos` (cross-platform typecheck).
/// See `docs/platform-macos.md`. Not a Field target.
#[cfg(any(target_os = "macos", feature = "macos"))]
pub mod macos;
