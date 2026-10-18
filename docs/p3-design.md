# P3 设计：GPU 后端（与软渲染共用场景图）

> **状态：P3 完成骨架。** 对应 [roadmap.md](roadmap.md) P3；约束见 [requirements.md](requirements.md)、[architecture.md](architecture.md)。
>
> 范围：`next-gui-render-gpu`、Linux 上经 **wgpu** 消费同一套 `DrawCommand`、示例可在 Soft / GPU 间切换；软渲染仍为兜底。不含 Field/Office Profile 开关（P4）、Immediate（P5）、原生窗口 surface 直出（可后续增强）。

---

## 1. 目标与验收

### 1.1 目标

1. 与 `render-soft` **共用** `scene` → `encode_scene` → `DrawCommand`（`FillRect` / `BlitAlpha`）。
2. 新增 **`next-gui-render-gpu`**：实现 `render::Renderer`；内部用 wgpu 离屏纹理栅格，再 **readback** 为 XRGB8888，经现有 `platform::present`（softbuffer）上屏——**不改** `ui`/`scene`/`layout` 语义，也不强迫改 Linux 窗口契约。
3. GPU **服务观感与动态内容**（同一命令路径上可合批/加速），非炫技；无适配器时 **`GpuRenderer::try_new` 失败**，应用回退 Soft。
4. 示例可 **切换** Soft ↔ GPU（有 GPU 时），视觉大体一致。

### 1.2 成功标准

| 项 | 通过样子 |
|----|----------|
| 共用命令 | GPU 与 Soft 都只吃 `encode_scene` 产出，不理解控件 |
| 创建 | `GpuRenderer::try_new(w,h,clear)` → `Ok` 或可诊断的 `Err` |
| 回退 | 无 GPU / 初始化失败时示例自动 Soft，不崩溃 |
| 切换 | 示例按键（如 Space）在两后端间切换并全量脏区重绘 |
| 脏区 | 按 dirty 清背景再画相交命令（与 Soft 同策略；P3 用 scissor） |
| 测试 | `cargo test -p next-gui-render-gpu`（无适配器则跳过 GPU 用例） |
| 编译 | `cargo build -p e05_backend_toggle` |

### 1.3 本阶段明确不做

- Field/Office `profile` 配置（P4）
- ~~经 wgpu Surface 直接 present~~ → **Office 线已实现**（见 [office-line.md](office-line.md)）；P3 示例仍默认 readback 保持 Softbuffer 窗简单
- 字形 GPU atlas / 距离场（仍用 CPU mask + `BlitAlpha`）
- Windows/macOS 专用 GPU 路径差异（wgpu 可多后端，但验收以 Linux 为主）
- 复杂特效、MSAA、动画系统

---

## 2. Crate 与依赖

```text
crates/next-gui-render-gpu/   # GpuRenderer : Renderer
examples/05_backend_toggle/   # Soft | GPU 切换演示
```

```text
ui / scene  →  encode_scene  →  DrawCommand[]
                                    │
                    ┌───────────────┴───────────────┐
                    ▼                               ▼
              render-soft                     render-gpu (wgpu)
                    │                               │
                    └──────── frame_pixels ─────────┘
                                    │
                            platform.present (XRGB)
```

依赖：`wgpu`、`pollster`（同步 `request_device`）。`render-gpu` → `render` + `scene`（颜色/脏区类型）。

硬规则：`ui` 不直连 wgpu；`render-gpu` 不持有 Button 等语义。

---

## 3. `GpuRenderer` API（P3）

```rust
GpuRenderer::try_new(width, height, clear) -> Result<Self, GpuInitError>
// Renderer: resize / render / frame_pixels
gpu.backend_name() -> &str   // e.g. "Vulkan" / "Gl"
```

- **离屏** `Rgba8Unorm` 纹理 + staging 读回 → `Vec<u32>` XRGB（与 Soft 的 `present` 契约一致）。
- **FillRect**：正交实心四边形（顶点色）。
- **BlitAlpha**：R8 覆盖纹理 + tint，src-over 混合。
- **脏区**：对每个 dirty rect 设 scissor，先铺 `clear`，再画相交命令。

---

## 4. 示例 `e05_backend_toggle`

- 简单 Column：标题显示当前后端、计数 Text、Increment/Reset、提示「Space 切换」。
- 启动：优先 `try_new` GPU，失败则 Soft 并在 stderr 说明。
- Space：若 GPU 可用则切换；切换后 `mark_full_dirty` + resize 对齐缓冲。

---

## 5. 风险与后续

| 风险 | 缓解 |
|------|------|
| 无 GPU / CI | `try_new` 失败 + Soft 兜底；测试 `#[ignore]` 或检测 adapter |
| readback 延迟 | P3 可接受；真 Surface present 后续 |
| 与 Soft 像素差 | 允许抗锯齿/混合舍入差；几何应对齐 |

下一步：Office 线做厚已完成；**P4 Profile 已落地**（见 p4-design.md）。
