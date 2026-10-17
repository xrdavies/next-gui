# Office 控件目录（对照 Ant Design / Element Plus / MUI）

> **状态：已确认（2026-10-17）。** Office 线「支持哪些控件、优先级如何」已锁定。本文以目录与优先级为主；**Shell 切片**（Icon / Toolbar / Panel / StatusBar / Splitter）与 **Input 切片**（ScrollArea / TextInput / Checkbox / Radio / Switch）以及 **Data 切片**（List / Tree / Table）与 **Nav 切片**（Menu / ContextMenu / Tabs / Select / Slider）已在 `next-gui-ui` + `examples/office_shell` / `office_inputs` / `office_data` / `office_nav` 落地。
>
> 配套：[office-line.md](office-line.md)（当前 Office 交付）、[requirements.md](requirements.md)、[architecture.md](architecture.md)。

---

## 1. 目的与用法

1. 对照主流 Web 前端组件库的**官方组件总览**，收敛出 next-gui **Office 档**建议支持的控件清单。
2. 用稳定英文 id 作为未来 `.ngui` / Rust API 名；中文名为产品侧称呼。
3. 按 **P0 / P1 / P2 / 范围外** 分层，方便一次确认 MVP，再分期实现。
4. 明确裁掉偏 Web 营销/滚动页的控件；补上工业 Office（SCADA/HMI 侧配置、监控、规划桌面）常用但三库偏弱的能力。

**用法：** §4「锁定 MVP（P0）」为已拍板名单；§5–§7 为全量对照与工业扩展；§10 记录拍板结论。改优先级直接在本文改表并更新修订记录。

---

## 2. 参考来源（官方总览）

| 库 | 官方组件总览 | 备注 |
|----|--------------|------|
| **Ant Design**（React） | https://ant.design/components/overview/ | 分类：General / Layout / Navigation / Data Entry / Data Display / Feedback / Other |
| **Element Plus**（Vue） | https://element-plus.org/en-US/component/overview.html | 分类：Basic / Form / Data / Navigation / Others；另有虚拟化 Table/Tree/Select |
| **Material UI**（React） | https://mui.com/material-ui/all-components/ | 分类：Inputs / Data display / Feedback / Surfaces / Navigation / Layout；Date/Tree/Data Grid 多在 MUI X |

可选扫过缺口（未逐条列入对照列）：Naive UI、Arco Design——与三库重叠度高，工业缺口主要靠本文 §7 补，而非再扩 Web 清单。

**对照列约定：** `Ant` / `EP` / `MUI` 单元格为 `Y`（有一等公民组件）、`~`（可组合或在扩展包如 MUI X）、`-`（无直接对应）。名称按各库习惯映射，不要求 API 同构。

---

## 3. 优先级与假设

| 档 | 含义 |
|----|------|
| **P0** | Office MVP **必须有**；没有则难搭「工具条 + 面板 + 表单 + 表格/树」型办公客户端 |
| **P1** | 很快需要；配置页、日志、厂站层级、告警与轻量可视化 |
| **P2** | 可后补；增强体验或低频场景 |
| **范围外** | 不做进 Office MVP（可永远不做，或等极后期）；多为 Web 页特有交互 |

**已有（实现侧，非本文目标）：** `Text`、`Button`、`Column`、`Row`、`Spacer`、`Separator`（见 office-line / P1）。目录里仍列出，标「已有」。

**仍成立的假设：**

1. 分类沿用 Ant 式六类 +「工业扩展」，便于对照，不绑定 Web 实现。
2. Office 与 Field **共享控件语义**；观感/预算差在 profile，不在两套控件树。
3. P0 输入仍是**指针 + 键盘**；`TextInput` 先可编辑拉丁/ASCII，**完整 IME/CJK 输入**可跟进但不阻塞录入控件落地。
4. 声明式 + retained；控件是长期节点，不是每帧 Immediate 重建（Immediate 视口见 P5，不在本目录）。
5. Dock 多窗格先做 **Splitter + Panel**，不做完整 IDE 级停靠协议。
6. 图表类（Gauge / Trend）走 retained 节点 + 可选 GPU 画动态内容，不引入 Web 图表壳；**仍属 P1，不抬到 P0**。

---

## 4. 锁定 MVP（P0）一览

**已拍板。** 实现必须覆盖本表（含 `text_input` + 基础 `table`/`tree`，以及一等公民 `toolbar` / `status_bar` / `panel`）；不是「先只做壳」。细节见 §5；拍板结论见 §10。

