# Capture provenance

- macOS 27.0.1 arm64; code `b8afc40b7b7820012e88855c4ab3f95c6f09b2f0`.
- Soft: actual numbered SQL editor regression, 80 lines, searched line 40; nearby function/parameter/BLOB/comment fixtures demonstrate distinct colors. Main/Find Undo and light-theme session recoloring asserted. QA_THEME=light selects light mode.
- Colored selection/search GPU readback actually ran, maximum channel delta 2 against Soft.
- Current native capture attempt failed for initial Office windows; a separate owned e28 Metal retry also failed: screencapture exit 1, "could not create image from window". e28 startup logged 1400×860 backend GPU, CGWindow existed at layer 0/on-screen. mob screenshot reported "screen unavailable: the office computer's display is asleep". A bounded caffeinate user-activity signal did not restore capture. No account/security setting was changed, and only owned processes were stopped.
- No new native/Office image is included. Prior line-number screenshots are historical baseline only. Native input/IME/clipboard, original DB4S binary and Windows/Linux runtime acceptance remain untested.

| Asset | SHA-256 |
|---|---|
| `sql-dark-soft.png` | `d7a4e76e099d1040d07319c19c3967a050887f1981a1b398981268d30747f357` |
| `sql-light-soft.png` | `6b44e67c8a9a632eebb1aa3ca389f1227051f3b754cb9c5284d5bf37c7373944` |
