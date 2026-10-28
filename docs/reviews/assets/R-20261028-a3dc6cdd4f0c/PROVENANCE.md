# Visual provenance

- Code target: `a3dc6cdd4f0ca54aded59889bd4cede0f58dc205`, platform deadline API `ffeb7a8`.
- Actual macOS27.0.1/arm64, actual Metal surfaces/readback; no reference bitmap substitution.
- `render-contact.png`: both themes ×1400×1000/800×600 ×held/released integer editor. Eight independently compared Soft/Metal pairs have max channel delta1–2. Final contrast-target render driver73154 exit0 awaited; all eight pairs independently rechecked. Required checks driver72693 exit0 awaited:644 tests/43 groups, Clippy, workspace build and host macOS/Windows feature checks. Draft stays exact above2^53; the database is unchanged until explicit Save Pragmas. Narrow tests scroll the target into view and verify its hit identity before pressing.
- `native-contact.png`: dark/light ×pragmas-bottom, pragmas-confirm, pragmas-hold (sorted in that order). Six actual owned native Metal windows captured by CGWindowID/PID; helper0/appSIGINT−2, final contrast driver21202 exit0 awaited.
- Held native readiness additionally requires a real scheduled repeated draft step, not just startup. Dark logged11 consecutive steps9007199254740995–9007199254741005, light10 through9007199254741004; every delta exactly1. QA prepares the held pointer programmatically after the final native initial resize. `native-light-held.png` is the full light held frame.
- `office-contact.png`:14 rebuilt e10–15/e20–27 owned native GPU windows, each helper0/appSIGINT−2; driver27680 exit0 awaited. All windows inspected. The final browser-only QA fixture and opt-in integer pressed-color changes do not affect these examples; they do not enable integer TextInput arrows.
- `native-deadline-probe.rs` reproduces the actual three-phase one-shot deadline check. Build with `rustc --edition2021 SOURCE --extern next_gui_platform=target/debug/libnext_gui_platform.rlib -L dependency=target/debug/deps -o /tmp/probe` after `cargo build -p next-gui-platform`; run with soak unset and then `NEXT_GUI_SOAK=1 NEXT_GUI_SOAK_MS=20`. Owned main-thread event loop exits through callbacks. Driver20875 exit0 awaited; the separate20ms soak run also exit0. Logs retained in `native-probes.log`.
- Initial arm regression failed before pointer tracking existed. The first narrow browser regression targeted an offscreen row; corrected by scrolling/rechecking hit identity. Initial native prepared holds were canceled by startup Resized; readiness correctly failed. QA now resets preparation on resize and final logs prove repeats. An earlier pre-final workspace test tree was deliberately stopped with exit143 during bound-feedback refinements; it is not passing evidence.
- This is synthetic pointer preparation and actual native scheduling/rendering. No actual OS mouse/keyboard/IME/picker or original-binary acceptance is claimed. Native focus events were observed by the deadline probe; native focus-loss cancellation is code-reviewed with shared cancellation tests.

## SHA-256

- `native-contact.png`: `8f67da079c8cb03a981c07aa96d717c00bcbe775969382061da223dae1c52ed7`
- `native-deadline-probe.rs`: `25f4c1fdb8e5a459d2d02c795a1642f8413faaa0c36bf678cfe54e37a5e609f1`
- `native-light-held.png`: `1ed33c8b3e0ca4c802b7be3f94bdf06b78e1f5951df47c1bdc321972631a0674`
- `native-probes.log`: `9686e8d080c8bd9d5de8cf890835042f83d67d0cc954e285a732d845056a6bb8`
- `office-contact.png`: `24238e2b4fd76162e2b647ac1afe440eae296b8adfde08fc974ced54edb3a9fa`
- `render-contact.png`: `8a26ee4c971045db669e3a4b4f5844b85b8426a3c20d878839b163beb072c846`
