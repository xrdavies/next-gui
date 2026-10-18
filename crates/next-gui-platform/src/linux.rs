//! Linux window backend using winit (+ optional softbuffer).

use crate::{
    clipboard::Clipboard, AppCallbacks, Event, ImeEvent, KeyCode, Modifiers, PlatformError,
    PointerButton, PresentBackend, TouchEvent, TouchPhase, WindowAttrs, WindowCtx, WindowHost,
    WindowId,
};
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, Ime as WinitIme, MouseButton, MouseScrollDelta, TouchPhase as WinitTouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::Window as WinitWindow;

struct Host {
    window_id: WindowId,
    window: Arc<WinitWindow>,
    /// Softbuffer surface when [`PresentBackend::Softbuffer`]; `None` for GPU-first.
    surface: Option<Surface<Arc<WinitWindow>, Arc<WinitWindow>>>,
    present_backend: PresentBackend,
    width: u32,
    height: u32,
    cursor: (f64, f64),
    modifiers: Modifiers,
    /// True while a non-empty IME preedit is active (suppress Key::Character).
    ime_composing: bool,
    /// Lazily opened OS clipboard (kept alive so Linux peers can read after set).
    clipboard: Option<Clipboard>,
    /// Set by [`WindowCtx::request_exit`]; checked after every `window_event` callback path.
    exit_requested: bool,
}

impl WindowHost for Host {
    fn window_id(&self) -> WindowId {
        self.window_id
    }

    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn present_backend(&self) -> PresentBackend {
        self.present_backend
    }

    fn set_title(&mut self, title: &str) {
        self.window.set_title(title);
    }

    fn request_redraw(&mut self) {
        self.window.request_redraw();
    }

    fn request_exit(&mut self) {
        self.exit_requested = true;
    }

    fn present(&mut self, width: u32, height: u32, pixels: &[u32]) -> Result<(), PlatformError> {
        let Some(surface) = self.surface.as_mut() else {
            return Err(PlatformError::new(
                "present: GpuSurface window has no softbuffer; use GpuRenderer native present",
            ));
        };
        if width == 0 || height == 0 {
            return Ok(());
        }
        if pixels.len() < (width as usize) * (height as usize) {
            return Err(PlatformError::new("present: pixel buffer too small"));
        }
        self.width = width;
        self.height = height;
        surface
            .resize(
                NonZeroU32::new(width).ok_or_else(|| PlatformError::new("width is zero"))?,
                NonZeroU32::new(height).ok_or_else(|| PlatformError::new("height is zero"))?,
            )
            .map_err(|e| PlatformError::new(format!("softbuffer resize: {e}")))?;
        let mut buffer = surface
            .buffer_mut()
            .map_err(|e| PlatformError::new(format!("softbuffer buffer: {e}")))?;
        let n = (width as usize) * (height as usize);
        buffer[..n].copy_from_slice(&pixels[..n]);
        buffer
            .present()
            .map_err(|e| PlatformError::new(format!("softbuffer present: {e}")))?;
        Ok(())
    }

    fn set_ime_allowed(&mut self, allowed: bool) {
        self.window.set_ime_allowed(allowed);
        if !allowed {
            self.ime_composing = false;
        }
    }

    fn set_ime_cursor_area(&mut self, x: f64, y: f64, width: f64, height: f64) {
        // Physical pixels from the app; winit accepts Physical via Into<Position/Size>.
        self.window.set_ime_cursor_area(
            winit::dpi::PhysicalPosition::new(x, y),
            winit::dpi::PhysicalSize::new(width.max(1.0), height.max(1.0)),
        );
    }

    fn clipboard_get_text(&mut self) -> Result<Option<String>, PlatformError> {
        if self.clipboard.is_none() {
            self.clipboard = Some(Clipboard::open()?);
        }
        self.clipboard
            .as_mut()
            .expect("clipboard just opened")
            .get_text()
    }

    fn clipboard_set_text(&mut self, text: &str) -> Result<(), PlatformError> {
        if self.clipboard.is_none() {
            self.clipboard = Some(Clipboard::open()?);
        }
        self.clipboard
            .as_mut()
            .expect("clipboard just opened")
            .set_text(text)
    }

    fn winit_window(&self) -> Arc<WinitWindow> {
        self.window.clone()
    }
}

struct Runner<A: AppCallbacks> {
    attrs: WindowAttrs,
    app: A,
    host: Option<Host>,
    started: bool,
}

