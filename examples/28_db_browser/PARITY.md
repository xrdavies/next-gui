# DB Browser for SQLite (DB4S) ↔ e28 parity checklist

**Goal:** 100% feature + UI parity with [DB Browser for SQLite](https://sqlitebrowser.org/) (sqlitebrowser / DB4S), with next-gui Theme so the UI can meet or exceed DB4S visually. Items marked `partial` or `todo` are remaining compatibility work. This is an example, not a certified DB4S replacement.

Reference: upstream **v3.13.1** `src/MainWindow.ui`, EditTableDialog, EditIndexDialog, ImportCsvDialog, ExportSqlDialog, CipherDialog, PreferencesDialog, FindReplaceDialog, PlotDock, EditDialog, TableBrowser.

Status legend: `done` = implemented local subset, not upstream parity or native acceptance; `partial` = known missing or defective behavior; `todo` = absent.

**Current audit:** [2026-10-25 page inventory, framework gaps and reproducible findings](../../docs/reviews/R-20261025-b90d5a7b4143.md). This supersedes earlier completeness claims. Source and Soft framebuffer review covered four main pages, five docks and all 13 overlay kinds; native OS input acceptance remains unavailable.

The earlier “done 101 / partial 0” claim was not supported by acceptance tests. The review below distinguishes implemented local workflows from upstream compatibility. See the 2026-10-25 review in `docs/reviews/INDEX.md`.

---

## Shell chrome

| Surface | Status | Notes |
|---------|--------|-------|
| Menubar File / Edit / View / Tools / Help | partial | Flat in-window menus; no native menu, nested submenus or application shortcut system. |
| Main toolbar: New, Open, Write Changes, Revert | done | + Theme (exceeds DB4S); View-toggleable |
| Extra DB / Project toolbars | partial | Local subset; no upstream movable toolbar/overflow behavior. |
| Main tabs (exactly 4): Structure / Browse / Pragmas / Execute SQL | done | Charts→Plot dock; SQL Log→Log dock |
| Right docks: Edit Cell / DB Schema / SQL Log / Plot / Remote | partial | One tabbed right pane; no independent floating/docking panes or persisted splitter positions. |
| Status bar: path, encoding, read-only, encryption, busy | partial | Encoding is hardcoded UTF-8 instead of read from PRAGMA encoding. |
| Soft↔GPU toggle | done | Space; exceeds DB4S |
| Theme light/dark | done | exceeds DB4S; Preferences default theme |

## File menu

| Action | Status | Notes |
|--------|--------|-------|
| New Database… | done | native `rfd` |
| New In-Memory Database | done | Isolated connection; same connection moves to the SQL worker |
| Open Database… | done | `rfd` |
| Open Database Read Only… | done | `open_readonly` + write blocked |
| Attach Database… | done | overlay: path + schema name → ATTACH |
| Recently opened | done | last 5 paths persisted (`.e28_recent`) |
| Close Database | done | confirm modal |
| Save Database As… | done | SQLite live backup includes WAL and in-memory data; new destination required; commits pending edits first; encrypted databases use encrypted export |
| Import → Database from SQL file… | done | Atomic savepoint import; dump transaction wrappers cannot commit prior edits |
| Import → Table from CSV file… | done | UTF-8, quoted multiline fields, strict field counts, atomic insert |
| Import → Table from CSV clipboard… | done | PasteCsv overlay |
| Export → Database to SQL file… | done | Uncapped typed rows, generated columns, triggers after data, AUTOINCREMENT sequence; virtual tables fail explicitly |
| Export → Table(s) as CSV… | partial | Complete current filtered table; no multi-table chooser; BLOB as hex; CSV does not preserve NULL/type distinctions |
| Export → Table(s) to JSON… | partial | Complete current filtered table; typed numbers/null, BLOB hex string; no multi-table chooser |
| Write Changes | done | COMMIT + re-BEGIN |
| Revert Changes | done | ROLLBACK + re-BEGIN |
| Compact Database (VACUUM) | done | |
| Exit | partial | DB writes guarded; unsaved SQL/editor drafts are not guarded (F02). |
| Open Project… | partial | Example JSON only; Unicode and malformed-input validation; upstream DB4S XML is rejected |
| Save Project / Save Project As… | partial | Example JSON; all browse tabs, filters/sort/pages plus SQL tabs/docks/toolbars; relative database paths resolved from project; no upstream XML compatibility |
| Close Project | partial | Clears project path only; no unsaved project/SQL state prompt (F02). |
| Save All | done | Prompt for project destination if missing; cancellation has no fallback write; commit DB and save JSON project |
| Print Schema… | partial | Exports schema text to a file; no print dialog or print preview. |

## Edit menu

| Action | Status | Notes |
|--------|--------|-------|
| Create Table… | partial | Basic column PK/NN/Unique/Default; full DB4S constraint editor remains |
| Create Index… | partial | Basic name/table/raw column SQL/unique; no structured order/expression/partial-index editor. |
| Modify Object… | partial | Native ALTER preserves constraints/triggers/indexes; definition changes/reordering and expression/partial indexes require explicit SQL |
| Delete Object… | done | confirm + DROP |
| Preferences… | partial | Five options only; project does not retain conditional rules/plot settings; canceled frozen-column edits can leak (F11). |

## View menu

| Action | Status | Notes |
|--------|--------|-------|
| Toggle Edit Database Cell dock | done | |
| Toggle DB Schema dock | done | |
| Toggle SQL Log dock | done | |
| Toggle Plot dock | done | |
| Toggle Remote dock | partial | Local path list only; no DB4S remote service |
| Toggle DB / Extra / Project / SQL toolbars | done | four independent View toggles |

## Tools menu

| Action | Status | Notes |
|--------|--------|-------|
| Set Encryption… (SQLCipher) | done | Open encrypted DB after key prompt; separate encrypted-copy export with validated page/KDF settings; key input masked |
| Load Extension… | done | overlay path + optional entry → `load_extension` |
| Integrity Check | done | → Execute SQL results |
| Quick Check | done | |
| Foreign Key Check | done | |
| Optimize | done | `PRAGMA optimize` |
| Row Counts | done | query into Execute SQL |

## Help menu

| Action | Status | Notes |
|--------|--------|-------|
| About | done | About overlay (version / SQLCipher linked) |
| Website / Wiki / SQLCipher FAQ / Bug / Feature / Donate / Updates | partial | Native opener; Updates is a download link/status, no update check |

## Database Structure tab

| Feature | Status | Notes |
|---------|--------|-------|
| Structure toolbar: New Table, New Index, Modify, Delete, Refresh | done | wizards + Modify wired |
| Schema tree Tables/Views/Indexes/Triggers | partial | Main schema only; no attached/temp schema navigation, column/detail tree or object context actions. |
| Object SQL preview | done | |
| Run DDL | partial | Synchronous execution on UI thread; long DDL can block interaction. |
| Print schema | done | File → Print Schema… |

## Browse Data tab

| Feature | Status | Notes |
|---------|--------|-------|
| Table selector | done | |
| Filter / WHERE | done | |
| Data grid | partial | 500-row pages; shared column resize/clip/scroll fix added. No range selection, bulk copy/paste, visible scrollbars, inline editor, hiding/reorder or total-count navigation. |
| Header-click sort | partial | Scroll-aware hit testing and literal identifiers fixed; single-column sort only, no clear/multi-sort UI. |
| Filter-per-column | partial | LIKE-substring inputs in a separate fixed row; not aligned with resized/scrolled columns, no DB4S operator parser. |
| Multi browse tabs | partial | Add/switch only; no close, reorder or overflow. Many tabs exceed available width. |
| Add / Delete row | done | New Record form supports Value/NULL/Default and required fields; Delete uses unshadowed rowid |
| Edit cell → Edit Cell dock | partial | Typed reads for rowid tables; edited numeric values become TEXT on no-affinity/BLOB columns, NULL/empty distinction cannot be chosen (F06). |
| Frozen columns | done | `Table::{frozen_cols,scroll_x}` + Freeze toggle |
| Conditional formatting | partial | Background only; rules global by column name, cannot edit/remove, not saved in projects (F11). |
| FK click-navigation | partial | Fails on implicit parent PK; ignores composite-key grouping and typed values (F05). |
| Record / form view | partial | Page-local record navigation; fields are unscrollable rows and overflow for wide schemas (F10). |

## Edit Pragmas tab

| Feature | Status | Notes |
|---------|--------|-------|
| Scroll form of pragmas | partial | Actually a fixed Column with ten PRAGMA keys; no scroll container or full upstream setting list. |
| Save / Apply pragmas | partial | auto_vacuum may remain unchanged while status says applied (F03); values need read-back validation. |
| Bool / Select editors | done | journal_mode / synchronous / FK switches |

## Execute SQL tab

| Feature | Status | Notes |
|---------|--------|-------|
| SQL editor | partial | Shared TextArea now has pointer/drag and keyboard selection, scrolling, undo/redo, IME replacement and rebuild state (F01 addressed). Syntax, completion, line numbers and grapheme-aware navigation remain. |
| Execute / message / results table | partial | Only final statement preview, at most 500 displayed rows; one global result across SQL tabs (F09). |
| Toolbar: Open Tab, Open/Save file, Execute, Stop, Find | partial | Execute uses selection or whole script; Cmd/Ctrl+Enter/F and editing shortcuts wired. Open SQL still replaces unsaved draft without prompt (F02). |
| Multi SQL tabs | partial | Add/switch only; drafts and editing sessions separated, results not; no close/reorder/overflow or dirty markers. |
| Find / Replace | partial | Shared editor in Find panel highlights/reveals matches; Replace acts on the selected match and Replace All is undoable (F04 addressed). Literal case-sensitive search only. |
| Async query + cancel | done | Live connection moves to worker; Write/Revert/keys/ATTACH shared; completion polled on redraw; Stop interrupts |
| Results → CSV / JSON / Save as view | partial | Full snapshot export works; Save as VIEW uses current editor and a fixed name, not the displayed result query (F09). |
| Toggle comment | partial | All physical lines only; no selected-block handling. Removing comments strips all leading hyphens, not exactly two. |
| Execute current line | done | Uses Unicode caret position; never replaces editor contents; one physical line |

## Docks

| Dock | Status | Notes |
|------|--------|-------|
| Edit Database Cell | partial | Single-line TextInput for Text/Hex/JSON; no Set NULL, import/export, XML/RTL/evaluation or full binary editor (F06). |
| DB Schema | partial | Flat main-schema list; no column tree, filter or SQL insertion. |
| SQL Log | partial | Last 200 manually logged strings; async batches log first line only, no all-SQL trace/category filters/export (F13). |
| Plot | partial | One numeric-column trend, first 64 samples; text becomes zero and query errors are swallowed (F07); no X/Y1/Y2 or query result source. |
| Remote | partial | Local path list; cloud protocol/auth/sync not implemented |

## Dialogs

| Dialog | Status | Notes |
|--------|--------|-------|
| EditTableDialog (full wizard) | partial | Basic create plus safe native ALTER subset; arbitrary rebuild migrations excluded |
| EditIndexDialog | partial | Basic raw columns entry/unique; no partial clause or structured column order UI. |
| ImportCsvDialog (delimiter, quote, encoding, …) | partial | Strict UTF-8 only; multiline quoting supported |
| ExportSqlDialog (options) | partial | Four switches, all tables; no multi-table selection, IF NOT EXISTS mode, INSERT batching or original-CREATE choice. |
| CipherDialog | partial | Unlock/export copy with password/page/KDF only; no rekey/remove encryption, raw key, HMAC/KDF algorithms or SQLCipher 3 preset. |
| PreferencesDialog | partial | Only font/theme/drop confirmation/CSV delimiter/frozen count; full upstream preferences absent. |
| FindReplaceDialog | partial | Visible match selection, correct current-match replacement and undo; no regex/case/word/backwards/selection-only search controls. |
| EditDialog (blob/image/hex) | partial | Basic text/hex/JSON and 24-cell mosaic image preview; no full raster/image controls (F12). |
| Load Extension | done | OverlayKind::LoadExtension |
| About | done | OverlayKind::About |

## Data / engine

| Feature | Status | Notes |
|---------|--------|-------|
| rusqlite bundled SQLite | done | + `hooks` + `load_extension` + `bundled-sqlcipher-vendored-openssl` |
| Deferred Write Changes / Revert (txn) | done | BEGIN on open; dirty flag |
| SQLCipher | partial | Core unlock/encrypted export tested; upstream compatibility controls/rekey/decrypt absent. |
| Extensions | done | Load Extension… |
| In-memory DB | done | Isolated connection, shared with worker by ownership transfer |
| ATTACH DATABASE | partial | Usable from SQL on the live connection; attached-schema tree/browse selector remains |

---

## Phase map

- **Phase 1:** chrome parity (4 tabs + docks + menubar + Write/Revert + rfd + header sort).
- **Phase 2:** EditTable/Index wizards, CSV import options + clipboard, filter-per-column, multi-browse tabs, Find/Replace, multi SQL tabs + Stop interrupt, pragma form, JSON export, Recent, Attach.
- **Phase 3:** SQLCipher UI, `.sqbpro` projects, frozen columns + conditional formatting + FK nav, Edit Cell Text/Hex/JSON/Image, ExportSql options, Preferences, Execute Line / toggle comment / Save as VIEW, multi toolbar View toggles, `FlexBuilder::dialog_panel`.
- **Phase 4:** Save All, Load Extension, Help URLs/About, Print Schema, Record/form view, Remote dock, SQLCipher linked build, true cell background fills, Edit Cell image decode (`image` 0.24) and shared Image preview, overlays on `dialog_panel`, status bar RO/encoding/encryption/busy polished.
- **Phase 4b (visual QA):** Table aux stride bug (only last row painted); Tabs recursive hide (Pragma bleed); Menu popup width; light clear/menubar; Remote dock default on; SQL log ellipsis; Plot clip/padding; Edit Cell col sync; Form toggle; Comment button label.

## Framework / external notes

- Overlays use `FlexBuilder::dialog_panel` (title + body + OK/Cancel) — dual Modal chrome reduced.
- Table `cell_bg` aux nodes provide true per-cell background fills.
- SQLCipher linked via `rusqlite` feature `bundled-sqlcipher-vendored-openssl` (verified `sqlcipher_linked` + `sqlcipher_key_roundtrip` tests).
- Image decoding uses `image` 0.24; rendering uses next-gui `Image`, currently a 24-cell mosaic rather than a full RGBA blit (F12).
- Remote currently means a local-path connection list. Cloud compatibility remains unimplemented; no claim about the upstream service license or protocol is made.

## 2026-10-25 verification and boundaries

- Live WAL/memory Save As, more than 500 exported rows, NULL/empty text/NUL/BLOB/numbers, trigger order and sequence state have regression tests.
- SQL batch parsing uses rusqlite/SQLite statement boundaries. Async execution uses the live connection. Explicit transaction commands in Execute SQL retain SQLite semantics. Interrupting a write can roll back the transaction; the error now states this.
- CSV import and index/table changes use savepoints. New databases and Save As never truncate an existing file. File-dialog cancellation has no implicit `/tmp` output.
- Project/prefs parsing uses serde JSON; project writes and database dumps publish their completed temporary file.
- macOS Soft screenshots cover all main pages and major dialogs. OS pointer/keyboard/file-picker acceptance is blocked by desktop capture permissions. Native Metal startup alone is not visual acceptance.
- DB4S XML, cloud sync, arbitrary constraint migrations, multi-table export and attached-schema browse navigation remain compatibility work. Native IME acceptance requires desktop permissions.

## Local workflow completion (2026-10-25)

- Browse pages carry real rowids; page/sort/refresh clears stale edit selection. New Record has a scrollable form for default, NULL or supplied values.
- SQL result export streams the full final result to a private temporary snapshot, then exports CSV/JSON without rerunning statements. The grid remains capped at 500 rows. Result snapshots are private plaintext files and consume temporary disk space until replaced/closed. BLOBs and non-finite numbers use strings in JSON; SQL dumps remain the lossless format.
- Dirty state comes from SQLite transactions, including PRAGMA and attached-schema writes; COMMIT/ROLLBACK clear it. Export refuses open databases and their journal/WAL/SHM paths.
- Toolbar Vacuum, Integrity and Save Project are connected. Dialog actions fit above the status bar. Confirm-drops preference is respected.
- TextArea accepts IME preedit/commit/cancel through the shared UI API. Secret inputs remain masked and cannot copy their value.
- JSON projects retain all browse tabs and reject saving an in-memory database until Save As gives it a durable path.

## Framework completion started in this audit

- `cb727d7`: shared Table header-edge resizing; scroll/frozen-aware sorting and hit tests; separate cell/header/body clipping; width and bounded viewport getter/setters; named Space input for TextInput/TextArea.
- `b90d5a7`: e28 keeps widths and viewport in rebuilds/projects, preserves wizard column definitions on selection, and rejects unsupported command chords instead of inserting letters.
- Remaining framework gaps and application ownership are tracked as NG-01 through NG-10 in the linked audit. No new dependency was added.
- Remote UI is a local-path list. [Upstream announced DBHub.io shutdown for February 2025](https://sqlitebrowser.org/blog/dbhub-shutdown/); restoring that hosted service is not an actionable parity target without a replacement service.

## Framework completion — text editing (2026-10-26)

NG-03 has its basic editing layer: shared TextArea selection/hit testing, bounded
viewport rendering, vertical scroll indicator, literal find, IME selection replacement,
undo/redo and opaque editing-state capture/restore. e28 uses these APIs directly for
SQL and Find; it no longer maintains a separate byte-offset search/replace algorithm.
F01/F04 are resolved at the shared/UI logic level. Full code-editor parity still needs
syntax, completion, line numbers, grapheme/word navigation and larger-file profiling.

F02/F03/F05–F13 remain open. This batch does not change SQLite writes, project format,
DDL migrations, cell typing, result ownership, docking or RGBA image support.
