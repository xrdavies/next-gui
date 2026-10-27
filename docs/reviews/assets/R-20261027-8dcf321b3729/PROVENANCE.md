# Column rename capture provenance

Target: `8dcf321b3729d7957040ecd8b8a33857258d656a`, macOS 27.0.1 arm64, 2026-10-27.

- Four real scene pairs at 1400×1000: dark/light × draft/saved. `NEXT_GUI_DB_BROWSER_RENAME_QA_OUT=/tmp/next-gui-column-rename cargo test --offline --locked -p e28_db_browser modify_table_tracks -- --nocapture`; process exit0 awaited. Independent full-frame PPM RGB deltas: dark draft2/saved2, light draft2/saved1. All eight readbacks skimmed via both contact sheets; full light draft inspected. Draft captures retain the status of an intentionally rejected reorder, followed by restoring column order and saving the rename.
- PPM→PNG via macOS `sips`; contact sheets via `/tmp/next-gui-scroll-contact.swift`. No generated or retouched product content. `metal-light-saved.png` is the complete readback.
- Native GPU own-window captures via `/tmp/next-gui-wizard-native-probe.py`, QA tabs `table-rename` (dark draft) / `table-renamed` (light saved). Window IDs2042/2045, bounds1400×892 at260,48, scale1 on1920×1080. Both full screenshots inspected. Child apps received SIGINT and were awaited with exit -2; both helpers exit0. NSApp active=false, hidden=false: static render coverage, no native input acceptance.
- The example keeps changes unwritten until Write Changes. Native capture children exited without committing the fixture rename.
