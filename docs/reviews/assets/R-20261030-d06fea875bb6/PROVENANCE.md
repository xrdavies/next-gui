# Conditional expression evidence

- Target: `d06fea875bb6bf8fc653ebdb2e63c23942bb2366`; base `0b1bd934f77d4f17471f5b2aa33ba5bae9def0f6`.
  macOS27.0.1 arm64, Rust1.86.0; RAM fixtures only.
- Final commands: offline workspace tests, strict all-target Clippy `-D warnings`,
  workspace build and host platform checks with `macos` / `windows` features.
  All five exited0;815 tests/43 groups,520 e28,221 shared UI, zero failed/ignored.
  Logs/scripts: `/tmp/next-gui-cond-expr-v1-{checks.py,checks.json,*-final.log}`.
- Four new matching tests cover typed integers/REAL/TEXT/BLOB/NULL, exact i64 bounds,
  linked SQLite real conversion, ranges/LIKE/current connection pragmas/escape/PCRE,
  empty versus percent-only expressions, quoted values and injection, live worker
  connection ownership, cache revisions/rule order/source changes and close cleanup.
  Content checks include UTF8 BOM/control/noncharacters/invalid bytes and512-byte
  quick-test boundaries. Printable BLOB content can match; the grid still shows a
  BLOB marker. This follows original default-UTF8 classification, not every codec.
- A new UI test covers all13 expression helpers, placeholder ranges and retained
  Undo/Redo in both themes/sizes; invalid-regex Save leaves the draft/rules intact;
  corrected Save changes actual painted matches. Manager helper edits never trigger
  Browse filtering. Existing manager regression tests also pass.
- Twelve whole-window Soft/actual Metal pairs: expression menu, invalid and corrected
  ranges in both themes at1400x860/13px and800x600/18px chrome. Maximum channel delta1,
  none above2. Both contact sheets and full-size dark narrow menu/error were inspected.
  `/tmp/next-gui-cond-expr-v1-{render-pipeline.py,render.log,converted/metrics.json}`.
- Eight owned native GPU workflows: themes x sizes x Save/Cancel, eight stages each
  (64 inspected frames). Private-PID keys open Shift+F10 and the nested helpers,
  insert >=?, replace3, Undo/Redo, then Save/Cancel. Save commits >=3; Cancel retains
  2~4. Actual blue area after Save is >1.5x Cancel; red areas remain within30 percent;
  all three rule colors appear. All children exited-2 and were awaited.
  Scripts `/tmp/next-gui-cond-expr-v1-{native-all.py,native-keys.py,native-key.swift,metrics.py}`;
  raw frames in `...-native-keys/`. All four native contact sheets were inspected.
-159 source hashes and browser SHA256
  `c5f993fc00fbd7c92250d58c11ed8ea9986773fd3422a780932e1fced8ecd8cb`
  frozen before final checks and unchanged through QA; included in metrics.
- Development: an ambiguous empty-array assertion failed to compile; replaced with
  is_empty. A precise Python replacement assertion stopped before writing. All final
  checks passed after correction and source freeze.
- Original v3.13.1 CondFormat.cpp, SqliteTableModel.cpp and Data.cpp were read from
  official GitHub; newly downloaded Data.cpp/.h hashes matched the pinned Git tree.
  Matching uses cached first-match rule indices and bound operands on the live SQLite
  connection. Loaded-page matching remains synchronous (<=500 rows), with an explicit
  shortcut comment. Full codecs, whole-result navigation and adversarial many-rule
  performance remain outside this bounded verification.
- Shared UI unchanged: fresh Office QA not_run. Ordinary frontmost input, native IME,
  a11y/Print/pickers and Linux/Windows runtime remain not_run. Earlier Mob desktop capture
  showed wallpaper only; private-PID keys do not establish ordinary desktop acceptance.
- No push attempted after the earlier Mob approval timeout denial.
