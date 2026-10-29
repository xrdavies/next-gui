# Conditional style checkbox evidence

- Target: `d0db5c12cb29542dbb4f851dd7fde71f2896ebb0`; base `d33e7b4d2a16378eba0ec4248066ee4ffe957c10`.
  macOS 27.0.1 arm64, Rust 1.86.0; owned RAM fixtures only.
- Final offline workspace tests, strict all-target Clippy `-D warnings`, workspace
  build and host platform macos/windows feature checks all exited 0. 822 tests in
  43 groups, including 523 e28 and unchanged 223 shared UI / 15 text; zero failed or
  ignored. Scripts/logs: `/tmp/next-gui-cond-checks-v1-{checks.py,checks.json,*-final.log}`.
- The single new integration test covers both themes at 1400x860/13px and 800x600/18px
  chrome. A real pointer click on another rule's B cell flushes the old expression
  into its original rule; the target lower controls synchronize. Space and arrows
  retain the exact selected column through rebuilds. Reordering retains rule identity;
  Cancel restores every committed rule; Save changes only the intended styles. The
  pending SQLite transaction and five source rows remain untouched. Copied B/I/U cell
  strings now use true/false while the existing house widget paints real checkboxes.
- Twenty inspected whole-window Soft/actual Metal pairs cover initial checks, pointer
  edits, reorder, keyboard edits and saved Browse, both themes/sizes. Maximum channel
  delta 1; none above 2. Both contact sheets and full-size native dark manager/Browse
  were inspected. `/tmp/next-gui-cond-checks-v1-{render-pipeline.py,render.log,convert.py}`.
- Eight owned native GPU workflows, 72 inspected stage frames: before, list, bold,
  italic, underline, other-rule, other-toggle, restored, after. Private-PID keys disable
  all three decorations on the first rule, move to the second rule, enable Bold and
  return to the first selected B cell, then Save/Cancel. All four contact sheets and
  full-size dark cross-row/saved windows were inspected. Owned exits (-2) all awaited.
  Drivers `/tmp/next-gui-cond-checks-v1-{native-keys.py,native-key.swift,metrics.py}`;
  stage frames and records in `...-native-keys/`. Raw flow decoration fields describe
  edited drafts; Cancel retains the original committed state.
- Actual Save/Cancel pixels: in both wide themes the first rule's pale ink decreases
  from 135 to 46 pixels after removing decorations. The second rule's blue background
  decreases from 5172 to 5160 (wide) and 2806 to 2794 (narrow), as the newly bold text
  occupies more pixels. All three background colors remain present. Narrow first-rule
  Right-aligned digits remain outside the partial column, so ink comparison is wide
  only. These relationships are asserted in metrics; no product code changed during QA.
- 162 source hashes and browser SHA256
  `450eba1163946d888554f3f9fca9e06b13725f01956cd8a1879477bfded33665`
  stayed unchanged through final checks, render and native groups; included in metrics.
- Development: a focus test initially counted buttons before the table in the wrong
  direction; corrected value-to-table navigation to three BackTabs. The initial full
  test/build/features passed, while Clippy requested is_none_or instead of a negated
  is_some_and. Applied that equivalent simplification, refroze sources and reran all
  five required checks successfully. Earlier logs are `...-development-*`.
- Shared UI/text code is unchanged in this scope. New Office run not_run (14 fresh
  windows were inspected for the immediately preceding shared font/style change).
  Ordinary frontmost input, native IME/a11y/Print/pickers and Linux/Windows runtime
  remain not_run. Private-PID keys do not establish ordinary desktop acceptance.
- The unsupported B/I/U summary glyph gap recorded in the preceding review is now
  resolved. Canonical conditional-format XML, original modal/Reset/Help, Browse context
  actions, whole-result Browse and full original/native/platform parity remain under
  stable F02. Separate original-app XML fixture investigation is not an accepted
  interoperability check. No push attempted after the earlier approval denial.
