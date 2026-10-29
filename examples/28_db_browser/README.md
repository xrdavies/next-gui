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

The structure tree groups objects under main, temp and attached databases, including empty attachments. Selection, definition previews and confirmed deletion preserve the database identity, including quoted names. Projects retain the selected object's identity with the DDL text. Browse tabs/selectors, record navigation, filters, conditional formats, typed editing/insert, row deletion, cut/paste, FK navigation and full filtered export preserve the database and table identity. Copy as SQL qualifies the target database. Plot selectors and numeric samples also preserve database identity. Create Table selects main/temp/attached databases; basic native table modification and index create/modify keep qualified targets and original identities for rollback. Table renaming follows browse tabs, saved options, conditional formats and Plot. Modify Table → Advanced can move an ordinary table between main, temp and attached databases while applying column edits. The move retains rows, rowids, generated values, self references, indexes, triggers and AUTOINCREMENT state inside the current transaction; browse/format/plot/project identities follow the destination. Conflicts, read-only destinations, newly invalid foreign keys and broken dependencies roll back the complete operation. Dependent views, chained views and their triggers now move with the table; their qualified query references and saved view settings follow the destination. Missing dependencies or destination view/trigger conflicts roll back the complete operation; some TEMP trigger/rename and name-shadowing combinations remain limited by the locked SQLite engine. Full constraint/migration and original interface parity remain unfinished. Definition previews outside main remain read-only; use qualified SQL in Execute SQL for those definitions.

Structure navigation retains focus, expanded groups and scroll position across page/UI rebuilds. Keyboard selection reveals the active object; expansion state follows database names when an attachment is detached. These view settings last for the current database session.

**Write Changes / Revert:** edits run inside a deferred transaction until Write (COMMIT) or Revert (ROLLBACK). Revert invalidates every Browse cache, clears sources removed by rollback and reloads the active surviving source. Valid table settings and independent SQL/definition drafts and result snapshots remain; invalid filter/plot queries report a refresh notice after the successful rollback.

**Phase 2:** EditTable/EditIndex overlay wizards · CSV import wizard (+ clipboard) · per-column browse filters + multi browse tabs · Execute SQL multi-tabs / Find-Replace / Stop (interrupt) · Pragma labeled form · JSON export · Recent files · Attach Database.

CSV imports validate and write on background workers with byte/record progress.
Cancel stops after the current read or record and rolls back the current file;
completed files and earlier pending edits remain available for Write Changes or
Revert. Other database actions wait until import or cancellation finishes.
The first20 records determine columns; shorter records retain missing cells as
NULL, while explicit empty cells follow the selected default/missing-value policy.
File and clipboard previews open in a centered modal with a fixed action row;
existing-table questions and cancellable progress nest above it. Escape/close
cancel only the current dialog; background database controls remain inactive.
Use local number conventions (off by default) applies system decimal/group/digit
symbols to inference and typed values before SQLite affinity. Each validated
snapshot keeps its numeric conventions until insertion finishes.

The File CSV/JSON export commands open a table/view chooser across schemas.
CSV supports headers, custom separator/quote and line endings; JSON supports
pretty output and preserves integers/NULL, with BLOBs encoded as base64. Multiple
objects export to a chosen directory using safe filenames. Existing destinations
require confirmation; all queries stage before any replacement, and each file
replaces atomically. A filesystem failure during final publication may leave
some files completed, with the completed count reported. Staging runs in a background worker with row/file progress. Cancel waits for the
current SQLite row or filesystem write, keeps all existing destinations and
returns the live connection with pending edits intact. Final publication disables
Cancel; a multi-file filesystem failure still reports already completed files.
SQL result CSV/JSON buttons share the format options in a compact dialog. They
export the complete captured result without executing SQL again; raw typed
snapshots retain BLOB/text distinctions, invalid UTF-8 and non-finite numbers
until conversion to the chosen output format.

Edit Pragmas exposes all18 original fields plus Cache Size and Busy Timeout.
Boolean switches and named choices share value validation; Secure Delete retains
Fast mode. Page Size/Auto Vacuum changes run VACUUM and refresh cached row IDs.
Existing database edits require a default-Cancel Save and Apply confirmation;
all values validate before any commit. Settings are read back from SQLite,
including inferred LIKE behavior, with failed/partially applied settings reported.
The form keeps focused editing/Undo, dropdown sessions and scrolling on rebuild.

