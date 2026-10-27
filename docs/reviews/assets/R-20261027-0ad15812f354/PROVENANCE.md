# Capture provenance

- macOS 27.0.1 arm64; code `0ad15812f354b0f2651e82153fd082644e1da553`.
- Soft: actual `browse_tab_buttons_restore_directional_selection_and_reject_busy_switches` QA_OUT; two independent tabs on an in-memory fixture, directional selection restored by application buttons/polling/rebuild. Light regenerated with the correct `NEXT_GUI_DB_BROWSER_QA_THEME=light` after an initial unused environment variable produced a second dark image; only the corrected light image is retained here.
- Native: owned e28 CGWindow, GPU_SURFACE=1, QA_TAB=browse, ordinary sample. Static painting only; does not demonstrate selection restoration or busy input.
- No native input/IME/OS clipboard, original DB4S binary or Windows/Linux runtime acceptance. In-memory test fixture changes only; no user database writes.

| Asset | SHA-256 |
|---|---|
| `selection-dark-soft.png` | `e384104a1f09d803880cd8cb3f94ff79c6db2d0d369cb553f661a74253df15f2` |
| `selection-light-soft.png` | `1c9de914c16ccfec2987efd4fc543c2436f89f75cb13e7518e510a2a5b3f1621` |
| `browse-native-metal.png` | `85c64cd01a5e4d1338107d50931cb11cc4efc6eb019b07f1e838fb8da477e48f` |
