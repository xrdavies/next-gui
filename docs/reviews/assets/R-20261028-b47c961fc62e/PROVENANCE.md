# Dependent view movement evidence

Target: `b47c961fc62e18da02284a58fe24997195f11e96`, macOS arm64.

- Soft/Metal: final `NEXT_GUI_DB_BROWSER_TABLE_MOVE_QA_OUT=/tmp/next-gui-view-move-final cargo test -p e28_db_browser table_move_schema_selector_preview_failure_and_save_keep_the_draft_and_rows -- --nocapture`; actual exit0 awaited. Both themes at1400×1000/800×600, four maximum channel deltas1; contact and full small light inspected. Fixture now includes dependent views and an INSTEAD OF trigger; real post-Save queries/updates asserted.
- Native: final workspace build with `NEXT_GUI_GPU_SURFACE=1`, `NEXT_GUI_DB_BROWSER_QA_TAB=table-move-views-saved` in both themes and `table-move-views` in light. Owned ready-marker/windowID captures only. All three app exits−2 and helpers0 awaited. Contact and full saved light inspected. Captures show the renamed qualified wizard and the moved view's actual display/amount/generated columns.
- The current desktop screenshot still showed wallpaper; titlebar controls are inactive. Native evidence is static only; no OS input/IME/picker acceptance claimed.
- Example-only SQLite/state/QA change; no shared UI/theme/platform API changed, so Office catalog not repeated.
- All fixtures isolated in memory or test-owned temporary files; images contain test data only.
