# Plot hover evidence

Reviewed target: `60838586a5cb270e44c7b8b2632748f8c9176cee`; shared implementation
`b8c4bf6650c9d205007c1a083fe3148e7e77bce5`; macOS27.0.1 arm64/Rust1.86.0.
All shell commands used `DEVELOPER_DIR=/Library/Developer/CommandLineTools`.

- Final public-feature regression: `/tmp/next-gui-plot-hover-v11-checks.py`, session31308 exit0 consumed:701 workspace tests/43 groups, focused coordinate tests, strict all-targets Clippy, workspace build, macOS/Windows host feature checks.
- Subsequent changes only moved the opt-in native fixture's hover setup after layout and guarded an absent chart. `/tmp/next-gui-plot-hover-v12-qa.py`, session93931 exit0 consumed: fresh strict Clippy, application regression, workspace build and four corrected native captures. The public feature/source hashes stayed fixed. Source and binary hashes are in metrics.json.
- Render: `NEXT_GUI_PLOT_HOVER_QA_OUT=/tmp/next-gui-plot-hover-v10-render cargo --offline test -p e28_db_browser plot_hover_keeps -- --nocapture`; final35587 exit0 consumed. Four Soft and four actual Metal frames, dark/light1400×1000/800×600. Full P6 dimensions/payload were validated after exit before sips conversion. All seven committed image contacts/frames were viewed. Chart maxima1; whole-frame2/1/1/1, zero channels above2.
- Native corrected aliases: `NEXT_GUI_DB_BROWSER_QA_TAB=plot-hover` or `plot-hover-narrow`, `NEXT_GUI_DB_BROWSER_QA_THEME=dark|light`, `NEXT_GUI_GPU_SURFACE=1`. Owned PID/CGWindowID and ready-log guards preceded screencapture. The fixture sets hover/focus during drawing; these are prepared-hover captures, not actual pointer acceptance. All four apps ended by owned SIGINT and awaited exit-2. The first v10 native captures lacked hover after startup events; they are excluded from assets.
- Native keyboard: interactive session89929, app94168/window5982, previous frontmost608. Observed the initial window, then sent private-source CGEvents Right/Home to this PID without activation. Commands session85003 exit0 consumed; app SIGINT exit-2 consumed after `finish`. Dates14→16→14 and cell1430 retained. Independent CoreGraphics RGBA comparison: Right8371 changed channels; Home0 changed channels. No clipboard, picker, print, database write or other application action.
- Office: v10 driver35587 sequentially opened e10–e15/e20–e27 (14 windows), captured each owned window, stopped each owned app and awaited exit-2. Both seven-window contacts inspected. Shared production code matches the reviewed target; later changes were e28 fixture-only. Static chrome acceptance only.
- Primary Mob screenshot again returned wallpaper. No ordinary frontmost pointer, drag, IME, picker, native leave gesture or Windows/Linux runtime claim.
- Original v3.13.1 PlotDock.cpp local Git blob hash `f1bb50737123575cac182cf8ca583a34ef3aba3e`, mouseMove lines921–950; QLocale/local-time behavior is not claimed equivalent.

Publication remains local after the earlier Mob push approval timed out with denial;
last verified remote `b64d8d8e0e64c2b8806f0ca9f2d6ba8086e573d5`.
