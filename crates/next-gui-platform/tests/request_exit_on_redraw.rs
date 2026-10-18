//! Linux GUI: request_exit from on_redraw must end run without a key.
//!
//! Skips when no desktop session is available so headless `cargo test` does not panic.
#![cfg(all(feature = "linux", target_os = "linux"))]

mod common;

use next_gui_platform::{linux, AppCallbacks, Event, WindowAttrs, WindowCtx};
use std::time::Duration;

struct QuitOnRedraw {
    n: u32,
}

impl AppCallbacks for QuitOnRedraw {
    fn on_start(&mut self, _: &mut WindowCtx<'_>) {}
    fn on_event(&mut self, _: &mut WindowCtx<'_>, _: Event) {}
    fn on_redraw(&mut self, ctx: &mut WindowCtx<'_>) {
        let (w, h) = ctx.size();
        let px = vec![0x2020_2020u32; (w as usize).saturating_mul(h as usize).max(1)];
        let _ = ctx.present(w.max(1), h.max(1), &px);
        self.n += 1;
        if self.n >= 1 {
            ctx.request_exit();
        }
    }
}

#[test]
fn request_exit_from_on_redraw_ends_run() {
    if common::skip_unless_display("request_exit_on_redraw") {
        return;
    }
    let handle = std::thread::spawn(|| {
        linux::run_any_thread(
            WindowAttrs::new("quit-on-redraw", 320, 120),
            QuitOnRedraw { n: 0 },
        )
    });
    common::join_probe(handle, Duration::from_secs(5), "on_redraw request_exit")
        .expect("on_redraw request_exit should end run");
}
