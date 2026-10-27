# Capture provenance

- macOS 27.0.1 arm64; final working tree committed as `3855f40fe8ba603782772ed0176bbdea298090b9`.
- Ran the actual `sql_completion_popup_dark_light_and_metal_readback` and `preferences_cancel_keeps` regressions with `NEXT_GUI_DB_BROWSER_COMPLETION_QA_OUT=/tmp/next-gui-keyword-parity`; converted their Soft PPM frames with `sips -s format png`.
- SQL fixture has 24 comments, a CREATE TABLE line using NUMERIC/STRICT, and a line-26 caret with a live completion menu. Both theme Metal panel comparisons actually ran with maximum channel delta 1.
- Preference switch is the draft UI; Cancel tests both completion flags without saving user preferences. Keyword case behavior is checked separately through candidate filtering and real editor range acceptance.
- Fresh mob screenshot still reported the office display asleep. No native e28 or original-binary/input/IME/clipboard runtime acceptance is implied.

| Asset | SHA-256 |
|---|---|
| `keywords-dark-soft.png` | `aab71b476a181a5fb9912645bcb29521aeb3dce7c85c790d39c94dbf606665c1` |
| `keywords-light-soft.png` | `9b44c127cbf61dda647960a842e8ab311664b8eaec923f062f5485f39f9d2a5a` |
| `preferences-soft.png` | `6655992097ccb2d84935033c66631d9fa8554a52875b7c2747dece5ce319b95b` |
