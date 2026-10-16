# P1 设计：紧凑声明式 UI + 简单布局

> **状态：实现中。** 对应 [roadmap.md](roadmap.md) P1；约束见 [requirements.md](requirements.md)、[architecture.md](architecture.md)。
>
> 范围：**Linux**、`layout` + `ui`（紧凑 Rust API / 宏式 builder）、Column / Button / Text、状态→视图更新、仍走 **软渲染 + 脏区**。不含 GPU、触控、Immediate、完整文件型 DSL 解析器、真实 shaping（CJK 在 P2）。

---

## 1. 目标与验收

### 1.1 目标

1. 提供 **紧凑声明式** 搭建 UI 的方式（P1 以 **Rust builder / API** 为主；独立 `.ngui` 文件语法后续再加，语义与本 API 对齐）。
2. 引入最小 **`layout`**：纵向 Column（可选横向 Row 工具函数），由期望尺寸算出绝对 `Rect`，写入 `scene`。
3. 基础控件：**Text**、**Button**；点击经命中映射到控件，更新应用状态后再刷新声明式内容。
4. 不绕过 retained：`ui` 只操作 `scene`（+ `layout`），渲染仍 `encode_scene` → `render-soft`。

### 1.2 成功标准

| 项 | 通过样子 |
|----|----------|
| 声明式 | 示例用 column + text + button 描述界面，而不是手写每帧 `FillRect` 指令 |
| 布局 | Column 按 padding/gap 排布子控件；改文案后可重新 layout |
| 状态 | 点击 Increment / Reset，计数状态变，Text 内容随之更新 |
| 脏区 | 变更仍走 `scene` 脏区；软渲染按脏区栅格 |
| 边界 | 无 GPU / Immediate / 触控；不引入第三方 UI 内核 |
| 编译 | `cargo build -p p1_counter` 通过；有 DISPLAY 时可运行 |

### 1.3 本阶段明确不做

- 满血 QML 式绑定语言 / 状态机 DSL
- 列表虚拟化、TextInput 完整编辑（可后补）
- `text` crate / CJK shaping（P2）；P1 Text 用 **内置点阵字**（ASCII 子集）画到 scene 矩形，仅作可读占位
- GPU、Windows/macOS、Immediate、触控

---

## 2. Crate 布局

```text
crates/
  next-gui-layout/     # 盒/流式几何；不碰 GPU
  next-gui-ui/         # 紧凑 API、控件、同步到 scene
examples/
  p1_counter/          # Column + Text + Button；点击改 count
docs/
  p1-design.md         # 本文
```

依赖方向：

```text
p1_counter → ui, layout, platform, scene, render, render-soft
ui         → layout, scene
layout     → scene（仅 Rect 等几何类型）
```

硬规则：`ui` 不直连 `render-*`；不依赖 Slint / Qt / egui 等。

---

## 3. `layout` 接口（P1）

```rust
pub enum Axis { Vertical, Horizontal }

pub struct FlexParams {
    pub axis: Axis,
    pub x: f32,
    pub y: f32,
    pub max_width: f32,
    pub max_height: f32,
    pub padding: f32,
    pub gap: f32,
}

pub struct ChildMeasure {
    pub width: f32,
    pub height: f32,
}

/// 按主轴依次放置子项，返回每个子项的绝对 bounds。
pub fn layout_flex(params: &FlexParams, children: &[ChildMeasure]) -> Vec<Rect>;
```

语义（P1）：

- Column = `Axis::Vertical`：主轴向下，交叉轴拉伸到 `max_width - 2*padding`（若 child 宽度更小则居中或左对齐——P1 **左对齐**）。
- 子项使用自身 `ChildMeasure`；不实现 flex-grow（后续可加）。
- 纯函数；不持有 scene。

---

## 4. `ui` 接口（P1）

### 4.1 紧凑 API 形态

```rust
let mut ui = Ui::new();
ui.column(ColumnStyle { padding: 24.0, gap: 12.0, .. }, |col| {
    let label = col.text("Count: 0");
    let inc = col.button("Increment");
    let reset = col.button("Reset");
    // 把 id 存到应用，供事件与 set_text 使用
});
ui.mount(scene.root(), &mut scene);
ui.layout(&mut scene, viewport_w, viewport_h);
```

点击后：

```rust
if let Some(id) = ui.hit_widget(&scene, x, y) {
    if id == inc { state.count += 1; }
    if id == reset { state.count = 0; }
    ui.set_text(label, format!("Count: {}", state.count));
    ui.layout(&mut scene, w, h); // 文案变长时重排
}
```

这是 **retained 树 + 紧凑 builder**，不是每帧 Immediate 主模型；builder 用于构建/更新声明结构。

### 4.2 控件

| 控件 | scene 表现 | 交互 |
|------|------------|------|
| `Column` | 可选透明/面板容器节点 | 不命中（点穿到子级）或整块背景可选 |
| `Text` | 容器 + 点阵字形小矩形 | 不作为按钮 |
| `Button` | 背景矩形 + 标签点阵 | 命中背景 → `hit_widget` 返回该 Button |

### 4.3 点阵字（P1 占位）

- 内置 5×7 ASCII 子集（数字、字母、空格、冒号等）。
- 仅用于 demo 可读；**P2 由 `text` 模块替换**，API（`set_text`）保持。

---

## 5. 示例 `p1_counter`

1. 窗口标题含 `next-gui P1`。
2. Column：一行 Text（Count）+ Increment 按钮 + Reset 按钮。
3. 左键点 Increment：`count += 1`，Text 更新；Reset 归零。
4. Esc / 关闭退出；stderr 可打脏区面积。

---

## 6. 与总架构对应

| 架构模块 | P1 |
|----------|-----|
| `layout` | Column/Row flex 最小集 |
| `ui` | 紧凑 Rust API + Text/Button/Column |
| `scene` / `render-soft` | 复用 P0 |
| DSL 文件 / 绑定语言 | 不做；API 语义留给后续 `.ngui` |
| `text` / GPU / Immediate | 不做 |

修订：随代码演进更新本文签名。
