# Capture provenance

- macOS 27.0.1 arm64; code `4c3ca63f83aca9014f4f972eec1216cda66d6235`.
- Soft: actual `numbered_sql_editor_retains_search_selection_and_undo_through_find_overlay`, QA_OUT, 80 Chinese SQL lines, searched line 40, snapshot and rebuild; light uses QA_THEME=light. Search/copy excludes numbers; overlay undo asserted after capture.
- Native Metal: owned e28 CGWindow, GPU_SURFACE=1, QA_TAB=sql, ordinary sample showing two physical lines. All fourteen Office windows captured and skimmed. No long/native interaction claim.
- Separate numbered selection/search GPU readback test ran, max channel delta 2 against Soft.
- Primary display screenshot was black this check. Native input/IME/OS clipboard, original DB4S binary and Windows/Linux runtime acceptance were not exercised; own-window static captures remain available.

| Asset | SHA-256 |
|---|---|
| `sql-dark-soft.png` | `77a0c7037ff7b9a4156084b180797a8d9e383112cd6f1b1834f86750d1386329` |
| `sql-light-soft.png` | `6dea4e3c2ce1c9ea39021439d95a527341e4993c47b98bb4919669fd250c3fa3` |
| `sql-native-metal.png` | `188e09bd584a6bd1f63f428726c9fe0c97dddd9780e4ce90960ecbd990789f82` |
| `office-native-contact.png` | `626eb94993cecf921fdea55503de8a250ff5e506d4c27b85b1cd976267931cbd` |
