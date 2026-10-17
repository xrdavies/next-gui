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
- 完整 IME/CJK 输入、List / Table / Tree / Select / Slider / Menu（后续切片）
- 动画系统、图表控件

---

## 2. GPU present：原生 Surface（已落地）

### 2.1 Office 路径（当前默认，有适配器时）

```text
encode_scene → DrawCommand[] → wgpu 离屏 retained 纹理
                              → blit → wgpu::Surface::present → OS 窗口
```

无 CPU readback。`GpuRenderer::present_path()` → [`GpuPresentPath::NativeSurface`]。

前提：窗口以 [`PresentBackend::GpuSurface`] 创建（**不**附着 softbuffer），再在 `on_start` 里
`GpuRenderer::try_new_native(winit_window, …)`。

### 2.2 Softbuffer / readback 路径（仍保留）

```text
encode_scene → DrawCommand[] → wgpu 离屏 → CPU readback (XRGB)
                                          → platform.present (softbuffer)
```

或纯 Soft：`SoftRenderer` → `platform.present`。

`PresentBackend::Softbuffer` 建窗；`GpuRenderer::try_new` → `GpuPresentPath::ReadbackSoftbuffer`。
P3 示例 `p3_backend_toggle` 仍走此路径。

### 2.3 Soft ↔ GPU 在 GpuSurface 窗口上

| 模式 | 做法 |
|------|------|
| GPU | `GpuRenderer::render` 内部已 Surface present |
| Soft | `SoftRenderer` 出 XRGB → `GpuRenderer::present_xrgb_frame` 上传并 blit 到同一 Surface |

不在同一窗口同时挂 softbuffer 与 wgpu Surface。

### 2.4 平台契约

| API | 说明 |
|-----|------|
| `WindowAttrs::with_gpu_surface()` | 建窗不创建 softbuffer |
| `WindowCtx::winit_window()` | `Arc<winit::Window>`，供 `try_new_native` |
| `WindowCtx::present` | 仅 Softbuffer；GpuSurface 上调用会报错 |
| `GpuRenderer::try_new_native` | 创建 Surface + 离屏 retained + copy pipeline |
| `GpuRenderer::present_xrgb_frame` | Soft-on-Surface 上传 present |

---

## 3. 布局与控件（本阶段交付）

| 能力 | 模块 | 说明 |
|------|------|------|
| `CrossAlign` | `layout` | Start / Center / End（交叉轴） |
| `Row` | `ui` + `.ngui` | 横向 flex 容器（工具条基础） |
| `Spacer` | `ui` + `.ngui` | 固定宽高占位 |
| `Separator` | `ui` + `.ngui` | 细分割线 |
| `Icon` | `ui` | 内置命名字形（工具条/状态栏） |
| `Toolbar` | `ui` | **一等公民**横向工具条 |
| `Panel` | `ui` | **一等公民**带标题内容容器 |
| `StatusBar` | `ui` | **一等公民**底栏分区 |
| `Splitter` | `ui` | 左右/上下拖拽分隔（指针拖 sash） |
| Soft 回退 | 示例 | GPU 失败或切换时仍可 Soft |
| Native Surface | `render-gpu` + `platform` | Office 默认（有适配器时） |
| `ScrollArea` | `ui` | 视口裁剪 + 滚轮/拖拽滚动 |
| `TextInput` | `ui` | 单行录入（ASCII）；IME/CJK 组合输入仍 stub |
| `Checkbox` / `Radio` / `Switch` | `ui` | 点击/Space 切换；Radio 分组互斥 |

示例：

- `examples/office_panel` — 早期 Row 工具条 + Soft/GPU 切换
- `examples/office_shell` — **P0 shell**：Toolbar + Icon + Panel + Splitter + StatusBar
- `examples/office_inputs` — **P0 input**：ScrollArea + TextInput + Checkbox + Radio + Switch

Rust API 优先；复杂控件暂未进 `.ngui`。下一切片：`Table` / `Tree` / `List` / `Select` / `Slider` / `Menu`。

---

## 3.1 控件目录（已确认）

完整对照表与优先级见 **[office-widget-catalog.md](office-widget-catalog.md)**（Ant Design / Element Plus / MUI + 工业扩展；**2026-10-17 已拍板**）。

本文仍只交付 GPU present / 布局壳；**P0 控件实现**按该目录 §4 / §10 推进（必含 `text_input` + 基础 `table`/`tree`；`toolbar` / `status_bar` / `panel` 一等公民；Form 校验与 `alarm_list`/`gauge`/`trend` 仍 P1；层级选择 P1 仅 `tree_select`）。

---

## 4. 与路线图的关系

```text
P0–P3 完成
    ↓
【当前】Office 线加厚（本文）  ←── P4 推迟
    · 布局/工具条控件 ✓
    · 原生 Surface present ✓
    · 控件目录（已确认）：[office-widget-catalog.md](office-widget-catalog.md)
    · 下一步：按锁定 P0 名单实现控件（先壳层再录入/表树，见目录 §10）
    ↓
P4 Profile（Field/Office 配置）稍后抽
    ↓
P5 Immediate → P6 工业打磨
```

---

## 5. 验收

| 项 | 通过样子 |
|----|----------|
| 文档 | 本文写明 Surface 路径与 Softbuffer 回退 |
| 布局 | `CrossAlign` + Row 工具条可测 |
| 控件 | Spacer / Separator 出现在场景中 |
| Soft | Softbuffer 窗或 Surface blit 可跑 |
| GPU native | 有适配器时 `present=NativeSurface`（见 stderr） |
| GPU readback | Softbuffer 窗上 `try_new` 仍可用 |
| 路线图 | P4 标明推迟；指向本文 |

运行：`cargo run -p office_panel`（需图形会话；CJK 字体同 P2）。
有适配器时应看到：`native Surface ready (…, present=NativeSurface)`。
