# Office P2 控件设计（目录 §6.2）

> **勿与 [p2-design.md](p2-design.md)（路线图 P2：CJK / 文本引擎）混淆。** 本文仅覆盖 [office-widget-catalog.md](office-widget-catalog.md) §6.2 的 Office 低频控件。

**状态：Office P2a–e 已齐**（`button_group` … `config_provider`；示例 `e26_office_p2`）。Cascader 仍可选 / 不进本切片。

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
| 原生系统文件对话框 / 完整响应式 12 栅格 | OS 对话框由应用 stub（P2d）；P2e `grid` 为固定列数 CSS-grid-lite |
| 覆盖 `docs/p2-design.md` | 禁止 |

---

## 3. 切片

| 切片 | 控件 | 说明 |
|------|------|------|
| **P2a** ✅ | `button_group`、`link`、`space` | 薄、无弹出层 |
| **P2b** ✅ | `steps`、`statistic`、`result` | 展示向；无文件/色板 |
| **P2c** ✅ | `popover`、`popconfirm` | 轻量浮层（复用 `open_popup` / overlay） |
| **P2d** ✅ | `upload`、`color_picker`、`autocomplete` | 录入增强；文件对话框可 stub |
| **P2e** ✅ | `grid`、`timeline`、`calendar`、`image`、`config_provider` | 布局/媒体/主题入口 |

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

## 6. P2b 契约

### 6.1 `steps`

- **语义：** 向导步骤条；标题 + 可选短描述；`current: usize`（0-based）。
- **状态色：** `i < current` = done，`i == current` = process，其后 = wait。
- **方向：** `StepsDirection::Horizontal`（默认）或 `Vertical`。
- **焦点：** 整条一个 Tab 停点；水平 Left/Right（兼 Up/Down），垂直 Up/Down（兼 Left/Right）改 `current`。
- **指针：** `StepsStyle::clickable`（默认 true）时点标记/槽位改选；`take_steps_change() -> Option<(WidgetId, usize)>`。
- **API：** `ui.steps(items, current, StepsStyle) -> WidgetId`；`items` 为 `StepItem`（`title` + `Option<description>`）；`steps_current` / `set_steps_current`。

### 6.2 `statistic`

- **语义：** KPI 块——标题 + 数值字符串；可选 prefix / suffix / hint（delta）。
- **焦点：** **不进 Tab 序**（纯展示）。
- **API：** `ui.statistic(title, value, StatisticStyle)`；`statistic_full(..., prefix, suffix, hint, style)`。
- **a11y：** `accessible_name` 回落为 `"title: value"`（或其一非空时的单侧）。

### 6.3 `result`

- **语义：** 结果页块——`ResultStatus`（Success / Info / Warning / Error）+ 标题 + 可选副标题 + 可选主/次操作文案。
- **焦点：** **有任一操作文案时** Result 本身进 Tab 序（操作是宿主上的 hit 区，无独立子 Widget）；无操作时不进焦序。
- **键盘：** 聚焦时 Space/Enter → Primary（若无 Primary 则 Secondary）；`take_result_action() -> Option<(WidgetId, ResultAction)>`。
- **指针：** 点主/次按钮矩形 → 对应 `ResultAction`。
- **外观：** Soft 友好（状态字标 + 居中标题/副标题 + 右下操作钮；复用 Empty/Alert 色调）。
- **API：** `ui.result(status, title, subtitle, primary_action, secondary_action, ResultStyle)`。

---

## 7. P2b 验收

| 项 | 通过样子 |
|----|----------|
| API | `steps` / `statistic` / `statistic_full` / `result` 可在 `FlexBuilder` 中声明 |
| Steps | 键盘与（clickable）指针改 `current`；`take_steps_change` 可排空 |
| Statistic | 挂载可画；不在 `focusable_widgets` |
| Result | 有操作时 Tab 可达；Space/Enter → Primary；指针分主/次；`take_result_action` 可排空 |
| 测试 | `office_p2b_*` 绿；不依赖 DISPLAY |
| 示例 | `e26_office_p2` 含 P2b 区；`NEXT_GUI_P2B_QA=1` 可 dump；无 DISPLAY 不挂死 |
| 文档 | 本文 + catalog §9 + office-line / roadmap / README 同步 |

---

## 8. P2c 契约

### 8.1 `popover`

