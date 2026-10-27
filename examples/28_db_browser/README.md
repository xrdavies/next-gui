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

Switching structure objects protects edited DDL: keep it in a new SQL tab, discard only that draft, or cancel the selection. Other SQL drafts and pending database changes stay in the session.

The structure tree groups objects under main, temp and attached databases, including empty attachments. Selection, definition previews and confirmed deletion preserve the database identity, including quoted names. Projects retain the selected object's identity with the DDL text. Browse tabs/selectors, record navigation, filters, conditional formats, typed editing/insert, row deletion, cut/paste, FK navigation and full filtered export preserve the database and table identity. Copy as SQL qualifies the target database. Plot selectors and numeric samples also preserve database identity; structured modify wizards still address main; attached/temp definitions use a read-only preview and qualified SQL in Execute SQL.

Structure navigation retains focus, expanded groups and scroll position across page/UI rebuilds. Keyboard selection reveals the active object; expansion state follows database names when an attachment is detached. These view settings last for the current database session.

**Write Changes / Revert:** edits run inside a deferred transaction until Write (COMMIT) or Revert (ROLLBACK).

**Phase 2:** EditTable/EditIndex overlay wizards · CSV import wizard (+ clipboard) · per-column browse filters + multi browse tabs · Execute SQL multi-tabs / Find-Replace / Stop (interrupt) · Pragma labeled form · JSON export · Recent files · Attach Database.

Native file pickers use `rfd`. Cancel means no operation.

Save As uses a live SQLite backup (including WAL), commits pending edits and requires a new destination. It rejects cell drafts and clears disconnected temp/attached browse results when the connection changes. SQL dumps export all rows with their original SQLite types. CSV/JSON export the complete filtered table; the grid displays 500-row pages. Use SQL dumps for a type-preserving round trip.

Execute SQL shares the current connection and pending transaction. Stop remains available while it runs. Explicit COMMIT/ROLLBACK in user SQL has its usual SQLite meaning. New/Open/Close/Exit use Save/Discard/Cancel for pending database and document edits.

Tools → Set Encryption exports a new encrypted copy; Open prompts for a key for encrypted files. Passphrases are masked and excluded from copy/accessibility labels. New `.sqbpro` projects use DB4S XML; legacy e28 JSON remains readable.

Verification: `cargo test -p e28_db_browser`, workspace tests, strict clippy and macOS feature checks. See `docs/reviews/INDEX.md` for measured results and unavailable acceptance.

Browse → New Record opens a form with Default / NULL / Value modes. Page buttons reach records beyond the first 500. Execute SQL → Results → CSV/JSON exports the full last result without rerunning SQL, including `RETURNING` results. Results use a private temporary file (plaintext, removed when the result is released), so very large queries need temporary disk space. JSON represents BLOBs/non-finite numbers as strings; use SQL dumps for a lossless backup.

Projects retain all local browse tabs, filters, sorting and page positions (additional local state uses a next_gui XML extension). Save an in-memory database with Save As before saving a project. The SQL editor supports IME composition through the shared UI; native OS input and file-picker acceptance still need a permitted desktop session.

Project opening preflights the restored browse tabs, including WHERE/REGEXP and
empty-table patterns. Invalid settings leave the previous connection and SQL tabs
open and report the offending browse tab. Shared rounded-control clipping now keeps
the original shape at viewport edges in Soft and GPU.

Latest audit: [page inventory, framework capability gaps and defect list](../../docs/reviews/R-20261025-b90d5a7b4143.md). Table headers now support edge dragging; column widths and scroll positions survive rebuilds and project save/load. All client-area widgets use next-gui. Native pickers still use `rfd` directly; image decoding uses `image`, and SQLite/SQLCipher uses `rusqlite`. This is a functional prototype with remaining advanced editing, docking and responsive-layout gaps, not full DB4S parity.

Additional framebuffer QA cases: `index`, `csv`, `paste`, `attach`, `find`, `extension`, `schema`, `frozen`, `remote`, `about`, `plot`, `log`, `form`, `image`. These captures exercise rendering only.

### SQL editing (2026-10-26)

The shared next-gui TextArea supplies click/drag selection, Shift+navigation,
Select All, selected-text copy/paste, IME replacement, wheel scrolling and
undo/redo. SQL tabs retain their own caret, selection, viewport and undo history
through UI rebuilds. These editing sessions are in memory; project files still
save SQL text only.

