# Capture provenance

Implementation `579bf820daab68420a032f1a24708b6e912c4cfc`; macOS27.0.1/arm64, Rust1.86.0, actual Metal.

- Final required83364 exit0:684 tests/43 groups, zero failed/ignored; workspace build, all-target strict Clippy and host macOS+Windows features passed. Production source matches this target.
- Focused tests35703 exit0: three format/PDF/atomicity/database/current-view tests. Earlier fixture compilation/type assumptions were corrected; they are not final acceptance runs.
- UI83979 exit0: four sequential owned browser windows, Browse/SQL full3000 rows, dark/light. Helpers exit0, owned applications stopped SIGINT(-2) and awaited. Static Save control inspected. No actual native file picker input or user files.
- Eight generated Soft/Metal P6 window frames at1400×1000/800×600, dark/light; exact payload lengths checked after writer exit. Four chart-interior comparisons, max channel delta1 (162720 wide/32144 narrow pixels). Whole-window parity not claimed.
- Sixteen actual plot exports across four formats/theme/size combinations. PNG/BMP decode losslessly, JPEG dimensions/colors checked, PDF object offsets/stream lengths/alpha flattening checked. All use the current zoom, selected row, independent Y1/Y2 series, legend and per-series styles. UPDATE RETURNING data was executed once.
- Apple CoreGraphics independently accepted each of four PDF fixtures as one page matching its PNG dimensions. CoreGraphics render versus original PNG has max channel delta0 in all four. Session59924 exit0; verifier log records327×447 and184×121 sizes. Files contact includes the independently rendered PDFs and original PNGs.
- Native/render/files contacts inspected. At narrow widths the existing dock scroll exposes clipped controls, and the small plot remains crowded; full visual refinement is not accepted.
- Shared framework change is an additive read-only plot-size query. The14 Office native windows at base72c67dd remain unchanged by this slice; no new catalog run. Native OS keyboard/IME/picker and original/other-platform runtime acceptance remain unverified.

SHA256:

- `native-contact.png`: `d57ee203af15cc1764590d51e09f49e797dfaaea56b1b54538c96c392d751413`
- `render-contact.png`: `524e6e4453d33ecade0018495ffc8a5b7ea414efd428cbb29dfb016ff0231e7d`
- `files-contact.png`: `7f328870cc25a84774631dd7dd47cc306af825889ec198025e3049eb1ae0956d`
- `plot-dark.png`: `95e29834374ea685a306b8f22f86b30c58042917afdb9aefd1cbbe2b99e84e97`
- `plot-dark.pdf`: `8cd883434cc3ba41b1e91e5c6eca6c222587ca8dc815e395a81f61944dfa0bae`