- **语义：** 宿主触发器 + 下方气泡（可选标题 + 正文）；**点击切换**开合（Tooltip 已覆盖悬停）。
- **关闭：** 再点宿主、点外侧、Escape；走既有 `open_popup` / overlay。
- **放置：** 默认宿主底边下方（`gap`）；右溢出时向左夹紧到 viewport。
- **焦点：** 宿主进 Tab 序；Space/Enter 切换开合；**无**完整 focus trap。
- **状态：** `popover_is_open` / `set_popover_open`；`take_popover_open_change() -> Option<(WidgetId, bool)>`。
- **API：** `ui.popover(trigger, body, PopoverStyle)`；`popover_titled(trigger, title, body, style)`。
- **内容：** MVP 纯文本；无富 HTML / 任意 children builder。

### 8.2 `popconfirm`

- **语义：** 确认气泡——触发器 + 文案 + OK / Cancel。
- **动作：**
  - Confirm（OK）→ `take_popconfirm_action(Confirm)` 并关闭；
  - Cancel 按钮 → `Cancel` 并关闭；
  - Escape / 外侧点击 / 宿主再点关闭 → `Dismiss` 并关闭。
- **焦点：** 宿主进 Tab 序；Space/Enter 切换开合。
- **API：** `ui.popconfirm(trigger, message, PopconfirmStyle)`（默认 OK/Cancel）；`popconfirm_labeled(..., ok, cancel, style)`。
- **状态：** `popconfirm_is_open` / `set_popconfirm_open`；`take_popconfirm_action() -> Option<(WidgetId, PopconfirmAction)>`。

---

## 9. P2c 验收

| 项 | 通过样子 |
|----|----------|
| API | `popover` / `popover_titled` / `popconfirm` / `popconfirm_labeled` 可在 `FlexBuilder` 中声明 |
| Popover | 点击切换；外侧 / Esc 关闭；`take_popover_open_change` 可排空 |
| Popconfirm | OK→Confirm、Cancel→Cancel、Esc/外侧→Dismiss；`take_popconfirm_action` 可排空 |
| Overlay | 复用 `ensure_overlay_host` / `open_popup`；不另起浮层系统 |
| 测试 | `office_p2c_*` 绿；不依赖 DISPLAY |
| 示例 | `e26_office_p2` 含 P2c 区；`NEXT_GUI_P2C_QA=1` 可 dump；无 DISPLAY 不挂死 |
| 文档 | 本文 + catalog §9 + office-line / roadmap / README 同步 |


---

## 10. P2d 契约

### 10.1 `upload`

- **语义：** 路径/文件名展示 + Browse（可选 Clear）；**不内置原生文件对话框**。
- **Browse：** 发出 `take_upload_browse() -> Option<WidgetId>`；应用打开系统对话框或 demo stub，再 `set_upload_path`。
- **Clear：** 指针点 Clear 或 `take_upload_clear`；也可直接 `set_upload_path(None)`。
- **焦点：** Tab 可达；Space / Enter → Browse。
- **API：** `ui.upload(placeholder, UploadStyle)`；`upload_labeled(..., browse, clear, path, style)`；`upload_path` / `set_upload_path`。
- **文档：** OS 对话框为应用责任（或未来 platform helper）。

### 10.2 `color_picker`

- **语义：** Soft 友好——**预设色板行 + `#RRGGBB` 文本**（无 RGB 滑条）。
- **值：** `next_gui_scene::Color`；`take_color_change() -> Option<(WidgetId, Color)>`；`color_picker_value` / `set_color_picker_value`。
- **焦点：** Tab 可达；Left/Right（兼 Up/Down）切换色板；可键入 hex，Enter 提交。
- **API：** `ui.color_picker(value, ColorPickerStyle)`；`color_picker_with_presets(...)`；辅助 `color_to_hex` / `parse_hex_color` / `default_color_presets`。

### 10.3 `autocomplete`

- **语义：** 文本框 + 过滤建议下拉（复用 Select/`open_popup` 模式）。
- **过滤：** 当前值对选项做**大小写不敏感子串**匹配；空值时下拉可含全部选项（打开条件见实现）。
- **选取：** 点选 / Enter → 写入值、关闭、`take_autocomplete_change`；Escape 关闭。
- **焦点：** Tab 可达；Up/Down 列表；键入走类 TextInput 路径。
- **API：** `ui.autocomplete(options, value, AutocompleteStyle)`；`autocomplete_placeholder(...)`；`autocomplete_value` / `set_autocomplete_value`。

---

## 11. P2d 验收

| 项 | 通过样子 |
|----|----------|
| API | `upload` / `color_picker` / `autocomplete` 可在 `FlexBuilder` 中声明 |
| Upload | Browse → `take_upload_browse`；stub/`set_upload_path`；Clear → `take_upload_clear`；Space/Enter → Browse |
| Color | 色板/键盘改色；`take_color_change`；hex Enter 提交 |
| Autocomplete | 过滤；Up/Down/Enter/Esc；`take_autocomplete_change` |
| 测试 | `office_p2d_*` 绿；不依赖 DISPLAY |
| 示例 | `e26_office_p2` 含 P2d 区；`NEXT_GUI_P2D_QA=1` 可 dump；无 DISPLAY 不挂死 |
| 文档 | 本文 + catalog §9 + office-line / roadmap / README 同步 |
| 依赖 | 无 `rfd` 等重依赖；对话框 stub 可接受 |

