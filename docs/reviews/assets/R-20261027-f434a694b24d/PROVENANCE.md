# Capture provenance

- Code target: `f434a694b24d101c21005795a49546d3e0056794`.
- `multi-sort-soft.png`: exact-target `browse_sort_tests::multi_sort_keeps_priority_toggles_columns_and_clears_explicitly` run with `NEXT_GUI_DB_BROWSER_QA_OUT=/tmp/next-gui-corners-f434a69.ppm`; converted with sips.
- Native images: fresh workspace build with source identical to target, `NEXT_GUI_GPU_SURFACE=1`; e28 additionally used `NEXT_GUI_DB_BROWSER_QA_TAB=browse`. Each process was probed by PID until its window had nonzero bounds, captured using window-only screencapture and stopped afterward.
- `office-native-contact.png` contains e10–e15/e20–e25; `office-native-extra.png` contains e26/e27. AppKit assembled the images. Every frame in both sheets and the full e28 capture was reviewed.
- e28 table bottom corners no longer expose black backgrounds; baseline native frame is in R-20261027-a37bee216b33 and baseline Soft frame in the same report. Captures establish rendering, not OS input/IME acceptance.

SHA-256:

- `7789479323b5c97feb8150970c715c30a4f9334c3f8f313bd855721ad1076c7c` `multi-sort-soft.png`
- `4ab7a56bd751e815ec99f94bd862469979370f71df4cb0dc9c0ae2a7ee694d14` `browse-native-metal.png`
- `e16464adbbdb68e5e7ade41cdd7627e35d38d1a79e989146d35be145ec16ba76` `office-native-contact.png`
- `091e71f50045a27a85511882307caaf1d431e4ccb813b2b50e847ef4e19c2ce8` `office-native-extra.png`
