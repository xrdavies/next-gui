# CSV progress and cancellation evidence

Code `15c13498159d3adb7e2a960355e48aa04745ab9d`, macOS27.0.1 arm64/Rust1.86.0. The actual CSV worker is
held inside a SQLite function using test-owned channels while the UI test thread
rebuilds/resizes, routes Escape, requests cancellation and paints progress.
Dark/light ×1400×1000/800×600 ×progress/canceling gives eight actual
Soft/Metal readback pairs, maximum channel delta1–2. Contact and full small light
progress were inspected. Releasing the worker verifies current-file rollback,
earlier completed files/pending edits, retry without duplicates and final Revert.
The tests explicitly poll asynchronous completion; production never blocks on a
join until the worker reports finished.

Six real native GPU windows use test-owned CSV paths and memory databases:
dark/light progress, canceling and existing-table confirmation. A QA-only SQLite
function pauses the first inserted row for30 seconds; native QA automatically
accepts the existing table and requests Cancel in the canceling fixture. Ready
marker plus an onscreen layer0 window >=1400×860 required before window-ID capture.
All six owned apps exited−2 (SIGINT awaited), helpers and driver0. Contact and full
light canceling were inspected. These are static prepared fixtures, not OS input,
native picker or original binary acceptance.

Final594 workspace tests/43 groups (e28361/UI168), strict Clippy, workspace build
and macOS/Windows host feature checks exited0 and were awaited. Windows/Linux
runtime and Windows-target compilation were unavailable. No shared widget/paint
change; Office catalog was not rerun. Existing streaming memory/temp disk limits
remain. Cooperative cancellation waits for an in-flight read or SQLite record;
SQLite interruption is deliberately avoided because it can roll back outer edits.

Logs: `/tmp/next-gui-csv-progress-{workspace,clippy,build,macos,windows}-final.log`,
`/tmp/next-gui-csv-progress-visual.log`, `/tmp/next-gui-csv-progress-errors.log`,
`/tmp/next-gui-csv-progress-native.log`.

SHA-256:

- `soft-metal-contact.png`: `4b9568f3721df718858df592568377d62bd84957323d30ad255448f7876ae622`
- `light-small-progress.png`: `1d224b28d52f47849186cbd68a9151f7d53b7f304ad316c3ae81781bf42b959e`
- `native-contact.png`: `5faa3266d4329743f9a747b1a82bb8a4691f6ccc7827e509e1ee34b3a6da2ad9`
- `native-light-canceling.png`: `5f0f159c490fd5f2e2ea94338428a766ac86ecdd9bf40f1f29f60bc8c88b587a`
