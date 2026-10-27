# Capture provenance

Code: f6bf184f24b31d46beb0fd2b76985fd56590b026; captures made immediately before the code commit, with exactly the committed code files.

Soft: NEXT_GUI_DB_BROWSER_CALLTIP_QA_OUT=/tmp/next-gui-function-help cargo test --offline --locked -p e28_db_browser sql_calltip_dark_light_and_metal_readback -- --nocapture. PPM converted via sips; actual Metal max channel delta 1 in both themes.

Native: current built e28, NEXT_GUI_GPU_SURFACE=1 and NEXT_GUI_DB_BROWSER_QA_TAB=sql-calltip. Own-PID CGWindow enumeration, screencapture of that window, then SIGINT and awaited own child exit. Seeded editor text was not executed. App-only help changed; shared Office UI was not recaptured. Static painting only; no native input acceptance.

All shell commands set DEVELOPER_DIR=/Library/Developer/CommandLineTools. No security/license changes or publication.

SHA-256:

calltip-dark-soft.png: b80ba3f823c497eb808da9a4114611a66bb5732627a5d633fe18423ba5310aa7
calltip-light-soft.png: ee5704dda1d28419ace8f47a3aab35da4411dc91c6b8e6e3faef9b7cf08b9f70
calltip-native.png: 2ab03d9bef3a26e0396fa2735ab720642405642c79cbb31ca58b96fa54213e58
