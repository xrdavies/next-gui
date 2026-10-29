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

## Icon buttons

`Button("Save database") { icon: save }` uses a square house icon while retaining
the full label for accessibility and the existing keyboard, pointer, focus and
disabled behavior. Unknown/empty glyph names are rejected. Omit `icon` for the
original text button. `Tooltip("Save database") { Button("Save database") { icon: save } }`
adds explicit help in `.ngui`.

Rust toolbar builders `tool_icon_button(IconName::Save, "Save database")` and
`tool_icon_button_styled` automatically show a tooltip from the current accessible
name, including later `set_button_label` changes. They return the actual button ID.
`Ui::set_button_icon(id, None)` restores the authored text sizing after `sync`.

`Ui::set_button_icon_layout` selects `ButtonIconLayout::IconOnly`, `TextOnly`,
`TextBesideIcon` or `TextUnderIcon`. Measurement includes both the icon and font;
the label, focus, disabled state and live tooltip retain their usual behavior.
The equivalent `.ngui` property is `icon_layout: icon_only | text_only |
text_beside_icon | text_under_icon`. Without an icon, normal text sizing applies.
The new house glyphs are `database`, `undo`, `step`, `code` and `wrap`; consumers matching
`IconName` exhaustively need corresponding arms. Existing button style structs and
text builders are unchanged.

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
- `set_text_area_wrap(id, bool)` / `text_area_wrap`: line wrapping, enabled by
  default; DSL `TextArea { wrap: false }` keeps physical lines and scrolls horizontally.
  Switching preserves text, selection and history, and reveals the caret. Wrapping
  uses the existing character layout; word-boundary wrapping is not yet provided.
- `text_area_scroll` / `set_text_area_scroll`: vertical pixel offset, retaining x.
  `text_area_scroll_offsets` / `set_text_area_scroll_offsets` expose both axes.
  Invalid floats are rejected; offsets clamp to the document, with x zero while
  wrapping. Wheel and draggable scrollbar thumbs share these offsets.
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
  per document; project files should save text separately. Restoration keeps the
  destination widget's wrap option and reveals the caret when geometry changes.
- `show_text_area_completions(id, range, items)`: application-filtered candidate
  labels/insertion text in a caret-anchored menu. The scalar range contains the
  unselected caret; acceptance is one undoable replacement. Up/Down, pages,
  Home/End and wheel reach every candidate; Enter/Tab or click accept; Escape
  dismisses. Other input continues to the editor; refresh suggestions after edits.
  Composition/read-only/foreign popups reject opening, and stale focus/document/
  caret state prevents insertion. Lists are transient across widget rebuilds.
- `set_text_area_hint(id, anchor, text)` / `text_area_hint(id)`: passive hints at
  a scalar anchor, with wrapped text bounded to the viewport and editor-sized
  type. They retain editor focus and let pointer/wheel input reach underlying
  controls. Refresh after edits; empty text clears. Escape suppresses the same
  anchor/prefix through argument edits and matching TextArea session restoration.
  Stale documents/selections, composition, read-only state, focus loss, offscreen
  anchors and open menus hide them. The shared Tooltip painter also wraps normal
  tooltips, flips above when needed and removes a previous host's tip.
- Map platform chords to `UiKey::Undo`, `Redo`, `SelectLeft`, `SelectRight`,
  `SelectUp`, `SelectDown`, `SelectHome`, `SelectEnd`. Existing exhaustive matches
  on `UiKey` must handle these variants. `focused_text_for_copy` now returns only
  a TextArea selection (None when empty); TextInput behavior is unchanged.

History stores up to 100 snapshots / 8 MiB per undo or redo stack, retaining one
oversized edit. Snapshots share immutable text across rebuilds. Glyph layout still
processes the complete document; masks and selection nodes cover the viewport.
Apps supply token colors, completion candidates and hint text; TextArea has no built-in
language lexer or semantic catalog. Grapheme-cluster caret navigation and word
selection are not provided. Clipped text rasterization bounds each viewport/glyph
mask to 64 MiB; larger masks return empty and require tiled rendering.

The e21 QA mode `NEXT_GUI_P1_REST_QA=textarea` captures the entry fields with the
Drawer closed and exits. e28 exercises the same widget for SQL, Find, DDL and CSV.


`Tabs { scrollable: true ... }` uses each title's natural width and shows arrow
buttons when needed. The default is `false` and retains equal-width tabs.
Arrow buttons scroll without switching pages; keyboard selection reveals the
chosen title. Rust builders use `TabsStyle::scrollable`; rebuilds can retain
manual strip scrolling with `Ui::tabs_state` / `Ui::restore_tabs_state`.
`strip_at_bottom: true` (Rust: `TabsStyle::strip_at_bottom`) places the same strip
below the page, including overflow arrows and keyboard selection; its default is `false`.


Rust `ChartStyle::show_plot_coordinates` (DSL `Chart { show_plot_coordinates: true }`)
opts into a passive pointer-following tooltip for Cartesian plot surfaces. It uses
the exact cached image viewport, shows named X/Y1 and optional Y2 values, formats
ISO date/time X values and omits categorical X like DB4S. Hovering a caption shows
its full title; axis gutters have no coordinates. Hover reuses image/axis caches; model edits and navigation
refresh them on sync. Tooltips wrap and stay within the viewport, hide for open
menus and pass hits to underlying widgets. Hosts should forward movement through
`pointer_hover` / `pointer_moved` and window leave through `Ui::pointer_left`, then
sync. The option defaults to false in both themes; exhaustive Rust `ChartStyle`
literals must add the field (or use `..Default::default()`).
