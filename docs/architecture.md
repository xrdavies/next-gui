# 架构

> **状态：已确认。** 与 [requirements.md](requirements.md) 配套：需求定「做什么」，本文定「怎么分层、模块与双档如何落」。

---

## 1. 一句话

**声明式 UI 描述界面；retained 树长期持有节点并做脏区更新；GPU 与软渲染是同一场景图的两种后端（GPU 服务观感与动态内容，非炫技）；Immediate 是显式嵌套的可选视口；Field / Office 是同一内核上的两套 Profile。**

实现语言：**Rust**。声明层：**自研紧凑 DSL + Rust API/宏**（起步紧凑组件标记，后续再丰富表达力），映射到同一 retained 语义。

---

## 2. 分层（Layers）

```text
┌─────────────────────────────────────────────┐
│ 应用（Field HMI / Office 规划·设计软件）      │
└─────────────────────┬───────────────────────┘
                      │
┌─────────────────────▼───────────────────────┐
│ ui：紧凑 DSL + Rust API / 组件 /（后续）绑定  │
└─────────────────────┬───────────────────────┘
                      │
┌─────────────────────▼───────────────────────┐
│ scene + layout：retained 树、脏区、焦点/命中、 │
│                 盒模型/约束布局               │
└─────────────────────┬───────────────────────┘
                      │
┌─────────────────────▼───────────────────────┐
│ render：场景图 → 绘制命令（后端无关）          │
└─────────────┬───────────────┬───────────────┘
              │               │
   ┌──────────▼──────┐  ┌─────▼──────────┐
   │ render-gpu      │  │ render-soft    │
   │ （Office 优先；  │  │ （Field 兜底；  │
   │  观感/动态内容） │  │  GPU 可关）     │
   └──────────┬──────┘  └─────┬──────────┘
              │               │
┌─────────────▼───────────────▼───────────────┐
│ platform：窗口 / 输入 / 时钟 / 剪贴板         │
│ （统一抽象；后端：Linux → Win → macOS Office）│
└─────────────────────────────────────────────┘

旁路：
  immediate ──显式挂到──→ scene 中的视口节点
  profile   ──配置──────→ 各层能力开关与性能预算
  text      ──经 render─→ 字形；不直连业务控件
```

| 层 | 职责 | 自研边界 |
|----|------|----------|
| 应用 | 现场 HMI、办公工具 | 产品代码 |
| `ui` | 紧凑 DSL/API、组件；（后续）绑定/状态/动画 | **必须自研** |
| `scene` / `layout` | 节点树、脏区、命中/焦点、布局 | **必须自研** |
| `render` | 场景图 → 绘制命令 | **必须自研** |
| `render-gpu` / `render-soft` | GPU 合批（观感/动态内容）/ CPU 栅格 | **必须自研**；对接系统图形 API |
| `text` | 字体、shaping、换行（含 CJK） | 自研接口；shaping 可阶段性借用成熟实现 |
| `platform` | 窗口、输入、时钟；**多 OS 统一接口** | 系统 API 薄封装（非第三方 UI 框架） |
| `immediate` | 每帧代码描述的子树/整窗 | 可选；显式开启 |
| `profile` | Field / Office 档位 | 配置，不是第二套 UI |

「不拼凑框架」= 不以现成 **UI 运行时** 为产品内核。窗口与 GPU 驱动、以及阶段性字体基础设施，不在此禁令内。

---

## 3. 模块边界（Module boundaries）

| 模块 | 职责 | 不越界 |
|------|------|--------|
| `platform` | 窗口、输入、时钟、剪贴板；**统一多 OS 抽象** | 不含控件、布局、绘制语义；不把某一 OS API 泄漏到上层 |
| `scene` | retained 节点树、脏区、命中 / 焦点 | 不含具体绘制 API、不含 DSL 解析 |
| `layout` | 盒模型 / 约束布局 | 不碰 GPU / 软渲染实现 |
| `render` | 场景图 → 绘制命令；定义后端接口 | 不含具体后端实现、不含控件语义 |
| `render-soft` | CPU 软渲染 | 不持有按钮/列表等业务控件语义 |
| `render-gpu` | GPU 后端（Vulkan/GL/Metal 等）；服务观感与动态内容 | 同上；不替代 scene 的更新模型；Field 可整档关闭 |
| `text` | 字体、shaping、换行（含 CJK） | 经 `render` 出字形；不直连 `ui` 控件逻辑 |
| `ui` | 紧凑 DSL / Rust API、组件；（后续）绑定等 | **只操作 `scene`（及布局结果）**，不直连 `render-*` |
| `immediate` | 可选 Immediate 视口 | 嵌在 `scene` 节点内；不当整栈默认 |
| `profile` | Field / Office 特性开关与性能预算 | 配置层；不是第三条 UI 栈 |

细节（crate 名、是否 workspace 拆包）可迭代；**职责与不越界**为锁定边界。

