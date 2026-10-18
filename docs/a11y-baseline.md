# 无障碍基线（P6d）

> **状态：已落地。** 对应 [p6-design.md](p6-design.md) §3.4。  
> **非目标：** 不做 AT-SPI / UI Automation / VoiceOver（或其它 OS 读屏总线）桥接；不做高对比主题整库重绘。

## 1. 焦点顺序

- **查询：** `Ui::focusable_widgets() -> Vec<WidgetId>`  
  稳定顺序：按 `Ui` **roots 声明顺序**，对每个根做 **DFS（先序）** 走容器 `children`（Column / Row / Toolbar / Panel / StatusBar / Splitter / ScrollArea / ContextMenu / Tabs / Badge / Tooltip / Form / Spin）。
- **参与种类：** 与指针按下已会 `set_focus` 的种类一致：  
  TextInput / TextArea / InputNumber / Checkbox / Radio / Switch / List / Tree / Table / Menu / Tabs / Select / TreeSelect / AlarmList / Slider / Segmented / Pagination / Collapse / DateTime / Transfer / PropertyGrid / TreeTable。  
  **不含** Button（指针点击会清焦点）、Tag / Alert / Breadcrumb / ScrollArea 等。
- **移动：** `focus_next` / `focus_prev`（环绕；无焦点时 next→首、prev→末）。
- **键盘：** `UiKey::Tab` / `UiKey::BackTab`（Shift+Tab）在 `key_input` **顶层**处理——不要求事先已有焦点；文本框不插入 Tab 字符。打开的 Modal / Drawer 在焦点落在其上时 **吸收** Tab/BackTab（焦点陷阱）。

示例映射（见 `examples/12_office_inputs` 等）：`KeyCode::Tab` + `modifiers.shift` → `UiKey::BackTab`。

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
- `focusable_order_and_tab_wrap`
- `key_input_tab_moves_focus_without_prior_focus`
- `key_input_tab_absorbed_by_modal_trap`

```bash
cargo test -p next-gui-ui --lib accessible_name_override
cargo test -p next-gui-ui --lib focusable_order
cargo test -p next-gui-ui --lib key_input_tab
```

## 4. 后续（非 P6d）

完整读屏遍历、平台 a11y 树同步、Button 纳入焦序（若产品需要与指针行为解耦）→ 另开阶段。
