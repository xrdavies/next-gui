# macOS platform backend (Office desktop)

> **Status: implemented (Office first pass).** Linux P0/P1 remain the behavioral reference; macOS uses the same public platform contract and soft-render path.

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
- Top-level `next_gui_platform::run` stays the Linux backend under feature `linux` (default). macOS examples call `next_gui_platform::macos::run` through a target `cfg`.
- The `macos` feature enables the shared `winit` / `softbuffer` dependencies for cross-target checks without changing the Linux default.

---

## 4. Checklist for the implementing agent

### 4.1 Contract (must match Linux)

- [x] `pub fn run(attrs: WindowAttrs, app: impl AppCallbacks) -> Result<(), PlatformError>`
- [x] Deliver `Event::{CloseRequested, Resized, RedrawRequested, PointerMoved, PointerButton, Key}`
- [x] Map primary mouse + Escape / Space (and `KeyCode::Other`) like Linux
- [x] `WindowCtx::present(width, height, &[u32] /* XRGB8888 */)` shows soft-rendered frames
- [x] `request_redraw` / resize update buffer size consistently

### 4.2 Product constraints

- [x] Documented as **Office only** (README / this doc); no Field acceptance criteria on Mac
- [x] Soft-render path works first (Office may add GPU later; do not block on Metal)
- [x] No changes that break `cargo run -p e01_soft_window` or `e02_counter` on Linux

### 4.3 Integration

- [x] `#[cfg(target_os = "macos")]` in examples calls `macos::run`
- [x] The `macos` feature enables dependencies for cross-target checks while Linux keeps the `linux` default
- [x] This doc records the implemented status and known gaps

### 4.4 Out of scope for first Mac drop

- Field / embedded profile
- Touch
- Full GPU / Metal backend
- Immediate viewport
- Replacing Linux backend or renaming traits

---

## 5. Verify before claiming done

On a Mac:

```bash
cargo check -p next-gui-platform --features macos
# after real windowing:
cargo run -p e01_soft_window   # or cfg'd mac example
cargo run -p e02_counter
```

On Linux (must stay green):

```bash
cargo build -p e01_soft_window -p e02_counter
cargo check -p next-gui-platform --features macos   # cross-target typecheck
```

## 6. Known gaps

- Retina scale is currently handled as winit physical-pixel events; there is no explicit DPI policy yet.
- **IME** is wired (same contract as Linux). **Clipboard** thin text API is available on `WindowCtx`.
- **Touch:** `Event::Touch` / `TouchEvent` stubs exist and are forwarded when winit emits them; macOS winit currently does not emit touch. Full industrial touch mapping is P6.
- Multi-window management and a Metal renderer remain out of scope for this Office pass.
- The backend intentionally keeps the Linux event and presentation behavior; platform-specific refinements can land without changing the upper-layer contract.

---

## 7. Coordination

Linux P0/P1 continue on `main` without waiting for macOS. Prefer small PRs that only touch `macos.rs`, Cargo features, and example `cfg` — avoid drive-by refactors of `linux.rs` unless fixing a shared trait bug.
