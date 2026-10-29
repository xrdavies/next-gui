# Conditional text style evidence

- Target: `5cf0d804d6cc4b5c994fd44e92f0079b26767504`; base `57f7162231469224dc1d4f25200a5e3cd6c72bf2`.
  macOS 27.0.1 arm64, Rust 1.86.0; owned RAM fixtures only.
- Final offline workspace tests, all-target Clippy `-D warnings`, workspace build,
  host platform `macos` and `windows` feature checks all exited 0. 821 tests across
  43 groups, including 522 e28, 223 shared UI and 15 text tests; zero failed/ignored.
  Scripts/logs: `/tmp/next-gui-font-styles-v1-{checks.py,checks.json,*-final.log}`.
- Text tests cover installed family resolution, aliases/case, real bold/italic faces,
  cache identity, styled fallback glyphs, synthetic style advances and clipped masks.
  Shared UI tests cover aligned/underlined bounded masks, color/selection precedence,
  frozen/virtual table rendering, unchanged copied text and atomic invalid metadata.
  App tests cover legacy defaults, invalid style drafts, JSON/XML extension round trips,
  missing-family fallback, all controls, retained history/selection and Save/Cancel.
- CoreText discovers activated font URLs and available family names. On this host,
  244 of 245 available families were indexed from 644 URLs (152 outside ordinary font
  directories). `GB18030 Bitmap` is the missing family: its NISC18030.ttf has no head
  table; both ttf-parser (`NoHeadTable`) and fontdue reject it. The direct probe is
  `/tmp/next-gui-font-styles-v1-nisc.{rs,log}`. Color/bitmap fonts and every installed
  face are not claimed. Font-file reads are bounded at 256 MiB with a shortcut note;
  the face cache retains at most 16 entries. The existing ttf-parser 0.20 dependency
  was promoted to a direct dependency; no newly resolved package was added.
- Sixteen inspected whole-window Soft/actual Metal pairs cover manager, installed
  font popup, lower controls and saved Browse in both themes, at 1400x860/13px and
  800x600/18px chrome. Maximum channel delta 2, none above 2. Contact sheets and
  full-size dark narrow manager/saved views were inspected. Scripts/logs:
  `/tmp/next-gui-font-styles-v1-{render-pipeline.py,render.log,convert.py}`.
- Eight owned native GPU flows (themes x sizes x Save/Cancel), 80 stage frames:
  before, foreground, Undo, Redo, font popup, family, size, decorations, alignment,
  after. Private-PID keys change foreground to #F8D56B, Menlo to Marker Felt, size
  to 20, disable B/I/U, and change Right to Center. Save commits; Cancel retains
  original styles. All four contact sheets plus full-size dark font/saved frames
  were inspected. All owned child exits (-2) were awaited. Drivers:
  `/tmp/next-gui-font-styles-v1-{native-keys.py,native-key.swift,metrics.py}`;
  original stage frames and exit records in `...-native-keys/`.
- Pixel assertions require edited foreground after every Save and none after Cancel,
  with all three background colors present. A first metrics assertion incorrectly
  required original foreground ink in the narrow Cancel view: the original Right
  alignment places those digits outside the partially visible column. Corrected
  that QA assumption to require original ink in wide Cancel only. Product code and
  frozen binary did not change during QA.
- Fourteen fresh Office examples were rebuilt, launched, captured and inspected in
  the Office contact sheet; all owned exits (-2) awaited. Sources and browser SHA
  were checked after rendering, native keyboard and Office groups.
- 162 source hashes and browser SHA256
  `1c967bc6aef8df139fcd14ee19eeceaf9d47f4bb47480471ae941373eaba4cf5`
  are included in metrics and remained unchanged through final checks/QA.
- Development corrections: typed empty alpha-mask storage; correct AppState fixture
  and input selection API; adjusted keyboard Save tab count after seven controls;
  Macintosh ASCII family names increased indexed families from 179 to 244. An initial
  strict Clippy run rejected a complex cache tuple; a named cache entry fixed it,
  followed by source refreeze and all five final checks passing.
- Original v3.13.1 conditional-format manager, table model and MainWindow sources,
  and Qt 5.15 QFont/CoreText sources were consulted. Application point conversion
  follows Qt's 72 dpi macOS / 96 dpi base. Custom DPI/high-DPI parity is not accepted.
- Known remaining UI gap: B/I/U summary glyphs in the manager are not supported by
  the current default font and render as small boxes. Actual lower checkbox controls
  work; replace summaries with real table checkboxes next. Canonical DB4S condition
  XML, per-column modal layout/Reset/Help, Browse format context actions, whole-result
  Browse and full original/native/platform parity remain under stable F02.
- Ordinary frontmost input, native IME/a11y/Print/pickers, Linux/Windows runtime and
  original-application project exchange remain not_run. Private-PID keys are not
  ordinary desktop acceptance. No push attempted after the earlier approval denial.
