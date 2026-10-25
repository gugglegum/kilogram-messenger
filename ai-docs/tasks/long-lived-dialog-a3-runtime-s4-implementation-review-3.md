# A3-S4 implementation review 3: F09 controller IO investigation

Root decision, 2026-10-25. Implementation is not accepted; no publication yet.

## Independently observed failure

`f-full-review2-frozen-1` ended with exit 1 in 365.2830575s. Its CLI
actor binary passed 5 cases; its process binary passed 8 and failed F09 with
unadorned Windows `os error 5`. The wrapper stopped at this failed binary, so
subsequent state/transport binaries in its executable manifest are not evidence
of executed tests. This complete invocation remains FAIL.

The F09 fixture `F-operational-C7STN1` retains both original `{}` schedule
files, each with Archive attributes. The source puts the first schedule update
after the first readable Text and before sending Text 2..5. This is consistent
with an error during the first controller update, but does not identify its
precise failing operation or establish an environmental cause.

Root inspected `Node::schedule`, `shared_admin::read_public`, and the local
tempfile 3.27.0 Windows implementation. Schedule updates create a same-directory
NamedTempFile, write/sync JSON, then persist over the destination. These operations
currently lack error context. On Windows tempfile's persist performs both
SetFileAttributesW and MoveFileExW; either can fail. Do not retrospectively label
the observed error as a confirmed rename/sharing violation.

The production EXE remains 66188800 bytes with SHA256
`d5d8c8c118925dfde67e37fdb32aa5a120293cde258e93843dd1939fddb15560`.
P under the review-2 one-run exception has not begun.

## Authorized narrow controller work

1. Preserve the failed run and fixture. Add operation/path error context to
   schedule serialization, temp creation, write, sync and atomic persist.
2. A bounded retry may be added only around this controller's atomic persist
   for Windows raw errors 5, 32 or 33. Retain the same prepared temporary file
   and bytes from PersistError; do not recreate, delete/truncate the destination,
   change ACLs/attributes of the destination, or use a non-atomic fallback.
   Use at most one second total retry time with short bounded backoff; retain
   the last error, attempts and paths on exhaustion. Other errors fail promptly.
   This tolerates temporary replacement contention without assuming that every
   AccessDenied is transient; a persistent denial must remain an error.
3. Verify the helper using ordinary Windows test-file handles that deny delete
   sharing: eventual release permits exact replacement; a held lock exhausts
   the bound without changing the original destination. Verify old/new complete
   bytes and no partial destination writes. A different OS code in this control
   does not prove the exact cause of the historical F09 failure.
4. Rebuild only the stable test harness after all previous processes stop. Keep
   the 42 frozen production source hashes and production EXE identical. Keep all
   protocol checks, fault timing, scenario counts and existing deadlines intact.
   Run the controller controls and one real F09 through the stable wrapper.

Return the exact controller diff, source/EXE correspondence and targeted results
to root before another complete F or P. No further product/performance changes,
silent retries or deadline changes are authorized. Review-2's unused one-run P
exception is unchanged. Root will separately authorize the complete reruns after
reviewing this repair; the original failed F remains in the development ledger.

## Repair reviewed; full verification authorized

Root inspected the complete controller helper and exact patch, checked the raw
test summaries, and independently rehashed all42 product files, all4 frozen
images, and every Rust source entry in the real-F09 snapshot: zero mismatches.
The patch is8857 bytes, SHA256
`8e66c7bb3f8c2b96ed482957878f590adf8a5311e95cc1490ac9de38d8102f6f`.
Only the controller module/delegation and new schedule_control.rs changed.

The retry budget is500ms with20ms backoff. The release control now drops its real
deny-delete handle in response to the first actual persist failure after checking
complete original bytes; it does not depend on a timed release. It passed with2
attempts/21092us and exact new bytes. Persistent contention returned Windows os5
after505717us, preserved both path/held-handle original bytes and retained the IO
error. That control illustrates the error class but does not prove the historical
F09 failure's exact cause.

Stable-runner results inspected: controller controls2/2 in22.3189717s; one real
F09 in35.3227291s, including fourth actual Native trial rollback, old prefix,
no premature receipt, C2 progress and successful retry. F09 did not need a schedule
persist retry. Strict six-package/all-target Clippy passed in3.9306888s. Existing
tracked diff whitespace check passed. No further helper change is required.

Root accepts this controller repair for verification, not the whole implementation.
Authorize one complete F on the unchanged d5d8c8 product and reviewed controller.
For conservative accounting, include the185.5564679s guard selection,22.3189717s
controls and35.3227291s targeted F09 plus this new complete F in the final F total;
their subtotal is243.1981687s and aggregate F remains<=1200s. Previous failed and
replaced candidates remain explicitly recorded outside that final successful set.

If this full F passes with complete executed-case evidence, proceed without another
permission round to the single unused full P attempt authorized by review2:
controller2950s, runner<=3000s, fresh fixture, all substantive assertions unchanged.
Run F then P sequentially because the current stable wrapper copies fixed EXEs on
each invocation; do not overwrite a live binary. On a new failure preserve evidence
and callback rather than changing sources/deadlines or silently retrying. On success
return exact P/F/component/static results, final source/EXE correspondence, limits,
candidate publication allowlist and callback for root's final implementation review.
No commit or push is authorized by this verification decision.
