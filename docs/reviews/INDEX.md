# Review index

This file is the shared review queue for Grok and Codex. The remote Git repository is the
handoff channel, so update this file in the same commit as the corresponding review or fix.

`open` and `in_progress` findings are actionable. `fixed` findings await Codex verification.
Rows are sorted by severity and newest update.

Process and ownership: [agent workflow](../agent-workflow.md). Report format:
[REPORT_TEMPLATE.md](REPORT_TEMPLATE.md). Remove the empty-state sentences when adding
the first entries. Keep the status in this index and its report synchronized.

## Findings

| ID | Status | Severity | Owner | Platform | Introduced | Location | Summary | Report | Fixed | Verified |
|---|---|---|---|---|---|---|---|---|---|---|
| 2026-10-25 | `3e40bd13dec9..b90d5a7b4143` + full e28 page inventory vs DB4S 3.13.1 | Codex | issues — 13 open findings, 2 repaired groups; 167 tests and clippy pass; 23 Soft captures; native acceptance unavailable | [Report](R-20261025-b90d5a7b4143.md) |
| R-20261025-b90d5a7b4143-F01 | open | P1 | grok | e28 / shared | existing branch | see report | 长 SQL 无法正常定位、滚动和选区 | [F01](R-20261025-b90d5a7b4143.md#r-20261025-b90d5a7b4143-f01) | — | — |
| R-20261025-b90d5a7b4143-F02 | open | P1 | grok | e28 / shared | existing branch | see report | 退出和打开 SQL 文件会丢弃未保存草稿 | [F02](R-20261025-b90d5a7b4143.md#r-20261025-b90d5a7b4143-f02) | — | — |
| R-20261025-b90d5a7b4143-F03 | open | P1 | grok | e28 / shared | existing branch | see report | PRAGMA 未生效却显示应用成功 | [F03](R-20261025-b90d5a7b4143.md#r-20261025-b90d5a7b4143-f03) | — | — |
| R-20261025-b90d5a7b4143-F04 | open | P1 | grok | e28 / shared | existing branch | see report | Replace 替换第一处，而不是 Find Next 找到的匹配 | [F04](R-20261025-b90d5a7b4143.md#r-20261025-b90d5a7b4143-f04) | — | — |
| R-20261025-b90d5a7b4143-F05 | open | P1 | grok | e28 / shared | existing branch | see report | 合法的隐式/复合外键导航失败或返回错误父记录 | [F05](R-20261025-b90d5a7b4143.md#r-20261025-b90d5a7b4143-f05) | — | — |
| R-20261025-b90d5a7b4143-F06 | open | P1 | grok | e28 / shared | existing branch | see report | 单元格类型会改变，错误的 Hex 草稿会被悄悄丢弃 | [F06](R-20261025-b90d5a7b4143.md#r-20261025-b90d5a7b4143-f06) | — | — |
| R-20261025-b90d5a7b4143-F07 | open | P1 | grok | e28 / shared | existing branch | see report | Plot 把文本画成 0 并隐藏数据库错误 | [F07](R-20261025-b90d5a7b4143.md#r-20261025-b90d5a7b4143-f07) | — | — |
| R-20261025-b90d5a7b4143-F08 | open | P1 | grok | e28 / shared | existing branch | see report | CSV 导入新表后保留旧表过滤条件，结果为空 | [F08](R-20261025-b90d5a7b4143.md#r-20261025-b90d5a7b4143-f08) | — | — |
| R-20261025-b90d5a7b4143-F09 | open | P1 | grok | e28 / shared | existing branch | see report | SQL Tab 共用结果且 Save as VIEW 与显示结果不绑定 | [F09](R-20261025-b90d5a7b4143.md#r-20261025-b90d5a7b4143-f09) | — | — |
| R-20261025-b90d5a7b4143-F10 | open | P1 | grok | e28 / shared | existing branch | see report | 窄窗口与宽表/多 Tab 的控件超出可见区域 | [F10](R-20261025-b90d5a7b4143.md#r-20261025-b90d5a7b4143-f10) | — | — |
| R-20261025-b90d5a7b4143-F11 | open | P2 | grok | e28 / shared | existing branch | see report | 条件格式、偏好设置与项目状态不一致 | [F11](R-20261025-b90d5a7b4143.md#r-20261025-b90d5a7b4143-f11) | — | — |
| R-20261025-b90d5a7b4143-F12 | open | P2 | grok | e28 / shared | existing branch | see report | 图片预览是 24 格马赛克，无法检查原始图像 | [F12](R-20261025-b90d5a7b4143.md#r-20261025-b90d5a7b4143-f12) | — | — |
| R-20261025-b90d5a7b4143-F13 | open | P2 | grok | e28 / shared | existing branch | see report | SQL Log 不能反映实际执行的完整 SQL | [F13](R-20261025-b90d5a7b4143.md#r-20261025-b90d5a7b4143-f13) | — | — |
| R-20261025-b90d5a7b4143-F14 | verified | P1 | codex | e28 / shared | existing branch | see report | Table resize/clip/hit and named Space repaired | [F14](R-20261025-b90d5a7b4143.md#r-20261025-b90d5a7b4143-f14) | cb727d7 | b90d5a7b4143 |
| R-20261025-b90d5a7b4143-F15 | verified | P1 | codex | e28 / shared | existing branch | see report | Grid rebuild, wizard column drafts and literal sort repaired | [F15](R-20261025-b90d5a7b4143.md#r-20261025-b90d5a7b4143-f15) | b90d5a7 | b90d5a7b4143 |
| R-20261024-42aab06eff27-F01 | verified | P0 | grok | PR #3 / all | PR #3 | e28 db::copy_database_file / state::save_as | Save As omits committed WAL pages; actual copy helper reproduced missing row | [F01](R-20261024-42aab06eff27.md#r-20261024-42aab06eff27-f01) | efefc068d21b | efefc068d21b |
| R-20261024-42aab06eff27-F02 | verified | P0 | grok | PR #3 / all | PR #3 | e28 query_sql / SQL dump exporters | SQL export silently caps each table at 500 rows | [F02](R-20261024-42aab06eff27.md#r-20261024-42aab06eff27-f02) | efefc068d21b | efefc068d21b |
| R-20261024-42aab06eff27-F03 | verified | P0 | grok | PR #3 / all | PR #3 | e28 cell_to_string / SQL dump exporters | SQL export changes empty TEXT to NULL and BLOB bytes to display labels | [F03](R-20261024-42aab06eff27.md#r-20261024-42aab06eff27-f03) | efefc068d21b | efefc068d21b |
| R-20261024-2c8b7c24dff7-F01 | open | P1 | grok | shared UI / main | f3765eaf | UI layout_tabs / paint_widget | Main still affected; feature-branch fix regression-verified on 2026-10-25 | [F01](R-20261024-2c8b7c24dff7.md#r-20261024-2c8b7c24dff7-f01) | — | — |
| R-20261024-2c8b7c24dff7-F02 | open | P2 | grok | shared renderer / main | d5796554 | render encode_node | Rounded clipping changes original geometry; Soft pixel regression reproduced | [F02](R-20261024-2c8b7c24dff7.md#r-20261024-2c8b7c24dff7-f02) | — | — |
| R-20261018-ded2d152ebca-F01 | verified | P1 | codex | macOS / non-Linux compile | 2c0c9f06a1db | platform Linux probe helper and test guards | Unguarded X11/Wayland helper broke macOS builds; target guards repaired | [F01](R-20261018-ded2d152ebca.md#r-20261018-ded2d152ebca-f01) | 11bbe29a2a27 | 11bbe29a2a27 |
| R-20261018-7c9427cf6006-F03 | verified | P1 | grok | shared / macOS observed | P0 exit; a69e733 / 7c9427c contracts; 33700c4 caller migration | e13 map_key / Escape handler | e13 idle Escape exits; consumed Escape keeps examples alive | [F03](R-20261018-7c9427cf6006.md#r-20261018-7c9427cf6006-f03) | 2c0c9f06a1db | 11bbe29a2a27 |
| R-20261018-7c9427cf6006-F04 | verified | P1 | grok | shared GPU / Metal observed | 69e6fb2 | render-gpu surface format/blit | Native Surface colors match tested Soft/readback paths | [F04](R-20261018-7c9427cf6006.md#r-20261018-7c9427cf6006-f04) | 33700c4d0fe5 | 0e892e6f7b78 |
| R-20261018-7c9427cf6006-F06 | verified | P1 | grok | shared UI / macOS observed | 454428e | UI paint_datetime_popup | Calendar days/actions now render above later widgets | [F06](R-20261018-7c9427cf6006.md#r-20261018-7c9427cf6006-f06) | 33700c4d0fe5 | 0e892e6f7b78 |
| R-20261018-7c9427cf6006-F02 | verified | P1 | codex | macOS | bd296561 | platform root run export | macOS-only consumer lacked common entry point | [F02](R-20261018-7c9427cf6006.md#r-20261018-7c9427cf6006-f02) | 847b4c4d1a1d | 847b4c4d1a1d |
| R-20261018-ded2d152ebca-F02 | verified | P2 | grok | Linux GUI test environments | 2c0c9f06a1db | platform request_exit pointer / run_any_thread_x11 | Pointer forces X11; headless Linux skips and macOS regressions pass; desktop evidence from Grok | [F02](R-20261018-ded2d152ebca.md#r-20261018-ded2d152ebca-f02) | 2c0638b2c49d | d84207275ff4 |
| R-20261018-0e892e6f7b78-F01 | verified | P2 | grok | shared Linux/macOS | 33700c4d0fe5 | platform callback exit flag check | Native start/redraw/pointer requests exit without another key | [F01](R-20261018-0e892e6f7b78.md#r-20261018-0e892e6f7b78-f01) | 2c0c9f06a1db | 11bbe29a2a27 |
| R-20261018-7c9427cf6006-F07 | verified | P2 | grok | shared UI | 02f5656; partial fix 33700c4 | UI paint_text_area | Wrapped caret and Left confirmed in Soft and Metal readback | [F07](R-20261018-7c9427cf6006.md#r-20261018-7c9427cf6006-f07) | 2c0c9f06a1db | 11bbe29a2a27 |
| R-20261018-7c9427cf6006-F05 | verified | P2 | grok | shared | multiple (see report) | render/text/UI clippy; Drawer assertion | Workspace clippy passes; Drawer assertion is concrete | [F05](R-20261018-7c9427cf6006.md#r-20261018-7c9427cf6006-f05) | 33700c4d0fe5 | 0e892e6f7b78 |
| R-20261018-7c9427cf6006-F01 | verified | P2 | codex | macOS / portable QA | 02f5656 and e22–e25 additions | UI raster tests; e21–e25 QA writers | QA images used Linux-only /workspace path | [F01](R-20261018-7c9427cf6006.md#r-20261018-7c9427cf6006-f01) | 2eded00e6748 | 847b4c4d1a1d |

## Review runs

| Date | Reviewed range / snapshot | Agent | Result | Report |
|---|---|---|---|---|
| 2026-10-25 | `dae24c2d5fa1..efefc068d21b` plus preserved incoming e28 edits | Codex | blocked acceptance — local core workflows completed; 3 P0 findings verified; 162 tests, clippy, builds and feature checks pass; 12 e28 Soft captures checked; native desktop permissions prevent OS interaction acceptance | [Report](R-20261025-efefc068d21b.md) |
| 2026-10-24 | main `2c8b7c24dff7`; history since `d84207275ff4` | Codex | issues — framework MVP assessment; 2 main findings; 28 isolated Rust tests + 16 tooling tests pass; current remote CI green; local workspace/GUI acceptance unavailable | [Report](R-20261024-2c8b7c24dff7.md) |
| 2026-10-24 | PR #3 selected paths at `42aab06eff27` | Codex | issues — 3 save/export data-safety findings; WAL helper and value encoding reproduced; source trace proves 500-row cap; full PR GUI/integration not run | [Report](R-20261024-42aab06eff27.md) |
| 2026-10-19 | `ec880768d8e4..d84207275ff4` | Codex | pass — F02 verified; forced-X11 path inspected and compiled on Linux; headless platform tests, 60 macOS tests, clippy, feature checks and 17 builds/GUI Escape checks pass | [Report](R-20261019-d84207275ff4.md) |
| 2026-10-19 | `a8163f8b34e7..7aa661f2b219` | Codex | issues — headless/tool guards and worker deadline checked; F02 reopened for mixed Wayland/X11; 60 tests, clippy, macOS checks and 17 builds/GUI Escape checks pass | [Report](R-20261019-7aa661f2b219.md) |
| 2026-10-18 | `24e0008bf5dc..ded2d152ebca` | Codex | issues — original three behavior findings verified; macOS build regression fixed in 11bbe29; 60 tests, clippy, macOS checks and 17 builds/GUI Escape checks pass; Linux test-environment finding open | [Report](R-20261018-ded2d152ebca.md) |
| 2026-10-18 | `96175797e2a9..0e892e6f7b78` | Codex | issues — F04/F05/F06 verified; F03/F07 reopened; 1 new shared finding; 59 tests, clippy and 17 builds pass; GUI failures and formatting differences recorded | [Report](R-20261018-0e892e6f7b78.md) |
| 2026-10-18 | Initial snapshot `7c9427cf60064739140281ed9adf65dc1ee7210b`; incoming `20eb0ee..7c9427c` inspected | Codex | issues — 2 macOS integration gaps fixed/verified; 57 tests and 17 example builds pass; 5 shared findings open; clippy fails | [Report](R-20261018-7c9427cf6006.md) |

## Status and severity

- `open`: found and requires action
- `in_progress`: currently being handled
- `fixed`: a fix and test evidence were supplied
- `verified`: Codex re-tested and confirmed the fix
- `wont_fix`: intentionally retained with a written reason
- `blocked`: waiting for an external dependency
- `P0`: crash, data loss, or cannot start
- `P1`: functional error, platform gap, or major regression
- `P2`: normal bug, edge case, or test gap
- `P3`: low-risk cleanup or maintainability issue

Review run results are `pass` (required checks passed and no open findings), `issues`
(actionable findings or failed checks), or `blocked` (required acceptance could not run).
Only record platforms that were actually tested as tested; a Linux reference read on
macOS is not a Linux runtime test.
