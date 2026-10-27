# Capture provenance

- macOS 27.0.1 arm64, tested code `caca01ed09f664ae90d930b14d7d26cc17bc8e75`.
- `native-viewport-soft.png`: SoftRenderer output from `native_xml_restores_and_writes_options_for_tables_that_are_not_active`, QA_OUT; native-convention fixture `(n,label)` imported, first visible column frozen, widths 177/233, `n<=9`, descending sort. QA explicitly selects Browse Data.
- `browse-native-metal.png`: owned e28 CGWindow capture, `NEXT_GUI_GPU_SURFACE=1 NEXT_GUI_DB_BROWSER_QA_TAB=browse`; sample database. Static painting only, not native project opening/input.
- Original DB4S binary, native input/IME/OS clipboard and Windows/Linux runtime were not exercised. Primary-display capture remains wallpaper; no desktop settings changed.

| Asset | SHA-256 |
|---|---|
| `native-viewport-soft.png` | `c8c0bb4e945e9cd172894e7186116c801e32c057c9a5e91882a4b91f52523e43` |
| `browse-native-metal.png` | `5d67ff8b7dd6e2631d4e3c4a72e4db042779916ddad6a8ec335e2f419c77bc6b` |
