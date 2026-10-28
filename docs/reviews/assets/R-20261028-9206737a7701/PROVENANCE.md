# Visual provenance

- Code target: `9206737a77010372c3a79bc0719ba4ef34968e08`; actual macOS27.0.1/arm64 Metal readback/native surfaces; no reference bitmap substitution.
- `render-contact.png`: dark/light ×1400×1000/800×600 ×virtual chooser/contentless data-only error. Focused production-path92355 exit0 awaited. Eight independently compared pairs have matching P6 dimensions/exact RGB lengths and max channel delta1–2. Conversion driver14887 exit0 awaited. All eight inspected.
- `native-contact.png`: dark/light ×ordinary/virtual chooser, four owned CGWindowID/PID GPU windows. Driver61611 exit0 awaited; each helper0/appSIGINT−2. All inspected. The fixtures own in-memory databases, never user files.
- Core data-only tests18256 exit0 awaited: twenty quoted-module modes (FTS3/4/5, RTree/RTree_i32 ×full/rows ×named/positional INSERT), pending source rollback, target append, exact large rowids when requested. Data-only targets additionally enable SQLite defensive mode in their isolated in-memory fixtures; no shadow writes are needed. Positional FTS rows intentionally acquire target-generated rowids, matching the existing documented INSERT option.
- Read-only fts5vocab values export into an ordinary target; contentless/external FTS guard failures retain destination and source pending rows. Full production dialog test verifies the guard/error/repair/full-export worker paths, not a native OS data-only gesture.
- Final required driver38328 exit0 awaited:650 tests/43 groups, strict Clippy, workspace build and host macOS/Windows feature checks. Zero failed/ignored. Original new regression failed101 because all virtual data-only modes were blocked.
- Shared framework and14 Office examples are unchanged; no new Office run claimed. Native OS input/IME/pickers/original-binary and actual Windows/Linux runtime acceptance remain unavailable/unrun.

## SHA-256

- `native-contact.png`: `8a3cdb7d7bf90e5b3459013e44a226640502ce83942b72992f097ee6ae3c990d`
- `render-contact.png`: `c4c19f6a3c813d153c2e4a4e9ecf6bc0955372171b6d9608b7e6e03adb48e21d`
