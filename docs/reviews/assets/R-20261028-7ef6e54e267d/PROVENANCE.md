# CSV encoding evidence

Target: `7ef6e54e267d8c64ecb7c1819e7c2e7249991e55`, macOS arm64, Rust1.86.0.

- `NEXT_GUI_DB_BROWSER_CSV_ENCODING_QA_OUT=/tmp/next-gui-csv-encoding-final cargo test -p e28_db_browser csv_encoding --offline`: five tests passed, actual exit0 awaited. Real files cover ten codecs, successful import/rollback, unknown-codec and malformed UTF-8 rejection with prior pending changes intact. UI test covers selection/custom-codec sessions,800↔1400 resize, error correction and clipboard independence in both themes.
- Eight final Soft/Metal pairs at1400×1000/800×600: maximum channel differences1–2. Contact and full small light custom view inspected. PPM writers exited before conversion.
- Four final native GPU captures, both themes with `NEXT_GUI_DB_BROWSER_QA_TAB=csv-encoding` / `csv-encoding-other`. Owned-windowID/ready-marker helper; all four applications exited−2 and helper exits0 awaited. Contact and full custom light inspected. Native fixture displays synthetic `customers.csv` only; actual decoding/import is tested with isolated files. These inactive windows prove static display only, not native input/picker/IME/resize.
- `/tmp/next-gui-csv-encoding-{workspace,clippy,build,macos,windows}-final.log`: all five actual exits0 awaited;572 tests/43 groups, e28339/UI168, zero failures/ignored. No shared UI painting/theme/platform change; Office catalog not repeated.
- `encoding_rs`0.8.35 is pinned for Rust1.86 compatibility. Official crate archive SHA256 matches Cargo.lock; hash-verified create-only cache installation preceded successful offline compilation. Initial0.8.42 MSRV failure/download timeout were corrected and superseded by final checks.
