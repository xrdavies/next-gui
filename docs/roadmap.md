# 路线图（P0–P6）

**实现语言已定为 Rust。** 声明式层为自研**紧凑 DSL** + Rust 宏/API（起步组件标记，后续再丰富表达力；见 [architecture.md](architecture.md)、[requirements.md](requirements.md)）。各阶段目标仍不绑死具体第三方 crate 名；模块按 `platform` / `scene` / `layout` / `render*` / `text` / `ui` / `immediate` / `profile` 推进。

平台实现顺序：**Linux → Windows → macOS（仅 Office）**。

**macOS Office 首版已实现：** `next-gui-platform::macos` 复用 Linux 的窗口、输入与软渲染契约；实现细节与已知缺口见 [platform-macos.md](platform-macos.md)。校验：`cargo check -p next-gui-platform --features macos`。

**Windows Soft 骨架（P6c）已实现：** `next-gui-platform::windows`；见 [platform-windows.md](platform-windows.md)。校验：`cargo check -p next-gui-platform --features windows`（Linux 宿主仅 typecheck；图形会话手工跟验）。

## P0 — 骨架

> **已完成骨架**（见 [p0-design.md](p0-design.md)、`examples/01_soft_window`）。

- 窗口创建与基础生命周期（`platform`）；**先做 Linux 后端**
- 输入：**指针 + 键盘**；触控为 `Event::Touch` stub（完整映射 P6）
- 剪贴板：UTF-8 文本 API（`WindowCtx::clipboard_get_text` / `clipboard_set_text`）
- retained 矩形树（最小 `scene` 节点模型）
- CPU **软渲染**路径（`render` + `render-soft`）
- **脏区**重绘（非每帧全屏无脑刷）

验收：空窗 + 若干矩形控件可交互，弱机可跑，变更区域可观测。

运行：`cargo run -p e01_soft_window`（需 Linux/macOS 图形桌面会话）。

## P1 — 声明式

> **已完成**（见 [p1-design.md](p1-design.md)、`examples/02_counter`、`examples/03_ngui_counter`）。

- **紧凑 Rust API / builder**（`ui`）；结构短，复杂逻辑在 Rust
- **独立 `.ngui` 文件 DSL**：`Column` / `Text` / `Button`，`@name` 取 `WidgetId`；`load_ngui` / `load_ngui_path` 映射到同一套 `ui`
- **简单 layout**（`layout`）：Column / Row flex 放置
- 基础控件：**Text**、**Button**（列表 / TextInput 后补）
- 点击 → 更新状态 → `set_text` / `sync`；仍软渲染 + 脏区
- P1 Text 用内置点阵字占位；真正 shaping / CJK 见 P2

验收：用紧凑声明式（Rust API 或 `.ngui`）描述即可搭出简单表单（如计数器），无需手写每帧绘制指令。

运行：

```bash
cargo run -p e02_counter          # Rust API
cargo run -p e03_ngui_counter     # .ngui 文件
```

（需 Linux/macOS 图形桌面会话）。

## P2 — 文本

> **已完成骨架**（见 [p2-design.md](p2-design.md)、`examples/04_cjk_text`）。

- 字体加载与度量（`next-gui-text`）
- 简单换行 / 行布局；栅格化为 alpha mask
- **CJK**（系统 Noto/PingFang/雅黑等；工业界面刚需）
- `ui` Text/Button 去掉 P1 点阵占位；软渲染 `BlitAlpha`
- 说明：栅格阶段性用 `fontdue`；复杂 shaping / IME 编辑后补

验收：中英混排可读；度量/布局 API 可用（完整可编辑控件后补）。

运行：`cargo run -p e04_cjk_text`（需图形会话 + CJK 字体，如 `fonts-noto-cjk`）。

## P3 — GPU 后端

> **已完成骨架**（见 [p3-design.md](p3-design.md)、`examples/05_backend_toggle`）。

- 与软渲染**共用场景图 / `DrawCommand`**（`next-gui-render-gpu`）
- Linux 上经 **wgpu** 离屏栅格 + CPU readback，再走现有 `platform.present`（softbuffer）；**原生 Surface present** 已在 Office 线落地（见 [office-line.md](office-line.md)）；P3 示例 `e05` 仍默认 readback
- GPU **服务观感与动态内容**（流畅重绘、动画、图表），非炫技
- 无适配器时 `GpuRenderer::try_new` 失败，应用回退 Soft
- Office 默认可走 GPU、Field 可关：配置层见 **P4**

验收：同一 UI 树可在软渲染与 GPU 间切换，视觉大体一致（允许抗锯齿等实现差）；无 GPU 亦可启动。

运行：`cargo run -p e05_backend_toggle`（需图形会话；有 GPU 时 Space 切换 Soft/GPU）。

