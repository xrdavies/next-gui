# Visual and experiment provenance

- Code target: `20580cba51bcdd0153990be742166c2adc58fd61`; macOS27.0.1/arm64, actual Metal surfaces/readback, no reference bitmap substitution.
- `render-contact.png`: two themes ×1400×1000/800×600 ×virtual chooser/data-only error. Final production-path focused test10604 exit0 awaited. Eight Soft/Metal pairs independently checked: P6 dimensions and exact RGB payload lengths, maximum channel delta1–2. Conversion/contact driver71911 exit0 awaited; all eight frames inspected.
- `native-contact.png`: dark/light ×ordinary export chooser/virtual export chooser. Four owned CGWindowID/PID GPU windows; final driver20750 exit0 awaited; each helper0/appSIGINT−2. All inspected. `native-light-chooser.png` retains the full light virtual frame. The fixture owns an in-memory database and pending FTS/RTree rows; no user database is opened.
- Existing14 Office native windows are unchanged by this example-only slice; previous report R-20261028-a3dc6cdd4f0c retains that evidence. No new Office run claimed.
- Standalone bundled-rusqlite probes demonstrate why an exporter savepoint is required: raw physical copying matched1 committed row but0 pending rows, then assertion failed101. Adding SAVEPOINT before reading matched1 in both modes and exited0. Source files retained; compile with the matching workspace rusqlite rlib and dependency directory. These are design probes, not a claim that the formerly guarded product emitted a corrupt dump.
- Original regression failed101 with `SQL dump of virtual tables is unsupported; use Save Database As`. Final three database regressions cover actual FTS4/5 MATCH and FTS3 catalog/selection isolation, contentless/external indexes, RTree, pending transaction/rollback, integrity checks, selected prefix isolation, exact large rowids, INSERT/CREATE variants, existing target rejection/explicit replacement and destination-preserving errors/cancellation.
- Final required driver3262 exit0 awaited:648 tests/43 groups, strict Clippy, workspace build, host macOS and Windows feature checks. Zero failed/ignored. Initial GUI fixtures incorrectly began a second transaction/changed state behind retained selectors; corrected to use the existing session and selector input. A transient snippet-formatting compile error was corrected before the final run.
- No native OS gesture, original-binary, picker/IME or actual Windows/Linux runtime acceptance is claimed. Synthetic input plus native static capture establish the stated narrower checks.

## SHA-256

- `native-contact.png`: `f2dbda64e47f1ccbee6327e08279e808f20c91ad198175b6fe129586b65b4e75`
- `native-light-chooser.png`: `b591b1da14d54e0b3ddd5c1ebda112971f4e87f26541a872d3038ccdf135bd9e`
- `pending-savepoint-probe.rs`: `1fa6ba6a68906a8a95abd7d5b05d8f72fc10a5b20b49157cc55e29e544198109`
- `pending-shadow-probe.rs`: `4c563ea69f88bd26658efa2270e4c1dbed1a3877382aa6ec0b1db708f3862845`
- `render-contact.png`: `abeb2315374d80538f8263d01bb93727c620d3d8db34cddd9fe14d7afbedd003`
