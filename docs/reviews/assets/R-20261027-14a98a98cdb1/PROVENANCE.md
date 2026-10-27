# Capture provenance

Code: 14a98a98cdb16804b9f9f3f80efe7c064979fe88. Soft: NEXT_GUI_DB_BROWSER_PLOT_QA_OUT=/tmp/next-gui-chart-caption cargo test --offline --locked -p e28_db_browser plot_namespace_selector -- --nocapture. Dark/light 1400x1000 and 800x600 use the actual qualified analytics selector and Plot action with 64 samples. All four Soft images inspected; matching Metal offscreen chart-region RGB readbacks have maximum difference 2 (not an assertion about the whole framebuffer or native surface). PPM converted to PNG with sips.

Native: NEXT_GUI_GPU_SURFACE=1; e28 uses NEXT_GUI_DB_BROWSER_QA_TAB=plot-caption and actual Chinese-named attached view/column with 15 points. Own CGWindow 1896 on-screen, active=false, logical 700x462, scale 2. PID-specific CGWindow lookup and screencapture -l. All fourteen Office examples and e28 freshly captured and skimmed in the contact sheet; e23 and e28 also inspected full size. Each owned process stopped and exit awaited. Captions stay above chart bodies without crossing tiles. Full caption remains in e28 tooltip and Chart accessible name; native hover/accessibility input not exercised.

Static painting only: native pointer, keyboard, IME, OS clipboard, file pickers, original binary and Linux/Windows runtime remain not_run. No blind OS events, security/license change or publication. Shell uses DEVELOPER_DIR=/Library/Developer/CommandLineTools.

SHA-256:

plot-dark-1400.png: 40bd6491c3be4a406687ba4daf88770a635a327fbe365ad5494fab1c52974ab5
plot-dark-800.png: a5bceddffca01d07c2b10a8cdf1ddeeaf3799653a1ff7c443455442fb6806010
plot-light-1400.png: 06054b643045c89791c5172af003743a73f712beb43db4220daa707452967433
plot-light-800.png: 49d989e9ea92ee6c5b062f27cf6ef870d864660c58b122c284ffb31dc75f0b2e
plot-native-cjk.png: c8cc1c4b3507b56c487c82fe6885adc9b7aa153c5e4ac38f26a9ff48d53279d6
office-charts-native.png: 51fc0d90b7185dd9955f7fb8dcf63efe3b6e66e4b32059c4cfb69aeed5517914
office-native-contact.png: 260ebe78333b9e4630eed37b2ecd7b3b1dd405050779fc0da285531433b893f0
