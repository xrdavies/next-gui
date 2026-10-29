# Automatic cell mode evidence

Target: bbd7ade72d85913cf6f8186dcdca28cb288486d0; parent: `4096b2f`.
macOS27.0.1 arm64, Rust1.86.0, CommandLineTools;2026-10-29 JST.

Final source freeze covers all browser Rust sources; source and browser binary
SHA256 values are recorded in metrics.json. Offline workspace tests:728/43 groups,
zero failed/ignored. Workspace all-target Clippy -D warnings, workspace build and
host macos/windows feature checks all0. Sessions are recorded in the report.

14 whole-frame Soft/actual Metal pairs (28 complete P6 frames) cover two themes,
1400/800px windows, Auto off/manual Binary vs Auto on/JSON, read-only Pragmas,
and PNG/BMP images. Whole-frame max channel delta2, no channel above2. Both contact
sheets and raw light narrow JSON/PNG and dark BMP frames were reviewed. Existing
image controls still scroll past the reserved text area; this accepts content and
format switching, not final image layout polish.

20 prepared native windows use ready/PID/CGWindow/minimum-size gates. Four private
CGEventSource postToPid workflows start in manual Binary, press Space on Auto,
disable and re-enable it. Mode becomes JSON, disabling keeps the chosen JSON view,
and re-enabling returns to the exact on-frame (zero changed channels). Read-only
Pragmas retains disabled Apply. Each of24 owned processes is awaited after
SIGINT(-2). The helper does not read or modify the clipboard. Source/binary hashes
are rechecked after all drivers finish. This browser-only change does not alter
shared framework code; Office was inspected in the immediately preceding shared
viewport review and is not claimed as freshly recaptured here.

The initial UI history assertion mixed a readonly-to-Browse source refresh with
same-mode toggling; the fixture now finishes that transition first. A legacy
read-only policy test compared formatted display with raw JSON storage; it now
compares the typed saved value. The earlier449-test run's lone failed assertion
is retained in local v7 logs and is not accepted. Final workspace evidence uses
the corrected fixtures. The backend tests also cover PNG/JPEG/BMP draft import,
exact original-byte Apply, corrupt-image fallback, manual mode persistence,
printable BLOB edits, integer/binary/noncharacter classification and readonly/error
boundaries without database writes from mode changes.

Mob still shows wallpaper/loginwindow. This is not ordinary frontmost pointer,
typing, IME, picker, drag, a11y, Print or other-host runtime acceptance. Startup
sizes are not OS resize. RTL shaping, SVG/other Qt formats/codecs, BOM semantics,
word/style wrap, indent/compact defaults, cross-mode histories and full parity
remain unfinished. Automatic parsing/image decoding are synchronous and retain
the existing16MP/128MiB image decode bounds. Image recognition requires successful
bounded decode before accepting the Image view.

Primary source: v3.13.1 EditDialog.cpp/Data.cpp/Settings.cpp:
https://github.com/sqlitebrowser/sqlitebrowser/blob/v3.13.1/src/EditDialog.cpp
https://github.com/sqlitebrowser/sqlitebrowser/blob/v3.13.1/src/Data.cpp
https://github.com/sqlitebrowser/sqlitebrowser/blob/v3.13.1/src/Settings.cpp

Local scripts/logs retain /tmp/next-gui-cell-auto-v8- (early runs v1..v9). Portable
metrics and images are archived here. No publication was performed.