| id | 中文 | 备注 |
|----|------|------|
| `text` | 文本 | **已有** |
| `button` | 按钮 | **已有** |
| `column` / `row` | 纵向/横向布局 | **已有** |
| `spacer` / `separator` | 占位 / 分割线 | **已有** |
| `icon` | 图标 | 工具条/状态必需 |
| `toolbar` | 工具条 | **一等公民（已拍板）**；非仅 Row 组合约定 |
| `menu` | 菜单 | 应用菜单 / 下拉 |
| `context_menu` | 右键菜单 | 工业桌面高频 |
| `tabs` | 标签页 | 多页配置/监视 |
| `splitter` | 分隔面板 | 左树右表经典布局 |
| `panel` | 面板 | **一等公民（已拍板）**；带标题的内容容器（Dock 轻量） |
| `scroll_area` | 滚动区域 | 长列表/属性页 |
| `text_input` | 单行输入 | **P0 必含（已拍板）**；配置项；IME 可分期 |
| `checkbox` | 复选框 | |
| `radio` | 单选框 | |
| `switch` | 开关 | 使能类设置 |
| `select` | 选择器 | 下拉枚举 |
| `slider` | 滑块 | 阈值/阈值类 |
| `list` | 列表 | 简单选择列表；虚拟化见 P1 |
| `table` | 表格 | **P0 必含（已拍板）**；基础行列；虚拟化/树表见 P1 |
| `tree` | 树 | **P0 必含（已拍板）**；设备/厂站层级 |
| `tag` | 标签/状态片 | 运行/故障/旁路等 |
| `badge` | 徽标 | 未读告警数等 |
| `tooltip` | 提示 | 工具条图标说明 |
| `modal` | 对话框 | 配置确认/详情 |
| `progress` | 进度条 | 下发/加载 |
| `alert` | 警告条 | 页内状态条 |
| `status_bar` | 状态栏 | **一等公民（已拍板）**；窗底连接/时钟/模式 |
| `status_bar` 旁：`label` | 标签 | 与 `text` 可合并；表单左侧说明 |

**P0 合计（去重后约）：** 布局已有 5 + 交互/录入/展示约 20+ 项（含输入与基础表/树）。**不含** Form 校验引擎、DateTime、告警专用表、图表（`alarm_list` / `gauge` / `trend` 仍 P1）。层级选择器 P1 只落 `tree_select`（见 §10）。

---

## 5. 全量对照表

图例：优先级列见 §3。`fit` 一行列 next-gui 适配要点。

### 5.1 通用（General）

| id | 中文 | Ant | EP | MUI | 优先级 | fit |
|----|------|-----|----|-----|--------|-----|
| `button` | 按钮 | Y | Y | Y | **P0·已有** | Field/Office 共用；指针点击 P0 |
| `icon` | 图标 | Y | Y | Y | **P0** | 矢量/位图均可；工具条密集区 |
| `text` / `typography` | 文本 | Y | Y | Y | **P0·已有** | CJK 已在 P2；可编辑见 `text_input` |
| `button_group` | 按钮组 | ~ | ~ | Y | P2 | 可用 Row+Button 组合 |
| `float_button` / FAB | 悬浮按钮 | Y | - | Y | **范围外** | Web/移动主操作；桌面 Office 用工具条 |
| `link` | 链接 | ~ | Y | Y | P2 | 桌面少用；可用 button 文本样式 |

### 5.2 布局（Layout）

| id | 中文 | Ant | EP | MUI | 优先级 | fit |
|----|------|-----|----|-----|--------|-----|
| `column` | 纵向布局 | ~ | ~ | ~ | **P0·已有** | flex 主轴 |
| `row` | 横向布局 | ~ | ~ | ~ | **P0·已有** | 工具条基础 |
| `spacer` | 占位 | ~ | ~ | ~ | **P0·已有** | |
| `separator` / `divider` | 分割线 | Y | Y | Y | **P0·已有** | |
| `space` | 间距容器 | Y | Y | Y(Stack) | P2 | 可用 spacer/padding 表达 |
| `grid` | 栅格 | Y | Y | Y | P2 | 办公桌面优先 flex+splitter |
| `splitter` | 分隔面板 | Y | Y | ~ | **P0** | 左导航/右内容；retained 拖拽分割 |
| `panel` | 面板 | ~ | ~ | Y(Paper/Card) | **P0** | **一等公民（已拍板）**；标题+内容；Dock 轻量 chrome |
| `scroll_area` | 滚动区域 | ~ | Y | ~ | **P0** | 指针滚轮+拖条；虚拟化另列 |
| `card` | 卡片 | Y | Y | Y | P1 | 与 panel 可合并或作为样式变体 |
| `layout_shell` | 页框架 | Y | Y(Container) | Y(AppBar…) | P1 | 顶栏+侧栏+内容约定，非必须独立控件 |
| `masonry` | 瀑布流 | Y | - | Y | **范围外** | 营销墙；非 HMI |
| `flex` 抽象 | 弹性 | Y | Y | Y | P1 | 已有 Row/Column；属性补齐即可 |

