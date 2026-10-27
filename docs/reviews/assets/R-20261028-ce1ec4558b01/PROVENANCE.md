# Capture provenance

Implementation `ce1ec4558b01c538c6e86e689a9c5dd7c8db6a56`, macOS27.0.1 arm64, 2026-10-28.

- `grids-contact.png` / `small-light.png`: final `NEXT_GUI_DB_BROWSER_CONSTRAINT_GRID_QA_OUT=/tmp/next-gui-constraint-grid cargo test --offline --locked -p e28_db_browser constraint_grids_keep -- --nocapture`. Writer81597 exit0 awaited, then PNG conversion/contact. Three pages at1400×1000 in both themes plus CHECK at800×600 in both themes: eight Soft/Metal pairs, max2 for dark index/FK, max1 for the other six. Contact/full small light inspected.
- `native-contact.png`: owned GPU in-memory QA tabs `table-keys-grid`, `table-foreign-keys-grid`, `table-checks-grid`. Dark windows2898/2932/2906; light2907/2936/2913, each1400×892 at260,48, ready logs confirmed. Helper10071 exit0 and all owned SIGINT-2 exits awaited. FK fixture initially retained bottom scrolling, corrected QA alias and reran both FK windows with final binary (helper21354 exit0; both SIGINT-2 awaited). Every full image inspected. All targets inactive; locked desktop prevents native input acceptance.
- `office-contact.png`: same shared renderer build,14 owned native windows e10–15/e20–27, inspected. Helper10071 and every owned SIGINT-2 exit awaited. Last e28-only QA fixture correction does not change Office binaries.

Final logs: `/tmp/next-gui-constraint-grids-{workspace-final,clippy-complete,build-complete,features-complete,visual-final,native}.log`; `/tmp/next-gui-constraint-grid-native-fk-complete.log`. Final515 workspace tests/43 result groups (e28298/UI152), zero failures/ignored; actual final exits awaited. Native captures establish static rendering only.
