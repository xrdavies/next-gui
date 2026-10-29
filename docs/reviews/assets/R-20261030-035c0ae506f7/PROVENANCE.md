# Filter helper verification

Target `035c0ae506f71c7478551da1534ef09c8efb4c1b`, base `b7846676c9f75d34ce666402f8d07aa229be886b`.
Host macOS27.0.1 arm64 / Rust1.86.0; 2026-10-30 Asia/Tokyo.
The 157 source hashes and browser SHA in metrics remained unchanged through final checks,
Metal readback, all native workflows and Office inspection. No user database was used.

- `export DEVELOPER_DIR=/Library/Developer/CommandLineTools`; offline workspace test,
  strict all-target Clippy, workspace build and host macOS/Windows feature checks all exit0.
- `NEXT_GUI_FILTER_QA_OUT` executes the three filter menu tests:13 whole-window
  Soft/actual Metal pairs, maximum channel difference2, none above2. Both contacts and
  a raw narrow dark submenu frame were inspected.
- Eight owned native GPU processes:dark/light,1400x860/800x600, column/global filters.
  Private CGEventSource.postToPid keys open Shift+F10, enter a submenu, choose a helper,
  replace its selection, Undo/Redo and clear.64 stage frames are represented by four
  inspected contacts. Greater-equal3 gives3 rows; /alp/ gives2; all clear to5.
  The temporary regexp placeholder itself produces a normal visible validation error;
  entering valid criteria recovers. All eight children were signaled SIGINT and wait(-2).
- Fourteen fresh Office GPU windows e10–15/e20–27 were captured and inspected, then
  SIGINT/wait(-2); native driver completion was consumed before final metrics.
- Vision OCR initially failed with e5rtError13/exit133 while GUI work was active.
  After every GUI child exited, the same OCR executable exited0 on16 result/clear frames;
  numeric row statuses were asserted. Other OCR text is advisory. The first metrics
  collection preceded the last Office screenshot and failed its14-frame assertion;
  rerunning after driver completion passed.
- Before the final freeze, development checks caught a wrong test builder signature,
  an accidental removal of private helpers while moving popup code, an incorrect overlay
  variant, a String/&str mismatch and suspicious assignment spacing. All were repaired;
  final full checks and connected regression tests passed. No failed final check is hidden.
- A fresh Mob screenshot during this run showed wallpaper only. These captures prove owned windows
  and the bounded private-PID route. Ordinary frontmost input, actual IME, accessibility,
  Retina/mixed-scale, native pickers/Print and Linux/Windows runtime remain unaccepted.

Browser SHA256 `9a1ce9521bfeb95220262defe74eb74c8557e920f953d4cdae712d2dc6d02860`.
Scripts retain their original `/tmp/next-gui-filter-menu-v1-` output prefix.
No publication was attempted; the earlier Mob push approval timeout remains denied.
