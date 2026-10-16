# next-gui

自研工业级 GUI 运行时：**声明式 UI + retained 场景树 + 可选 GPU 加速 + 可选 Immediate 视口**。

不把现成 GUI 框架（Slint / Qt / Flutter / ImGui 等）当作产品内核；从窗口、输入、场景图与渲染后端自底向上构建。OS 窗口与显卡 API 属于平台层；字体 shaping 等可作为阶段性基础设施接入。

## 目标

- **一套声明式 + retained 内核**，服务两种部署场景
- **弱机兼容优先**（现场），办公室在同一底座上开更高视觉与交互预算
- Immediate 仅作**显式可选**能力（整窗或局部），不是默认整应用模型

## 语言

| 层 | 选择 |
|----|------|
| **实现语言** | **Rust**（C++ 仅在对接既有工控生态时作可选 FFI，非默认实现语言） |
| **声明式 UI** | **自研 DSL**（类似 Slint / QML）为主写屏；**Rust 宏 / API** 为代码侧等价物；二者映射到同一 retained 运行时。不以 JS/TS 为默认声明层 |

模块划分与依赖边界见 [docs/architecture.md](docs/architecture.md)。

## 双场景

| 档位 | 对象 | 渲染与刷新 | Immediate |
|------|------|------------|-----------|
| **Field（现场）** | 嵌入式 / 工控弱机，多核显或软渲染 | retained 脏区；软渲染兜底；稳优先、少特效、可停帧/降帧 | 默认关闭 |
| **Office（办公）** | 规划 / 设计人员桌面 | 同一场景图；GPU 优先；更丰富动效与多窗口 | 局部视口可选 |

共享：声明式组件、设计 token、业务侧契约。分叉：后端档位与特性开关，而非两套 UI 重写。

## 文档

| 文档 | 内容 |
|------|------|
| [docs/architecture.md](docs/architecture.md) | 语言选型、模块边界、分层、Field vs Office |
| [docs/roadmap.md](docs/roadmap.md) | P0–P6 落地阶段 |
| [docs/rationale.md](docs/rationale.md) | 为何自研、为何不以 Immediate / Web 壳为主路线 |
| [docs/glossary.md](docs/glossary.md) | Immediate / retained / 声明式等术语 |

## 状态

**设计阶段** — 语言与模块边界已敲定，运行时实现尚未开始。

## 仓库

https://github.com/xrdavies/next-gui
