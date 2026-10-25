# e28_db_browser — DB Browser for SQLite (next-gui)

A local SQLite browser example inspired by [DB Browser for SQLite](https://sqlitebrowser.org/) (DB4S), using next-gui themes, SQLCipher and native file dialogs. Full upstream compatibility remains a goal, not a completion claim.

See **[PARITY.md](./PARITY.md)** for the implementation checklist (`done` / `partial` / `todo`). Remaining compatibility gaps are marked explicitly.

## Run

```bash
cargo run -p e28_db_browser
NEXT_GUI_DB_BROWSER_QA=1 cargo run -p e28_db_browser   # Soft PPM → /tmp/next-gui-db-browser.ppm
NEXT_GUI_DB_BROWSER_QA_TAB=browse|pragmas|sql|insert|wizard|cipher|prefs|exportsql|cond cargo run -p e28_db_browser
```

On first launch the example opens/creates `examples/28_db_browser/sample.db` with a small demo schema.

**Chrome (DB4S-aligned):** File/Edit/View/Tools/Help menubar · New/Open/Write Changes/Revert toolbar · exactly **4** main tabs (Structure / Browse / Pragmas / Execute SQL) · right docks Edit Cell / Schema / Log / Plot (View menu) · Theme light/dark · Soft↔GPU (Space).

**Write Changes / Revert:** edits run inside a deferred transaction until Write (COMMIT) or Revert (ROLLBACK).

**Phase 2:** EditTable/EditIndex overlay wizards · CSV import wizard (+ clipboard) · per-column browse filters + multi browse tabs · Execute SQL multi-tabs / Find-Replace / Stop (interrupt) · Pragma labeled form · JSON export · Recent files · Attach Database.

Native file pickers use `rfd`. Cancel means no operation.

Save As uses a live SQLite backup (including WAL), commits pending edits and requires a new destination. SQL dumps export all rows with their original SQLite types. CSV/JSON export the complete filtered table; the grid displays 500-row pages. Use SQL dumps for a type-preserving round trip.

Execute SQL shares the current connection and pending transaction. Stop remains available while it runs. Explicit COMMIT/ROLLBACK in user SQL has its usual SQLite meaning. New/Open refuses to replace a database with unwritten edits.

Tools → Set Encryption exports a new encrypted copy; Open prompts for a key for encrypted files. Passphrases are masked and excluded from copy/accessibility labels. Project files are example-specific JSON, not upstream DB4S XML.

Verification: `cargo test -p e28_db_browser`, workspace tests, strict clippy and macOS feature checks. See `docs/reviews/INDEX.md` for measured results and unavailable acceptance.

Browse → New Record opens a form with Default / NULL / Value modes. Page buttons reach records beyond the first 500. Execute SQL → Results → CSV/JSON exports the full last result without rerunning SQL, including `RETURNING` results. Results use a private temporary file (plaintext, removed when the result is released), so very large queries need temporary disk space. JSON represents BLOBs/non-finite numbers as strings; use SQL dumps for a lossless backup.

Project JSON retains all browse tabs, filters, sorting and page positions. Save an in-memory database with Save As before saving a project. The SQL editor supports IME composition through the shared UI; native OS input and file-picker acceptance still need a permitted desktop session.

Latest audit: [page inventory, framework capability gaps and defect list](../../docs/reviews/R-20261025-b90d5a7b4143.md). Table headers now support edge dragging; column widths and scroll positions survive rebuilds and JSON project save/load. All client-area widgets use next-gui. Native pickers still use `rfd` directly; image decoding uses `image`, and SQLite/SQLCipher uses `rusqlite`. This is a functional prototype with known editor, navigation and layout defects, not full DB4S parity.

Additional framebuffer QA cases: `index`, `csv`, `paste`, `attach`, `find`, `extension`, `schema`, `frozen`, `remote`, `about`, `plot`, `log`, `form`. These captures exercise rendering only.