### 5.3 导航（Navigation）

| id | 中文 | Ant | EP | MUI | 优先级 | fit |
|----|------|-----|----|-----|--------|-----|
| `toolbar` | 工具条 | ~ | ~ | ~ | **P0** | **一等公民（已拍板）**；Office 线已有 Row 示范 |
| `menu` | 菜单 | Y | Y | Y | **P0** | 键盘导航预留 |
| `dropdown` | 下拉菜单 | Y | Y | Y | **P0** | 可与 menu 同一套弹出层 |
| `context_menu` | 右键菜单 | ~ | ~ | ~ | **P0** | 三库弱、桌面强；指针右键 P0 |
| `tabs` | 标签页 | Y | Y | Y | **P0** | 多文档/多配置页 |
| `breadcrumb` | 面包屑 | Y | Y | Y | P1 | 厂站路径导航 |
| `pagination` | 分页 | Y | Y | Y | P1 | 大表；可与虚拟化二选一策略 |
| `steps` | 步骤条 | Y | Y | Y | P2 | 向导式配置 |
| `anchor` | 页内锚点 | Y | Y | - | **范围外** | 长文档 Web 页 |
| `bottom_nav` | 底部导航 | - | - | Y | **范围外** | 移动端 |
| `speed_dial` | 速度拨号 | - | - | Y | **范围外** | 同 FAB |

### 5.4 数据录入（Data Entry）

| id | 中文 | Ant | EP | MUI | 优先级 | fit |
|----|------|-----|----|-----|--------|-----|
| `text_input` | 单行输入 | Y | Y | Y | **P0** | **必含（已拍板）**；焦点环+键盘；IME 可分期 |
| `text_area` | 多行输入 | Y | Y | ~ | P1 | 备注/脚本片段 |
| `checkbox` | 复选 | Y | Y | Y | **P0** | |
| `radio` | 单选 | Y | Y | Y | **P0** | |
| `switch` | 开关 | Y | Y | Y | **P0** | |
| `select` | 选择器 | Y | Y | Y | **P0** | |
| `slider` | 滑块 | Y | Y | Y | **P0** | 与 Field 设定值场景共享 |
| `input_number` | 数字输入 | Y | Y | ~ | P1 | 工程量；步进/范围校验 |
| `form` | 表单+校验 | Y | Y | ~ | P1 | **校验引擎 P1（已拍板）**；P0 仅独立录入控件，校验由应用代码处理 |
| `datetime` | 日期时间 | Y | Y | ~(X) | P1 | 日志检索/计划；Office 更刚需 |
| `cascader` | 级联选择 | Y | Y | - | P2 / 可选 | **P1 不单独落**；厂站层级用 `tree_select`；必要时作别名或后补 |
| `tree_select` | 树选择 | Y | Y | ~(X) | **P1（已拍板唯一）** | 厂站/设备树选点；优于多级 Cascader |
| `transfer` | 穿梭框 | Y | Y | Y | P1 | 权限/点表批量选择 |
| `upload` | 上传/文件 | Y | Y | - | P2 | 配置导入；走系统文件对话框 |
| `color_picker` | 颜色 | Y | Y | - | P2 | 趋势笔色等 |
| `autocomplete` | 自动完成 | Y | Y | Y | P2 | |
| `rate` | 评分 | Y | Y | Y | **范围外** | 消费级 |
| `mentions` | 提及 | Y | Y | - | **范围外** | 社交/协作 Web |
| `input_otp` | 验证码 | - | Y | - | **范围外** | |
| `segmented` | 分段控制 | Y | Y | Y(Toggle) | P1 | 视图模式切换（表/图） |

### 5.5 数据展示（Data Display）

