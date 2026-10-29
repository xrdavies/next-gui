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
| Main toolbar: New, Open, Write Changes, Revert | partial | Five original display styles with house icons/full live help, measured row heights and retained preferences; View-toggleable. Full original actions/order, movement and overflow behavior remain. |
| Extra DB / Project toolbars | partial | Independently toggleable groups share a scrollable row and the original Main display-style preference; full accessible help. Original actions/order, movement and overflow remain. |
| Main tabs (exactly 4): Structure / Browse / Pragmas / Execute SQL | done | Charts→Plot dock; SQL Log→Log dock |
| Right docks: Edit Cell / DB Schema / SQL Log / Plot / Remote | partial | Independent upper cell editor and lower bottom-tabbed Log/Plot/Schema/Remote; close/reopen retains drafts, preferred splitter ratios and per-page scrolls. Plot options scroll independently above a retained chart. Floating/rearranging and Qt dock-state compatibility remain. |
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
| Import → Table from CSV file… | partial | Bounded preview, supported codecs/BOMs, custom characters/codepoints, trim/type/default/missing/conflict policies, multi-file selection/shared or per-file targets/four-choice append confirmation/retry and settings; validated streaming snapshots, cancellable worker progress and system numeric conventions; centered modal/fixed actions/nested questions; localized Windows/POSIX symbols/Qt codecs/parser edges pending |
| Import → Table from CSV clipboard… | partial | Unicode clipboard, shared preview/options/type/policy engine, validated anonymous snapshot and safe four-choice positional append; cancellable worker progress and centered modal/fixed actions; remaining codecs/parser/platform-symbol and full original workflow acceptance pending |
| Export → Database to SQL file… | partial | Modal table/view selection and original content/schema/column-name/multi-row/original-CREATE options, persisted settings and suggested filenames; cancellable worker, guarded atomic output, generated columns, triggers after data and selected AUTOINCREMENT high-water marks; full/structure FTS/RTree indexes (including contentless/external/pending) verified; stored virtual data-only append verified; contentless/external modes require full export, module compatibility and full native/original acceptance pending |
| Export → Table(s) as CSV… | partial | Multi-table/view chooser across namespaces; full rows, header/custom separator/quote/newline and safe staged per-file replacement; linked SQLite numeric text and printable text/base64 binary; cancellable background row/file progress preserves destinations and pending DB edits; native picker/byte-format edge acceptance pending |
| Export → Table(s) to JSON… | partial | Multi-table/view chooser across namespaces; full rows, typed exact integers/null and base64 BLOBs, pretty/compact output and safe staged replacement; cancellable background row/file progress; native picker/byte-format edge acceptance pending |
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
| Create Table… | partial | Main/temp/attached schema selection; editable declared types with suggestions and genuine untyped columns, PK/AI/NN/Unique/Default/Collation/first column CHECK/inline FK/generated expressions, STRICT/WITHOUT ROWID, bounded scrolling form; full DB4S constraints/migrations/moving tables remain. |
| Create Index… | partial | Qualified table selection and atomic original-identity replacement; live expression/collation/order and partial WHERE editing, available/ordered key lists, add/remove/move/edit actions and a read-only generated SQL preview, declaration types and an ASC/DESC selector that preserves key expressions. |
| Modify Object… | partial | Native column/table rename (including column swaps/case-only changes) preserves identities and qualified browse/format/plot settings; Declared type, NN, single-column UNIQUE, default and column order now rebuild safely with existing constraints/dependencies, rowids/sequence and pending edits preserved. Primary-key membership can also be added/changed/removed; AI can be enabled/disabled with type/key synchronization and sequence-safe rebuild; main/temp/attached moves also retain qualified browse/format/plot/project settings, sparse rowids, composite keys, generated data, indexes/triggers, sequence and pending deferred foreign keys. Target conflicts/read-only/new violations/broken dependencies roll back atomically; dependent views/chains and view triggers now move with qualified SELECT/WHEN/DML/UPSERT references and view settings; explicit aliases remain stable. Original dialog polish and full constraint workflows remain. Temp trigger/view rename has a locked-engine limitation with atomic rollback. Expression/partial index definitions edit in the qualified wizard. |
| Delete Object… | done | confirm + DROP |
| Preferences… | partial | Font/theme/SQL/general options and five-group toolbar display styles; bounded scrolling with fixed Save/Cancel and retained draft controls. Full upstream preferences remain. |

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
| Structure toolbar: New Table, New Index, Modify, Delete, Refresh | partial | Basic qualified table/index wizards and Delete cover all namespaces; definition previews outside main stay read-only. Expression/partial definitions, ordered key actions/types/ASC-DESC and preview now edit across namespaces; safe definition/reorder rebuilding now covers all namespaces; named inline/table PK/UNIQUE, table CHECK/FK pages and original tab order now edit with safe identity/transaction preservation. Cross-database moves now share the qualified Modify Table draft and safety checks; dependent views and their triggers now move atomically; original selectors/main-dialog polish and native acceptance remain. |
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
| Multi browse tabs | partial | Add/switch/close guard cell drafts, SQL ownership and query failure. Valid cached rectangles restore by typed rowid/primary key (or unchanged typed rows for unkeyed sources), with directional anchors, scroll and current editor; last-tab close clears the editor. Reorder, per-tab close affordances and whole-result selection remain. |
| Add / Delete row | done | New Record form supports Value/NULL/Default and required fields; Delete uses an unshadowed rowid alias or every typed primary-key component; stale/ambiguous keys and ignored trigger writes fail atomically |
| Record identity | partial | Every Browse edit, rectangle, delete and FK navigation uses exact SQLite values with an unshadowed rowid alias or all ordered primary-key components. WITHOUT ROWID and shadowed/generated rowid names work; ambiguous/stale keys fail without partial writes. Real-table key edits restore selection using affinity-normalized RETURNING keys. Custom view keys, optional row-id display and moved virtual-table keys remain. |
| Edit cell → Edit Cell dock | partial | Session-preserving Text/Hex/Image/JSON/XML/Evaluation views, explicit typed Apply, readonly guards and row/focus restoration. Persisted toolbar styles, bounded file drafts, Auto mode, Word wrap/horizontal viewports and Indent/Compact; fitting words and CJK punctuation use shared Unicode15 opportunities with grapheme fallback. Raw UTF8 validation, source-bound malformed-format confirmation/deferred Save All, typed unchanged/ignored writes, quoted targets, UTF8 BOM preservation and stored infinities are verified. Dedicated adaptive image preview retains tool focus. Live visible UTF16 character counts, Hex byte sizes and rejected paste diagnostics, NULL/image dimensions and record/result captions update with typing/Undo/Redo and retained tooltips. Dedicated address/Hex/ASCII editing, bounded byte-delta histories, insert/overwrite and readable copy/export are implemented. File/link import, default-application URL/file opening and source-guarded external-copy Apply/Cancel are implemented; readonly copies only close. RTL, full binary settings/context actions, full codecs/UTF16/32, cross-mode history, shaping/non-LF/style wrap, Print/context actions and shell/docking polish remain. |
| Frozen columns | partial | Freeze through the active column; per-table/tab overrides, JSON/extension and native XML counts restore. Legacy global counts retained. Header right-click freezing supported; native viewport indices now translate the leading row-id column (caca01e); row-id display remains. |
| Conditional formatting | partial | Table-scoped background rules persisted in projects (F11 addressed); no rule edit/remove UI or font/alignment controls. |
| FK click-navigation | partial | Implicit parent PK, composite grouping and stored typed keys supported; NULL reports no parent. Source requires an unshadowed rowid (F05 addressed). |
| Table selector / cycle | partial | Dropdown and Ctrl/Cmd+PageUp/PageDown share guarded switching through tables/views with wraparound. Query failure restores the previous context and reports errors. Main/temp/attached identity follows selectors, tabs, navigation, edits, typed insert, delete, cut/paste, FK lookup and full filtered export. Per-database/table filters, ordering, widths and frozen counts restore across switches and JSON/XML projects. Missing temp targets reopen empty with a notice; temp contents remain connection-only. Hidden/formats remain. |
| Record / form view | partial | First/Last and one-based Go to record cover the full filtered/sorted result, preserve the column, reveal the target and guard drafts/busy/query errors. Form arrows cross pages; empty results remain unselected. Previous/Next uses live visible rows minus one. Whole-result virtual scrolling, original whole-row/top alignment and full form controls remain. Plain PageUp/PageDown and Ctrl/Cmd+Home/End reach the full result; Shift ranges remain cache-local. |

## Edit Pragmas tab

