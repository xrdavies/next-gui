# Automatic Browse filtering QA

Target: `8df5c111638d58fb82c199931d79a163ad04fd80`. macOS27.0.1 arm64 / Rust1.86.0.
The155 Rust/Cargo hashes and final browser binary SHA are in metrics.json.
All5 full checks exit0;798 tests/43 groups,508 browser tests. The final frozen
v2 run generated32 P6 frames/16 actual Metal versus Soft pairs, all channels≤2.
Both contact sheets and their raw narrow preference/error frames were inspected.

Eight owned GPU windows received private CGEventSource.postToPid single-character
input: alp produces2 rows, A!%B with ! escape produces1, Cmd+A/Backspace restores5.
All24 screenshots were reviewed through two sheets; actual wide escape frame
was inspected. Four additional preference windows and14 Office windows were
captured and inspected. All26 owned children were awaited at SIGINT exit-2.
Window IDs/PIDs and status OCR are recorded; OCR assists visual checks and is
not used as proof of every glyph. The Mob07:53UTC screenshot showed wallpaper
only. These are owned-window GPU/key checks, not frontmost input/IME/a11y,
file picker, Print, Linux or Windows runtime acceptance.

Earlier development checks: wrong Cargo package name; missing public method doc;
test referenced nonexistent error field; clone/needless-borrow Clippy warnings.
All corrected before v2. Initial v1 render used the Plot-specific helper and
failed because these fixtures have no visible Plot. A whole-window renderer
replaced it. Superseded v1 workspace/native-wait processes were explicitly
stopped and consumed; no result from v1 is claimed as final acceptance.
A metrics attempt before the14th Office capture finished failed its count
assertion; the final run after all children exited passed. No user preferences
are read/written by unit tests; QA runtime prefs use a PID-owned temp path.

Reference: official DB4S v3.13.1 FilterLineEdit.cpp, CondFormat.cpp,
Settings.cpp and PreferencesDialog.ui. Default200ms/maximum5000 and backslash
escape were checked against source. Raw WHERE is an extra explicit-only field.
