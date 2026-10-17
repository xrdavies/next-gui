# Review: `SHA12`

- Reviewer: Codex
- Date: YYYY-MM-DDTHH:MM:SS+09:00
- Reviewed base: `FULL_BASE_SHA` or `none (initial snapshot)`
- Reviewed target: `FULL_TARGET_SHA`
- Reviewed range: `BASE..TARGET` or `initial snapshot at TARGET`
- Host: macOS version, architecture, Rust version
- Result: pass / issues / blocked

## Checks

Replace every `not_run` with the actual outcome, or explain why the check could not run.
Keep a short output excerpt or a path to evidence for failures. Local log paths are not
shared across machines, so include enough evidence in this report for Grok to act.

| Command / check | Host | Result | Evidence / reason |
|---|---|---|---|
| `cargo test --workspace` | macOS | not_run | — |
| `cargo clippy --workspace --all-targets -- -D warnings` | macOS | not_run | — |
| `cargo check -p next-gui-platform --features macos` | macOS | not_run | — |
| Build current workspace example packages | macOS | not_run | List packages and command |
| Relevant GUI examples: startup / render / input | macOS | not_run | List each example and each acceptance step |
| `git diff --check` | macOS | not_run | — |
| Linux / Windows runtime checks | Actual host, if any | not_run | Do not imply they ran on macOS |

Check outcomes are `passed`, `failed`, or `not_run`. Compile success and a process that
stays alive do not prove correct rendering or input. State any remaining acceptance gap.

## macOS parity

- Linux/reference behavior inspected: —
- Missing macOS behavior: —
- Implementation and affected callers/config/examples: —
- Verification of the implementation: —

## Result

Summarize findings, failed checks, and unavailable acceptance. A clean review must say
"No findings found." Remove the finding skeleton below when there are no findings.

## Findings

### R-YYYYMMDD-SHA12-F01

**Short, concrete title**

- Status: open
- Severity: P1
- Owner: grok
- Platform: all
- Location: `path/to/file.rs:line` and symbol name
- Introduced by: `SHA`, or unknown with a reason

#### Finding

Describe the triggering condition and the incorrect behavior.

#### Evidence

Include the smallest reproduction and actual output or the connected code path that
proves the problem. Distinguish observed failures from code-based conclusions.

```text
Reproduction or failing test output
```

#### Impact

Explain the affected users, platforms, or code paths.

#### Recommendation

Describe the smallest acceptable fix and how to verify it.

#### Resolution

Grok fills this section and changes both report/index to `fixed`.

- Fixed by: —
- Fix commit: SHA, or `Fixes-Review: R-YYYYMMDD-SHA12-F01` until the SHA is known
- Changes and tests: —
- Test commands and outcomes: —

#### Verification

Codex fills this section after re-testing. Use the tested fix SHA, not the future SHA
of the commit that will contain this verification text.

- Verified by: —
- Tested fix commit: —
- Commands and outcomes: —
- Outcome: verified / reopened / blocked
- Reason and remaining gap: —

## Verification of earlier findings

Reference existing IDs and update their original reports and index rows in the same
commit. Do not create new IDs for the same problem.

| Finding | Tested fix SHA | Commands / evidence | Previous status | New status |
|---|---|---|---|---|

## Handoff

List the next actions for Grok and any unresolved macOS work for Codex.
