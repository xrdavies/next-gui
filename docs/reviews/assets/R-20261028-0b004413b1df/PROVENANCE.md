# Table movement evidence

Target: `0b004413b1dfc6db5dffccb82c675dfc1ef6d74b` on macOS arm64.

- Soft/Metal: `NEXT_GUI_DB_BROWSER_TABLE_MOVE_QA_OUT=/tmp/next-gui-table-move-final cargo test -p e28_db_browser table_move -- --nocapture`; final actual exit0 awaited. Both themes at1400×1000 and800×600, four maximum channel deltas1. Contact and full small light inspected after responsive correction. First-row geometry asserted at both sizes.
- Native: final workspace build, `NEXT_GUI_GPU_SURFACE=1`, `NEXT_GUI_DB_BROWSER_QA_TAB=table-move` in both themes plus `table-move-saved` in light. Helper waited for the app's ready marker and its own onscreen layer0 window, captured only its windowID, then stopped and awaited every owned process (three app exits−2, helpers0). Contact and full light inspected. Source files `/tmp/next-gui-table-move-native-{dark,light}-table-move/e28_db_browser.png` and `/tmp/next-gui-table-move-native-light-table-move-saved/e28_db_browser.png`.
- Desktop screenshot remained wallpaper only; owned titlebar controls are inactive. Native evidence is static rendering only. No OS keyboard/pointer/IME/picker acceptance inferred.
- No framework widget or shared theme changed; Office catalog not repeated for this example-only change.
- All fixtures are isolated in-memory databases or test-owned temporary files. Images contain test data.
