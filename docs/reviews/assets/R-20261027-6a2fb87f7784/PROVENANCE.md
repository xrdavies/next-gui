# Capture provenance

Code: 6a2fb87f7784aff222fef2245e495b00a49be05a. NEXT_GUI_DB_BROWSER_WIZARD_QA_OUT=/tmp/next-gui-schema-wizards cargo test --offline --locked -p e28_db_browser schema_wizard_selectors -- --nocapture. Ten frames per renderer: Create Table/Index, dark/light, 1400x1000/800x600, plus narrow Table scroll endpoints. Actual selector/Save/failed replacement/type round-trip regressions precede rendering. PPM converted with sips; Swift AppKit generated both contact sheets. All twenty frames skimmed; wide table Soft/Metal and narrow table top/bottom/index inspected full-size.

Actual Metal readback: input interiors (excluding rounded primitive borders and clipped edges) max RGB difference 1 for all ten frames. Full viewport maxima: table top 73 in all themes/sizes, table bottom dark 11/light 12, index 10. The major difference is the selected List's fractional top edge: Soft (57,113,220), Metal (44,79,147) at (301,315) in wide dark; F01 tracks coverage parity. These measurements do not support a whole-frame delta-1 claim.

Native: NEXT_GUI_GPU_SURFACE=1 NEXT_GUI_DB_BROWSER_QA_TAB=table-namespaces / index-namespaces (second uses QA_THEME=light). Actual attached analytics customers. Own CGWindows 1919 and 1922, logical 700x462, on-screen, scale 2; NSApp active=false, hidden=false. Captured by own PID/CGWindow ID; each owned process stopped and exit awaited. Both full native frames inspected. Static painting only; no OS input/IME/clipboard/picker acceptance. No blind events or security/license changes. Original binary and Linux/Windows runtime not exercised. Shared Select accessor changes no painting/chrome; fourteen Office captures from the preceding Chart review remain separately scoped.

Shell uses DEVELOPER_DIR=/Library/Developer/CommandLineTools. No publication performed.

SHA-256:

wizard-soft-contact.png: 10df4715c1b91207f82ef35f6f7a925b896a8c4a3615ef92f9d132af9c7376bc
wizard-metal-contact.png: 912d63ac3449b1629dca63658160587c045626687b50ae951f59f7646f0f6086
table-dark-wide.png: e65f5304ac65daaddaceda14b4720b8be913276d7e039438a100c621898d6c5b
table-dark-wide-metal.png: 051403fc0521336ef5cc7c4c8eb837f66506003e45815fa9e01eaaf97d787b48
table-light-narrow.png: 30c361adf357785423d07318695fdcd007de3a661f25e4e5e1c7461d0ee5925c
table-light-narrow-bottom.png: b0061414ea356b77b51b670638a091bfaaa2e147c3de1b1af7d9a2675faf1499
index-dark-narrow.png: a91cb76236a6a24c578a75935f57cfaa1242f813502ec447247b268c3d9c41a3
table-native.png: e05eebcdb1871be5ccbaed9c6cae3684aabb17478245b75ffd9f2abb9fd640ad
index-native.png: f469be48e8cf26293a6d1700428f2f31c58ebe47297edb5393e952ba4f538b6e