| Feature | Status | Notes |
|---------|--------|-------|
| Scroll form of pragmas | partial | All original18 fields plus cache_size/busy_timeout; accessible boolean/enum/numeric controls and retained focus/Undo/scroll. Exact i64 numeric steppers, shared Undo/resize history and measured label-column alignment verified; 500/150ms pointer hold, pause/rearm, cancellation and active contrast verified; native original/wheel/modifier acceptance remain. |
| Save / Apply pragmas | partial | Validated before any COMMIT; dirty edits require default-Cancel confirmation. Writes read actual state; page_size/auto_vacuum run VACUUM and clear cached row identities. File persistence and partial errors verified. VACUUM is synchronous; native input acceptance remains. |
| Bool / Select editors | done | All original boolean/enum fields, including Page Size, Temp Store and secure_delete Fast retained. |

## Execute SQL tab

| Feature | Status | Notes |
|---------|--------|-------|
| SQL editor | partial | Shared TextArea now has pointer/drag and keyboard selection, scrolling, undo/redo, IME replacement and rebuild state (F01 addressed). PageUp/PageDown with Shift selection now moves the caret/viewport by visual lines. Vertical keys retain the desired column through short lines and same-layout sessions. Optional shared physical line numbers are enabled in SQL/Find, with wrap/scroll/pointer/IME alignment. Lexical token colors cover runtime keywords, literals, quoted names, parameters, functions and comments. Cached keyword/function/schema/table/column suggestions support automatic/manual triggering, quoted/qualified replacement and undo. Original extra keyword/type words now share syntax/completion; keyword output case is a persisted draft preference. Cached runtime function signatures now show overloads, current argument, variadic and zero-argument forms in passive hints; nested calls/comments/quoted commas preserve context and Escape suppression survives editor sessions. Original-covered functions now have parameter roles/descriptions, overload controls and bounded wheel/keyboard help scrolling; Option/Ctrl word movement, Shift selection and word deletion preserve Unicode word boundaries and editor history; full syntax styling and complete grapheme-aware navigation remain. |
| Execute / message / results table | partial | Complete final-statement typed result is indexed during execution and scrolled through an on-demand table source. Global End/Page/Shift/copy, directional selections, widths/viewports and snapshots belong to each SQL tab; full Plot selections reveal global result rows without SQL replay. Selected endpoints read the complete typed snapshot in the source-guarded read-only Edit Cell dock, including Text/Hex/JSON/image views; empty SQL selection clears and Browse return restores its model. Original sort/copy-format and full cell-session details remain. |
| Toolbar: Open Tab, Open/Save file, Execute, Stop, Find | partial | Compact icon actions/full help fit800px main pane; selection/whole-script execution, source-bound Save/Save As and shortcuts; external file changes reject overwrite; opening preserves existing drafts. Original modes/complete actions and overflow remain. |
| Multi SQL tabs | partial | Independent drafts/sessions/results, file names and dirty markers; Close Tab, move arrows and Cmd/Ctrl+T/W/S/Shift+S. Pointer tab dragging, close affordances and rename remain. |
| Find / Replace | partial | Shared editor in Find panel highlights/reveals matches; Replace acts on the selected match and Replace All is undoable (F04 addressed). Match case/whole words/wrap/backwards/selection-only/PCRE2, Find All highlights and capture replacement are implemented; upstream regex dialect, large-document worker and native acceptance remain. |
| Async query + cancel | done | Live connection moves to worker; Write/Revert/keys/ATTACH shared; completion polled on redraw; Stop interrupts |
| Results → CSV / JSON / Save as view | done | Full immutable typed snapshot export without re-execution, shared CSV/JSON format options in compact modal, exact integers/null/base64 and atomic output; editable view name/source tied to executed statement. Cancellable background row/file progress; native input/byte-format edge acceptance pending. |
| Toggle comment | partial | Current/selected physical lines, first-line mode, exact two-hyphen removal, LF/CRLF preservation and one undo/redo edit. Ctrl/Cmd+/ and toolbar retain tab/file sessions. CR-only editor lines and native acceptance remain. |
| Execute current line | done | Uses Unicode caret position; never replaces editor contents; one physical line |

## Docks

| Dock | Status | Notes |
|------|--------|-------|
| Edit Database Cell | partial | Typed multiline/Hex/JSON/XML/image views and explicit SQLite expression evaluation; complete read-only SQL cells retain source identity and raw storage bytes without replay or Browse writes. Inactive/read-only/generated/unsupported-row-identity selections block writes. Source-scoped sessions, file drafts and row-identity restoration after Apply verified. Address/Hex/ASCII byte editing, insert/overwrite and readable copy/export are implemented. File/link/default-application and private external-copy Apply/Cancel actions are implemented. Full RTL/binary settings/context/format settings/error flows/print/custom view keys/row-id display/native acceptance remain. |
| DB Schema | partial | Flat main/temp/attached object list; no column tree, filter or SQL insertion. |
| SQL Log | partial | SQLite statement/trigger tracing with expanded ordinary values; User/Application/Error/All, full read-only text, copy, scoped clear/export and visible eviction counts (200/category). Global SQLite diagnostics and native acceptance remain. |
| Plot | partial | Current typed Browse/SQL sources, named X/Y1/Y2 matrix, independent series styles/colors, legend, selection, retained navigation and canonical native project settings. Cancellable Load All uses the live Browse connection or immutable SQL snapshot without replay/commit; cached full curves survive matching page changes and out-of-page Browse point reveal. Shared Cartesian rendering supports date/time/category/dual-Y, fifteen markers and bounded raster output. Save… writes current viewport/selection/legend to PNG/JPEG/BMP/lossless raster PDF with staged replacement and database protection. Copy/Cmd+C renders the current view to the native image clipboard. Constrained controls wrap and Plot dock minimum sizing retains preferred split ratios. Current-view context Copy/Save/Fit/legend/fixed numeric formatting and signed grouped/stacked bars with row hit tests and category ticks verified. Cached named X/Y1/Y2 pointer tooltips, caption help and passive bounds/routing are verified. Table-to-Plot highlighting, Print/vector PDF, exact conversion/control/rotation/custom-style details, dense native throughput and original interface/native/platform acceptance remain. |
| Remote | partial | Local path list; cloud protocol/auth/sync not implemented |

## Dialogs

| Dialog | Status | Notes |
|--------|--------|-------|
| EditTableDialog (full wizard) | partial | Qualified create/modify/rebuild, stable column identities/generated columns, named multi-constraint drafts, original four grids and resizable SQL preview; direct Fields text/checks, constraint text/type/reference cells and ordered local-column dialogs. Existing-field confirmation, linked constraint removal and safe dependency rollback verified. Main/temp/attached movement now saves through the Advanced schema selector, retains related browse/project identities, and rejects unsafe dependencies atomically. Dependent view/chained-view/trigger movement and alias-aware view settings are implemented; structural history, original combo/main-dialog/anchored-popup polish and native acceptance remain. |
| EditIndexDialog | partial | Qualified expression keys/unique/partial WHERE, live definition retention and atomic replacement; available/ordered lists, add/remove/move/expression/edit controls and read-only SQL preview. Multiline SQL, focus/selection/undo and list viewports survive rebuilding; ASC/DESC changes preserve operands/comments/collation and share Undo; declaration types include generated columns. Exact original transfer filtering/interface and native acceptance remain. |
| ImportCsvDialog (delimiter, quote, encoding, …) | partial | UTF-8 plus BOM-detected UTF-16LE/BE; malformed units/surrogates and unsupported UTF-32 BOMs are rejected before writing; delimiter/quote/header and multiline quoting supported; legacy code pages and an explicit encoding picker remain |
| ExportSqlDialog (options) | partial | Four switches, all tables; no multi-table selection, IF NOT EXISTS mode, INSERT batching or original-CREATE choice. |
| CipherDialog | partial | Unlock/export copy with password/page/KDF only; no rekey/remove encryption, raw key, HMAC/KDF algorithms or SQLCipher 3 preset. |
| PreferencesDialog | partial | Font/theme/drop confirmation/CSV delimiter/frozen count/SQL auto completion/keyword case; Cancel preserves live settings (F11 addressed); full upstream preferences absent. |
| FindReplaceDialog | partial | Six search options, Find All and disjoint highlights, Unicode captures/escapes, F3/Shift+F3 and atomic undoable replacement; PCRE2 is not certified Scintilla regex compatibility. One-second scan budget; large backward searches still need a cancellable worker. |
| EditDialog (blob/image/hex) | partial | Multiline text/JSON, typed/NULL values, Hex and RGBA preview; Fit/1:1/zoom/pan/original image export supported (F12 addressed). Address/Hex/ASCII byte editing, insert/overwrite, byte selections/history and readable copy/export implemented; configured binary font/context and print workflows remain. |
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
- SQL result export streams the full final result to a private temporary snapshot, then exports CSV/JSON without rerunning statements. The SQL grid reads all indexed rows on demand; the Plot keeps an optional500-row preview before Load All. Result snapshots are private plaintext files and consume temporary disk space until replaced/closed. BLOBs and non-finite numbers use strings in JSON; SQL dumps remain the lossless format.
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
  Canonical native Browse Plot axes/colors/active/per-series styles are applied and saved. Qt dock state and advanced browse filters/formats are retained but unapplied. Temporary table contents remain connection-only; missing temp browse targets reopen empty with a notice. Extra local state lives in a
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

