# P6 设计：工业打磨

> **状态：P6a / P6b 已落地（触控合成 + DPI 契约）；P6c–P6e 未开。** 对应 [roadmap.md](roadmap.md) P6；约束见 [requirements.md](requirements.md)、[architecture.md](architecture.md) §5、[platform-macos.md](platform-macos.md)。
>
> 范围：触控 → 指针 HMI 映射、DPI / 缩放策略、Windows 后端骨架、无障碍基线、7×24 / 泄漏探测；macOS Office 工业打磨为跟进项，不挡 Linux 切片。
>
> **本阶段不做：** Cascader、Office P2 控件加厚、复杂控件进 `.ngui`、自绘 IME 候选窗、满血 QML 式语言。Field Soft 兜底不得丢掉。

---

## 1. 目标与非目标

### 1.1 目标

1. **触控可用：** 工控触屏上单指点击 / 拖动可驱动现有 retained UI（命中、按钮、滚动），不必手写双套事件路径。
2. **DPI 有策略：** 逻辑坐标与物理像素关系写清；Soft 与 GpuSurface 行为一致，Retina / 混缩有验收口径。
3. **Windows 可跑：** 与 Linux P0 对等的窗口 + Soft present + 指针/键盘；Field Soft 仍可无 GPU 启动。
4. **无障碍起步：** 焦点顺序与可访问名称/标签基线；不做完整 AT-SPI / UI Automation 矩阵。
5. **长时运行可观测：** 泄漏 / soak 有文档化探针或可选示例，便于 7×24 试点。

### 1.2 非目标（P6 明确不做）

| 项 | 说明 |
|----|------|
| Cascader | 仍不进 P1；不占用 P6 |
| Office P2 控件目录 | `button_group` / `upload` / … 可选并行，不挡 P6 |
| 复杂控件进 `.ngui` | 仍 Rust API 优先 |
| 自绘 IME 候选窗 | 继续走 OS IME |
| 满血手势语言 | 双指缩放、旋转、自定义手势语言 → 后补 |
| 完整 a11y 平台桥 | AT-SPI / UIA / VoiceOver 深度集成 → 后补 |
| 丢掉 Soft 兜底 | Field / 降级路径必须保留 |
| macOS Field | 仍仅 Office |

---

## 2. 切片与推荐顺序

P6 按 **切片** 落地，可独立合入；推荐顺序如下。实现前本文件保持「设计」状态；某切片合入后在本节勾选并回写 roadmap。

| 切片 | 主题 | 主战场 | 依赖 |
|------|------|--------|------|
| **P6a** ✅ | 触控 → 指针 / 点击 HMI 映射 | Linux + macOS（共享 `touch_synth`） | 现有 `Event::Touch` + 主触点合成指针 |
| **P6b** ✅ | DPI / scale 策略 | Linux + macOS（`scale_factor` + `dpi` 助手）；Retina 手工跟验 | P6a 坐标约定 |
| **P6c** | Windows 后端骨架 | Windows | platform trait；对标 Linux P0 |
| **P6d** | 无障碍基线 | 跨平台（先 Linux 演示） | 焦点链已存在于 ui |
| **P6e** | 7×24 / 泄漏 / soak | 工具或文档 + 可选示例 | 任意；建议在 P6a–c 有窗口后再跑 |

**推荐实施顺序：P6a → P6b → P6c → P6d → P6e。**

- **P6a 优先：** stub 已有，Linux 触屏现场最刚需；映射进现有 Ui 指针路径，立刻提升可交互性。
- **P6b 紧随：** 触控与 DPI 都碰「坐标空间」；先定逻辑/物理约定，避免 Windows / Retina 返工。
- **P6c：** OS 顺序 Linux → **Windows** → macOS；Windows 运行时尚未验收。
- **P6d / P6e：** 基线与探针，不挡窗口/输入主路径；可与 P6c 尾部并行。
- **macOS 工业跟进**（DPI / IME 打磨 / 触控若 winit 支持）：不阻塞 Linux P6a；在对应切片验收后再补 [platform-macos.md](platform-macos.md)。

工控**实体按键**（功能键、急停映射等）：P6a 预留键码 / 配置钩子即可，完整现场键表可后补，不堵触控切片。

---

## 3. 各切片说明

### 3.1 P6a — 触控 HMI 映射

**现状：** `TouchPhase` / `TouchEvent` / `Event::Touch` 已定义；Linux / macOS 在 winit 发出时转发。应用层大多忽略；无「触 → 点」合成。

