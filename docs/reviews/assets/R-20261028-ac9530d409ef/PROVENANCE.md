# Current result plotting evidence

Target `ac9530d409efbd31ec0ef84796677860d94362cb`, base `f2945ebca53b8671de28d9a9e08a60c0423bcb5a`, shared ColorPicker fix `7278ac92bb192fe463590d59cfcae01d5717208a`.
macOS27.0.1/arm64, Rust1.86.0, CommandLineTools, actual Metal.

Final required driver `/tmp/next-gui-current-plot-v5-checks.py`, session34804,
actual exit0 awaited. Workspace tests independently counted666 passed in43 groups,
zero failed/ignored. Strict workspace/all-targets Clippy, workspace build and host
macOS/Windows feature checks all exited0. Logs:
`/tmp/next-gui-current-plot-v5-{workspace,clippy,build,macos,windows}-final.log`.
Earlier runs caught a cfg(test)-only production helper call and the two ColorPicker
failures; the final source passes. Windows feature compilation is not Windows execution.

ColorPicker pixel regression99414 exit101 reproduced hidden text. After raising
the text,66793 exit101 exposed the empty-palette key guard.88885 exit0 verifies
visible pixels, pointer focus, Enter publication and empty-palette arrow handling.
The final full suite repeats this regression successfully.

Final UI driver77110 exited0 after another workspace build, sequential native
captures, then the render integration test. Logs:
`/tmp/next-gui-current-plot-v4-{build,native,render}-qa.log`.
`NEXT_GUI_CURRENT_PLOT_QA_OUT=/tmp/next-gui-current-plot-v4` drives the checked-in
`current_plot_controls_link_rows_preserve_drafts_and_follow_browse_and_sql_results`
test. Eight P6 frames are Browse/SQL × dark/light ×1400×1000/800×600.
Each payload length was checked against width×height×3 after writer exit before
sips conversion. Every actual plot-interior Soft/Metal channel delta is1.
This comparison excludes tile edges and is not a whole-window pixel-equality claim.
The test intentionally exercises a rejected cell draft; retained informational
status text in those diagnostic frames is not an unhandled error.

Native fixtures `plot-current-browse` and `plot-current-sql` use the checked-in
owned in-memory18-row dataset, date X, revenue Y1, expenses Y2, NULL break and Disc
markers. Native screenshot helpers `/tmp/next-gui-current-plot-v4-native.py` and
`/tmp/next-gui-wizard-native-ready-probe.py` use `NEXT_GUI_GPU_SURFACE=1`,
`NEXT_GUI_DB_BROWSER_QA_THEME=dark|light`, and
`NEXT_GUI_DB_BROWSER_QA_TAB=plot-current-browse|plot-current-sql`.
They wait for the owned PID's onscreen window and app-ready marker, capture its
CGWindowID, stop only that child, and await exit−2. Four browser windows and all14
Office examples were captured sequentially and inspected. Raw images/logs live
under `/tmp/next-gui-current-plot-v4-native-*` and
`/tmp/next-gui-current-plot-v4-office-native`.
The new ColorPicker hex text is visible in native-browse.png. Office captures are
regression scans; they do not imply every scrolled-off control received native input.

Fresh primary Mob capture still showed wallpaper without listed windows. No blind
pointer/keyboard action, security-setting change or native input acceptance claim.
Programmatic widget input and owned-window static captures are separate evidence.

Original source contract:
https://github.com/sqlitebrowser/sqlitebrowser/blob/v3.13.1/src/PlotDock.cpp
The45114-byte file was fetched through GitHub's contents API, decoded and checked
against its Git blob SHA1 before reading. Local copy:
`/tmp/next-gui-db4s-reference-v3.13.1/PlotDock.cpp`.
The verified official3.13.1 app baseline remains in the prior review; its Plot dock
and complete gesture/export behavior have not yet received native acceptance.

Asset SHA256:
- `render-contact.png`: `344af766c04129be3ef538b3ee4ed35af946bbe555217843e6828da13336051f`
- `native-contact.png`: `77bd2f5c0f017a07585536c4c80b867b4a0d287b6aba6d7a055458f2d6d559d0`
- `office-contact.png`: `3d534e0a7ce7ee952a58d88633ce29af0b24eb2dff5d98923eb5633eb71ff148`
- `native-browse.png`: `c81421495a33842ce6772466799fb9cedb938295d3420c4acf0474c09b08dbcb`
