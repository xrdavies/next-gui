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

Execute SQL shares the current connection and pending transaction. Stop remains available while it runs. Explicit COMMIT/ROLLBACK in user SQL has its usual SQLite meaning. New/Open/Close/Exit use Save/Discard/Cancel for pending database and document edits.

Tools → Set Encryption exports a new encrypted copy; Open prompts for a key for encrypted files. Passphrases are masked and excluded from copy/accessibility labels. New `.sqbpro` projects use DB4S XML; legacy e28 JSON remains readable.

Verification: `cargo test -p e28_db_browser`, workspace tests, strict clippy and macOS feature checks. See `docs/reviews/INDEX.md` for measured results and unavailable acceptance.

Browse → New Record opens a form with Default / NULL / Value modes. Page buttons reach records beyond the first 500. Execute SQL → Results → CSV/JSON exports the full last result without rerunning SQL, including `RETURNING` results. Results use a private temporary file (plaintext, removed when the result is released), so very large queries need temporary disk space. JSON represents BLOBs/non-finite numbers as strings; use SQL dumps for a lossless backup.

Projects retain all local browse tabs, filters, sorting and page positions (additional local state uses a next_gui XML extension). Save an in-memory database with Save As before saving a project. The SQL editor supports IME composition through the shared UI; native OS input and file-picker acceptance still need a permitted desktop session.

Latest audit: [page inventory, framework capability gaps and defect list](../../docs/reviews/R-20261025-b90d5a7b4143.md). Table headers now support edge dragging; column widths and scroll positions survive rebuilds and project save/load. All client-area widgets use next-gui. Native pickers still use `rfd` directly; image decoding uses `image`, and SQLite/SQLCipher uses `rusqlite`. This is a functional prototype with remaining advanced editing, docking and responsive-layout gaps, not full DB4S parity.

Additional framebuffer QA cases: `index`, `csv`, `paste`, `attach`, `find`, `extension`, `schema`, `frozen`, `remote`, `about`, `plot`, `log`, `form`, `image`. These captures exercise rendering only.

### SQL editing (2026-10-26)

The shared next-gui TextArea supplies click/drag selection, Shift+navigation,
Select All, selected-text copy/paste, IME replacement, wheel scrolling and
undo/redo. SQL tabs retain their own caret, selection, viewport and undo history
through UI rebuilds. These editing sessions are in memory; project files still
save SQL text only.

Execute runs the selection when present, otherwise the entire script.
Cmd/Ctrl+Enter executes, Cmd/Ctrl+F opens Find, Cmd/Ctrl+Z undoes, and
Cmd/Ctrl+Shift+Z or Ctrl+Y redoes. Cmd/Ctrl+C/X/V copy, cut and paste the SQL
selection. Find displays the same editable script, selects each match, and Replace
changes the selected match. Replace All is one undoable edit. Search is literal
and case sensitive; regex/options remain future work.

`NEXT_GUI_DB_BROWSER_QA=1 NEXT_GUI_DB_BROWSER_QA_TAB=sql-long` captures an 80-line
script scrolled to a highlighted Unicode match. `find` captures the same script in
the Find panel; set `NEXT_GUI_DB_BROWSER_QA_THEME=light` for the light theme.
The earlier F01/F04 editing findings are addressed; draft transition UX and the other remaining parity findings still apply; see the
2026-10-27 follow-up below.


### Image preview (2026-10-26)

Edit Cell → Image uses the shared next-gui RGBA renderer, with transparency and
aspect-ratio fitting in Soft and GPU. PNG/JPEG pixels retain their original size;
preview decoding rejects images over 16 megapixels and limits decoder allocations
to 128 MiB. Errors leave the database value untouched. Fit/1:1/+/− controls use shared `ImageView` state; drag or wheel pans a
magnified image. Export Image saves the original encoded BLOB without re-encoding it.

`NEXT_GUI_DB_BROWSER_QA=1 NEXT_GUI_DB_BROWSER_QA_TAB=image` captures a detailed,
half-transparent 320×180 PNG through the same decoder and Image control. Add
`NEXT_GUI_DB_BROWSER_QA_THEME=light` for the light theme.

### Workflow and framework follow-up (2026-10-27)

- SQL file opening adds a tab instead of replacing the current draft. Exit checks
  SQL, DDL, project, cell and PRAGMA drafts even when the database has no pending
  writes; Cancel keeps the session, and confirmed Exit discards it. Save SQL or
  Save Project explicitly to retain drafts. New/Open/Close/Exit now share Save/Discard/Cancel. Closing one SQL tab applies
  Save/Discard only to that document and keeps other drafts/database edits.
