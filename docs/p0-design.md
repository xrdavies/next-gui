# P0 设计：平台 / 场景 / 软渲染

> **状态：实现中。** 对应 [roadmap.md](roadmap.md) P0；约束见 [requirements.md](requirements.md)、[architecture.md](architecture.md)。
>
> 范围：**Linux**、`platform` + `scene` + `render` + `render-soft`、retained 矩形树、脏区重绘、指针 + 键盘。不含 DSL / GPU / 触控 / Immediate / layout 完整实现。

---

## 1. 目标与验收

### 1.1 目标

1. 在 Linux 上打开自有窗口生命周期（经 `platform` 统一接口）。
2. 维护一棵 **retained** 矩形节点树（`scene`），变更时标记脏区。
3. 经 `render` 产出后端无关绘制命令，由 `render-soft` 栅格到 CPU 像素缓冲。
4. 仅重绘脏区（可观测：非全屏无脑刷）。
5. 响应简单指针与键盘事件。

### 1.2 成功标准（可运行 demo）

| 项 | 通过样子 |
|----|----------|
| 开窗 | Linux 下 `e01_soft_window` 能创建窗口并显示内容 |
| retained | 树上多个矩形节点；改属性不销毁整树 |
| 脏区 | 颜色/位置变更只把相关矩形并入脏区；软渲染按脏区裁剪绘制 |
| 输入 | 指针按下命中节点可改色；键盘（如 Esc 退出、空格切换演示色）可用 |
| 模块边界 | `ui`/`gpu`/`immediate` 未引入；示例只依赖 platform/scene/render/render-soft |
| 编译 | `cargo build -p e01_soft_window` 在仓库内通过 |

### 1.3 本阶段明确不做

- 紧凑 DSL / `ui` crate
- GPU 后端
- 触控
- Immediate 视口
- 完整约束布局（P0 节点自带显式几何）
- Windows / macOS 后端实现（接口可预留 stub）

---

## 2. Crate 布局

```text
next-gui/
  Cargo.toml                 # workspace
  crates/
    next-gui-platform/       # 窗口、事件、像素呈现（Linux 先做）
    next-gui-scene/          # retained 树、脏区、命中
    next-gui-render/         # DrawCommand + Renderer trait
    next-gui-render-soft/    # CPU 软渲染实现
  examples/
    01_soft_window/          # 可运行演示
  docs/
    p0-design.md             # 本文
```

依赖方向（硬规则）：

```text
e01_soft_window
    → platform, scene, render, render-soft

render-soft → render, scene（仅几何/颜色类型，或经 render 的命令）
render      → scene（Rect/Color 等共享几何类型；或 render 自带几何，scene 复用）
scene       → （无平台 / 无渲染依赖）
platform    → （无 scene / 无 UI）
```

P0 为减少重复，将 `Rect` / `Color` 放在 `next-gui-scene`，由 `render` / `render-soft` 依赖 `scene` 的纯数据类型。后续若膨胀可抽 `next-gui-geom`。

---

## 3. `platform` 接口

职责：窗口生命周期、事件泵、把 CPU RGBA（或 XRGB）帧呈到窗口。不含控件与布局。

### 3.1 核心类型（概念）

```rust
pub struct WindowId(u64);

pub struct WindowAttrs {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

pub enum PointerButton { Left, Right, Middle }

pub enum KeyCode {
    Escape, Space, // P0 最小集；其余可 Extended(u32)
    Other(u32),
}

pub enum Event {
    CloseRequested { window: WindowId },
    Resized { window: WindowId, width: u32, height: u32 },
    RedrawRequested { window: WindowId },
    PointerMoved { window: WindowId, x: f64, y: f64 },
    PointerButton {
        window: WindowId,
        button: PointerButton,
        pressed: bool,
        x: f64,
        y: f64,
    },
    Key { window: WindowId, key: KeyCode, pressed: bool },
}
```

### 3.2 运行时 API（P0 落地）

winit 0.30 以 `ApplicationHandler` 驱动事件循环，P0 落地为回调式主机，而非手写 `pump_events` 轮询：

```rust
pub trait AppCallbacks {
    fn on_start(&mut self, ctx: &mut WindowCtx<'_>);
    fn on_event(&mut self, ctx: &mut WindowCtx<'_>, event: Event);
    fn on_redraw(&mut self, ctx: &mut WindowCtx<'_>);
}

pub struct WindowCtx<'a> { /* present / size / set_title / request_redraw */ }

pub fn run(attrs: WindowAttrs, app: impl AppCallbacks) -> Result<(), PlatformError>;
```

语义仍覆盖「建窗 / 事件 / 呈现像素」；上层不依赖 winit 类型。若后续需要游戏式 `pump_events`，可在同 crate 增补而不改 `scene`/`render`。

### 3.3 Linux 实现策略

- **允许**薄封装系统窗口库：`winit`（事件/窗口）+ `softbuffer`（CPU 帧呈现）。  
- 这属于「OS API 层」，**不是**第三方 UI 运行时。  
- 业务与 `scene` 只依赖上述 `Platform` trait，不直接依赖 winit 类型。  
- Windows / macOS：同 trait，P0 可不实现（编译 feature `linux` 默认）。

触控相关事件类型可在枚举中预留注释或 `#[non_exhaustive]`，P0 不产生。

---

## 4. `scene` 接口

职责：retained 节点树、脏区累积、命中测试。不含绘制 API。

### 4.1 节点模型（P0 最小）

