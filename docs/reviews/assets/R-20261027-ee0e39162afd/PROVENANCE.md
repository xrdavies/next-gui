# Table definition rebuild capture provenance

Reviewed target: `ee0e39162afda5f66ec3c02d8a63b2e6dd35985b`, macOS27.0.1 arm64, 2026-10-27.

- Four genuine 1400×1000 scene pairs, dark/light × draft/saved. `NEXT_GUI_DB_BROWSER_REBUILD_QA_OUT=/tmp/next-gui-table-rebuild-reviewed cargo test --offline --locked -p e28_db_browser modify_table_controls_save -- --nocapture`; exit0 awaited. Whole-frame independent RGB maxima: dark draft2/saved2, light draft2/saved1. Both contact sheets and full light Metal draft inspected. PPM converted using sips; contacts via `/tmp/next-gui-scroll-contact.swift`. No generated or retouched UI.
- Native GPU own windows2056 dark draft /2055 light saved captured using `/tmp/next-gui-wizard-native-probe.py`, tabs table-rebuild/table-rebuilt. 1400×892 at260,48 on1920×1080 scale1. Both full captures inspected. Helpers exit0; children received SIGINT and exited-2, awaited. The QA fixture is an isolated in-memory database.
- Captures precede the final database-only existing-FK-violation and legacy-ALTER guards; no rendering/UI code changed afterward. The complete target's successful UI flows were re-tested in the final435-test workspace run. Static captures do not prove native pointer/keyboard/IME/clipboard/picker acceptance; NSApp active=false, hidden=false.
