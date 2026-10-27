# DB Browser for SQLite (DB4S) ↔ e28 parity checklist

**Goal:** 100% feature + UI parity with [DB Browser for SQLite](https://sqlitebrowser.org/) (sqlitebrowser / DB4S), with next-gui Theme so the UI can meet or exceed DB4S visually. Items marked `partial` or `todo` are remaining compatibility work. This is an example, not a certified DB4S replacement.

Reference: upstream **v3.13.1** `src/MainWindow.ui`, EditTableDialog, EditIndexDialog, ImportCsvDialog, ExportSqlDialog, CipherDialog, PreferencesDialog, FindReplaceDialog, PlotDock, EditDialog, TableBrowser.

Status legend: `done` = implemented local subset, not upstream parity or native acceptance; `partial` = known missing or defective behavior; `todo` = absent.

**Current regression/evidence correction:** [2026-10-27 report](../../docs/reviews/R-20261027-5e23229548d4.md). This records the latest tested scope and corrects unsupported earlier claims.

**Page inventory audit:** [2026-10-25 page inventory, framework gaps and reproducible findings](../../docs/reviews/R-20261025-b90d5a7b4143.md). This supersedes earlier completeness claims. Source and Soft framebuffer review covered four main pages, five docks and all 13 overlay kinds; native OS input acceptance remains unavailable.

The earlier “done 101 / partial 0” claim was not supported by acceptance tests. The review below distinguishes implemented local workflows from upstream compatibility. See the 2026-10-25 review in `docs/reviews/INDEX.md`.

---

## Shell chrome

| Surface | Status | Notes |
|---------|--------|-------|
| Menubar File / Edit / View / Tools / Help | partial | Flat in-window menus; no native menu, nested submenus or application shortcut system. |
| Main toolbar: New, Open, Write Changes, Revert | done | + Theme (exceeds DB4S); View-toggleable |
| Extra DB / Project toolbars | partial | Local subset; no upstream movable toolbar/overflow behavior. |
| Main tabs (exactly 4): Structure / Browse / Pragmas / Execute SQL | done | Charts→Plot dock; SQL Log→Log dock |
| Right docks: Edit Cell / DB Schema / SQL Log / Plot / Remote | partial | One tabbed right pane; content-width titles, overflow arrows and keyboard reveal remain usable at narrow widths, with rebuild viewport retention. Body/structure ratios persist in projects. Independent floating/docking panes remain absent. |
| Status bar: path, encoding, read-only, encryption, busy | done | Reads actual main.encoding on schema refresh; cached while SQL runs; clears on Close. UTF-8 and both UTF-16 byte orders tested. |
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
| Exit | done | Unified Save/Discard/Cancel protects database, SQL, DDL, project, cell and PRAGMA drafts across New/Open/Close/Exit; canceled destinations preserve the session (F02 verified). |
| Open Project… | partial | DB4S XML + legacy JSON; relative paths, SQL references, connection PRAGMAs/attachments and basic browse state restored. All restored browse queries/patterns preflight before replacing the session. Advanced XML settings are retained and reported as unapplied. |
| Save Project / Save Project As… | partial | DB4S XML output with an optional next_gui extension; unknown XML survives. Native DB4S round-trip and advanced settings compatibility remain. |
| Close Project | done | Unified Save/Discard/Cancel; closes the project database as upstream does. |
| Save All | done | Choose destinations before saving; apply cell/PRAGMA drafts, commit DB, save linked SQL files and the project. Failures keep documents open; writes across files are not one atomic transaction. |
| Print Schema… | partial | Exports schema text to a file; no print dialog or print preview. |

## Edit menu

| Action | Status | Notes |
|--------|--------|-------|
| Create Table… | partial | Main/temp/attached schema selection; editable declared types with suggestions and genuine untyped columns, PK/AI/NN/Unique/Default/Collation, bounded scrolling form; full DB4S constraints/migrations/moving tables remain. |
| Create Index… | partial | Qualified table selection and atomic original-identity replacement; live expression/collation/order and partial WHERE editing, available/ordered key lists, add/remove/move/edit actions and a read-only generated SQL preview, declaration types and an ASC/DESC selector that preserves key expressions. |
| Modify Object… | partial | Native column/table rename (including column swaps/case-only changes) preserves identities and qualified browse/format/plot settings; Declared type, NN, single-column UNIQUE, default and column order now rebuild safely with existing constraints/dependencies, rowids/sequence and pending edits preserved. Primary-key membership can also be added/changed/removed; AI can be enabled/disabled with type/key synchronization and sequence-safe rebuild; options/full constraint editing and moving tables remain. Temp trigger/view rename has a locked-engine limitation with atomic rollback. Expression/partial index definitions edit in the qualified wizard. |
| Delete Object… | done | confirm + DROP |
| Preferences… | partial | Seven options including SQL auto completion and keyword output case; preference edits stay in drafts until Apply; full upstream preferences remain absent. |

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
| Structure toolbar: New Table, New Index, Modify, Delete, Refresh | partial | Basic qualified table/index wizards and Delete cover all namespaces; definition previews outside main stay read-only. Expression/partial definitions, ordered key actions/types/ASC-DESC and preview now edit across namespaces; safe definition/reorder rebuilding now covers all namespaces; full constraint/move and exact original/native acceptance remain. |
| Schema tree Tables/Views/Indexes/Triggers | partial | Main/temp/attached namespaces and empty databases, quoted object identity, selection/project definition context and qualified deletion are verified. Expansion, scroll, group selection and keyboard focus persist for the database session; active selection reveals ancestors. Column/detail tree, object context actions and project view persistence remain. |
| Object SQL preview | partial | Schema identity retained in projects; attached/temp stored definitions are read-only, since SQLite removes their database/TEMP qualifiers. Structured qualified editing remains. |
| Run DDL | partial | Edited DDL is protected on object selection with Keep in SQL tab / Discard / Cancel; other drafts and pending DB edits survive. Execution remains synchronous; long DDL can block interaction. |
| Print schema | partial | Text export only; no OS print dialog or preview. |

## Browse Data tab

| Feature | Status | Notes |
|---------|--------|-------|
| Table selector | done | |
| Filter / WHERE | partial | Raw WHERE and original any-column term search share export conditions; Clear Filters, Enter and Cmd/Ctrl+Alt+F. Automatic debounce and escape preferences remain. |
| Data grid | partial | Filtered totals and clamped 500-row pages; resize, frozen columns, scrollbars and focus reveal. Anchored Shift ranges and pointer drag rectangles survive rebuilds. Copy/cut/paste supports typed internal cells with HTML provenance, quoted TSV, headers (Cmd/Ctrl+Shift+C), INSERT statements (Cmd/Ctrl+Alt+C), single-cell fill, and atomic row updates/rollback. External TSV preserves literal NULL as text. Inline editors, discontiguous/cross-page ranges, stationary edge autoscroll, hiding/reorder and complete bulk operations remain. |
| Header-click sort | done | Ordinary clicks toggle one column; Ctrl/Cmd clicks append/toggle priorities. Priority labels and Clear Sort match upstream; filtered CSV/JSON exports and native XML retain all terms. Native modifier gestures await OS input acceptance. |
| Filter-per-column | partial | Shared inputs align with resized/scrolled/frozen columns. DB4S comparisons, numeric ranges, NULL/empty text, wildcards and Unicode PCRE2 REGEXP are supported. Escape preference and automatic debounce remain. |
| Multi browse tabs | partial | Add/switch/close guard cell drafts, SQL ownership and query failure. Valid cached rectangles restore by rowid (or unchanged typed rows without rowids), with directional anchors, scroll and current editor; last-tab close clears the editor. Reorder, per-tab close affordances and whole-result selection remain. |
| Add / Delete row | done | New Record form supports Value/NULL/Default and required fields; Delete uses unshadowed rowid |
| Edit cell → Edit Cell dock | partial | Explicit NULL/INTEGER/REAL/TEXT/BLOB and multiline editing; numeric types retained; Hex validation errors preserve the draft (F06 addressed). SQLite affinity still applies. |
| Frozen columns | partial | Freeze through the active column; per-table/tab overrides, JSON/extension and native XML counts restore. Legacy global counts retained. Header right-click freezing supported; native viewport indices now translate the leading row-id column (caca01e); row-id display remains. |
| Conditional formatting | partial | Table-scoped background rules persisted in projects (F11 addressed); no rule edit/remove UI or font/alignment controls. |
| FK click-navigation | partial | Implicit parent PK, composite grouping and stored typed keys supported; NULL reports no parent. Source requires an unshadowed rowid (F05 addressed). |
| Table selector / cycle | partial | Dropdown and Ctrl/Cmd+PageUp/PageDown share guarded switching through tables/views with wraparound. Query failure restores the previous context and reports errors. Main/temp/attached identity follows selectors, tabs, navigation, edits, typed insert, delete, cut/paste, FK lookup and full filtered export. Per-database/table filters, ordering, widths and frozen counts restore across switches and JSON/XML projects. Missing temp targets reopen empty with a notice; temp contents remain connection-only. Hidden/formats remain. |
| Record / form view | partial | First/Last and one-based Go to record cover the full filtered/sorted result, preserve the column, reveal the target and guard drafts/busy/query errors. Form arrows cross pages; empty results remain unselected. Previous/Next uses live visible rows minus one. Whole-result virtual scrolling, original whole-row/top alignment and full form controls remain. Plain PageUp/PageDown and Ctrl/Cmd+Home/End reach the full result; Shift ranges remain cache-local. |

## Edit Pragmas tab

| Feature | Status | Notes |
|---------|--------|-------|
| Scroll form of pragmas | partial | Scrollable form for ten PRAGMA keys; full upstream setting list remains absent. |
| Save / Apply pragmas | partial | Writes are read back; unapplied values report actual state and earlier partial application (F03 addressed). No automatic VACUUM for auto_vacuum. |
| Bool / Select editors | done | journal_mode / synchronous / FK switches |

## Execute SQL tab

| Feature | Status | Notes |
|---------|--------|-------|
| SQL editor | partial | Shared TextArea now has pointer/drag and keyboard selection, scrolling, undo/redo, IME replacement and rebuild state (F01 addressed). PageUp/PageDown with Shift selection now moves the caret/viewport by visual lines. Vertical keys retain the desired column through short lines and same-layout sessions. Optional shared physical line numbers are enabled in SQL/Find, with wrap/scroll/pointer/IME alignment. Lexical token colors cover runtime keywords, literals, quoted names, parameters, functions and comments. Cached keyword/function/schema/table/column suggestions support automatic/manual triggering, quoted/qualified replacement and undo. Original extra keyword/type words now share syntax/completion; keyword output case is a persisted draft preference. Cached runtime function signatures now show overloads, current argument, variadic and zero-argument forms in passive hints; nested calls/comments/quoted commas preserve context and Escape suppression survives editor sessions. Original-covered functions now have parameter roles/descriptions, overload controls and bounded wheel/keyboard help scrolling; Option/Ctrl word movement, Shift selection and word deletion preserve Unicode word boundaries and editor history; full syntax styling and complete grapheme-aware navigation remain. |
| Execute / message / results table | partial | Final statement preview, at most 500 displayed rows; results/messages/export snapshots belong to each SQL tab (F09 addressed). |
| Toolbar: Open Tab, Open/Save file, Execute, Stop, Find | partial | Selection/whole-script execution, source-bound Save/Save As and shortcuts; external file changes reject overwrite; opening preserves existing drafts. |
| Multi SQL tabs | partial | Independent drafts/sessions/results, file names and dirty markers; Close Tab, move arrows and Cmd/Ctrl+T/W/S/Shift+S. Pointer tab dragging, close affordances and rename remain. |
| Find / Replace | partial | Shared editor in Find panel highlights/reveals matches; Replace acts on the selected match and Replace All is undoable (F04 addressed). Match case/whole words/wrap/backwards/selection-only/PCRE2, Find All highlights and capture replacement are implemented; upstream regex dialect, large-document worker and native acceptance remain. |
| Async query + cancel | done | Live connection moves to worker; Write/Revert/keys/ATTACH shared; completion polled on redraw; Stop interrupts |
| Results → CSV / JSON / Save as view | done | Full snapshot export without re-execution; editable view name and source tied to the executed result statement. |
| Toggle comment | partial | Current/selected physical lines, first-line mode, exact two-hyphen removal, LF/CRLF preservation and one undo/redo edit. Ctrl/Cmd+/ and toolbar retain tab/file sessions. CR-only editor lines and native acceptance remain. |
| Execute current line | done | Uses Unicode caret position; never replaces editor contents; one physical line |

## Docks

| Dock | Status | Notes |
|------|--------|-------|
| Edit Database Cell | partial | Shared multiline TextArea, explicit storage type/NULL, validated Hex and PNG/JPEG view; full binary/XML/RTL/evaluation and generic import/export remain. |
| DB Schema | partial | Flat main/temp/attached object list; no column tree, filter or SQL insertion. |
| SQL Log | partial | SQLite statement/trigger tracing with expanded ordinary values; User/Application/Error/All, full read-only text, copy, scoped clear/export and visible eviction counts (200/category). Global SQLite diagnostics and native acceptance remain. |
| Plot | partial | One numeric-column trend from an explicit main/temp/attached table/view, at most 64 source rows; invalid samples skipped and query errors clear stale data (F07 addressed). Database identity survives local projects; absent sources are reported. No X/Y1/Y2 or result-set source. |
| Remote | partial | Local path list; cloud protocol/auth/sync not implemented |

## Dialogs

| Dialog | Status | Notes |
|--------|--------|-------|
| EditTableDialog (full wizard) | partial | Basic create plus safe native ALTER subset; arbitrary rebuild migrations excluded |
| EditIndexDialog | partial | Qualified expression keys/unique/partial WHERE, live definition retention and atomic replacement; available/ordered lists, add/remove/move/expression/edit controls and read-only SQL preview. Multiline SQL, focus/selection/undo and list viewports survive rebuilding; ASC/DESC changes preserve operands/comments/collation and share Undo; declaration types include generated columns. Exact original transfer filtering/interface and native acceptance remain. |
| ImportCsvDialog (delimiter, quote, encoding, …) | partial | UTF-8 plus BOM-detected UTF-16LE/BE; malformed units/surrogates and unsupported UTF-32 BOMs are rejected before writing; delimiter/quote/header and multiline quoting supported; legacy code pages and an explicit encoding picker remain |
| ExportSqlDialog (options) | partial | Four switches, all tables; no multi-table selection, IF NOT EXISTS mode, INSERT batching or original-CREATE choice. |
| CipherDialog | partial | Unlock/export copy with password/page/KDF only; no rekey/remove encryption, raw key, HMAC/KDF algorithms or SQLCipher 3 preset. |
| PreferencesDialog | partial | Font/theme/drop confirmation/CSV delimiter/frozen count/SQL auto completion/keyword case; Cancel preserves live settings (F11 addressed); full upstream preferences absent. |
| FindReplaceDialog | partial | Six search options, Find All and disjoint highlights, Unicode captures/escapes, F3/Shift+F3 and atomic undoable replacement; PCRE2 is not certified Scintilla regex compatibility. One-second scan budget; large backward searches still need a cancellable worker. |
| EditDialog (blob/image/hex) | partial | Multiline text/JSON, typed/NULL values, Hex and RGBA preview; Fit/1:1/zoom/pan/original image export supported (F12 addressed). Full binary editor remains. |
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
| ATTACH DATABASE | partial | Live connection, namespace tree/definition/deletion and Browse data workflows preserve explicit database identity; file-backed attachments and their browse settings restore through projects. Basic structured table/index creation and safe modification cover all namespaces; safe type/NN/U/default/order rebuilding preserves dependencies in all namespaces; full constraint/move workflows remain. |

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
- Image decoding uses `image` 0.24; rendering uses next-gui `Image` with shared RGBA drawing in Soft/GPU. Original PNG/JPEG pixels are retained up to 16 megapixels; shared Fit/1:1/zoom/pan and original-image export are available (F12 verified on 2026-10-27).
- Remote currently means a local-path connection list. Cloud compatibility remains unimplemented; no claim about the upstream service license or protocol is made.

## 2026-10-25 verification and boundaries

- Live WAL/memory Save As, more than 500 exported rows, NULL/empty text/NUL/BLOB/numbers, trigger order and sequence state have regression tests.
- SQL batch parsing uses rusqlite/SQLite statement boundaries. Async execution uses the live connection. Explicit transaction commands in Execute SQL retain SQLite semantics. Interrupting a write can roll back the transaction; the error now states this.
- CSV import and index/table changes use savepoints. New databases and Save As never truncate an existing file. File-dialog cancellation has no implicit `/tmp` output.
- Project/prefs parsing uses serde JSON; project writes and database dumps publish their completed temporary file.
- macOS Soft screenshots cover all main pages and major dialogs. OS pointer/keyboard/file-picker acceptance is blocked by desktop capture permissions. Native Metal startup alone is not visual acceptance.
- DB4S XML, cloud sync, arbitrary constraint migrations, multi-table export and full attached-schema constraint/migration editing remain compatibility work. Native IME acceptance requires desktop permissions.

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


## Framework completion — RGBA rendering (2026-10-26)

NG-04 now has validated shared image resources, RGBA draw commands, source-over
alpha blending, rectangular clipping and nearest-neighbor scaling in both Soft
and GPU. The Image widget fits the source aspect ratio inside the panel and
clips its caption. GPU images larger than an adapter texture limit are tiled.
The public `mosaic_max` field remains for source compatibility and is ignored.
Incomplete pixel buffers render a placeholder instead of allocating padded data.

e28 retains the decoded PNG/JPEG resolution and alpha. The dock shows the original
size; failed/oversized decoding reports an error without modifying the BLOB.
The preview limit is 16 megapixels; decoder allocations are limited to 128 MiB.
F12 is **in_progress**: the mosaic defect is resolved, but interactive zoom/pan,
1:1 inspection and image export are still missing. F02/F03/F05–F11/F13 remain open.
No database or project format changed. Native desktop input acceptance remains
unavailable; framebuffer captures and Metal readback are recorded separately.

## Workflow and framework follow-up — 2026-10-27

The latest code addresses the core behavior behind F03/F05/F06/F07/F08/F09/F11/F12.
F02 is verified after unified document transitions and per-document SQL close.
F10 adaptive panes, ratio persistence, filter geometry, scrollbars and focus reveal
are implemented; native input acceptance and F13 full SQL tracing remain pending. Earlier sections are historical records.

Shared additions: `ImageView` get/set with validated scale and bounded pan,
`FlexBuilder::scroll_row`, public validated ScrollArea offset restoration, correct
nested wheel routing and expanded-content extent calculation. e28 uses these APIs
for images, long strips, narrow windows and scrollable pages/docks. No dependency
was added and database formats are unchanged; JSON projects gain optional fields
with backwards-compatible defaults.


## Desktop workflows and project compatibility — 2026-10-27

- Shared Modal supports Save/Discard/Cancel with focus cycling and wrapped body.
  Whole-workspace transitions save/discard all applicable drafts; closing one SQL
  tab saves/discards only that document. SQL dirty markers move with their tab.
- e28 uses compact consistent typography, neutral toolbar actions, light/dark
  palettes and semantic primary actions. The 800×600 layout adapts pane widths;
  dragged body/structure ratios survive rebuilds and project save/load. Long
  toolbars remain scrollable. Font fallbacks supply missing macOS symbols.
- `.sqbpro` now uses the DB4S 3.13.1 XML structure. Legacy JSON remains readable.
  Core paths/tabs/file references, access mode, five connection PRAGMAs, attached
  files, main/attached browse identity and per-database widths/multi-column sort are supported. Unknown XML is retained.
  Qt dock state and advanced browse filters/formats/plots are reported as retained but unapplied. Temporary table contents remain connection-only; missing temp browse targets reopen empty with a notice. Extra local state lives in a
  next_gui extension ignored by DB4S. This remains partial interoperability.
- New XML parsing uses the already installed quick-xml version. XML is UTF-8 with
  16 MiB / 64-level bounds; DTD/external entities and malformed identifiers are
  rejected. Existing SQL files and new connections are prepared before replacing
  a session. Read-only project attachments are opened read-only.
- Native desktop capture still shows wallpaper only. Application framebuffer
  review and logic tests do not replace real OS input/IME/picker acceptance or a
  round trip through the DB4S binary.

## Filters and shared scrolling — 2026-10-27

- Native project column/global filters now restore and save. Any-column search
  ANDs terms and ORs columns using the same operator syntax as column filters;
  complete filtered exports use the same conditions. Applying filters keeps sorting.
- PCRE2 provides Unicode properties, lookaround and backreferences with cached
  SQLite REGEXP. Match limits are 1,000,000 backtracking steps / depth 1,000;
  errors are reported. Escape preferences and automatic debounce are still absent.
- Filter fields share table widths, scroll and frozen-column geometry. Shared
  overlay scrollbars support drag and track paging; keyboard focus reveals nested
  scroll content without resetting manual scrolling. Normal shell/SQL panes fit
  the available height; 800×600 pages remain scrollable. Hidden docks reclaim width.
- 217 workspace tests and strict clippy/build checks pass. See the
  [latest review](../../docs/reviews/R-20261027-2bfc8ad51380.md) for captures,
  the project browse-error finding and native acceptance limits.


## Grid correctness and evidence correction — 2026-10-27

- Shared Table keeps directional selection anchors, ordered copy bounds and the
  active column in view. `TableStyle::drag_select` enables rectangular drag
  selection using the same frozen/scrolled geometry. Existing tables keep body
  panning by default. e28 defers selection changes until pointer release and
  preserves grid focus through rebuilds.
- The platform now exposes HTML clipboard reads/writes on WindowCtx on all three
  backends. e28 uses an HTML stamp to recognize its typed internal copy buffer.
  Preview snapshots retain SQLite storage classes and raw TEXT/BLOB bytes, with payloads over 8 MiB moved to temporary disk storage; typed
  copies do not substitute display labels. External TSV remains a text format.
- Paste/cut uses one UPDATE per record inside a savepoint. Constraints, a stale
  target or trigger-induced row loss roll back the whole operation while keeping
  earlier unwritten edits. Bounds are checked before writes.
- Invalid UTF-8 TEXT can be browsed, copied, pasted and used in foreign keys
  without a panic. The Value-based cell editor still cannot edit such TEXT; it
  reports the limitation instead of converting the stored bytes.
- This is still partial DB4S parity. Native OS input/clipboard/IME, Linux/Windows
  runtime, performance budgets and DB4S binary round trips remain acceptance gaps.
  A window-only native render capture is separate from those checks.

## Button availability (2026-10-27)

Shared buttons now support disabled pointer/keyboard/focus behavior and themed
colors. e28 page/record navigation reflects boundaries; clean, read-only, busy or
disconnected databases disable Write/Revert. 271 workspace tests pass. Native
Metal browse and Office renders were inspected; OS input/IME/clipboard acceptance
and complete upstream action-state coverage remain. See
[R-20261027-a99567b81242](../../docs/reviews/R-20261027-a99567b81242.md).

## Multi-column sorting (2026-10-27)

Replaced the earlier incorrect three-state cycle with upstream two-direction
header gestures, Ctrl/Cmd priority sorting and explicit Clear Sort. Native XML and
complete filtered CSV/JSON exports retain all priorities. Pointer button events
now carry modifiers on every host, including touch synthesis. 277 workspace tests
pass; native rendering was inspected, while native gesture and DB4S binary
interoperability acceptance remain. See
[R-20261027-a37bee216b33](../../docs/reviews/R-20261027-a37bee216b33.md).

## Container rendering polish (2026-10-27)

Fixed the shared no-background container visibility error behind black table
bottom corners. Seven container types retain explicit fills, transparent gaps and
child hits through full/partial repaint. 278 workspace tests pass; fresh e28
Soft/Metal and every Office example window were visually inspected. This render
check does not establish native input or full original interface parity. See
[R-20261027-f434a694b24d](../../docs/reviews/R-20261027-f434a694b24d.md).

### Record navigation checkpoint (2026-10-27)

`ebeb580e892d` adds First/Last/Go to record and exposes shared Table selection
reveal. Bounds, filtered ordering, failed-query restoration, drafts, empty/shrinking
results and narrow-window hit targets pass regression tests. Workspace: 284 tests;
clippy/build/platform features pass. Soft wide/narrow and native Metal captures
were inspected. See [review and limitations](../../docs/reviews/R-20261027-ebeb580e892d.md).
Original visible-row page stepping, whole-result scrolling and native input
acceptance remain open; this checkpoint does not complete full reproduction.

### Visible-row navigation checkpoint (2026-10-27)

`5c9c20d208ae` replaces the fixed-page buttons with bottom Previous/Next controls
that move by visible rows minus one. The shared counter accounts for half rows
and resize; cross-cache selection, errors and drafts are covered. Workspace:
287 tests, clippy/build/platform features pass. Soft wide/narrow and native Metal
painting were inspected. See [review and remaining behavioral differences](../../docs/reviews/R-20261027-5c9c20d208ae.md).
Original row/top alignment, whole-result scrolling and native input remain open.

### Page-key checkpoint (2026-10-27)

`3f3153b4c200` forwards PageUp/PageDown from all desktop hosts and implements shared
Table page/range keys. e28 plain page keys and Ctrl/Cmd+Home/End use guarded
full-result navigation. All exhaustive Office key adapters were migrated.
Workspace: 289 tests, clippy/build/platform features pass. e28 Soft/Metal and all
14 Office windows were inspected. Native hardware input, cross-cache Shift ranges,
whole-result scrolling and original row/top alignment remain open. See
[review and API migration](../../docs/reviews/R-20261027-3f3153b4c200.md).

### Safe table-switch checkpoint (2026-10-27)

`8ab0778f3a16` restores the prior table/result/editor after failed switching and
reports the error; the before-fix broken-view regression failed as intended.
Dropdown, Apply and Ctrl/Cmd+PageUp/PageDown use one guarded path. Cycles wrap
through tables/views, and source filters do not leak into unrelated schemas.
Workspace: 292 tests, clippy/build/platform features pass; Soft failure recovery
and native Metal painting inspected. Per-table option restoration is the next
gap; full reproduction and native input remain open. See [review](../../docs/reviews/R-20261027-8ab0778f3a16.md).

### Per-table option checkpoint (2026-10-27)

`db7f25222647` restores independent per-table WHERE/global/column filters, ordered
sort terms and column widths in each browse tab. JSON, the e28 XML extension and
native DB4S supported fields retain inactive-table options. Project preflight
checks inactive saved queries before adopting a database; failures retain the
current connection and editor. Native-only XML roundtrip and inactive freeze
metadata preservation are covered. Workspace: 296 tests, clippy/build/platform
features pass; Soft restored state and native Metal painting inspected. Per-table
freeze restoration, hidden/display formats, native binary/input acceptance and
full reproduction remain open. See [review](../../docs/reviews/R-20261027-db7f25222647.md).

### Frozen-column checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-5bd5104966ae.md): toolbar freezes through the
active column, table switches restore counts, native XML counts validate/clamp,
and old global counts still load. Duplicate-table tabs keep separate extension
state. 299 workspace tests and strict checks pass; Soft geometry and native Metal
painting inspected. Header context actions and native input acceptance remain.

### Header-menu checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-fdbf1e3c3942.md): header right-click actions
freeze through the clicked column, select the current cache's column, and copy its
name. Shared detached menus support keys, enabled-item navigation, hover contrast
and owner cleanup. Active cells can be retained inside a rectangular selection.
Both themes' selected/highlighted style pairs meet 4.5 contrast; 307 tests and
strict checks pass, with Soft/Metal and 14 Office render inspections.

F03 was subsequently verified in `caca01e`: widths/frozen prefixes translate the leading row-id logical column while schema sort/filter indices stay unchanged. This supersedes earlier handmade viewport fixture claims. Row-id display, complete header actions, whole-result column selection and native input remain absent.

### Native viewport compatibility checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-caca01ed09f6.md): source-aligned ordinary/view/WITHOUT ROWID fixtures, native-only save/reload, old e28 project migration, independent tab widths and row-id metadata preservation pass. The before-fix regression reproduced shifted widths and an extra frozen visible column. 310 workspace tests and strict checks pass; Soft imported viewport and native Metal painting inspected. Original-binary interchange and native input remain unverified.


### SQL comment checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-3ab164fed903.md): comments now affect current/selected lines, preserve unrelated SQL and LF/CRLF, and restore selection/history through rebuild/tab switches. A shared validated TextArea range replacement also serves search/replace. File drafts stay in memory; disk SQL and database are untouched. Baseline whole-document/EOL corruption reproduced before the fix. 316 tests and strict checks pass; dark/light Soft, native SQL painting and all fourteen Office windows inspected. CR-only lines, complete SQL editor and native acceptance remain open.


### TextArea page navigation checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-4bb6e7d46905.md): shared TextArea now handles page movement/Shift selection for SQL and read-only text. Wrapped/tiny/empty bounds and rebuild sessions pass. A middle-line test exposed a fractional-height row drift; keyboard targets now use nearest layout rows, preserving pointer behavior. 320 workspace tests and strict checks pass; dark/light selected page views, native SQL painting and all fourteen Office windows inspected. Desired-column retention across short lines (F03), complete editor and native acceptance remain open.


### Desired-column checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-4356e82aef7b.md): vertical/page/Shift movement remembers its horizontal goal through shorter lines and rebuild. Explicit placements/edits and changed font/width reset it. The 14-vs-21 baseline now passes; 322 tests and strict checks pass, with Soft page-session and native SQL/all Office render inspection. This closes page checkpoint F03. Complete editor, dock overflow refinement and full native/original acceptance remain open.

### Dock tab overflow checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-ef5bd92927fb.md): shared optional content-width strips provide overflow arrows, selected-title reveal and opaque rebuild viewport retention. e28 enables them on dock titles; dark/light 800×600/1400×860, native browse and all Office static renders inspected, with 327 tests and strict checks passed. This supersedes the earlier pending dock-overflow refinement, not independent docking/dragging/closing or full narrow-window parity. Native interaction acceptance and actual Remote service remain open.

### Browse-tab session checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-0ad15812f354.md): switch/close preserves valid per-tab directional rectangles, scroll and editor; rowids remap within refreshed cache, changed/missing selections clear safely, and keyless tables/views require unchanged typed rows. SQL worker ownership blocks all three tab mutations and disables their buttons. Both failing baselines now pass; 332 tests/strict checks, dark/light Soft switch captures and native painting inspected. Whole-result selection and native/original interaction acceptance remain open.

### SQL line-number checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-4c3ca63f83ac.md): shared optional gutter numbers physical LF/CRLF lines, including blanks, without numbering wrap continuations or entering copied text. SQL/Find enable it; sessions and pointer/key/scroll/IME geometry account for its width. 336 tests/strict checks and numbered Soft/Metal readback (delta 2) pass; dark/light, native SQL and all Office static renders inspected. CR-only lines, margin gestures/folding, syntax/completion and native/original acceptance remain open.

### SQL token-color checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-b8afc40b7b78.md): shared validated foreground spans support app lexical colors without changing copied SQL, selection/history or IME composition. Main/Find colors refresh after edits and theme/session restoration. 341 tests/strict checks and colored Soft/Metal readback (delta 2) pass; dark/light Soft inspected. Auxiliary reuse layering regression was caught and fixed. Native/Office capture failed while the display slept, so current native visual acceptance remains incomplete. Synchronous lexing, semantic completion, full style preferences/folding and original/native acceptance remain open.


## SQL namespace completion checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-605b04835365.md): shared caret-anchored, paged completion menus accept one undoable replacement, retain editor focus, and reject stale/read-only/composing contexts. SQL/Find supply cached engine functions and all attached/temp schema/table/column paths, quote-aware Unicode replacements, three-character auto triggering and explicit Ctrl/Command+Space. Built-in functions work while disconnected, and auto completion has a compatible persisted draft preference. 351 tests/strict checks pass; dark/light Soft and preference overlay inspected; actual completion Metal readback delta 1. Long menu-label overflow was reproduced and fixed. F02 tracks original extra word/case settings; calltips, full editor/native/original acceptance remain open. Query-scope alias/CTE analysis is outside the reference static API provider verified here.


## Original keyword parity checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-3855f40fe8ba.md): F02 verified after reproducing missing ANY. Eleven original editor/type words now share keyword colors and completion; keyword output case defaults uppercase and has a persisted draft preference. Functions and object spellings stay intact. 353 tests/strict checks, dark/light Soft, preference overlay and actual completion Metal delta 1 pass. Native display remains asleep; function calltips, broader editor/preferences and native/original acceptance remain unfinished.


## Runtime function hint checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-76baeacfe7e5.md): shared passive TextArea hints wrap/clip/flip within the window, retain focus, pass pointer/wheel input through and preserve Escape suppression in matching editing sessions. Cached SQLite function arities provide nested-call/current-argument, overload, variadic and zero-argument hints. Stale tooltip-host overlays and shadowed function metadata were reproduced and fixed. 363 tests and strict checks pass; both-theme actual Metal delta 1, Soft views, native e28 and all fourteen Office static windows inspected. Native actuation still shows no app window and activation fails, so key/IME/clipboard acceptance remains open. F03 tracks meaningful argument names/descriptions and long overload/help interaction. Complete editor and full DB4S parity remain unfinished.


## Descriptive function help checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-f6bf184f24b3.md): original-covered core/date/aggregate/window/math functions now show meaningful parameter roles and concise explanations, gated by live built-in arities. Custom overrides retain generic help; current arguments follow fixed and repeated roles. 366 workspace tests and strict checks pass; both-theme Soft/Metal delta 1 and native e28 static painting inspected. F03 stays in progress for long overload selection/browsing. Full editor, native input and complete DB4S acceptance remain unfinished.


## Function overload browsing checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-6ee1edbad2a0.md): F03 verified for descriptive function help with one overload per page, previous/next controls, Cmd/Ctrl+Shift+Space open/cycle, and clipped-text wheel/Cmd/Ctrl+Shift+Page scrolling. Selection, focus, Undo and page/scroll sessions survive refresh/rebuild; preferred overload follows changed argument context. Sixteen overload and 127-argument stress cases pass. 371 tests and strict checks pass; dark/light actual Metal delta 1/2, Soft endpoints, native e28/all fourteen Office inspected. Static capture is not native input acceptance. Full editor, large-font layout/preferences and complete DB4S parity remain unfinished.


## Column metadata resilience checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-4511f2bc5257.md): a user table named pragma_table_xinfo no longer disables column completion. Schema-qualified PRAGMA loading preserves generated/temp/attached columns and quoted names. Baseline error reproduced; 372 workspace tests and strict checks pass, with both-theme completion Metal delta 1. Query-only fix; no fresh UI/native acceptance claim. Complete DB4S parity remains unfinished.


## Word editing checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-f4d258fe1169.md): shared TextArea word movement/selection/deletion is mapped to Option on macOS and Ctrl on Windows/Linux in SQL/Find. Unicode/underscore identifiers, punctuation, whitespace and grapheme clusters at word boundaries are handled. Selected deletion is one undoable edit; editor sessions, read-only and IME preedit guards pass. Reproduced caret 25-vs-16 baseline now passes. 376 tests and strict checks pass; Soft/native static word selection inspected. Full character/pointer/vertical grapheme handling, native input and complete editor/DB4S parity remain unfinished.


## Namespace Browse checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-901390905c25.md): main/temp/attached identity now reaches selectors/tabs, record navigation, counts/filter/order, typed edit/insert/delete, cut/paste, same-database FK navigation, conditional formats, SQL copies and full filtered export. Per-database/table settings and file-backed attachments round-trip/preflight through native XML plus the local extension. Legacy JSON defaults to main; missing temporary targets reopen empty with a notice, and Save As clears disconnected views. 391 tests/strict checks and both-theme wide/narrow/conditional/native static captures pass. Exact original fallback behavior, namespace Plot/structured wizards, advanced settings and native input acceptance remain unfinished; F02 stays in_progress.


## Namespace Plot checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-1ca20533cf37.md): explicit main/temp/attached table/view selection, generated/quoted columns, local project schema restoration and stale-sample clearing now pass 393 tests and strict checks. Source and Plot controls fit in two rows at narrow widths; both-theme Soft/native Metal static captures inspected. Sampling remains at most 64 source rows. F01 tracks clipped narrow captions; original current-model binding, full axes/series/interaction/export, structured namespace wizards and native input acceptance remain unfinished.


## Readable Chart caption checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-14a98a98cdb1.md): shared Chart wraps bounded captions with grapheme-safe ellipsis, keeps chart bodies below them and exposes full accessible text; e28 adds its existing tooltip. F01 verified. 395 tests/strict checks, four theme/size chart Metal readback deltas of 2, native CJK Plot and all fourteen Office static captures pass. Full Plot/DB4S, typography polish and native input acceptance remain unfinished.


## Schema wizard helper checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-30de3bbd49ad.md): explicit main/temp/attached CREATE TABLE, CREATE INDEX and atomic native ALTER helpers pass 396 workspace tests and strict checks. Index-dependent column removals are guarded against quoted attached-column conversion; expression-index removals require an explicit migration. Controls remain main-only until UI integration. Full schema migration/editor and native acceptance remain unfinished.


## Qualified schema wizard controls checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-6a2fb87f7784.md): basic table create/modify and atomic index create/replace now carry main/temp/attached identities. Renaming follows browse tabs/settings/formats/Plot; custom/empty types and edited DDL survive. Scrollable forms keep Save visible at narrow sizes. 399 tests/strict checks, ten frames per renderer and two native static captures pass; input interiors differ by 1, primitive edges up to 73 remain F01. F02 tracks column-setting refresh after ALTER. Full constraint/migration, expression/partial-index, original appearance and native input acceptance remain unfinished.


## Browse column identity checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-80c8803f4766.md): F02 verified. Surviving column names now map filters, sorting, widths and the frozen prefix after schema changes, including export before refresh and active/inactive JSON/native XML projects. Invalid raw WHERE drafts remain intact. Re-saved XML indices match the new columns. 401 tests/strict checks and scoped Soft/native Browse static captures pass. Legacy unnamed settings retain first-load positions; external column renames, renderer F01, full schema/Plot/interface and native input acceptance remain unfinished.


## Fractional primitive coverage checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-977efec45f5f.md): renderer F01 verified. Shared pixel-center bounds, signed-distance rounded edges, sharp small-radius fallback and straight-alpha rectangles align Soft/GPU. Actual primitive readback max 1; all ten wizard whole frames max 2; fresh e28 plus fourteen Office native static windows inspected. 403 tests/strict checks pass. A new P1 finding tracks generated schema fields reaching execute_batch; full editors/Plot/original interface and native input acceptance remain unfinished.


## Structured schema statement boundary checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-b9a0f821765e.md): generated-DDL F01 verified after reproducing a Columns field executing an extra DELETE. CREATE/ALTER/index replacement now reject tails before stepping or preparing PRAGMAs, preserving data and pending transactions. Quoted semicolons, custom types, expression creation, CSV imports and intentional scripts still work. 405 tests/strict checks pass. No layout change or new native acceptance; full expression/partial-index controls and schema/interface parity remain unfinished.


## Expression and partial index checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-68b65c9c64b6.md): live main/temp/attached definitions now retain nested expression keys, collation/order, comments and partial WHERE through editing and atomic replacement. Actual unique/partial behavior and invalid-condition/tail rollback pass; WHERE is available in both themes at narrow widths. 407 tests/strict checks, ten Soft/Metal pairs and two native static forms pass. Full ordered-key controls, multiline editing polish, table constraints/migration and original interface/native acceptance remain unfinished.

## Multiline index editing checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-abc0450a381d.md): syntax-colored key and partial WHERE TextAreas preserve focus, selection and Undo through rebuilding; multiline expressions/comments and failed replacement retain their exact drafts. Narrow top/bottom scrolling keeps actions reachable. 407 tests/strict checks, twelve Soft/Metal pairs and two native static forms inspected. Available/ordered key controls, SQL preview, full table migration and native input acceptance remain unfinished.

## Ordered index key and preview checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-0333104cbccf.md): available/ordered key lists now add generated/quoted columns and expressions, edit/remove/reorder through the same undoable SQL draft, and show execution-identical read-only SQL. Shared List offset/reveal APIs retain long-list sessions; preview selection/focus also restores. 410 tests/strict checks, fourteen Soft/Metal pairs and two native static forms inspected. Dedicated key-order selector/type presentation, exact original transfer behavior, full table migration and native acceptance remain unfinished.

## Index key order and type checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-24b5a0495d77.md): selected-key ASC/DESC now uses SQLite expression preparation to preserve keyword-named operands, collations and comments. Order edits share Undo/Redo; row changes/reopen preserve raw SQL and selector focus. Available declaration types include generated columns and resist PRAGMA-table shadowing. 412 tests/strict checks, fourteen Soft/Metal pairs and two native static forms inspected. Exact upstream transfer filtering/interface, full table constraints/migrations and native acceptance remain unfinished.

## Editable column declarations checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-30646db63f83.md): column types accept custom declarations or no type, with suggestions and rebuild-safe IME/selection/focus. Shared Autocomplete now supports clipboard, composition, bounded keyboard/wheel/pointer suggestions and readable theme highlights. Actual CREATE/ADD metadata and affinity pass across main/temp/quoted attachments; statement tails preserve the database. 418 tests/strict checks, twenty-two Soft/Metal pairs and native static captures inspected. Full table constraints/rebuild/move, richer editor selection/Undo and original/native acceptance remain unfinished.

## Column rename identity checkpoint (2026-10-27)

[Review](../../docs/reviews/R-20261027-8dcf321b3729.md): Modify Table now saves column renames, name swaps and case-only changes using explicit source identities and atomic native ALTER. Qualified filters/order/widths/frozen prefixes, conditional formats and Plot references follow the same columns. Main/attached dependencies, constraints, generated values, sequence and pending transaction preservation pass; raw WHERE errors report saved DDL and clear stale browse data. 424 tests/strict checks, four Soft/Metal pairs and two native static windows inspected. Full rebuild/constraints/move, temp-trigger engine workaround, original interface and native input acceptance remain.

### Safe definition rebuild progress (2026-10-27)

[Review](../../docs/reviews/R-20261027-ee0e39162afd.md): Modify Table saves declared type, NN, single-column UNIQUE, default and editable-column order through a constraint-preserving SQLite AST and atomic rebuild. Named/composite/FK/check/collation/generated/AI/options survive unchanged; qualified dependencies, rowids, sequence, pending transactions and settings are validated/restored. Copy conflicts abort; ambiguous existing FK violation identities decline safely. 435 tests/strict checks pass. Four Soft/Metal pairs and two native static captures inspected before final DB-only guards; successful UI paths re-tested afterward. PK membership/AI/options/advanced constraint controls, cross-database move, original layout and native input acceptance remain.

### Primary key editing progress (2026-10-27)

[Review](../../docs/reviews/R-20261027-67aa8705b3dd.md): existing PK controls now save absent/composite/single/no-key transitions through the safe rebuild. Key names, retained ordering/collation/conflict policy, requested implicit NN, hidden rowids and active AI sequences are preserved; NULL/duplicate/ambiguous FK identities roll back. 440 tests/strict checks, four Soft/Metal pairs and two native static forms inspected. PK-column deletion currently requires saving key removal first. AI/options/advanced constraints/move and exact original/native acceptance remain.

### Autoincrement editing progress (2026-10-28)

[Review](../../docs/reviews/R-20261028-1999dc2e641c.md): AI metadata now loads from inline/table-level keys. Its accessible control synchronizes INTEGER/PK/competing keys, saves through CREATE/rebuild and reopens accurately. Existing integer IDs survive; explicitly converting NULL IDs allocates after existing values. Active sequence history is retained, incompatible/exhausted IDs roll back, and disabling AI keeps PK. 443 tests/strict checks, four Soft/Metal pairs and two native static windows inspected. Exact original eager validation/cancellation, advanced constraints/options/move/layout and native input acceptance remain.

### Declared collation editing progress (2026-10-28)

[Review](../../docs/reviews/R-20261028-3a5677711d33.md): the editable Collation control loads actual declared metadata, offers registered names, preserves IME/focus across rebuilds and clears to default BINARY. CREATE/rebuild use quoted names and preserve effective constraint names; actual NOCASE/RTRIM/BINARY comparisons, uniqueness/FK conflict rollback and unknown-name drafts pass. 446 tests/strict checks, four Soft/Metal pairs and two native static forms inspected. CHECK/FK/generated/options/full constraints/move, original grid/tab presentation and native input acceptance remain.
