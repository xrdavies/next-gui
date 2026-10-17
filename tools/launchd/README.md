# Codex launchd runner

See [the agent workflow](../../docs/agent-workflow.md) for the complete handoff protocol.
The fixed prompt in [codex-review-loop.sh](../codex-review-loop.sh) selects the Codex
role. There is no role environment variable and no Grok launcher.

## Configure

Use a dedicated macOS clone on `main`. The script derives its repository directory
from its own path, locates `codex` through PATH, and stores progress in that clone's Git
metadata directory (normally `.git/codex-review/`). No tracked script edits are needed.

Edit the plist's absolute repository paths, log paths, and `PATH` for the target machine.
The checked-in plist
targets `/Users/r001/opensource/next-gui` and includes `~/.cargo/bin` in its explicit
PATH. launchd does not source the interactive shell's PATH or expand `~` in plist values.

Run as the logged-in Mac user, with Git identity and credentials already configured:

```bash
codex login status
git config user.name
git config user.email
git ls-remote --exit-code origin refs/heads/main
git push --dry-run origin HEAD:main
```

If login is needed, run `codex login` interactively first. Git must fetch/push without a
password prompt: use the user's SSH agent or credential helper, not a token in the plist.
The runner disables Git terminal prompts and uses SSH batch mode unless a custom
`GIT_SSH_COMMAND` is already configured. Use a full clone so older review bases exist.

## First run

With no `last-seen`, the runner reviews the current repository snapshot. To process
only future commits, initialize the baseline **before** loading the LaunchAgent:

```bash
state_dir="$(git rev-parse --absolute-git-dir)/codex-review"
mkdir -p "$state_dir"
git fetch origin main
git rev-parse origin/main > "$state_dir/last-seen"
```

You may first run `./tools/codex-review-loop.sh` in the configured clone. It performs real
Codex work and Git pushes when there is pending work. Test coverage for the runner uses
temporary local repositories and a fake CLI instead:

```bash
python3 tools/tests/test_codex_review_loop.py
```

## Install and start

Create the log directory before loading the job:

```bash
mkdir -p "$HOME/Library/Logs/next-gui-codex" "$HOME/Library/LaunchAgents"
cp tools/launchd/com.nextgui.codex-review.plist \
  "$HOME/Library/LaunchAgents/com.nextgui.codex-review.plist"
plutil -lint "$HOME/Library/LaunchAgents/com.nextgui.codex-review.plist"

launchctl bootstrap "gui/$(id -u)" \
  "$HOME/Library/LaunchAgents/com.nextgui.codex-review.plist"
```

RunAtLoad starts the first poll immediately; StartInterval repeats every 300 seconds.
A run already in progress is not restarted at the next interval. The script also uses
a native zsh file lock to exclude manual overlapping runs. Polls are missed while the
Mac sleeps, and the user must be logged into an Aqua session for this LaunchAgent.
Locked/headless sessions may still prevent GUI acceptance; record that in the report.

Inspect or request a poll without killing a running review:

```bash
launchctl print "gui/$(id -u)/com.nextgui.codex-review"
launchctl kickstart "gui/$(id -u)/com.nextgui.codex-review"
tail -n 80 "$HOME/Library/Logs/next-gui-codex/stdout.log"
tail -n 80 "$HOME/Library/Logs/next-gui-codex/stderr.log"
```

## Stop and recover

To unload, wait until the current review finishes, then run:

```bash
launchctl bootout "gui/$(id -u)/com.nextgui.codex-review"
```

Unloading or `kickstart -k` during a review can interrupt commands and leave unfinished
work. For a plist update, unload after completion, copy the new plist, then bootstrap.
Never delete the lock file to bypass a running job.

After an error, inspect the logs and the clone:

```bash
git status --short --branch
git log --oneline origin/main..HEAD
```

Commit/recover unfinished work and reconcile unpublished commits manually. The runner
refuses dirty trees, other branches, divergent history, and invalid progress; it never
discards that work. Once the clone is safely synchronized, request another poll.
Do not remove or advance `last-seen` without checking which range was actually reviewed.

## Local state and execution scope

State is untracked, under `$(git rev-parse --absolute-git-dir)/codex-review/`:

- `last-seen`: last safely handed-off commit, saved atomically
- `run.lock`: native advisory lock, released automatically when the process exits
- `last-result.md`: latest Codex final message, useful after a failure

Logs are `~/Library/Logs/next-gui-codex/{stdout,stderr}.log`. They are local diagnostics;
review evidence needed by Grok belongs in the committed report.

The CLI invocation uses `--ask-for-approval never` before `exec`, followed by
`--sandbox danger-full-access`. These arguments were checked against Codex CLI 0.163.0.
This permits unattended tests, GUI tools, and Git writes; configure it only for a
trusted repository on the intended machine. Installation is explicit: committing the
plist does not load or start the service.
