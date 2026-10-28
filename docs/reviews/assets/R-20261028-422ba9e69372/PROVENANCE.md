# Fields CHECK acceptance provenance

Implementation: `422ba9e69372e134a055b753061599526c1ff6d7`, macOS27.0.1 arm64, Rust1.86.0.

`field_check_text_cells_preserve_named_checks_cancel_history_and_qualified_data`
ran with `NEXT_GUI_DB_BROWSER_FIELD_CHECK_QA_OUT=/tmp/next-gui-field-check-final`.
Four final Soft/Metal pairs: dark1400/800 delta2/1, light1400/800 delta2/1.
Actual test exit0 awaited before PPM conversion with sips. Full contact and small
light capture inspected. A first inspection caught a partially hidden800px editor;
all active Fields editors now reveal their cell and geometry assertions pass.

The final built e28GPU ran QA_TAB `table-field-check-editor`, both QA_THEME values,
under the ready-window helper, output `/tmp/next-gui-field-check-native-final-*`.
Each captured its own onscreen layer0 window after its ready log, received SIGINT
and was awaited exit−2; helper exit0. Final native contact inspected. Shared UI
code did not change, so Office captures were not repeated.

Desktop screenshot still shows wallpaper and owned windows are inactive. Native
static capture does not establish keyboard/pointer/IME/Save acceptance. Original
binary and Windows/Linux runtime unavailable.

Final workspace544 tests/43 groups, e28316/UI163, no failures/ignored; actual exit0
awaited. Strict Clippy/build/macOS and Windows feature checks each awaited exit0.
An initial assertion expected != while the existing AST emitter normalizes to <>;
the assertion was corrected. The first geometry assertion used a nonexistent Ui
accessor; it now reads the existing Scene paint nodes. Final checks pass.

No dependency/config/platform/shared-widget source changes. No push: publication
approval1960 timed out/denied earlier in this session.

- `native-contact.png` SHA256 `cace280ff3bfa62d1a5189715cef47866a97d8f75c99bb92f384e30b32a7ce38`
- `native-light.png` SHA256 `06613566ea0318234e1117a53dfa5fe7ad21c7a7027dc506ab0d3c6cbbe01e39`
- `small-light.png` SHA256 `63fc2c16d52109579fd0fc866862a1295bdf5360cf21a236e162fdd01875555c`
- `soft-metal-contact.png` SHA256 `a929ce0e94f70f1359e5d75d15e75f7a8f96197b397bfba14ddafbf74bc679d4`
