# macOS platform backend (Office desktop)

> **Status: Office P0–P5 paths implemented; tracked findings verified.** Linux remains the behavioral reference. macOS uses the same platform contract, retained widgets, Profile configuration, and optional Immediate viewport. See the [latest fix verification](reviews/R-20261019-d84207275ff4.md) for macOS checks and Linux probe verification; the [previous review](reviews/R-20261018-ded2d152ebca.md) records the repaired macOS build regression. Targeted checks do not constitute exhaustive GUI acceptance.

---

## 1. Scope

| Item | Decision |
|------|----------|
| Product profile | **Office desktop only** |
| Field (industrial / embedded) | **Not required** on macOS |
| OS order | Linux (done) → Windows (P6c Soft) → **macOS (Office)** |
| Input for first pass | Pointer + keyboard + IME (same as Linux); `Event::Touch` stub (winit macOS does not emit touch); full HMI touch is P6 |
| Clipboard | UTF-8 text, HTML and RGBA8 images via `WindowCtx::clipboard_*` (`arboard`); images limited to 128 MiB |
| Full windowing | **Implemented** with `winit` + optional softbuffer; `PresentBackend::GpuSurface` supported (same as Linux); do not block Linux demos |

macOS must implement the **same public platform contract** as Linux (`AppCallbacks`, `WindowAttrs`, `WindowCtx`, `Event`, `present` of XRGB8888). Upper layers (`scene` / `ui` / soft render) must not grow macOS-specific branches.

---

## 2. Reference implementation

| What | Where |
|------|--------|
| Working Linux backend | [`crates/next-gui-platform/src/linux.rs`](../crates/next-gui-platform/src/linux.rs) |
| Shared types & traits | [`crates/next-gui-platform/src/lib.rs`](../crates/next-gui-platform/src/lib.rs) |
| Architecture / OS order | [architecture.md](architecture.md) §5 |
| Requirements | [requirements.md](requirements.md) |

**Do not** invent a second event model. Port behavior from `linux::run`: create window → `on_start` → event loop → `on_event` / `on_redraw` → `WindowCtx::present`.

Suggested stack (non-binding, match Linux unless justified): `winit` event loop + `softbuffer` (or native present) behind the existing `WindowHost` / `present` shape. Prefer reusing workspace deps already declared for Linux when possible.

---

## 3. Backend location

```text
crates/next-gui-platform/
  src/
    lib.rs      # cfg-gates `macos` module
    linux.rs    # reference
    macos.rs    # Office window, input, and XRGB8888 soft-buffer backend
```

- Module is compiled when `target_os = "macos"` **or** feature `macos` (so Linux CI can `cargo check -p next-gui-platform --features macos`).
- Top-level `next_gui_platform::run` selects `macos::run` on macOS, including `default-features = false, features = ["macos"]`. Other hosts retain the Linux entry point under feature `linux` (default). Existing examples may also call `macos::run` explicitly.
- The `macos` feature enables the shared `winit` / `softbuffer` dependencies for cross-target checks without changing the Linux default.

---

## 4. Checklist for the implementing agent

### 4.1 Contract (must match Linux)

- [x] `pub fn run(attrs: WindowAttrs, app: impl AppCallbacks) -> Result<(), PlatformError>`
- [x] Deliver `Event::{CloseRequested, Resized, ScaleFactorChanged, RedrawRequested, PointerMoved, PointerButton, PointerScroll, Key, Ime}` plus the `Event::Touch` stub (P6a synth; P6b scale)
- [x] Map mouse buttons, wheel/trackpad deltas, editing keys, characters, and modifiers like Linux (including Command for select-all)
- [x] Enable system IME for focused TextInput; forward preedit/commit and caret coordinates
- [x] `WindowCtx::present(width, height, &[u32] /* XRGB8888 */)` shows soft-rendered frames
- [x] `request_redraw` / resize update buffer size consistently

### 4.2 Product constraints

- [x] Documented as **Office only** (README / this doc); no Field acceptance criteria on Mac
- [x] Soft-render path works first (Office may add GPU later; do not block on Metal)
- [x] No changes that break `cargo run -p e01_soft_window` or `e02_counter` on Linux

### 4.3 Integration

- [x] Examples reach `macos::run` through a target `cfg` or the shared native `run` entry point
- [x] The `macos` feature enables dependencies for cross-target checks while Linux keeps the `linux` default
- [x] This doc records the implemented status and known gaps

### 4.4 Shared Office capabilities

- [x] CJK font loading, measurement, and rasterization through `next-gui-text`
- [x] GPU readback and native wgpu Surface presentation through Metal; Soft can also upload to a GPU-surface window
- [x] P0/P1 Office widgets and Chart use the shared retained UI and draw commands
- [x] P4 Profile configuration and P5 optional Immediate viewport run without macOS-specific UI branches
- [x] QA image output uses the ignored workspace `tmp/demo-shots` directory rather than a Linux-only absolute path
- [x] UTF-8 clipboard text is available through `WindowCtx::clipboard_get_text` / `clipboard_set_text`

### 4.5 Remaining scope

- Field / embedded profile
- Full multi-touch gestures (P6a primary-finger → pointer is wired; multi-finger gestures remain later — see [p6-design.md](p6-design.md))
- Retina / mixed-scale **manual** acceptance (P6b contract + `scale_factor` / `dpi` are landed; hardware follow-on)
- Accessibility and long-running industrial acceptance (P6d / P6e; macOS follow-on after Linux)
- Replacing Linux backend or renaming traits

