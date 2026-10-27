# Capture provenance

Source: 76baeacfe7e5216056f918991c87857b8fceac7a (committed, clean code tree).

Soft: NEXT_GUI_DB_BROWSER_CALLTIP_QA_OUT=/tmp/next-gui-calltip cargo test --offline --locked -p e28_db_browser sql_calltip_dark_light_and_metal_readback -- --nocapture; PPM converted using sips. Actual Metal readback maximum channel delta 1 in both themes.

Native: NEXT_GUI_GPU_SURFACE=1, e28 NEXT_GUI_DB_BROWSER_QA_TAB=sql-calltip; own-PID CGWindow enumeration, screencapture -l <own window ID>. All 15 final captures succeeded and own test processes were stopped. Office contact sheet uses all fourteen current native screenshots. Static inspection only; actuation display shows wallpaper and activation returned false. No native key/pointer/IME/clipboard claim.

All shell commands set DEVELOPER_DIR=/Library/Developer/CommandLineTools. No Xcode license/security changes or push.

SHA-256:

calltip-dark-soft.png: 6b9a01de1c16d1a714679443d55d170a5ed62d24ee02236f578253f549d0edc5
calltip-light-soft.png: 42f4854915f62773aaff8d5ad9dc1f1f10d4fcd9dfb8087c7907620b4780b883
calltip-native.png: ebd75d9ba354e66adfdffcfa312123dec2a7a9c5f6fe5da7722de1f17bff4e54
office-contact.png: dc78bdb0176c2f13ade4c523b95b61db437183093fed9a1b485a8cab20e651ba
