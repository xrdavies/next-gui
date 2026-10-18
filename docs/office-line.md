# Office 线：办公桌面能力加厚（相对 Field Soft）

> **状态：首轮加厚已完成；P4 Profile + P5 Immediate 已落地**（见 [p4-design.md](p4-design.md)、[p5-design.md](p5-design.md)、`e24` / `e25`）。
>
> 历史决策：曾跳过 P4 先做厚 Office（GPU present / 布局 / 办公控件）；Soft 始终可回退。现 Profile 配置层已抽出。

---

## 1. 目标

Office 档面向规划 / 设计人员桌面（Linux → Windows → macOS Office）：

1. **GPU 优先观感**：流畅重绘、工具条与面板布局；无适配器或初始化失败时 **Soft 兜底**。
2. **布局更像办公软件**：横向工具条（Row）、占位（Spacer）、分隔线（Separator）、交叉轴对齐。
3. **同一套 `ui` / `scene` / `DrawCommand`**：不另起 Office 专用场景图。

非目标 / 已划出本线：

- ~~Field/Office **Profile 配置 crate**~~ → **P4 已落地**（`next-gui-profile`）
- ~~Immediate 视口（P5）~~ → **已落地**（`next-gui-immediate` / e25）
- P0 反馈控件 Tag/Badge/Tooltip/Modal/Progress/Alert 已落地；TextInput IME/CJK 预编辑+提交已接（候选窗靠系统 IME）
- ~~图表控件~~ → **Chart 多瓦片已落地**（`e23_office_charts`；Gauge/Trend 共享软光栅）
- 仍后置：动画系统、自绘 IME 候选窗、Office P2e+ 低频控件、Cascader（不进 P1）

---

## 2. GPU present：原生 Surface（已落地）

### 2.1 Office 路径（当前默认，有适配器时）

```text
encode_scene → DrawCommand[] → wgpu 离屏 retained 纹理
                              → blit → wgpu::Surface::present → OS 窗口
```

无 CPU readback。`GpuRenderer::present_path()` → [`GpuPresentPath::NativeSurface`]。

前提：窗口以 [`PresentBackend::GpuSurface`] 创建（**不**附着 softbuffer），再在 `on_start` 里
`GpuRenderer::try_new_native(winit_window, …)`。

### 2.2 Softbuffer / readback 路径（仍保留）

```text
encode_scene → DrawCommand[] → wgpu 离屏 → CPU readback (XRGB)
                                          → platform.present (softbuffer)
```

或纯 Soft：`SoftRenderer` → `platform.present`。

`PresentBackend::Softbuffer` 建窗；`GpuRenderer::try_new` → `GpuPresentPath::ReadbackSoftbuffer`。
P3 示例 `e05_backend_toggle` 仍走此路径。

### 2.3 Soft ↔ GPU 在 GpuSurface 窗口上

| 模式 | 做法 |
|------|------|
| GPU | `GpuRenderer::render` 内部已 Surface present |
| Soft | `SoftRenderer` 出 XRGB → `GpuRenderer::present_xrgb_frame` 上传并 blit 到同一 Surface |

不在同一窗口同时挂 softbuffer 与 wgpu Surface。

### 2.4 平台契约

| API | 说明 |
|-----|------|
| `WindowAttrs::with_gpu_surface()` | 建窗不创建 softbuffer |
| `WindowCtx::winit_window()` | `Arc<winit::Window>`，供 `try_new_native` |
| `WindowCtx::present` | 仅 Softbuffer；GpuSurface 上调用会报错 |
| `GpuRenderer::try_new_native` | 创建 Surface + 离屏 retained + copy pipeline |
| `GpuRenderer::present_xrgb_frame` | Soft-on-Surface 上传 present |

---

## 3. 布局与控件（本阶段交付）

