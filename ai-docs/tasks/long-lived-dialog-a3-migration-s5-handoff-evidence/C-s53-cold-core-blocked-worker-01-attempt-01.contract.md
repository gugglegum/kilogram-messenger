# C2-Q: blocked worker and external deadline

2026-10-31. **Contract only; callback/STOP before implementation.** Baseline389ff0b863980733dc10bc21cdf43be43ca3bb72. One synthetic case under [r42](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md)/[r43](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md); accepted [C2-P inventory/static arithmetic](C-s53-cold-core-inventory-boundary-01-attempt-01.accepted.md) stays closed.

Source anchors/pins: [Core](../../../scripts/migration-s53-cold-preservation.ps1)80325/fc4b9a78b1215b132644b8dd0ef544a6fcc304f85b42641614407fbbe92823cb; [deadline job](../../../scripts/migration-s5-l1-deadline.ps1)6038/0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45; current [W](C-s53-cold-core-inventory-boundary-01-attempt-01.preflight.ps1)78503/9e6375bd2babc7fd742d3f6a37df78827c9b781dfe9f344f59fa60d11b8db2f5 and [S](C-s53-cold-core-inventory-boundary-01-attempt-01.watchdog.ps1)34040/cc90b350404c34d584500c033a5dea26494d2c1759de6a241b301fdde9d3f32b. Reuse their guardian/job/pipe framework; no rewrite of deadline.ps1.

## Which mechanism acts

| Mechanism / source | Actual behavior | This case establishes |
| --- | --- | --- |
|Cooperative Check/Work: W22–29; Core Work/Phase |Throws only when execution returns to a check. Job CheckWork63 likewise only throws when called. |No worker work_expired claim while its execution thread is blocked. |
|Job work thread51–56; Children79/KillChildren84–93 |At work expiry sets expired and repeatedly terminates job **children excluding its owner**. Here the owner is the worker itself. |It does not unblock/kill this worker. Witness checks its descendants0. |
|S Check22–26, wait loop399–403, cleanup444–456 |Responsive parent detects Age>=20 and its cleanup Kill/WaitForExit targets its retained child Process object. |Primary mechanism: external supervisor work cutoff kills the genuinely pending worker. |
|Job whole thread57–60; S guardian338–356 |Worker's non-breakaway job terminates at its configured whole cutoff (about shared Age27). Independent parent runspace kills child from27 and parent from29. |Retained fallback protection, not an observed firing branch in the expected case. |

**One case suffices** for blocked-worker cutoff plus an observed bounded whole return. It does not test deliberate failure of the supervisor or runtime firing of either hard fallback. No second case disabling a protection is proposed for this obligation. Do not label the20-second primary kill as a hard-guardian test.

## Fixed block and durable reached-point evidence

Proposed family C-s53-cold-core-blocked-worker-01-attempt-01, leaf blocked-scan, selector deadline-blocked-scan. Future Core changes only append the closed selector/fixed root; use the existing fixture hook, unchanged caps and production logic.

Use healthy five-row/52B mirrors, six empty locks and safe-empty-S. Candidate3586B inventory/SHA2560567f3bb1b6057021c8746377d4ba54e25d1ddca41b29fc9fae05464d1e342c0 is derived in memory from the accepted healthy literal with only the fixed root changed. Expected setup16dirs/13files/3690B/57stream checks,39 file owners/30 enumerators closed and15 case guards released before Core;5 family guards remain.

Block at the **scan_before fixture callback**, after CaptureTree(s/b) and Core1030's expected-set/capture timestamp, before the first Scan1031. Passive hook sequence locks/inventory/capture_s/capture_bkp/scan_before, acquisitions10. Last complete38-field snapshot: expected_set_captured_before_hash=true/captureQPC>0, completed=false/observedsetsfalse/inputreleasedfalse, locks6, expected5/52, hashes0, historical H54b0a17d6616443770af22aceaf86ca63b36bc3bfd27f8afc979610b2d167df5, four observed digestsnull, failure/cause null. Source model:32 retained Core inputs, peak33,36 interned strings/path7024/record12086928, inventory3586. Safe-empty-S is not yet scanned/retained. No output owner/E/receipts.

Before blocking, write one bounded CREATE_NEW **witness.json**, Flush(true) and close its writer. Include attempt/allocation/source/shell pins, shared start/frequency, supervisor PID and worker PID/startUTC/process path, MTA state, exact phase/hook counts/Core snapshot, completed setup/stream/ownership evidence, fresh4types/3initializers/diagnostics and deadline job Children()==0. Record a pre-wait QPC, not a fictitious post-wait timestamp.

To distinguish this point from a stale/unfinished file, propose exactly two fresh fixed named manual-reset events, initially nonsignaled: Local\Kilogram.S53.C2Q01a01.ready and Local\Kilogram.S53.C2Q01a01.block. Parent creates them after allocation/guardian setup, requires CreatedNew for both and refuses collisions without modifying existing objects. Worker opens only these names before writing the witness. After witness closure, its hook calls **WaitHandle.SignalAndWait(ready,block)** with infinite wait. No code ever signals block; an unexpected return/exception is a failed case. Parent only polls ready nonblocking and reads the completed witness once after ready is observed.