### Column CHECK editing progress (2026-10-28)

[Review](../../docs/reviews/R-20261028-716541440405.md): first column CHECK reads/edits/removes through the safe rebuild, retains names and all untouched constraints/dependencies, validates data and rejects SQL tails atomically. Multiline SQL sessions follow column switch/reorder/add/remove. Shared TextArea now retains active composition, with focused CHECK/SQL/index restoration and current theme colors. 451 tests/strict checks, four Soft/Metal pairs and two native static forms inspected. Advanced column/table CHECK/FK/composite/generated/options editors, move, original grid/tab/preview layout and native acceptance remain.

### Table option editing progress (2026-10-28)

[Review](../../docs/reviews/R-20261028-afe386bb71d9.md): STRICT and WITHOUT ROWID switches load/save real flags through CREATE/rebuild and reset for new tables. Qualified conversions preserve data/dependencies/settings/pending transactions, ordinary rowids and requested PK nullability; incompatible values/types/keys/AI and ambiguous FK identities roll back. No-op options preserve custom type spelling; errors show actual table/column. 454 tests/strict checks, four Soft/Metal pairs and native static forms inspected. Primary-key conflict policy, full advanced constraints/generated/move, original grid/tabs/preview/polish and native input acceptance remain.

[Review](../../docs/reviews/R-20261028-287e6c8d2dea.md): primary-key Default/ROLLBACK/ABORT/FAIL/IGNORE/REPLACE loads/edits/clears through CREATE and qualified rebuild, preserving key names/order/collation and independent policies. Copy conflicts always abort without row loss or outer rollback. Actual GUI rollback errors now recover an unwritten transaction, invalidate stale row identities and retain failed input; corrected edits remain revertible. 460 tests/strict checks, four Soft/Metal pairs and native static forms inspected. Full advanced constraints/original layout/move/polish/native acceptance remain.

[Review](../../docs/reviews/R-20261028-df5946de66c9.md): first inline foreign key loads/edits/removes with action/deferral clauses, named and separated deferrals, other inline/table keys and dependencies preserved. Missing/non-unique parents/orphan data/injection fail atomically; CREATE/ADD enforce references. Column SQL editors retain independent sessions/IME through switches/reorders. Pinned parser named-DEFERRABLE omission fixed without executing temporary checks. 466 tests/strict checks, four Soft/Metal pairs and native static captures inspected. Full named/composite/all-constraint pages/generated/move/original layout/native acceptance remain.

### Generated-column editing progress (2026-10-28)

[Review](../../docs/reviews/R-20261028-d0bb5af15e83.md): generated columns load in structure editing and save VIRTUAL/STORED expressions through the qualified atomic rebuild. Conversions recompute or freeze values as requested; named constraints, dependencies, rowids/sequence and pending edits survive, invalid definitions/data/FK changes roll back. Browse values are read-only, ordinary edits recalculate, insert/SQL-copy/dump omit computed fields while plain copy/CSV/JSON retain results. Multiline sessions follow identities. 472 tests/strict checks, four Soft/Metal pairs and native static draft/calculated Browse inspected. Full advanced constraint pages/move/original grid/tabs/live preview/polish/native acceptance remain.

### Table SQL preview progress (2026-10-28)

[Review](../../docs/reviews/R-20261028-5caa315d88e3.md): fixed live CREATE TABLE preview retains existing constraints and uses pure AST column/table/self-reference renames, preserving literals/functions/external identities without querying or mutating the database. Formatted read-only SQL updates immediately, supports selection/copy and survives themes/rebuilds/resizing. Geometry and own scrolling keep preview/Save available at800×600. Actual saved AST equality and 475 tests/strict checks, four Soft/Metal pairs/native top/bottom forms pass. Full named/composite/all-constraint pages, original field grid/tabs/advanced disclosure/move/polish/native acceptance remain.

### Named table CHECK progress (2026-10-28)

[Review](../../docs/reviews/R-20261028-8e840a8e2ed1.md): Check Constraints page edits optional names and multiline SQL, adds/removes/reorders all table CHECKs and updates the fixed preview. Qualified atomic rebuild preserves inline constraints/dependencies/rowids/sequence/settings/pending data; violations/injection fail without mutation. Independent undo/redo/composition survive page switches/reorders; failed drafts remain correctable. 478 tests/strict checks, four Soft/Metal pairs and native draft/saved windows inspected. Edited CHECK rename rebinding, remaining advanced pages/original grid/layout/move/polish/native acceptance remain.

### CHECK reference identity progress (2026-10-28)

[Review](../../docs/reviews/R-20261028-6fdb899a760b.md): edited and unedited table CHECKs follow column/table renames, temporary duplicate/empty names and swaps with stable identity mappings. Malformed expressions keep SQL and reject renames; bindings follow column reorder/add/remove. Qualified local inline/table CHECKs survive temporary rebuild; preview/native/rebuild share one decision and match saved constraints.483 tests/strict checks, four Soft/Metal pairs and final native draft/saved windows inspected. Changed reference expressions reset their CHECK edit history; wizard Undo, all-constraint editors/original layout/move/polish/native acceptance remain.

### Named composite foreign-key progress (2026-10-28)

[Review](../../docs/reviews/R-20261028-62d67efc8c2c.md): Foreign Keys page edits all table FK names, ordered local columns and REFERENCES/action/deferral SQL; add/remove/reorder, independent undo/redo/composition, errors/correction/Create and fixed preview verified. Shared identity maps retain local/self/external parent semantics; qualified atomic migration preserves dependencies/data/sequence/settings/pending edits. Actual cascade/update, delete/set-null and deferred repair pass. CHECK/FK viewport restoration now runs after layout and survives small-window rebuilds.487 tests/strict checks, four Soft/Metal pairs and final native draft/saved windows inspected. Reference rewrites can reformat SQL/reset editor history; Index Constraints/all inline exposure/original grid/selectors/layout/move/native acceptance remain.

### Named PK/UNIQUE constraint progress (2026-10-28)

[Review](../../docs/reviews/R-20261028-08a2792a44c9.md): Index Constraints edits every inline/table PK and UNIQUE name, ordered SQL columns/collation/ASC-DESC and conflict action, with add/remove/reorder, independent editing sessions and Fields PK/AI/Unique synchronization. The four pages now follow the original order. Qualified preview/save and real policy enforcement retain dependencies/data/rowids/sequence/pending edits; duplicate migrations never drop data. Inline INTEGER PRIMARY KEY DESC stays inline/pinned; separate key collations require Fields to avoid changing identity. Failed membership changes restore draft names/type/bindings.492 tests/strict checks, four Soft/Metal pairs and two native static windows inspected. Exact grids/selectors/advanced disclosure, complete column constraints, wizard-wide Undo/history transforms, movement and full product/native parity remain.

### Complete inline foreign-key exposure (2026-10-28)

[Review](../../docs/reviews/R-20261028-af285645d2ec.md): Foreign Keys now exposes every inline reference alongside table FKs, retaining owner/name/actions and named/separated deferrals. Fields edits/removal, scope-preserving reorder, stable-owner column deletion and composite conversion share one draft; pure qualified previews/save preserve local/self/external identities, dependencies/data/rowids/sequence and pending transactions. Actual cascades/deferred failures/repair pass.496 tests/strict checks, four Soft/Metal pairs and two native static windows inspected. Cross-view changes reset linked history; full column CHECK exposure, original grids/selectors/layout/history/movement/details and full product/native acceptance remain.

### Complete column CHECK exposure (2026-10-28)

[Review](../../docs/reviews/R-20261028-7f33ef98dafd.md): Check Constraints exposes all named column CHECKs beside table CHECKs, with explicit scope, Add column for the selected field, independent expressions/names and scope-preserving reorder/removal. Fields first-CHECK editing/removal shares the complete draft and reveals remaining checks. Qualified previews/save/swaps/dependencies/data/rowids/sequence/pending transactions and SQL rejection are verified.500 tests/strict checks, four Soft/Metal pairs and native static windows inspected. Existing TEMP-trigger referenced-column rename failure rolls back; linked history resets, original grids/selectors/layout/history/movement/details and full product/native acceptance remain.

### Fields grid and endpoint movement (2026-10-28)

