# 路线图（P0–P6）

**实现语言已定为 Rust。** 声明式层为自研 DSL + Rust 宏/API（见 [architecture.md](architecture.md)）。各阶段目标仍不绑死具体第三方 crate 名；模块按 `platform` / `scene` / `layout` / `render*` / `text` / `ui` / `immediate` / `profile` 推进。

## P0 — 骨架

- 窗口创建与基础生命周期（`platform`）
- 输入（指针、键盘；为触控预留）
- retained 矩形树（最小 `scene` 节点模型）
- CPU **软渲染**路径（`render` + `render-soft`）
- **脏区**重绘（非每帧全屏无脑刷）

验收：空窗 + 若干矩形控件可交互，弱机可跑，变更区域可观测。

## P1 — 声明式

- 最小 DSL 或等价 Rust API（`ui`）
- 状态绑定
- 基础控件：按钮、列表、文本输入等

验收：用声明式描述即可搭出简单表单，无需手写每帧绘制指令。

## P2 — 文本

- 字体加载与度量（`text`）
- 换行 / 布局
- **CJK**（工业界面刚需）

说明：字体 shaping 可阶段性接入成熟算法实现，对外仍是自研文本引擎接口。

验收：中英混排可读、可编辑基线可用。

## P3 — GPU 后端

- 与软渲染**共用场景图**（`render-gpu`）
- 合批 / 命令录制对接系统图形 API
- Office 可默认走 GPU；Field 可关或自动降级

验收：同一 UI 树可在软渲染与 GPU 间切换，视觉一致（允许抗锯齿等实现差）。

## P4 — 双档 Profile

- Field / Office 特性开关与性能预算（`profile`）
- 最低机型清单与降级策略（停帧、降 fps、关特效）
- 打包与依赖裁剪差异

验收：同一应用源可打出两档配置，Field 档在弱机上稳定。

## P5 — Immediate 视口

- 显式 `immediate { }`（或等价 API）区域（`immediate`）
- 支持局部嵌入；保留整窗 Immediate 能力但不作 Field 默认
- 输入焦点、裁剪、刷新边界清晰

验收：retained 主界面中可嵌一块 Immediate 工具面板且互不抢事件。

## P6 — 工业打磨

- 触控与工控按键映射
- 无障碍基线
- 7×24 稳定性（泄漏、长时间运行）
- 多分辨率 / DPI

验收：面向现场与办公室的真实场景试点可长期运行。

## 建议顺序

先打通 **P0 软渲染 + 脏区**，再声明式与文本，再 GPU 与双档，最后 Immediate 与工业打磨。不要先做满血 GPU demo 再回头补弱机路径。
