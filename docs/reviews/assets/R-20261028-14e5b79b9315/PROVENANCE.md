# Visual provenance

- Code target: `14e5b79b9315422f6191fef01dea14647cde7805` (modal) and `4621b06` (oversized alpha masks).
- Actual macOS27.0.1/arm64, Metal adapter texture limit16384.
- Production browser interaction test prepares a real memory database with a quoted Japanese/Latin table name repeated80 times and a retained row. Captures both themes at1400×1000 and800×600 before/after scrolling. All8 independently compared Soft/Metal pixel pairs have max channel delta1–2.
- `render-contact.png` contains those8 software captures. The production test emits both render paths, does not substitute a reference bitmap, retains the selected Cancel action across rebuilding/resizing, and confirms the table remains after cancellation.
- `native-contact.png`: six actual owned GPU-surface windows, themes dark/light × long-confirm, long-confirm-bottom, pragmas-confirm. Window readiness and CGWindowID capture scoped to each owned PID. Each helper exit0; appSIGINT exit−2; combined driver80178 exit0 awaited.
- `native-light-bottom.png` is the full corresponding native light window.
- `office-contact.png`: rebuilt e10–15/e20–27 (14 windows), native Metal surfaces, each owned process captured thenSIGINT exit−2; driver80178 exit0 awaited. Each window inspected in the contact.
- Eight-pair independent comparison/conversion driver88001 exit0 awaited; focused browser test1459 exit0 awaited; UI/GPU focused driver69593 exit0 awaited.
- Initial oversized alpha test failed with create_texture dimension16387>16384; the first original long browser capture/native attempt also failed. After tiling, both-axis/dirty-patch regression, final browser captures and all native launches pass. These assets are from the corrected run.
- Native QA fixtures prepare modal and scrolling programmatically. This is static rendering plus synthetic interaction evidence, not native OS mouse/key/IME/picker or original-binary acceptance.

## SHA-256

- `native-contact.png`: `9d02240d340b70f566fffde6a7a961d9255a46c48022196d9f46f70d5bc48378`
- `native-light-bottom.png`: `5d719e38c1c073771528697129b9cb96b1d1b8f9dcb7694af5ef8b4425fa7be2`
- `office-contact.png`: `9513fcf30f98117f6e26b7f3d062b8d2954321dc34c0366454f4997c4985e85b`
- `render-contact.png`: `20bb6e0758e9d8e476662bda0a857376b00dc58fc5d13ee728be51dc6c9df857`
