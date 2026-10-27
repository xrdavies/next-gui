# `.ngui` widget surface

> Status: **full FlexBuilder coverage** (2026-10-19). Maps to the same retained `Ui` as the Rust API. Exception: raw `ImageRgba` pixels are Rust-only — use `ImagePlaceholder` in `.ngui`.

## Conventions

- **PascalCase** keywords (`TextInput`, `ButtonGroup`, `ColorPicker`, …). Alias: `CheckBox` → `Checkbox`.
- `@name` binds a `WidgetId` in `LoadedUi::names`. Place **before** `{ … }` when a body follows: `Panel("T") @p { … }`.
- Positional args in `(…)`; style / seed props as `key: value` inside `{…}`; nested widgets are PascalCase children.
- Lists `[…]`, objects `{ key: value }`, bools `true`/`false`, colors `#RRGGBB`, `//` line comments.
- Document root is **`Column`**. Optional overlays after the root: `Modal` / `Drawer` / `Notify`.
- **Theme:** optional `theme: light` or `theme: dark` as a **document-level** directive before the root `Column`, or as a prop on the root `Column` (root prop wins if both are set). Default is dark (same as `Ui::new`). Widget Style defaults use `XxxStyle::from_theme`; explicit color props (`background`, `bg`, `fg`, …) still override. Color props also accept unquoted **token names** matching `ThemeColors` fields (`surface`, `accent`, `text_primary`, …).

## Keywords

| Keyword | Notes |
|---------|--------|
| `Column` `Row` `Toolbar` `Panel` `StatusBar` `ScrollArea` `Space` `Grid` `Spin` | Containers with children |
| `Splitter` | Exactly **two** child widgets (panes) |
| `Text` `Button` `Icon` `Spacer` `Separator` `Link` | Basics |
| `TextInput` `TextArea` `Checkbox` `Radio` `Switch` `InputNumber` `Upload` `ColorPicker` `Autocomplete` | Inputs |
| `List` `VirtualList` `Tree` `Table` `VirtualTable` `TreeTable` `Select` `TreeSelect` `Slider` `Transfer` `PropertyGrid` `DateTime` | Data |
| `Menu` `ContextMenu` `Tabs` / `Tab` `Breadcrumb` `Pagination` `Steps` `ButtonGroup` `Segmented` | Nav |
| `Tag` `Badge` `BadgeDot` `Tooltip` `Progress` `Alert` `Empty` `Collapse` `Descriptions` `Card` `Statistic` `Result` `Popover` `Popconfirm` `Timeline` `Calendar` `ImagePlaceholder` | Display / feedback |
| `Form` / `Field` | `Field("key","Label") { rules: […] control }` inside `Form` |
| `AlarmList` `Gauge` `Trend` `Chart` | Industrial / chart |
| `Modal` `Drawer` `Notify` | Document-level overlays (not flex children) |

**Not in `.ngui`:** `ImageRgba` (binary pixel buffer) — Rust `FlexBuilder::image_rgba` only.

## Literal forms (minimal)

```text
List(["a", "b"])
Tree([{ id: 1, label: "root", children: [{ id: 2, label: "leaf" }] }])
Table @t { columns: [["Name", 80]], rows: [["x"]] }
Menu("File", ["Open", "Save"])
Tabs @tabs { Tab("One") { Text("p1") } }
Select(["x","y"]) { selected: 0 }
Form @f { Field("name", "Name") { rules: ["required"] TextInput("") } }
Chart { tiles: [{ kind: gauge, title: "G", value: 50, min: 0, max: 100 }] }
Modal("Title", "Body", "OK") @m { open: false secondary: "Cancel" }
```

Heavy trees/tables support enough literals to construct via existing APIs; not a full spreadsheet language.

## Theme

```text
theme: light
Column {
  width: 640
  height: 400
  // no background → Theme::light().colors.surface
  Button("OK")           // theme button colors
  Button("X") { bg: #6b7280 fg: #ffffff }  // explicit wins
}
```