**设计：**

1. **主路径：合成进现有指针事件。**  
   - `Started` → `PointerMoved`（到触点）+ `PointerButton { Left, pressed: true }`  
   - `Moved` → `PointerMoved`  
   - `Ended` → `PointerButton { Left, pressed: false }`（可选再发一次 Moved）  
   - `Cancelled` → 释放按下态（等同 Ended 的 button up，避免粘键）
2. **多指：** P6a 只认 **主触点**（第一枚 `id`，或 Started 时尚未有 active 的那枚）；其余触点可保留在 `Event::Touch` 供后补手势，**不**合成第二套指针。
3. **坐标：** 与当前指针一致，先用 **物理像素**（与现 stub 的 `x`/`y` 一致）；逻辑像素换算归 P6b。
4. **投放点：** 优先在 `platform` 后端（linux，必要时 macos）或薄适配层合成后仍走同一 `Event` 回调，使 `ui` / 示例零改或微改即可点按钮、拖滑条。
5. **保留 `Event::Touch`：** 不删除；需要原始触点的应用仍可听。文档写明：默认 HMI 应消费合成指针，而非强制双处理。
6. **工控键：** 预留 `KeyCode::Other` / 配置表扩展点；不在 P6a 做完整现场键谱。

**实现（合入后）：** `next_gui_platform::touch_synth::TouchPointerSynth`；linux / macos 后端在转发 `Event::Touch` 后对主触点投递合成指针；主触点按下期间抑制 OS 镜像的 Left 鼠标，减轻双击。单测见该模块；无触屏 CI 不依赖硬件。

**示例 / 验收线索：** Linux 触屏或触摸仿真下跑 `e01` / `e12` / `e20`：点按、拖动滚动可完成；无触屏机器用单测模拟 `Touch` → 期望的 Pointer 序列。

### 3.2 P6b — DPI / scale 策略

**现状（合入前）：** 事件与 present 多为物理像素；macOS Retina / 混缩未正式验收；无统一 `scale_factor` 策略。

**设计：**

1. 明确两套坐标：
   - **物理像素：** framebuffer / Soft 缓冲 / GpuSurface 尺寸；指针 / 触控 `x`/`y`；`Event::Resized`；`present` / `size()`
   - **逻辑像素：** 布局、控件命中、DSL 尺寸（应用经助手换算后使用）
2. `platform` 暴露 `scale_factor`（创建时与 `ScaleFactorChanged` 时更新）；**API 默认仍为物理**，避免广改 layout。
3. Soft 与 GpuSurface：**同一逻辑 UI** 在 1× / 2× 下视觉尺度一致（允许 AA 差）——由应用按 `scale_factor` 放大布局再栅格化到物理缓冲。
4. 变更 scale 时：触控合成与指针命中使用同一 [`dpi`](../crates/next-gui-platform/src/dpi.rs) 换算。

**实现（合入后）：**

| 项 | 说明 |
|----|------|
| `WindowCtx::scale_factor()` | OS 因子（winit）；默认 `1.0` |
| `Event::ScaleFactorChanged` | linux / macos 转发 `WindowEvent::ScaleFactorChanged` |
| `next_gui_platform::dpi` | `logical_to_physical` / `physical_to_logical` 及 size/point 变体；单测 1× / 2× / round-trip |
| 事件单位 | **不**静默改单位；`WindowAttrs` width/height 仍为物理像素 |
| 非目标 | 未重写全部 widget；无每控件 DPI 资源包；示例多数仍忽略 scale（跟进） |

无 Retina 显示时 CI 不依赖硬件；macOS 混缩手工验仍为跟进项（见 [platform-macos.md](platform-macos.md)）。

**P6b 不做：** 每控件独立 dpi 资源包、矢量图标全库重做、强制逻辑坐标 API。

### 3.3 P6c — Windows 后端骨架

**对标 Linux P0（非一次性搬完 Office 全示例）：**

| 能力 | P6c 要求 |
|------|----------|
| 窗口生命周期 | 创建、关闭、resize、redraw |
| Present | **Softbuffer（或等价）Soft 路径必达** |
| 输入 | 指针 + 键盘；IME 尽力（可标已知缺口） |
| 剪贴板 | UTF-8 文本 API 与现契约一致 |
| GPU | 可选后补；无适配器时 Soft 启动 |
| Feature | `windows`（或等价）cfg；不破坏 linux/macos |

架构示意保持：

```text
          ┌─ linux_backend   （已有）
platform ─┼─ win_backend     （P6c）
  trait   └─ macos_backend   （Office；工业跟进）
```