| id | 中文 | Ant | EP | MUI | 优先级 | fit |
|----|------|-----|----|-----|--------|-----|
| `list` | 列表 | Y* | Y | Y | **P0** | *Ant List 已弃用倾向虚拟列表；我们要稳定 List |
| `table` | 表格 | Y | Y | Y | **P0** | **必含（已拍板）**；排序/选择列 P1 增强 |
| `tree` | 树 | Y | Y | ~(X) | **P0** | **必含（已拍板）**；设备/厂站层级 |
| `tag` | 标签 | Y | Y | Y(Chip) | **P0** | 状态色语义（运行/停机/告警） |
| `badge` | 徽标 | Y | Y | Y | **P0** | |
| `tooltip` | 文字提示 | Y | Y | Y | **P0** | |
| `progress` | 进度 | Y | Y | Y | **P0** | |
| `empty` | 空状态 | Y | Y | - | P1 | |
| `collapse` | 折叠面板 | Y | Y | Y(Accord.) | P1 | 属性分组 |
| `descriptions` | 描述列表 | Y | Y | - | P1 | 只读详情；近 property_grid |
| `statistic` | 统计数值 | Y | Y | - | P2 | KPI 卡片 |
| `timeline` | 时间轴 | Y | Y | Y | P2 | 事件流；告警可专用控件 |
| `calendar` | 日历 | Y | Y | ~(X) | P2 | |
| `image` | 图片 | Y | Y | ~ | P2 | 示意图/快照 |
| `avatar` | 头像 | Y | Y | Y | **范围外** | 账号产品向 |
| `carousel` | 走马灯 | Y | Y | - | **范围外** | |
| `popover` | 气泡卡片 | Y | Y | - | P2 | 富内容提示 |
| `qrcode` | 二维码 | Y | - | - | **范围外** | 非核心 |
| `tour` | 漫游引导 | Y | Y | - | **范围外** | 营销 onboarding |

### 5.6 反馈（Feedback）

| id | 中文 | Ant | EP | MUI | 优先级 | fit |
|----|------|-----|----|-----|--------|-----|
| `modal` / `dialog` | 对话框 | Y | Y | Y | **P0** | 模态层；焦点陷阱 |
| `alert` | 警告提示 | Y | Y | Y | **P0** | 页内横幅 |
| `drawer` | 抽屉 | Y | Y | Y | P1 | 侧滑配置 |
| `notification` / `message` | 通知/消息 | Y | Y | Y(Snack) | P1 | 全局队列；勿滥动画 |
| `popconfirm` | 气泡确认 | Y | Y | - | P2 | 可用 modal 替代 |
| `result` | 结果页 | Y | Y | - | P2 | |
| `spin` / `loading` | 加载中 | Y | Y | Y | P1 | 局部遮罩 |
| `skeleton` | 骨架屏 | Y | Y | Y | **范围外** | Web 首屏占位；本地应用次要 |
| `watermark` | 水印 | Y | Y | - | **范围外** | 合规场景极少且可用层叠自行做 |
| `backdrop` | 遮罩 | - | - | Y | P1 | modal/drawer 基础设施 |

### 5.7 其他 / 基础设施

| id | 中文 | Ant | EP | MUI | 优先级 | fit |
|----|------|-----|----|-----|--------|-----|
| `status_bar` | 状态栏 | - | - | - | **P0** | **一等公民（已拍板）**；三库皆弱；桌面 Office 刚需 |
| `affix` | 固钉 | Y | Y | - | **范围外** | 滚动页 |
| `backtop` | 回到顶部 | - | Y | - | **范围外** | |
| `config_provider` | 全局配置 | Y | Y | ~ | P2 | 主题/locale；对标设计 token，非 Web Context 照搬 |
| `border_beam` 等装饰 | 装饰特效 | Y | - | - | **范围外** | |

---

## 6. P1 / P2 汇总（便于排期）

### 6.1 P1（soon）

`card`，`breadcrumb`，`pagination`，`text_area`，`input_number`，`form`（含校验引擎），`datetime`，**`tree_select`（P1 唯一层级选择器）**，`transfer`，`segmented`，`empty`，`collapse`，`descriptions`，`drawer`，`notification`，`spin`，`backdrop`，以及 §7 工业扩展中的：`property_grid`，`tree_table`，`virtual_list`，`alarm_list`，`gauge`，`trend`。

> `cascader`：**不进 P1 实现清单**；需要时后补或作 `tree_select` 别名（见 §10）。

### 6.2 P2（later）

`button_group`，`link`，`space`，`grid`，`steps`，`upload`，`color_picker`，`autocomplete`，`statistic`，`timeline`，`calendar`，`image`，`popover`，`popconfirm`，`result`，`config_provider`，以及可选的 `cascader`（若未作为 `tree_select` 别名吸收）。

---

## 7. 工业扩展（三库偏弱或缺失）

这些是 Office/SCADA 侧常见、但 Ant/EP/MUI **没有一等公民或明显偏 Web** 的能力。建议单独排期，不假装「Web 组件库已覆盖」。

