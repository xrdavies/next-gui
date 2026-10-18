# P5 设计：Immediate 视口

> **状态：P5 完成骨架。** 对应 [roadmap.md](roadmap.md) P5；约束见 [requirements.md](requirements.md)、[architecture.md](architecture.md) §7。
>
> 范围：`next-gui-immediate`（局部 Immediate 视口）、示例 `e25_immediate_viewport`（retained 壳 + Immediate 工具面板）；**不**把 Immediate 设为整窗 / Field 默认。不含 P6（Windows / 触控 / DPI / a11y）、Cascader、自绘 IME 候选。

---

## 1. 目标与验收

### 1.1 目标

1. **显式** Immediate 区域 API（概念 `immediate { … }` → Rust：`ImmediateViewport` + `begin_frame`）。
2. **局部嵌入** retained 树（推荐）；整窗 Immediate 能力保留但不作 Field 默认。
3. 边界清晰：
   - **输入焦点**：视口内外不互相抢事件
   - **裁剪**：绘制与命中限制在视口矩形内
   - **刷新**：Immediate 可按帧刷新；外层 retained 仍走脏区
4. 与 GPU / Soft **正交**（同一 `DrawCommand` 管线）。

### 1.2 成功标准

| 项 | 通过样子 |
|----|----------|
| Crate | `next-gui-immediate`；不成为 `ui` 默认路径 |
| 宿主 | `mount` 在 scene 上挂 clipped host 节点 |
| 帧 API | `begin_frame` → `label` / `button` / `sparkline` / `fill_rect` / `text_at` |
| 事件 | 视口外 press 不 focus、不 click；Esc 仅 blur Immediate |
| Profile | Field `allow_immediate=false`；Office `true`；仍需应用显式 `set_enabled` |
| 示例 | retained 左壳 + 右 Immediate 工具面板 |
| 测试 | clip / hit / focus / mount clip / dirty 局部 |
| QA | `NEXT_GUI_IMMEDIATE_QA=1` Soft PPM |

### 1.3 本阶段明确不做

- 整应用默认 Immediate
- Immediate DSL / `.ngui` 语法
- 自绘 IME 候选、Cascader
- P6 Windows / 触控 / 无障碍 / DPI

---

## 2. 为何独立 crate

Architecture 模块表将 `immediate` 列为旁路挂到 `scene` 视口节点，且硬规则：`immediate` 不得成为 `ui` 的默认后端路径。

| 选项 | 结论 |
|------|------|
| `crates/next-gui-immediate` | **采用**：与 `profile` 同级；依赖 `scene` + `render` + `text`；`ui` 不被迫链入 |
| `ui` 内模块 | 易与 retained 控件树纠缠，违反「不当默认路径」 |
| 仅 `scene` 扩展 | scene 不应懂按钮/sparkline 语义 |

依赖方向：

```text
应用 ──► ui (retained) ──► scene ──► render ──► soft|gpu
     └──► immediate ─────► scene / DrawCommand（旁路合入）
     └──► profile（allow_immediate 开关，不持有视口）
```

---

## 3. API（P5）

```rust
ImmediateViewport::new(bounds, ImmediateViewportStyle)
viewport.set_enabled(bool)      // Field 默认关
viewport.mount(scene, parent)   // clipped host + 可选 title strip
viewport.contains(x, y)
viewport.begin_frame(&font) -> ImmediateCtx
  ctx.paint_title(); ctx.label(..); ctx.button(..); ctx.sparkline(..);
viewport.draw_commands()        // 追加到 encode_scene 之后
viewport.pointer_pressed / pointer_released / take_clicked
viewport.handle_escape()        // 有焦点则 blur 并消费 Esc
Scene::mark_dirty(rect)         // Immediate 帧刷新只脏视口
ProfileConfig.allow_immediate   // Field false / Office true
```

整窗 Immediate：把 `bounds` 设为客户区全窗即可；产品仍推荐局部。

---

## 4. 边界约定

| 边界 | 实现 |
|------|------|
| 焦点 | 点在视口内 → `focused=true` 且 **不**把该 press 交给 retained；点外 → blur + retained 命中 |
| 裁剪 | 所有 `FillRect` / `BlitAlpha` / hit rect 与 `bounds` 求交；host `Node::clip = bounds` |
| 刷新 | 动画 tick 只 `scene.mark_dirty(viewport.bounds())`；retained 变更才全窗或控件脏区 |
| Esc | 第一次 Esc blur Immediate；未聚焦时 Esc 不由本模块消费（应用可决定退出） |

---

## 5. 示例 `e25_immediate_viewport`

- 左：Toolbar + retained Panel（`Retained +1` / `Toggle Immediate`）
- 右：Immediate 工具面板（每帧计数、sparkline、Immediate 按钮）
- `NEXT_GUI_PROFILE=field|office`；`NEXT_GUI_IMMEDIATE=1` 强制开
- QA：`NEXT_GUI_IMMEDIATE_QA=1` → Field Soft + Immediate on → PPM

```bash
cargo run -p e25_immediate_viewport
NEXT_GUI_PROFILE=office cargo run -p e25_immediate_viewport
NEXT_GUI_IMMEDIATE_QA=1 cargo run -p e25_immediate_viewport
```

---

## 6. 验收对照

| 需求 | 证据 |
|------|------|
| retained 主界面嵌 Immediate | e25 左右分栏 |
| 互不抢事件 | 单测 `outside_pointer_does_not_focus_or_click`；演示左按钮不影响右 |
| 裁剪 | 单测 `contains_and_clip_hit`；host clip |
| 非默认路径 | Field `allow_immediate=false`；无 ui 后端切换 |

下一步：**P6 工业打磨**（见 [p6-design.md](p6-design.md)）。
