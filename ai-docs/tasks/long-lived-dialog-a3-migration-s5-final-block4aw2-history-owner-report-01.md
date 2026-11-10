# Block4AW2: real history admission, A only

2026-11-10. Review candidate on HEAD `0ccdfca5088f0b6716a31c4defacd29f6229848a`. Three Rust files plus this report and pins. No commit/push before root acceptance. AW1 B/C remain separate accepted evidence; they were not repeated. No new F, Prepare/Run, WholeP, production control, CLI bridge, runner, chat or agent. F01-F11 remain consumed; S5.3 closed and S5.4 open. No old AU/AM16/AD16 raw logs, private profiles, keys or DB files were inspected.

## Implementation and actual proof

handoff_tests.rs adds an optional genuine two-context geometry. Existing constructors still choose None and preserve their one-context defaults. The extension uses actual create/import, reloads the Engines' committed Specs cache after the second import, admits/activates both contexts, selects committed sorted C1 at physical1, and queues real legacy A3/A4 after delivered A1/A2/B1/B2. Both source stores undergo the existing storage migration and profile open. Cold Ready1 snapshots establish C1[3,3], C2[1,1]; initialization preserves A queued2 and actual64/64 shares. No lanes, receipts, counters or ack_next are fabricated.

handoff_integration_tests.rs parameterizes existing Net by resolved slot; Net::new still delegates to slot0. Role array indices retain their meaning. Managed Binding/provenance prefix and existing Net::Drop are byte-identical to HEAD. The ordinary-only A admission diagnostic preserves the original A error if a peer closure masks it. The explicit finish helper, used only by new ordinary A cases, disconnects/joins tasks, checks window baseline, stops/joins owner threads and closes endpoints with errors propagated.

history_busy_tests.rs uses fresh synthetic ordinary fixtures and the existing actors, transport, legacy worker and production IPC handler through ipc_value. Real L1 delivery materializes migrated A3/A4 and reaches[5,3]. An actual request_upgrade/propose/prepare/select/commit_b/activate ceremony establishes cuts[5,3]. Actual queued Event2 A5/A6 and B3..B34 are materialized and explicitly delivered with Net to both owners at[7,35]. Both repositories retain the same40 unique sources; event_at plus verify_source2/data2::facts authenticates IDs, roles, ordinals and formats. This is manually driven outgoing RPC coverage, not an autonomous established-worker claim.

The real nonempty Owner7::ack_push2 return is retained outside its mailbox callback; its DispatchExisting7 owns32 references. Existing ACK queries clear genuine debt without changing the retained push cursor. Both owners must have zero queue/debt/pending. With current[0,32], actual production history IPC(slot1,None,16) returns exactly SchedulingWindowBusy while independent C2(slot0) succeeds. Dropping the actual retained object returns current[0,0]; the identical history request returns first16 with real view/frontier[7,35] and cursor. Reacquiring a genuine eligible ACK makes the unchanged cursor/limit32 suffix busy; dropping it returns window0 and the suffix succeeds with24, finished=true and the same view/frontier. The union is exactly the40 authenticated originals, with LegacySnapshot/FullL1/FullMixed proof kinds. Global native call counts and both OwnerStats remain unchanged during admission/read/release checks.

Small sensitivity coverage holds an actual ACK on opposite roleB at physical1, checks Busy there and successful independent physical0, then release/free-page success. The default-constructor compatibility case uses roleB/physical0 with its actual128 share and four genuine imported rows. It does not test a two-context main at physical0 or a full geometry cross-product. Both C2 repository observations remain unchanged.

Held dispatches live inside exercise(), whose completion/error/unwind drops them before Net cleanup. Successful cases explicitly verify zero refs, disconnect/abort/join tasks, stop/join owners and close endpoints; cleanup error remains failure. Each case has one absolute240s bound including cleanup/join; the whole focused command has570/600/30 work/whole/cleanup limits. Kept fresh test directories and all failed attempts remain in the AW2 administration directory.

## P2 cleanup review correction

Root verified the original functional A evidence and requested strict task-error propagation (root-review.json). Only the ordinary A path now opts into strict disconnect; ordinary_cleanup defaults false. Static checks confirm the original managed disconnect body after that guard and Net::Drop are raw-byte-identical, and handoff_tests.rs is unchanged from the reviewed candidate.

The ordinary drain inspects every owned Net JoinSet outcome. A JoinError panic and unexpected task Err fail; cancellation is accepted only after this drain requests abort_all. Already observed cancellation before that request fails. Expected owned-close errors require an actual owned connection close and a typed Iroh ConnectionError: LocallyClosed or ApplicationClosed with code0 and exact existing connection complete reason bytes. Message substrings alone never qualify. Tasks are fully drained even after a failure. finish retains the first error while still checking windows, signalling both owners, joining both threads and closing both endpoints; it returns that error after cleanup. This is a test-only opt-in, not a managed-behavior change.

