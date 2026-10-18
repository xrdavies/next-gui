# 7×24 / 泄漏 / soak（P6e）

> **状态：已落地。** 对应 [p6-design.md](p6-design.md) §3.5。  
> **非目标：** CI **不**默认跑 24h；不做 AddressSanitizer / 全自动泄漏矩阵；GpuSurface 纹理计数仅为尽力（有显示工具时手工看）。

## 1. 跑什么

推荐 **Soft** 路径、体量小、易观察：

```bash
# 主探针（本切片接好）
NEXT_GUI_SOAK=1 cargo run -p e02_counter

# 可选：唤醒 / 自增间隔（毫秒，默认 1000；非法/0 → 1000；上限 1h）
NEXT_GUI_SOAK=1 NEXT_GUI_SOAK_MS=2000 cargo run -p e02_counter
```

- `NEXT_GUI_SOAK` 未设置时：`e02_counter` 与平台行为与今日完全一致（`ControlFlow::Wait`，无定时 redraw）。
- 开启后：`next-gui-platform` 在 `about_to_wait` 里用 `WaitUntil` 周期性 `request_redraw`；示例每个 tick **自增计数**（走 retained dirty + Soft `present`）。
- Field Soft 兜底在 soak 下仍然有效（本探针即 Softbuffer）。

其它 Soft 示例（`e15_office_feedback`、`e20_office_industrial` 等）也可长挂做试点，但未必接 `NEXT_GUI_SOAK` 自增；可手工操作或另接同一 env。

## 2. 看什么（RSS / 句柄 / 纹理）

### Linux（推荐）

示例 stderr 会周期性打 `VmRSS`（读 `/proc/self/status`）。也可另开终端：

```bash
# 找到进程
pgrep -a e02_counter
# 周期性 RSS（kB）
watch -n 60 'grep VmRSS /proc/$(pgrep -n e02_counter)/status'
# 或
pidstat -r -p $(pgrep -n e02_counter) 60
ps -o pid,rss,vsz,cmd -p $(pgrep -n e02_counter)
```

句柄粗看：`ls /proc/<pid>/fd | wc -l`（应在稳态附近波动，不小时级单调暴涨）。

### Windows

任务管理器 → 详细信息 → `e02_counter.exe` → **工作集（内存）**；或 Performance Monitor。句柄列可选。

### macOS

活动监视器 → 内存；或 `ps -o rss= -p <pid>`（单位页，需换算）。

### GpuSurface（尽力）

本探针走 Soft，不绑 GPU。若对 Office 原生 Surface 做 soak，用平台工具看纹理/显存（如 `nvidia-smi`、RenderDoc）；P6e **不**要求自动化纹理计数。

## 3. 「坏」长什么样

| 现象 | 解读 |
|------|------|
| 启动后 RSS 爬升几十分钟后 **平台 / 锯齿稳态** | 常见（缓存、分配器）；记录稳态带即可 |
| **按小时单调上升、无上沿**（例如每小时 +数 MiB～数十 MiB 且不回落） | 视为明显泄漏嫌疑 → 停机、对 diff、查 scene/UI 是否无限堆积 |
| fd / 句柄数同步单调涨 | 资源未关（surface、文件、剪贴板等） |
| Soft 缓冲尺寸随 resize 涨但不随缩回落 | 检查 `resize` / 缓冲复用 |

记录建议：开始时刻、间隔、每 30–60 min 一条 RSS（或贴 stderr soak 行），跑满试点时长（工控常见 8–24h）。

## 4. 范围

| 在 P6e 内 | 不在 P6e |
|-----------|----------|
| 可重复步骤文档 + `NEXT_GUI_SOAK` 可选探针 | 默认 CI 24h job |
| Soft 路径心跳 / RSS 日志（Linux） | 强制 sanitizer / heaptrack 集成 |
| 能凭步骤发现「明显泄漏」 | 完整 Gpu 纹理审计、多示例矩阵必达 |
| Field Soft 在 soak 下仍可启动 | 新 a11y 总线 / Windows 图形会话必验 |

## 5. API 速查

| 项 | 说明 |
|----|------|
| `next_gui_platform::soak::enabled` | 读 `NEXT_GUI_SOAK` |
| `next_gui_platform::soak::interval` | 读 `NEXT_GUI_SOAK_MS` |
| `next_gui_platform::soak::linux_vm_rss_kb` | Linux `VmRSS`；其它 OS → `None` |
| `SoakDriver` | 后端 `about_to_wait` 用；env 未开则不改 control flow |

```bash
cargo test -p next-gui-platform --lib soak
cargo check -p e02_counter
```

## 6. 链接

- [p6-design.md](p6-design.md) §3.5
- [roadmap.md](roadmap.md) P6
- 示例：`examples/02_counter`
