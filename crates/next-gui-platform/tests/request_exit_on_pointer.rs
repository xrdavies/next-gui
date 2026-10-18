//! Linux GUI: request_exit from pointer callback must end run without a key.
#![cfg(all(feature = "linux", target_os = "linux"))]

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
    let ready = Arc::new(Mutex::new(false));
    let ready_t = Arc::clone(&ready);
    let handle = std::thread::spawn(move || {
        linux::run_any_thread(
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
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if handle.is_finished() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        handle.is_finished(),
        "run did not exit within 5s after pointer request_exit"
    );
    handle
        .join()
        .expect("pointer worker panic")
        .expect("pointer request_exit should end run");
}
