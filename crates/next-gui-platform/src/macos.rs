//! macOS Office window backend using winit + softbuffer (OS API layer only).

use crate::{
    AppCallbacks, Event, KeyCode, PlatformError, PointerButton, WindowAttrs, WindowCtx, WindowHost,
    WindowId,
};
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::rc::Rc;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::Window as WinitWindow;

struct Host {
    window_id: WindowId,
    window: Rc<WinitWindow>,
    surface: Surface<Rc<WinitWindow>, Rc<WinitWindow>>,
    width: u32,
    height: u32,
    cursor: (f64, f64),
}

impl WindowHost for Host {
    fn window_id(&self) -> WindowId {
        self.window_id
    }

    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn set_title(&mut self, title: &str) {
        self.window.set_title(title);
    }

    fn request_redraw(&mut self) {
        self.window.request_redraw();
    }

    fn present(&mut self, width: u32, height: u32, pixels: &[u32]) -> Result<(), PlatformError> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        if pixels.len() < (width as usize) * (height as usize) {
            return Err(PlatformError::new("present: pixel buffer too small"));
        }
        self.width = width;
        self.height = height;
        self.surface
            .resize(
                NonZeroU32::new(width).ok_or_else(|| PlatformError::new("width is zero"))?,
                NonZeroU32::new(height).ok_or_else(|| PlatformError::new("height is zero"))?,
            )
            .map_err(|e| PlatformError::new(format!("softbuffer resize: {e}")))?;
        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|e| PlatformError::new(format!("softbuffer buffer: {e}")))?;
        let n = (width as usize) * (height as usize);
        buffer[..n].copy_from_slice(&pixels[..n]);
        buffer
            .present()
            .map_err(|e| PlatformError::new(format!("softbuffer present: {e}")))?;
        Ok(())
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
            Ok(w) => Rc::new(w),
            Err(e) => {
                eprintln!("next-gui-platform: failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };
        let context = match Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("next-gui-platform: softbuffer context: {e}");
                event_loop.exit();
                return;
            }
        };
        let surface = match Surface::new(&context, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("next-gui-platform: softbuffer surface: {e}");
                event_loop.exit();
                return;
            }
        };
        let inner = window.inner_size();
        self.host = Some(Host {
            window_id: WindowId(1),
            window,
            surface,
            width: inner.width.max(1),
            height: inner.height.max(1),
            cursor: (0.0, 0.0),
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
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                let pressed = key_event.state == ElementState::Pressed;
                let key = match &key_event.logical_key {
                    Key::Named(NamedKey::Escape) => KeyCode::Escape,
                    Key::Named(NamedKey::Space) => KeyCode::Space,
                    _ => KeyCode::Other(0),
                };
                if matches!(key, KeyCode::Other(_)) && !pressed {
                    return;
                }
                // Only forward Escape/Space for P0 noise reduction; still allow Other on press if needed
                if matches!(key, KeyCode::Escape | KeyCode::Space) {
                    let mut ctx = WindowCtx { inner: host };
                    self.app.on_event(
                        &mut ctx,
                        Event::Key {
                            window: WindowId(1),
                            key,
                            pressed,
                        },
                    );
                    if matches!(key, KeyCode::Escape) && pressed {
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }
}

/// Run a single-window macOS Office app until exit.
///
/// Requires a macOS windowing session on the host.
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
