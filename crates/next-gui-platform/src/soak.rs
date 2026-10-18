//! P6e soak / leak probe helpers (`NEXT_GUI_SOAK`).
//!
//! When [`enabled`] is true, OS backends schedule periodic
//! [`ControlFlow::WaitUntil`](winit::event_loop::ControlFlow::WaitUntil) wakes and
//! `request_redraw` so Soft examples can exercise retained + dirty paths for hours.
//! Env unset → no behavior change (default `ControlFlow::Wait`).
//!
//! Operator guide: `docs/soak.md`. **Not** wired into CI for 24h runs.

use std::env;
use std::time::{Duration, Instant};

/// Env flag: `NEXT_GUI_SOAK=1` / `true` / `yes` / `on` (case-insensitive).
pub const ENV_SOAK: &str = "NEXT_GUI_SOAK";

/// Optional redraw interval in milliseconds (`NEXT_GUI_SOAK_MS`). Default [`DEFAULT_INTERVAL_MS`].
pub const ENV_SOAK_MS: &str = "NEXT_GUI_SOAK_MS";

/// Default soak wake / redraw interval when env is unset or invalid.
pub const DEFAULT_INTERVAL_MS: u64 = 1000;

/// True when soak mode is requested via [`ENV_SOAK`].
///
/// Unset, empty, `0`, `false`, `no`, `off` → false.
pub fn enabled() -> bool {
    parse_enabled(env::var_os(ENV_SOAK).and_then(|v| v.into_string().ok()).as_deref())
}

/// Soak redraw interval from [`ENV_SOAK_MS`], or [`DEFAULT_INTERVAL_MS`].
///
/// Invalid / zero / missing → default. Cap at 1 hour to avoid accidental near-stall.
pub fn interval() -> Duration {
    Duration::from_millis(parse_interval_ms(
        env::var_os(ENV_SOAK_MS)
            .and_then(|v| v.into_string().ok())
            .as_deref(),
    ))
}

fn parse_enabled(raw: Option<&str>) -> bool {
    match raw.map(str::trim) {
        None | Some("") => false,
        Some(s) => matches!(
            s.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
    }
}

fn parse_interval_ms(raw: Option<&str>) -> u64 {
    const MAX_MS: u64 = 3_600_000;
    match raw.and_then(|s| s.trim().parse::<u64>().ok()) {
        Some(ms) if ms > 0 => ms.min(MAX_MS),
        _ => DEFAULT_INTERVAL_MS,
    }
}

/// Read Linux process RSS from `/proc/self/status` (`VmRSS`, kibibytes).
///
/// Returns `None` on non-Linux or parse failure — callers should skip gracefully.
pub fn linux_vm_rss_kb() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/proc/self/status").ok()?;
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("VmRSS:") else {
                continue;
            };
            // Format: "VmRSS:\t  12345 kB"
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb);
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Drives periodic wakes under soak; no-op when soak is off.
///
/// Backends call [`SoakDriver::about_to_wait`] from winit `ApplicationHandler::about_to_wait`.
#[derive(Debug, Clone)]
pub struct SoakDriver {
    enabled: bool,
    interval: Duration,
    next_wake: Instant,
}

impl SoakDriver {
    /// Snapshot env at construction (call once when entering `run`).
    pub fn from_env() -> Self {
        let enabled = enabled();
        let interval = interval();
        Self {
            enabled,
            interval,
            next_wake: Instant::now() + interval,
        }
    }

    /// Whether soak was enabled when this driver was created.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Interval used for WaitUntil / redraw pacing.
    #[inline]
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Schedule the next wait deadline; return `true` when a redraw wake is due.
    ///
    /// When disabled, returns `false` and does not change control flow (caller keeps `Wait`).
    /// Requires a windowing feature (`linux` / `macos` / `windows`) so `winit` is linked.
    #[cfg(any(feature = "linux", feature = "macos", feature = "windows"))]
    pub fn about_to_wait(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) -> bool {
        if !self.enabled {
            return false;
        }
        let now = Instant::now();
        let due = now >= self.next_wake;
        if due {
            // Keep cadence even if we woke late.
            self.next_wake = now + self.interval;
        }
        event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(self.next_wake));
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soak_env_defaults_off() {
        assert!(!parse_enabled(None));
        assert!(!parse_enabled(Some("")));
        assert!(!parse_enabled(Some("0")));
        assert!(!parse_enabled(Some("false")));
        assert!(!parse_enabled(Some("no")));
        assert!(!parse_enabled(Some("off")));
        assert!(!parse_enabled(Some("  ")));
    }

    #[test]
    fn soak_env_truthy() {
        assert!(parse_enabled(Some("1")));
        assert!(parse_enabled(Some("true")));
        assert!(parse_enabled(Some("TRUE")));
        assert!(parse_enabled(Some("yes")));
        assert!(parse_enabled(Some("on")));
        assert!(parse_enabled(Some(" On ")));
    }

    #[test]
    fn soak_interval_defaults_and_caps() {
        assert_eq!(parse_interval_ms(None), DEFAULT_INTERVAL_MS);
        assert_eq!(parse_interval_ms(Some("")), DEFAULT_INTERVAL_MS);
        assert_eq!(parse_interval_ms(Some("0")), DEFAULT_INTERVAL_MS);
        assert_eq!(parse_interval_ms(Some("-1")), DEFAULT_INTERVAL_MS);
        assert_eq!(parse_interval_ms(Some("abc")), DEFAULT_INTERVAL_MS);
        assert_eq!(parse_interval_ms(Some("500")), 500);
        assert_eq!(parse_interval_ms(Some("999999999")), 3_600_000);
    }

    #[test]
    fn soak_driver_from_env_respects_disabled_default() {
        // Construction reads process env; we only assert the type + disabled path
        // when NEXT_GUI_SOAK is unset in the test process (CI / local default).
        if std::env::var_os(ENV_SOAK).is_some() {
            eprintln!("soak_driver_from_env_respects_disabled_default: env set; skip assert");
            return;
        }
        let d = SoakDriver::from_env();
        assert!(!d.is_enabled());
        assert_eq!(d.interval(), Duration::from_millis(DEFAULT_INTERVAL_MS));
    }

    #[test]
    fn linux_vm_rss_readable_on_linux() {
        let rss = linux_vm_rss_kb();
        #[cfg(target_os = "linux")]
        {
            let kb = rss.expect("VmRSS should parse on Linux");
            assert!(kb > 0, "VmRSS should be > 0");
        }
        #[cfg(not(target_os = "linux"))]
        {
            assert!(rss.is_none());
        }
    }
}
