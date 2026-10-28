# Windows platform backend (Field + Office)

> **Status: P6c Soft skeleton.** Compile-checked via `--features windows` on any host;
> **runtime GUI acceptance on a Windows graphics session is still manual / follow-up.**
> Same public platform contract as Linux (`AppCallbacks`, `WindowAttrs`, `WindowCtx`,
> `Event`, Softbuffer `present`). See [p6-design.md](p6-design.md) §3.3.

---

## 1. Scope

| Item | Decision |
|------|----------|
| Product profile | **Field + Office** (both; Soft must start without GPU) |
| Present (P6c) | **Softbuffer Soft path required** |
| GPU Surface | Optional later; not required to start |
| Input | Pointer + keyboard; IME best-effort (same winit forward as linux/macos) |
| Clipboard | UTF-8 text via `WindowCtx::clipboard_*` (`arboard`) |
| Touch / DPI | P6a `TouchPointerSynth` + P6b `scale_factor` / `ScaleFactorChanged` (shared) |
| OS order | Linux (done) → **Windows (P6c Soft)** → macOS (Office) |

Upper layers (`scene` / `ui` / soft render) must not grow Windows-specific branches.

---

## 2. Reference implementation

| What | Where |
|------|--------|
| Windows backend | [`crates/next-gui-platform/src/windows.rs`](../crates/next-gui-platform/src/windows.rs) |
| Adapted from | [`macos.rs`](../crates/next-gui-platform/src/macos.rs) (no Wayland/X11 builders) |
| Shared types | [`lib.rs`](../crates/next-gui-platform/src/lib.rs) |
| Architecture | [architecture.md](architecture.md) §5 |
| Design slice | [p6-design.md](p6-design.md) §3.3 |

Stack: `winit` event loop + `softbuffer` behind `WindowHost` / `present`. Feature `windows`
enables deps for cross-target typecheck (same pattern as `macos`).

`run_any_thread` is **N/A** on Windows (Linux-only probe helpers for X11/Wayland).

---

## 3. Backend location

```text
crates/next-gui-platform/
  src/
    lib.rs      # cfg-gates `windows` module + mutual-exclusion of `run`
    linux.rs
    macos.rs
    windows.rs  # P6c Soft skeleton
```

- Module compiles when `target_os = "windows"` **or** feature `windows`.
- Top-level `next_gui_platform::run` selects `windows::run` on Windows hosts.
- Linux `run` is gated `all(feature = "linux", not(macos), not(windows))` so a Windows
  host never picks the Linux entry.

---

## 4. How to check

On Linux (cross-target typecheck; no Windows GUI session):

```bash
cargo check -p next-gui-platform --features windows
cargo check -p next-gui-platform --features linux,macos,windows
cargo test -p next-gui-platform --lib
cargo clippy -p next-gui-platform --all-features -- -D warnings
```

On a native Windows graphics session (manual acceptance — not claimed by Linux CI):

```bash
cargo run -p e01_soft_window
# Soft Field path must work with no GPU adapter
```

`cargo run -p … --features windows` on Linux only typechecks the module; it does **not**
open a Win32 window.

---

## 5. Known gaps

- **No Windows runtime CI** on Linux agents; Soft window interactivity must be verified
  on a Windows desktop / GPU session.
- **IME:** forwarded via winit like linux/macos; Windows IME quirks (candidate UI,
  composition key suppression) need hardware follow-on.
- **GPU Surface:** optional; P6c does not require an adapter. Soft must start without one.
- Full Office example matrix on Windows is out of P6c scope (P0 Soft parity first).
- Accessibility / soak: P6d / P6e (shared; not Windows-specific blockers here).

---

## 6. Field Soft

Field Soft fallback applies on Windows the same as Linux: no adapter → Softbuffer present
still works. Do not drop Soft for a GPU-only Windows path.

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

All three winit hosts forward named PageUp/PageDown on press and release as
`KeyCode::PageUp` / `KeyCode::PageDown`, retaining event modifiers. Exhaustive
consumers must handle these new variants. Office key adapters map them to shared
Table page movement; Shift-aware adapters use `SelectPageUp` / `SelectPageDown`.
Windows runtime/hardware acceptance and TextArea page movement remain unverified.

## System numeric conventions

`number_conventions::NumberConventions::system()` returns an owned, thread-safe snapshot without changing the process locale. CSV import freezes this snapshot for preview and insertion.

The Windows provider reads the current user locale with GetLocaleInfoEx, including decimal/grouping/native digits and signs. ASCII exponent fallback remains a documented Qt parity gap; host feature checks do not compile or execute this target-specific branch.