---

## 4. 依赖方向（Dependency direction）

```text
ui  →  scene / layout  →  render  →  render-soft | render-gpu
                ↑
           platform（垫底；多 OS 后端）

text  →  render（提供字形/度量；被 layout/scene 查询度量）

immediate ──旁路──→ scene（仅显式视口节点）
profile ──横切配置──→ 各层（开关与预算，不形成第二条 UI 栈）
```

硬规则：

1. `ui` 不得绕过 `scene`/`render` 抽象直接调 GPU。  
2. `layout` 纯几何，不感知后端。  
3. `render-*` 只消费绘制命令，不理解控件语义。  
4. `platform` 不包含控件与布局；上层只依赖统一接口，不依赖具体 OS。  
5. `immediate` 不得成为 `ui` 的默认后端路径。  
6. `profile` 只改能力与预算，不复制一套声明式语义。

---

## 5. 平台抽象与 OS 顺序（Platform）

`platform` **尽早**定统一接口（窗口创建/事件循环、指针、键盘、时钟、剪贴板等），各 OS 做独立后端实现。业务、`scene`、`ui` **不感知**具体操作系统。

| 顺序 | 目标 | 范围 |
|------|------|------|
| 1 | **Linux** | Field + Office（第一实现） |
| 2 | **Windows** | Field + Office；P6c Soft 骨架见 [platform-windows.md](platform-windows.md) |
| 3 | **macOS** | **仅 Office 桌面**；Field 不要求；首版实现与缺口见 [platform-macos.md](platform-macos.md) |

P0 输入范围：**指针 + 键盘**。触控以 `Event::Touch` / `TouchEvent` / `TouchPhase` 转发；**P6a** 将主触点合成进指针路径（`touch_synth`）。多指手势 / 完整工控键表仍属后续（见 [p6-design.md](p6-design.md)）。

**DPI（P6b）：** 事件坐标、`Resized`、`present` / `size()` 默认 **物理像素**；`WindowCtx::scale_factor()` 与 `Event::ScaleFactorChanged` 暴露 OS 缩放；逻辑布局经 `next_gui_platform::dpi` 换算。触控合成与指针命中须用同一换算。不强制改写全部 widget。

剪贴板：`WindowCtx::clipboard_get_text` / `clipboard_set_text`（UTF-8 文本，`arboard`；Linux 优先 X11，Wayland 可能受限；见 `next_gui_platform::clipboard`）。

```text
          ┌─ linux_backend   （先做）
platform ─┼─ win_backend     （P6c Soft 骨架；运行时跟验）
  trait   └─ macos_backend   （Office only；首版已实现）
```

---

## 6. 双档 Profile（Field / Office）

一套场景图，两套 **Backend + Profile**，不是两套 UI。

| | Field（现场） | Office（办公） |
|--|---------------|----------------|
| 树模型 | retained（同一套） | retained（同一套） |
| 声明式 | 同一紧凑 DSL / Rust API / 组件库 | 同左 |
| 渲染 | **软渲染优先**；核显可选；**GPU 可关** | **GPU 优先**（观感/动态内容）；保留软渲染降级 |
| 刷新 | 脏区 + 停帧 / 降 fps | 更高帧率与动效预算 |
| 特效 | 少动画、少模糊/重阴影 | 更丰富动效、图表、多窗口 |
| Immediate | **默认关**（或仅调试） | **局部**工具面板可选 |
| 视觉 | 高对比、清晰、稳 | 同一设计系统上更好的观感与动态表现 |
| OS | Linux → Windows | Linux → Windows → **macOS（仅 Office）** |
| 打包 | 少依赖、可裁剪 | 可带更完整资源与后端 |

`profile` 模块负责：特性开关、性能预算、最低机型与降级策略（关特效、降帧、强制 soft、关 GPU）。应用源码与 DSL **不**按场景分叉成两套产品。

**实现（P4）：** crate `next-gui-profile`（`ProfileConfig::{field,office,from_env}`）；设计见 [p4-design.md](p4-design.md)；演示 `cargo run -p e24_profile_dual`。Field Soft 裁剪：`./tools/ci/field-soft.sh` 与 CI job **field-soft** 证明 Soft 子集不链 `next-gui-render-gpu` / wgpu；`e24` 以 feature `gpu`（默认开）可选链接 GPU。

**GPU 在架构中的定位：** 提升观感与动态内容绘制质量（流畅重绘、动画、图表），不是「炫技层」。Field 档可关闭而不改变声明式 / retained 语义。

---

## 7. Immediate 嵌套（Immediate nesting）

> 实现见 [p5-design.md](p5-design.md)、`next-gui-immediate`、示例 `e25_immediate_viewport`（P5 骨架）。

### 7.1 定位

- Immediate = **每帧用代码重建 UI 描述**的可选区域；框架不在该区域内默认长期持有完整控件对象树。  
- 与 retained **正交于「是否用 GPU」**：Immediate 区域仍可走同一 `render` 管线。  
- 产品默认：**主体 retained**；Immediate 按需挂载。

