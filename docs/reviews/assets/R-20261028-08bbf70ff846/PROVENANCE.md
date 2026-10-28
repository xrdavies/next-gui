# Field removal and modal acceptance provenance

Implementation: `08bbf70ff846f52da83620b537731b9e8554114e`, macOS27.0.1 arm64, Rust1.86.0.

`field_removal_confirms_and_updates_keys_without_losing_qualified_rows_or_dependencies`
ran with `NEXT_GUI_DB_BROWSER_FIELD_REMOVE_QA_OUT=/tmp/next-gui-column-removal-final`.
Four final Soft/Metal comparisons: dark1400/800 delta1/1; light1400/800 delta2/2.
Actual test exit0 awaited before sips PPM conversion. Contact and full light800 image
inspected. First captures showed opaque scrims and low-contrast light buttons;
final captures verify translucent backgrounds, readable separate action colors,
compact confirmation and Cancel default. Shared tests assert4.5:1 contrast in both
focus states and both default themes.

Final UI build ran owned e28GPU QA_TAB `table-field-remove-confirm`, both QA_THEME
values, output `/tmp/next-gui-column-removal-native-final-*`. Own onscreen layer0
window IDs captured after ready logs; each app SIGINT/awaited exit−2, helper0.
The14 Office examples (e10–15,e20–27) ran sequentially with owned exits−2/helper0.
Complete native contacts and full light e28 inspected. The subsequent retired-field
migration-only refinement changed no rendering; final tests/build include it.

Actual desktop remains wallpaper with inactive owned windows. Static captures
establish rendering, not native keyboard/pointer/IME/Save acceptance. Original
binary and Windows/Linux runtime unavailable.

Final workspace549 tests/43 groups, e28320/UI164, no failures/ignored; actual exit0
awaited. Strict Clippy/build/macOS and Windows feature checks each awaited exit0.
Dependency tests exercise21 main/temp/attached combinations: independent/expression
indexes, quoted views, triggers, removed-name reuse, absent override and successful
retained dependency/sequence/pending transaction rollback. Generated removal after
retained type edits passes all three schemas, including STRICT; ordinary-table
focused regression passed before strengthening that fixture to STRICT.

Initial tests caught automatic-index protection, native PK DROP, quoted-view DQS
fallback, overly broad strict parsing during TEMP renames, modal focus restoration
and discarded generated NOT NULL checks. All corrected before final checks. No new
dependency/platform source change. Publication approval1960 timed out/denied earlier;
no push attempted.

- `native-contact.png` SHA256 `9c3f68612bf33aa67f2aa406cd3f37b1e8520a3bd46d6f9c230fda979308fb9d`
- `native-light.png` SHA256 `5c51856adf4009d012695b3cf067f93f5f751eec9674237687616eeb4fab3fe4`
- `office-contact.png` SHA256 `1fb37b118f47b343fcff995ec66799d216e65ca4ffe4e177e225666ce9abbfaf`
- `small-light.png` SHA256 `c3238e61a3aa9969a9519f79eb465ab1efaff6d939793b83b5a7e1b37a16492c`
- `soft-metal-contact.png` SHA256 `fe7dc4a2e1d72cbfc52ba842aec0459e0e956142118ef927f0b9f7647c3ceb77`