Native file pickers use `rfd`. Cancel means no operation.

File → Export → Database to SQL opens a table/view chooser with structure/data,
existing-schema, column-name, multi-row and original-CREATE options. It runs on
a background worker with row progress and cooperative cancellation; completed
output replaces the destination atomically. Cancel keeps the original file and
pending database edits. Export settings persist after success; table selection
stays specific to the current database. Without INSERT column names, implicit
rowids cannot be included.
Virtual tables are selected as complete units with their internal index tables.
Everything and Structure only support FTS3/4/5 and RTree; Everything preserves
contentless/external-content indexes and pending rows without committing edits.
Restore Everything into a new database or select Replace: Keep rejects existing
virtual table names before changing data. Data only appends visible rows from
stored-content FTS, RTree and other readable virtual modules, without shadow writes;
INSERT column names retain implicit rowids. Contentless/external-content FTS require
Everything to preserve their indexes. Full dumps of modules without persistent
shadow tables are explicitly rejected; use a row export. Restoring physical indexes
requires compatible SQLite modules and permission to write shadow tables; no
connection security setting is changed by export.

Save As uses a live SQLite backup (including WAL), commits pending edits and requires a new destination. It rejects cell drafts and clears disconnected temp/attached browse results when the connection changes. SQL dumps export all rows with their original SQLite types. File CSV/JSON commands export all selected tables/views; SQL result exports include every captured row beyond the 500-row display preview. Use SQL dumps for a type-preserving round trip.

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
- Edit Cell uses TextArea for text and a dedicated address/Hex/ASCII byte editor
  for Binary, with an explicit NULL/INTEGER/REAL/TEXT/BLOB selector. Numeric edits
  preserve their type; invalid JSON remains a draft, while invalid/odd Hex paste
  is rejected before changing any bytes. Apply or Discard Draft before changing the browse target. SQLite
  column affinity still determines the stored value, as with explicit SQL writes.
- Edit Cell's checked Word wrap tool persists its preference and switches between
  Unicode line opportunities and horizontal scrolling without changing text,
  storage, selection or Undo/Redo. Long words fall back to whole grapheme clusters;
  fitting words and CJK punctuation stay together. Read-only views share it.
  Unicode15 line rules use the existing scalar metrics; full shaping, bidi/RTL,
  discretionary hyphens and Scintilla style-specific wrapping remain unfinished.
- Cell size follows the current visible draft through typing and Undo/Redo.
  Text/JSON/XML/expressions use the original Qt UTF-16 character count; Hex shows
  the current byte count; rejected Hex paste reports an error without changing it. Image captions include format,
  pixel dimensions and source bytes. Record/result position and full hover text
  update together without rebuilding the editor or evaluating SQL expressions.
- Binary mode displays 16 bytes per addressed row in system monospace, with
  hexadecimal nibble editing and a synchronized printable ASCII view. Tab switches
  Hex to ASCII; a second Tab leaves the editor, and Shift+Tab returns to Hex.
  Insert / Overwrite is available on the tool row and via Insert where supported;
  Delete/Cut removes bytes in Insert and zeros them in Overwrite. Selection,
  two-axis scrolling, caret, mode and Undo/Redo survive same-source rebuilds and
  read-only returns. Copy returns byte pairs; Copy/Export Hex + ASCII produces
  addressed readable rows, while ordinary Export retains the raw cell bytes.
  Only visible rows are rasterized, including million-byte buffers. Bytes remain
  in memory; editing may copy the shared buffer. History retains 100 byte deltas /
  8 MiB per stack, with one oversized edit retained. File import remains 16 MiB.
  Configured font families, original Print/context actions and cross-mode
  history remain unfinished.
