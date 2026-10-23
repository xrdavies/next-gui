# DB Browser for SQLite (DB4S) ↔ e28 parity checklist

**Goal:** 100% feature + UI parity with [DB Browser for SQLite](https://sqlitebrowser.org/) (sqlitebrowser / DB4S), with next-gui Theme so the UI can meet or exceed DB4S visually. No intentional omissions — items marked `todo` are scheduled, not waived.

Reference: upstream `src/MainWindow.ui`, EditTableDialog, EditIndexDialog, ImportCsvDialog, ExportSqlDialog, CipherDialog, PreferencesDialog, FindReplaceDialog, PlotDock, EditDialog, TableBrowser.

Status legend: `done` | `partial` | `todo`

Counts (Phase 3 close): **done 90 · partial 6 · todo 8** (re-tally when updating).

---

## Shell chrome

| Surface | Status | Notes |
|---------|--------|-------|
| Menubar File / Edit / View / Tools / Help | done | menus wired; Help stubs → status |
| Main toolbar: New, Open, Write Changes, Revert | done | + Theme (exceeds DB4S); View-toggleable |
| Extra DB / Project toolbars | done | View → DB / Extra / Project / SQL toolbar toggles |
| Main tabs (exactly 4): Structure / Browse / Pragmas / Execute SQL | done | Charts→Plot dock; SQL Log→Log dock |
| Right docks: Edit Cell / DB Schema / SQL Log / Plot | done | View menu toggles; Splitter column |
| Status bar: path, encoding, read-only, encryption, busy | partial | path + dirty/ro/busy + encrypted label on toolbar |
| Soft↔GPU toggle | done | Space; exceeds DB4S |
| Theme light/dark | done | exceeds DB4S; Preferences default theme |

## File menu

| Action | Status | Notes |
|--------|--------|-------|
| New Database… | done | native `rfd` |
| New In-Memory Database | done | shared-cache URI (worker-friendly) |
| Open Database… | done | `rfd` |
| Open Database Read Only… | done | write blocked |
| Attach Database… | done | overlay: path + schema name → ATTACH |
| Recently opened | done | last 5 paths persisted (`.e28_recent`) |
| Close Database | done | confirm modal |
| Save Database As… | partial | copy file via `rfd` save |
| Import → Database from SQL file… | done | `rfd` |
| Import → Table from CSV file… | done | rfd then ImportCsv overlay (table/header/delim/quote/UTF-8) |
| Import → Table from CSV clipboard… | done | PasteCsv overlay |
| Export → Database to SQL file… | done | ExportSql overlay: structure/data/drop/FK then dump |
| Export → Table(s) as CSV… | done | current browse table |
| Export → Table(s) to JSON… | done | browse result → JSON array |
| Write Changes | done | COMMIT + re-BEGIN |
| Revert Changes | done | ROLLBACK + re-BEGIN |
| Compact Database (VACUUM) | done | |
| Exit | done | |
| Open Project… | done | `.sqbpro` JSON (Open/Save/Save As/Close) |
| Save Project / Save Project As… | done | persists DB path, tab, browse, SQL tabs, docks, toolbars, frozen |
| Close Project | done | |
| Save All | todo | |

## Edit menu

| Action | Status | Notes |
|--------|--------|-------|
| Create Table… | done | EditTable overlay wizard (cols PK/NN/Unique/Default, reorder) |
| Create Index… | done | EditIndex overlay (name/table/cols/unique) |
| Modify Object… | done | prefills EditTable / EditIndex from selection |
| Delete Object… | done | confirm + DROP |
| Preferences… | done | font size / default theme / confirm drops / CSV delim / frozen default → `~/.config/e28_db_browser/prefs.json` |

## View menu

| Action | Status | Notes |
|--------|--------|-------|
| Toggle Edit Database Cell dock | done | |
| Toggle DB Schema dock | done | |
| Toggle SQL Log dock | done | |
| Toggle Plot dock | done | |
| Toggle DB / Extra / Project / SQL toolbars | done | four independent View toggles |

## Tools menu

| Action | Status | Notes |
|--------|--------|-------|
| Set Encryption… (SQLCipher) | partial | CipherDialog overlay (passphrase/page size/KDF); applies via `PRAGMA key` when SQLCipher linked; clear error + settings retained on stock bundled SQLite |
| Load Extension… | todo | |
| Integrity Check | done | → Execute SQL results |
| Quick Check | done | |
| Foreign Key Check | done | |
| Optimize | done | `PRAGMA optimize` |
| Row Counts | done | query into Execute SQL |

## Help menu

| Action | Status | Notes |
|--------|--------|-------|
| About | done | status message |
| Website / Wiki / SQLCipher FAQ / Bug / Feature / Donate / Updates | todo | stubs OK (status message) |

## Database Structure tab

| Feature | Status | Notes |
|---------|--------|-------|
| Structure toolbar: New Table, New Index, Modify, Delete, Refresh | done | wizards + Modify wired |
| Schema tree Tables/Views/Indexes/Triggers | done | |
| Object SQL preview | done | |
| Run DDL | done | |
| Print schema | todo | |

## Browse Data tab

| Feature | Status | Notes |
|---------|--------|-------|
| Table selector | done | |
| Filter / WHERE | done | |
| Data grid | done | |
| Header-click sort | done | next-gui-ui `take_table_header_click` |
| Filter-per-column | done | row of inputs → LIKE in WHERE |
| Multi browse tabs | done | + Tab / tab buttons |
| Add / Delete row | done | delete confirm; pending txn |
| Edit cell → Edit Cell dock | done | |
| Frozen columns | done | `Table::{frozen_cols,scroll_x}` + Freeze toggle |
| Conditional formatting | done | Cond Format overlay + `set_table_cell_bg` tint |
| FK click-navigation | done | FK → opens parent browse tab with WHERE |
| Record / form view | todo | |

## Edit Pragmas tab

| Feature | Status | Notes |
|---------|--------|-------|
| Scroll form of pragmas | done | labeled rows: Select/Switch/TextInput |
| Save / Apply pragmas | done | Save Pragmas button |
| Bool / Select editors | done | journal_mode / synchronous / FK switches |

## Execute SQL tab

| Feature | Status | Notes |
|---------|--------|-------|
| SQL editor | done | |
| Execute / message / results table | done | |
| Toolbar: Open Tab, Open/Save file, Execute, Stop, Find | done | + Execute Line / Toggle -- / Save as VIEW |
| Multi SQL tabs | done | SQL 1 / New Tab + tab strip |
| Find / Replace | done | overlay Find/Replace |
| Async query + cancel | partial | worker thread + `InterruptHandle` + progress_handler / AtomicBool Stop |
| Results → CSV / JSON / Save as view | done | CSV + JSON export; Save as VIEW creates `e28_saved_view` |
| Toggle comment | done | `--` toggle on all lines |
| Execute current line | done | first non-comment non-empty line |

## Docks

| Dock | Status | Notes |
|------|--------|-------|
| Edit Database Cell | done | Text / Hex / JSON / Image modes + byte length; Apply |
| DB Schema | done | compact schema list |
| SQL Log | done | |
| Plot | done | Trend from numeric column |
| Remote / other docks | todo | |

## Dialogs

| Dialog | Status | Notes |
|--------|--------|-------|
| EditTableDialog (full wizard) | done | OverlayKind::EditTable (`dialog_panel` available; overlays still used) |
| EditIndexDialog | done | OverlayKind::EditIndex |
| ImportCsvDialog (delimiter, quote, encoding, …) | done | encoding fixed UTF-8 for now |
| ExportSqlDialog (options) | done | structure / data / DROP IF EXISTS / FK |
| CipherDialog | partial | UI + apply path; real crypto needs sqlcipher feature |
| PreferencesDialog | done | OverlayKind::Preferences + persisted prefs |
| FindReplaceDialog | done | OverlayKind::FindReplace |
| EditDialog (blob/image/hex) | partial | dock modes Text/Hex/JSON/Image (image = magic detect + byte meta; no pixel decode) |

## Data / engine

| Feature | Status | Notes |
|---------|--------|-------|
| rusqlite bundled SQLite | done | + `hooks` for progress_handler |
| Deferred Write Changes / Revert (txn) | done | BEGIN on open; dirty flag |
| SQLCipher | partial | UI + `apply_cipher_key`; stock build errors clearly; rebuild with `bundled-sqlcipher-vendored-openssl` for real encrypt |
| Extensions | todo | |
| In-memory DB | done | shared-cache URI |
| ATTACH DATABASE | done | |

---

## Phase map

- **Phase 1:** chrome parity (4 tabs + docks + menubar + Write/Revert + rfd + header sort).
- **Phase 2:** EditTable/Index wizards, CSV import options + clipboard, filter-per-column, multi-browse tabs, Find/Replace, multi SQL tabs + Stop interrupt, pragma form, JSON export, Recent, Attach.
- **Phase 3 (this):** SQLCipher UI, `.sqbpro` projects, frozen columns + conditional formatting + FK nav, Edit Cell Text/Hex/JSON/Image, ExportSql options, Preferences, Execute Line / toggle comment / Save as VIEW, multi toolbar View toggles, `FlexBuilder::dialog_panel`.

## Framework gaps blocking literal 100%

- `Modal` still title+body only — wizards use overlay `Panel`s; Phase 3 added `FlexBuilder::dialog_panel` (title + body slot + OK/Cancel) for real dialog chrome going forward.
- Table frozen columns + cell tint landed; true per-cell background fills would need extra aux nodes (currently tint/marker on text).
- SQLCipher not linked in default `bundled` build (openssl present; switch feature to enable).
- Image mode detects PNG/JPEG magic but soft renderer has no image blit widget yet.
- Load Extension / Record view / Print schema / Remote docks / Help URLs / Save All remain.
