#!/usr/bin/env python3
"""Exercise the real runner against local Git remotes; never invoke the real Codex."""

import fcntl
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "codex-review-loop.sh"
FAKE_CODEX = r'''
import json, os, pathlib, re, subprocess, sys

args = sys.argv[1:]
assert args[:3] == ["--ask-for-approval", "never", "exec"], args
repo = pathlib.Path(args[args.index("--cd") + 1])
prompt = sys.stdin.read()
target = re.search(r"^Current target commit: (.+)$", prompt, re.M).group(1)
report = re.search(r"^Expected review report: (.+)$", prompt, re.M).group(1)
assert "Your fixed operational role is Codex." in prompt
with open(os.environ["REVIEW_TEST_CALLS"], "a") as calls:
    calls.write(json.dumps({"target": target, "prompt": prompt}) + "\n")
mode = os.environ.get("REVIEW_TEST_MODE", "success")
if mode == "fail":
    sys.exit(7)
if mode == "noop":
    sys.exit(0)

def git(*args, cwd=repo):
    return subprocess.check_output(["git", *args], cwd=cwd, stderr=subprocess.STDOUT, text=True).strip()

if mode == "dirty":
    (repo / "unfinished.txt").write_text("preserve me")
    sys.exit(0)
if mode != "missing-report":
    (repo / report).write_text("# Test review\n\nReviewed target: " + target + "\n")
with (repo / "docs/reviews/INDEX.md").open("a") as index:
    index.write("\nTest review: " + target + "\n")
agent = "Grok" if mode == "wrong-agent" else "Codex"
git("add", ".")
git("commit", "-m", "review: test handoff\n\nAgent: " + agent + "\nReviewed-Commit: " + target)

def grok_push():
    writer = pathlib.Path(os.environ["REVIEW_TEST_WRITER"])
    git("pull", "--ff-only", cwd=writer)
    (writer / "concurrent.txt").write_text(target)
    git("add", ".", cwd=writer)
    git("commit", "-m", "feat: concurrent work\n\nAgent: Grok", cwd=writer)
    git("push", "origin", "main", cwd=writer)

if mode == "push-rejected":
    grok_push()
if mode != "no-push":
    git("push", "origin", "main")
if mode == "remote-ahead":
    grok_push()
pathlib.Path(args[args.index("--output-last-message") + 1]).write_text("Test complete\n")
'''


class ReviewLoopTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="next-gui review ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.remote = self.root / "origin.git"
        self.writer = self.root / "grok"
        self.repo = self.root / "mac"
        self.state = self.repo / ".git/codex-review"
        self.calls = self.root / "calls.jsonl"
        (self.root / "test bin").mkdir()
        self.fake = self.root / "test bin/codex"
        self.env = dict(os.environ, GIT_TERMINAL_PROMPT="0",
                        PATH=str(self.fake.parent) + os.pathsep + os.environ["PATH"],
                        REVIEW_TEST_CALLS=str(self.calls),
                        REVIEW_TEST_WRITER=str(self.writer))
        self.fake.write_text("#!" + sys.executable + "\n" + FAKE_CODEX)
        self.fake.chmod(0o755)
        self.git("init", "--bare", "--initial-branch=main", str(self.remote), cwd=self.root)
        self.git("clone", str(self.remote), str(self.writer), cwd=self.root)
        self.configure(self.writer)
        (self.writer / "tools").mkdir()
        (self.writer / "docs/reviews").mkdir(parents=True)
        shutil.copy2(SOURCE, self.writer / "tools/codex-review-loop.sh")
        (self.writer / "docs/reviews/INDEX.md").write_text("# Test index\n")
        self.git("add", ".", cwd=self.writer)
        self.git("commit", "-m", "test: seed\n\nAgent: Grok", cwd=self.writer)
        self.git("push", "-u", "origin", "main", cwd=self.writer)
        self.seed = self.git("rev-parse", "HEAD", cwd=self.writer)
        self.git("clone", str(self.remote), str(self.repo), cwd=self.root)
        self.configure(self.repo)
        self.counter = 0

    def git(self, *args, cwd=None):
        return subprocess.check_output(
            ["git", *args], cwd=cwd or self.repo, env=self.env,
            stderr=subprocess.STDOUT, text=True,
        ).strip()

    def configure(self, repo):
        for key, value in [
            ("user.name", "Review runner test"),
            ("user.email", "runner@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "/dev/null"),
        ]:
            self.git("config", key, value, cwd=repo)

    def checkpoint(self, sha):
        self.state.mkdir(exist_ok=True)
        (self.state / "last-seen").write_text(sha + "\n")

    def last_seen(self):
        path = self.state / "last-seen"
        return path.read_text().strip() if path.exists() else None

    def incoming(self, agent="Grok"):
        self.git("pull", "--ff-only", cwd=self.writer)
        self.counter += 1
        (self.writer / f"change-{self.counter}.txt").write_text(agent)
        self.git("add", ".", cwd=self.writer)
        self.git("commit", "-m", f"feat: change {self.counter}\n\nAgent: {agent}", cwd=self.writer)
        self.git("push", "origin", "main", cwd=self.writer)
        return self.git("rev-parse", "HEAD", cwd=self.writer)

    def run_loop(self, mode="success"):
        return subprocess.run(
            ["/bin/zsh", "-f", str(self.repo / "tools/codex-review-loop.sh")],
            env=dict(self.env, REVIEW_TEST_MODE=mode), text=True,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=30,
        )

    def records(self):
        return [json.loads(line) for line in self.calls.read_text().splitlines()] if self.calls.exists() else []

    def test_initial_snapshot_then_unchanged_head_is_idle(self):
        result = self.run_loop()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(self.last_seen(), self.git("rev-parse", "HEAD"))
        self.assertEqual(len(self.records()), 1)
        self.assertIn("none (initial snapshot)", self.records()[0]["prompt"])
        result = self.run_loop()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(len(self.records()), 1)

    def test_all_codex_range_skips_model(self):
        self.checkpoint(self.seed)
        target = self.incoming("Codex")
        result = self.run_loop()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(self.last_seen(), target)
        self.assertEqual(self.records(), [])

    def test_codex_tip_does_not_hide_grok_commits(self):
        self.checkpoint(self.seed)
        self.incoming("Grok")
        target = self.incoming("Codex")
        result = self.run_loop()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(self.records()[0]["target"], target)
        self.assertIn(self.seed, self.records()[0]["prompt"])

    def test_dirty_tree_is_preserved(self):
        self.checkpoint(self.seed)
        path = self.repo / "unfinished.txt"
        path.write_text("preserve me")
        result = self.run_loop()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(path.read_text(), "preserve me")
        self.assertEqual(self.last_seen(), self.seed)
        self.assertEqual(self.records(), [])

    def test_unpublished_commit_is_preserved(self):
        self.checkpoint(self.seed)
        self.git("commit", "--allow-empty", "-m", "test: unpublished work")
        local = self.git("rev-parse", "HEAD")
        result = self.run_loop()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.git("rev-parse", "HEAD"), local)
        self.assertEqual(self.last_seen(), self.seed)
        self.assertEqual(self.records(), [])

    def test_other_branch_is_untouched(self):
        self.git("switch", "-c", "work-in-progress")
        result = self.run_loop()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.git("branch", "--show-current"), "work-in-progress")
        self.assertEqual(self.records(), [])

    def test_model_failure_does_not_advance_and_releases_lock(self):
        self.checkpoint(self.seed)
        self.incoming()
        result = self.run_loop("fail")
        self.assertEqual(result.returncode, 7, result.stdout)
        self.assertEqual(self.last_seen(), self.seed)
        result = self.run_loop()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(len(self.records()), 2)

    def test_zero_exit_without_report_does_not_advance(self):
        self.checkpoint(self.seed)
        self.incoming()
        result = self.run_loop("noop")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.last_seen(), self.seed)

    def test_dirty_model_output_is_preserved_without_advancing(self):
        self.checkpoint(self.seed)
        self.incoming()
        result = self.run_loop("dirty")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.last_seen(), self.seed)
        self.assertEqual((self.repo / "unfinished.txt").read_text(), "preserve me")

    def test_partial_report_is_rejected(self):
        self.checkpoint(self.seed)
        self.incoming()
        result = self.run_loop("missing-report")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.last_seen(), self.seed)
        self.assertIn("expected review report was not committed", result.stdout)

    def test_unpushed_output_keeps_local_commits(self):
        self.checkpoint(self.seed)
        target = self.incoming()
        result = self.run_loop("no-push")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.last_seen(), self.seed)
        self.assertNotEqual(self.git("rev-parse", "HEAD"), target)
        self.assertIn("not on origin/main", result.stdout)

    def test_wrong_commit_identity_is_rejected(self):
        self.checkpoint(self.seed)
        self.incoming()
        result = self.run_loop("wrong-agent")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.last_seen(), self.seed)
        self.assertIn("unexpected non-Codex commit", result.stdout)

    def test_concurrent_grok_push_is_left_for_next_run(self):
        self.checkpoint(self.seed)
        self.incoming()
        result = self.run_loop("remote-ahead")
        self.assertEqual(result.returncode, 0, result.stdout)
        reviewed = self.git("rev-parse", "HEAD")
        pending = self.git("rev-parse", "origin/main")
        self.assertNotEqual(reviewed, pending)
        self.assertEqual(self.last_seen(), reviewed)
        result = self.run_loop()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(self.records()[-1]["target"], pending)
        self.assertEqual(len(self.records()), 2)

    def test_rejected_push_preserves_output_and_checkpoint(self):
        self.checkpoint(self.seed)
        target = self.incoming()
        result = self.run_loop("push-rejected")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.last_seen(), self.seed)
        local = self.git("rev-parse", "HEAD")
        self.assertNotEqual(local, target)
        result = self.run_loop()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.git("rev-parse", "HEAD"), local)
        self.assertEqual(len(self.records()), 1)

    def test_lock_contention_skips_model(self):
        self.state.mkdir()
        with (self.state / "run.lock").open("a+") as lock:
            fcntl.lockf(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            result = self.run_loop()
            self.assertEqual(result.returncode, 0, result.stdout)
            self.assertEqual(self.records(), [])
        result = self.run_loop()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(len(self.records()), 1)

    def test_invalid_checkpoint_is_not_silently_reset(self):
        self.checkpoint("invalid-commit")
        result = self.run_loop()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.last_seen(), "invalid-commit")
        self.assertEqual(self.records(), [])


if __name__ == "__main__":
    unittest.main(verbosity=2)
