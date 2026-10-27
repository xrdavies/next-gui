# Ordered index keys and SQL preview

Code: `0333104cbccffd8c16a0fa28f466439c5f6748ef`, macOS arm64.

- `NEXT_GUI_DB_BROWSER_WIZARD_QA_OUT=/tmp/next-gui-index-key-rows-wizards cargo test --offline --locked -p e28_db_browser schema_wizard_selectors_keep_qualified_targets_types_and_scroll -- --nocapture`: fourteen exact scene pairs (28 frames), both themes, wide and narrow top/middle/bottom phases. Actual Soft/Metal readback, PPM to PNG via sips. Both contacts skimmed; wide dark and narrow light middle inspected full-size. Primitive-inclusive viewport delta <=2; independent whole-frame comparison: all Index and narrow Table frames max1, wide light Table max2. Middle phase asserts full key editor visibility; bottom asserts full SQL preview visibility; fixed Save and retained parent scroll offsets checked.
- Native fresh GPU dark/light, QA_TAB=index-namespaces and QA_THEME=light for light. Own PID CGWindow lookup, on-screen windows2021/2024 at260,48, 1400x892, scale1, active=false/hidden=false. Actual windows captured and inspected; each owned child stopped with SIGINT and awaited (-2), helper exit0. Native top viewport shows available/ordered lists and raw fields; lower preview needs scrolling, verified in logical UI only.

No native pointer/keyboard/IME/clipboard/picker acceptance. Shared changes expose existing List scroll/reveal behavior without changing painters/chrome; no fresh Office sweep. Original binary/Linux/Windows runtime not accepted.
