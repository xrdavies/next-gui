# Office 线：在跳过 P4 的前提下做厚办公桌面能力

> **状态：进行中。** P4（Field/Office `profile`）**暂缓**；先把 Office 体验做厚，Field 继续吃 Soft 路径。
>
> 对应决策：跳过 P4 → 加强 GPU present / 布局 / 办公向控件；Soft 始终可回退。

---

## 1. 目标

Office 档面向规划 / 设计人员桌面（Linux → Windows → macOS Office）：

1. **GPU 优先观感**：流畅重绘、工具条与面板布局；无适配器或初始化失败时 **Soft 兜底**。
2. **布局更像办公软件**：横向工具条（Row）、占位（Spacer）、分隔线（Separator）、交叉轴对齐。
3. **同一套 `ui` / `scene` / `DrawCommand`**：不另起 Office 专用场景图。

非目标（本阶段）：

- Field/Office **Profile 配置 crate**（P4，已推迟）
- Immediate 视口（P5）
- 完整表单控件（TextInput / List / Tree）与 IME
- 动画系统、图表控件

---

## 2. GPU present：优先原生 Surface，当前缺口

### 2.1 期望（Office 理想路径）

```text
encode_scene → DrawCommand[] → wgpu 栅格 → wgpu::Surface::present → OS 窗口
```

无 CPU readback，利于高刷新与动态内容。

### 2.2 现状（P3 骨架）

```text
encode_scene → DrawCommand[] → wgpu 离屏纹理 → CPU readback (XRGB)
                                              → platform.present (softbuffer)
```

`GpuRenderer::present_path()` 返回 `GpuPresentPath::ReadbackSoftbuffer`。

### 2.3 为何还没有 Native Surface

| 阻塞 | 说明 |
|------|------|
| softbuffer 占窗 | Linux/macOS `platform` 在建窗时即附着 softbuffer Surface；与 wgpu Surface **争用同一窗口**，不能简单叠加 |
| 契约单一 | `WindowCtx::present(pixels)` 只接受 CPU 像素；无「GPU 帧已直出」分支 |
| 双路径生命周期 | Soft 需要 softbuffer；Native Surface 需要 **不挂 softbuffer 的 GPU-first 窗口**（或运行时切换，复杂度高） |

### 2.4 后续落地条件（未实现）

1. `platform` 增加 GPU-first 建窗模式（可选：不创建 softbuffer）。
2. 暴露 `raw_window_handle` / `raw_display_handle`（或直接在 platform 内创建 `wgpu::Surface`）。
3. `GpuRenderer` 支持 `GpuPresentPath::NativeSurface`：按 surface 格式建 pipeline、`get_current_texture`、渲染后 `present`；失败则回退 Readback 或 Soft。
4. Soft 路径保持现有 softbuffer，**永不依赖** GPU Surface。

**本阶段策略：** 文档化缺口 + API 标明 present 路径；继续用 Readback 服务 Office demo；控件与布局先行。

---

## 3. 布局与控件（本阶段交付）

| 能力 | 模块 | 说明 |
|------|------|------|
| `CrossAlign` | `layout` | Start / Center / End（交叉轴） |
| `Row` | `ui` + `.ngui` | 横向 flex 容器（工具条） |
| `Spacer` | `ui` + `.ngui` | 固定宽高占位 |
| `Separator` | `ui` + `.ngui` | 细分割线 |
| Soft 回退 | 示例 | GPU 失败或切换时仍走 Soft |

示例：`examples/office_panel`（工具条 Row + 内容区 + Soft/GPU 切换）。

---

## 4. 与路线图的关系

```text
P0–P3 完成
    ↓
【当前】Office 线加厚（本文）  ←── P4 推迟
    ↓
P4 Profile（Field/Office 配置）稍后抽
    ↓
P5 Immediate → P6 工业打磨
```

---

## 5. 验收

| 项 | 通过样子 |
|----|----------|
| 文档 | 本文存在且写明 Surface 缺口与回退 |
| 布局 | `CrossAlign` + Row 工具条可测 |
| 控件 | Spacer / Separator 出现在场景中 |
| Soft | 无 GPU 或切 Soft 时示例可跑 |
| GPU | 有适配器时默认 GPU（仍为 readback present） |
| 路线图 | P4 标明推迟；指向本文 |

运行：`cargo run -p office_panel`（需图形会话；CJK 字体同 P2）。
