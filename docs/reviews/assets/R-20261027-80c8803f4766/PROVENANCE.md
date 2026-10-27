# Browse setting refresh evidence

Code: `80c8803f4766ecc9ab3529b5b3e368f1e49c0c5a`, macOS arm64.

- `browse-options-soft.png`: existing per-table UI regression, `NEXT_GUI_DB_BROWSER_QA_OUT=/tmp/next-gui-browse-schema-remap-static.ppm cargo test --offline --locked -p e28_db_browser per_table_filters_sort_and_widths_restore_in_each_tab_and_project_roundtrip -- --nocapture`; Soft PPM converted with sips, inspected at full size. Shows restored filters, sort and widths; this is not the removed-column fixture.
- `browse-attached-native.png`: fresh built binary, `NEXT_GUI_GPU_SURFACE=1`, `NEXT_GUI_DB_BROWSER_QA_TAB=browse-namespaces`. Own PID queried through CGWindow, window 1925, on-screen logical 700x462 at 610,156, scale 2. NSApp active=false/hidden=false. Actual GPU window captured with screencapture and inspected full size. Helper awaited owned child after SIGINT, return code -2; helper exit 0. Static namespace scene; no pointer/keyboard/IME/clipboard/picker acceptance.

Deleted-column behavior is covered by data/state/project tests, not inferred from these static images. No shared widget/layout/painter change, so no fresh Office-wide run or Soft/Metal delta claim.
