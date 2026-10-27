# Capture provenance

- Code target: `a37bee216b33c5368111824be25b6ea4fef5de2c`.
- `multi-sort-soft.png`: exact-target test `browse_sort_tests::multi_sort_keeps_priority_toggles_columns_and_clears_explicitly`, generated with `NEXT_GUI_DB_BROWSER_QA_OUT=/tmp/next-gui-multisort-a37bee2.ppm`, converted using sips. Shows three priorities, whitespace/arrow identifiers and first sorted cell/editor consistency.
- `browse-native-metal.png`: final implementation build (byte-identical source to target), own process with `NEXT_GUI_GPU_SURFACE=1 NEXT_GUI_DB_BROWSER_QA_TAB=browse`, window-only screencapture, ID 1361; process stopped afterward. Captures Clear Sort disabled on an unsorted table. This is render evidence, not native modifier/input acceptance.

SHA-256:

- `d5be43c085f6be0118d5e81bf008347d1ed211e90a95684a564bbb3a1a0fb1cc` `multi-sort-soft.png`
- `05146406ef3279eefef0062746260c3f21a84bc77f75151accab45c4ac8143db` `browse-native-metal.png`
