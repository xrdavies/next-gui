# 路线图（P0–P6）

**实现语言已定为 Rust。** 声明式层为自研**紧凑 DSL** + Rust 宏/API（起步组件标记，后续再丰富表达力；见 [architecture.md](architecture.md)、[requirements.md](requirements.md)）。各阶段目标仍不绑死具体第三方 crate 名；模块按 `platform` / `scene` / `layout` / `render*` / `text` / `ui` / `immediate` / `profile` 推进。

平台实现顺序：**Linux → Windows → macOS（仅 Office）**。

**macOS Office 首版已实现：** `next-gui-platform::macos` 复用 Linux 的窗口、输入与软渲染契约；实现细节与已知缺口见 [platform-macos.md](platform-macos.md)。校验：`cargo check -p next-gui-platform --features macos`。

## P0 — 骨架

> **已完成骨架**（见 [p0-design.md](p0-design.md)、`examples/p0_soft_window`）。

- 窗口创建与基础生命周期（`platform`）；**先做 Linux 后端**
- 输入：**指针 + 键盘**（触控接口预留，实现后补）
- retained 矩形树（最小 `scene` 节点模型）
- CPU **软渲染**路径（`render` + `render-soft`）
- **脏区**重绘（非每帧全屏无脑刷）

验收：空窗 + 若干矩形控件可交互，弱机可跑，变更区域可观测。

运行：`cargo run -p p0_soft_window`（需 Linux/macOS 图形桌面会话）。

## P1 — 声明式

> **已完成**（见 [p1-design.md](p1-design.md)、`examples/p1_counter`、`examples/p1_ngui_counter`）。

- **紧凑 Rust API / builder**（`ui`）；结构短，复杂逻辑在 Rust
- **独立 `.ngui` 文件 DSL**：`Column` / `Text` / `Button`，`@name` 取 `WidgetId`；`load_ngui` / `load_ngui_path` 映射到同一套 `ui`
- **简单 layout**（`layout`）：Column / Row flex 放置
- 基础控件：**Text**、**Button**（列表 / TextInput 后补）
- 点击 → 更新状态 → `set_text` / `sync`；仍软渲染 + 脏区
- P1 Text 用内置点阵字占位；真正 shaping / CJK 见 P2

验收：用紧凑声明式（Rust API 或 `.ngui`）描述即可搭出简单表单（如计数器），无需手写每帧绘制指令。

运行：

```bash
cargo run -p p1_counter          # Rust API
cargo run -p p1_ngui_counter     # .ngui 文件
```

（需 Linux/macOS 图形桌面会话）。

## P2 — 文本

> **已完成骨架**（见 [p2-design.md](p2-design.md)、`examples/p2_cjk_text`）。

- 字体加载与度量（`next-gui-text`）
- 简单换行 / 行布局；栅格化为 alpha mask
- **CJK**（系统 Noto/PingFang/雅黑等；工业界面刚需）
- `ui` Text/Button 去掉 P1 点阵占位；软渲染 `BlitAlpha`
- 说明：栅格阶段性用 `fontdue`；复杂 shaping / IME 编辑后补

验收：中英混排可读；度量/布局 API 可用（完整可编辑控件后补）。

运行：`cargo run -p p2_cjk_text`（需图形会话 + CJK 字体，如 `fonts-noto-cjk`）。

## P3 — GPU 后端

- 与软渲染**共用场景图**（`render-gpu`）
- 合批 / 命令录制对接系统图形 API
- GPU **服务观感与动态内容**（流畅重绘、动画、图表），非炫技
- Office 可默认走 GPU；Field 可关或自动降级

验收：同一 UI 树可在软渲染与 GPU 间切换，视觉大体一致（允许抗锯齿等实现差）；无 GPU 亦可启动。

## P4 — 双档 Profile

- Field / Office 特性开关与性能预算（`profile`）
- 最低机型清单与降级策略（停帧、降 fps、关特效、强制 soft、关 GPU）
- 打包与依赖裁剪差异

验收：同一应用源可打出两档配置，Field 档在约定弱机上稳定。

## P5 — Immediate 视口

- 显式 `immediate { }`（或等价 API）区域（`immediate`）
- 支持局部嵌入；保留整窗 Immediate 能力但不作 Field 默认
- 输入焦点、裁剪、刷新边界清晰

验收：retained 主界面中可嵌一块 Immediate 工具面板且互不抢事件。

## P6 — 工业打磨

- **触控**与工控按键映射（P0 未做，本阶段补齐）
- 无障碍基线
- 7×24 稳定性（泄漏、长时间运行）
- 多分辨率 / DPI
- **Windows** 平台后端；macOS Office 首版已完成，后续补齐 DPI、IME、触控等工业打磨

验收：面向现场与办公室的真实场景试点可长期运行；OS 按序补齐。

## 建议顺序

先打通 **P0 软渲染 + 脏区（Linux，指针+键盘）**，再紧凑声明式与文本，再 GPU 与双档，最后 Immediate、触控与多 OS。不要先做满血 GPU demo 再回头补弱机路径；也不要第一天做满血 QML 式语言。
