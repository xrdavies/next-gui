# Unicode word wrapping evidence

Target8569b7c4f6028f96be543affe26a1ea940eed289; base d6462fc;
shared core9affefb855bc9639b8a5290341ef5d09ddf41e6b.
macOS27.0.1 arm64/Rust1.86.0/CommandLineTools,2026-10-29 JST.
Final checks56841, render48630 and native84110 exited0 and were consumed.
749 tests/43 groups,0 failed/ignored. Offline workspace all-target Clippy
-D warnings, workspace build, host macos/windows feature checks all0.
79 frozen source/config hashes checked before and after native work. Browser
SHA2560da9826973c9cb504e46da427c43bba8cac560bae96c84cdf4db3e76da774962.
14 Office binary hashes frozen before launching and matched afterwards.

32 complete P6 frames/16 whole-frame Soft/actual Metal pairs cover dark/light,
1400/font13 and800/font18, on/off and start/end source views. Max delta2,
none above2; both sheets and raw narrow start inspected.12 ready/PID/window/size
guarded native browser windows cover both themes and wide/narrow/readonly modes.
4 private CGEventSource postToPid flows exercise Home/End and Space on/off/on.
Each changes over200 channels and restores the whole frame exactly.14 rebuilt
Office windows captured and inspected. All30 owned processes awaited SIGINT(-2).
No clipboard used. Native/keyboard/Office sheets and raw Home inspected.

Before implementation, word-boundary regression failed as expected. Initial test
width made second word too wide even alone; corrected to max(first-space,second)
plus2 and retained proof that old character layout cuts it. Added trailing-space
regression and31 focused shared checks. Direct official unicode-linebreak0.1.5
crate archive SHA2563b09c83c3c29d37506a3e260c08c03743a6bb66a9cd432c6934ab501a190571f
matched cached registry index before offline verification. One cargo fetch was
intentionally stopped130 on registry timeouts. Native63795 was stopped130 while
waiting before windows to freeze Office hashes;84110 is final. Failed/pre-final
iterations are not accepted evidence.

Original v3.13.1 uses Scintilla Word and Qt WordBoundaryOrAnywhere wrapping:
https://github.com/sqlitebrowser/sqlitebrowser/blob/v3.13.1/src/EditDialog.cpp
Unicode line opportunities come from pinned0.1.5, Unicode15/MSRV1.56/Apache2:
https://github.com/axelf4/unicode-linebreak
https://docs.rs/unicode-linebreak/0.1.5/unicode_linebreak/
Existing unicode-segmentation supplies extended grapheme fallback. Fontdue's
installed word layout rounds metrics and differs from our fractional geometry.

Mob screenshot02:27:12UTC still wallpaper/loginwindow. Ordinary frontmost
pointer/typing/IME/pickers/drag/resize/a11y/Print and Linux/Windows runtime remain
not_run. Private process keys and startup sizes do not establish these paths.
Scalar caret editing, complex shaping/bidi/RTL/missing emoji, discretionary
hyphens, non-LF mandatory separators, style-specific wrapping and full original
binary/codecs/actions/docking/product parity remain. F02 in_progress. No
completion/publication claim; earlier Mob push approval timed out/denied.
Local scripts/logs /tmp/next-gui-word-wrap-v8-; portable metrics/assets here.
