# e28_db_browser — DB Browser for SQLite (next-gui)

**Goal:** 100% feature + UI parity with [DB Browser for SQLite](https://sqlitebrowser.org/) (DB4S), with Theme so the UI can meet or exceed DB4S visually.

See **[PARITY.md](./PARITY.md)** for the exhaustive checklist (`done` / `partial` / `todo`). There is no “limitations excused” list — gaps are tracked as parity work.

## Run

```bash
cargo run -p e28_db_browser
NEXT_GUI_DB_BROWSER_QA=1 cargo run -p e28_db_browser   # Soft PPM → /tmp/next-gui-db-browser.ppm
NEXT_GUI_DB_BROWSER_QA_TAB=browse|pragmas|sql|wizard cargo run -p e28_db_browser
```

On first launch the example opens/creates `examples/28_db_browser/sample.db` with a small demo schema.

**Chrome (DB4S-aligned):** File/Edit/View/Tools/Help menubar · New/Open/Write Changes/Revert toolbar · exactly **4** main tabs (Structure / Browse / Pragmas / Execute SQL) · right docks Edit Cell / Schema / Log / Plot (View menu) · Theme light/dark · Soft↔GPU (Space).

**Write Changes / Revert:** edits run inside a deferred transaction until Write (COMMIT) or Revert (ROLLBACK).

**Phase 2:** EditTable/EditIndex overlay wizards · CSV import wizard (+ clipboard) · per-column browse filters + multi browse tabs · Execute SQL multi-tabs / Find-Replace / Stop (interrupt) · Pragma labeled form · JSON export · Recent files · Attach Database.

Native file pickers use `rfd` (falls back when cancelled / headless).
