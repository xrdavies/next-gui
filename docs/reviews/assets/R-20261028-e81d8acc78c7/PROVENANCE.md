# Reference editor acceptance provenance

Implementation: `e81d8acc78c7b42827a95f30b6c8c4a2677785ce`, macOS27.0.1 arm64, Rust1.86.0.

The final in-process test `reference_cells_keep_schema_identity_composites_reset_cancel_and_real_cascades`
ran with `NEXT_GUI_DB_BROWSER_REFERENCE_QA_OUT=/tmp/next-gui-reference-final`.
All six Soft/Metal comparisons passed (dark reference/tables/small:2/2/2;
light:1/1/2 maximum per-channel difference). The process actually exited0 before
PPM conversion with sips. Final contacts and full small-light image were inspected.

Ready-window native helper `/tmp/next-gui-wizard-native-ready-probe.py` ran
`target/debug/e28_db_browser` with `NEXT_GUI_GPU_SURFACE=1`, QA_TAB
`table-foreign-keys-editor`, and QA_THEME dark/light. Both owned layer0 windows
were captured by their own window IDs after the ready log; each owned app received
SIGINT and was awaited (exit−2), each helper exited0. Final native contact inspected.
The14 Office windows (e10–15, e20–27) were run sequentially by the owned-window
helper, captured and inspected as a contact. Every child exit−2 and helper exit0
was awaited. These used the same shared framework implementation; later changes
were isolated e28 test/navigation changes and final required build/feature checks passed.

Desktop screenshot still showed wallpaper and app windows stayed inactive.
These are **static native captures only**; no native keyboard/pointer/IME/Save
acceptance is inferred. Input routing and real SQLite behavior have synthetic tests.
Original binary and Windows/Linux runtime unavailable.

Initial fixture method/type/borrow assumptions, old single-input test expectations,
small-window clipping, implicit-primary-key completion and reset navigation identity
were corrected before final acceptance. Early workspace failures are superseded by
final538 tests/43 groups (e28311/UI162), no failures/ignored, actual exit0.
Strict workspace Clippy/build/macOS/Windows feature checks each exited0.
No dependency, config or platform source change. No publication; approval1960 timed out/denied.

- `soft-metal-contact.png` SHA256 `ed17fa604639e61390388bfa37455f3195c008b97079e7c7ce1daa6679be7c44`
- `small-light.png` SHA256 `3f7dbcb0faf4091c59d25c567d5966e254389e7a5a682719f82fbdb22f1b12ec`
- `native-reference-cells.png` SHA256 `a3c09275ffe6d87ea8dac7b9acf43dfca9636d82d75eee9d1d88c99302a2e8e3`
- `native-light.png` SHA256 `dcd359738f05ef58bc720e905fd06d2b5a4e894a052787ab97fb9a8eed449f06`
- `office-contact.png` SHA256 `fac97eacd0f09055c1515f089fc4ec4cc005d2e93a9bbc3c876470b76d40ff79`
