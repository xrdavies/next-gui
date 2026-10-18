# Theme tokens（MVP）

> **状态：进行中（分支 `feat/theme-tokens`）**。不是 CSS、不是 Qt Style 引擎、不是 ImGui 全局 StyleVar。
> 产品路线：retained 组件 + **Theme tokens** + 渐进加强 `.ngui`。

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
    col.text("主题色文字");           // theme text_primary + body
    col.button("OK");                 // theme button_*
    col.text_input("placeholder");    // TextInputStyle::from_theme*
    col.link("docs");                 // LinkStyle::from_theme*
});

// 显式 Style 仍优先：
ui.column(ColumnStyle { background: Some(...), ..ui.column_style() }, ...);
```

Tokens：`ThemeColors` / `ThemeTypography` / `ThemeSpacing` / `ThemeRadii`。  
Density：`UiDensity::spacing_scale` / `font_scale` 在 `from_theme_with_density` 与 unstyled Text/Button 上生效。

## MVP 已接线的 Style

| 路径 | 说明 |
|--|--|
| `ColumnStyle` / `RowStyle` / `ToolbarStyle` / `PanelStyle` / `StatusBarStyle` | `from_theme` / `from_theme_with_density` + `Ui::*_style()` |
| `TextInputStyle` / `LinkStyle` / `CalendarStyle` | 同上 |
| `FlexBuilder::text` / `button` / `tool_button` / `icon` / `text_input` / `link` | 无参重载读当前 Theme |

`XxxStyle::default()` **保持历史硬编码**（≈ dark），避免静默破坏旧示例；新代码优先 `from_theme` / `ui.column_style()`。

## 尚未迁移（跟进）

其余 Office / P1 Style（Table、Menu、Modal、Chart、…）仍用 `Default` 硬编码。迁移方式：为每个 Style 增加同样的 `from_theme_with_density`，必要时加 `Ui::foo_style()`；paint 路径已读 `UiConfig::accent` 的继续用 `Ui::resolve_accent()`。

可选下一步：`.ngui` 引用 token 名；暗色跟随系统；有限样式表（仍不做满血 CSS）。

## 校验

```bash
cargo test -p next-gui-ui theme -- --nocapture
cargo test -p next-gui-ui
```
