# Theme tokens

> **状态：全 Style 覆盖 + `.ngui` theme / token 名 + Soft/GPU 圆角（含 GPU SDF ~1px AA）+ 示例已迁到 `from_theme`（分支 `feat/theme-tokens`）**。不是 CSS、不是 Qt Style 引擎、不是 ImGui 全局 StyleVar。
> 产品路线：retained 组件 + **Theme tokens** + 渐进加强 `.ngui`。
> 示例：`cargo run -p e27_theme_tokens`（`NEXT_GUI_THEME_QA=1` 导出 PPM）。

## 和 Profile / UiConfig 的分工

| 层 | 管什么 | API |
|--|--|--|
| **Profile**（Field / Office） | Soft/GPU、帧率、Immediate 开关 | `ProfileConfig` / `NEXT_GUI_PROFILE` |
| **Theme** | 色板、字阶、间距、圆角 | `Theme::light()` / `Theme::dark()`、`Ui::set_theme` |
| **UiConfig** | locale、density、accent 覆盖 | `Ui::set_config` |

`Profile` 不管亮暗；亮暗与密度由 Theme + UiConfig 决定。

## API 草图

```rust
ui.set_theme(Theme::light());
ui.set_config(UiConfig {
    density: UiDensity::Compact,
    accent: Some(Color::rgb(0xe0, 0x60, 0x60)), // 覆盖 theme.accent
    ..UiConfig::default()
});

ui.column(ui.column_style(), |col| {
    col.text("主题色文字");
    col.button("OK");
    col.checkbox("On", true);          // theme
    col.text_input("placeholder");
    col.link("docs");
    col.list(items, ui.list_style());  // 需 style 的 API 用 helpers
});

// 显式 Style 仍优先：
ui.column(ColumnStyle { background: Some(...), ..ui.column_style() }, ...);
```

Tokens：`ThemeColors`（含 surface / accent / danger / alert 底色等语义色）/
`ThemeTypography` / `ThemeSpacing` / `ThemeRadii`。  
Density：`UiDensity::spacing_scale` / `font_scale`。

`set_theme` / `set_config` 会刷新内置 `notification_style` toast 外观。

## 全量 Style 覆盖

每个 `pub struct XxxStyle` 均提供 `from_theme` + `from_theme_with_density`；`Ui` 有对应 `*_style()`（通知 toast 为 `notification_toast_style`，避免与字段名冲突）。

| 组 | Styles |
|--|--|
| Shell | Column, Row, Toolbar, Panel, StatusBar, Splitter, ScrollArea, Space, Grid |
| Input | TextInput, TextArea, InputNumber, Checkbox, Radio, Switch, Select, Slider, Autocomplete, ColorPicker, Upload, DateTime |
| Data | List, Tree, Table, TreeTable, Transfer, PropertyGrid, Descriptions, AlarmList, Chart, Gauge, Trend, Timeline, Calendar, Image |
| Nav / chrome | Menu, ContextMenu, Tabs, Breadcrumb, Pagination, Segmented, ButtonGroup, Steps, Link |
| Feedback | Tag, Badge, Tooltip, Modal, Drawer, Backdrop, Progress, Alert, Notification, Spin, Empty, Collapse, Card, Popover, Popconfirm, Result, Statistic |

无参 / 轻量构造（`text` / `button` / `tool_button` / `icon` / `text_input` / `link` / `checkbox` / `radio` / `switch` / `tag` / `badge_*` / `tooltip` / `progress` / `alert`）读当前 Theme。

`XxxStyle::default()` **保持历史硬编码**（≈ dark），避免静默破坏旧示例与测试；新代码优先 `from_theme` / `ui.*_style()`。

## 有意例外

- **`UiConfig`**：不是视觉 Style，是 locale / density / accent 袋。
- **枚举色**：`ResultStatus::color_for_theme` / `AlarmSeverity::color_for_theme` 读 `ThemeColors`（`info` / `success` / `warning` / `danger` / …）；无参 `color()` 仍对齐 dark 历史值。
- **`.ngui` DSL**：已支持 `theme: light|dark`（文档级或根 `Column` prop）；无显式色时用 `from_theme`。颜色 prop（`background` / `bg` / `fg` / `color` / ColorPicker `value` 等）可写 **未加引号的 token 名**（`ThemeColors` 字段，如 `background: surface`、`bg: accent`）；`#RRGGBB` 仍可用；未知名报错。
- **Radii**：Soft 经 `Ui::apply_corner_radius` 把 `ThemeRadii`（`control` / `panel`）接到有填充 chrome 的控件上：
  - **已覆盖（host / popup / 选中行等）**：Button、TextInput、Panel、Card、Modal·Drawer、Tag、Alert、Select·TreeSelect（字段+popup）、Autocomplete、Checkbox、Switch、Tooltip、Popover·Popconfirm（trigger+popup）、List、Badge、Progress、Tabs、Toolbar·StatusBar、TextArea、InputNumber、Notification；以及 Tree / Table / TreeTable / VirtualTable（同 Table）、Menu·ContextMenu（标题/host + popup）、Empty、Collapse、Descriptions、Segmented、ButtonGroup、Pagination 按钮、Statistic、Result、Calendar、Timeline、Transfer、PropertyGrid、AlarmList、Form·Grid、Upload、ColorPicker、DateTime、Image、Trend / Chart 主机、Spin 遮罩。Steps 标记与 Timeline 点为几何圆（`set_corner_radius(半宽)`）；Radio / Switch thumb 同理。
  - **有意仍直角 / 不套 radii**：Link 下划线、Breadcrumb 纯文字、Steps 连接线与 Timeline 轨道（发丝线）、Table / TreeTable / PropertyGrid **表头条**（贴在已圆角 host 顶边，四角全圆会错）、Gauge 表盘与 Trend 折线（alpha mask 几何）、Image 马赛克单元格。圆角数值只在 Theme，不在 `.ngui` 单独配 `radius`。
  - GPU `FillRoundedRect`：SDF fragment + alpha blend，~1px coverage AA（与 Soft 同量级）；`radius < 0.5` 仍走不透明直角填充。`FillRect` / `BlitAlpha` 路径不变。
  - **`.ngui` 写 `radius` / `radii`：刻意延后，非当前重点**（圆角只在 Theme `ThemeRadii`）。

## 示例迁移

旧 Office / 小 demo（e02、e04、e05、e10–e15、e20–e26）已从 `XxxStyle::default()` 改为 `XxxStyle::from_theme(&theme)`（构建前 `let theme = *ui.theme()`），保留字段级覆盖。`e27_theme_tokens` 仍是 `ui.*_style()` 参考。`ImmediateViewportStyle` 无 `from_theme`，e25 仍用 `::default()`。库内 `XxxStyle::default()` 语义未改（历史硬编码）。

## 校验

```bash
cargo test -p next-gui-ui theme
cargo test -p next-gui-ui
cargo clippy -p next-gui-ui --all-targets -- -D warnings
```
