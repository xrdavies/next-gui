# Capture provenance

Implementation `72c67dde0e91e089e5d5f8aeadbbd44e3e2ae092`; macOS27.0.1/arm64, Rust1.86.0, actual Metal.

- Required final50895 exit0:681 tests/43 groups, zero failed/ignored; workspace build, strict all-target Clippy and host macOS+Windows feature checks all passed. Production behavior unchanged after driver start. A final public API doc sentence was added later; fixture SQL text was corrected and separately re-tested/rendered49019 exit0.
- UI48458 exit0: four sequential browser windows (3000-row SQL final rows/full Browse, dark/light), followed by14 rebuilt Office windows. Owned PID/CGWindowID captures, helpers exit0, owned applications deliberately stopped SIGINT(-2) and awaited. No overlapping native drivers.
- Final render49019 exit0: the actual UPDATE RETURNING text is displayed in the fixture. Eight Soft/Metal P6 frames at1400×1000 and800×600, both themes; payload lengths validated after writer exit and converted to PNG. Four SQL table-interior comparisons, max channel delta1 (432112 wide/58000 narrow pixels). Whole-window parity is not claimed.
- All three contacts and full native SQL final rows inspected; Office e21 virtual table also inspected at full size. Programmatic End/Page/Shift/copy/tab/rebuild/Plot selection tests are separate from native OS input acceptance.
- Million-row synthetic data source proves visible-only reads/global navigation and bounded scene nodes; the workspace's existing million-row actual RETURNING/full Plot diagnostic also ran. No UI performance benchmark claim.
- Native OS keys/IME/pickers and original side-by-side/other-platform runtime remain unverified. Primary desktop capture omits windows; no blind input or permission changes.

SHA256:

- `native-contact.png`: `68259f4aa741ce6a0fd375f47ca70c3ca386dff898825f94cf07f6ffc11a818c`
- `render-contact.png`: `02cd76c63eb07426c5a3b12c902cd42d04b3f7535098ccb2a9ca30a4ab4d80db`
- `office-contact.png`: `774c7e9acebfb8509aa1391f54adab406830b6c06229ac8fa64cd171b623c881`
- `native-sql-end.png`: `a0aa82c00c7e41155cc6f07e544bfc691d9f3b7c4172ab29aa81e91ab221bf18`