[Review](../../docs/reviews/R-20261028-cdf760f3ad1d.md): Fields displays the original ten property columns with frozen names, resizable widths, retained scroll/cell/row/focus and live draft updates. Top/Bottom move complete identities/editor sessions with qualified migration/data/dependency checks.502 tests/strict checks, four Soft/Metal pairs and final native dark/light windows inspected. Native focus/select-all/paste rename updates dependent SQL; single-line Undo is missing (F01). Inline editors/checkboxes, original selectors/advanced layout, full history and remaining product/runtime acceptance are unfinished.

### Single-line Undo and rebuild sessions (2026-10-28)

[Review](../../docs/reviews/R-20261028-56b5b3eebc46.md): Shared TextInput/Autocomplete now retain bounded Undo/Redo for editing, clipboard, IME and suggestion selection, plus matching-document sessions. Focused table name/column name/default survive rebuild; constraint identity/preview/actual migrated values and enforcement are verified through Undo/Redo.505 tests/strict checks, two Soft/Metal pairs and15 native static windows inspected. Native shortcut re-test unavailable (black desktop/inactive target), so F01 remains in_progress. Full structural history, sessions across field switches, inline editing and original layout/product/runtime parity remain unfinished.

### Global table header / Advanced / native Undo (2026-10-28)

[Review](../../docs/reviews/R-20261028-0da0f3c214c0.md): Table name and Advanced options now sit above all four pages; hiding options preserves schema/flags/conflict. Small editors, preview and Save remain reachable at800×600. Schema changes refresh page captions, and table-name history survives disclosure/reset between drafts. Shared Checkbox uses the existing Check icon instead of a missing font glyph.506 tests/strict checks, four Soft/Metal pairs,14 Office windows and final native captures inspected. Actual native column/table Undo/Redo, mouse/keyboard disclosure, Save and Browse preserve the two original records/generated values; single-line F01 now verified. Inline field/checkbox editors, original constraint grids/selectors, full history/movement/details and broader product/runtime parity remain unfinished.

### Direct Fields checkboxes (2026-10-28)

[Review](../../docs/reviews/R-20261028-672e462f70c8.md): NN/PK/AI/Unique can be toggled in the Fields grid by pointer or Space. Shared Table supports opt-in checks with frozen/scrolled geometry and visible unchecked outlines. Cross-row draft capture, invalid generated PK rollback, AI/type/membership coupling, invalid WITHOUT ROWID Save and qualified data/constraints are verified.508 tests/strict checks, two Soft/Metal pairs (delta1), both native themes and14 Office static captures inspected. Native input unavailable (black desktop/inactive target); inline text editors, original selectors/grids/full history and overall product/runtime parity remain unfinished.

### Inline Fields text editors (2026-10-28)

[Review](../../docs/reviews/R-20261028-0ff71c70e6fd.md): Name/Type/Default/Collation now edit in place with F2, typing, selected click, Tab/BackTab, Enter/Esc and standard history/IME/suggestions. Shared input selection contrast, long caret and frozen-filter clipping pass.512 tests/strict checks, four Soft/Metal pairs (delta1), both native themes/14 Office static captures inspected. Desktop locked; actual native input, inline SQL cells, original constraint grids/selectors, resizable preview, full cross-field history and broader product/runtime parity remain unfinished.

### Original constraint grids (2026-10-28)

[Review](../../docs/reviews/R-20261028-ce1ec4558b01.md): Index/FK/CHECK pages now show the original columns in frozen/resizable/scrollable shared tables. Draft-safe selection/focus/view persistence, validated SQL fragments, quoted multiline copy and actual Save retain rowids/sequence/view/trigger/enforced constraints.515 tests/strict checks, eight Soft/Metal pairs (delta<=2), six native pages/14 Office static windows inspected. Direct constraint editors/selectors, inline SQL, resizable preview, full history/movement/details and broader product/runtime parity remain unfinished; native input unavailable on locked desktop.

### Resizable table definition and SQL preview (2026-10-28)

[Review](../../docs/reviews/R-20261028-bafb781fb129.md): Original-style vertical split now resizes live and retains ratio across pages/rebuild/resize. Shared splitter minima and viewport-filling scroll pages prevent overlap and preserve sash access; hidden pages no longer receive keyboard focus. Read-only preview sessions, small-window controls and actual migrated data/view/CHECK verified.518 tests/strict checks, four Soft/Metal pairs (delta2), ten native configurations/14 Office static captures inspected. Native drag/input, direct constraint selectors/editors, inline SQL/full history/movement/details, original dialog polish and broader product/runtime parity remain unfinished.

### Direct constraint text cells (2026-10-28)

[Review](../../docs/reviews/R-20261028-cf5fc8ffa9e6.md): Index names, FK names/references and CHECK names/expressions now edit in place through the existing validated drafts. Cancellation, typing/F2/Tab/pointer activation, blur, resize/rebuild and active-cell Undo/Redo are verified with actual SQLite rowid/data/sequence/view/trigger, ordered keys and deferred composite FK semantics. Shared TextInput/Autocomplete display multiline paste on one line while retaining raw SQL/copy/history and matching IME scroll/secret geometry.520 tests/strict checks, four Soft/Metal pairs and six native pages/14 Office static windows inspected. Native input, ordered/reference selectors/type switching, inline Fields SQL/full cross-view history/movement/details, original dialog polish and broader product/runtime parity remain unfinished.

### Primary/unique type cells (2026-10-28)

[Review](../../docs/reviews/R-20261028-4731f9c09196.md): Index Type cells use the shared standard Select. Atomic validation rejects duplicate primary/generated keys, cancellation restores AUTOINCREMENT and full key metadata, and real Save preserves ordered key semantics/rowids/data/generated values/view/trigger while enforcing intentional type changes. Popup/whole-cell Escape staging, Tab, resize/rebuild and visible correction status/database flags are verified.522 tests/strict checks, four Soft/Metal pairs (delta2), final native type popups/14 Office static windows inspected. Native input, custom-content modal/ordered and reference selectors, all-row combo presentation, inline Fields SQL/full cross-view history/movement/details, original dialog polish and broader product/runtime parity remain unfinished.

## Ordered constraint column dialogs (2026-10-28)

[Review](../../docs/reviews/R-20261028-fe17276209d7.md): Index/FK Columns cells open Available/Selected dialogs with Add/Remove/Up/Down/Apply/Cancel, Enter/double-click and retained long-list viewports. SQL operands keep duplicate names/COLLATE/ASC/DESC, unchanged comments and constraint metadata; full proposed SQL validates before retaining changes. Real Save preserves rowids/data/sequence/view/trigger and composite FK actions/deferral. Shared custom-content dialogs handle nested focus, child popup/completion ownership, body/footer geometry and background blocking.532 tests/strict checks, eight Soft/Metal pairs (delta2), four final native pickers/14 Office static windows inspected. Native input, referenced-target selectors, original dialog/anchored-popup presentation, inline Fields SQL/full cross-view history/movement/details and broader product/runtime parity remain unfinished.

### Composite foreign-key reference cells (2026-10-28)

[Review](../../docs/reviews/R-20261028-e81d8acc78c7.md): Foreign Keys References cells now mount parent selection, ordered target completion, clauses and Reset. Same-schema/self draft identities, composite/implicit-primary targets, actions/named deferrals, unchanged SQL, cancel/history/resize and removal navigation are verified. Actual Save retains data/view, cascade/SET NULL and remaining inline FK/CHECK; invalid SQL remains a repairable draft. Shared Row cell editors handle child layout/focus/navigation/clipping.538 tests/strict checks, six Soft/Metal pairs (delta≤2), final native windows/14 Office static captures inspected. Fields reference entry/disabled clause behavior, original combo/dialog presentation, inline Fields SQL/full history/movement/details, native input and broader product/runtime parity remain unfinished.

### Fields foreign-key cells and disabled inputs (2026-10-28)

[Review](../../docs/reviews/R-20261028-68add2bacda7.md): Fields reuses parent/ordered target/clauses/Reset, preserving the first inline FK and every other named FK/CHECK through clear/cancel/blur, raw repair, resize/history and actual Save. Empty parent disables clauses through the shared TextInput enabled state; navigation and dialog focus restoration respect it. First-column self-reference drafts now use the current table during a simultaneous table/key rename and rebuild, with main/temp/attached links/cascades/pending transaction rollback verified.543 tests/strict checks, six Soft/Metal pairs (delta≤2), four native/14 Office static windows inspected. Inline Fields CHECK, original combo/dialog presentation, full history/movement/details, native input and broader product/runtime parity remain unfinished.

### Fields CHECK cells (2026-10-28)