验收：`cargo run` 至少一个 Soft 窗口示例在 Windows 图形会话下可交互；Linux/macOS 回归不破。Field Soft 兜底文档写明 Windows 同样适用。

### 3.4 P6d — 无障碍基线

**克制范围：**

- Tab / 焦序可文档化或由现有焦点链演示
- 控件可提供 **可访问名**（标签文本 / `accessible_name` 类 API 起步）
- 不承诺读屏完整遍历；不绑死某一 OS a11y 总线

验收：文档 + 小示例或单测证明焦序与名称可查询；后续总线桥另开阶段。

### 3.5 P6e — 7×24 / 泄漏 / soak

- 文档：如何跑长时示例、关注 RSS / 句柄 / 纹理数量
- 可选：`NEXT_GUI_SOAK=1` 类模式或轻量探针示例（定时 redraw、周期性打开关闭弹层）
- CI：**不**默认跑 24h；本地 / 试点机可选

验收：有可重复步骤；能发现「明显泄漏」（例如每小时单调涨且无上限）的操作说明。

---

## 4. 触控契约（相对现有指针路径）

```text
winit Touch ──► Event::Touch (保留，原始)
                    │
                    ▼  P6a 合成（主触点）
              PointerMoved / PointerButton(Left)
                    │
                    ▼
              现有 ui 命中 / 焦点 / 点击 / 拖动
```

| 问题 | P6 约定 |
|------|---------|
| 是否改 `ui` 控件逐个认 Touch？ | **否**；默认走合成指针 |
| 是否删除 `Event::Touch`？ | **否**；手势 / 多指后补仍用 |
| 多指捏合？ | **不做**（非目标） |
| 与 Immediate 视口？ | 合成指针仍受现有 contains/focus 规则约束 |
| 无触屏 CI？ | 单测喂 `TouchEvent`；有显示再手工验 |

---

## 5. 验收总表

| 切片 | 通过样子 |
|------|----------|
| P6a ✅ | 主触点合成指针；单测覆盖 Started/Moved/Ended/Cancelled + 次指不合成；linux/macos 已接；触屏手工验可选 |
| P6b ✅ | `scale_factor` + `ScaleFactorChanged`（linux/macos）；`dpi` 助手与单测；契约成文；示例多用物理、逻辑换算可选 |
| P6c | Windows Soft 窗口 + 指针/键盘；`platform` feature 可检；Linux/macOS 不回归 |
| P6d | 焦序与可访问名基线有文档/示例；不宣称完整读屏 |
| P6e | soak 步骤可重复；可选探针；CI 不强制长跑 |
| 全局 | Field Soft 仍可无 GPU 启动；无满血 QML；Cascader / Office P2 / 自绘 IME 未混入本阶段必达 |

---

## 6. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 触控与鼠标同时产生双击 | 合成时抑制重复；文档说明主指针源 |
| Wayland / 无触屏开发机 | 单测 + 可选 evdev/仿真；验收不绑特定硬件型号 |
| DPI 中途改 API 导致控件返工 | P6b 先定契约再广改 layout |
| Windows 一次性对标全部 Office 示例 | P6c 只申明 P0 对等；示例矩阵后续加厚 |
| a11y 范围膨胀 | P6d 书面非目标：无完整总线 |
| soak 误进默认 CI | 仅文档 / 显式 env |

---

## 7. 与可选并行工作的边界

以下**可并行**，但**不是 P6 必达**，互不阻塞：

- Office P2 控件（catalog later 层）
- 复杂控件进 `.ngui`
- 自绘 IME 候选
- Cascader

合入时勿改写本文件的非目标表来「顺带」扩 scope。

---

## 8. 链接

| 文档 | 用途 |
|------|------|
| [roadmap.md](roadmap.md) | P6 阶段入口 |
| [architecture.md](architecture.md) | platform 顺序、输入范围 |
| [requirements.md](requirements.md) | 工业约束、OS 顺序、成功标准 |
| [platform-macos.md](platform-macos.md) | macOS 缺口；P6 跟进项 |
| [p5-design.md](p5-design.md) | 前一阶段；Immediate 边界 |
| [office-line.md](office-line.md) | Office 加厚；下一步指向 P6 |
| [office-widget-catalog.md](office-widget-catalog.md) | P2 控件不在本阶段 |

下一步：**P6c**（Windows 后端骨架）→ P6d → P6e。每切片合入后回写本节勾选状态。