The [.NET API](https://learn.microsoft.com/en-us/dotnet/api/system.threading.waithandle.signalandwait?view=net-10.0) signals one handle while waiting on another, and rejects STA. Launch only this worker with explicit -Mta and require actual MTA in the witness. [Windows documents](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-signalobjectandwait) that signal/wait are not guaranteed atomic: ready is evidence of **entry to this pending call**, not an instantaneous scheduler/stack trace. A never-signaled block event, source with no further operations before return, validated durable witness and worker still alive immediately before cutoff together establish the controlled blocked call. Missing response/file alone is insufficient.

## One start and bounded timeline

Use current S's monotonic StartTimestamp, derived once from its native process start; pass it unchanged to worker. All decisions thereafter use the same QPC/frequency. Worker job retains current remaining-time construction (work20/whole27); no timer reset after compilation/setup.

| Shared Age | Required behavior |
| --- | --- |
|Before20 |Guardian already armed; validate pins/claim, fresh events and owned worker; enter job before fixtures. Obtain ready plus complete witness before20; verify retained Process PID/startUTC/handle, exact pins/phase and worker still live. |
|At/after20, before26.5 |Parent Check records primary reason work_expired/cutoff QPC. Require witness already accepted and child live just before kill; record Kill attempt/return and observed nonzero exit. Retain actual signed exit code without treating arbitrary nonzero as PASS. |
|Before26.5/27.5 |Existing bounded cleanup reaps this child, then drains both EOFs. No descendant is launched by this case; pre-block job Children0 plus no spawn path after it and exited registered worker support owned-process absence. |
|Before28 |Close parent-owned inputs/events/pipes; persist complete bounded watchdog outcome. Known parent cleanup and actual observed child exit are mandatory. |
|27/29 fallback, whole30 |Job/guardian remain armed until normal parent completion/disarm. If primary cutoff fails, fallback outcomes are retained as failure/unknown for this case, not substituted PASS. Actual native wrapper return must fit whole30. |

Preserve the first parent cause/cutoff separately from kill/reap/pipe/cleanup failures. Kill only the retained launched Process object, never a guessed PID/name, root Codex or system process. No additional child processes or unregistered descendants. A running tool session may be polled for this same invocation; polling is not a second test.

## Exact outcome and honest forced cleanup

Expected supervisor shape: result=observed_expected_forced_timeout, primary_reason=work_expired, primary_mechanism=supervisor_work_cutoff, reached_witness_valid=true, ready_seen=true, child_alive_before_kill=true, kill_returned=true, **forced=true**, guardian_forced=false, observed nonzero child_exit, child_reaped=true, stdout_eof/stderr_eof=true, empty bounded pipes, output_cap=false, supervisor_cleanup_unknown=false, observed_owned_child_processes_after=0, whole_attempt_verdict=not_established. Pin witness length/digest and all QPCs; verify attempt/identity/pins and exact last snapshot independently in parent. Early/unrelated exit, missing/partial witness, absent ready, unknown identity, timeout without kill evidence, fallback activation or incomplete cleanup fails.

Keep the guardian's own forced flag distinct; aggregate forced includes the parent's intentional kill. Replace the case-specific happy-path validator, not its factual flags. Supervisor exits0 only for this complete expected-negative oracle; worker does not return0. Partial files/error returns remain preserved without retries.

There is **no final Core snapshot**: last remaining_owned_handles32/inputreleasedfalse stays unchanged. Record worker_final_core_observation=null, worker_finally_completion_observed=false, worker_close_count=null and worker application cleanup state not_observed_forced_exit. Do not fabricate CloseHandle/Dispose/hash finally/Finish calls or final bookkeeping0. [Windows termination](https://learn.microsoft.com/en-us/windows/win32/procthread/terminating-a-process) closes the terminating process's kernel handles, without executing termination handlers; that OS-release inference follows observed exit, not per-handle instrumentation. Process termination alone does not kill descendants: the no-descendant witness/source premise is required. Parent still owns its event references until its own successful disposal. Ambiguous worker ownership/process/pipe state remains cleanup_unknown and cannot pass this case.

## Scope and stop

Future writes only fresh synthetic files and three bounded native reports: started.json/witness.json/watchdog.json,65536B each/196608 total; run.json later records exact invocation/actual item/UTC/input/report pins. Pipes retain4096-character caps; source131072 each. Two parent/two worker event handles are separate from Core's ledger, with explicit current/partial-construction/closure paths. No generic IPC layer, event ACL changes, OS settings, blocked parent, extra processes or external network. Unexpected setup/error diagnostics remain bounded and are not accepted as timeout evidence.

Prospective **one work20/whole30/lane1/retry0** invocation; no allocation/reservation/ledger change here. Required source changes later: new W/S, witness/readiness and forced-negative oracle, fixed -Mta worker launch, minimal selector binding. Deadline/guardian framework stays intact. Native behavior remains unexecuted; source acceptance and separate allocation come first.

After accepted C2-Q, the finite synthetic remainder is producer-size positive with required receipts/phase/OS/resource metrics. Frozen launcher inference and private preservation/typed prerequisites remain separate; no new audit/gate, accepted-test repeat or S5.3/S5.4 completion claim. Only this contract is added: no script edits, runtime/compile/.tmp/private operations, budgets, commit/push. **Callback and STOP.**
