# P4 设计：双档 Profile（Field / Office）

> **状态：P4 完成骨架。** 对应 [roadmap.md](roadmap.md) P4；约束见 [requirements.md](requirements.md)、[architecture.md](architecture.md) §6。
>
> 范围：`next-gui-profile` 配置层（特性开关 + 性能预算 + 降级）、示例 `e24_profile_dual` 同一 UI 在 Field ↔ Office 间切换；**不**分叉 `ui` / `scene` / `DrawCommand`。不含 Immediate（P5）、Windows / 触控（P6）、完整打包矩阵 CI。

---

## 1. 目标与验收

### 1.1 目标

1. **同一应用源**可按 Field / Office 两档配置运行；语义仍是一套 retained UI。
2. Field：**强制 Soft**、关 GPU、较低 fps / 图表刷新、关特效；无 GPU 亦可启动。
3. Office：允许 GPU、优先原生 Surface（有适配器时）、更高帧率与动效预算；失败时降级 Soft。
4. `profile` 是 **config layer**，不是第三条 UI 栈；不持有控件、不编码绘制。

### 1.2 成功标准

| 项 | 通过样子 |
|----|----------|
| 预设 | `ProfileConfig::field()` / `office()` 预算与开关符合 architecture §6 |
| 环境 | `NEXT_GUI_PROFILE=field\|office`（默认 Office） |
| 建议 present | Field → Softbuffer；Office + GPU → GpuSurface 建议 |
| 降级 | `degrade(NoGpu)` → force Soft、关特效 |
| 示例 | Space / 按钮切换档位；StatusBar 显示预算；Field 不尝试 GPU |
| 测试 | `cargo test -p next-gui-profile` |
| 编译 | `cargo build -p e24_profile_dual` |

### 1.3 本阶段明确不做

- Immediate 视口（P5）
- Windows 后端 / 触控映射（P6）
- Cascader、自绘 IME 候选
- 完整「Field 裁剪依赖」CI 矩阵（仅 Cargo features 意图 + 文档）
- 把所有 Office 示例强制接 profile（轻接入：新示例 + 文档）

---

## 2. Crate 与依赖

```text
crates/next-gui-profile/     # ProfileConfig / GuiProfile（无 platform / gpu 依赖）
examples/24_profile_dual/    # 同一 UI；Field ↔ Office 切换
```

```text
应用 ──► ProfileConfig ──► 是否 try GPU / WindowAttrs / fps 预算
              │
              ▼
        同一 ui → scene → DrawCommand → Soft | GPU
```

硬规则：`profile` **不**依赖 `render-gpu` / `platform`；应用自行把 `SuggestedPresent` 映射到 `PresentBackend`。这样 Field 打包可裁掉 GPU crate 而不链进 profile。

Cargo features（打包意图）：`field` / `office` — MVP 不强制门控代码；运行时 `ProfileConfig` 为准。

---

## 3. API（P4）

```rust
GuiProfile::{ Field, Office }
SuggestedPresent::{ Softbuffer, GpuSurface }
AnimationBudget::{ Minimal, Full }
DegradationReason::{ NoGpu, ForceSoft, Thermal, Other }

ProfileConfig {
  profile, allow_gpu, prefer_native_surface, max_fps,
  effects_enabled, force_soft, chart_refresh_hz, animation_budget
}
ProfileConfig::field() / office() / from_env()
should_try_gpu() / prefer_gpu_surface_window() / suggested_present()
frame_interval() / chart_interval() / degrade() / toggle() / summary()
```

### 3.1 默认预算

| | Field | Office |
|--|-------|--------|
| force_soft | true | false |
| allow_gpu | false | true |
| prefer_native_surface | false | true |
| max_fps | 24 | 60 |
| chart_refresh_hz | 4 | 15 |
| effects | off | on |
| animation | Minimal | Full |

---

## 4. 示例 `e24_profile_dual`

- Panel + StatusBar 展示档位与预算；Toggle / Space 切换预设。
- 启动：`from_env()`；Office + 适配器 → `PresentBackend::GpuSurface`；Field → Softbuffer + Soft。
- 运行时切换：在同一窗口上 Soft ↔ GPU（与 Office 线 e10 一致）；**不**热重建窗口 present 后端。
- QA：`NEXT_GUI_PROFILE_QA=1` 默认 Field，软路径 dump PPM。

```bash
cargo run -p e24_profile_dual
NEXT_GUI_PROFILE=field cargo run -p e24_profile_dual
NEXT_GUI_PROFILE_QA=1 cargo run -p e24_profile_dual
```

---

## 5. 打包意图（文档级）

| 档 | 依赖意图 |
|----|----------|
| Field | 可 omit `next-gui-render-gpu` / wgpu；仅 Soft + platform Softbuffer |
| Office | 带 `render-gpu`；启动时按 profile 试 native Surface |

MVP 不强制 feature 门控所有示例；后续可将 Office 示例的 gpu 依赖标 optional。

---

## 6. 风险与后续

| 风险 | 缓解 |
|------|------|
| 窗口 present 后端创建后难换 | 启动时按 profile 选；运行时仅换 Soft/GPU 路径 |
| 忙等帧循环 | 示例按 `chart_interval` sleep，Field 保持冷静 |
| 预算未绑死全库动画 | 控件侧后续读 `effects_enabled`；P4 先暴露开关 |

下一步：**P5 Immediate 视口**（已落地，见 [p5-design.md](p5-design.md)）→ P6；Office P2 可选项与打磨可并行。
