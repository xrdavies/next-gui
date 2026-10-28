# Field dropdown evidence

Target: `82c5b65cb2f84d1e5b0d75af895ee19aeeef2156`, macOS arm64.

- Final `NEXT_GUI_DB_BROWSER_FIELD_DROPDOWN_QA_OUT=/tmp/next-gui-field-dropdown-reviewed cargo test -p e28_db_browser field_dropdowns_open_on_first_click_keep_custom_types_and_save_collation -- --nocapture`: actual exit0 awaited. Both themes,1400×1000/800×600, closed/type/collation phases:12 Soft/Metal pairs, all maximum channel delta1. Contact and full small light inspected.
- Native: final workspace build, `NEXT_GUI_GPU_SURFACE=1`; type/collation/strict field fixtures in both themes. Ready-marker and owned-windowID captures from `/tmp/next-gui-wizard-native-ready-probe.py`. All six application exits−2/helpers0 awaited. Contact and full strict light inspected. The inactive windows did not retain open popups; these images verify closed controls/editor/static layout only. Native open popups/input/IME/pickers remain not_run.
- Office: e10–15/e20–27,14 owned GPU windows, sequential captures; all app exits−2/helper0 awaited. Entire contact and full e14 inspected. Shared Tabs text contrast and existing Select/Table/Autocomplete consumers checked statically. No native input acceptance inferred.
- Desktop screenshots remained wallpaper only,1920×1080; no blind pointer/keyboard automation performed.
- Fixtures contain isolated test data only. Full check logs under `/tmp/next-gui-field-dropdown-{workspace,clippy,build,macos,windows}-final.log`; all final exits0 awaited,565 tests/43 groups (e28333/UI167), zero failures/ignored.
