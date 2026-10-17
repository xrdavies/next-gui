# Grok / Codex 跨机器协作

两个 agent 各自使用自己的 clone，通过 `origin/main` 交换代码、review 和验收结果。
Grok 负责产品开发与 review 修复；Mac 上的 Codex 负责 macOS Office 补齐、代码审阅与复验。
角色由各自启动脚本中的固定提示词声明，不使用角色环境变量。

## 一轮交接

1. Grok 拉取 `main`，读 `AGENTS.md`、[review 索引](reviews/INDEX.md)及相关报告，
   先处理适用的反馈，再开发并测试。提交带 `Agent: Grok`，随后正常 push。
2. Mac 的 LaunchAgent 每 300 秒运行 [Codex 脚本](../tools/codex-review-loop.sh)。
   脚本加本机文件锁，确认分支和工作区，fetch 后以快进方式更新 `main`。
3. 没有新提交时直接退出；有新提交时检查整个待处理区间。只有区间内全部提交都带
   `Agent: Codex` 才跳过模型调用，因此 Grok 的修改不会被后面的 Codex 提交遮住。
   未标记身份的旧提交也会进入审阅。
4. Codex 在固定的目标 SHA 上读需求、设计、路线图，检查 Linux 行为与 macOS Office
   的差距，并补齐必要实现。普通产品问题写入 review，交给 Grok 处理。
5. Codex 执行 workspace 测试、clippy、macOS feature 检查、示例构建及相关 GUI 验收。
   示例清单从当前仓库发现，不固定在 P0/P1。GUI 运行应有时间边界，结束自己启动的进程；
   构建、启动、画面和输入分别记录，不能以“进程没退出”代替交互验收。
6. Codex 按[模板](reviews/REPORT_TEMPLATE.md)创建报告，更新索引并提交。
   即使没有发现问题，也必须保存审阅范围和检查结果。最后一个提交包含
   `Agent: Codex` 和 `Reviewed-Commit: <完整目标 SHA>`，随后正常 push。
7. 脚本检查报告与索引确实随提交更新、新增提交的身份标记正确，并确认提交已在远端。
   通过后才原子更新本地 `last-seen`。这验证了交接产物；测试结论仍以报告中的实际证据为准。
8. Grok 下一轮 pull 后读取索引和详情，修复问题并标为 `fixed`。Codex 下一轮重新测试，
   成功改为 `verified`，失败改回 `open` 并补充证据。

`launchd` 启动的是新的非交互 `codex exec` 进程，不会唤醒已有聊天会话，也不继承本次
聊天记录。长期规则必须写在仓库文件和固定提示词中。Grok 的触发器由 Grok 自己实现；
本仓库暂不包含 Grok 启动脚本。

## 谁在什么时候写什么

| 文件 | Grok | Codex |
|---|---|---|
| `AGENTS.md` | 启动前阅读 | 启动前阅读 |
| `docs/reviews/INDEX.md` | 更新处理状态与修复证据 | 增加 review 记录、问题及复验状态 |
| `docs/reviews/R-YYYYMMDD-SHA12.md` | 填写 Resolution | 填写发现、检查结果及 Verification |
| 产品代码 | 开发、修复普通问题 | 实现 macOS 必要差距 |
| 本机 `last-seen` | 不访问 | 仅轮询脚本在交接校验后更新 |

这里的顺序依靠两个 agent 的工作约定，不是跨机器分布式锁。本机锁只防止多个 Codex
轮询进程重叠。Grok 应在 Codex push 完成交接后开始下一轮，避免长时间同时写 `main`。
如果确实出现并发，正常 Git push 会拒绝非快进更新，脚本保留现场供恢复。

## Review 约定

报告名由执行日期和本轮目标 SHA 的前 12 位组成，例如
`R-20261017-123456789abc.md`。问题 ID 使用报告前缀加递增序号，例如
`R-20261017-123456789abc-F01`。索引的详情链接指向
`R-20261017-123456789abc.md#r-20261017-123456789abc-f01`。

问题必须包含具体位置、触发条件、证据、影响、修复建议及验证方法。报告中的命令结果
使用 `passed / failed / not_run`；总体结果为 `pass / issues / blocked`。
没有 GUI 会话时写明未完成验收，不能写“验收通过”。Mac 上查看 Linux 代码不等于在
Linux 测试；Windows 同理。失败证据应写入受版本控制的报告，不能只给另一个机器无法
读取的本地日志路径。

| 状态 | 写入者与条件 |
|---|---|
| `open` | Codex 发现问题，或复验失败后重新打开 |
| `in_progress` | 负责修复的 agent 已开始处理 |
| `fixed` | Grok 已提交修复和测试证据，等待 Codex 复验 |
| `verified` | Codex 在具体修复提交上重新测试通过 |
| `blocked` | 当前无法继续，必须说明依赖、负责人和解除条件 |
| `wont_fix` | 有明确理由，并经 reviewer 确认 |

严重级别：P0 为崩溃、数据损坏或无法启动；P1 为功能错误、平台缺失或主要回归；
P2 为普通问题、边界或测试缺口；P3 为低风险维护问题。优先写可执行、可证明的问题，
不把主观偏好当成缺陷。问题 ID 保留，不删除历史证据，也不为同一问题反复创建新 ID。

修复提交可以写：

```text
fix(ui): handle resize invalidation

Agent: Grok
Fixes-Review: R-20261017-123456789abc-F01
```

提交时尚不知道自身 SHA，因此 Grok 可以先用 `Fixes-Review` trailer 作为修复引用，
Codex 复验时再补齐真实修复 SHA。身份 trailer 用于协作路由和审计，不是身份认证。

## 本机运行与恢复

安装、停用、状态初始化及检查命令见 [launchd README](../tools/launchd/README.md)。
使用专用 clone，不要与日常开发工作树同时使用。进度和锁保存在该 clone 的 Git 元数据
目录中（通常是 `.git/codex-review/`），不进入版本控制，也不会与另一个 clone 共用。

- 没有 `last-seen` 时，首轮检查当前完整快照。若只想处理未来提交，先按安装文档登记基线。
- 正常处理后，进度记录本轮 Codex 输出提交。如果随后 Grok 又 push，下一轮仍会处理新增范围。
- 进程失败、工作区变脏、未推送提交、缺少报告/索引或推送失败时，不推进进度。
  脚本不会清理这些工作，也不会调用 `reset --hard`。
- 本地领先或分叉时，停止任务，检查 `git status` 和
  `git log --oneline origin/main..HEAD`，保留并人工处理未推送的结果，再恢复运行。
  不要靠删除进度或硬重置绕过错误。
- 因断电丢失最后一次进度保存时可能重复审阅；先检查远端报告和日志，再决定恢复基线。
  不应仅因 CLI 返回零就跳过缺少产物的审阅。
- 测试失败或环境缺失必须在报告中保留，并创建适用的后续事项；这些结果不代表验收成功。
  Grok 的新修复提交会触发下一轮。若只恢复了环境、没有新提交，轮询不会自动重跑；
  应手动以 Codex 角色补充原报告的验收证据，提交并 push，不需要篡改处理进度。

本次实现不增加 webhook、远端消息通知或跨机器任务服务。轮询只处理可信仓库；
脚本调用的 Codex 使用 `danger-full-access` 和 `never` 审批策略，以完成本机 GUI、测试
和 Git 提交推送，因此应在配置时明确接受这一执行范围。