- Edit Cell → Import offers file contents or a link. Links store the full UTF-8
  filename as a TEXT draft; files are not read or embedded for a link. Open URL /
  File sends the current text to its default application; relative filenames
  resolve beside the database. Binary/image modes do not interpret bytes as URLs.
  Open in External Editor writes a private type-suffixed copy, then offers Apply /
  Cancel. Apply reads the saved regular file (up to16 MiB), verifies the original
  cell/source/draft and uses the existing typed Apply/format-error flow. Cancel
  retains the draft/history; readonly copies only offer Close. Copies are removed
  on completion; read failures retain the copy for retry. An abnormal process
  termination can leave a private temporary copy. Default application launches
  run off the event thread; failures are shown without applying data.
- Edit Cell's checked Auto mode tool defaults on and persists its preference.
  Printable UTF-8, non-number JSON, XML declarations and supported PNG/JPEG/BMP
  pictures choose a view; corrupt/oversized images fall back to Binary. Turning
  it off retains the selected mode for later cells. Storage types and raw bytes
  remain separate from the view; image imports are drafts until Apply. RTL and
  additional Qt image formats/codecs remain unfinished.
- Image mode uses a dedicated preview with a horizontally scrollable Fit/1:1/zoom/
  original-image export row. The preview fills its pane without reserving a text
  editor; zoom/pan and focused controls survive rebuilds. Empty previews disable
  image actions, and large fonts retain the existing outer shell scroll.
- Edit Cell's checked Indent / Compact preference defaults off. JSON/XML keep
  their raw view until enabled; then loads use four-space indentation and explicit
  Apply compacts while retaining TEXT/BLOB storage. Editable toggles preserve Undo;
  readonly views never write the database. JSON formatting preserves number/string
  spellings, object order and duplicate keys. It is bounded to16 MiB input/32 MiB
  output/depth128; XML retains its existing bounds and normalizes only inter-tag
  line indentation, preserving mixed text, inline spaces, CDATA, entities and
  xml:space. Full Qt serialization and cross-mode history details remain.
- Foreign-key navigation resolves omitted parent PK columns, groups composite keys
  and filters on the stored typed values; NULL keys report no parent. Source rows
  use typed row keys: an unshadowed rowid alias, or every ordered primary-key
  component for WITHOUT ROWID and fully shadowed tables. Ambiguous or stale keys
  reject the entire operation. View headers offer Unlock View Editing: select a
  unique output column and use the view's INSTEAD OF triggers for Apply, paste,
  insertion and deletion. Relock with the checked header action; locked and
  readonly views reject writes. Failed writes retain drafts and earlier pending
  edits. Key settings follow tabs/tables and JSON/native XML projects.
- The Browse header's Show Rowid Column action exposes the accessible SQLite
  rowid alias with exact 64-bit values, sorting, filtering and guarded editing.
  Hiding it retains its filter and ordering; Clear Filters also clears its hidden
  filter. Views, WITHOUT ROWID tables and fully shadowed aliases omit this action.
  Column widths and frozen prefixes map across display changes and project saves.
  Native project width/freeze indices are compatible; next-gui extensions retain
  implicit-rowid filter/order terms that the original XML cannot represent safely.
- SQL tabs own results, export snapshots, messages, widths and scroll positions.
  Save as VIEW takes an editable name and the executed result statement, so editing
  the script after execution does not change the view source.
- PRAGMA writes are read back; settings that do not take effect report the actual
  value and possible earlier partial application. NONE→auto_vacuum requires an
  explicit VACUUM. Plot binds typed Browse/SQL sources, exposes X/Y1/Y2 columns and retained series styles, and can load complete sources through cancellable workers. Zoom/pan, point selection, named hover information, fixed number formats and signed grouped/stacked bars retain source row identities. Current-view image clipboard and staged PNG/JPEG/BMP/raster-PDF saves are implemented. Table-to-Plot highlighting, Print/vector PDF, remaining conversion/style details and original native/platform acceptance remain.
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


Numeric PRAGMA fields use next-gui's exact signed integer steppers: click the
right-side arrows, use Up/Down for one or PageUp/PageDown for ten, or edit/paste
the integer directly. Changes retain Undo/Redo and focus through resizing.
The original form's labels share a measured alignment column; compact forms
stack labels above controls. Validation still runs before applying any setting.

