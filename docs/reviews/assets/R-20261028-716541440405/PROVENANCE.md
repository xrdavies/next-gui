# Column CHECK verification

Final implementation `716541440405e16a936dcfb37dfc61bb6601d2c5`. Actual test scene readbacks, no mock images or generated assets.

`NEXT_GUI_DB_BROWSER_CHECK_QA_OUT=/tmp/next-gui-column-check-final cargo test --offline --locked -p e28_db_browser table_wizard_check_editor -- --nocapture` completed exit0. Four 1400×1000 Soft/Metal pairs: dark draft/saved max2/2, light draft/saved max1/1, measured across the entire frame including primitive edges. Both contact sheets and full light draft inspected. Draft follows deliberate rejected saves; the footer retains the last operation status. CHECK SQL and fixed Save/Cancel fit the form.

Native GPU surface: isolated memory QA tabs `table-check` and `table-checked`, dark draft/light saved. Own-PID CGWindow IDs2073/2072, bounds1400×892 at260,48, screen1920×1080 scale1. NSApp active=false, hidden=false. Captured via screencapture; both full images inspected. Helpers exited0; owned app children were SIGINT-stopped and exits-2 awaited. Dark form intentionally starts scrolled to the bottom, displaying the full CHECK field and fixed actions.

Source logs `/tmp/next-gui-check-visual-complete.log`, `/tmp/next-gui-check-native-{dark,light}.log`; native metadata in corresponding capture directories. No pointer/keyboard/OS IME/clipboard/picker acceptance: programmatic input and static capture are separate evidence. No original-binary, Linux or Windows runtime comparison. Shared renderer/chrome unchanged; no Office catalog sweep repeated for the session-only TextArea API change.
