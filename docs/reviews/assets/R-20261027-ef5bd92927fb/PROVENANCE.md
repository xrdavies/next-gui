# Capture provenance

- macOS 27.0.1 arm64; code tested at `ef5bd92927fb74d892cd7b265de690ff46677471`.
- Four Soft images: e28 `dock_tab_overflow_preserves_scroll_across_rebuild_and_reveals_selected_panels`, QA_OUT; actual End selection/poll/rebuild, 800×600 and 1400×860, default dark and light themes. No database writes.
- Native Metal: owned e28 CGWindow, GPU_SURFACE=1, QA_TAB=browse, normal sample with Edit Cell dock. All fourteen Office windows captured/skimmed. Captured after UI changes; final addition was DSL binding/test/docs, no painting change.
- Primary display wallpaper only. Native pointer/keyboard/IME/OS clipboard, original DB4S binary and Windows/Linux runtime not exercised. Static captures are not interaction acceptance.

| Asset | SHA-256 |
|---|---|
| `dock-dark-wide.png` | `51ce58b537d4ff76cf307ca59435d831bcc10bbee2fbd623e4f3984327e3ebf4` |
| `dock-dark-narrow.png` | `a208916f6f651dcfa33ff7378c2b7b8c25515fde4b78e359d70dc5227f6bbb6e` |
| `dock-light-wide.png` | `5b7848dda464a63d199261255daeb8e2750bead803a3f114c0e150a55a161f57` |
| `dock-light-narrow.png` | `1afa028718a7f6c50d5ffce78ba05672b7332b969e5ff76089277226f87897d2` |
| `browse-native-metal.png` | `85c64cd01a5e4d1338107d50931cb11cc4efc6eb019b07f1e838fb8da477e48f` |
| `office-native-contact.png` | `35f8f7c189f1fe44ffdf019cd8b8d93da4fe3752cd08f26cb9c32205f6454b70` |