Long confirmation messages keep their actions fixed while the explanation scrolls.
Use the wheel, scrollbar, Up/Down, PageUp/PageDown or Home/End to read the full
message. Rebuilding or resizing retains the reading position and active action
for the same question; a new question starts at its defaults. Long table names
also use bounded GPU texture tiles instead of exceeding the adapter limit.

Holding a numeric arrow steps once immediately, waits500ms, then repeats every
150ms, matching Qt's common spinbox mouse timing. Moving outside its arrow
pauses; returning starts the delay again. Release, native window leave/focus loss,
resizing, scrolling, typing/paste, IME composition or a new modal stops tracking.
Reaching a bound clears the pressed appearance; delayed frames never burst.
Repeated steps keep exact i64 values and Undo/Redo. PRAGMA edits remain drafts
until Save Pragmas is chosen.

Hosts using the shared input steppers should call `Ui::repeat_pointer()` during
redraw, sync/repaint when it returns true, and schedule the next
`Ui::pointer_repeat_delay()` through `WindowCtx::request_redraw_after`.
Deliver release events and call `Ui::cancel_pointer_repeat()` on native pointer
leave, including when tracking is paused outside an arrow. A fresh declarative
UI deliberately cancels capture; editor values/history use their existing sessions.

Cell JSON/XML diagnostics now mark parser positions, including CJK and end-of-input,
without replacing selection/history. Apply and focused Cmd/Ctrl+Enter open the same
Cancel-default house confirmation for malformed format data. Apply Anyway stores
the exact typed draft without formatting; it repeats writable/generated/source and
storage checks and rejects changed confirmations. Save All retains chosen destinations
and postpones SQL-file/database/project writes and document closing until the question
is resolved. Cancel/Escape retain the draft and selection. Parser resource limits,
INTEGER/REAL conversion errors and SQLite write failures cannot use this override.
JSON error columns use parser byte units; visual marks convert them to scalar indices.

Structured cell validation checks original BLOB UTF-8 before trusting a replacement-
character preview; invalid source bytes stay raw until explicit confirmation or a
real edit repairs them. Indent does not format that lossy preview. Apply compares
the actual target's SQLite type and bytes, skips unchanged UPDATEs/triggers/dirty
flags, restores ASCII-case aliases to the right column, and rejects vanished rows.
A savepoint rolls back ignored updates and their trigger side effects while retaining
previous pending changes and the current draft. Explicit evaluation still runs once.

Text and evaluation views hide one leading UTF-8 BOM. Text edits retain it exactly
once, including typed BLOB/TEXT conversion and staged export; Hex exposes the raw
bytes and can remove it. JSON accepts a BOM only at the absolute start, preserves
it during lossless indentation/compaction and accounts for it in error positions.
This deliberately preserves source bytes instead of normalizing them like the
original JSON serializer. UTF-16/32 and selectable cell codecs remain unfinished.
Stored SQLite REAL infinities remain clean and can be replaced with finite values;
typing new infinities/NaN still fails validation. Evaluation retains SQLite types.


View keys use the original `unlock_view_pk` project setting. Empty or `_rowid_`
means locked when importing an original project. An actual view output column
named `_rowid_` can be selected locally and retained by next-gui's XML extension;
the original application reserves that spelling for its locked sentinel. Unlock
chooses a key without scanning the entire view; every write rejects missing or
ambiguous target keys. Trigger behavior belongs to the database, and the grid
reloads the stored result after Apply. Missing INSTEAD OF triggers, ignored
writes and SQLite errors retain the draft. Native desktop/platform and full
original product/interface parity remain unfinished.


Browse filters apply after a short typing pause (default200ms), immediately on
focus loss, or with Enter/Apply. Preferences → Filter delay accepts0–5000ms;
zero applies immediately. Filter escape defaults to backslash; use it before
LIKE `%`/`_` to match literal wildcards, choose another single character, or
leave it empty to disable escaping. Queries run on the existing connection in
a cancellable background worker. New typing cancels old criteria, and failures
retain the previous result and input. Composition, unapplied cell drafts and
modal work suspend automatic filtering. The raw WHERE field still uses explicit
Apply/Enter. Filter context helpers and complete original parity remain pending.
