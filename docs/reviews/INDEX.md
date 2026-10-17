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

No findings have been recorded yet.

## Review runs

| Date | Reviewed range / snapshot | Agent | Result | Report |
|---|---|---|---|---|

No review runs have been recorded yet.

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
