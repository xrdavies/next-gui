# Indent / Compact cell evidence

Target: 6153c6ec8f5ec73e90d076efe90f79ed2dc9de52; parent: 6bd60ae.
macOS27.0.1 arm64, Rust1.86.0, CommandLineTools;2026-10-29 JST.

Source freeze includes42 browser Rust files plus Cargo.toml/Cargo.lock; hashes
and final binary SHA256 are in metrics.json. Final workspace733/43 groups, zero
failed/ignored; strict workspace all-target Clippy -D warnings, workspace build
and host macos/windows feature checks all0. Final checks33440, render5586 and native69610 all exited0 and were consumed. The earlier v6 checks/render/native runs passed, but
editable clean formatting scrolled to the ending brackets in a narrow window.
Clean toggles now reset caret/viewport to the start; dirty editing keeps its history.
Final v7 evidence is recollected after this visual correction. No dependencies/features were added.

12 whole-frame Soft/actual Metal pairs (24 complete P6 frames) cover two themes,
1400/font13 and800/font18, JSON/XML, editable and readonly Pragmas. Checked tool,
raw/default vs formatted views, unchanged clean storage, retained focus/revision,
Undo/Redo, dirty XML Apply and readonly guards are asserted. Both sheets and raw
narrow JSON/wide XML frames were reviewed. Native prepared windows cover both
formats and two sizes; private process keys cycle the focused option on/off/on
in two themes and readonly/editable sources. Off retains raw clean data, first
and repeated on-frames match exactly. Every owned process is awaited SIGINT(-2),
no clipboard is used, and final sources/binary are rechecked after drivers.

Early v2 state regression exposed Apply writing compact bytes then rejecting its
own refresh because the old display baseline still looked dirty. Compacted Apply
now refreshes its stored/display baseline before browsing refresh. The v3 JSON
candidate's arbitrary_precision feature could reinterpret a specially named object
as a number; that candidate was rejected, the feature removed, and a lossless
whitespace formatter uses the existing serde validator without constructing Value.
Regressions cover that key, duplicate keys, huge integers/fractions/exponents,
escaped strings, empty containers, null, malformed/trailing JSON and format bounds.
The v5 workspace run failed3 legacy tests expecting unconditional pretty JSON;
they now explicitly enable the original-default-off preference. v5 is not accepted.
An early Clippy assignment-spacing failure was corrected. XML tests cover raw
BOM/prolog/DTD/attributes/comments/PI/entities, mixed content, inline spaces,
whitespace-only leaves, CDATA, xml:space, existing line indentation, idempotence,
invalid syntax and existing bounds. Backend tests cover TEXT/BLOB, raw off-Apply,
clean formatted views and explicit typed compaction with safe record restoration.

Mob still shows wallpaper/loginwindow. No ordinary frontmost pointer/typing/IME,
picker/drag/resize/a11y/Print or Linux/Windows runtime acceptance. Private CGEvent
postToPid acceptance is reported separately. This browser-only change does not
alter shared code, and no fresh Office capture is claimed. Narrow tools still
scroll horizontally; large fonts retain outer shell scrolling.

JSON formatting:16 MiB input,32 MiB output,depth128. XML retains16 MiB input,
250000 parser nodes,depth128,32 MiB output. XML compaction normalizes only literal
inter-tag line indentation, preserving inline spaces/mixed text/CDATA/entities/
xml:space; it does not emulate every Qt QDom serialization normalization. Original
invalid-format indicators/override, full tab-width preferences, cross-mode/history
workflows, RTL/BOM/codecs/binary UI, original Print/Remote and broader visual/
product/native/platform parity remain unfinished. No completion/publication claim.

Primary v3.13.1 EditDialog.cpp loading/application/default preference behavior:
https://github.com/sqlitebrowser/sqlitebrowser/blob/v3.13.1/src/EditDialog.cpp
https://github.com/sqlitebrowser/sqlitebrowser/blob/v3.13.1/src/Settings.cpp
Qt QDom whitespace behavior reference (later Qt docs distinguish preserved nodes):
https://doc.qt.io/qt-6.8/qdomdocument.html
The formatter intentionally retains content tokens rather than QDom normalization.

Local final scripts/logs use /tmp/next-gui-cell-indent-v7-; earlier rejected runs
v1..v5 remain local. Portable metrics/images/helper source archived here.
