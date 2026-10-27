# Capture provenance

Code: 1ca20533cf37a13e9ad6f077988f8f8c8b6a51cd. Soft: NEXT_GUI_DB_BROWSER_PLOT_QA_OUT=/tmp/next-gui-plot-namespaces cargo test --offline --locked -p e28_db_browser plot_namespace_selector -- --nocapture. Actual UI selector/Plot button uses same-named main/temp/analytics tables, 75 analytics rows and 64 plotted source rows. Dark/light 1400x1000 and 800x600 inspected; sips PNG conversion. Controls are on two rows and remain visible at narrow width. Long chart captions clip at narrow width (F01); this is not full visual acceptance.

Native: built e28, NEXT_GUI_GPU_SURFACE=1 NEXT_GUI_DB_BROWSER_QA_TAB=plot-namespaces, 15 analytics fixture rows/points. Own CGWindow 1891 captured via PID-specific lookup/screencapture -l; on-screen, NSApp active=false, logical 700x462, scale 2. Owned child stopped with SIGINT; actual exit observed. Static Metal painting only. No blind OS events or input acceptance. No new pixel-delta claim.

No shared painter/chrome/platform edit in this commit; Office examples built, previous captures remain separately scoped. Original binary and Linux/Windows runtime not exercised. Shell uses DEVELOPER_DIR=/Library/Developer/CommandLineTools. No security/license change or publication.

SHA-256:

plot-dark.png: 40bd6491c3be4a406687ba4daf88770a635a327fbe365ad5494fab1c52974ab5
plot-dark-narrow.png: d18e86ffd48460f5d0979ddd4a12ac84503f74185834bc264d40ec4cee15472d
plot-light.png: 06054b643045c89791c5172af003743a73f712beb43db4220daa707452967433
plot-light-narrow.png: 7529bb2fd4fdafc9a41f7d4dedc69d916ec1fbe73444088793a5332a2abdb6e3
plot-native.png: ad3f2a106c82d57547b3b2e08538c3fec5a31c733b7f59851885b7dc1f8f6b96