## Office 线（P3 之后、P4 之前）— **已完成首轮加厚**

> 曾跳过 P4 先做厚 Office；设计见 [office-line.md](office-line.md)。**P4 Profile 现已落地**（见下）。

- 布局：`CrossAlign`；控件：`Row` / `Spacer` / `Separator`（工具条）
- GPU：**原生 `wgpu::Surface` present 已落地**（`PresentBackend::GpuSurface` + `GpuRenderer::try_new_native`）；Softbuffer/readback 仍保留
- Soft 始终可回退；示例 `cargo run -p e10_office_panel`
- 控件目录：[office-widget-catalog.md](office-widget-catalog.md)
- **P0 / P1（含 rest + deferred）已齐**；Chart 多图 + 键盘打磨见 `e23_office_charts`。Cascader 仍不进 P1
- **Office P2a** ✅（`button_group` / `link` / `space`；[office-p2-design.md](office-p2-design.md)、`e26`）；下一步 P2b；Cascader 仍可选
- 后续可选：Office P2b+、控件进 `.ngui`、自绘 IME 候选

## P4 — 双档 Profile

> **已完成骨架**（见 [p4-design.md](p4-design.md)、`examples/24_profile_dual`）。

- Field / Office 特性开关与性能预算（`next-gui-profile`）
- 降级策略：`degrade` → 强制 soft、关 GPU/特效、降 fps
- 打包意图：Cargo features `field` / `office` + 文档；运行时 `ProfileConfig` 为准
- 验收：同一应用源两档配置；Field 不依赖 GPU 即可 Soft 稳跑

运行：

```bash
cargo run -p e24_profile_dual
NEXT_GUI_PROFILE=field cargo run -p e24_profile_dual
NEXT_GUI_PROFILE_QA=1 cargo run -p e24_profile_dual
```

## P5 — Immediate 视口

**状态：骨架已落地**（见 [p5-design.md](p5-design.md)）。

- ~~显式 `immediate { }`（或等价 API）区域（`immediate`）~~ → `next-gui-immediate`：`ImmediateViewport` / `begin_frame`
- ~~支持局部嵌入；保留整窗 Immediate 能力但不作 Field 默认~~ → 局部 mount；Field `allow_immediate=false`
- ~~输入焦点、裁剪、刷新边界清晰~~ → contains/focus/Esc；clip 绘制与命中；`Scene::mark_dirty` 视口脏区

验收：retained 主界面中可嵌一块 Immediate 工具面板且互不抢事件 — **e25**。

```bash
cargo run -p e25_immediate_viewport
NEXT_GUI_IMMEDIATE_QA=1 cargo run -p e25_immediate_viewport
```

## P6 — 工业打磨

> **P6a–P6e 已通**（见 [p6-design.md](p6-design.md)、[a11y-baseline.md](a11y-baseline.md)、[soak.md](soak.md)）。切片：**P6a** ✅ 触控 HMI → **P6b** ✅ DPI → **P6c** ✅ Windows Soft 骨架 → **P6d** ✅ a11y 基线 → **P6e** ✅ 7×24/soak；Windows / macOS Retina 运行时手工跟验为跟进项。

- **触控** HMI：`Event::Touch` 保留；P6a 主触点合成 `PointerMoved` / `PointerButton`（`touch_synth`）；工控完整键表后补（`KeyCode::Other` 为扩展钩）
- 无障碍基线（P6d ✅：焦序 + `accessible_name`；Button 进焦序 + `take_button_activate`；无 AT 总线）
- 7×24 稳定性（泄漏、长时间运行；P6e ✅：`NEXT_GUI_SOAK` + [soak.md](soak.md)）
- 多分辨率 / DPI（P6b ✅：`scale_factor` + `dpi` 助手；契约默认物理）
- **Windows** 平台后端（P6c ✅ Soft 骨架；运行时手工跟验）；macOS Office 首版已完成；DPI 契约已通，Retina 混缩手工验 / IME 打磨仍为跟进

验收：面向现场与办公室的真实场景试点可长期运行；OS 按序补齐。各切片验收口径见 [p6-design.md](p6-design.md)。

## 建议顺序

P0–P6 已通（**P6a–P6e**；见 [p6-design.md](p6-design.md)、[soak.md](soak.md)）。跟进：Windows 图形会话运行时 GUI 验、macOS Retina 混缩手工验。**Button 进焦序** ✅（`take_button_activate`；见 [a11y-baseline.md](a11y-baseline.md)）。**Office P2a** ✅（见 [office-p2-design.md](office-p2-design.md)）；下一步 P2b。不要丢掉 Field Soft 兜底；也不要第一天做满血 QML 式语言。可选并行：Office P2b+、控件进 `.ngui`、自绘 IME 候选。
