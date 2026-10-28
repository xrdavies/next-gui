# Responsive Plot evidence

macOS 27.0.1 arm64, Rust 1.86.0, default13px browser font.

Native contact: dark/light wide1400×860 fixtures and actual800×600 startup fixtures. Captures use observed owned CGWindowIDs; all app exits -2 and driver exits were awaited. The narrow alias only controls the QA fixture and initial WindowAttrs; ordinary launches retain1400×860. No actual OS drag resize is claimed.

AX attempts: the first title guard failed because the AX title was the binary name. A second attempt targeted the sole AX window of the observed owned process; setting AXSize returned a non-success result. Both helpers stopped and owned apps were shut down. No account or security settings changed. Wide captures precede these failed attempts.

Render contact: eight P6 frames (Soft/Metal, dark/light,1400×1000 and800×600) validated after completed writes and converted with sips. Four chart-interior comparisons max channel delta1. The before/after800px pair uses the same export fixture, source viewport, focused Copy control, legend and series styles. Earlier before frame is from4398409; after is3242c29. Labels/style/action rows now fit and focusing Copy no longer scrolls the whole dock horizontally.

Final driver66602 exit0: build,687 tests /43 groups with zero failures/ignored, strict all-targets Clippy, macOS+Windows feature checks. Prior driver49019 also passed before the narrow startup fixture was added; final results come from the second full run. Render51664 and native90156 exit0 awaited. Native drag, large-font layout and other-host runtime acceptance remain unverified.
