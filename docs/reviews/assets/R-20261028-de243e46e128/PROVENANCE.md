# Original CSV record behavior evidence

Code `de243e46e12806ec8c7235c34a1e9dfe3769ef21`, macOS27.0.1 arm64/Rust1.86.0. Four dark/light ×
1400×1000/800×600 variable-record preview frames use the real file parser and
worker import. Actual Soft/Metal readback maximum channel delta1; contact and
full small light inspected. Missing fields display blank, wider sampled records
add field3, and middle/closing quoted segments have original-compatible text.
The test then imports and checks actual SQLite NULLs, values and column identity.
A separate run compared24 existing batch/progress frames (delta1–2); these were
automated rendering regressions, not additional new-data screenshot acceptance.

Two real native GPU windows in both themes show a four-column preview with a
shorter row and Japanese quoted segments. Owned CSV paths and memory DB only;
ready marker plus onscreen layer0 >=1400×860 required before window-ID capture.
Both apps exited−2 (SIGINT awaited), helpers/driver0. Contact/full light inspected.
Prepared static fixtures do not establish native picker/OS input/original binary
or Windows/Linux acceptance.

Two new source-compatibility regressions first failed at the old closing-quote
and fixed-width guards (test process exit101). Both pass after the fix. Final597
workspace tests/43 groups (e28364/UI168), strict Clippy, workspace build and host
macOS/Windows feature checks all exited0 and were awaited. No shared UI/paint API
change; Office catalog not rerun. Current source wider than its first20-record
schema is rejected before mutation, preventing silent loss of extra fields.

Logs: `/tmp/next-gui-csv-parser-before.log`,
`/tmp/next-gui-csv-parser-{workspace,clippy,build,macos,windows}-final.log`,
`/tmp/next-gui-csv-parser-focused.log`, `/tmp/next-gui-csv-parser-ragged.log`,
`/tmp/next-gui-csv-parser-native.log`.

SHA-256:

- `variable-records-contact.png`: `b77c98c93bbbbd293eaf472d3b7e8c5d366a13a34d7e08ce4477ac7462bdb489`
- `light-small-records.png`: `1aa66510cb34d7e05f66df3ba8140e236ad56398a619fd3f79175292b175d5b5`
- `native-contact.png`: `7357a4edab0fcaa89353d734fe37344c40c7f8494862be8eee58e7f668e9ab6e`
- `native-light-records.png`: `aa0afe9b486438131d8e5312bd30d529a22c4b1118e92c2b91737996414182ed`
