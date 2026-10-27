# Fields grid verification

Implementation cdf760f3ad1d4e721feeb43b3aa0f8aede5a4628. Final502 workspace tests/43 groups, e28292/UI145, strict clippy/build and macOS/Windows feature check passed; all exits awaited. Logs `/tmp/next-gui-field-grid-{workspace,clippy,build}-verified.log`, features-final.log.

`NEXT_GUI_DB_BROWSER_FIELD_GRID_QA_OUT=/tmp/next-gui-field-grid-verified cargo test --offline --locked -p e28_db_browser field_ -- --nocapture`: two tests, four1400×1000 Soft/Metal pairs. Dark overview2/references2, light overview2/references2 maximum channel delta. Contacts both inspected; input PPMs at the above prefix, log visual-verified.log. Final Yes flags replace the missing-glyph checkmark found in earlier captures.

Native GPU: table-fields dark/window2190 and table-fields-references light/window2200,1400×892 at260,48,1920×1080 scale1, active=true. Both full captures inspected. The ready helper filters own-PID onscreen layer0 windows by minimum size/readiness, excluding auxiliary500×500/menu windows. Helpers0/owned SIGINT children-2 awaited. Directories `/tmp/next-gui-field-grid-native-final-{dark,light}` retain metadata/logs.

Limited native input: owned PID51050/window2210; mouse focus on Name, Cmd+A and paste customer_name updated Name, grid and dependent CHECK/SQL preview. Cmd+Z twice did not undo; shared implementation gap recorded as F01. `native-input-undo.png` records this failure. The desktop later showed another foreground app; no controls in that app or its notification permission prompt were touched. Owned PTY58506 stopped with Ctrl-C, actual exit1 awaited. No native Save/IME/copy/original-binary/cross-platform acceptance is claimed.
