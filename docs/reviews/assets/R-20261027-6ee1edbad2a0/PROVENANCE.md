# Capture provenance

Code: 6ee1edbad2a0fc408d210066f8e4f80c734aef1c. Captures immediately before commit match the committed code files.

Soft: NEXT_GUI_DB_BROWSER_CALLTIP_QA_OUT=/tmp/next-gui-hint-pages cargo test --offline --locked -p e28_db_browser calltip -- --nocapture. PPM converted via sips. Actual Metal max channel delta: dark 1, light 2. Long signature uses a registered 127-argument function at 28px preference in 800x600; top/end screenshots are a clipping/scroll stress case, not full large-font UI acceptance.

Native: NEXT_GUI_GPU_SURFACE=1 and e28 NEXT_GUI_DB_BROWSER_QA_TAB=sql-calltip. Own-PID CGWindow enumeration and screencapture of each own window; all fifteen captures succeeded, children stopped with SIGINT and awaited. All fourteen Office windows are in the contact sheet. Static painting only. Fresh mob display screenshot still shows wallpaper without an app; no blind input, no native key/pointer/IME/clipboard acceptance.

All shell commands set DEVELOPER_DIR=/Library/Developer/CommandLineTools. No security/license changes or push.

SHA-256:

calltip-dark-soft.png: ba8d38b9eb36d064efb2d5bbb925097d8a8ee7933480bedcb40d956b4d64e063
calltip-light-soft.png: 467e65e255b906f7b3b740d65fe61aeadaad33ba09f47a9e63e14bcef91e7609
calltip-native.png: fc6c2be6ce9c5749b6d1f33798cf7b17b9d9fde4b0516ef14079e7db833c9ea7
long-end-soft.png: c422b9f7a2c1cc31e148271082100b7b70233455a86d1135a14971f59f9c963d
long-top-soft.png: cc1ffa4f9bdcf68efdefafc6e97a0e3e3e70e4fce636fa8b3576c46feb71ac10
office-contact.png: a25daeebfef377e426e3d5f4ad46ef94b9d97b173603284472b4df5f0f4edf87
