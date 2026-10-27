# Capture provenance

Code: 901390905c2515f91dce0f4a7135ec85044f927e. Soft Browse: NEXT_GUI_DB_BROWSER_NAMESPACE_QA_OUT=/tmp/next-gui-browse-namespaces cargo test --offline --locked -p e28_db_browser browse_namespace_selector -- --nocapture. Actual interaction fixture uses in-memory main/temp/analytics same-name tables; dark/light 1400x1000 and 800x600 inspected. The test later verifies row deletion addresses analytics only. sips PNG conversion.

Native: built e28, NEXT_GUI_GPU_SURFACE=1 NEXT_GUI_DB_BROWSER_QA_TAB=browse-namespaces; own window 1886 captured using own PID/window lookup and screencapture -l. CGWindowIsOnscreen=true; NSApp active=false, logical 700x462, scale 2. Child stopped with SIGINT and actual exit observed. Native static paint only; no OS keyboard/pointer/IME/clipboard acceptance. No blind events.

Format overlay: built e28 with NEXT_GUI_DB_BROWSER_QA=1 NEXT_GUI_DB_BROWSER_QA_TAB=cond, default dark / NEXT_GUI_DB_BROWSER_QA_THEME=light; actual Soft frame 1400x860. Main sample fixture read without edits. Explicit database label and table-scoped rule count inspected.

No shared painter/chrome/platform change; Office examples built but not recaptured for this slice. Earlier Office evidence stays separately scoped. No fresh Soft/Metal pixel-delta claim; native Metal static screenshot inspected. Original binary and Linux/Windows runtime not exercised.

All shell commands set DEVELOPER_DIR=/Library/Developer/CommandLineTools. No security/license change or publication.

SHA-256:

browse-dark.png: f8b2cfc84452123251d6741d0723cb52887ec5441373b28a6a90b28ce5d79748
browse-light.png: 28c4420f87f501c38c112113a919e676fbb13b34746ccf91de3ee40681d686a6
browse-narrow-dark.png: 89e9de38d75ce4da09473a0bf36c722e4c885d6eb719986c205f9a2bc30f21cd
browse-narrow-light.png: 0c1bd235ea775e05cce64864c048fce543c52e5298f3ff85aeafa7af5fe6967a
browse-native.png: 084b22ec5694542b2bd5d57fc8025c7796e54cd9cf71809c669a452c8ec2ff88
format-dark.png: 7298bc6e7e327e36dee53f95daf279d27ec92e911b1e86c01aa67e03a479a279
format-light.png: 6fccbb9faba8489e678b8d85d6be7217305b88fdc8ee93ea7d2d6c06121c3e0e