impl<A: AppCallbacks> ApplicationHandler for Runner<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.host.is_some() {
            return;
        }
        let size = PhysicalSize::new(self.attrs.width.max(1), self.attrs.height.max(1));
        let winit_attrs = WinitWindow::default_attributes()
            .with_title(self.attrs.title.clone())
            .with_inner_size(size);
        let window = match event_loop.create_window(winit_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("next-gui-platform: failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };

        let present_backend = self.attrs.present_backend;
        let surface = match present_backend {
            PresentBackend::Softbuffer => {
                let context = match Context::new(window.clone()) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("next-gui-platform: softbuffer context: {e}");
                        event_loop.exit();
                        return;
                    }
                };
                match Surface::new(&context, window.clone()) {
                    Ok(s) => Some(s),
                    Err(e) => {
                        eprintln!("next-gui-platform: softbuffer surface: {e}");
                        event_loop.exit();
                        return;
                    }
                }
            }
            PresentBackend::GpuSurface => {
                // Leave the window free for wgpu::Surface (no softbuffer contention).
                None
            }
        };

        let inner = window.inner_size();
        self.host = Some(Host {
            window_id: WindowId(1),
            window,
            surface,
            present_backend,
            width: inner.width.max(1),
            height: inner.height.max(1),
            cursor: (0.0, 0.0),
            modifiers: Modifiers::default(),
            ime_composing: false,
            clipboard: None,
            exit_requested: false,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(host) = self.host.as_mut() else {
            return;
        };

        match &event {
            WindowEvent::CloseRequested => {
                let mut ctx = WindowCtx { inner: host };
                self.app.on_event(
                    &mut ctx,
                    Event::CloseRequested {
                        window: WindowId(1),
                    },
                );
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                host.width = size.width.max(1);
                host.height = size.height.max(1);
                let mut ctx = WindowCtx { inner: host };
                self.app.on_event(
                    &mut ctx,
                    Event::Resized {
                        window: WindowId(1),
                        width: size.width.max(1),
                        height: size.height.max(1),
                    },
                );
                ctx.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                if !self.started {
                    self.started = true;
                    let mut ctx = WindowCtx { inner: host };
                    self.app.on_start(&mut ctx);
                }
                let mut ctx = WindowCtx { inner: host };
                self.app.on_event(
                    &mut ctx,
                    Event::RedrawRequested {
                        window: WindowId(1),
                    },
                );
                self.app.on_redraw(&mut ctx);
            }
            WindowEvent::CursorMoved { position, .. } => {
                host.cursor = (position.x, position.y);
                let (x, y) = host.cursor;
                let mut ctx = WindowCtx { inner: host };
                self.app.on_event(
                    &mut ctx,
                    Event::PointerMoved {
                        window: WindowId(1),
                        x,
                        y,
                    },
                );
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = *state == ElementState::Pressed;
                let button = match button {
                    MouseButton::Left => PointerButton::Left,
                    MouseButton::Right => PointerButton::Right,
                    MouseButton::Middle => PointerButton::Middle,
                    MouseButton::Back | MouseButton::Forward | MouseButton::Other(_) => {
                        PointerButton::Other(0)
                    }
                };
                let (x, y) = host.cursor;
                let mut ctx = WindowCtx { inner: host };
                self.app.on_event(
                    &mut ctx,
                    Event::PointerButton {
                        window: WindowId(1),
                        button,
                        pressed,
                        x,
                        y,
                    },
                );
            }
            WindowEvent::ModifiersChanged(mods) => {
                let state = mods.state();
                host.modifiers = Modifiers {
                    ctrl: state.control_key(),
                    shift: state.shift_key(),
                    alt: state.alt_key(),
                    meta: state.super_key(),
                };
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => ((*x as f64) * 24.0, (*y as f64) * 24.0),
                    MouseScrollDelta::PixelDelta(p) => (p.x, p.y),
                };
                let (x, y) = host.cursor;
                let mut ctx = WindowCtx { inner: host };
                self.app.on_event(
                    &mut ctx,
                    Event::PointerScroll {
                        window: WindowId(1),
                        x,
                        y,
                        delta_x: dx,
                        // winit: positive y = scroll up; invert so positive delta_y moves content down.
                        delta_y: -dy,
                    },
                );
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                let pressed = key_event.state == ElementState::Pressed;
                let modifiers = host.modifiers;
                let key = match &key_event.logical_key {
                    Key::Named(NamedKey::Escape) => KeyCode::Escape,
                    Key::Named(NamedKey::Space) => KeyCode::Space,
                    Key::Named(NamedKey::Backspace) => KeyCode::Backspace,
                    Key::Named(NamedKey::Delete) => KeyCode::Delete,
                    Key::Named(NamedKey::Enter) => KeyCode::Enter,
                    Key::Named(NamedKey::Tab) => KeyCode::Tab,
                    Key::Named(NamedKey::ArrowLeft) => KeyCode::Left,
                    Key::Named(NamedKey::ArrowRight) => KeyCode::Right,
                    Key::Named(NamedKey::ArrowUp) => KeyCode::Up,
                    Key::Named(NamedKey::ArrowDown) => KeyCode::Down,
                    Key::Named(NamedKey::Home) => KeyCode::Home,
                    Key::Named(NamedKey::End) => KeyCode::End,
                    Key::Character(s) => {
                        // While IME is composing, text goes through Event::Ime — do not
                        // also insert Latin Key::Character from the composing keys.
                        if host.ime_composing {
                            KeyCode::Other(0)
                        } else {
                            let mut chars = s.chars();
                            match (chars.next(), chars.next()) {
                                (Some(c), None) if !c.is_control() => KeyCode::Char(c),
                                _ => KeyCode::Other(0),
                            }
                        }
                    }
                    _ => KeyCode::Other(0),
                };
                // Deliver presses for editing keys; Space/Escape also on release for legacy demos.
                // Skip Other / non-deliver keys without returning from window_event so the
                // shared exit_requested check below still runs after earlier callbacks.
                let deliver = !matches!(key, KeyCode::Other(_))
                    && (pressed || matches!(key, KeyCode::Escape | KeyCode::Space));
                if deliver {
                    let mut ctx = WindowCtx { inner: host };
                    self.app.on_event(
                        &mut ctx,
                        Event::Key {
                            window: WindowId(1),
                            key,
                            pressed,
                            modifiers,
                        },
                    );
                }
            }
            WindowEvent::Ime(ime) => {
                let mapped = match ime {
                    WinitIme::Enabled => ImeEvent::Enabled,
                    WinitIme::Preedit(text, cursor) => {
                        host.ime_composing = !text.is_empty();
                        ImeEvent::Preedit {
                            text: text.clone(),
                            cursor: *cursor,
                        }
                    }
                    WinitIme::Commit(text) => {
                        host.ime_composing = false;
                        ImeEvent::Commit {
                            text: text.clone(),
                        }
                    }
                    WinitIme::Disabled => {
                        host.ime_composing = false;
                        ImeEvent::Disabled
                    }
                };
                let mut ctx = WindowCtx { inner: host };
                self.app.on_event(
                    &mut ctx,
                    Event::Ime {
                        window: WindowId(1),
                        ime: mapped,
                    },
                );
            }
            WindowEvent::Touch(t) => {
                let phase = match t.phase {
                    WinitTouchPhase::Started => TouchPhase::Started,
                    WinitTouchPhase::Moved => TouchPhase::Moved,
                    WinitTouchPhase::Ended => TouchPhase::Ended,
                    WinitTouchPhase::Cancelled => TouchPhase::Cancelled,
                };
                let mut ctx = WindowCtx { inner: host };
                self.app.on_event(
                    &mut ctx,
                    Event::Touch {
                        window: WindowId(1),
                        touch: TouchEvent {
                            id: t.id,
                            phase,
                            x: t.location.x,
                            y: t.location.y,
                        },
                    },
                );
            }
            _ => {}
        }
        // Honor request_exit after every callback path (start/redraw/pointer/key/ime/touch),
        // not only KeyboardInput — otherwise Quit buttons and on_start exit never leave.
        if host.exit_requested {
            event_loop.exit();
        }
    }
}

