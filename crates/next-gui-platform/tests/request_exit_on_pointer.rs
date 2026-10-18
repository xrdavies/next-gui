//! Linux GUI: request_exit from pointer callback must end run without a key.
//!
//! Skips when no desktop / X11 / xdotool is available so headless `cargo test`
//! does not panic. Wayland-only sessions are skipped (xdotool is X11-only).
//! Mixed Wayland+XWayland sessions force the X11 backend so `xdotool` can
//! address the window (default winit prefers Wayland when both are set).
#![cfg(all(feature = "linux", target_os = "linux"))]

mod common;

use next_gui_platform::{linux, AppCallbacks, Event, WindowAttrs, WindowCtx};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct QuitOnPointer {
    ready: Arc<Mutex<bool>>,
}

impl AppCallbacks for QuitOnPointer {
    fn on_start(&mut self, ctx: &mut WindowCtx<'_>) {
        *self.ready.lock().unwrap() = true;
        ctx.request_redraw();
    }
    fn on_event(&mut self, ctx: &mut WindowCtx<'_>, event: Event) {
        if let Event::PointerButton { pressed: true, .. } = event {
            ctx.request_exit();
        }
    }
    fn on_redraw(&mut self, ctx: &mut WindowCtx<'_>) {
        let (w, h) = ctx.size();
        let px = vec![0x4040_4040u32; (w as usize).saturating_mul(h as usize).max(1)];
        let _ = ctx.present(w.max(1), h.max(1), &px);
    }
}

#[test]
fn request_exit_from_pointer_ends_run() {
    if common::skip_unless_x11_pointer("request_exit_on_pointer") {
        return;
    }
    let ready = Arc::new(Mutex::new(false));
    let ready_t = Arc::clone(&ready);
    let handle = std::thread::spawn(move || {
        // Force X11: on mixed Wayland+XWayland desktops, default run_any_thread
        // would prefer Wayland and xdotool could not find the window.
        linux::run_any_thread_x11(
            WindowAttrs::new("quit-on-pointer", 320, 120),
            QuitOnPointer { ready: ready_t },
        )
    });
    let deadline_ready = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline_ready {
        if *ready.lock().unwrap() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(*ready.lock().unwrap(), "pointer probe on_start never ran");
    std::thread::sleep(Duration::from_millis(250));
    let status = std::process::Command::new("bash")
        .args([
            "-lc",
            r#"WID=$(xdotool search --name 'quit-on-pointer' | head -n1); test -n "$WID" && xdotool windowactivate --sync "$WID" mousemove --window "$WID" 160 60 click 1"#,
        ])
        .status()
        .expect("spawn xdotool");
    assert!(status.success(), "xdotool click failed");
    common::join_probe(handle, Duration::from_secs(5), "pointer request_exit")
        .expect("pointer request_exit should end run");
}
