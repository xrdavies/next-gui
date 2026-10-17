# P2 设计：文本引擎 + CJK（软渲染）

> **状态：P2 完成骨架。** 对应 [roadmap.md](roadmap.md) P2；约束见 [requirements.md](requirements.md)、[architecture.md](architecture.md)。
>
> 范围：`text` crate（字体加载 / 度量 / 简单换行 / CJK）、`DrawCommand::BlitAlpha`、软渲染 tint、`ui` Text/Button 标签去掉点阵占位。不含 GPU、复杂脚本 shaping（HarfBuzz）、IME 编辑、触控。

---

## 1. 目标与验收

### 1.1 目标

1. 自研 **`next-gui-text` 接口**：`Font` 加载 TTF/OTF/TTC、度量、行布局、可选按宽换行、栅格化为 alpha mask。
2. 阶段性借用成熟栅格化（`fontdue`）；对外仍是自有 API，后续可换 shaping 实现。
3. **中英混排可读**：默认尝试系统 Noto Sans CJK / PingFang / 微软雅黑等。
4. `ui` 的 Text/Button 经 mask 写入 `scene`；`encode_scene` → `BlitAlpha` → `render-soft`。
5. 仍无 GPU；脏区路径保持。

### 1.2 成功标准

| 项 | 通过样子 |
|----|----------|
| 字体 | `Font::from_path` / `from_bytes` / `load_default_cjk_font` |
| 度量 | `measure_line` / `measure_wrapped`；ascent/descent/line_height |
| CJK | 「你好」「工业」等与 Latin 同框可栅格且非空 mask |
| 渲染 | 软渲染 `BlitAlpha` 按 coverage tint；无 GPU |
| UI | P1 计数器仍可用；点阵 `bitmap_font` 移除 |
| 示例 | `cargo run -p e04_cjk_text` 显示中英标题与按钮 |
| 测试 | `cargo test -p next-gui-text -p next-gui-ui -p next-gui-render-soft` |

### 1.3 本阶段明确不做

- HarfBuzz / 复杂脚本 / 竖排
- 可编辑 TextInput + IME（验收里的「可编辑基线」仅指度量/布局 API 就绪，编辑器控件后补）
- GPU 字形图集
- 内嵌整包 CJK 字体到仓库（依赖系统字体；文档注明包名）

---

## 2. Crate 与依赖

```text
crates/next-gui-text/     # Font, metrics, layout, AlphaMask raster
crates/next-gui-scene/    # Node.mask: Option<AlphaMask>
crates/next-gui-render/   # DrawCommand::BlitAlpha
crates/next-gui-render-soft/
crates/next-gui-ui/       # uses text for measure + paint
examples/04_cjk_text/
```

```text
ui → text, layout, scene
text ↛ ui
encode_scene(scene) → BlitAlpha | FillRect → render-soft
```

硬规则：`text` 不理解 Button/Column；`render-*` 只认 mask/矩形，不懂「字」。

---

## 3. `text` API（P2）

```rust
Font::from_bytes(data, collection_index) -> Result<Font, FontError>
Font::from_path(path, collection_index) -> Result<Font, FontError>
load_default_cjk_font() -> Result<Font, FontError>

font.metrics(px) -> FontMetrics
font.measure_line(text, px) -> (w, h)
font.measure_wrapped(text, px, max_width) -> (w, h)
font.layout(text, px, max_width: Option<f32>) -> TextLayout
font.rasterize_text(text, px, max_width) -> (TextLayout, AlphaMask)
```

布局：从左到右 advance；`\n` 换行；可选按 `max_width` 在字符边界贪心折行。不做 kerning 精调以外的复杂 shaping。

---

## 4. 场景与软渲染

- `Node.mask`：若存在则 `encode_scene` 输出 `BlitAlpha`（不再对该节点输出实心 `FillRect`）。
- Button：背景节点无 mask（实心），子节点挂标签 mask。
- Text：节点 fill = 文字颜色，mask = 整段栅格。

---

## 5. 系统字体候选

见 `DEFAULT_CJK_FONT_CANDIDATES`（Linux Noto CJK TTC index 优先 SC、macOS PingFang、Windows 雅黑/宋体）。

Debian/Ubuntu：`sudo apt install fonts-noto-cjk`

---

## 6. 示例

```bash
cargo run -p e04_cjk_text
```

显示「next-gui P2 — 文本 / CJK」「Hello 工业界面 · 你好世界」与中英按钮；点击更新「计数 Count: N」。

---

## 7. 与总架构对应

| 架构模块 | P2 |
|----------|-----|
| `text` | 新建；fontdue 作栅格后端 |
| `render` / `render-soft` | `BlitAlpha` |
| `ui` | 替换点阵；API `text_styled(..., size_px)` |
| GPU / Immediate / 触控 | 不做 |

修订：随代码演进更新本文。