[Review](../../docs/reviews/R-20261028-422ba9e69372.md): Fields CHECK edits in place with F2/typing/Tab, Enter/Escape, raw SQL/history and first-constraint identity retained through clear/cancel. Other named CHECKs remain intact; final removal reveals the next CHECK. Attached-schema Save retains data/generated values/view/trigger/sequence and enforces the intended checks. Active Fields editors reveal after resize, including800px.544 tests/strict checks, four Soft/Metal pairs (delta≤2) and both final native static themes inspected. Structural/cross-view history, deletion linkage, original combo/dialog presentation, movement/details and broader product/native/runtime parity remain unfinished.

### Field removal and modal quality (2026-10-28)

[Review](../../docs/reviews/R-20261028-08bbf70ff846.md): Existing fields require confirmation with Cancel default, restored grid focus and atomic stable-identity removal. Named ordered PK/UNIQUE, conflicts/collations and inline constraints stay synchronized; composite FK parent definitions remain repairable. Qualified migration replaces automatic indexes, validates native dependencies after rebuilding, blocks quoted-view DQS fallback and restores pending transactions/settings on failure. Discarded generated/NOT NULL rules do not constrain retained edits, including STRICT. Shared modal alpha/contrast defaults improve confirmation context.549 tests/strict checks, four Soft/Metal pairs (delta≤2), both native themes/14 Office static windows inspected. Manual/expression dependencies still require explicit migration; table movement/details, original combo/dialog presentation, full history and broader product/native/runtime parity remain unfinished.

### Fields type and collation dropdowns (2026-10-28)

[Review](../../docs/reviews/R-20261028-82c5b65cb2f8.md): Type/Collation controls now appear in every visible row and activate on first click. Original ordinary/strict type lists/order, custom type cancel/Save and noneditable strict/collation choices verified; real Save preserves rows/default/PK and applies collation semantics. Shared table dropdown metadata, all-options autocomplete sessions, bounded Select popup geometry/visible-index pointer/wheel selection and selected text/light Tabs contrast are covered.565 tests/strict checks,12 Soft/Metal pairs (delta1), six native static fixtures/14 Office windows inspected. The inactive native windows lost open popups: no native popup/input acceptance claimed. Select popup retention across rebuild/resize, original modal/focus/detail presentation, engine TEMP limits and broader product/runtime parity remain unfinished.

### Cell Select popup retention (2026-10-28)

[Review](../../docs/reviews/R-20261028-6030849e55c7.md): shared value-checked Select sessions retain focused Fields/constraint type/foreign-key parent popup state across rebuild/resize. Closed popups stay closed; changed values reject stale restoration; staged Escape restores only the active draft.567 tests/strict checks,16 Soft/Metal pairs (delta1) and six visible native static popups inspected. Native fixtures programmatically prepare the open dropdown, so OS pointer/keyboard/resize/IME/picker acceptance is still absent. Original dialog/focus/details, engine TEMP limits and broader Browse/Plot/import/export/preferences/runtime parity remain unfinished.

### CSV file encodings — 2026-10-28

[Review](../../docs/reviews/R-20261028-7ef6e54e267d.md): original UTF-8/UTF-16/ISO-8859-1/Other selector and custom completion now decode supported legacy codecs and UTF-32, including BOM override. Strict malformed/unknown-codec errors occur before mutation; real imports and rollback preserve existing pending changes. Control sessions survive rebuild/resize and corrected labels import successfully; clipboard remains Unicode.572 tests/strict checks, eight Soft/Metal pairs (delta1–2) and four native static windows inspected. CSV preview/trim/type/default/conflict/multifile/custom separators/settings/Qt-only codecs and broader product/native parity remain unfinished.

### CSV preview and import policies — 2026-10-28

[Review](../../docs/reviews/R-20261028-db71e556cb1f.md): files decode in chunks and share a parser with Unicode clipboard input; preview stops at20 records and displays inferred types. Custom character/codepoint separators/quotes, trimming, type detection, defaults/missing values and Abort/Ignore/Replace policies reach actual SQLite imports. Existing tables match writable-column position with default-Cancel confirmation. Saved settings, focus/Undo/scroll/refresh and pending-transaction safety are verified.577 tests/strict checks,12 Soft/Metal pairs (delta1–2), six native static windows inspected. Multi-file/separate tables, local number conventions/Qt-only codecs, cancellation/progress/streamed insertion/parser compatibility, original modal presentation and broader product/native parity remain unfinished.

### CSV multi-file workflow and abort safety — 2026-10-28

[Review](../../docs/reviews/R-20261028-83bec7daaef8.md): native multi-file paths, checkbox selection, Select All/Match Similar, shared/separate per-file names, original Yes/No/YesAll/Cancel default-No confirmation and partial retry now reach actual SQLite imports. Confirmation snapshots, focus/width/scroll/resize sessions, original header-off/trim-on defaults and explicit trigger rollback recovery pass. A reproduced inherited ROLLBACK-policy data-loss bug is fixed by explicit OR ABORT.587 tests/strict checks,16 Soft/Metal pairs (delta1–2) and six native static windows inspected. CSV locale/Qt codecs/worker progress/cancellation/streamed insertion/parser compatibility/original modal and broader interface/product/native parity remain in_progress.

### CSV validated streaming snapshots — 2026-10-28

[Review](../../docs/reviews/R-20261028-16d823ae8faf.md): actual file/clipboard imports now validate to an anonymous private snapshot, retain the first20 raw records for preview/type detection, and insert rows incrementally through the shared savepoint/policy engine. Independent positional readers and exact record counts reject corrupted/truncated snapshots; current confirmation retains the same accepted data.50,000 multiline rows and SQLite count/sum/rollback,589 tests/strict checks and two current native static confirmations verified. Memory still includes sampled/largest records and temp disk scales with parsed data. Background progress/cancellation, local conventions/Qt codecs/parser compatibility/original modal and broader product/native parity remain unfinished.

### CSV background progress and cancellation — 2026-10-28

[Review](../../docs/reviews/R-20261028-15c13498159d.md): real file/clipboard preparation and streamed SQLite insertion run on workers, with byte/record progress and modal Cancel/Escape. Current-file rollback preserves completed files and prior pending edits; retry never duplicates completed files. Database/document actions wait, and simultaneous SQLite errors/trigger rollback remain visible.594 tests/strict checks, eight Soft/Metal pairs (delta1–2) and six native static windows verified. Cancellation waits for an in-flight read/record; snapshot memory/temp disk limits remain. CSV locale/Qt codecs/parser compatibility/original modal and full product/interface/native parity remain unfinished.

### CSV original record semantics — 2026-10-28

[Review](../../docs/reviews/R-20261028-de243e46e128.md): shared file/clipboard parser accepts original quoted segments and post-quote text, expands column names across the first20 records and preserves shorter records. Preview pads display cells; raw snapshots distinguish missing cells (NULL) from explicit empties (default/missing policy). Successful new batch targets are accepted for later files. Reproduced before/after failures,597 tests/strict checks, four new-data Soft/Metal pairs (delta1) and two native static previews verified. Extra fields beyond sampled schema reject before mutation. CSV locale/Qt codecs/remaining parser edges/original modal and full product/interface/native parity remain unfinished.

### CSV system number conventions — 2026-10-28

[Review](../../docs/reviews/R-20261028-15202e836792.md): opt-in OS decimal/group/digit/sign symbols now drive inference and typed integer/real binding on the real CSV worker. Owned metadata is frozen in validated snapshots; default-off/persisted options and unconverted raw text are retained.604 tests/strict checks, four Soft/Metal pairs (delta1) and four native static previews verified. Windows/POSIX localized-sign/exponent fallbacks, Qt-only codecs/parser edges/original modal and complete product/interface/native acceptance remain unfinished. Actual Windows/Linux target installation was unavailable after official Rust SSL timeout; host feature checks do not compile those branches.

### CSV layered modals — 2026-10-28

[Review](../../docs/reviews/R-20261028-1b3a0d8d26b2.md): file/clipboard previews now use real centered modal focus above retained database tabs/docks, fixed Cancel/Import actions and nested existing-table/progress dialogs. Escape/title-close/Cancel, focused edit/Undo/resize, blocked background actions and existing batch recovery pass.605 tests/strict checks,36 Soft/Metal pairs (maxdelta2), eight native Retina windows and limited actual OS toggle/Space/import/Browse/clipboard verified. Qt-only codecs/parser edges/localized Windows/POSIX symbols and complete original/interface/native acceptance remain unfinished.

### Full original PRAGMA catalog and safe apply — 2026-10-28

[Review](../../docs/reviews/R-20261028-2850bcb3abde.md): original18 fields plus two retained extras now share validated accessible controls. Default-Cancel confirmation protects pending edits, invalid input precedes all writes, page/auto-vacuum rebuilds persist, actual states/errors and fresh transactions are retained.623 tests/strict checks,12 Soft/Metal pairs and six native static windows verified. Numeric steppers, shared word wrapping/layout polish, asynchronous VACUUM and complete native/product parity remain.

