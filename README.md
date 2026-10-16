# next-gui

自研工业级 GUI 运行时：**声明式 UI + retained 场景树 + 可选 GPU 加速 + 可选 Immediate 视口**。

不把现成 GUI 框架（Slint / Qt / Flutter / ImGui 等）当作产品内核；从窗口、输入、场景图与渲染后端自底向上构建。OS 窗口与显卡 API 属于平台层；字体 shaping 等可作为阶段性基础设施接入。

## 目标

- **一套声明式 + retained 内核**，服务两种部署场景
- **弱机兼容优先**（现场），办公室在同一底座上开更高视觉与交互预算
- Immediate 仅作**显式可选**能力（整窗或局部），不是默认整应用模型

## 双场景

| 档位 | 对象 | 渲染与刷新 | Immediate |
|------|------|------------|-----------|
| **Field（现场）** | 嵌入式 / 工控弱机，多核显或软渲染 | retained 脏区；软渲染兜底；稳优先、少特效、可停帧/降帧 | 默认关闭 |
| **Office（办公）** | 规划 / 设计人员桌面 | 同一场景图；GPU 优先；更丰富动效与多窗口 | 局部视口可选 |

共享：声明式组件、设计 token、业务侧契约。分叉：后端档位与特性开关，而非两套 UI 重写。

## 文档

| 文档 | 内容 |
|------|------|
| [docs/architecture.md](docs/architecture.md) | 分层、声明式 / retained / GPU / Immediate 关系、Field vs Office |
| [docs/roadmap.md](docs/roadmap.md) | P0–P6 落地阶段 |
| [docs/rationale.md](docs/rationale.md) | 为何自研、为何不以 Immediate / Web 壳为主路线 |
| [docs/glossary.md](docs/glossary.md) | Immediate / retained / 声明式等术语 |

## 状态

**设计阶段** — 方案与路线已落文档，运行时实现尚未开始。

实现语言候选：**Rust** 或 **C++**（未最终选定）。

## 仓库

https://github.com/xrdavies/next-gui
