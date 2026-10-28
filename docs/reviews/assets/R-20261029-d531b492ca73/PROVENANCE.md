# XML and evaluation cell evidence

Target: `d531b492ca732db1234e4608b8f3775bed9280ee`; host macOS27.0.1 arm64/Rust1.86.0.
Binary: `ff2ffda5a06db04ca60db2591b6ecf8555a8fa3b9f835456184b27483cd13ed1`.
Source files, artifact hashes, exact windows/processes and full-frame metrics are in `metrics.json`.

All commands used `DEVELOPER_DIR=/Library/Developer/CommandLineTools`.
Final `/tmp/next-gui-cell-formats-v15-checks.py` ran offline workspace tests,
strict all-targets Clippy, workspace build and host macOS/Windows feature checks;
79055 completed exit0. Focused identity regression and26 cell-related tests passed.
The source SHA256 freeze was verified before native runs and commit.

`NEXT_GUI_CELL_FORMATS_QA_OUT=/tmp/next-gui-cell-formats-v15-render cargo --offline test -p e28_db_browser cell_modes_keep_keyboard -- --nocapture`
completed exit0 (15768,75.65s). Only after writer exit,72489 validated22 complete
P6 bodies, converted with sips, compared11 full-frame pairs and generated contacts.
Every channel differs by at most2. Both final contacts and raw applied result were inspected.

Native fixtures: `NEXT_GUI_GPU_SURFACE=1 NEXT_GUI_DB_BROWSER_QA_TAB=cell-mode-{xml,evaluation,menu}[-narrow]`,
`NEXT_GUI_DB_BROWSER_QA_THEME=dark|light`; default font13, requested1400×860 or800×600.
The ready-probe requires the owned PID, visible layer0 CGWindow and minimum dimensions;
it captures only that window, sends SIGINT and waits for -2.33824 completed exit0
for all12 captures, then verified final static and keyboard binary hashes still match.
The labeled contact and raw narrow light popup were inspected.

The keyboard driver86089 completed exit0 for two additional800×600 windows.
The archived Swift helper uses a private CGEventSource posted only to the owned
`e28_db_browser` PID: Cmd+Enter, then Cmd+A/C. Clipboard data is copied in memory,
an owned marker guards the test, the exact result `中文A • 18` is required and the
original clipboard restored. Both copy-result logs confirm this. All six native
stages were inspected; Apply changes41721/41718 channels in dark/light.
No original clipboard contents were written to artifacts.

Earlier v12 checks passed numerically but native visual review rejected the cleared
editor after Apply. Temporary diagnostics traced this to refresh_browse clearing
selection/editor. They were removed; row-identity restoration, focus and assertions
were added. Only final v15 evidence is accepted here. The initial unsafe rusqlite
Value conversion panic on invalid UTF-8 TEXT was replaced with checked raw reading.

Primary display still shows loginwindow wallpaper. These are real owned Metal
windows and targeted process keyboard tests, not ordinary frontmost pointer,
IME/picker/drag/Print/a11y acceptance. No shared UI change: Office was not rerun.
No Linux/Windows runtime. All fixture databases are owned memory/scratch data.