/// Run a single-window Linux app until exit.
///
/// Requires a working `DISPLAY` (or Wayland) on the host.
/// Use [`WindowAttrs::with_gpu_surface`] for Office native wgpu present.
pub fn run(attrs: WindowAttrs, app: impl AppCallbacks) -> Result<(), PlatformError> {
    let event_loop =
        EventLoop::new().map_err(|e| PlatformError::new(format!("event loop: {e}")))?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut runner = Runner {
        attrs,
        app,
        host: None,
        started: false,
    };
    event_loop
        .run_app(&mut runner)
        .map_err(|e| PlatformError::new(format!("run_app: {e}")))?;
    Ok(())
}

/// Event loop for tests/probes that may not run on the process main thread.
///
/// Prefer [`run`] for applications. Hidden from docs; used by Linux GUI regressions.
#[doc(hidden)]
#[cfg(target_os = "linux")]
pub fn run_any_thread(attrs: WindowAttrs, app: impl AppCallbacks) -> Result<(), PlatformError> {
    use winit::platform::x11::EventLoopBuilderExtX11;
    use winit::platform::wayland::EventLoopBuilderExtWayland;
    let mut builder = EventLoop::builder();
    // Prefer X11 any-thread; Wayland builder ext is also applied for compositors that need it.
    EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
    EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
    let event_loop = builder
        .build()
        .map_err(|e| PlatformError::new(format!("event loop: {e}")))?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut runner = Runner {
        attrs,
        app,
        host: None,
        started: false,
    };
    event_loop
        .run_app(&mut runner)
        .map_err(|e| PlatformError::new(format!("run_app: {e}")))?;
    Ok(())
}
