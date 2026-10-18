# CI helpers

| Script / job | Purpose |
|--------------|---------|
| [`field-soft.sh`](field-soft.sh) | Field Soft packaging gate: Soft package set + `e24 --no-default-features`; asserts no `wgpu` / `next-gui-render-gpu` in those trees |
| GitHub **field-soft** | Runs `field-soft.sh` on Ubuntu |
| GitHub **workspace** | Full workspace `cargo check` + `cargo test` (GPU included; GUI tests skip without `DISPLAY`) |
| GitHub **platform-features** | `cargo check -p next-gui-platform --features macos` and `--features windows` |
| GitHub **clippy** | `cargo clippy --workspace --all-targets` |

Workflow: [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml). Design notes: [docs/p4-design.md](../../docs/p4-design.md) §5.

## Enabling GitHub Actions

Canonical workflow definition: [`github-actions-ci.yml`](github-actions-ci.yml).

Copy into the repo root to activate (requires a credential with the `workflow` scope):

```bash
mkdir -p .github/workflows
cp tools/ci/github-actions-ci.yml .github/workflows/ci.yml
git add .github/workflows/ci.yml
git commit -m "ci: enable GitHub Actions workflow"
git push
```
