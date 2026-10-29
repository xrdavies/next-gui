# Binary cell editor evidence

Target5232512e0e9d951e0f0dfe9483b7de9e1b13a2e5; base4db4731;
shared implementation8bdb459. macOS27.0.1 arm64 / Rust1.86.0,
CommandLineTools,2026-10-29 JST. Final checks2099, render42521 and native85216
exited0 and were consumed.760 tests/43 groups,0 failed/ignored; offline
workspace all-target Clippy -D warnings, workspace build and host macos/windows
feature checks all0.115 source/config hashes checked before and after native
work. Browser SHA256231e6713aff48bc8ae7ae61c40f0d8a93ab010dc3bf5ae4ff991a93d1144d7c5
and14 Office binary hashes match the final binaries; see metrics.json.

24 complete P6 frames/12 whole-frame Soft/actual Metal pairs cover dark/light,
1400/font13 and800/font18, selection, readonly and end-of-buffer. Max delta2,
none above2. Both sheets inspected.12 native windows cover259 bytes, selected
narrow views, readonly, empty,1048579-byte EOF and other-mode text.4 private
CGEventSource postToPid flows select before the baseline and type one character
in Hex or ASCII, then Cmd+Z / Shift+Cmd+Z. Changed channels344537..344589;
Undo restores whole frames exactly0, Redo restores changed frames exactly.
No clipboard mutation.14 rebuilt Office windows captured and inspected. All30
owned processes awaited SIGINT(-2). Native/keyboard/Office sheets and raw
wide/selected/empty/million-byte captures inspected.

Earlier v4 workspace run failed one stale test that still expected the old Hex
string representation; migrated the assertion to raw bytes. v4 native driver
exited1 before starting any GUI because checks had failed. Subsequent review
found that equal-value overwrite could move the caret but return false, missing
a redraw. Fixed that return and added a regression. All final checks, rendering
and native acceptance repeated as v5 after the source freeze; no failed run
supports acceptance. The standalone final core suite has5 focused Hex tests;
the app matrix covers byte sessions, readonly, strict paste/UTF8, typed Apply,
no DB writes before Apply, history, readable export and source isolation.

Original pinned3.13.1 references:
https://github.com/sqlitebrowser/sqlitebrowser/blob/v3.13.1/libs/qhexedit/src/qhexedit.cpp
https://github.com/sqlitebrowser/sqlitebrowser/blob/v3.13.1/src/EditDialog.cpp
Address width,16-byte rows, insert/overwrite, selection readable format and
initial insertion follow the reference. Paste deliberately rejects malformed
hex rather than silently discarding junk/odd nibbles. Original EditDialog Hex
context exposes Print and Copy Hex+ASCII; library search APIs are not evidence
of additional controls in that dialog.

Mob screenshot03:50UTC wallpaper only. Ordinary frontmost pointer/typing/IME,
pickers/drag/resize/accessibility/Print and Linux/Windows runtime not_run;
private process keys/startup sizes are narrower evidence. Narrow800/font18
still has few visible byte rows; toolbars/ASCII remain accessible by scrolling.
Buffer is in memory, Arc sharing may copy on edit; history bounded100 changes/
8MiB per stack retaining one oversized change. Import limit16MiB. Full original
font preferences/context/print/external workflows, RTL/codecs/cross-mode history,
docking and complete visual/product/native/platform parity remain. Stable F02
in_progress; no completion/publication claim. Earlier exact push approval timed
out/denied; no new authorization. Local scripts/logs /tmp/next-gui-hex-editor-v5-.
