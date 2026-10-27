# Capture provenance

Implementation `0ff71c70e6fd546a3aa077878e871873e203a823`, macOS27.0.1 arm64, 2026-10-28.

- `fields-contact.png`: completed `NEXT_GUI_DB_BROWSER_FIELD_EDITOR_QA_OUT=/tmp/next-gui-field-editor cargo test --offline --locked -p e28_db_browser field_text_cells -- --nocapture`, both themes at1400×1000/800×600. Writer54738 exit0 awaited before PNG conversion/contact; all four Soft/Metal maximum channel deltas1. Full small light image and contact inspected.
- `native-dark.png` / `native-light.png`: exact final build, owned GPU in-memory `table-field-editor` fixture, ready logs confirmed, owned onscreen windows2846/2849 at260,48/1400×892. Helper14835 final exit0, owned children SIGINT-2 awaited. Name selection visible in both themes; full images inspected. Targets inactive.
- `office-contact.png`: same final build,14 owned native windows e10–15/e20–27. Sequential helper14835 exit0, every owned SIGINT-2 exit awaited; contact inspected.
- Interactive fixture PID65683/window2839 was inactive; desktop screenshot showed lock screen. No keyboard/pointer sent through lock screen. PTY30505 stopped withCtrlC, actual exit1 awaited. These captures establish static rendering only.

Final logs: `/tmp/next-gui-field-editor-{workspace,clippy,build,features,visual}-complete.log`; `/tmp/next-gui-field-editor-native-{dark,light}-complete.log`; `/tmp/next-gui-field-editor-office-native-complete.log`. Workspace512 tests/43 groups, e28296/UI151; all final required commands exit0 awaited.
