# Compact icon toolbar evidence

Reviewed application `716afa8e3e77cad16fbd9851fbeb292aea697049`; shared UI `65c1ab054f9ba575ba2f6edbfd1ecd94e33c5e09`. macOS27.0.1 arm64,
Rust1.86.0. All shell commands used DEVELOPER_DIR=/Library/Developer/CommandLineTools.

- Frozen source: `/tmp/next-gui-icon-buttons-v6-source.json`; all hashes matched after checks and before commits. Source/binary hashes are in metrics.json.
- Final checks: `/tmp/next-gui-icon-buttons-v6-checks.py`, session37546 exit0 consumed:705 workspace tests/43 groups, zero failed/ignored; strict all-targets Clippy, workspace build and host macOS/Windows feature checks all0.
- Focused shared icon/DSL/glyph/coordinate tests: v4 session18401 exit0 consumed; app `icon_toolbars_fit_narrow_pages_keep_help_and_retain_actions_and_drafts`, v5 session15091 exit0 consumed. The first DSL test used the wrong named-widget syntax; corrected before final verification.
- Render: `NEXT_GUI_ICON_TOOLBAR_QA_OUT=/tmp/next-gui-icon-buttons-v5-render cargo --offline test -p e28_db_browser icon_toolbars_fit -- --nocapture`. Eight Soft and eight actual Metal frames, dark/light, Browse/SQL,1400×860/800×600. All8 chart interior maxima1; whole-frame1/1/2/2/2/1/2/2, no channel above2. Conversion session20602 exit0 consumed validated complete P6 headers/payloads after writer exit before sips. Both contacts and raw light narrow SQL frame viewed.
- Native v5:8 windows using toolbar-browse/toolbar-sql with optional -narrow, both themes and GPU1. Ready/PID/owned CGWindowID/minimum-size guards; every app SIGINT exit-2 awaited. Driver32990 exit0 consumed; contact and raw light narrow SQL viewed. Opt-in fixtures prepare focus once after layout; no ordinary pointer claim.
- Native keyboard v6: session7770, app95906/window6013, previous frontmost608. Inspect before; private-source process-targeted Enter activates Comment, Cmd+Z undoes, Cmd+Shift+Z redoes. Commands26349 exit0; final app SIGINT exit-2 consumed. CoreGraphics region comparison75315 exit0:3790 changed comment channels; Undo/Redo editor regions each0, Plot/results and cell1430 regions each0. Contact/raw changed frame viewed. No clipboard or database write.
- Final Office v7: session6493 exit0 consumed, launched after final workspace build,14 e10–e15/e20–e27 current binaries, all owned SIGINT exits-2 awaited. Contact conversion29716 exit0 consumed; both seven-window contacts viewed. Earlier v6 Office captures preceded the final workspace build and are excluded.
- Original v3.13.1 MainWindow.ui local Git blob bf2dd590c0bbb3c68606e3505c17872fb1e55991: SQL toolbar actions1235ff, DB/Project TextBesideIcon1402ff/1656ff; Settings.cpp246ff defaults confirmed. Current compact icons do not establish original toolbar mode/action-order/overflow parity.
- Primary Mob capture showed wallpaper; PID608 is loginwindow. No ordinary frontmost pointer/drag, picker, IME, physical printing or Linux/Windows runtime acceptance. No account/security settings changed.

Publication remains local after the earlier Mob push approval timeout denial;
last verified remote b64d8d8e0e64c2b8806f0ca9f2d6ba8086e573d5.
