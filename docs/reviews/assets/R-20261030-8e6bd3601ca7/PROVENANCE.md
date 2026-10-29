# Conditional format manager evidence

- Code target: `8e6bd3601ca7c33fe9e1dce4dcb3632514e3b762`; macOS27.0.1 arm64, Rust1.86.0.
- RAM fixtures only: main/temp/attached same-named sources, retained pending inserts,
  overlapping background rules, project serialization and source/rule stale guards.
- Commands: `cargo --offline test --workspace`, `cargo --offline clippy --workspace --all-targets -- -D warnings`, `cargo --offline build --workspace`, and host
  `cargo --offline check -p next-gui-platform --features macos` / `windows`.
- Final raw logs/scripts: `/tmp/next-gui-cond-manager-v1-{checks.py,checks.json,*-final.log}`.
  All five commands exited0. The full run contains810 tests across43 groups;
  e28 has515 tests and shared UI has221. No failed or ignored tests.
- Seventeen whole-window Soft/actual Metal P6 pairs were captured by
  `NEXT_GUI_FILTER_QA_OUT=/tmp/next-gui-cond-manager-v1-render cargo --offline test -p e28_db_browser cond_format::tests -- --nocapture`.
  `/tmp/next-gui-cond-manager-v1-{render-pipeline.py,convert.py}` writes PNG/contact sheets
  and asserts whole-frame RGB deltas. Maximum1; no channel above2. Both contact sheets
  and full-size dark normal/narrow frames were inspected. Raster tests use13px chrome.
- Eight native GPU windows: dark/light ×1400×860 /800×600 ×Save/Cancel.
  Native narrow fixtures use18px chrome; wide13px. Existing table typography is13px.
  Private `CGEventSource` keys posted to each owned PID replace2 with3, Undo/Redo,
  tab to Move Down, reorder, tab to Save/Cancel and activate. Six stages per window
  (48 total) were captured with `screencapture -l`; all four contact sheets and full-size
  dark wide/narrow before/after/reordered frames were inspected.
- `/tmp/next-gui-cond-manager-v1-{native-all.py,native-keys.py,native-key.swift}`
  waits for ready owned windows, captures each stage and awaits every child exit(-2).
  `/tmp/next-gui-cond-manager-v1-{colors.swift,metrics.py}` asserts native saved blue
  area >1.5×canceled blue area, canceled red area >2×saved red area and all three
  rule colors present, for both themes and sizes. This checks the saved priority
  effect on real painted cells; screenshots show canceled edits retain old matches.
- 158 source SHA256 values and browser binary
  `0396a5ad6b2354f29b79af389208a751ae858d165f229b7c65442df4577d5035`
  were frozen before final checks and verified unchanged after QA. Hashes in metrics.
- Development attempts failed on a generic Select iterator, test borrow ordering,
  replacement of a dirty fixture, stale test catalog after ATTACH, the button pointer
  dual-path expectation, a missing manager body scroll area, and a QA focus block
  placed before UI construction. These were corrected before the frozen final run.
- Original manager `.cpp/.ui/.h` were read at v3.13.1 from official GitHub raw files;
  Git blob SHA1 matched the pinned official tree. Original supports full filter syntax,
  foreground/background/font/size/B/I/U/alignment and per-column priority; this change
  covers the existing five background-rule operators only.
- Shared UI code was unchanged; fresh Office captures were not run for this app-only
  scope. Native ordinary frontmost input, system IME, a11y, pickers/Print and Linux/
  Windows runtime acceptance remain not_run. Prior Mob desktop capture was wallpaper
  only. Private-PID keys and owned-window captures do not establish ordinary input.
- No remote push was attempted after the earlier Mob approval timeout denial.
