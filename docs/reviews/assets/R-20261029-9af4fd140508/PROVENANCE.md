# Provenance

Target: `9af4fd140508a169061140413e185e99fed78bfa` on `feat/sqlitebrowser-example`.
Host: macOS27.0.1 arm64, Rust1.86.0, CommandLineTools SDK.
Final QA prefix: `/tmp/next-gui-show-rowid-v3-`.

`metrics.json` records154 source hashes,789 tests/43 groups, all five strict
check exits,20 whole-frame Soft/actual-Metal comparisons,8 native static
captures and4 private-PID key workflows. The maximum whole-frame channel
difference is1; no channel exceeds2. Binary SHA256:
`5f467dcc102d99cb70bc4a58e21a4bd1c049f389c1328ee1b511c0cc48df62eb`.

Native GPU windows were selected by owned PID and CGWindow number after the
ready marker. Wide fixtures use1400x860/font13; narrow fixtures800x600/font18.
Static captures cover shown rowid, readonly and actual header menus in both
themes. Four Apply flows send Enter to the focused action using the archived
private CGEventSource driver; all changed frames and selected exact keys were
inspected. All12 processes were awaited with SIGINT(-2). Both render sheets,
all three native sheets and representative full-resolution frames were viewed.

Mob screenshot at approximately06:45UTC showed wallpaper only. Owned-window
captures and private-PID keys do not establish ordinary frontmost pointer,
typing/IME, pickers, drag/resize/a11y/Print or Linux/Windows runtime acceptance.
No shared widgets changed, so Office examples were not rerun for this scope.
All databases and files were owned QA fixtures. No user database was modified.

Intermediate runs exposed a native-fixture test-only call, a missing scene sync
before menu capture and two existing-test regressions (menu last item and
invalid export sorting). These were corrected before the final frozen v3 run.
An independent existing Revert/catalog failure remains open in the report.
