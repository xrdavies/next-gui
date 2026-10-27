# 无障碍基线（P6d）

> **状态：已落地。** 对应 [p6-design.md](p6-design.md) §3.4。  
> **非目标：** 不做 AT-SPI / UI Automation / VoiceOver（或其它 OS 读屏总线）桥接；不做高对比主题整库重绘。

## 1. 焦点顺序

- **查询：** `Ui::focusable_widgets() -> Vec<WidgetId>`  
  稳定顺序：按 `Ui` **roots 声明顺序**，对每个根做 **DFS（先序）** 走容器 `children`（Column / Row / Toolbar / Panel / StatusBar / Splitter / ScrollArea / ContextMenu / Tabs / Badge / Tooltip / Form / Spin）。
- **参与种类：** 与指针按下已会 `set_focus` 的种类一致，并含 **Button**（`tool_button` 同为 `Kind::Button`）：  
  **Button** / TextInput / TextArea / InputNumber / Checkbox / Radio / Switch / List / Tree / Table / Menu / Tabs / Select / TreeSelect / AlarmList / Slider / Segmented / Pagination / Collapse / DateTime / Transfer / PropertyGrid / TreeTable。  
  **不含** Tag / Alert / Breadcrumb / ScrollArea 等。
- **移动：** `focus_next` / `focus_prev`（环绕；无焦点时 next→首、prev→末）。
- **键盘：** `UiKey::Tab` / `UiKey::BackTab`（Shift+Tab）在 `key_input` **顶层**处理——不要求事先已有焦点；文本框不插入 Tab 字符。打开的 Modal / Drawer 在焦点落在其上时 **吸收** Tab/BackTab（焦点陷阱）。

示例映射（见 `examples/12_office_inputs` 等）：`KeyCode::Tab` + `modifiers.shift` → `UiKey::BackTab`。

### 1.1 Button：焦序 + 双路径激活

| 路径 | 行为 |
|------|------|
| **指针** | `pointer_pressed` 在 Button 上 **设焦点** 并返回 `false`；应用仍用 `hit_widget` 处理点击语义（与改前一致）。 |
| **键盘** | 焦点在 Button 时 `Space` / `Enter` → `key_input` 返回 true，并记下待激活 id；应用调用 `Ui::take_button_activate() -> Option<WidgetId>`（一次性 drain，类似 `take_menu_action`）。 |
| **视觉** | Button 绘制暂无独立 focus 边框（避免重 chrome）；焦点仍参与焦序与键盘激活。 |

`examples/02_counter`（及同类 Soft demo）在 `key_input` 之后 drain `take_button_activate`，与 `hit_widget` 点击共用同一套按钮逻辑。

按钮默认启用。`set_button_enabled(id, false)` 从焦序中移除该按钮，拒绝直接设焦点，
清除其焦点和待处理的键盘激活；指针按下返回 true 消费事件，并保留其他控件的焦点。
`button_enabled(id)` 可供应用处理直接调用的动作；`hit_widget` 仍能命中禁用按钮，
因此应用应先检查 `pointer_pressed` 的消费结果或查询启用状态。`sync` 后标签使用
Theme 的 `disabled`、背景使用 `muted`，重新启用恢复原有样式。此状态尚未桥接到 OS 读屏接口。

### 1.2 只读多行文本

`Ui::set_text_area_read_only(id, true)` 允许焦点、导航、选择和复制，阻止输入、
粘贴、删除、撤销/重做与 IME 修改。`text_input_wants_ime()` 对只读文本区返回 false；
设置只读会清除未提交的组合文本。程序调用 `set_text_area_value` 仍可更新内容。
现有 TextArea 默认可编辑，编辑历史在切换只读期间保留。这是共享控件行为，
不表示已向 OS 暴露只读可访问状态。

### 1.3 多处文本高亮

`Ui::set_text_area_highlights(id, ranges, color)` 接受 Unicode scalar 范围，
保留选区与撤销历史，支持空范围的位置标记；非法范围会整体拒绝。
不相邻的匹配分别显示，高亮随相同文档的 `TextAreaState` 恢复，实际编辑、
撤销/重做或程序替换内容后清除。组合输入期间暂不显示旧范围。
颜色之外的结果计数与当前匹配说明由应用提供；这不是 OS 可访问搜索结果接口。

### 1.4 可撤销范围替换

`Ui::replace_text_area_range(id, range, text)` 使用 Unicode scalar 范围，
在不预先改变选区的情况下记录一次编辑，Undo 恢复原光标和选区方向。
非法范围、错误控件和只读文本区返回 false 且不改变编辑会话。
`replace_text_area_selection` 复用同一实现；相同文档的 `TextAreaState`
可保留范围编辑的撤销/重做历史。实际修改清除旧高亮和组合文本并显示新光标。

