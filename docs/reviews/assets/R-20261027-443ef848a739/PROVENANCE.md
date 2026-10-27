# Capture provenance

Code: 443ef848a7391ee6e8aa236f9c0f40273a8b6d9c. Soft: NEXT_GUI_DB_BROWSER_SCHEMA_QA_OUT=/tmp/next-gui-schema-namespaces cargo test --offline --locked -p e28_db_browser schema_namespace_selection -- --nocapture, dark/light 1400x1000, sips PNG conversion. Own in-memory schemas only; the interaction test inserts one main row before capture and later verifies qualified DROP.

Native: built e28, NEXT_GUI_GPU_SURFACE=1, NEXT_GUI_DB_BROWSER_QA_TAB=schema-namespaces. Owned temporary local application bundle, launched with open -a; own PID 82156, own window 1852 capture. NSApp remained inactive; mob screenshot remained wallpaper despite open_app. No native key/pointer event sent. Exact owned PID stopped with SIGINT; process exit observed. QA fixture stays in memory; startup demo state is replaced before seeding. No shared painter/chrome changes, Office compiled but not recaptured.

Tree/DDL interior Soft/Metal comparison max channel delta 1 for both themes. An exploratory whole-window comparison reached 13 at dock-tab rounded corner (1069,192); strict comparison is scoped to the changed Tree/DDL widgets, excluding two-pixel edges. Do not interpret this as whole-window max-delta-2 acceptance.

All shell commands set DEVELOPER_DIR=/Library/Developer/CommandLineTools. No security/license change or publication.

SHA-256:

schema-dark.png: 3bb60afb479ac010e4c473055215a1f0a6a1cb802e6b25e5b8c75b9dd07c6096
schema-light.png: 51b0c58078c4e7c2c187efb545e4fe8af7162d849b867614c2256f6997cb7f8b
schema-native.png: 7fcb7e158ce975e9c95367ce494895d931b9b9607127d6a382c5fa81d527a915