### Static text word wrapping — 2026-10-28

[Review](../../docs/reviews/R-20261028-f36b581271db.md): shared next-gui modal/tooltip text keeps fitting words intact while preserving paragraphs and long-path/CJK emergency wrapping.626 tests/strict checks, confirmation/calltip Metal comparisons and four native e28/14 rebuilt Office windows inspected. Complete Unicode shaping, long modal scrolling, numeric stepper/alignment and original/native product parity remain.

### Exact integer PRAGMA editors — 2026-10-28

[Review](../../docs/reviews/R-20261028-680be34446cf.md): shared next-gui opt-in i64 TextInput stepping avoids floating conversion, retains clipboard/Undo/sessions and protects composition/disabled/invalid drafts. All numeric PRAGMAs share bounds, text/candidate geometry reserves arrows, and measured labels align controls.629 tests/strict checks,16 render pairs and six browser/14 rebuilt Office native windows verified. Press-and-hold repetition, native OS input and complete original/product parity remain.

### Original SQL export options and progress (2026-10-28)

[Review](../../docs/reviews/R-20261028-fff58fe35415.md): main table/view chooser, original content/schema/column-name/multi-row/original-CREATE settings, safe suggested filenames and connection-owned cancellable progress verified. Selected sequence restoration preserves source and destination high-water marks; typed/generated/trigger ordering and guarded atomic publication retain pending edits and existing files. Final396 example tests, workspace634/43,16 render pairs and eight native static windows pass. Selected virtual tables, original binary/native input/picker/platform and broader product parity remain unfinished.

### Long confirmations and GPU text limits —2026-10-28

[Review](../../docs/reviews/R-20261028-14e5b79b9315.md): shared modal prose scrolls by wheel/keys/thumb within a clipped viewport above fixed actions, preserves matching reading/action state across rebuild/resize and exposes its complete accessible description. Oversized alpha masks now tile beyond actual adapter limits, avoiding a reproduced long-name native GPU abort.640 workspace tests/strict checks, eight render pairs (max2), six browser and14 rebuilt Office native windows inspected. Native OS input/original binary/platform acceptance, complete text shaping and broader product parity remain unfinished.

### Held integer editors and idle-safe host deadlines — 2026-10-28

[Review](../../docs/reviews/R-20261028-a3dc6cdd4f0c.md): shared exact integer arrows repeat with500ms initial/150ms cadence, pause outside, stop at bounds and cancel across release/focus/IME/edit/modal/resize/rebuild. All hosts expose coalesced one-shot redraw deadlines without idle polling; actual macOS deadline and20ms soak probes pass. Browser repeats update only the draft until explicit Save; both-theme pressed contrast corrected.644 tests/strict checks, eight Soft/Metal pairs (max2), six native browser/14 rebuilt Office windows inspected. Native original gestures, wheel/modifiers, actual Windows/Linux runtime and full product/interface parity remain unfinished.

### Virtual SQL indexes and pending snapshots — 2026-10-28

[Review](../../docs/reviews/R-20261028-20580cba51bc.md): virtual roots include their shadow tables as a unit. Full/structure SQL exports restore FTS3/4/5 and RTree, including contentless/external FTS and pending indexes; nested savepoints flush buffers without committing user edits, staged publication preserves destinations. Keep rejects existing roots; explicit Replace rebuilds them.648 tests/strict checks, eight Soft/Metal pairs (max2) and four native static windows inspected. Data-only virtual export, modules without persistent shadows, original/module/native compatibility and complete product/interface/runtime parity remain unfinished.

### Virtual visible-row SQL append — 2026-10-28

[Review](../../docs/reviews/R-20261028-9206737a7701.md): Data only appends stored FTS3/4/5 and RTree/RTree_i32 visible rows through ordinary typed INSERTs, preserving existing data and requested exact rowids; defensive target connections need no shadow writes. Read-only derived modules can export visible rows into ordinary targets. Contentless/external FTS retain the explicit full-export requirement to protect their indexes.650 tests/strict checks, eight Soft/Metal pairs and four native static windows inspected. Module/original/native compatibility, full Plot and broader product/interface/runtime parity remain unfinished.

## Cartesian plotting checkpoint (2026-10-28)

[Review](../../docs/reviews/R-20261028-c987810c86e8.md): shared Chart now supports dual axes, ISO date/time/categories, six line styles,15 point shapes, bars, source highlighting, zoom/pan/keyboard/session state and bounded RGBA rendering. Dense straight-line extrema/gaps and retained image caching are regression-tested. The example uses500-row f64 numeric previews with original source positions;659 tests/strict checks, four actual Metal comparisons and four browser+14 Office native static captures pass. A verified official3.13.1 original four-page baseline is now available. Current Browse/SQL binding, column matrix/style/color controls, linked table selection, Load All, plot file export, original settings and full native/product/interface parity remain unfinished.


## Current result Plot checkpoint (2026-10-28)

[Review](../../docs/reviews/R-20261028-ac9530d409ef.md): current typed Browse/SQL previews now feed the X/Y1/Y2 column matrix, line/marker/RGB/legend controls and linked table selection with cell-draft protection. Source order, NULL/nonfinite gaps, named settings, tab/rename lifecycles, shared buffers and matching views are verified. Shared ColorPicker text layering and empty-palette hex editing are fixed.666 tests/strict checks, eight Metal comparisons and four browser+14 Office native static captures pass. Preview500/within-page selection, full-data loading, native Plot project tags, export/clipboard/print, date/conversion details, partial control sessions and full original/product/native/platform parity remain unfinished; F02 stays in_progress.


## Native Plot settings checkpoint (2026-10-29)

[Review](../../docs/reviews/R-20261029-9ac4fdb9cb96.md): canonical original plot_x_axis/plot_y_axes now read/write named X/Y1/Y2, colors, active states and per-series line/shape settings, including attached and unopened tables, while retaining unknown metadata. Selected-series controls, polling/rebuild safety, source isolation and categorical bars are verified.670 tests/strict checks, eight actual Soft/Metal comparisons and six native static captures pass. Preview500, Load All, out-of-page linkage, exports/clipboard/print, original visual/native/platform/full-product acceptance remain unfinished; stable F02 stays in_progress. Legacy ssDot is approximated, custom styles/noncanonical QColor and translated Row # need further compatibility work.


## Complete Plot loading checkpoint (2026-10-29)

[Review](../../docs/reviews/R-20261029-0135ff155bff.md): cancellable connection-owned Browse/full immutable SQL snapshots now load all rows without replaying RETURNING or committing pending edits. Typed page verification/all-schema data versions guard stale data; matching Browse pages retain full curves/navigation and out-of-page point selection reveals a page. Cached style rebuild/cancel rollback, million-row backend diagnostic,677 tests/strict checks, eight Metal comparisons and six native static windows verified. SQL grid/out-of-preview SQL links, range reveal beyond one page, export/clipboard/print, original style/conversion/interface/native/platform/full-product acceptance remain; F02 stays in_progress.


## Complete SQL grid checkpoint (2026-10-29)

[Review](../../docs/reviews/R-20261029-72c67dde0e91.md): shared immutable data_table sources fetch only visible rows while retaining global scrolling/navigation/selection/copy. SQL serializes its complete typed snapshot and byte index in one execution; all rows are available in the grid automatically, and Load All reuses that index. SQL tab direction/focus/viewport and out-of-preview Plot point/range reveal verified.681 tests/strict checks, four table-interior Metal comparisons and four browser+14 Office native static windows pass. Read-only SQL grid copy/sort/edit-cell details, table-to-Plot highlighting, continuous Browse/cross-page ranges, Plot exports/clipboard/print and full interface/native/platform acceptance remain; F02 stays in_progress.


## Plot saving checkpoint (2026-10-29)

[Review](../../docs/reviews/R-20261029-579bf820daab.md): Save… writes current viewport/selection/legend/styles to PNG/JPEG/BMP/PDF with original suffix fallback, staged replacement and main/attached database protection.684 tests/strict checks, four Soft/Metal comparisons/four native static windows and independent CoreGraphics PDF acceptance (pixel delta0) verified. PDF is lossless raster, with vector/print parity explicitly pending. Clipboard/context menus/stacked bars/format/hover, synchronous large render limits, narrow layout and full original/native/platform/product acceptance remain; F02 stays in_progress.


## Plot clipboard checkpoint (2026-10-29)

