# Cell malformed-format confirmation evidence

Target: 987bcc9e25ac0230100c2f097dcbc5c7707d6527; parent: 8a037e8.
macOS27.0.1 arm64, Rust1.86.0, CommandLineTools;2026-10-29 JST.

Final checks3851, render65016 and native43228 all exited0 and were consumed.
737 workspace tests/43 groups,0 failed/ignored; strict workspace all-target Clippy
-D warnings, workspace build and host macos/windows feature checks all0.44 frozen
source/config hashes and final binary SHA256 are in metrics.json and were rechecked
after every driver. No dependency/features or shared UI changes.

20 whole-frame Soft/actual Metal pairs (40 complete P6 frames) cover dark/light,
1400/font13 and800/font18, JSON/XML marks and Cancel-default confirmation plus narrow
readonly views. Both sheets and raw readonly JSON/marked XML frames were inspected.
12 ready/PID/size-guarded native windows cover editable invalid drafts, prepared
questions and actual stored invalid readonly cells. Four private CGEventSource
postToPid flows in two themes and formats use Cmd+Enter, default Enter cancel,
Cmd+Enter again, Tab/Enter explicit Apply. All16 owned processes were awaited
SIGINT(-2), no clipboard is used. Native contacts/raw question/applied XML inspected.
Cancel interior pixels excluding top40/bottom90 (title/shadow/status) match exactly
in all four flows. The status notice intentionally changes. This does not assert
whole-window exact restoration, ordinary frontmost input or native database bytes;
typed saved bytes and document continuation are established separately by tests.

Focused pure/state/UI1286 exited0, Save All70513 exited0, final focus/readonly47250
exited0. v1 compile failed until the new ConfirmKind arms and mutable transaction
guard were integrated; v3 test compile exposed an Option<String>/Option<&str>
assertion mismatch, corrected. v4 pure test passed (its script result ID was not
retained; it was rerun with explicit final exits). v7 checks/render passed, but review
of confirmation's revision-changing apply path found focus could stay on an old
widget. The waiting v7 native driver87887 was intentionally interrupted130 before
launching windows, then v8 adds post-refresh focus and narrow readonly assertions
and repeats all checks/render/native evidence. No earlier run is final acceptance.

Regressions cover CJK JSON byte-to-scalar positions, multiline errors, JSON/XML EOF,
XML limits, typed TEXT/BLOB explicit raw overrides, no write before confirmation,
cancel/replay rejection, exact changed draft/column/row identity and readonly guards,
mode/source invalidation, unchanged malformed binary byte preservation and SQLite
trigger rejection. GUI tests retain selection/history/revision and modal focus;
Undo/Redo and valid edits refresh diagnostics. Save All keeps chosen paths, writes
no SQL/database/project on cancel, and defers Exit until explicit format acceptance
and successful save. Existing later save errors are not a new all-file transaction.

Mob screenshot at01:37:51UTC still shows wallpaper/loginwindow. No ordinary
frontmost pointer/typing/IME/picker/drag/resize/a11y/Print or Linux/Windows runtime
acceptance. Host feature checks are compile-only. Browser-only changes do not
require fresh Office captures; none claimed.

Original v3.13.1 EditDialog.cpp promptInvalidData577/accept: JSON marks a byte
position and XML a line/column before Apply/Cancel; malformed text may be stored
when explicitly accepted. Our existing house modal and Unicode scalar marks are
reused. JSON message columns retain parser byte units while visual marks convert
them; XML uses parser scalar columns and EOF marks. Parser resource limits,
INTEGER/REAL conversion and SQLite/readonly/generated errors are not overridden.
Confirmation binds source revision/path/browse/qualified table/rowid/column/mode/
type/draft/original/saved/compact preference and repeats write/storage guards.
https://github.com/sqlitebrowser/sqlitebrowser/blob/v3.13.1/src/EditDialog.cpp

Remaining: diagnostics validate the displayed UTF8 string; an unchanged raw BLOB
with invalid UTF8 inside otherwise valid lossy XML/JSON text needs source-encoding
validation/confirmation. Existing baselines retain its original bytes (no silent
replacement); compact-on rejects non-UTF8 storage. No-op Apply still updates and
marks dirty, target-column restoration remains case-sensitive, full BOM/codecs/
RTL/binary/history/word/style wrap/external/Print/Remote and complete original
visual/product/native/platform parity remain. F02 stays in_progress, no completion
or publication claim. Earlier Mob push approval timed out/denied; no new request.

Final local scripts/logs use /tmp/next-gui-cell-format-v8-; portable metrics/images/
helper source are archived here. Earlier iterations remain local.