### 1.5 多行文本分页导航

TextArea 的 PageUp/PageDown 同时移动光标和视口，步长为可见完整视觉行数减一，
至少一行。Shift+PageUp/PageDown 从固定锚点扩展选区；自动换行、只读文档、
空文档和极小视口也处理。键盘直接按最近布局行定位，避免小数字号的向下取整偏移；
鼠标仍按指向的视觉行定位。跨短行保留目标横坐标，原生键盘/读屏验收未完成。

### 1.6 垂直导航的目标列

Up/Down、分页及其 Shift 扩选在短行后继续使用原目标横坐标；相同布局的
`TextAreaState` 恢复也保留它。明确选择/范围替换、点击或拖动、横向移动、
实际撤销/重做会重置目标。恢复时宽度或字号变化会清除旧像素目标，
字号变化同时请求显示光标；只改变高度保留目标横坐标。

## 2. 可访问名

| API | 说明 |
|-----|------|
| `set_accessible_name(id, name)` | 覆盖名；空串等同清除 |
| `clear_accessible_name(id)` | 清除覆盖 |
| `accessible_name(id) -> Option<String>` | 有覆盖用覆盖；否则 Kind 尽力回退 |

**回退（非空时）：** Button / Checkbox / Switch / Radio / Tag / Gauge → `label`；Panel / Menu / Modal / Drawer / Card → `title`；TextInput / TextArea → 当前 `value`，否则 `placeholder`；Select → 选中项或 `placeholder`；TreeSelect → `placeholder`；Alert → `message`；Empty → `description`；Tabs / Segmented → 当前标题/选项；Text → `content`。其余 Kind 可为 `None`。

应用层可据此做自测、日志或后续总线适配；**本阶段不**向 OS 推送 a11y 树。

## 3. 验收与单测

- `accessible_name_override_and_button_fallback`
- `focusable_order_and_tab_wrap`（含 Button）
- `button_tab_reach_and_keyboard_activate`
- `pointer_on_button_sets_focus_but_returns_false`
- `key_input_tab_moves_focus_without_prior_focus`
- `key_input_tab_absorbed_by_modal_trap`

```bash
cargo test -p next-gui-ui --lib accessible_name_override
cargo test -p next-gui-ui --lib focusable_order
cargo test -p next-gui-ui --lib button_tab_reach
cargo test -p next-gui-ui --lib pointer_on_button
cargo test -p next-gui-ui --lib key_input_tab
```

## 4. 后续（非 P6d）

完整读屏遍历、平台 a11y 树同步 → 另开阶段。Button 进焦序与键盘激活已在本基线上完成（见上 §1.1）。

## Table page navigation

`UiKey::PageUp` / `PageDown` moves a focused Table by its currently visible row
count minus one, with a minimum step of one. `SelectPageUp` / `SelectPageDown`
keeps the original anchor while extending/reversing the range. Active columns
and frozen columns are preserved, and the destination is revealed. Empty tables
remain unselected. This operates on rows supplied to the widget; applications
with virtual data must load further rows themselves (e28 plain PageUp/PageDown
uses its guarded full-result navigation). This is keyboard behavior, not an OS
accessibility-tree bridge or native input acceptance.

## Menu and context popup navigation

A focused Menu opens with Enter, Space or Down. An open Menu/ContextMenu consumes
keys before its underlying widget: Up/Down wrap across enabled items, Home/End
select the first/last enabled item, and Enter/Space queues `take_menu_action`.
Escape closes; Tab/BackTab closes and continues focus traversal. Disabled entries
cannot be activated. Menu hover uses the same highlight as keyboard navigation;
`MenuStyle` and `ContextMenuStyle` expose `hover_fg` (theme `on_accent`) for
legible highlighted labels. External exhaustive style literals need the new field;
existing `..Default::default()` / theme constructors keep working. Applications must forward pointer motion to `Ui::pointer_hover` and sync on change.

`Ui::context_menu_popup(style)` declares a popup without a layout host.
`open_context_menu(id, items, x, y)` replaces its items and opens it, rejecting
nonfinite coordinates, empty items and wrong widget IDs. It keeps the underlying
focus; `has_open_popup` lets application shortcuts respect popup ownership.
Context panels that fit the viewport are clamped to its edges. Very tall menu
scrolling and a native accessibility bridge remain unimplemented.

`table_header_column_at(scene, id, x, y)` shares resize/scroll/freeze geometry with
Table painting and sorting. Applications check `hit_widget` for occlusion first.
`set_table_active_cell` moves the active cell within an existing selection while
retaining its range/anchors; e28 uses it to retain an entire-column selection
across rebuilds. These are synthetic widget contracts, not native input acceptance.
