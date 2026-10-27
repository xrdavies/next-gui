# Capture provenance

All captures were generated from code target `a99567b81242db84f07a739e4854e75886438adf` on macOS.

- `disabled-form-soft.png`: exact `interaction_tests::browse_buttons_follow_page_record_and_write_boundaries` run with `NEXT_GUI_DB_BROWSER_QA_OUT=/tmp/next-gui-disabled-buttons-a99567b.ppm`; converted using sips. A 501-record table, first page/record, clean transaction, Form View.
- `browse-native-metal.png`: freshly built `target/debug/e28_db_browser`, `NEXT_GUI_GPU_SURFACE=1 NEXT_GUI_DB_BROWSER_QA_TAB=browse`, captured with window-only screencapture using that process's CGWindow ID.
- `office-native-contact.png`: native Metal window captures from e10–e15 and e20–e25, assembled with AppKit. Each process was independently probed by PID and stopped after its capture. Default buttons remain enabled. The sheet was visually inspected; this is render evidence, not input acceptance.

Initial probes after two seconds returned no windows. Waiting for window creation produced valid captures; no desktop permissions or settings were changed. Full-display capture still showed wallpaper. Native keyboard, pointer, IME and clipboard tests were not performed.

SHA-256:

- `082c098a452377e93faadbf786182e45854d36aea07e952a46531f0f4fbe2bb2` `disabled-form-soft.png`
- `ac6e1d1d635ddda63487cc8f2de204b813a3557c396b7c2ad0019e958c8462c5` `browse-native-metal.png`
- `54fcd60f1f5ff78f0b44849166f05bfa38a8a688d8e5b031076ad5316b4dda1b` `office-native-contact.png`
