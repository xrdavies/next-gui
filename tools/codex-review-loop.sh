#!/bin/zsh -f
set -euo pipefail

REPO="${0:A:h:h}"
STATE_DIR="$(git -C "$REPO" rev-parse --absolute-git-dir)/codex-review"
STATE_FILE="$STATE_DIR/last-seen"

log() {
  print -r -- "$(date '+%Y-%m-%dT%H:%M:%S%z') $*"
}

fail() {
  log "$*" >&2
  exit 1
}

save_state() {
  print -r -- "$1" > "$STATE_FILE.tmp"
  mv "$STATE_FILE.tmp" "$STATE_FILE"
}

is_codex_commit() {
  [[ "$(git show -s --format='%(trailers:key=Agent,valueonly)' "$1")" == "Codex" ]]
}

mkdir -p "$STATE_DIR"
zmodload zsh/system
touch "$STATE_DIR/run.lock"
# A kernel lock is released on exit, including crashes; no stale PID cleanup is needed.
if ! zsystem flock -t 0 -f REVIEW_LOCK_FD "$STATE_DIR/run.lock"; then
  log "another review is running; skipping this interval"
  exit 0
fi

export GIT_TERMINAL_PROMPT=0
export GIT_SSH_COMMAND="${GIT_SSH_COMMAND:-ssh -o BatchMode=yes}"
cd "$REPO"
[[ "$(git branch --show-current)" == "main" ]] || fail "expected main; leave other branches untouched"
[[ -z "$(git status --porcelain=v1)" ]] || fail "worktree is dirty; preserve local changes"

git fetch origin main
TARGET=$(git rev-parse origin/main)
git merge-base --is-ancestor HEAD "$TARGET" || fail "local main has unpublished or divergent commits; recover manually"
git merge --ff-only --quiet "$TARGET"

BASE=""
if [[ -f "$STATE_FILE" ]]; then
  BASE=$(cat "$STATE_FILE")
  [[ "$BASE" =~ '^([0-9a-f]{40}|[0-9a-f]{64})$' ]] || fail "invalid last-seen commit; inspect the state file"
  git merge-base --is-ancestor "$BASE" "$TARGET" || fail "last-seen is not an ancestor of origin/main"
fi
[[ "$TARGET" != "$BASE" ]] || exit 0

if [[ -n "$BASE" ]]; then
  needs_review=0
  for commit in ${(f)"$(git rev-list "$BASE..$TARGET")"}; do
    if ! is_codex_commit "$commit"; then
      needs_review=1
      break
    fi
  done
  if (( ! needs_review )); then
    save_state "$TARGET"
    log "only Codex commits since last-seen; no review required"
    exit 0
  fi
fi

CODEX=$(command -v codex) || fail "codex executable not found in PATH"
REPORT="docs/reviews/R-$(date +%Y%m%d)-${TARGET[1,12]}.md"
PROMPT=$(cat <<'PROMPT_EOF'
You are the Codex handoff agent for next-gui. Your fixed operational role is Codex.
Read AGENTS.md, docs/reviews/INDEX.md, and the linked reports before working.
The runner has already fetched and pinned main to the target below. Verify HEAD matches
that target. Do not pull/rebase or change the review scope during this run.

1. Review all incoming non-Codex commits in the supplied range, including their callers
   and tests. With no previous commit, review the current repository snapshot.
2. Read the requirements, architecture, current roadmap, and relevant design documents.
   Compare the Linux implementation with macOS Office and implement missing macOS
   support. Leave unrelated product fixes to Grok and record them as findings.
3. Run cargo test --workspace, cargo clippy --workspace --all-targets -- -D warnings,
   cargo check -p next-gui-platform --features macos, and git diff --check. Build the
   workspace example packages. Discover current examples from Cargo.toml and docs;
   do not limit the check to P0/P1. Re-run relevant checks after code changes.
4. Run relevant macOS GUI examples with bounded execution. Verify rendering and input
   when possible, close only processes started by this run, and record build, startup,
   visual, and interaction results separately. Missing GUI access is not a pass.
5. Re-test fixed findings, including those still assigned to Grok. Mark verified only
   with evidence; otherwise reopen or leave blocked and state the reason.
6. Write the exact report path supplied below, using docs/reviews/REPORT_TEMPLATE.md.
   Record the reviewed base and target, actual command outcomes, macOS changes, and
   specific findings with stable IDs. Update docs/reviews/INDEX.md in the same run.
   Even a clean review requires a report and an index entry. Record failed or unavailable
   checks honestly; never silently treat them as passed.
7. Commit focused changes with Conventional Commits. Every new commit must contain
   Agent: Codex. The final commit must also contain Reviewed-Commit: <full target SHA>.
   Do not create an empty commit. Push normally to origin/main.
8. If push is rejected, stop and preserve the local commits for recovery. Do not force
   push, reset, or merge new Grok work into this run. Do not change last-seen yourself.
PROMPT_EOF
)
PROMPT="$PROMPT

Current target commit: $TARGET
Previous processed commit: ${BASE:-none (initial snapshot)}
Expected review report: $REPORT"

log "reviewing $TARGET"
"$CODEX" --ask-for-approval never exec \
  --cd "$REPO" \
  --sandbox danger-full-access \
  --ephemeral \
  --color never \
  --output-last-message "$STATE_DIR/last-result.md" \
  - <<< "$PROMPT"

[[ "$(git branch --show-current)" == "main" ]] || fail "Codex changed branch; state was not advanced"
[[ -z "$(git status --porcelain=v1)" ]] || fail "Codex left uncommitted changes; state was not advanced"
FINAL=$(git rev-parse HEAD)
[[ "$FINAL" != "$TARGET" ]] || fail "Codex created no review commit; state was not advanced"
git merge-base --is-ancestor "$TARGET" "$FINAL" || fail "Codex rewrote the pinned history"
for commit in ${(f)"$(git rev-list "$TARGET..$FINAL")"}; do
  is_codex_commit "$commit" || fail "unexpected non-Codex commit in output; inspect concurrent changes"
done
[[ "$(git show -s --format='%(trailers:key=Reviewed-Commit,valueonly)' "$FINAL")" == "$TARGET" ]] \
  || fail "final commit is missing the expected Reviewed-Commit trailer"
git cat-file -e "$FINAL:$REPORT" || fail "expected review report was not committed"
git cat-file -e "$FINAL:docs/reviews/INDEX.md" || fail "review index was not committed"
if git diff --quiet "$TARGET" "$FINAL" -- "$REPORT"; then
  fail "review report was not updated"
fi
if git diff --quiet "$TARGET" "$FINAL" -- docs/reviews/INDEX.md; then
  fail "review index was not updated"
fi

git fetch origin main
git merge-base --is-ancestor "$FINAL" origin/main || fail "review commits are not on origin/main; recover the local commits"
# Save only the reviewed output; a newer Grok push is picked up on the next interval.
save_state "$FINAL"
log "processed $TARGET; saved checkpoint $FINAL"
