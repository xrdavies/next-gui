# Capture provenance

- macOS 27.0.1 arm64; tested code `3ab164fed903d2415b6ff9f354a8361f6c0577ff` (final addition is a CR-only limitation comment).
- Dark/light Soft: actual `sql_comment_changes_selected_lines_as_one_undoable_edit` regression after redo, QA_OUT; selected second line of Chinese CRLF SQL is commented, first/third unchanged. Light uses QA_THEME=light.
- Native Metal SQL: owned e28 CGWindow, GPU_SURFACE=1, QA_TAB=sql; normal sample SQL. Painting evidence only, not gesture execution.
- Office contact sheet: all fourteen e10–e15/e20–e27 owned native Metal windows, each captured and skimmed; no catalog-wide input acceptance.
- Original DB4S binary, native input/IME/OS clipboard, CR-only editor support and Windows/Linux runtime not exercised.

| Asset | SHA-256 |
|---|---|
| `sql-comment-dark-soft.png` | `c02ba28baea63d502219eff5fc235b2afe82c89fdb27307fdbdb435401ca6002` |
| `sql-comment-light-soft.png` | `e7ccc8ae58e9e0db0fd8652a3dc17abe0740974b49fb2decd8f7c4f7197659a4` |
| `sql-native-metal.png` | `d28f76d212bc6f58eee7345d1afc7565111397ecb7263d9a36ab664557b5089a` |
| `office-native-contact.png` | `66695fdbf400508285b2007250a96e45300b966c9bae9b454df93b886db8b368` |