| 能力 | 模块 | 说明 |
|------|------|------|
| `CrossAlign` | `layout` | Start / Center / End（交叉轴） |
| `Row` | `ui` + `.ngui` | 横向 flex 容器（工具条基础） |
| `Spacer` | `ui` + `.ngui` | 固定宽高占位 |
| `Separator` | `ui` + `.ngui` | 细分割线 |
| `Icon` | `ui` | 内置命名字形（工具条/状态栏） |
| `Toolbar` | `ui` | **一等公民**横向工具条 |
| `Panel` | `ui` | **一等公民**带标题内容容器 |
| `StatusBar` | `ui` | **一等公民**底栏分区 |
| `Splitter` | `ui` | 左右/上下拖拽分隔（指针拖 sash） |
| Soft 回退 | 示例 | GPU 失败或切换时仍可 Soft |
| Native Surface | `render-gpu` + `platform` | Office 默认（有适配器时） |
| `ScrollArea` | `ui` | 视口裁剪 + 滚轮/拖拽滚动 |
| `TextInput` | `ui` | 单行录入；IME preedit/commit（CJK）；系统候选窗 |
| `Checkbox` / `Radio` / `Switch` | `ui` | 点击/Space 切换；Radio 分组互斥 |
| `List` | `ui` | 垂直项 + 单选 + ↑↓；内建滚动 |
| `Tree` | `ui` | 层级展开/折叠 + 单选；厂站/设备树 MVP |
| `Table` | `ui` | 表头 + 文本单元格 + 行选；列宽拖拽 stub；无编辑 |
| `Menu` | `ui` | 标题按钮 + 下拉项；`take_menu_action` |
| `ContextMenu` | `ui` | 宿主区域右键弹层；同 MenuItem 模型 |
| `Tabs` | `ui` | 标签条 + 内容页；点击 / ←→ |
| `Select` | `ui` | 单选下拉（combobox-lite） |
| `Slider` | `ui` | 水平滑块；拖拽 / 点击轨道 / ←→ |
| `Tag` | `ui` | 状态片；可选关闭 |
| `Badge` | `ui` | 计数/圆点；可宿主叠加 |
| `Tooltip` | `ui` | 悬停提示（即时显示 MVP） |
| `Modal` | `ui` | 居中对话框 + 遮罩；Esc/×/按钮 |
| `Progress` | `ui` | 确定进度条 0..=1；不确定为 stub |
| `Alert` | `ui` | 页内横幅；可关闭 |
| `TreeSelect` | `ui` | **P1** 厂站/设备树选择（Cascader 仍后置） |
| `Form` + `FormRule` | `ui` | **P1** 表单容器 + Required/Min/Max/Digits 校验 |
| `AlarmList` | `ui` | **P1** 告警列表；分级色 + 未确认闪烁 + ack |
| `Gauge` | `ui` | **P1** 仪表盘（软光栅弧 + 指针） |
| `Trend` | `ui` | **P1** 趋势/迷你折线（软光栅） |
| `Chart` | `ui` | **增强** 多图面板（Gauge/Trend 瓦片网格；不抬 P0） |
| `Drawer` | `ui` | **P1** 侧滑抽屉 + Backdrop；Esc/×/遮罩关闭 |
| `VirtualList` / `VirtualTable` | `ui` | **P1** 窗口化虚表（`virtual_list` / `virtual_table`） |
| `TextArea` / `InputNumber` | `ui` | **P1** 多行录入 / 数字步进 |
| `DateTime` | `ui` | **P1** 日期时间选择（日历网格 + 时分；Confirm/Enter） |
| `Transfer` | `ui` | **P1** 穿梭框双列表批量选择 |
| `PropertyGrid` | `ui` | **P1** 属性网格键值编辑 |
| `TreeTable` | `ui` | **P1** 树表（层级列 + 数据列） |
| `Notification` | `ui` | **P1** 全局 toast 队列 |
| `Spin` / `Empty` | `ui` | **P1** 加载遮罩 / 空状态 |
| `Collapse` / `Descriptions` / `Card` | `ui` | **P1** 折叠 / 描述列表 / 卡片 |
| `Segmented` / `Breadcrumb` / `Pagination` | `ui` | **P1** 分段 / 面包屑 / 分页 |

示例：