Equivalent root prop: `Column { theme: dark … }`. Unknown names (`theme: neon`) are rejected.

**Limits:** nested widgets cannot set `theme` (unknown Column property). Unknown color token names are rejected. `ImageRgba` remains Rust-only.

```text
theme: light
Column {
  width: 640
  height: 400
  background: surface
  Button("OK") { bg: accent fg: on_accent }
  Text("hi") { fg: text_secondary }
}
```

## Tests

- Unit: `crates/next-gui-ui/src/dsl/mod.rs` (counter, row, unknown prop, theme light/dark, **all_widgets** fixture).
- Fixture: `crates/next-gui-ui/tests/fixtures/all_widgets.ngui`.

```bash
cargo test -p next-gui-ui --lib dsl::
```

## TextArea editing API

`FlexBuilder::text_area` keeps its existing builder/DSL interface. Shared TextArea now
supports pointer caret placement and drag selection, Shift+arrows/Home/End,
Select All, selected-text copy/paste/IME replacement, wheel scrolling, a viewport
indicator, and undo/redo. The caret follows wrapped visual lines and is revealed
after editing or finding text. Both renderers use the same selection alpha mask.

- `Ui::text_area_selection` / `select_text_area_range`: Unicode scalar offsets,
  not UTF-8 byte offsets. Empty ranges place the caret.
- `text_area_selected_text` / `replace_text_area_selection`: selected text and
  one undoable replacement; empty replacement deletes the selection.
- `find_in_text_area`: literal, case-sensitive forward search with one wrap;
  selects and reveals the next match without changing focus.
- `text_area_scroll` / `set_text_area_scroll`: vertical pixel offset; invalid
  floats are rejected and the offset is clamped to the document.
- `set_text_area_foregrounds(id, spans)`: disjoint Unicode scalar ranges with
  per-range colors; rejects overlap/out-of-bounds ranges atomically. Empty
  ranges are ignored. Selection uses the normal foreground for readability.
  Same-document sessions retain colors; edits/Undo/Redo clear stale offsets.
  Preedit composition temporarily uses the normal foreground. Supply fresh
  spans after edits/theme changes; this is presentation, not a language parser.
- `set_text_area_line_numbers(id, bool)`: optional physical line-number gutter;
  DSL `TextArea { line_numbers: true }`. LF/CRLF, wrapped continuations, empty
  lines and digit-width changes follow the same text layout as pointer/IME and
  selection. Numbers are excluded from document copy and undo. Default is off.
- `text_area_state` / `restore_text_area_state`: opaque in-memory editing session
  for rebuilding widgets. Restore rejects a different document. Keep one session
  per document; project files should save text separately.
- Map platform chords to `UiKey::Undo`, `Redo`, `SelectLeft`, `SelectRight`,
  `SelectUp`, `SelectDown`, `SelectHome`, `SelectEnd`. Existing exhaustive matches
  on `UiKey` must handle these variants. `focused_text_for_copy` now returns only
  a TextArea selection (None when empty); TextInput behavior is unchanged.

History stores up to 100 snapshots / 8 MiB per undo or redo stack, retaining one
oversized edit. Snapshots share immutable text across rebuilds. Glyph layout still
processes the complete document; masks and selection nodes cover the viewport.
This is plain-text editing: syntax/completion, grapheme-cluster navigation, word
selection, horizontal scrolling and draggable scrollbar thumbs are not provided.

The e21 QA mode `NEXT_GUI_P1_REST_QA=textarea` captures the entry fields with the
Drawer closed and exits. e28 exercises the same widget for SQL, Find, DDL and CSV.


`Tabs { scrollable: true ... }` uses each title's natural width and shows arrow
buttons when needed. The default is `false` and retains equal-width tabs.
Arrow buttons scroll without switching pages; keyboard selection reveals the
chosen title. Rust builders use `TabsStyle::scrollable`; rebuilds can retain
manual strip scrolling with `Ui::tabs_state` / `Ui::restore_tabs_state`.
