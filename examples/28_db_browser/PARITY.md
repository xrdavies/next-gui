# DB Browser for SQLite (DB4S) ↔ e28 parity checklist

**Goal:** 100% feature + UI parity with [DB Browser for SQLite](https://sqlitebrowser.org/) (sqlitebrowser / DB4S), with next-gui Theme so the UI can meet or exceed DB4S visually. No intentional omissions — items marked `todo` are scheduled, not waived.

Reference: upstream `src/MainWindow.ui`, EditTableDialog, EditIndexDialog, ImportCsvDialog, ExportSqlDialog, CipherDialog, PreferencesDialog, FindReplaceDialog, PlotDock, EditDialog, TableBrowser.

Status legend: `done` | `partial` | `todo`

Counts (Phase 2 close): **done 69 · partial 4 · todo 20** (re-tally when updating).

---

## Shell chrome

| Surface | Status | Notes |
|---------|--------|-------|
| Menubar File / Edit / View / Tools / Help | done | menus wired; remaining stubs → status |
| Main toolbar: New, Open, Write Changes, Revert | done | + Theme (exceeds DB4S) |
| Main tabs (exactly 4): Structure / Browse / Pragmas / Execute SQL | done | Charts→Plot dock; SQL Log→Log dock |
| Right docks: Edit Cell / DB Schema / SQL Log / Plot | done | View menu toggles; Splitter column |
| Status bar: path, encoding, read-only, encryption, busy | partial | path + dirty/ro/busy + UTF-8 / not encrypted labels |
| Soft↔GPU toggle | done | Space; exceeds DB4S |
| Theme light/dark | done | exceeds DB4S |

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
| Export → Database to SQL file… | done | `rfd` |
| Export → Table(s) as CSV… | done | current browse table |
| Export → Table(s) to JSON… | done | browse result → JSON array |
| Write Changes | done | COMMIT + re-BEGIN |
| Revert Changes | done | ROLLBACK + re-BEGIN |
| Compact Database (VACUUM) | done | |
| Exit | done | |
| Open / Save / Close Project | todo | `.sqbpro` |
| Save All | todo | |

## Edit menu

| Action | Status | Notes |
|--------|--------|-------|
| Create Table… | done | EditTable overlay wizard (cols PK/NN/Unique/Default, reorder) |
| Create Index… | done | EditIndex overlay (name/table/cols/unique) |
| Modify Object… | done | prefills EditTable / EditIndex from selection |
| Delete Object… | done | confirm + DROP |
| Preferences… | todo | PreferencesDialog |

## View menu

| Action | Status | Notes |
|--------|--------|-------|
| Toggle Edit Database Cell dock | done | |
| Toggle DB Schema dock | done | |
| Toggle SQL Log dock | done | |
| Toggle Plot dock | done | |
| Toggle DB / Extra / Project toolbars | todo | single main toolbar for now |

## Tools menu

| Action | Status | Notes |
|--------|--------|-------|
| Set Encryption… (SQLCipher) | todo | CipherDialog |
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
| Frozen columns | todo | |
| Conditional formatting | todo | |
| FK click-navigation | todo | |
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
| Toolbar: Open Tab, Open/Save file, Execute, Stop, Find | done | Execute Line still stub |
| Multi SQL tabs | done | SQL 1 / New Tab + tab strip |
| Find / Replace | done | overlay Find/Replace |
| Async query + cancel | partial | worker thread + `InterruptHandle` + progress_handler / AtomicBool Stop |
| Results → CSV / JSON / Save as view | partial | CSV + JSON export; Save as view todo |
| Toggle comment | todo | |

## Docks

| Dock | Status | Notes |
|------|--------|-------|
| Edit Database Cell | done | column + value + Apply; follows browse selection |
| DB Schema | done | compact schema list |
| SQL Log | done | |
| Plot | done | Trend from numeric column |
| Remote / other docks | todo | |

## Dialogs

| Dialog | Status | Notes |
|--------|--------|-------|
| EditTableDialog (full wizard) | done | OverlayKind::EditTable (Modal cannot host custom children) |
| EditIndexDialog | done | OverlayKind::EditIndex |
| ImportCsvDialog (delimiter, quote, encoding, …) | done | encoding fixed UTF-8 for now |
| ExportSqlDialog (options) | todo | dump path only |
| CipherDialog | todo | |
| PreferencesDialog | todo | |
| FindReplaceDialog | done | OverlayKind::FindReplace |
| EditDialog (blob/image/hex) | todo | |

## Data / engine

| Feature | Status | Notes |
|---------|--------|-------|
| rusqlite bundled SQLite | done | + `hooks` for progress_handler |
| Deferred Write Changes / Revert (txn) | done | BEGIN on open; dirty flag |
| SQLCipher | todo | |
| Extensions | todo | |
| In-memory DB | done | shared-cache URI |
| ATTACH DATABASE | done | |

---

## Phase map

- **Phase 1:** chrome parity (4 tabs + docks + menubar + Write/Revert + rfd + header sort).
- **Phase 2 (this):** EditTable/Index wizards, CSV import options + clipboard, filter-per-column, multi-browse tabs, Find/Replace, multi SQL tabs + Stop interrupt, pragma form, JSON export, Recent, Attach.
- **Phase 3:** SQLCipher, projects (`.sqbpro`), frozen columns, conditional formatting, FK nav, blob/image editor, ExportSql options, Preferences, Execute Line / toggle comment, Save as view.

## Framework gaps blocking 100%

- `Modal` / `Drawer` accept title+body only — no custom child widgets → wizards use full-window `OverlayKind` panels (functional parity; not pixel-identical dialog chrome).
- No native multi-document toolbar set (DB / Extra / Project) — single toolbar.
- Table widget has no frozen-column / conditional-format APIs yet.
- Blob/image/hex cell editor needs richer media widgets.