### 7.2 两种形态

| 形态 | 含义 | 建议 |
|------|------|------|
| **局部** | retained 树中显式视口节点（概念：`immediate { ... }`） | **推荐**；属性面板、调试曲线、脚本沙盒 |
| **整窗** | 整个窗口走 Immediate | 允许，适合工具/诊断窗；**不作 Field 主 HMI 默认** |

### 7.3 边界要求

嵌入时必须划清：

- **输入焦点**：视口内外不互相抢事件  
- **裁剪**：绘制与命中限制在视口矩形内  
- **刷新**：Immediate 区可按帧刷新；外层 retained 仍走脏区，避免整窗被带着每帧全量重绘  

禁止：把整应用默认标成 Immediate，否则 Field 的弱机优势丢失。

### 7.4 与声明式的关系

```text
紧凑 DSL / Rust API
        │
        ▼
  retained 节点树 ──默认──→ 脏区 → render → soft|gpu
        │
        └── 某节点声明为 Immediate 视口
                 │
                 ▼
           每帧代码描述 → 合入该视口的绘制命令
```

开发者**显式声明**某部分为 Immediate；不是运行时静默整窗切换模型。

---

## 8. 声明式层如何接到 retained

| 写法 | 角色 |
|------|------|
| **自研紧凑 DSL** | 主写屏：组件标记风格、结构短；配屏、HMI；热重载/检查随能力演进 |
| **Rust 宏 / API** | 复杂逻辑、自定义控件、与业务库直连；扩展控件的主路径 |
| **后续演进** | 按需增加绑定、状态、动画等，向更强（类 QML）表达力靠拢；**非第一天满血 QML** |

二者编译/绑定到**同一** `scene` 语义。  
`ui` 产出/更新的是 retained 节点与（后续）绑定，而不是直接下发 GPU 调用。

紧凑起步意味着：声明文件偏「树结构 + 少量属性」；复杂行为优先落在 Rust。语言面保持小，解析与工具链先跑通，再长能力。

---

## 9. 渲染路径（与弱机）

```text
scene（脏区） → render 命令流
                    │
         ┌──────────┴──────────┐
         ▼                     ▼
   render-gpu              render-soft
   Office 常用              Field 必达兜底
   （观感/动态内容）         （无 GPU 亦可启动）
```

- GPU 优化「怎么画、画得是否够顺够好看」，不改变「树是否 retained」。  
- Field **不得**出现「没有 GPU 就无法启动」的架构依赖。  
- 软渲染与 GPU 必须共用场景图，切换后布局/事件语义不变（抗锯齿等呈现差允许）。

---

## 10. 明确不做（架构层面）

- 现成 GUI 框架作内核  
- Immediate + 满血 GPU 作整栈默认  
- 把 GPU 当作炫技层而非观感/动态内容手段  
- 产品叙事上的「整窗 Immediate ↔ retained 运行时互切」  
- Field 以 Web 壳为 UI 底层  
- JS/TS 默认声明层  
- **QML 优先 / 第一天满血声明式语言**  
- `ui` 直连 `render-gpu`  
- 为 Office / Field 维护两套不兼容的场景图语义  
- 上层绑定某一 OS；或要求 macOS 支持 Field  

---

## 11. 与需求文档的对应

| 需求要点 | 架构落点 |
|----------|----------|
| 自研、双场景 | 分层自研 + `profile` 双档 |
| 声明式 + retained | `ui` → `scene`/`layout` |
| 紧凑 DSL → 再丰富 | `ui` 起步紧凑；绑定/状态后增 |
| GPU 服务观感与动态内容 | `render-gpu`；Field 可关 |
| 软渲染兜底 | `render-soft` 必达 |
| Immediate 可选 | `immediate` 挂 `scene` |
| Rust + DSL | 实现语言与 `ui` 双写法 |
| 弱机优先 | Field profile；P0 先 soft + 脏区 |
| OS：Linux → Win → macOS Office | `platform` 统一抽象 + 按序后端 |
| P0 指针+键盘；P6a 触控→指针 | `Event::Touch` + 主触点合成；见 [p6-design.md](p6-design.md) |
| P6b DPI / scale | 默认物理；`scale_factor` + `dpi`；见 [p6-design.md](p6-design.md) §3.2 |
| P6d a11y 基线 | 焦序（含 Button）+ `accessible_name` + `take_button_activate`；无 AT 总线；见 [a11y-baseline.md](a11y-baseline.md) |
| P6e soak / 7×24 | `NEXT_GUI_SOAK` + [soak.md](soak.md)；CI 不强制长跑 |
| 剪贴板文本 API | `WindowCtx::clipboard_*`（platform） |

本文已定稿；与 [requirements.md](requirements.md)、[roadmap.md](roadmap.md) 一并作为实现依据。
