# Live cell draft information evidence

Target d11fa4e48f10a5cd49044f8db0ef371c1755128a; base3d8902b;
shared Ui tooltip implementation52043f5. macOS27.0.1 arm64/Rust1.86.0,
CommandLineTools,2026-10-29 JST. Final checks61898, render42490 and native45616
exited0 and were consumed.753 tests/43 groups,0 failed/ignored; offline workspace
all-target Clippy -D warnings, workspace build, host macos/windows checks all0.
79 frozen source/config hashes checked before and after native work;14 Office
binary hashes and browser SHA256
f14d37a72217ce19ce2607a53d135923eb9790294582ac597ff05ba9f39d5122 match.

16 complete P6 frames/8 whole-frame Soft/actual Metal pairs cover dark/light,
1400/font13 and800/font18, edited text and Hex. Max delta2, none above2; both
sheets inspected.12 native windows cover text, Hex, invalid/empty data and
wide/narrow image dimensions.4 private CGEventSource postToPid flows select a
baseline, type one character or delete, Undo and Redo. Actual text count5→1 and
Hex count3→0; changed channels8021/8025 and3626/3622. Undo whole frame exact0;
Redo matches changed frame exactly. No clipboard used.14 rebuilt Office windows
captured/reviewed; all30 final owned processes awaited SIGINT(-2). Native,
keyboard and Office sheets and raw edited text/empty Hex/image inspected.

First native driver38359 exited1 after the helper injected multiple characters
as one keyboard event. The platform key route filters multi-character logical
keys; frames showed only Select All. Its16 processes were awaited but images
were rejected. Corrected helper sends one character or Backspace, selects before
the baseline to account for Undo selection restoration, and repeats all native
drivers. No Rust edits occurred after the79-file freeze, so accepted checks and
render remained valid. No failed iteration supports acceptance.

State/GUI regressions cover Qt UTF16 units, empty/singular/CJK/combining/ZWJ,
valid/invalid Hex and Unicode whitespace, all256 bytes and300000-byte size scan,
NULL, malformed JSON/XML, unevaluated SQL, real image decode and SQL record
positions.24 editable GUI mode/theme/size cases plus24 readonly returns retain IDs, revision, focus,
caret/history, readonly guards and stored values. Tooltip fallback changes and
empty help retain host/hover/children/focus and update the actual mask.

Original editTextChanged counts QString length; updateCellInfoAndMode shows
text/numeric/JSON character sizes, binary bytes and image dimensions:
https://github.com/sqlitebrowser/sqlitebrowser/blob/v3.13.1/src/EditDialog.cpp
Our footer consistently counts the visible draft, including hidden-BOM views;
this does not copy the original raw-load vs edited-buffer count inconsistency.
Hex metadata scans without allocating decoded data, and Apply shares validation.
ASCII signs such as+1 are now invalid Hex. No new dependencies.

Mob screenshot03:08UTC wallpaper only. Frontmost pointer/typing/IME/pickers/
drag/resize/a11y/Print and Linux/Windows runtime not_run. Private process keys
and startup sizes are narrower evidence. UTF16 is not a grapheme count; NULL
storage conversion is not undone by text history. Narrow captions retain full
accessible/hover text but ordinary native hover was not accepted. Dedicated
binary editor, full codecs/RTL/history/actions/docking and original product/
visual/native/platform parity remain. Stable F02 in_progress; no completion or
publication claim. Earlier Mob push approval timed out/denied.
Local scripts/logs /tmp/next-gui-cell-info-v3-; portable metrics/assets here.
