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
| **Office（办公）** | 规划 / 设计人员桌面 | 同一场景图；GPU 优先（观感/动态内容）；保留软渲染降级 | 局部视口可选 | Linux → Windows → macOS（仅 Office） |

共享：紧凑 DSL / Rust API、组件与设计 token、retained 语义。分叉：`profile` 后端档位与特性开关，而非两套 UI 重写。

## 平台

`platform` 尽早抽象多 OS 接口；实现顺序：**Linux → Windows → macOS（仅 Office）**。P0 输入为指针 + 键盘；触控后续补充。

**macOS Office 后端首版已实现**：`crates/next-gui-platform/src/macos.rs` + [docs/platform-macos.md](docs/platform-macos.md)。它复用 Linux 的统一事件契约与 `winit`/`softbuffer` 软渲染路径；Field 仍不以 macOS 为交付目标。

## 文档

| 文档 | 内容 |
|------|------|
| [docs/requirements.md](docs/requirements.md) | 目标、非目标、场景、约束与成功标准（已确认） |
| [docs/architecture.md](docs/architecture.md) | 分层、模块边界、平台顺序、双档 Profile、Immediate（已确认） |
| [docs/roadmap.md](docs/roadmap.md) | P0–P6 落地阶段 |
| [docs/rationale.md](docs/rationale.md) | 为何自研、为何不以 Immediate / Web 壳为主路线 |
| [docs/glossary.md](docs/glossary.md) | Immediate / retained / 声明式等术语 |
| [docs/p0-design.md](docs/p0-design.md) | P0 模块接口、crate 布局与可运行验收 |
| [docs/p1-design.md](docs/p1-design.md) | P1 紧凑 UI / layout / 控件范围 |
| [docs/platform-macos.md](docs/platform-macos.md) | macOS **Office 桌面**后端实现与已知缺口 |

## 状态

**需求与架构已确认；P0 已通；P1 编码已开始；macOS Office 平台首版已通。** Linux 与 macOS 上可运行软渲染 retained 示例，以及紧凑声明式 Column/Text/Button 计数器示例。落地顺序：声明式与布局 → 文本（CJK）→ GPU 与双档 → Immediate 与工业打磨；macOS 仍只覆盖 Office 桌面。

### 试跑 P1（Linux/macOS，需要图形桌面会话）

```bash
cargo run -p p1_counter
```

操作：点击 **Increment** / **Reset** 更新计数（状态驱动 Text）；关闭窗口退出。stderr 打印脏区面积。

### 试跑 P0（Linux/macOS，需要图形桌面会话）

```bash
cargo run -p p0_soft_window
```

操作：左键点击色块换色；`Space` 轮换全部色块；`Esc` 退出。stderr 会打印每帧脏区面积。

系统依赖（Debian/Ubuntu）：`libxkbcommon-x11-0`（以及可用的 X11/Wayland DISPLAY）。Rust **1.86+**（见 `rust-toolchain.toml`）。

## 仓库

https://github.com/xrdavies/next-gui