| id | 中文 | 建议优先级 | 说明 |
|----|------|------------|------|
| `status_bar` | 状态栏 | **P0** | **一等公民（已拍板）**；连接态、服务器时间、当前用户/模式 |
| `toolbar` | 工具条 | **P0** | **一等公民（已拍板）**；图标按钮+分组+溢出菜单 |
| `context_menu` | 右键菜单 | **P0** | 树节点/表行操作 |
| `panel` + dock 约定 | 停靠面板 | **P0→P1** | **Panel 一等公民（已拍板）**；P0 = Panel+Splitter；多面板拖拽停靠 P1+ |
| `property_grid` | 属性网格 | P1 | 选中对象键值编辑；近 Descriptions+Form |
| `tree_table` | 树表 | P1 | 层级+列；设备点表常见 |
| `virtual_list` / `virtual_table` | 虚拟化列表/表 | P1 | 万级点名/日志；EP 有 TableV2/TreeV2 可对标语义 |
| `alarm_list` | 告警列表 | **P1（不抬 P0）** | 确认/分级/闪烁语义；非普通 Table 皮肤 |
| `tag`（状态语义） | 状态片 | **P0** | 与设计 token 绑定色（运行/故障/旁路） |
| `gauge` | 仪表盘 | **P1（不抬 P0）** | 实时量；GPU 可选画动态 |
| `trend` / `sparkline` | 趋势/迷你图 | **P1（不抬 P0）** | 历史曲线；Office 监视核心 |
| `splitter` | 分隔面板 | **P0** | 见布局表 |

---

## 8. 明确裁掉或降级（Web-only / 低契合）

默认 **范围外**（除非未来有强产品理由）：

- Affix、BackTop、Watermark、FloatButton / FAB、Speed Dial、Bottom Navigation  
- Tour、Skeleton（作为主加载范式）、Carousel、Masonry、BorderBeam  
- Rate、Mentions、QRCode、Input OTP、Avatar（账号向）  
- Anchor（长文页内锚点）、Infinite Scroll（营销信息流）

降级为「可组合、不单列 P0」：`button_group`、`space`、`link`、复杂 `layout_shell`。

---

## 9. 与实现状态的关系

| 状态 | 控件 |
|------|------|
| 已实现（Office 线/P1） | `text`，`button`，`column`，`row`，`spacer`，`separator` |
| 锁定下一步实现（P0 缺口） | `icon`，`toolbar`，`menu`/`dropdown`，`context_menu`，`tabs`，`splitter`，`panel`，`scroll_area`，`text_input`，`checkbox`，`radio`，`switch`，`select`，`slider`，`list`，`table`，`tree`，`tag`，`badge`，`tooltip`，`modal`，`progress`，`alert`，`status_bar` |
| 未实现且非本文范围 | 动画系统、完整 IME、Immediate 视口、Profile crate（P4） |

实现时仍遵守：同一 `ui` / `scene` / `DrawCommand`；GPU 可选；Soft 可回退。

---

## 10. 拍板结论（2026-10-17）

| # | 议题 | 结论 |
|---|------|------|
| 1 | P0 是否含 `text_input` + 基础 `table`/`tree` | **必须包含。** 输入很重要；不是先只做壳再补数据控件。 |
| 2 | `toolbar` / `status_bar` / `panel` | **升为一等公民**，列入 P0（非仅 Row/组合约定）。 |
| 3 | `cascader` vs `tree_select` | **P1 只落 `tree_select`**（更贴厂站/设备树数据）。`cascader` 后补或作别名/可选（P2）。 |
| 4 | `alarm_list` / `gauge` / `trend` | **不抬到 P0**，保持 **P1**。 |
| 5 | Form 校验 | **延后到 P1**；P0 只做独立录入控件，校验由应用代码处理。 |

其余原 §4 P0 MVP 名单保持不变（已有布局 5 件 + Icon / Menu / ContextMenu / Tabs / Splitter / ScrollArea / Checkbox / Radio / Switch / Select / Slider / List / Tag / Badge / Tooltip / Modal / Progress / Alert 等）。

---

## 11. 修订记录

| 日期 | 说明 |
|------|------|
| 2026-10-17 | 初稿：对照 Ant Design / Element Plus / MUI 官方总览，整理 Office 优先级与工业扩展；待确认。 |
| 2026-10-17 | **拍板锁定：** P0 含 TextInput+基础 Table/Tree；Toolbar/StatusBar/Panel 一等公民；P1 层级选择仅 TreeSelect；AlarmList/Gauge/Trend 仍 P1；Form 校验 P1。 |
| 2026-10-17 | **Shell 切片落地：** `icon` / `toolbar` / `panel` / `status_bar` / `splitter`（Rust API + `office_shell` 示例）；录入/表/树仍待办。 |
