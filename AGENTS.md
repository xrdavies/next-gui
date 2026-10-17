# Agent guidance for next-gui

The remote Git repository is the only handoff channel between agents. Grok and Codex
take turns on `main`: Grok pushes, Codex reviews and completes macOS parity, then Grok
handles the feedback. See [the workflow](docs/agent-workflow.md) for setup and recovery.

## Startup

Before changing anything:

1. Read this file.
2. Check the branch and confirm that the worktree is clean before updating it.
3. For interactive work, update `main` with `git fetch origin main` and
   `git pull --ff-only`. Stop if there are unpublished commits or divergent history.
4. For an automated run with a pinned target, verify that `HEAD` equals that target.
   The runner already updated `main`; do not pull or rebase during the run.
5. Read `docs/reviews/INDEX.md`.
6. Read applicable `open`, `in_progress`, and `blocked` findings. Codex must also read
   all `fixed` findings, including those assigned to Grok, to verify their fixes.
7. For platform work, read `docs/platform-macos.md` and the relevant design documents.

The launcher or task prompt states the current role explicitly. The Codex script has a
fixed Codex prompt; Grok uses its own launcher. No role environment variable is needed.
Do not infer the role from the hostname, branch name, user name, or commit authors.
If the role is unspecified, clarify it before modifying files. Preserve existing work.

## Grok role

Grok owns product changes and review fixes.

When starting a change:

- Read all applicable review findings first.
- Preserve the finding ID in the implementation commit.
- Add resolution evidence to the corresponding review report.
- Update both the report and index; mark a finding `fixed`, never `verified`.
- If the fix SHA is not known until commit creation, use the `Fixes-Review` trailer as
  the reference. Codex records the actual fix SHA during verification.
- Run the relevant tests before committing.

Grok commits must use Conventional Commits and include:

    Agent: Grok

For a review fix, also include:

    Fixes-Review: R-YYYYMMDD-SHA12-FNN

## Codex role

Codex owns macOS parity, review, and verification.

For every new remote change:

- Inspect the commits since the previous Codex run.
- Compare macOS behavior with the Linux reference implementation.
- Implement missing macOS support when required.
- Run workspace tests, clippy checks, macOS feature checks, and available examples.
- Review the incoming code for bugs, regressions, portability problems, and missing tests.
- Write or update `docs/reviews/INDEX.md`.
- Write a report under `docs/reviews/` using `REPORT_TEMPLATE.md`.
- Mark findings `open` or `verified` only when evidence supports it.
- Keep ordinary product findings assigned to Grok; do not expand the macOS task into
  unrelated product work. Use `codex` as owner for unfinished macOS parity work.
- Record the result of every required check as `passed`, `failed`, or `not_run`, with
  a reason for failures and unavailable checks. Building a GUI is not GUI acceptance.

Codex commits must use Conventional Commits and include:

    Agent: Codex

The final commit of every automated review must also include the full target SHA:

    Reviewed-Commit: <sha>

## Review protocol

Review files:

- Index: `docs/reviews/INDEX.md`
- Report: `docs/reviews/R-YYYYMMDD-SHA12.md`

Finding statuses:

- `open`: found and requires action
- `in_progress`: currently being handled
- `fixed`: Grok supplied a fix and test evidence
- `verified`: Codex re-tested and confirmed the fix
- `wont_fix`: intentionally retained with an explanation
- `blocked`: cannot proceed because of an external dependency

Finding IDs are `R-YYYYMMDD-SHA12-FNN`, using the report date and the first 12 characters
of its reviewed target SHA. Keep IDs stable across fixes. Link the index to the exact
finding heading. New runs may reference earlier findings instead of duplicating them.

Never delete a finding. Record a reason for `blocked` or `wont_fix`; a reviewer must
confirm `wont_fix`. Reopen a failed fix as `open` and preserve the earlier evidence.

## Completion

Before pushing:

- Run the relevant tests.
- Run `git diff --check`.
- Create focused Conventional Commits for the completed changes and review evidence.
- Confirm the worktree is clean.
- Confirm the commit has the correct `Agent:` trailer.
- Push with a normal fast-forward push.

Every review, including a clean one, must commit a report and an index entry. Report
failed checks honestly rather than claiming acceptance. If push is rejected, stop and
preserve local commits; do not force-push, reset, or merge new Grok work into the run.
