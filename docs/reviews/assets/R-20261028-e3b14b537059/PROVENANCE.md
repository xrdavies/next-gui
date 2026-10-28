# Multi-table export evidence

Code `e3b14b537059340cc0e09955ec40861284c8ce33`, macOS27.0.1 arm64/Rust1.86.0. Four focused tests include
12 dark/light ×1400×1000/800×600 ×CSV/overwrite/JSON Soft/Metal frame pairs,
maximum channel delta2. All final frames in the contact sheet and full small
light CSV inspected. Checkbox Space/SelectAll, custom separator, focus/resize,
default-Cancel/Escape, replacement and settings roundtrip exercised synthetically.

Four actual native Retina GPU windows (CSV/JSON ×both themes) captured by owned
window ID. Current window was630×417 points/1260×834 physical; minimum capture
size corrected from1400×860 to1200×760. Initial four attempts missed windows
and are not acceptance; helper now fails missing captures. Final4 apps−2,
helpers0/driver0 awaited. Both contact and full light CSV inspected. A separate
input attempt launched owned memory DB but Mob screenshot returned a black
screen; no unknown controls were actuated. App−2/driver0 awaited. Thus native
mouse/key/picker, IME and full original binary comparison remain not_run.

609 tests/43groups/e28374/UI168, strict Clippy, workspace build and host macOS/
Windows feature checks0 awaited. Final whole-suite run preceded only the target-
cfg Windows CRLF default and rustfmt; all4 focused tests were repeated afterward
(exit0 awaited; Mac default unchanged). Actual Windows/Linux targets/runtime
unavailable. Logs `/tmp/next-gui-data-export-{workspace,clippy,build,macos,windows}-final.log`,
`/tmp/next-gui-data-export-checks.log`, `...-focused-cfg-final.log`,
`...-native-final.log`. CSV QA helper reused for render capture only.

SHA-256:

- `render-contact.png`: `44c49944f7f37358a017f1eccae73e14a16d548e4a9e75a617643f30df6ed8c2`
- `light-small-csv.png`: `308ce4d60c5674114f020a26191888a5b3fcffe9896895704146920e10822aac`
- `native-contact.png`: `185ed569249cb493c7e61e80c944d650de9a5c1d346b9104e28e6423018b8d99`
- `native-light-csv.png`: `9269c81a1bb51e46c1c4b58264ff1e9018c715b1d310b1a8d64776a3755a23b9`
