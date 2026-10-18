//! Shared helpers for Linux GUI probe integration tests.
//!
//! These probes open a real window. They need a desktop session. When
//! `DISPLAY` / `WAYLAND_DISPLAY` / `WAYLAND_SOCKET` are all unset, callers
//! skip (pass) instead of panicking on event-loop creation. The pointer
//! probe additionally needs X11 `DISPLAY` and `xdotool` on `PATH`.

#![allow(dead_code)] // each integration binary uses a subset of these helpers

use std::env;
use std::path::Path;
use std::time::{Duration, Instant};

/// True when any of DISPLAY / WAYLAND_DISPLAY / WAYLAND_SOCKET is set and non-empty.
pub fn has_display_session() -> bool {
    ["DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET"]
        .iter()
        .any(|k| env::var_os(k).map(|v| !v.is_empty()).unwrap_or(false))
}

/// X11 `DISPLAY` is set (required for xdotool window search/click).
pub fn has_x11_display() -> bool {
    env::var_os("DISPLAY")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

pub fn xdotool_on_path() -> bool {
    if Path::new("/usr/bin/xdotool").is_file() {
        return true;
    }
    env::var_os("PATH")
        .map(|p| {
            env::split_paths(&p).any(|dir| {
                let candidate = dir.join("xdotool");
                candidate.is_file()
            })
        })
        .unwrap_or(false)
}

/// Returns `true` when the caller should return early (test passes as skip).
pub fn skip_unless_display(probe: &str) -> bool {
    if has_display_session() {
        return false;
    }
    eprintln!(
        "{probe}: skip — no DISPLAY/WAYLAND_DISPLAY/WAYLAND_SOCKET \
         (GUI probes need a desktop session)"
    );
    true
}

/// Pointer probe: needs a display session, X11 DISPLAY, and xdotool.
pub fn skip_unless_x11_pointer(probe: &str) -> bool {
    if skip_unless_display(probe) {
        return true;
    }
    if !has_x11_display() {
        eprintln!(
            "{probe}: skip — pointer probe needs X11 DISPLAY \
             (xdotool cannot drive Wayland-only sessions)"
        );
        return true;
    }
    if !xdotool_on_path() {
        eprintln!("{probe}: skip — xdotool not on PATH");
        return true;
    }
    false
}

/// Join a probe worker with a hard deadline so a broken exit path cannot hang CI.
pub fn join_probe<T>(handle: std::thread::JoinHandle<T>, timeout: Duration, what: &str) -> T {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if handle.is_finished() {
            return handle
                .join()
                .unwrap_or_else(|_| panic!("{what}: worker panicked"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("{what}: did not finish within {timeout:?}");
}