- `examples/10_office_panel` — 早期 Row 工具条 + Soft/GPU 切换
- `examples/11_office_shell` — **P0 shell**：Toolbar + Icon + Panel + Splitter + StatusBar
- `examples/12_office_inputs` — **P0 input**：ScrollArea + TextInput + Checkbox + Radio + Switch
- `examples/13_office_data` — **P0 data**：List + Tree + Table
- `examples/14_office_nav` — **P0 nav**：Menu + ContextMenu + Tabs + Select + Slider
- `examples/15_office_feedback` — **P0 feedback**：Tag + Badge + Tooltip + Modal + Progress + Alert
- `examples/20_office_industrial` — **P1 核心**：TreeSelect + Form 校验 + AlarmList + Gauge + Trend
- `examples/21_office_p1_rest` — **P1 rest**：Drawer / VirtualList·Table / TextArea / InputNumber / Notification / Spin / Empty / Collapse / Descriptions / Card / Segmented / Breadcrumb / Pagination
- `examples/22_office_p1_deferred` — **P1 deferred**：DateTime / Transfer / PropertyGrid / TreeTable
- `examples/23_office_charts` — **增强**：多瓦片 Chart + 独立 Gauge/Trend
- `examples/24_profile_dual` — **P4**：Field/Office Profile 双档
- `examples/25_immediate_viewport` — **P5**：retained 壳 + Immediate 工具面板
- `examples/26_office_p2` — **Office P2a–P2d**：button_group / link / space / steps / statistic / result / popover / popconfirm / upload / color_picker / autocomplete

Rust API 优先；复杂控件暂未进 `.ngui`。**P0 齐；P1（含 rest + deferred）齐；Chart；IME/CJK；剪贴板文本 API；P6a 触控→指针；P6b DPI 契约；P6c Windows Soft 骨架；P6d 无障碍基线；P6e soak；P4 Profile；P5 Immediate 已落地**。评测队列已清。下一步：P6 切片已齐（含 [soak.md](soak.md)）；**Button 进焦序** ✅；可选 Office P2 / `.ngui` 进复杂控件；跟进 Windows 运行时 / Retina 手工验。

---

## 3.1 控件目录（已确认）

完整对照表与优先级见 **[office-widget-catalog.md](office-widget-catalog.md)**（Ant Design / Element Plus / MUI + 工业扩展；**2026-10-17 已拍板**）。

**P0 目录控件（Rust API）已按 §4 / §10 落地。** P1 核心 / rest / deferred（含 Form 校验、`alarm_list` / `gauge` / `trend`、DateTime 等）均已落地；层级选择仅 `tree_select`（Cascader 仍不进 P1）。**Office P2a–P2d**（含 `upload` / `color_picker` / `autocomplete`）已落地，见 [office-p2-design.md](office-p2-design.md)；下一步 P2e。

---

## 4. 与路线图的关系

```text
P0–P3 完成
    ↓
【已完成】Office 线加厚（本文） + P4 Profile
    · 布局/工具条控件 ✓
    · 原生 Surface present ✓
    · 控件目录（已确认）：[office-widget-catalog.md](office-widget-catalog.md)
    · P0/P1 齐；Chart 多图增强 + P1 键盘打磨已落地；IME/CJK 已接
    ↓
P4 Profile（Field/Office 配置）已抽 → `next-gui-profile` / e24
    ↓
P5 Immediate 已落地；**P6a–P6e** 已通（[p6-design.md](p6-design.md)、[soak.md](soak.md)）
    ↓
Office **P2a–P2d** 已落地（含 `upload` / `color_picker` / `autocomplete`；[office-p2-design.md](office-p2-design.md)、`e26`）；下一步 **P2e**
```

---

## 5. 验收

| 项 | 通过样子 |
|----|----------|
| 文档 | 本文写明 Surface 路径与 Softbuffer 回退 |
| 布局 | `CrossAlign` + Row 工具条可测 |
| 控件 | Spacer / Separator 出现在场景中 |
| Soft | Softbuffer 窗或 Surface blit 可跑 |
| GPU native | 有适配器时 `present=NativeSurface`（见 stderr） |
| GPU readback | Softbuffer 窗上 `try_new` 仍可用 |
| 路线图 | P4 已完成（见 p4-design）；指向本文 |
| **视觉自测** | 新控件/示例合入前须跑相关 example，核对照面布局与交互是否符合预期；有桌面时截图复查。仅编译/单测不算 UI 完成（见 [AGENTS.md](../AGENTS.md) Git Workflow / QA） |

运行：`cargo run -p e10_office_panel`（需图形会话；CJK 字体同 P2）。
有适配器时应看到：`native Surface ready (…, present=NativeSurface)`。
Office / 后续示例：`e11`–`e15`（P0）、`e20`–`e22`（P1）、`e23`（Chart）、`e24`（Profile）、`e25`（Immediate）、`e26`（Office P2a–P2d）。
