# Capture provenance

Implementation `bafb781fb129d62e47c6a8ceec5489e5a6a12c25`, macOS27.0.1 arm64, 2026-10-28.

- `splitter-contact.png` / `small-light.png`: final `NEXT_GUI_DB_BROWSER_TABLE_SPLITTER_QA_OUT=/tmp/next-gui-table-splitter cargo test --offline --locked -p e28_db_browser table_preview_splitter -- --nocapture`. Writer21568 exit0 awaited before PNG conversion/contact4164 exit0. Four Soft/Metal pairs, both themes1400×1000/800×600 after live drags with Advanced shown; max channel delta2 each. Contact and full small light inspected. The small capture deliberately gives more space to SQL; the definition page scrolls independently.
- `native-pages.png`: owned GPU in-memory fixtures `table-field-editor`, `table-keys-grid`, `table-foreign-keys-grid`, `table-checks-grid` in both themes. Every full page/contact inspected. Helper49119 exit0; all owned SIGINT-2 exits awaited. Captured before the final constrained-height calculation for small windows; these1400×892 pages do not hit that constraint.
- `native-advanced-light.png`: final build Advanced page (dark counterpart also inspected); helper8294 exit0, both owned SIGINT-2 exits awaited. Ten distinct native configurations in total. Window metadata/readiness logs under `/tmp/next-gui-table-splitter-native-*`. Targets inactive; no native input acceptance on locked desktop.
- `office-contact.png`:14 owned native windows e10–15/e20–27, contact inspected. Helper49119 exit0 and every owned SIGINT-2 exit awaited. Captures precede a custom-minimum drag-bound correction; these Office splitters retain zero minima and unchanged behavior.

Final logs: `/tmp/next-gui-table-splitter-{workspace,clippy,build,features}-complete.log`; `/tmp/next-gui-table-splitter-visual-current.log`; `/tmp/next-gui-table-splitter-{minima-complete,scroll-shared-complete,preview-regression,app-complete,native}.log`. Final518 tests/43 groups (e28299/UI154), zero failures/ignored; every final exit0 awaited.
