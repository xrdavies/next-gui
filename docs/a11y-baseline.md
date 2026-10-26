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
