# P1 设计：紧凑声明式 UI + 简单布局 + `.ngui` DSL

> **状态：P1 完成（Rust API + 文件型 DSL）。** 对应 [roadmap.md](roadmap.md) P1；约束见 [requirements.md](requirements.md)、[architecture.md](architecture.md)。
>
> 范围：**Linux**、`layout` + `ui`（紧凑 Rust API + **`.ngui` 文件**）、Column / Text / Button、状态→视图更新、仍走 **软渲染 + 脏区**。不含 GPU、触控、Immediate、真实 shaping（CJK 在 P2）。

---

## 1. 目标与验收

### 1.1 目标

1. 提供 **紧凑声明式** 搭建 UI 的方式：
   - **Rust builder / API**（代码侧）
   - **独立 `.ngui` 文件**（配屏侧；解析后映射到同一套 `ui` / `scene`）
2. 引入最小 **`layout`**：纵向 Column（可选横向 Row 工具函数），由期望尺寸算出绝对 `Rect`，写入 `scene`。
3. 基础控件：**Text**、**Button**；点击经命中映射到控件，更新应用状态后再刷新声明式内容。
4. 不绕过 retained：`ui` 只操作 `scene`（+ `layout`），渲染仍 `encode_scene` → `render-soft`。

### 1.2 成功标准

| 项 | 通过样子 |
|----|----------|
| 声明式 API | 示例用 column + text + button 描述界面，而不是手写每帧 `FillRect` 指令 |
| `.ngui` | 同一界面可由 `.ngui` 文件描述；`load_ngui` → `Ui`；`@name` 取回 `WidgetId` |
| 布局 | Column 按 padding/gap 排布子控件；改文案后可重新 layout |
| 状态 | 点击 Increment / Reset，计数状态变，Text 内容随之更新 |
| 脏区 | 变更仍走 `scene` 脏区；软渲染按脏区栅格 |
| 边界 | 无 GPU / Immediate / 触控；不引入第三方 UI 内核 |
| 编译 | `cargo build -p p1_counter -p p1_ngui_counter`；有 DISPLAY 时可运行 |

### 1.3 本阶段明确不做

- 满血 QML 式绑定语言 / 状态机 DSL（绑定能力后续演进）
- 列表虚拟化、TextInput 完整编辑（可后补）
- `text` crate / CJK shaping（P2）；P1 曾用点阵占位；**P2 已由 `text` 替换**
- GPU、Windows/macOS、Immediate、触控

---

## 2. Crate 布局

```text
crates/
  next-gui-layout/     # 盒/流式几何；不碰 GPU
  next-gui-ui/         # 紧凑 API、.ngui 解析、控件、同步到 scene
examples/
  p1_counter/          # Rust API：Column + Text + Button
  p1_ngui_counter/     # .ngui 文件加载同一套 UI
    ui/counter.ngui
docs/
  p1-design.md         # 本文
```

依赖方向：

```text
p1_counter / p1_ngui_counter → ui, layout, platform, scene, render, render-soft
ui         → layout, scene
layout     → scene（仅 Rect 等几何类型）
```

硬规则：`ui` 不直连 `render-*`；不依赖 Slint / Qt / egui 等。

---

## 3. `layout` 接口（P1）

```rust
pub enum Axis { Vertical, Horizontal }

pub struct FlexParams { /* axis, x, y, max_width, max_height, padding, gap */ }
pub struct ChildMeasure { pub width: f32, pub height: f32 }

pub fn layout_flex(params: &FlexParams, children: &[ChildMeasure]) -> Vec<Rect>;
```

语义（P1）：Column = `Axis::Vertical`；子项用自身 measure；无 flex-grow；纯函数。

---

## 4. `ui` 接口（P1）

### 4.1 紧凑 Rust API

```rust
let mut ui = Ui::new();
ui.column(ColumnStyle { padding: 24.0, gap: 12.0, .. }, |col| {
    let label = col.text("Count: 0");
    let inc = col.button("Increment");
    let reset = col.button("Reset");
});
ui.sync(&mut scene);
```

### 4.2 `.ngui` 文件 DSL（紧凑组件标记）

最小语法（递归下降解析，无第三方解析器依赖）：

```text
// 行注释（`#` 保留给 #RRGGBB 颜色）

Column {                         # 文档根；可加 @root_name
  x: 40
  y: 40
  width: 360
  height: 520
  padding: 20
  gap: 14
  background: #2a2a2e            # 可选；#RRGGBB

  Text("Count: 0") @label        # @name → WidgetId
  Button("Increment") @inc
  Button("Reset") @reset {
    bg: #6b7280                  # 可选按钮属性
    fg: #ffffff
  }

  Column {                       # 可嵌套
    padding: 8
    Text("nested")
  }
}
```

| 构造 | 含义 |
|------|------|
| `Column { props… children… }` | retained 纵向容器；props：`x y width height padding gap background` |
| `Text("…") @name` | 文本；`@name` 可选 |
| `Button("…") @name { bg fg }` | 按钮；属性块可选 |
| `//` | 行注释（`#` 专用于颜色） |

加载：

```rust
use next_gui_ui::load_ngui;

let loaded = load_ngui(include_str!("../ui/counter.ngui"))?;
let label = loaded.require("label")?;
let mut ui = loaded.ui;
ui.sync(&mut scene);
```

也可用 `load_ngui_path(path)` 读磁盘文件。解析错误带行列号（`DslError`）。

Rust API 与 `.ngui` **语义对齐**：后者只是前者的文件前端，不另起运行时。

### 4.3 控件

| 控件 | scene 表现 | 交互 |
|------|------------|------|
| `Column` | 可选面板容器 | 点穿到子级 |
| `Text` | 点阵字形小矩形 | 非按钮 |
| `Button` | 背景 + 标签点阵 | `hit_widget` 返回该 Button |

### 4.4 点阵字（P1 占位）

P2 起由 `next-gui-text` 提供 TrueType/CJK；`set_text` API 保持。点阵模块已移除。

---

## 5. 示例

### `p1_counter`

Rust builder 手写 Column；点击 Increment / Reset。

```bash
cargo run -p p1_counter
```

### `p1_ngui_counter`

从 `examples/p1_ngui_counter/ui/counter.ngui` 加载同一界面；用 `@label` / `@inc` / `@reset` 接线。

```bash
cargo run -p p1_ngui_counter
```

---

## 6. 与总架构对应

| 架构模块 | P1 |
|----------|-----|
| `layout` | Column/Row flex 最小集 |
| `ui` | 紧凑 Rust API + **`.ngui` 解析** + Text/Button/Column |
| `scene` / `render-soft` | 复用 P0 |
| 绑定语言 / 状态机 | 不做；后续在紧凑 DSL 上演进 |
| `text` / GPU / Immediate | 不做 |

修订：随代码演进更新本文签名。
