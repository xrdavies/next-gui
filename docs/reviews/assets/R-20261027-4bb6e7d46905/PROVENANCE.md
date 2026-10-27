# Capture provenance

- macOS 27.0.1 arm64, code `4bb6e7d46905e95dce57962755ca90a7e82b73cf` (final addition is a known desired-column limitation comment).
- Dark/light Soft: `sql_editor_page_navigation_and_selection_survive_rebuild`, QA_OUT; Chinese 80-line SQL, caret begins on a middle visible line, PageDown then Shift+PageDown, rebuild retains selection/viewport. Light uses QA_THEME=light.
- Native SQL: owned e28 CGWindow, GPU_SURFACE=1, QA_TAB=sql-long without QA exit flag. This shows the ordinary sample SQL: long-fixture seeding requires the exit flag. Static painting only; no native page gesture claim.
- Office contact sheet: all fourteen e10–e15/e20–e27 owned windows captured and skimmed after final build.
- Primary display capture still shows wallpaper. No blind input or desktop/security changes. Original binary, native input/IME/clipboard and Windows/Linux runtime not exercised.

| Asset | SHA-256 |
|---|---|
| `page-dark-soft.png` | `cad425b21c11fff82d509f086054f5fc40448b51b316bf1ca83a121cccc2d1ce` |
| `page-light-soft.png` | `6902549a74edb57b66446739a326dbfe2d35eb75c8fbdf5c1123d8639b9788f3` |
| `sql-native-metal.png` | `d28f76d212bc6f58eee7345d1afc7565111397ecb7263d9a36ab664557b5089a` |
| `office-native-contact.png` | `6c5998845f654760202a899d95526ec12610e7b7c817772125d26c6f5251bf15` |
