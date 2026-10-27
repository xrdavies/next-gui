# Partial/expression index controls

Code: `68b65c9c64b6a57f289070088e892b629b8845dd`, macOS arm64.

- Ten exact scene pairs: `NEXT_GUI_DB_BROWSER_WIZARD_QA_OUT=/tmp/next-gui-partial-index-wizards cargo test --offline --locked -p e28_db_browser schema_wizard_selectors_keep_qualified_targets_types_and_scroll -- --nocapture`. Both ten-frame contacts inspected; narrow light index and wide dark Metal index inspected full-size. All four Index whole frames max channel delta 1; unchanged Table fixtures max 1 except wide light max 2. Actual readback; PPM converted to PNG with sips. The logical UI test creates/reopens a partial expression index and preserves the WHERE field through failed replacement and successful cross-namespace replacement.
- Native dark/light: fresh e28 binary, `NEXT_GUI_GPU_SURFACE=1`, QA_TAB=index-namespaces, light uses QA_THEME=light. Own PID CGWindow lookup, windows 1953/1954 on-screen at 610,156, logical 700x462, scale2; active=false/hidden=false. Both actual GPU windows captured and inspected full-size. Helper exit 0, each owned child stopped with SIGINT and awaited (-2).

No native pointer/keyboard/IME/clipboard/picker or original-binary acceptance. No shared painting/layout change; Office capture from preceding renderer review remains scoped. Multiline SQL presentation and full structured ordered-key controls remain follow-up work.
