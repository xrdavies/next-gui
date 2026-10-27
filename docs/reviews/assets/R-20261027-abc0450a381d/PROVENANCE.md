# Multiline index editing

Code: `abc0450a381dad07e75f52539352f1966d31a699`, macOS arm64.

- Exact scene pairs: `NEXT_GUI_DB_BROWSER_WIZARD_QA_OUT=/tmp/next-gui-index-multiline-wizards cargo test --offline --locked -p e28_db_browser schema_wizard_selectors_keep_qualified_targets_types_and_scroll -- --nocapture`. Twelve Soft/Metal pairs; both themes, wide and narrow top/bottom, same scenes and actual GPU readback. PPM converted with sips; contacts and wide dark/narrow light bottom frames inspected. Full viewport primitive-inclusive delta <=2 (Index 1). The test checks fixed Save, fully visible bottom WHERE, retained scroll offsets and both editor sessions.
- Native fresh dark/light GPU: QA_TAB=index-namespaces, QA_THEME=light for the light capture. Own PID CGWindow lookup finds on-screen windows1960/1963, 700x462 at610,156, scale2. Active=false/hidden=false. Both actual windows inspected, children stopped with SIGINT and awaited (-2), helpers exit0.

No native input/IME/clipboard/picker or original-binary acceptance. No shared painter/layout change; no fresh Office run required for this example-only slice. Full structured index editor remains follow-up work.
