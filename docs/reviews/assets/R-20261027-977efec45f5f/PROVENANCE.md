# Fractional primitive rendering evidence

Code: `977efec45f5f351b0da231a67eff2b8ac1afd486`, macOS arm64.

- Ten exact scene pairs: `NEXT_GUI_DB_BROWSER_WIZARD_QA_OUT=/tmp/next-gui-primitive-wizards cargo test --offline --locked -p e28_db_browser schema_wizard_selectors_keep_qualified_targets_types_and_scroll -- --nocapture`. Table/Index, dark/light, 1400x1000/800x600, plus Table narrow bottom. Actual Metal readback and Soft PPMs converted with sips. Both ten-frame contacts skimmed; dark table pair and light bottom at full size inspected. UI test now asserts viewport including internal primitive edges (outer 2px inset) <=2. Independent PPM comparison across the entire frame: all ten pairs delta 1 except wide light Table delta 2. No image tolerance masking.
- Original List edge (301,315), wide dark Table: Soft (43,79,147), Metal (44,79,147), replacing prior max 73 difference.
- Native helper `/tmp/next-gui-primitive-native-probe.py` launched fifteen fresh built apps, own PID/CGWindow lookup, screencapture, SIGINT and awaited child exit -2 for all; helper exit 0. All fifteen windows on-screen, active=false/hidden=false. e28 Table wizard and all fourteen Office windows inspected via full-size e28/e23 and Office contact. e10/e11/e12/e13/e14/e15/e20/e24 logs explicitly report Metal native; e21/e22/e23/e25 visibly GPU; e26/e27 visibly Soft. This is static painting acceptance, not input acceptance.

No native pointer/keyboard/IME/clipboard/picker/original binary/Windows/Linux runtime check. No publication performed.