```rust
pub struct NodeId(u64);

pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub bounds: Rect,      // 相对父或绝对：P0 用【窗口绝对坐标】简化
    pub fill: Color,
    pub visible: bool,
}
```

P0 布局：调用方直接设绝对 `bounds`；完整 `layout` 模块后移。

### 4.2 Scene API

```rust
impl Scene {
    pub fn new() -> Self;
    pub fn root(&self) -> NodeId;
    pub fn insert(&mut self, parent: NodeId, bounds: Rect, fill: Color) -> NodeId;
    pub fn set_fill(&mut self, id: NodeId, fill: Color);
    pub fn set_bounds(&mut self, id: NodeId, bounds: Rect);
    pub fn set_visible(&mut self, id: NodeId, visible: bool);
    pub fn remove(&mut self, id: NodeId);

    /// 取出并清空当前脏区（一组矩形，已做简单合并或原样列表）。
    pub fn take_dirty(&mut self) -> DirtyRegion;
    pub fn mark_full_dirty(&mut self, width: f32, height: f32);

    /// 自上而下命中最前（后插入优先）的可见节点。
    pub fn hit_test(&self, x: f32, y: f32) -> Option<NodeId>;

    pub fn get(&self, id: NodeId) -> Option<&Node>;
    pub fn iter_paint_order(&self) -> impl Iterator<Item = &Node>; // 父先于子或画家算法
}
```

### 4.3 脏区

```rust
pub struct DirtyRegion {
    pub rects: Vec<Rect>,
}

impl DirtyRegion {
    pub fn is_empty(&self) -> bool;
    pub fn add(&mut self, r: Rect);
    /// P0：简单轴对齐并集扩展为单个包围盒，或保留列表供 soft 逐块清。
    pub fn simplify(&mut self);
}
```

规则：

- `set_fill` / `set_visible`：把节点旧+新 bounds 加入脏区（可见性变化时）。  
- `set_bounds`：旧 bounds ∪ 新 bounds。  
- `insert` / `remove`：对应区域脏。  
- 窗口 resize：`mark_full_dirty`。

---

## 5. `render` 接口

职责：场景 → 绘制命令；定义 `Renderer` trait。不含具体栅格实现。

```rust
pub enum DrawCommand {
    FillRect { rect: Rect, color: Color },
}

pub trait Renderer {
    fn resize(&mut self, width: u32, height: u32);
    fn render(&mut self, commands: &[DrawCommand], dirty: &DirtyRegion);
    /// 取回当前帧缓冲（软渲染用）；GPU 后端后续可改为 present 到 swapchain。
    fn frame_pixels(&self) -> Option<(&[u32], u32, u32)>; // (pixels, w, h)
}

/// 将 scene 中与 dirty 相交的可见节点转为命令（画家顺序）。
pub fn encode_scene(scene: &Scene, dirty: &DirtyRegion) -> Vec<DrawCommand>;
```

硬规则：`Renderer` 实现不理解「按钮」语义，只消费 `DrawCommand`。

---

## 6. `render-soft` 接口

```rust
pub struct SoftRenderer {
    width: u32,
    height: u32,
    /// packed XRGB8888，row-major
    pixels: Vec<u32>,
}

impl Renderer for SoftRenderer {
    // resize 时分配缓冲并 full clear
    // render：对 dirty 各矩形先清背景色，再执行与该区相交的 FillRect（裁剪）
    // frame_pixels：返回 &pixels
}
```

背景色：P0 约定窗口默认 capturable 的深灰（如 `0xFF202020`），由示例或 SoftRenderer 构造参数指定。

脏区绘制步骤：

1. 对每个 dirty rect（与帧缓冲求交）填充背景。  
2. 遍历命令，与 dirty 求交后 `fill_rect_clipped`。  
3. 调用方 `platform.present` 整帧或（后续优化）只 present dirty——P0 **整帧 present** 可接受，但 **栅格阶段必须按脏区**，以便弱机少算像素；文档与 demo 日志可打印脏区面积。

---

## 7. 示例 `e01_soft_window` 行为

1. 创建 800×600 窗口，标题含 `next-gui P0`。  
2. Scene：根 + 背景矩形 + 若干可点色块（模拟按钮）。  
3. 循环：`pump_events` → 更新 scene → `take_dirty` → `encode_scene` → `soft.render` → `present`。  
4. 左键命中色块：切换 fill，产生脏区。  
5. `Space`：切换演示色；`Escape` / 关闭：退出。  
6. 可选：标题栏或 stderr 打印本帧脏区矩形数量/面积，证明非「无脏区概念」。

无 DISPLAY 时：示例应能 **编译**；运行失败需给出明确错误（不强依赖 CI 有图形）。

---

## 8. 实现分期（P0 内）

| 步 | 交付 |
|----|------|
| A | 本文 + workspace 空 crate 可 `cargo check` |
| B | `scene` + `render` + `render-soft` 单元可测（脏区、fill 像素） |
| C | Linux `platform`（winit + softbuffer）+ 示例开窗交互 |

若单次 PR 无法完成 C 的完整交互：至少 A+B + platform stub，并在 README 注明缺口。目标仍是尽量完成 C。

---

## 9. 与总架构的对应

| 架构模块 | P0 |
|----------|-----|
| `platform` | Linux 实现 + trait |
| `scene` | 矩形 retained + dirty + hit_test |
| `layout` | 不做；绝对坐标 |
| `render` / `render-soft` | 做 |
| `render-gpu` / `ui` / `text` / `immediate` / `profile` | 不做 |

修订：随代码演进更新本文接口签名；破坏性变更记一笔日期即可。