Function hints show connection-aware signatures, parameter names and descriptions.
Use the hint's arrows to browse overloads. Cmd/Ctrl+Shift+Space opens a dismissed
hint or cycles to the next overload. Long hint text scrolls with the wheel over
the hint or Cmd/Ctrl+Shift+PageUp/PageDown. The SQL editor keeps focus, selection
and undo history; ordinary PageUp/PageDown still navigates the document.

Option+Left/Right on macOS (Ctrl+Left/Right on Windows/Linux) moves by word in
the SQL editor; Shift extends the selection. The same modifier with Backspace
or forward Delete removes the selection or one word in that direction, as one
undoable edit. Identifier runs include Unicode letters and underscores.

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
  visible scrollbars and table/filter alignment are implemented; full docking remains pending.
- SQL Log traces SQLite statements/triggers with categorized full text; see the
  SQL log section below for retention, redaction and remaining diagnostic work.
- The status bar reads the database's actual encoding, including UTF-16le/be,
  and retains it while SQL runs. Structure toolbar controls fit vertically at
  normal and narrow widths.

These changes do not claim full DB4S compatibility or native desktop acceptance.

### DB4S project interchange (2026-10-27)

New `.sqbpro` files use the XML layout from DB4S 3.13.1. Open restores the
main database path/access mode, five connection PRAGMAs, existing attached files,
main/attached browse tabs, per-database table settings, column/global filters, widths/sort, SQL text/file references and
active main/SQL tabs. Relative database, attachment and SQL paths resolve against
the project directory. Linked SQL text is read from its file before replacing the
session. Read-only projects also attach files read-only. Missing files, malformed
XML, unsupported entities and invalid settings report an error. New database
connections are prepared before replacing the current connection.

Unrecognized elements/attributes and Qt dock data survive XML saves; unsupported
advanced browse formatting/plots are reported as retained but not applied.
Temporary table contents belong to the original connection; a missing temporary
browse target reopens as an empty tab with a notice. In-memory attachments cannot
be saved in a project. This is partial interoperability, not complete DB4S
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
pending. Project loading preflights restored queries and reports errors while
preserving the previous session; see the [recovery/clipping review](../../docs/reviews/R-20261027-2b16b7cb2279.md).

### SQL log (2026-10-27)

The dock shows User, Application, Error Log or All. It records complete submitted
batches and SQLite statement/trigger events, including ordinary bound values as
in DB4S. The read-only text view supports selection, copy, scrolling and live updates
while SQL runs. Clear and Export apply to the selected category; All applies to
all categories. Export waits for running SQL so open database paths remain protected.

Each category retains 200 entries, with an always-visible eviction count. Full
retained text is exported atomically; individual SQL text is not shortened.
SQLCipher key/rekey/hex-key statements, ATTACH KEY and key-bearing file URIs are
redacted before entering history. The global SQLite diagnostic callback is not
connected yet; the Error category contains application/execution failures.

`NEXT_GUI_DB_BROWSER_QA=1 NEXT_GUI_DB_BROWSER_QA_TAB=log` captures the log dock;
`NEXT_GUI_DB_BROWSER_QA_THEME=light` selects the light theme.

### SQL search (2026-10-27)

Find / Replace has Match case, Whole words, Wrap around, Backwards, Selection only
and Regular expression options. Find All marks all matches without changing the
selection. F3 follows the current direction; Shift+F3 reverses it once. Replace
acts on a matching selection, and Replace All is one undoable edit.

Selection only keeps its original range through subsequent matches/replacements.
Manual document changes require choosing the range again; switching SQL tabs or
restoring projects resets the search session. The compact 800×600 panel temporarily
hides the extra/project toolbars so the editor remains visible, restoring their
settings when closed.

Regex uses the existing Unicode PCRE2 engine: capture replacements `\0`–`\9`,
`\n`, `\r`, `\t` and `\\` escapes; literal mode keeps replacement text literal.
Invalid expressions, split Unicode captures or exhausted limits leave the document
unchanged. Match/depth limits are 1,000,000/1,000 and each scan has a one-second
budget. Forward navigation stops at its first match. Large backward overlap scans
still need a cancellable worker; full Scintilla regex dialect/native acceptance
is not established. The six options and transient search scope are session state,
not additional project XML settings.

### Browse row selection (2026-10-27)

The shared Table supports a contiguous row range with Shift+Up/Down. The active
range remains selected through e28 rebuilds and is copied as TSV with Cmd/Ctrl+C;
copying a text editor still uses the existing Unicode selection path. Cell-level
ranges, TSV paste/bulk updates, inline editing, column hide/reorder and total-count
navigation remain open parity work.
