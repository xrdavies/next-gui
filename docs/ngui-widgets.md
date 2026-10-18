# `.ngui` widget surface

> Status: **full FlexBuilder coverage** (2026-10-19). Maps to the same retained `Ui` as the Rust API. Exception: raw `ImageRgba` pixels are Rust-only — use `ImagePlaceholder` in `.ngui`.

## Conventions

- **PascalCase** keywords (`TextInput`, `ButtonGroup`, `ColorPicker`, …). Alias: `CheckBox` → `Checkbox`.
- `@name` binds a `WidgetId` in `LoadedUi::names`. Place **before** `{ … }` when a body follows: `Panel("T") @p { … }`.
- Positional args in `(…)`; style / seed props as `key: value` inside `{…}`; nested widgets are PascalCase children.
- Lists `[…]`, objects `{ key: value }`, bools `true`/`false`, colors `#RRGGBB`, `//` line comments.
- Document root is **`Column`**. Optional overlays after the root: `Modal` / `Drawer` / `Notify`.

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

## Tests

- Unit: `crates/next-gui-ui/src/dsl/mod.rs` (counter, row, unknown prop, **all_widgets** fixture).
- Fixture: `crates/next-gui-ui/tests/fixtures/all_widgets.ngui`.

```bash
cargo test -p next-gui-ui --lib dsl::
```
