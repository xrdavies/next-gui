# Capture provenance

- `backwards-selection-15d4a96.png`: Soft framebuffer produced by the backwards
  selection regression in the worktree committed as 15d4a96. Shows a real reversed
  selection after repeated Shift steps and rebuilds. Not a native screenshot.
- `pointer-selection-5e23229.png`: exact target 5e23229548d4.
  `NEXT_GUI_DB_BROWSER_QA_OUT=/tmp/next-gui-pointer-selection-5e23229.ppm cargo test --offline --locked -p e28_db_browser interaction_tests::browse_drag_commits_selection_after_release_and_keeps_the_editor_value -- --exact`; converted with sips. Soft framebuffer, not OS input acceptance.
- `native-metal-141a9ab.png`: actual window capture at 141a9ab before the storage-only
  spill change, after `NEXT_GUI_GPU_SURFACE=1 NEXT_GUI_DB_BROWSER_QA_TAB=browse target/debug/e28_db_browser`.
  macOS window 1338, own PID 54401, captured with screencapture. Startup logged
  1400x860 backend=GPU. The process was stopped afterwards. This proves a Surface
  rendered; no pointer/keyboard, clipboard, IME or picker action was performed.

All three images were opened and inspected. Full-display capture returned wallpaper
or black, so interaction acceptance was not inferred from a window-only image.

- `backwards-selection-15d4a96.png`: SHA-256 `47a5f2cfa9cf1f4a4234183273c074c13de04172d86b797912b120d971a6c680`
- `pointer-selection-5e23229.png`: SHA-256 `3d794813b593c9ac0453357818906307f4278b465d3aa381b892802acf8d07dd`
- `native-metal-141a9ab.png`: SHA-256 `8f44d70ed02903398d928e6ecf20f3ac5147e01a9a889f6e55893018b2372226`
