# Editable declared types and Autocomplete

Code: `30646db63f83130caac78b16de69bea091b88ca0`, macOS arm64.

- `NEXT_GUI_DB_BROWSER_WIZARD_QA_OUT=/tmp/next-gui-editable-type-reviewed-wizards cargo test --offline --locked -p e28_db_browser schema_wizard_selectors_keep_qualified_targets_types_and_scroll -- --nocapture`: twenty-two exact scene pairs/44 actual Soft and Metal readbacks. Both-theme table wide/top/suggestions/IME and narrow top/bottom/suggestions/IME; index wide and narrow top/middle/bottom. Both contacts skimmed; narrow light Metal suggestions inspected full-size. Independent whole frames max2 (wide light suggestions2, every other frame1). PPM converted with sips. All existing parent-scroll/fixed-action reachability assertions pass. Exit0 awaited.
- Fresh native own GPU e28 windows2038 dark and2037 light, on-screen at260,48, 1400x892 scale1, active=false/hidden=false; both inspected. Type field shows DECIMAL(10,2), list includes an untyped column. Fresh Office26 Soft own window2036, on-screen at600,31, 720x959 scale1; initial viewport inspected, Autocomplete is below it. No native scrolling acceptance. Each owned child SIGINT/awaited (-2), helper exit0.
- `soft-contact.png`, `metal-contact.png`: complete contacts. `type-options-light-metal.png`, `type-ime-dark-metal.png`: full narrow scenes. `native-dark.png`, `native-light.png`, `office26-native.png`: actual window captures.

Rendered preedit is driven through the shared Ui API, not OS IME. No native input/clipboard/picker, original binary or Linux/Windows runtime acceptance. No complete Office sweep; the changed shared component is used by e26/e28.
