# Capture provenance

Final implementation: `0135ff155bffd44c687e56bba615f69c335ecd69`. Host macOS27.0.1/arm64, Rust1.86.0, actual Metal.

- Required final driver66853 exit0:677 tests/43 groups, zero failed/ignored; workspace build, strict all-target Clippy and host macOS+Windows features all exit0. Production source unchanged after driver start. The external-write regression was extended afterward and separately passed98556; final Clippy compiled that extension.
- Native/render driver24836 exit0. Six sequential owned windows: full Browse, full SQL and actual gated SQLite progress, dark/light. Owned PID/CGWindowID capture, helpers exit0, applications intentionally stopped with SIGINT(-2) and awaited. No simultaneous native drivers or user databases.
- Native full fixtures contain3000 typed rows. The progress fixture gates a real SQLite view outside its500-row preview until cancellation or timeout. The load is still pending in these progress images.
- Widget integration captures loading, loaded and cancelled states at1400×1000 and800×600, both themes:12 Soft plus12 actual Metal P6 frames. Render captures deliberately leave a completed worker unconsumed during the loading-stage capture; they establish modal appearance, not elapsed-time progress. Each P6 payload length checked after writer exit before PNG conversion.
- Eight loaded/cancelled chart-interior comparisons have max channel delta1 (293760 wide/53136 narrow pixels). Loading modals and whole windows were not numerically compared.
- Native, Soft and Metal contacts plus the full native progress window inspected. Native OS keyboard/IME/pickers, original Plot gestures and other-platform runtime remain unverified. Primary Mob screenshot omits windows; no blind OS input or security changes. Shared widgets unchanged, so no new Office catalog run.
- Separate explicit million-row test45301 exit0:1,000,000 executed UPDATE RETURNING rows, full snapshot/cached style/cancel/invalidation workflow in11.39181125s. Debug backend diagnostic, not UI rendering or responsiveness benchmark.

SHA256:

- `native-contact.png`: `bad591205b8564f9945da6ab627627db2f3dc480158108a0c1368541df100c2e`
- `render-contact.png`: `e5ab8bfa0643e987c343fda1282918545aef8f64fbcfd94ea66231a3e6bd85f0`
- `metal-contact.png`: `02a9f0ab76731305b73bdb8ed513a417bac207fc48098aedf26f022c922a8560`
- `native-progress.png`: `c0d98d988d4944f3522ad894706cc17a68d5328258147c2b62b96d68f1b3e96c`
- `native-browse.png`: `ff6064398e37dc4559d460fc4fcda0e1d32c2f8d493b4ae19f871090b2bbb53a`