The new third selector uses real Tokio tasks: unexpected Err impersonating a close is rejected with/without owned-close permission; an actual preexisting cancellation JoinError is rejected. It deliberately panics a task on a real ordinary Net and verifies finish returns its genuine panic JoinError after the pending-task Drop guard ran, both owner stop flags were set and both endpoints closed. A separately owned pending task is deliberately aborted/drained successfully. Cancellation classification awaits the real JoinError without an is_finished/notification race. It does not inject owner-thread panic, missing-binding or endpoint failures; those paths are code-reviewed, not independently fault-injected.

## Current verification

Rust1.98 Windows NativeToken, offline/locked, Jobs2 BelowNormal, warmed verification-scale opt3. Compile-only uses --no-run; all network test execution uses the existing stable runner/fixed harness paths and --test-threads=1. Strict Clippy includes --lib --tests -- -D warnings. No AW1 or full-matrix rerun.

| Current check | Actual child/owner exit | Seconds | Work/whole/cleanup seconds |
|---|---|---|---|
|compile-05|0/0|132.2510542|3600/3660/60|
|clippy-06|0/0|12.1808542|3600/3660/60|
|fmtcheck-03|0/0|1.3917465|570/600/30|
|focus-03|0/0|24.6423057|570/600/30|

Three current AW2 tests passed; no failed/ignored cases. Other package harnesses ran zero matching tests:

- `ordered_dialog::runtime7::handoff_tests::integration::history_busy::s54_history_owner_default_fixture_and_slot_zero_compatibility`
- `ordered_dialog::runtime7::handoff_tests::integration::history_busy::s54_history_owner_real_dispatch_first_suffix_and_independent_context`
- `ordered_dialog::runtime7::handoff_tests::integration::history_busy::s54_history_owner_task_panic_error_and_owned_cancellation`

Immediate before/after source3 and executed library EXE pins match: 33309184B SHA256 `a6b72b573dd5aff3c4d2cdde012a0a7507efdd2242957d046465040f5b6deb69` at its fixed diagnostic verification-scale path. Current source closure is derived as536 files, all rehashed successfully; all nine production images are unchanged. final-process-inventory-02.json independently finds zero live exact diagnostic harnesses. Source diff including the new file passes reverse git apply --check.

The actual passing library suite took17.43s (wrapper24.6423057s); final focus cleanup was unforced, empty and verified. Fresh stderr still includes the normal existing connection complete peer-close messages and a nonfatal L1 background lease-absent diagnostic. These are retained; strict Net JoinSet result handling does not claim to fault-inject or reclassify internal production L1 worker diagnostics.

## Preserved attempts and limits

The original reviewed candidate5, diff, closure, manifest and static checks are retained in before-cleanup-review. focus-02 passed the original two A tests; its normal owned peer-close diagnostics informed the typed classifier. focus-01 failed both tests: default B peer-close at29080ms masked the original A cause (still unproved); unknown OD1 context was corrected by reloading committed Specs after the second import. The optimized retry did not change the production30s deadline. Failed source3 and original outputs/receipts remain preserved; no fixture was repaired/retried in place. The original report records those results in full.

Clippy-05/compile-04 passed the first cleanup revision; the cancellation regression was then made deterministic, followed by current Clippy-06/compile-05/fmtcheck-03/focus-03. A local static-check helper initially assumed CRLF guard bytes; the source uses LF, so that administrative assertion was corrected without changing source behavior.

All22 bounded owner receipts are retained. Forced owned residual cleanup occurred at clippy-01=[42976]; compile-01=[51644]; compile-02=[48304]; compile-03=[41372]; compile-04=[25880]; compile-05=[11532]. Residual executable identities were not retained and remain unknown; all after-lists are empty/verified. No final deadline expired. The current test receipt supplies actual exits, EOF and cleanup rather than substituting a task count for success.

A proves real storage/Owner7/production IPC-handler admission with manually driven Net outgoing RPCs. It does not establish combined CLI/socket/Controller/Owner7 end-to-end, autonomous established-worker coverage, full F/169/218 acceptance or the original F11 holder/release. F01-F11 remain consumed. No new F/Prepare/Run/WholeP, production control, bridge, runner, agent or chat. Historical raw/private access remains closed.

Publication candidate remains source3 + report/pins (five paths); foreign work is excluded. HEAD/index are unchanged/empty. The pins file omits its own recursive hash; administration/candidate-manifest-02.json pins all five final raw hashes/Git blobs/modes. Await root review and explicit publication assignment; no commit/push performed.
