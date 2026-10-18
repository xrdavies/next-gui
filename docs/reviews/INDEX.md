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

| R-20261018-7c9427cf6006-F03 | open | P1 | grok | shared / macOS observed | P0 exit; a69e733 / 7c9427c contracts; 33700c4 caller migration | e13 map_key / Escape handler | Consumed Escape fixed; e13 drops Escape before quit branch | [F03](R-20261018-7c9427cf6006.md#r-20261018-7c9427cf6006-f03) | 33700c4d0fe5 (partial) | — |
| R-20261018-7c9427cf6006-F04 | verified | P1 | grok | shared GPU / Metal observed | 69e6fb2 | render-gpu surface format/blit | Native Surface colors match tested Soft/readback paths | [F04](R-20261018-7c9427cf6006.md#r-20261018-7c9427cf6006-f04) | 33700c4d0fe5 | 0e892e6f7b78 |
| R-20261018-7c9427cf6006-F06 | verified | P1 | grok | shared UI / macOS observed | 454428e | UI paint_datetime_popup | Calendar days/actions now render above later widgets | [F06](R-20261018-7c9427cf6006.md#r-20261018-7c9427cf6006-f06) | 33700c4d0fe5 | 0e892e6f7b78 |
| R-20261018-7c9427cf6006-F02 | verified | P1 | codex | macOS | bd296561 | platform root run export | macOS-only consumer lacked common entry point | [F02](R-20261018-7c9427cf6006.md#r-20261018-7c9427cf6006-f02) | 847b4c4d1a1d | 847b4c4d1a1d |
| R-20261018-0e892e6f7b78-F01 | open | P2 | grok | shared Linux/macOS | 33700c4d0fe5 | platform keyboard-only exit flag check | request_exit outside key callbacks waits for unrelated key | [F01](R-20261018-0e892e6f7b78.md#r-20261018-0e892e6f7b78-f01) | — | — |
| R-20261018-7c9427cf6006-F07 | open | P2 | grok | shared UI | 02f5656; partial fix 33700c4 | UI paint_text_area | Newlines and short caret fixed; wrapped caret is clipped | [F07](R-20261018-7c9427cf6006.md#r-20261018-7c9427cf6006-f07) | 33700c4d0fe5 (partial) | — |
| R-20261018-7c9427cf6006-F05 | verified | P2 | grok | shared | multiple (see report) | render/text/UI clippy; Drawer assertion | Workspace clippy passes; Drawer assertion is concrete | [F05](R-20261018-7c9427cf6006.md#r-20261018-7c9427cf6006-f05) | 33700c4d0fe5 | 0e892e6f7b78 |
| R-20261018-7c9427cf6006-F01 | verified | P2 | codex | macOS / portable QA | 02f5656 and e22–e25 additions | UI raster tests; e21–e25 QA writers | QA images used Linux-only /workspace path | [F01](R-20261018-7c9427cf6006.md#r-20261018-7c9427cf6006-f01) | 2eded00e6748 | 847b4c4d1a1d |

## Review runs

| Date | Reviewed range / snapshot | Agent | Result | Report |
|---|---|---|---|---|

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