[Review](../../docs/reviews/R-20261029-43984094dce2.md): bounded straight RGBA clipboard APIs now reach all window hosts; current-view Plot Copy/Cmd+C retain focus and share the export renderer.686 tests/strict checks, exact native RGBA+alpha round trip, targeted native process Cmd+C with independent AppKit read, four Metal comparisons and four native theme captures verified. Ordinary native focus/pointer/picker/IME and other-host runtime acceptance remain absent. Decoder allocation precedes the read cap; synchronous render cost, narrow layout, context menus/print/vector output and full original/product parity remain; stable F02 stays in_progress.


## Responsive Plot checkpoint (2026-10-29)

[Review](../../docs/reviews/R-20261029-3242c297114c.md): constrained Plot controls now wrap, chart height accounts for extra rows and adaptive dock width preserves saved split preferences. Focus no longer scrolls away the chart.687 tests/strict checks, four Soft/Metal comparisons and four actual native theme/size captures verified. AX resizing failed; startup800×600 capture is separate from ordinary OS drag acceptance. Large fonts, original docking/toolbar design, menus/stacked bars/formats/hover/printing and complete product/native/platform parity remain; stable F02 stays in_progress.


## Plot context and format checkpoint (2026-10-29)

[Review](../../docs/reviews/R-20261029-333f5ae66ebb.md): shared controlled house-icon checks and measured context menus now support Plot right-click/Shift+F10 Copy/Save/Fit/legend/fixed numeric format. All three axes follow visible-range precision; full buffers, view, SQL identity and JSON/XML settings survive toggles.691 tests/strict checks, eight chart/four popup Metal comparisons, two native process keyboard/clipboard workflows and14 Office native static captures verified. Ordinary frontmost pointer/IME/picker/drag and other-host runtime acceptance remain absent. Grouped/stacked bars, hover/Print/vector output, Qt conversion details, original docking/design and full parity remain; stable F02 stays in_progress. Publication is local after the earlier approval timeout denial.


## Grouped and stacked Plot checkpoint (2026-10-29)

[Review](../../docs/reviews/R-20261029-67a732194708.md): grouped/stacked signed bars now share limits, rendering, interior row hits and endpoint markers across screen/save/Copy. Duplicate/near-key extrema, unsorted source order, independent axes, finite overflow,100k dense-key lookup, retained view/full buffers/no SQL replay and JSON/XML settings verified. Category ticks show all fitting visible labels.695 tests/strict checks, eight actual Metal comparisons, eight native theme/size/mode captures and two native process keyboard/clipboard workflows pass. Ordinary native input, dense native throughput/peak memory, Print/hover/vector output, exact Qt controls/rotation/spacing, independent docks and full parity remain; F02 stays in_progress. Publication remains local after approval timeout denial.

## 2026-10-29 independent dock layout

[Review](../../docs/reviews/R-20261029-b7120e3c67dd.md): the cell editor now stays independently visible above bottom-tabbed Log/Plot/Schema/Remote. Close/reopen retains drafts; editor/lower and Plot controls/chart split preferences persist through adaptive sizing and project JSON/XML extensions. Only laid-out per-page scrolls are captured, including hidden-page return; tab focus/header updates survive rebuilds. Compact grouped toolbars and scrolling main titles reclaim narrow space.697 tests/strict/build/host feature checks,4 Soft/Metal pairs (whole-frame max2),4 native windows, final process-keyboard navigation/close and14 Office static captures pass. Ordinary pointer/drag/picker/IME, other-host runtime, floating/rearranging/Qt dock-state, large fonts and complete original/product parity remain; F02 stays in_progress. Publication remains local after approval timeout denial.


## 2026-10-29 Plot coordinate hover

[Review](../../docs/reviews/R-20261029-60838586a5cb.md): opt-in shared Chart tooltips use the exact cached image viewport for named X/Y1/Y2 coordinates, preserve full caption help and reuse image/point buffers. Date/time/category/extreme ranges, fractional image sizing, navigation, bounded long text, hit pass-through and menu/window-leave suppression pass.701 tests/strict/build/host feature checks,4 Soft/Metal pairs (whole-frame max2),4 corrected prepared-hover native windows, native Right/Home date updates/exact pixel restoration and14 Office static captures are verified. Prepared hover/focus does not establish ordinary pointer acceptance. Exact Qt locale/DST, category rotation/control states, Print/vector output, dense native throughput, icon toolbars/docking, SQL/Browse/cell details, Remote and full original/native/platform parity remain; F02 stays in_progress. Publication remains local after approval timeout denial.


## 2026-10-29 compact icon toolbars

[Review](../../docs/reviews/R-20261029-716afa8e3e77.md): shared square icon buttons retain full live accessible help, theme/density, hits/focus/disabled behavior and reversible text sizing; validated DSL and four new house glyphs are available. Grouped main/SQL actions fit800px, keep independent View toggles and retain SQL drafts/current cell values.705 tests/strict/build/host feature checks,8 Soft/Metal pairs (whole-frame max2),8 native windows, targeted Enter/Undo/Redo and14 freshly built Office static captures verified. Frontmost loginwindow/desktop wallpaper prevents ordinary pointer acceptance. Original TextBesideIcon/per-toolbar modes/preferences, complete action order/overflow/movement, large fonts, SQL/Browse/cell details, Plot/Print, Remote and full product/native/platform parity remain; F02 stays in_progress. Publication remains local after approval timeout denial.


## 2026-10-29 complete read-only SQL cells

[Review](../../docs/reviews/R-20261029-532eb13e3540.md): SQL result endpoints now load complete typed snapshots into Edit Cell without replay, including row3000, exact i64/NULL/BLOB/invalid UTF-8 TEXT bytes. Explicit source identity guards backend writes and draft replacement, caches unchanged/failed reads and retains view modes; empty SQL selection clears, Browse return and query/dock lifecycles refresh correctly. Copy and Text/Hex/JSON/image preview remain available.707 tests/strict/build/host feature checks,4 Soft/Metal pairs (whole-frame max1),4 native windows and process Home/End region changes/exact cell restoration verified. Native typing focus/ordinary input remain unaccepted. Other inactive-page read-only behavior, full cell sessions, SQL sort/copy, continuous Browse, original toolbar modes/docking, Plot/Print/Remote and full product/native/platform parity remain; F02 stays in_progress. Publication remains local after approval timeout denial.


## Cell page policy checkpoint (2026-10-29)

[Review](../../docs/reviews/R-20261029-cebc44a051a4.md): Structure/Pragmas and unwritable Browse selections now share source/page guards, disabled editing/actions, copy and read-only preview modes. JSON/Hex switches cannot create false drafts; Pragmas rebuilds cannot write stale widget text back; true Browse drafts survive and block navigation. Existing transaction recovery diagnostics retained.708 workspace tests/strict/build/host feature checks,8 Soft/Metal pairs (whole-frame max1) and8 owned native windows verified. Original primary-key/custom identities, full editor/toolbar/dock/Plot/Print/Remote and complete native/product/platform parity remain; F02 stays in_progress. Publication remains local after approval timeout denial.


## Toolbar styles and cell files checkpoint (2026-10-29)

[Review](../../docs/reviews/R-20261029-fa43090d0524.md): five independent original toolbar choices/defaults now reach Main/Structure/Browse/SQL/Edit Cell through a shared accessible ButtonIconLayout API/DSL. Preference scrolling, restored visible dropdowns and Cancel verified. Cell import creates a typed draft; staged export preserves exact raw bytes and database files; NULL/JSON actions exposed.712 tests/strict/build/host feature checks,10 Soft/Metal pairs (full-frame max2),10 browser+14 fresh Office native static windows verified. Under-icon layouts still clip the upper editor at800×600 until scrolled; full editor sessions/modes/actions, original order/overflow/docking, Print/Remote and full native/product/platform parity remain. Stable F02 stays in_progress. Publication remains local after approval timeout denial.


## Cell editor sessions checkpoint (2026-10-29)

[Review](../../docs/reviews/R-20261029-e7256c7abafb.md): existing text sessions now retain focus, Unicode directional selection/copy, IME candidates, viewport and Undo/Redo through same-cell rebuilds and panel reopen. Loaded revisions isolate identical-text cells and prevent stale input snapshots after file/NULL/discard/mode replacement. Range-anchor-only updates no longer reload the selected endpoint and erase Redo.713 tests/strict/build/host feature checks,4 Soft/Metal pairs (full-frame max1),4 prepared native windows and2 targeted native Undo/Redo workflows (whole window exact restoration) pass. Ordinary frontmost input remains unaccepted. Full modes/binary/XML/RTL/evaluation/word-wrap/indent, cross-mode histories, narrow/large-font polish, other product/native/platform gaps remain; F02 stays in_progress. Publication remains local after approval timeout denial.

## XML and evaluated cells checkpoint (2026-10-29)

