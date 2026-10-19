# Theme tokens

> **状态：全 Style 覆盖（分支 `feat/theme-tokens`）**。不是 CSS、不是 Qt Style 引擎、不是 ImGui 全局 StyleVar。
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
- **枚举色**（如 `ResultStatus::color`、`AlarmSeverity`）：状态语义色，暂不进 Theme；后续可挂 token。
- **`.ngui` DSL**：仍用 `ColumnStyle::default()` 解析默认；token 名进 DSL 为跟进项。
- **Radii**：token 已存，多数 Soft 控件尚未画圆角。

## 校验

```bash
cargo test -p next-gui-ui theme
cargo test -p next-gui-ui
cargo clippy -p next-gui-ui --all-targets -- -D warnings
```
