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
