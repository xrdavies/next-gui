# e28_db_browser — DB Browser for SQLite (next-gui)

Example that mirrors **DB Browser for SQLite (DB4S)** workflows using `rusqlite` (bundled) and next-gui Theme tokens.

## Run

```bash
cargo run -p e28_db_browser
NEXT_GUI_DB_BROWSER_QA=1 cargo run -p e28_db_browser   # Soft PPM → /tmp/next-gui-db-browser.ppm
```

On first launch the example opens/creates `examples/28_db_browser/sample.db` with a small demo schema (customers / products / orders).

Controls: **Space** Soft↔GPU (when unfocused), toolbar **Theme** light/dark, Esc quits (or closes modal).

## Features

| Area | Status |
|------|--------|
| New / Open / Close / Demo DB | Done (path TextInput; no OS file dialog) |
| Vacuum (compact) | Done |
| Structure tree (tables/views/indexes/triggers) | Done |
| Create table / Run DDL / Drop object | Done (confirm Modal) |
| Browse Data grid + WHERE filter | Done |
| Add / delete row / edit cell | Done (rowid-based; views read-only) |
| Execute SQL + results + error Alert | Done |
| SQL log | Done |
| Edit Pragmas | Done (grid + value field) |
| CSV import/export | Done (path TextInput) |
| SQL dump import/export | Done |
| Charts (Trend + Gauge from numeric column) | Done |
| Theme light/dark + Soft/GPU | Done |

## Limitations

- No SQLCipher / encryption UI.
- No native OS file picker (`rfd`); enter paths in the toolbar / Import-Export field.
- Table header click does **not** sort (next-gui Table header is a stub). Put a column name in **Edit cell → column** and press **Apply** to ORDER BY (toggle ASC/DESC on repeat).
- No frozen columns, conditional formatting, or FK click-navigation.
- Long-running query cancel is not implemented (SQLite calls are synchronous on the UI thread).
- PropertyGrid text cells are not inline-editable; use the pragma **Value** field or Select/Bool toggles.
