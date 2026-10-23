# DB Browser for SQLite (DB4S) ↔ e28 parity checklist

**Goal:** 100% feature + UI parity with [DB Browser for SQLite](https://sqlitebrowser.org/) (sqlitebrowser / DB4S), with next-gui Theme so the UI can meet or exceed DB4S visually. No intentional omissions — items marked `todo` are scheduled, not waived.

Reference: upstream `src/MainWindow.ui`, EditTableDialog, EditIndexDialog, ImportCsvDialog, ExportSqlDialog, CipherDialog, PreferencesDialog, FindReplaceDialog, PlotDock, EditDialog, TableBrowser.

Status legend: `done` | `partial` | `todo`

Counts (Phase 1 close): **done 50 · partial 9 · todo 32** (re-tally when updating).

---

## Shell chrome

| Surface | Status | Notes |
|---------|--------|-------|
| Menubar File / Edit / View / Tools / Help | done | Phase 1: menus wired; some items stub → status / later dialog |
| Main toolbar: New, Open, Write Changes, Revert | done | + Theme (exceeds DB4S) |
| Main tabs (exactly 4): Structure / Browse / Pragmas / Execute SQL | done | Charts→Plot dock; SQL Log→Log dock |
| Right docks: Edit Cell / DB Schema / SQL Log / Plot | done | View menu toggles; Splitter column |
| Status bar: path, encoding, read-only, encryption, busy | partial | path + dirty/ro + msg; encoding/encryption/busy stubs |
| Soft↔GPU toggle | done | Space; exceeds DB4S |
| Theme light/dark | done | exceeds DB4S |

## File menu

| Action | Status | Notes |
|--------|--------|-------|
| New Database… | done | native `rfd` + fallback path |
| New In-Memory Database | done | |
| Open Database… | done | `rfd` |
| Open Database Read Only… | partial | opens read-only flag; write blocked |
| Attach Database… | todo | |
| Recently opened | todo | |
| Close Database | done | confirm modal |
| Save Database As… | partial | copy file via `rfd` save |
| Import → Database from SQL file… | done | `rfd` |
| Import → Table from CSV file… | done | `rfd`; wizard options later |
| Import → Table from CSV clipboard… | todo | |
| Export → Database to SQL file… | done | `rfd` |
| Export → Table(s) as CSV… | done | current browse table |
| Export → Table(s) to JSON… | todo | |
| Write Changes | done | COMMIT + re-BEGIN |
| Revert Changes | done | ROLLBACK + re-BEGIN |
| Compact Database (VACUUM) | done | Tools + File |
| Exit | done | |
| Open / Save / Close Project | todo | `.sqbpro` |
| Save All | todo | |

## Edit menu

| Action | Status | Notes |
|--------|--------|-------|
| Create Table… | partial | quick name+cols; full EditTableDialog wizard todo |
| Create Index… | partial | simple CREATE INDEX DDL path |
| Modify Object… | todo | EditTable / EditIndex dialogs |
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
| About | done | status / modal text |
| Website / Wiki / SQLCipher FAQ / Bug / Feature / Donate / Updates | todo | stubs OK (status message) |

## Database Structure tab

| Feature | Status | Notes |
|---------|--------|-------|
| Structure toolbar: New Table, New Index, Modify, Delete, Refresh | done | Modify stub |
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
| Filter-per-column | todo | |
| Multi browse tabs | todo | |
| Add / Delete row | done | delete confirm; pending txn |
| Edit cell → Edit Cell dock | done | |
| Frozen columns | todo | |
| Conditional formatting | todo | |
| FK click-navigation | todo | |
| Record / form view | todo | |

## Edit Pragmas tab

| Feature | Status | Notes |
|---------|--------|-------|
| Scroll form of pragmas | partial | PropertyGrid + Value/Apply; closer form layout Phase 2 |
| Save / Apply pragmas | done | |
| Bool / Select editors | done | |

## Execute SQL tab

| Feature | Status | Notes |
|---------|--------|-------|
| SQL editor | done | |
| Execute / message / results table | done | |
| Toolbar: Open Tab, Open/Save file, Execute, Execute Line, Stop, Find/Replace, Results export | partial | Execute + Open/Save SQL file + New Tab stub; Stop/Find later |
| Multi SQL tabs | partial | nested Tabs + New Tab adds editor slot (single active body Phase 1) |
| Async query + cancel | todo | |
| Results → CSV / JSON / Save as view | partial | CSV via export path |
| Toggle comment | todo | |

## Docks

| Dock | Status | Notes |
|------|--------|-------|
| Edit Database Cell | done | column + value + Apply; follows browse selection |
| DB Schema | done | compact schema list / tree mirror |
| SQL Log | done | |
| Plot | done | Trend from numeric column |
| Remote / other docks | todo | |

## Dialogs (Phase 2+)

| Dialog | Status |
|--------|--------|
| EditTableDialog (full wizard) | todo |
| EditIndexDialog | todo |
| ImportCsvDialog (delimiter, quote, encoding, …) | todo |
| ExportSqlDialog (options) | todo |
| CipherDialog | todo |
| PreferencesDialog | todo |
| FindReplaceDialog | todo |
| EditDialog (blob/image/hex) | todo |

## Data / engine

| Feature | Status | Notes |
|---------|--------|-------|
| rusqlite bundled SQLite | done | |
| Deferred Write Changes / Revert (txn) | done | BEGIN on open; dirty flag |
| SQLCipher | todo | |
| Extensions | todo | |
| In-memory DB | done | |

---

## Phase map

- **Phase 1 (this):** chrome parity (4 tabs + docks + menubar + Write/Revert + rfd + header sort) — see commits on `feat/sqlitebrowser-example`.
- **Phase 2:** EditTable/Index wizards, CSV import options, filter-per-column, multi-browse tabs, Find/Replace, pragma form polish.
- **Phase 3:** SQLCipher, async cancel, frozen columns, conditional formatting, FK nav, projects, JSON export, clipboard CSV.

