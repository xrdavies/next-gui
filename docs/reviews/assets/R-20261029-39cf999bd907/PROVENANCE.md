# Cell wrap and horizontal viewport evidence

Target: `39cf999bd9071e1005305aea9bc057dc0c04aff3`; shared framework parent: `87b82e24f929b244b74136d4ef1696a39072b34b`.
macOS27.0.1 arm64, Rust1.86.0, CommandLineTools; 2026-10-29 JST.

Final source freeze covers74 Rust files across next-gui-text, next-gui-ui and
the browser; hashes and final browser/14 Office binary hashes are in metrics.json.
Offline workspace tests report725 passed/43 groups, no failed or ignored; strict
all-target Clippy -D warnings, workspace build and host macos/windows feature
checks all exit0. A final focused browser test also passed after the QA-only
assignment-spacing lint fix. Initial Clippy failures (boolean simplification and
assignment spacing) are retained in local logs; no failed check is accepted.

16 complete P6 frames produce8 whole-frame Soft/actual Metal pairs; max channel
delta2, zero channels above2. Both contact sheets and representative raw narrow
frames were reviewed. The cell wrap test also checks unchanged draft/revision,
selection/history, read-only actions, font/theme/size rebuilds and fixed footer.

12 ready/PID/CGWindow/size-guarded prepared browser windows and4 targeted private
CGEventSource postToPid flows cover two themes. Home/End navigate long physical
lines; repeated Space on the focused Wrap control changes wrapping and retains
focus. Each flow changes over200 channels then restores the entire window exactly
(zero changed channels). No clipboard is read or changed by this helper. All30
owned browser/Office processes were captured and awaited after SIGINT(-2).
Source and final binary hashes are checked again after native drivers finish.

Mob still shows wallpaper/loginwindow. These tests do not accept ordinary
frontmost pointer/typing/IME/picker/drag/native a11y/Print or other-host runtime.
Prepared startup sizes are not OS resize acceptance. Emoji fallback, RTL shaping,
full grapheme caret handling and original word/style-boundary wrapping remain.
Enabled wrapping still uses the framework's existing character layout. Whole
text layout remains O(document length); each clipped viewport/glyph mask has a
64 MiB budget, with larger viewports requiring tiled text rendering.

Primary upstream: v3.13.1 EditDialog.cpp setWordWrapping and Settings.cpp defaults:
https://github.com/sqlitebrowser/sqlitebrowser/blob/v3.13.1/src/EditDialog.cpp
https://github.com/sqlitebrowser/sqlitebrowser/blob/v3.13.1/src/Settings.cpp
Scintilla word/style-boundary wrap behavior:
https://www.scintilla.org/ScintillaDoc.html#SCI_SETWRAPMODE

Scripts/logs retain the /tmp/next-gui-cell-wrap-v7- prefix. PNGs/portable metrics
are archived here. The earlier v6 check failure and v7 initial spacing failure
are not represented as successful final checks. No publication was performed.
