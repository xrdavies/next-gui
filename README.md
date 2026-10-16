# next-gui

自研工业级 GUI 运行时：**声明式 UI + retained 场景树 + 可选 GPU 加速 + 可选 Immediate 视口**。

不把现成 GUI 框架（Slint / Qt / Flutter / ImGui 等）当作产品内核；从窗口、输入、场景图与渲染后端自底向上构建。OS 窗口与显卡 API 属于平台层；字体 shaping 等可作为阶段性基础设施接入。

## 目标

- **一套声明式 + retained 内核**，服务 Field（现场）与 Office（办公）两档部署
- **弱机兼容与长期稳定优先**；GPU 服务观感与动态内容（流畅重绘、动画、图表），非炫技
- Immediate 仅作**显式可选**能力（整窗或局部），不是默认整应用模型

## 语言

| 层 | 选择 |
|----|------|
| **实现语言** | **Rust**（C++ 仅在对接既有工控生态时作可选 FFI） |
| **声明式 UI** | **自研紧凑 DSL**（起步组件标记风格，后续再丰富绑定/状态/动画）+ **Rust 宏 / API**；二者映射到同一 retained 运行时。不以 JS/TS 为默认声明层；不上来就做满血 QML |

模块划分与依赖边界见 [docs/architecture.md](docs/architecture.md)。

## 双场景

| 档位 | 对象 | 渲染与刷新 | Immediate | OS |
|------|------|------------|-----------|-----|
| **Field（现场）** | 嵌入式 / 工控弱机 | retained 脏区；软渲染必达；GPU 可关；少特效、可停帧/降帧 | 默认关闭 | Linux → Windows（不做 macOS） |
| **Office（办公）** | 规划 / 设计人员桌面 | 同一场景图；GPU 优先（观感/动态内容）；保留软渲染降级 | 局部视口可选 | Linux → Windows → macOS（后补，仅 Office） |

共享：紧凑 DSL / Rust API、组件与设计 token、retained 语义。分叉：`profile` 后端档位与特性开关，而非两套 UI 重写。

## 平台

`platform` 尽早抽象多 OS 接口；实现顺序：**Linux → Windows → macOS（仅 Office）**。P0 输入为指针 + 键盘；触控后续补充。

## 文档

| 文档 | 内容 |
|------|------|
| [docs/requirements.md](docs/requirements.md) | 目标、非目标、场景、约束与成功标准（已确认） |
| [docs/architecture.md](docs/architecture.md) | 分层、模块边界、平台顺序、双档 Profile、Immediate（已确认） |
| [docs/roadmap.md](docs/roadmap.md) | P0–P6 落地阶段 |
| [docs/rationale.md](docs/rationale.md) | 为何自研、为何不以 Immediate / Web 壳为主路线 |
| [docs/glossary.md](docs/glossary.md) | Immediate / retained / 声明式等术语 |

## 状态

**需求与架构已确认** — 运行时实现尚未开始。落地顺序：先软渲染 + 脏区，再声明式与文本，再 GPU 与双档，最后 Immediate 与工业打磨。

## 仓库

https://github.com/xrdavies/next-gui
