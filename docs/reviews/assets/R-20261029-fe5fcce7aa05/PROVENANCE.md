# Cell dock layout evidence

Target: `fe5fcce7aa057ebd11ac437ef9b0c9fd5cc1545c`; parent shared fix: `e6c44801ab9c6af839632dfe9b92813f72c285d1`.
macOS27.0.1 arm64, Rust1.86.0, CommandLineTools; 2026-10-29 JST.

Final source freeze:43 files, SHA256 values in metrics.json. Browser binary SHA256
and all14 freshly workspace-built Office binary hashes are recorded there.

`/tmp/next-gui-cell-layout-v14-checks.py` ran offline workspace all-target Clippy
with -D warnings, workspace tests/build and host platform macos/windows feature
checks. Session18365 exit0 was consumed.721 tests/43 groups, no failed or ignored.

Session20060 exit0 was consumed after the new layout test and converter both
completed.16 complete P6 frames form8 Soft/actual Metal pairs; the entire frames
have max channel delta2 and no channel above2. Both contact sheets and the raw
800px light under-icon/large-font frames were visually reviewed. Narrow prepared
render captures focus the chart to reveal its whole body through shell scrolling.

Final native drivers:14 theme/size/font/style/popup/revealed-chart windows,
2 targeted native apply/copy workflows and14 Office static windows. Each owned
process was checked by PID/CGWindow, captured and awaited after SIGINT(-2).
The browser ready marker and startup size gates were required. Final source and
binary hashes were rechecked by native-all.py. Session39287 is consumed separately.

The archived Swift helper uses a private CGEventSource and postToPid. Cmd+Enter
applies, Cmd+A/C must return exact `中文A • 18` in both themes. All original pasteboard
types are held in RAM and restored only while still owned; clipboard contents
are never written to logs/files. The copy logs confirmed successful restoration.

The Mob screenshot remains wallpaper/loginwindow: this evidence does not accept
ordinary frontmost pointer/typing/IME/picker/drag/native a11y/Print or other-host
runtime behavior. Prepared startup sizes are not OS resize acceptance.

Earlier v10 frames were rejected: a100px chart collapsed its data area and broke
coordinate-tooltip regression. v13 checks passed but large-font visual review
still showed a compressed graph; dynamic title/font reservation and a110px plot
image minimum assertion were added, then all final checks/evidence rerun at v14.

Local scripts/logs retain the `/tmp/next-gui-cell-layout-v14-` prefix. PNGs and
portable aggregate outcomes are archived here. No publication was performed.
