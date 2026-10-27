# 无障碍基线（P6d）

Tree exposes `tree_expanded` (including hidden descendants), finite/clamped `tree_scroll_offset` / `set_tree_scroll_offset`, and `reveal_tree_selection`. Reveal expands only the selected node's ancestors and scrolls it into view without changing focus or selection. Hosts that rebuild the UI retain their own stable node identity, expansion and viewport state; e28 keys expansion by schema name to survive attachment reordering.

> **状态：已落地。** 对应 [p6-design.md](p6-design.md) §3.4。  
> **非目标：** 不做 AT-SPI / UI Automation / VoiceOver（或其它 OS 读屏总线）桥接；不做高对比主题整库重绘。

## 1. 焦点顺序

- **查询：** `Ui::focusable_widgets() -> Vec<WidgetId>`  
  稳定顺序：按 `Ui` **roots 声明顺序**，对每个根做 **DFS（先序）** 走容器 `children`（Column / Row / Toolbar / Panel / StatusBar / Splitter / ScrollArea / ContextMenu / Tabs / Badge / Tooltip / Form / Spin）。
- **参与种类：** 与指针按下已会 `set_focus` 的种类一致，并含 **Button**（`tool_button` 同为 `Kind::Button`）：  
  **Button** / TextInput / TextArea / Autocomplete / InputNumber / Checkbox / Radio / Switch / List / Tree / Table / Menu / Tabs / Select / TreeSelect / AlarmList / Slider / Segmented / Pagination / Collapse / DateTime / Transfer / PropertyGrid / TreeTable。
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

Autocomplete 复用 TextInput 的全选、删除、Home/End、字符输入路径，并支持共享
IME/剪贴板 API。组合文本带下划线、候选位置跟随可见光标；长值在字段内裁剪并跟随光标。
建议窗口按视口向上/下展开，方向键或滚轮翻动候选，Escape 关闭建议并保留编辑焦点。
`autocomplete_state` / `set_autocomplete_state` 保留光标、全选、组合和按文字定位的候选，
只恢复文字仍相同的控件；应用负责恢复焦点及转发 OS IME/剪贴板事件。
`delete_focused_text_selection` 在剪贴板写入成功后删除对应复制范围；单行控件删除整值，
多行控件仅删除选区，密码和只读保护仍生效。这些逻辑测试不代表 OS 输入验收。

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

### 1.7 可滚动页签

`TabsStyle.scrollable`（DSL 同名属性）启用按标题宽度排布及溢出箭头，默认仍等宽。
箭头只滚动标题栏；已有方向键、Home/End 选择后自动显示所选标题。
箭头不单独进入焦序，当前页签可访问名仍是完整标题。相同标题/选择/布局的
`TabsState` 可在重建时恢复手动滚动；不同宽度/字号重新显示所选标题。
此行为尚未桥接到 OS 读屏接口。

### 1.8 多行文本行号栏

`set_text_area_line_numbers` / DSL `line_numbers` 可选显示物理行号，默认关闭。
LF/CRLF、空行及末尾空行参与编号；自动换行的续行不重复编号。
行号不进入文档、复制、搜索或撤销内容；键盘和 IME 光标位置使用扣除行号栏后的布局。
切换显示或恢复到不同显示模式时重新显示光标并清除旧目标横坐标。
行号没有单独焦点，尚未桥接到 OS 读屏接口；行号栏整行选择与折叠仍未提供。

### 1.9 多行文本前景色范围

`set_text_area_foregrounds` 接受不重叠的 Unicode scalar 范围和颜色，非法范围整体拒绝。
着色不改变文档、复制、选区或撤销历史；选中文本用正常前景色保证可读性。
相同文档会话保留颜色，实际编辑/撤销/重做清除旧范围，IME 组合期间暂用正常颜色。
应用需在文档和主题变化后提供新范围；颜色不提供额外 OS 可访问语义。

### 1.10 多行文本补全

`show_text_area_completions` 复用主题菜单弹层，焦点留在编辑器。
上下键、PageUp/PageDown、Home/End 和滚轮可到达全部候选；Enter/Tab 或点击接受，
Escape 关闭，其他按键继续编辑。替换范围使用 Unicode scalar 偏移，接受后可一次撤销。
文档、光标、选区、焦点、只读或 IME 组合变化后拒绝旧候选插入；应用负责重新筛选候选。
长标签裁剪到行内，重建时弹层不保留。已有候选列表尚未桥接到 OS 读屏的活动候选语义。

### 1.11 多行文本提示

`set_text_area_hint` 以 Unicode scalar 位置定位被动提示，不抢焦点；点击和滚轮穿透到下层。
提示自动换行并限制在窗口内，Escape 关闭；同一位置/前缀的关闭状态随编辑会话恢复。
菜单、选区、失焦、只读、过期文档、离屏定位与 IME 组合会隐藏提示。
`text_area_hint` 提供当前有效文字，提示组件的可访问名也包含文字；尚无 OS 读屏提示播报桥接。

### 按词移动、选择和删除

TextArea 支持 `UiKey::WordLeft` / `WordRight`、`SelectWordLeft` /
`SelectWordRight` 和 `DeleteWordLeft` / `DeleteWordRight`。默认词类为
Unicode 字母/数字/下划线、空白、标点；词边界保留组合字符和 emoji 字素簇。
删除优先作用于已有选择，否则删除至对应词边界，并保留一次 Undo。
只读控件允许移动/选择并阻止删除；IME 组合期间这些命令不修改组合或文档。
e28 在 macOS 映射 Option，在 Windows/Linux 映射 Ctrl；Shift 扩展选择。
其他调用者可映射这些枚举值；下游穷尽匹配 `UiKey` 时需要增加相应分支。
普通逐字符移动和指针命中的完整字素支持仍未实现；这不是完整原生编辑器验收。


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

List exposes `list_scroll_offset`, finite/clamped `set_list_scroll_offset` and `reveal_list_selection`. Hosts can restore selection plus viewport after rebuild; explicit reveal scrolls only enough to show the selected row and keeps focus. Keyboard list navigation uses the same reveal path.
