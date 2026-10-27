# Capture provenance

- Target: `5bd5104966ae4600d36523df2afc39d2eba874ee`.
- `frozen-prefix-soft.png`: e28 `freeze_uses_selected_column_and_keeps_frozen_cells_and_filters_stationary`; 1000 × 860, four 240 px columns, freeze prefix 2 restored after switching tables. Soft scene render, PPM converted with sips. Test separately asserts scrolling/filter position and pointer hit.
- `frozen-native-metal.png`: own macOS window of the built e28 executable, `NEXT_GUI_GPU_SURFACE=1`, `NEXT_GUI_DB_BROWSER_QA_TAB=frozen`; CGWindow capture at 1400 × 860 content. Static one-column freeze painting only.
- No native pointer/keyboard, IME or original DB4S binary acceptance is implied.

| File | SHA-256 |
|---|---|
| frozen-native-metal.png | `c0ce828da819cd4c41396443f6875cb2c5959b046b1a17ca25632fa96e56f32e` |
| frozen-prefix-soft.png | `611f655403d4f35734ff76f5338e6b323ebae36bf849b60259c9793ebd9ec228` |
