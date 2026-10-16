# macOS platform backend (Office desktop)

> **Status: placeholder for parallel work.** Linux P0/P1 remain the reference implementation. This document is the brief for the agent implementing macOS windowing.

---

## 1. Scope

| Item | Decision |
|------|----------|
| Product profile | **Office desktop only** |
| Field (industrial / embedded) | **Not required** on macOS |
| OS order | Linux (done) → Windows (later) → **macOS (Office)** |
| Input for first pass | Pointer + keyboard (same as Linux P0); touch later |
| Full windowing | **Not started** — stub only; do not block Linux demos |

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

## 3. Stub location

```text
crates/next-gui-platform/
  src/
    lib.rs      # cfg-gates `macos` module
    linux.rs    # reference
    macos.rs    # placeholder: run() → clear Unimplemented error
```

- Module is compiled when `target_os = "macos"` **or** feature `macos` (so Linux CI can `cargo check -p next-gui-platform --features macos`).
- Top-level `next_gui_platform::run` stays the Linux backend under feature `linux` (default). Call `next_gui_platform::macos::run` on macOS until examples grow OS `cfg`.
- Stub must **not** pull broken deps or break `cargo build` / demos on Linux.

---

## 4. Checklist for the implementing agent

### 4.1 Contract (must match Linux)

- [ ] `pub fn run(attrs: WindowAttrs, app: impl AppCallbacks) -> Result<(), PlatformError>`
- [ ] Deliver `Event::{CloseRequested, Resized, RedrawRequested, PointerMoved, PointerButton, Key}`
- [ ] Map primary mouse + Escape / Space (and `KeyCode::Other`) like Linux
- [ ] `WindowCtx::present(width, height, &[u32] /* XRGB8888 */)` shows soft-rendered frames
- [ ] `request_redraw` / resize update buffer size consistently

### 4.2 Product constraints

- [ ] Documented as **Office only** (README / this doc); no Field acceptance criteria on Mac
- [ ] Soft-render path works first (Office may add GPU later; do not block on Metal)
- [ ] No changes that break `cargo run -p p0_soft_window` or `p1_counter` on Linux

### 4.3 Integration

- [ ] Optional: `#[cfg(target_os = "macos")]` in examples to call `macos::run`
- [ ] Feature `macos` enabled by default on macOS targets when ready (keep Linux default = `linux`)
- [ ] Update this doc status from “placeholder” → “implemented” and note known gaps (IME, Retina scale, etc.)

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
cargo run -p p0_soft_window   # or cfg'd mac example
cargo run -p p1_counter
```

On Linux (must stay green):

```bash
cargo build -p p0_soft_window -p p1_counter
cargo check -p next-gui-platform --features macos   # stub typechecks
```

---

## 6. Coordination

Linux P0/P1 continue on `main` without waiting for macOS. Prefer small PRs that only touch `macos.rs`, Cargo features, and example `cfg` — avoid drive-by refactors of `linux.rs` unless fixing a shared trait bug.