---


---

## 12. P2e 契约

### 12.1 `grid`

- **语义：** 固定列数布局（CSS-grid-lite）；子项 L→R，满列换行。
- **API：** `ui.grid(columns, GridStyle, |ui| { … })` / `FlexBuilder::grid`（同 Column/Row 容器）。
- **非目标：** 响应式 12 列、`fr` 轨道、跨列 span。

### 12.2 `timeline`

- **语义：** 垂直事件流——时间/标签 + 正文；可选点色 `TimelineItem::color_rgb`。
- **焦点：** `TimelineStyle::clickable`（默认 true）时进 Tab；点行 → `take_timeline_select() -> Option<(WidgetId, usize)>`。
- **API：** `ui.timeline(items, TimelineStyle)`；`timeline_selected` / `set_timeline_selected`。

### 12.3 `calendar`

- **语义：** 月视图；**周起日 = Sunday（Sun–Sat）**；`year` / `month` / `selected_day`。
- **导航：** 头栏 ‹ › 换月；点日 / 方向键改日 → `take_calendar_change() -> Option<(WidgetId, CalendarDate)>`。
- **焦点：** Tab 可达；无完整日程事件系统。
- **API：** `ui.calendar(year, month, day, CalendarStyle)`；`calendar_date` / `set_calendar_date`。
- **accent：** 若 `UiConfig::accent` 已设，选中日色优先用它。

### 12.4 `image`

- **语义：** Soft 友好光栅预览。无 PNG 解码依赖。
- **路径：**
  - `image_rgba(w, h, &[u8], ImageStyle)` / `image_rgba_caption`——拥有 RGBA8（`ImagePixels`）；通过共享 scene `RgbaImage` / `BlitRgba` 在 Soft/GPU 绘制真实像素，保持宽高比并支持透明混合、裁剪与最近邻缩放；`mosaic_max` 仅保留源码兼容，不再生效；
  - `image_placeholder(caption, style)`——纯占位底 + 文案。
- **焦点：** 默认不进 Tab。

### 12.5 `config_provider`

- **语义：** **Ui 级**默认袋（非 Web Context 克隆；**无独立 visual Kind**）。
- **API：** `ui.set_config(UiConfig)` / `ui.config()`；字段 `locale`、`density`（Compact|Default|Comfortable）、可选 `accent`。
- **消费：** 本切片仅 Calendar（及 Timeline 选中点）在 trivial 处读 accent；不改写全部 P0/P1。主题 token 体系后置。

---

## 13. P2e 验收

| 项 | 通过样子 |
|----|----------|
| API | `grid` / `timeline` / `calendar` / `image_*` / `set_config` 可用 |
| Grid | 固定列换行；子项可测 L→R / wrap |
| Timeline | clickable 时 `take_timeline_select` 可排空 |
| Calendar | Sun–Sat；点日/箭头 → `take_calendar_change` |
| Image | RGBA 图像或 placeholder；不进焦序 |
| Config | `set_config` / `config`；无 Kind |
| 测试 | `office_p2e_*` 绿；不依赖 DISPLAY |
| 示例 | `e26_office_p2` 含 P2e；`NEXT_GUI_P2E_QA=1` 可 dump |
| 文档 | 本文 + catalog §9 + office-line / roadmap / README；**Office P2 齐**（Cascader 可选） |

---

## 14. 实现落点

- 样式 / 数据 / `UiConfig`：`crates/next-gui-ui/src/office_p2.rs`
- Kind / 交互 / paint / `Ui::set_config`：`crates/next-gui-ui/src/lib.rs`
- 示例：`examples/26_office_p2`

---

## 15. 修订记录

| 日期 | 说明 |
|------|------|
| 2026-10-19 | 初稿 + **P2a 落地**：button_group / link / space + `e26_office_p2`。 |
| 2026-10-19 | **P2b 落地**：steps / statistic / result；契约 §6–7；下一步 P2c。 |
| 2026-10-19 | **P2c 落地**：popover / popconfirm；契约 §8–9；下一步 P2d。 |
| 2026-10-19 | **P2d 落地**：upload / color_picker / autocomplete；契约 §10–11；下一步 P2e。 |
| 2026-10-19 | **P2e 落地**：grid / timeline / calendar / image / config_provider；**Office P2a–e 已齐**；Cascader 仍可选。 |
