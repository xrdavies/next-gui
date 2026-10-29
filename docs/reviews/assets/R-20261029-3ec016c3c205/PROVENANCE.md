# Dedicated image preview evidence

Target: 3ec016c3c2059e56ad264f6f6936660841bb50c6; parent: 05ea4ea.
macOS27.0.1 arm64, Rust1.86.0, CommandLineTools;2026-10-29 JST.

All browser Rust sources are frozen and hashed. Metrics record those hashes and
final browser binary SHA256. Offline workspace tests729/43 groups, zero failed or
ignored; strict all-target workspace Clippy -D warnings, workspace build and host
macos/windows feature checks all0. Focused preview25146 and dock80607 exit0; final
checks85495, render68465 and native42401 exited0 and were consumed.

12 whole-frame Soft/actual Metal pairs (24 complete P6 frames) cover two themes,
1400px/font13 and800px/font18/font28, editable Browse and readonly Pragmas. The
preview is at least96px high, replaces the text editor, and fills the remaining
cell content height. Image tools scroll horizontally. The inner content viewport
has no vertical overflow, including the empty-preview path; Apply/Discard/metadata
remain in the footer. Fit,1:1, zoom, bounded pan, focused image/tools, source bytes
and readonly guards survive rebuilds. Both sheets and raw light/font18 and dark/
readonly/font28 frames were reviewed. Font28 still needs outer shell scrolling;
these are not final whole-product layout acceptance screenshots.

10 prepared native windows use ready/PID/CGWindow/minimum-size gates, covering
PNG/BMP wide/narrow and PNG readonly in both themes. Four private CGEventSource
postToPid workflows start focused on1:1 while the image fits, press Space, move
Tab to zoom in, then Shift+Tab to Fit, press Space and return focus to1:1. Actual
size and zoom change the frame; Fit returns to the exact starting frame. Each of
14 owned processes is awaited after SIGINT(-2). The helper does not use the
clipboard. Archived metrics recheck sources after drivers and native-all verifies
binary hashes before and after. No shared framework changes; Office captures
remain in the immediately preceding shared viewport review, not freshly claimed.

Mob still shows wallpaper/loginwindow. No frontmost pointer/typing/IME/picker,
actual pan drag, OS resize, accessibility, Print or other-host runtime acceptance.
The pan regression uses the existing bounded ImageView API, not native dragging.
Source tests now assert zero image-content scroll because removing the redundant
text area deliberately eliminates the old overflow; Plot scroll retention remains
positive and tested. Generic text/binary/JSON/XML/evaluation views still use the
existing TextArea and its session policy.

Remaining original modes/actions, image codecs/SVG, RTL/BOM, word/style wrap,
indent/compact/error workflows, cross-mode histories, shell/docking and full visual,
product/native/platform parity remain unfinished. Image decoding is unchanged:
16MP/128MiB bounded synchronous PNG/JPEG/BMP preview. Native Print/remote and broader
schema/Browse/SQL/Plot workflows remain partial. No completion/publication claim.

Local scripts/logs: /tmp/next-gui-cell-image-v1-. Portable metrics/images/helper
source archived here. No publication performed.