---

## 5. Verify before claiming done

On a Mac:

```bash
cargo check -p next-gui-platform --features macos
cargo test -p next-gui-platform --no-default-features --features macos
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --bins
cargo run -p e01_soft_window
cargo run -p e02_counter
cargo run -p e03_ngui_counter
cargo run -p e12_office_inputs
NEXT_GUI_GPU_SURFACE=1 cargo run -p e12_office_inputs
cargo run -p e24_profile_dual
cargo run -p e25_immediate_viewport
```

Run all current examples when shared platform behavior changes. Verify the actual
window, pointer/keyboard input, IME, resize, and each presentation path; a successful
build alone is not GUI acceptance. The five e21–e25 QA modes write PPM files under
`tmp/demo-shots` and create the directory themselves.

On Linux (must stay green):

```bash
cargo build -p e01_soft_window -p e02_counter
cargo check -p next-gui-platform --features macos   # cross-target typecheck
```

## 6. Known gaps

- **DPI (P6b):** `WindowCtx::scale_factor`, `Event::ScaleFactorChanged`, and `next_gui_platform::dpi` helpers are available; events remain physical by default. Retina / mixed-scale display transitions still need manual acceptance on hardware.
- **IME** is wired (same contract as Linux). **Clipboard** thin text API is available on `WindowCtx`.
- **Touch:** `Event::Touch` forwarded when winit emits it; P6a primary-finger pointer synth is shared with Linux. macOS winit often does not emit touch on trackpads.
- Multi-window management and accessibility remain future work shared with the other platforms.
- Re-verification passed native GPU color agreement, DateTime overlay stacking and required clippy checks. Follow-up checks also verify e13 Escape exit (F03), TextArea wrapped carets in Soft/Metal (F07), and `request_exit` from startup/redraw/pointer callbacks without another key. Codex added Linux target guards to the new X11/Wayland probe runner and its callers, restoring default macOS builds. Grok's Linux probes now skip unavailable desktop/tool prerequisites and force X11 for pointer injection; F02 is verified with source inspection, Linux compile/headless checks and Grok's desktop evidence. Current macOS tests and all 17 example startup/Escape checks pass; see the linked review.
- The macOS review used an Apple Silicon host and a 1× display; Retina and mixed-scale display transitions have not been accepted.
- The backend intentionally keeps the Linux event and presentation behavior; platform-specific refinements can land without changing the upper-layer contract.

---

## 7. Coordination

Linux P0/P1 continue on `main` without waiting for macOS. Prefer small PRs that only touch `macos.rs`, Cargo features, and example `cfg` — avoid drive-by refactors of `linux.rs` unless fixing a shared trait bug.

## Function keys (2026-10-27)

All winit backends forward F1–F24 as `KeyCode::Function(1..=24)` on press,
retaining modifiers. They are not text/IME input. Applications with exhaustive
key matches must handle this variant; Office examples ignore it unless they bind
an action (e28 uses F3/Shift+F3 for SQL search). Native hardware acceptance remains
separate from shared mapping tests and feature compilation.

## Pointer modifiers

`Event::PointerButton` includes `modifiers` at press/release time. Linux, macOS
and Windows use the same current winit modifier state; Command is `meta` on macOS.
Event consumers can use `modifiers.commandish()` for Ctrl/Command gestures. Direct
constructors must supply the field; exhaustive patterns should include it or `..`.
Touch backends call `synthesize_with_modifiers`; the existing `synthesize` helper
continues to produce unmodified pointer events. Native gesture acceptance remains
a separate check from compile and synthetic event tests.

## Page keys

All three winit hosts now forward named PageUp/PageDown on press and release as
`KeyCode::PageUp` / `KeyCode::PageDown`, retaining event modifiers. Consumers with
exhaustive key matches must handle the two new variants. Office key adapters map
them to `UiKey::PageUp` / `UiKey::PageDown`; Table supports visible-page movement
and the `SelectPageUp` / `SelectPageDown` range variants. TextArea page movement
and native hardware/IME acceptance remain separate work.

## System numeric conventions

`number_conventions::NumberConventions::system()` returns an owned, thread-safe snapshot without changing the process locale. CSV import freezes this snapshot for preview and insertion.

The macOS provider uses owned Core Foundation locale/formatter objects, copies decimal/group/digit/sign/exponent properties, then releases all native objects. Native named-locale checks cover German, Indian grouping and Arabic digits.

## Delayed redraw and tracking cancellation (2026-10-28)

`WindowCtx::request_redraw_after(Duration)` schedules a one-shot native redraw.
Repeated requests keep the earliest deadline, ordinary redraws leave it pending,
and consumed requests return to the normal idle wait. A later deadline does not
postpone an earlier one; deadlines beyond the clock range are ignored. The same
scheduler composes with `NEXT_GUI_SOAK` by retaining the earlier wake time.

All three backends forward `Event::FocusChanged` and `Event::PointerLeft`.
Hosts can release captured controls on focus loss and cancel held-button repeats
when the pointer leaves the window. The DB Browser uses this for integer arrows;
UI repeat timers stop on release and do not run continuously while idle.

A real macOS event-loop probe verified three delayed redraws, earlier/later
coalescing, no idle spin, callback exit and coexistence with20ms soak wakes.
Actual Windows/Linux runtime acceptance remains unavailable on this Mac.
