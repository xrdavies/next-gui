# DB Browser for SQLite (DB4S) ↔ e28 parity checklist

**Goal:** 100% feature + UI parity with [DB Browser for SQLite](https://sqlitebrowser.org/) (sqlitebrowser / DB4S), with next-gui Theme so the UI can meet or exceed DB4S visually. Items marked `partial` or `todo` are remaining compatibility work. This is an example, not a certified DB4S replacement.

Reference: upstream `src/MainWindow.ui`, EditTableDialog, EditIndexDialog, ImportCsvDialog, ExportSqlDialog, CipherDialog, PreferencesDialog, FindReplaceDialog, PlotDock, EditDialog, TableBrowser.

Status legend: `done` | `partial` | `todo`

The earlier “done 101 / partial 0” claim was not supported by acceptance tests. The review below distinguishes implemented local workflows from upstream compatibility. See the 2026-10-25 review in `docs/reviews/INDEX.md`.

---

## Shell chrome

| Surface | Status | Notes |
|---------|--------|-------|
| Menubar File / Edit / View / Tools / Help | done | Native URL opener on macOS/Linux/Windows; launch failures are reported |
| Main toolbar: New, Open, Write Changes, Revert | done | + Theme (exceeds DB4S); View-toggleable |
| Extra DB / Project toolbars | done | View → DB / Extra / Project / SQL toolbar toggles |
| Main tabs (exactly 4): Structure / Browse / Pragmas / Execute SQL | done | Charts→Plot dock; SQL Log→Log dock |
| Right docks: Edit Cell / DB Schema / SQL Log / Plot / Remote | done | View menu toggles; Splitter column |
| Status bar: path, encoding, read-only, encryption, busy | done | path · RO/RW · modified · busy · UTF-8 · SQLCipher · encrypted |
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
| Exit | done | |
| Open Project… | partial | Example JSON only; Unicode and malformed-input validation; upstream DB4S XML is rejected |
| Save Project / Save Project As… | partial | Example JSON; all browse tabs, filters/sort/pages plus SQL tabs/docks/toolbars; relative database paths resolved from project; no upstream XML compatibility |
| Close Project | done | |
| Save All | done | Prompt for project destination if missing; cancellation has no fallback write; commit DB and save JSON project |
| Print Schema… | done | `schema_report` → `rfd` save SQL/text |

## Edit menu

| Action | Status | Notes |
|--------|--------|-------|
| Create Table… | partial | Basic column PK/NN/Unique/Default; full DB4S constraint editor remains |
| Create Index… | done | EditIndex overlay (name/table/cols/unique) |
| Modify Object… | partial | Native ALTER preserves constraints/triggers/indexes; definition changes/reordering and expression/partial indexes require explicit SQL |
| Delete Object… | done | confirm + DROP |
| Preferences… | done | font size / default theme / confirm drops / CSV delim / frozen default → `~/.config/e28_db_browser/prefs.json` |

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
| Schema tree Tables/Views/Indexes/Triggers | done | |
| Object SQL preview | done | |
| Run DDL | done | |
| Print schema | done | File → Print Schema… |

## Browse Data tab

| Feature | Status | Notes |
|---------|--------|-------|
| Table selector | done | |
| Filter / WHERE | done | |
| Data grid | done | 500-row pages with Prev/Next; full exports independent of visible page |
| Header-click sort | done | next-gui-ui `take_table_header_click` |
| Filter-per-column | done | row of inputs → LIKE in WHERE |
| Multi browse tabs | done | + Tab / tab buttons |
| Add / Delete row | done | New Record form supports Value/NULL/Default and required fields; Delete uses unshadowed rowid |
| Edit cell → Edit Cell dock | done | Raw selected values; Hex writes BLOB, empty text stays text, unchanged NULL/numbers retain type; views/WITHOUT ROWID/shadowed rowid not edited |
| Frozen columns | done | `Table::{frozen_cols,scroll_x}` + Freeze toggle |
| Conditional formatting | done | Cond Format overlay + true per-cell **background** fill (`set_table_cell_bg`) |
| FK click-navigation | done | FK → opens parent browse tab with WHERE |
| Record / form view | done | Prev/Next and field buttons select Edit Cell dock; no non-saving editable fields |

## Edit Pragmas tab

| Feature | Status | Notes |
|---------|--------|-------|
| Scroll form of pragmas | done | labeled rows: Select/Switch/TextInput |
| Save / Apply pragmas | done | Save Pragmas button |
| Bool / Select editors | done | journal_mode / synchronous / FK switches |

## Execute SQL tab

| Feature | Status | Notes |
|---------|--------|-------|
| SQL editor | done | Unicode caret and IME composition/commit/cancel; native input-method acceptance still pending |
| Execute / message / results table | done | |
| Toolbar: Open Tab, Open/Save file, Execute, Stop, Find | done | + Execute Line / Toggle -- / Save as VIEW |
| Multi SQL tabs | done | SQL 1 / New Tab + tab strip |
| Find / Replace | partial | Text search/replace works; no visual match selection, current replacement is first match |
| Async query + cancel | done | Live connection moves to worker; Write/Revert/keys/ATTACH shared; completion polled on redraw; Stop interrupts |
| Results → CSV / JSON / Save as view | done | Full last-result snapshot export without rerunning SQL; CSV/JSON buttons below editor; Save as VIEW wired |
| Toggle comment | done | `--` toggle on all lines |
| Execute current line | done | Uses Unicode caret position; never replaces editor contents; one physical line |

## Docks

| Dock | Status | Notes |
|------|--------|-------|
| Edit Database Cell | done | Text / Hex / JSON / Image modes; Image soft-blits PNG/JPEG via `image` crate |
| DB Schema | done | compact schema list |
| SQL Log | done | |
| Plot | done | Trend from numeric column |
| Remote | partial | Local path list; cloud protocol/auth/sync not implemented |

## Dialogs

| Dialog | Status | Notes |
|--------|--------|-------|
| EditTableDialog (full wizard) | partial | Basic create plus safe native ALTER subset; arbitrary rebuild migrations excluded |
| EditIndexDialog | done | OverlayKind::EditIndex via `dialog_panel` |
| ImportCsvDialog (delimiter, quote, encoding, …) | partial | Strict UTF-8 only; multiline quoting supported |
| ExportSqlDialog (options) | done | structure / data / DROP IF EXISTS / FK |
| CipherDialog | done | Unlock existing encrypted file or export encrypted copy; no in-place rekey |
| PreferencesDialog | done | OverlayKind::Preferences + persisted prefs |
| FindReplaceDialog | done | OverlayKind::FindReplace |
| EditDialog (blob/image/hex) | done | Text/Hex/JSON/Image; PNG/JPEG thumbnails in Edit Cell dock |
| Load Extension | done | OverlayKind::LoadExtension |
| About | done | OverlayKind::About |

## Data / engine

| Feature | Status | Notes |
|---------|--------|-------|
| rusqlite bundled SQLite | done | + `hooks` + `load_extension` + `bundled-sqlcipher-vendored-openssl` |
| Deferred Write Changes / Revert (txn) | done | BEGIN on open; dirty flag |
| SQLCipher | done | linked (`SQLITE_HAS_CODEC`); CipherDialog apply + encrypt export; tests green |
| Extensions | done | Load Extension… |
| In-memory DB | done | Isolated connection, shared with worker by ownership transfer |
| ATTACH DATABASE | partial | Usable from SQL on the live connection; attached-schema tree/browse selector remains |

---

## Phase map

- **Phase 1:** chrome parity (4 tabs + docks + menubar + Write/Revert + rfd + header sort).
- **Phase 2:** EditTable/Index wizards, CSV import options + clipboard, filter-per-column, multi-browse tabs, Find/Replace, multi SQL tabs + Stop interrupt, pragma form, JSON export, Recent, Attach.
- **Phase 3:** SQLCipher UI, `.sqbpro` projects, frozen columns + conditional formatting + FK nav, Edit Cell Text/Hex/JSON/Image, ExportSql options, Preferences, Execute Line / toggle comment / Save as VIEW, multi toolbar View toggles, `FlexBuilder::dialog_panel`.
- **Phase 4:** Save All, Load Extension, Help URLs/About, Print Schema, Record/form view, Remote dock, SQLCipher linked build, true cell background fills, Edit Cell image blit (`image` 0.24), overlays on `dialog_panel`, status bar RO/encoding/encryption/busy polished.
- **Phase 4b (visual QA):** Table aux stride bug (only last row painted); Tabs recursive hide (Pragma bleed); Menu popup width; light clear/menubar; Remote dock default on; SQL log ellipsis; Plot clip/padding; Edit Cell col sync; Form toggle; Comment button label.

## Framework / external notes

- Overlays use `FlexBuilder::dialog_panel` (title + body + OK/Cancel) — dual Modal chrome reduced.
- Table `cell_bg` aux nodes provide true per-cell background fills.
- SQLCipher linked via `rusqlite` feature `bundled-sqlcipher-vendored-openssl` (verified `sqlcipher_linked` + `sqlcipher_key_roundtrip` tests).
- Image soft blit uses `image` 0.24 (0.25 needs rustc ≥1.88; host 1.86).
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
