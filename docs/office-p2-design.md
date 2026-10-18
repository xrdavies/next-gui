# Office P2 控件设计（目录 §6.2）

> **勿与 [p2-design.md](p2-design.md)（路线图 P2：CJK / 文本引擎）混淆。** 本文仅覆盖 [office-widget-catalog.md](office-widget-catalog.md) §6.2 的 Office 低频控件。

**状态：P2a 已落地**（`button_group` / `link` / `space`；示例 `e26_office_p2`）。后续切片见下。

配套：[office-line.md](office-line.md)、[office-widget-catalog.md](office-widget-catalog.md)、[roadmap.md](roadmap.md)、[a11y-baseline.md](a11y-baseline.md)。

---

## 1. 目标

在现有 retained `Ui` / `Scene` / Soft↔GPU 契约上，按目录补齐 P2 低频控件，保持 Field Soft 可回退、无新渲染后端。

---

## 2. 非目标

| 项 | 说明 |
|----|------|
| Cascader | 仍可选；层级选择继续以 `tree_select` 为主（见目录 §10） |
| 动画系统 | 不引入通用动画 / transition |
| AT 总线 | 不接读屏；沿用 P6d 焦序 + `accessible_name` |
| Field Soft | 不削弱 Soft 兜底；P2 控件必须 Soft 可画 |
| 上传系统对话框 / 完整栅格系统 | 留 P2d / P2e |
| 覆盖 `docs/p2-design.md` | 禁止 |

---

## 3. 切片

| 切片 | 控件 | 说明 |
|------|------|------|
| **P2a** ✅ | `button_group`、`link`、`space` | 薄、无弹出层；本切片 |
| **P2b** | `steps`、`statistic`、`result` | 展示向；无文件/色板 |
| **P2c** | `popover`、`popconfirm` | 轻量浮层（复用 overlay 约定） |
| **P2d** | `upload`、`color_picker`、`autocomplete` | 录入增强；文件对话框可 stub |
| **P2e** | `grid`、`timeline`、`calendar`、`image`、`config_provider` | 布局/媒体/主题入口 |

可微调切片边界，但保持「先薄、后弹出/文件」。

---

## 4. P2a 契约

### 4.1 `button_group`

- **语义：** 互斥选择（轻量 Segmented 风格），**不是**仅布局一排独立 Button。
- **焦点：** 整组一个 Tab 停点（同 Segmented）；Left/Right（及 Up/Down）切换选项。
- **状态：** `selected: Option<usize>`（`None` = 未选）；指针点选 / 键盘改选后可 `take_button_group_change() -> Option<(WidgetId, Option<usize>)>`。
- **外观：** 离散按钮条（间隙 + 选中底），非整条 Segmented 轨道。
- **API：** `ui.button_group(labels, selected, ButtonGroupStyle) -> WidgetId`；`button_group_selected` / `set_button_group_selected`。

### 4.2 `link`

- **语义：** 文本样式可点击（默认下划线 + 强调色）。
- **焦点：** Tab 可达；Space / Enter → `take_link_activate() -> Option<WidgetId>`。
- **指针：** 与 Button 双路径一致——`pointer_pressed` 设焦点并返回 `false`；应用仍用 `hit_widget` 处理点击。
- **API：** `ui.link(label)` / `link_styled(label, LinkStyle)`。

### 4.3 `space`

- **语义：** 间距容器（Ant Space / MUI Stack 对标）。
- **实现：** **无独立 `Kind`**——`space(SpaceStyle, build)` 按 `SpaceDirection` 糖衣到既有 `Row` / `Column`（写入 gap / padding / 尺寸 / 背景）。
- **vs Row/Column：** 同一套 flex 布局；`space` 是目录名与方向开关的便捷入口；需要显式轴语义时继续用 `row` / `column`。

---

## 5. P2a 验收

| 项 | 通过样子 |
|----|----------|
| API | `button_group` / `link` / `space` 可在 `FlexBuilder` 中声明 |
| 选择 | 键盘 Left/Right 与指针改选；`take_button_group_change` 可排空 |
| Link | Tab 可达；Space/Enter → `take_link_activate`；指针双路径 |
| Space | 水平/垂直 gap 可见（示例）；实现为 Row/Column 糖衣 |
| 测试 | `cargo test -p next-gui-ui --lib` 相关用例绿；不依赖 DISPLAY |
| 示例 | `e26_office_p2`；无 DISPLAY 时跳过 GUI 跑，不挂死 |
| 文档 | 本文 + catalog §9 + office-line / roadmap / README 同步 |

---

## 6. 实现落点

- 样式 / 方向：`crates/next-gui-ui/src/office_p2.rs`
- Kind / 交互 / paint：`crates/next-gui-ui/src/lib.rs`（`ButtonGroup`、`Link`）
- 示例：`examples/26_office_p2`

---

## 7. 修订记录

| 日期 | 说明 |
|------|------|
| 2026-10-19 | 初稿 + **P2a 落地**：button_group / link / space + `e26_office_p2`。 |
