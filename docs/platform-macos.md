# macOS platform backend (Office desktop)

> **Status: Office P0–P5 paths implemented; acceptance has shared issues.** Linux remains the behavioral reference. macOS uses the same platform contract, retained widgets, Profile configuration, and optional Immediate viewport. See the [2026-10-18 re-verification](reviews/R-20261018-0e892e6f7b78.md) for actual checks and remaining defects.

---

## 1. Scope

| Item | Decision |
|------|----------|
| Product profile | **Office desktop only** |
| Field (industrial / embedded) | **Not required** on macOS |
| OS order | Linux (done) → Windows (later) → **macOS (Office)** |
| Input for first pass | Pointer + keyboard + IME (same as Linux); `Event::Touch` stub (winit macOS does not emit touch); full HMI touch is P6 |
| Clipboard | UTF-8 text via `WindowCtx::clipboard_*` (`arboard`) |
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
- [x] Deliver `Event::{CloseRequested, Resized, RedrawRequested, PointerMoved, PointerButton, PointerScroll, Key, Ime}` plus the `Event::Touch` stub
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
- Full HMI touch mapping and gestures (the `Event::Touch` stub is present; P6 owns the mapping)
- Explicit DPI policy, accessibility, and long-running industrial acceptance (P6)
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

- Retina scale is currently handled as winit physical-pixel events; there is no explicit DPI policy yet.
- **IME** is wired (same contract as Linux). **Clipboard** thin text API is available on `WindowCtx`.
- **Touch:** `Event::Touch` / `TouchEvent` stubs exist and are forwarded when winit emits them; macOS winit currently does not emit touch. Full industrial touch mapping is P6.
- Multi-window management and accessibility remain future work shared with the other platforms.
- Re-verification passed native GPU color agreement, DateTime overlay stacking and required clippy checks. Consumed Escape now dismisses/blurs without exiting, but e13 still drops Escape before its quit branch (F03). TextArea explicit newlines and short-line carets work; automatic wrapping still clips the caret (F07). The new `request_exit` API also waits for a keyboard event when requested from other callbacks. These shared findings remain assigned to Grok in the linked review.
- The macOS review used an Apple Silicon host and a 1× display; Retina and mixed-scale display transitions have not been accepted.
- The backend intentionally keeps the Linux event and presentation behavior; platform-specific refinements can land without changing the upper-layer contract.

---

## 7. Coordination

Linux P0/P1 continue on `main` without waiting for macOS. Prefer small PRs that only touch `macos.rs`, Cargo features, and example `cfg` — avoid drive-by refactors of `linux.rs` unless fixing a shared trait bug.
