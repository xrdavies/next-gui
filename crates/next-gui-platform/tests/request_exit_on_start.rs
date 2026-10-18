//! Linux GUI: request_exit from on_start must end run without a key.
//!
//! Skips when no desktop session is available so headless `cargo test` does not panic.
#![cfg(all(feature = "linux", target_os = "linux"))]

mod common;

use next_gui_platform::{linux, AppCallbacks, Event, WindowAttrs, WindowCtx};
use std::time::Duration;

struct QuitOnStart;

impl AppCallbacks for QuitOnStart {
    fn on_start(&mut self, ctx: &mut WindowCtx<'_>) {
        ctx.request_exit();
    }
    fn on_event(&mut self, _: &mut WindowCtx<'_>, _: Event) {}
    fn on_redraw(&mut self, ctx: &mut WindowCtx<'_>) {
        let (w, h) = ctx.size();
        let px = vec![0x3030_3030u32; (w as usize).saturating_mul(h as usize).max(1)];
        let _ = ctx.present(w.max(1), h.max(1), &px);
    }
}

#[test]
fn request_exit_from_on_start_ends_run() {
    if common::skip_unless_display("request_exit_on_start") {
        return;
    }
    let handle = std::thread::spawn(|| {
        linux::run_any_thread(WindowAttrs::new("quit-on-start", 320, 120), QuitOnStart)
    });
    common::join_probe(handle, Duration::from_secs(5), "on_start request_exit")
        .expect("on_start request_exit should end run");
}