- Edit Cell uses the shared multiline TextArea and an explicit NULL/INTEGER/REAL/
  TEXT/BLOB selector. Numeric edits preserve their type; invalid Hex/JSON is retained
  with an error. Apply or Discard Draft before changing the browse target. SQLite
  column affinity still determines the stored value, as with explicit SQL writes.
- Foreign-key navigation resolves omitted parent PK columns, groups composite keys
  and filters on the stored typed values; NULL keys report no parent. Source rows
  still require an unshadowed rowid.
- SQL tabs own results, export snapshots, messages, widths and scroll positions.
  Save as VIEW takes an editable name and the executed result statement, so editing
  the script after execution does not change the view source.
- PRAGMA writes are read back; settings that do not take effect report the actual
  value and possible earlier partial application. NONE→auto_vacuum requires an
  explicit VACUUM. Plot skips nonnumeric/non-finite values and reports query errors.
- CSV import resets the old table's filter/order/page/selection. Conditional formats
  are scoped to a table and saved with plot source, read-only mode and DDL text in
  JSON projects. Canceling Preferences does not change frozen columns.
- Shared scroll rows expose overflowing tab/action/filter strips. Nested scrolling
  starts at the inner control and bubbles at its limits; expanded container bounds
  count toward the scroll range. The shell, main pages and editing docks use these
  viewports and retain their offsets in memory. The shell now adapts at 800×600 and persists body/structure splitter ratios;
  visible scrollbars, table/filter alignment and full docking remain pending.
- SQL Log retains whole manually executed batches (up to 200 entries), including
  multiline scripts. It is still not a complete categorized SQLite trace.

These changes do not claim full DB4S compatibility or native desktop acceptance.

### DB4S project interchange (2026-10-27)

New `.sqbpro` files use the XML layout from DB4S 3.13.1. Open restores the
main database path/access mode, five connection PRAGMAs, existing attached files,
main-schema browse tabs, column/global filters, widths/primary sort, SQL text/file references and
active main/SQL tabs. Relative database, attachment and SQL paths resolve against
the project directory. Linked SQL text is read from its file before replacing the
session. Read-only projects also attach files read-only. Missing files, malformed
XML, unsupported entities and invalid settings report an error. New database
connections are prepared before replacing the current connection.

Unrecognized elements/attributes and Qt dock data survive XML saves; unsupported
advanced browse formatting/plots and attached-schema browse tabs are reported as
retained but not applied. This is partial interoperability, not complete DB4S
project acceptance. The `next_gui` XML extension retains additional local layout
and draft settings; upstream ignores it. Legacy JSON projects remain readable,
and choosing a `.json` destination writes JSON. SQLCipher keys are never added to
projects. XML text is UTF-8, limited to 16 MiB / 64 nesting levels; XML cannot
represent NUL/control characters, so those drafts require JSON. DB4S binary
round-trip acceptance has not run.

SQL tabs display file names and dirty markers, retain source-file bindings, and
support Close Tab and move arrows. Cmd/Ctrl+T/W opens/closes tabs; Cmd/Ctrl+S saves
to the bound file and Shift+S chooses a new destination. Saving rejects a bound
file changed externally. The shared three-action modal traps focus and supports
Tab/Shift+Tab, arrows, Enter/Space and Escape. The desktop theme uses compact
consistent text and restrained surfaces; macOS missing symbols use a fallback
font. Pointer tab dragging, rename and independent floating docks remain pending.

### Filtering and scrolling (2026-10-27)

Column filters support DB4S comparisons (`>=2`, `=NULL`, `<>NULL`, `=''`),
ordered numeric ranges (`10~20`), LIKE wildcards and `/PCRE2 pattern/` regular
expressions. “Filter in any column” searches each space-separated term across
columns; all terms must match somewhere. Enter applies filters, Clear Filters
resets them, and Cmd/Ctrl+Alt+F focuses global search. The separate WHERE field
accepts SQL. Full CSV/JSON export uses the same filter conditions.

Filter inputs track resized, scrolled and frozen columns. Shared overlay bars
support thumb dragging and track paging; Tab focus reveals controls in enclosing
scroll areas. Hidden docks return their width to the workspace. REGEXP uses
Unicode properties, lookaround and backreferences, bounded to 1,000,000 match
steps / depth 1,000. Escape preferences and automatic filter debounce remain
pending. Project loading still needs to report invalid restored query errors;
see the [latest review](../../docs/reviews/R-20261027-2bfc8ad51380.md).