[Review](../../docs/reviews/R-20261029-d531b492ca73.md): six-mode house selector, checked XML/raw-token indentation and baselines, invalid draft switch/export, NULL input and explicit typed scalar evaluation verified. Apply restores the visible record by identity and shortcut focus.719 tests/strict checks,11 Metal pairs,12 native static windows and2 native exact-result apply/copy workflows pass. The earlier cleared editor observed in native review was fixed and final evidence rerun. Full XML/JSON/RTL/binary/word-wrap/auto-switch/error/action/history workflows, narrow/large-font polish and complete original/product/native/platform parity remain; F02 stays in_progress. Publication remains local after approval timeout denial.


### Cell dock layout update (2026-10-29)

F02 remains **in_progress**. [Review](../../docs/reviews/R-20261029-fe5fcce7aa05.md) verifies fixed cell actions/metadata, combined scrollable mode/tools, actual cell fonts and adaptive readable editor/Plot minima with retained preferred splits/drafts/carets. The60-case matrix exposed and fixed shared exact-fit floating ratio panic.721 tests/strict checks,8 final Metal pairs,14 prepared native browser windows,2 exact-result native apply/copy flows and14 fresh Office captures pass. Earlier collapsed graph frames were rejected and final evidence rerun. Shell scrolling is necessary in constrained large-font windows; other fixed Browse/Plot metrics, full editor/toolbar/docking/product features and ordinary native/other-host acceptance remain. No completion or publication claim.


### Cell wrapping and horizontal viewport update (2026-10-29)

F02 remains **in_progress**. [Review](../../docs/reviews/R-20261029-39cf999bd907.md) verifies the checked persisted Wrap lines tool, retained draft/selection/history/focus, shared horizontal paint/hit/IME/hint/scrollbar/session geometry and viewport-bounded masks.725 tests/strict checks,8 whole-frame Metal pairs,12 prepared native browser windows,4 native exact-restoration keyboard flows and14 fresh Office windows pass. Enabled wrapping still uses character boundaries; original word/style wrapping, full shaping, auto-switch/format/binary/action/history workflows and complete original/product/native/platform parity remain. No completion or publication claim.

## Automatic cell modes checkpoint (2026-10-29)

F02 remains **in_progress**. [Review](../../docs/reviews/R-20261029-bbd7ade72d85.md) verifies persisted Auto mode, safe classification/manual choices, raw storage/image draft Apply and retained focused toggles.728 tests/strict checks,14 whole-frame Metal pairs,20 prepared native windows and4 exact-restoration keyboard workflows pass. Image layout, original indent/compact/format-error/actions, RTL/codecs/BOM, dedicated binary editing and full product/native/platform parity remain. No completion or publication claim.

## Dedicated image preview checkpoint (2026-10-29)

F02 remains **in_progress**. [Review](../../docs/reviews/R-20261029-3ec016c3c205.md) verifies image-only adaptive previews, fixed footer, retained zoom/pan/focus and unavailable-image guards.729 tests/strict checks,12 whole-frame Metal pairs,10 native windows and4 exact Fit-restoration keyboard workflows pass. Large fonts retain outer shell scrolling; original format/actions, remaining codecs and full visual/product/native/platform parity remain. No completion or publication claim.

### Cell Indent / Compact update (2026-10-29)

F02 remains **in_progress**. [Review](../../docs/reviews/R-20261029-6153c6ec8f5e.md) verifies original-default-off Indent / Compact, lossless JSON tokens, conservative XML line compaction, typed explicit Apply and clean view/history/focus/readonly guards.733 tests/strict checks,12 whole-frame Metal pairs,12 native windows and8 exact-return keyboard workflows pass after fixing the narrow clean-view caret. Original error confirmation/markers, full history/settings/actions and complete visual/product/native/platform parity remain. No completion or publication claim.

### Malformed cell confirmation update (2026-10-29)

F02 remains **in_progress**. [Review](../../docs/reviews/R-20261029-987bcc9e25ac.md) verifies Unicode/EOF format marks, source-bound explicit typed Apply Anyway, Cancel/history/focus/readonly guards and deferred Save All paths/writes/document actions.737 tests/strict checks,20 whole-frame Metal pairs,12 native windows and4 exact-interior cancel/reopen/apply keyboard flows pass. Original raw-encoding validation, no-op Apply, full modes/history/actions and complete visual/product/native/platform parity remain. No completion or publication claim.


### Typed source and unchanged Apply update (2026-10-29)

F02 remains **in_progress**. [Review](../../docs/reviews/R-20261029-fd6d0d14751d.md) verifies raw UTF-8 diagnostics/explicit confirmation/repair, actual typed no-op comparison, ignored-trigger rollback with retained prior pending changes/drafts, and quoted/ASCII-alias target guards/restoration.741 tests/strict checks,24 whole-frame Metal pairs,12 native windows and8 raw/no-op keyboard workflows pass. BOM/codecs, nonfinite REAL source handling, full modes/history/actions and complete visual/product/native/platform parity remain. No completion or publication claim.


### BOM and stored REAL source update (2026-10-29)

F02 remains **in_progress**. [Review](../../docs/reviews/R-20261029-f543ef56518e.md) verifies UTF8 BOM text/source round-trip, typed edits/export/Hex/type transitions, leading JSON BOM Auto/formatting/source-relative error marks, readonly/evaluation and existing REAL infinity handling without new nonfinite input.746 tests/strict checks,20 whole-frame Metal pairs,16 native windows and8 BOM/no-op keyboard workflows pass. JSON source BOM preservation deliberately differs from original serializer normalization. Full codecs/RTL/binary/history/word/style wrap/actions and complete visual/product/native/platform parity remain. No completion or publication claim.

## Shared Unicode word wrapping checkpoint (2026-10-29)

[Review](../../docs/reviews/R-20261029-8569b7c4f602.md): fitting words move as a unit; long words fall back to grapheme clusters and CJK punctuation follows Unicode15 opportunities. Painting, pointer/caret/selection, Home/End and IME/hint positions share geometry. Checked Word wrap, horizontal scrolling and browser drafts/history/readonly guards are retained.749 tests/strict checks,16 whole-frame Metal pairs,12 native windows,4 exact-return keyboard flows and14 rebuilt Office captures pass. Scalar caret editing, complex shaping/bidi/RTL/missing emoji, discretionary hyphens/non-LF separators/style wrap and complete binary/codecs/actions/docking/product/native/platform parity remain. F02 stays in_progress; earlier publication approval timed out/denied.

## Live cell draft information checkpoint (2026-10-29)

[Review](../../docs/reviews/R-20261029-d11fa4e48f10.md): cell captions now follow visible UTF16 character counts, strict Hex bytes/invalid data, NULL and image dimensions, retaining record/result context. Typing and Undo/Redo update labels/accessible names/tooltip help without replacing the editor or evaluating SQL.753 tests/strict checks,8 whole-frame Metal pairs,12 native windows,4 exact native Undo/Redo workflows and14 rebuilt Office captures pass. Multi-character synthetic key events were rejected as evidence; final flows use actual single-character/delete routes. Ordinary frontmost input remains unavailable. Counts are not grapheme counts; cross-type history, dedicated binary editing and complete codecs/RTL/actions/docking/product/native/platform parity remain. Stable F02 stays in_progress and publication remains local after earlier approval timeout/denial.


### Shared binary cell editor verification (2026-10-29)

Status remains **in_progress**. [Review](../../docs/reviews/R-20261029-5232512e0e9d.md) verifies dedicated16-byte address/Hex/ASCII editing, nibble/ASCII input, insert/overwrite, byte selection and bounded delta history, two-axis visible-row rendering, readonly/source-scoped sessions, strict paste/UTF8 and explicit typed Apply/readable copy/export.760 tests/strict checks,12 whole-frame Metal pairs,12 native windows,4 exact Hex/ASCII Undo/Redo flows and14 fresh Office captures pass. Buffers remain in memory; narrow large-font docking, font preferences/context/print/external actions, RTL/codecs/cross-mode history and full original visual/product/native/platform parity remain. No completion/publication claim; earlier exact push approval timed out/denied.


### Cell file and external editor verification (2026-10-29)

Status remains **in_progress**. [Review](../../docs/reviews/R-20261029-44c453d1409d.md) verifies file/link import choices, database-relative link targets, nonblocking native launch requests, source/draft-bound external-copy Apply/Cancel and readonly Close, bounded regular-file reload and retained error/history/format flows.765 tests/strict checks,16 Metal pairs,10 native windows and8 prepared-copy keyboard workflows pass; Cmd+S leaves all modal frames unchanged. Actual default-app success/editor-save, full Qt URL normalization and native picker/input remain unaccepted. Async feedback F01 is open for the next cycle; Print/context, full codecs/RTL/history, primary/custom identities, docking and full original product/visual/platform parity remain. No completion/publication claim; earlier exact push approval timed out/denied.
