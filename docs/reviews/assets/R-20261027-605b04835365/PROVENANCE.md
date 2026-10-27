# Capture provenance

- macOS 27.0.1 arm64; code `605b04835365492845d25c16805a94a1f9bc809a`.
- At this committed target, ran `NEXT_GUI_DB_BROWSER_COMPLETION_QA_OUT=/tmp/next-gui-completion cargo test --offline --locked -p e28_db_browser sql_completion_popup_dark_light_and_metal_readback -- --nocapture` and the `preferences_cancel_keeps` regression with the same prefix. Converted their actual Soft PPM frames with `sips -s format png`.
- Completion fixture: 25 physical comment lines, scrolled caret on line 26, live function/table/column/database candidates including quoted and Unicode names. Both dark/light Metal readback comparisons actually ran, maximum channel delta 1 in the panel.
- Preference image is the actual draft overlay before the Cancel assertion; the regression changes the switch, snapshots the draft, and verifies live preferences remain unchanged on Cancel. It does not save user preferences.
- Fresh mob screenshot reports the office display asleep. Native e28/14 Office captures and native key/pointer/IME/clipboard acceptance were not run for this slice. Prior native captures are historical evidence only.

| Asset | SHA-256 |
|---|---|
| `completion-dark-soft.png` | `28ea92c744b24cec4bc6baa87c0a35dbb9fbe49c060d6e0304534474a733a613` |
| `completion-light-soft.png` | `b431578290a6818e6c26becbc86b26238785606666e0e15c4a2a23ce52785f36` |
| `preferences-soft.png` | `03f74dcd94d22f04c0cd993fe17e107a025069f7cd3cd33d4dc136941d64b895` |
