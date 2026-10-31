# Cold-A timeout cleanup: bounded diagnosis / attempt02 proposal

2026-10-31. Source/public evidence only. Baseline **a20dcb3dea0b83d0b2cda07ad6b1b8d7de5f9078**, parent0640216. Attempt01 failed evidence is published unchanged; this proposal implements and authorizes **nothing**.

## What is established

[Attempt01 review](P2-s53-exhausted-cold-a-01-attempt-01.actual-review.md) and [run](P2-s53-exhausted-cold-a-01-attempt-01.run.md): work_expired; no normal successful wait; Supervisor killed/reaped Parent in cleanup; P2/Parent absent. Preservation and private cleanup remain unknown. Handles14513/WS230580224 do **not** locate a Core phase or prove capture/hash progress.

[Launcher](../../../scripts/invoke-migration-s53-cold-preservation.ps1):814 immediately kills a live retained child on entering finally, before its52s reap allowance. This demonstrably cuts short the Parent cleanup/report window; it plausibly explains missing diagnostics, **not the original45s overrun**. No phase/I/O-speed explanation is established; setup, metadata acquisition, scan/hash/receipt work and blocking I/O cannot be distinguished from these reports.

## Existing cancellation, cleanup and reporting boundaries

- Launcher:30–33/644: common work45; Parent job whole50. [Core](../../../scripts/migration-s53-cold-preservation.ps1):471–480 polls the same absolute work deadline. It does not cancel an in-flight synchronous native I/O call; a blocking call may not return in time to unwind.
- Core:674–697 preserves the first failure code/phase/role. CaptureAndCompare:1069–1075 closes inputs/hash state in finally, then stops the current phase and updates facts. CloseInputs:1036–1048 and Dispose:1310–1321 have **no Work() gate**; cleanup can proceed after45. Phase durations can include final input cleanup, so they are not pure hash-speed measurements.
- Parent:681–691 switches to cleanupReserve, disposes Core/output/guards, closes public streams and cleans job descendants. Existing Core-Facts after disposal remains usable: aggregate failure_phase/failure_role/phase_seconds, capture/pass/hash counts, remaining handles and cleanup flags. Owner Dispose:231–240 also closes without a work-deadline check.
- Parent:694–718 writes existing failed P2/Parent only after disposal, under Check() cutoff50. Capture failure skips private WriteReceipts; no speculative receipt repair is proposed.
- [Deadline helper](../../../scripts/migration-s5-l1-deadline.ps1):51–63 sets WorkExpired at45 and kills **descendants**, not its owner. Its whole thread terminates the job, including Parent, at50 if still armed. Finish:100–107 stops both helper threads/disarms that job after children0; therefore do not assume its50s kill remains armed after Finish.
- Launcher's independent Parent guardian:863–877 remains separate until outer finally and kills Parent at51. Supervisor guardian still targets its retained child at55/self59; outer60 remains. None guarantees final managed reports when native I/O/disposal is stuck.

## Smallest proposed repair

Change only launcher timeout supervision/failed-report collection, plus fixed attempt02 identity bindings described below. Keep shared deadline helper, Core comparison/cleanup/receipt algorithms, output ownership and all work45 checks frozen.

1. Freeze first failure/pre-cleanup gate at expiry; normalCandidate/normal gate stay false. Enter **cleanup only**. Pump bounded pipes and perform actual retained-process waits through cooperative grace, without immediate Kill(). Work never resumes.
2. Parent has until50 for cleanup/report writes; its independent51 guardian remains armed for post-Finish/report stalls. If still live at common age51, Supervisor attempts one retained-child Kill, then waits/reaps until52, EOF drain54, report57, guardian child55/self59, outer60. Grace uses the same absolute start, never a reset or extension. Already-exited-but-unreaped is waited, not killed; exceptions preserve cleanup_unknown.
3. After reap/EOF, use the existing Read-Report/closed field/type/size checks to collect any actually present claim/P2/Parent during the Supervisor report reserve. Verify attempt/D1/common clock, PID/UTC, claim→started and Parent→P2 raw hashes; missing/malformed/partial remains missing/unknown. Do **not** call success-only Check-ChildReports to validate failed observations, and do not weaken it. Keep successful checks unchanged on the normal path. Failed-report read errors are secondary and cannot replace work_expired.
4. Surface the retained Core aggregate facts already in failed P2. Keep Supervisor hash bindings when available and distinguish valid failed child diagnostics from complete success evidence. No private E/input/receipt re-open. No new phase telemetry/checkpoint initially: graceful failed reports are the first diagnostic source. At most one bounded force-call QPC alongside existing cleanup waits/gate may be justified to prove the force boundary; not a telemetry framework.
5. Even late exit0 or late complete comparison cannot promote this attempt: successful normal wait/exit0/bothEOF/full source-and-child checks must still finish before45. Cleanup reap, timeout, nonzero exit, force or unknown remains failed_or_unknown. Private cleanup is known only from valid final observations, never inferred from process exit.

A50/51 guardian kill can still prevent diagnostics if Parent remains blocked or cleanup exceeds its window; preserve that truthful unknown. A future checkpoint would require a demonstrated remaining diagnostic need and separate tiny review, not automatic instrumentation. No blanket work-deadline increase is proposed.

## Finite validation and distinct next attempt

Propose **one focused C allocation work20/whole30**, lane1/retry0, containing four closed dummy controls (five fixed scenarios, including the two Parent-guardian subconditions below), with short test-only timings preserving cutoff order. Reuse actual reviewed supervision/report functions and deadline helper via explicit pinned mappings/AST extraction, not a duplicate algorithm/general harness. No inventory, private tree, real-size Core or old comparison matrix.

| Dummy control | Required evidence |
| --- | --- |
| Normal child returns before work cutoff | Actual normal wait+exit0+bothEOF; existing success criteria unchanged. |
| Work expires; child cooperatively reports/cleans in grace | No premature kill; failed first reason unchanged; real cleanup wait/reap, valid failed report/hash linkage retained. |
| Parent blocks across its job/guardian cutoff | Actual job50-equivalent kill (and independent51-equivalent fallback after simulated Finish); no guaranteed P2/Core cleanup claim. Two fixed subconditions within this control. |
| Supervisor fallback must force a live retained dummy | No force before grace; one force attempt at the fixed bound, actual successful reap by its cutoff; missing EOF/unreaped/late exit0 never becomes normal success. |

Native test source/mappings/caps need review before allocation/execution; if the single bounded control run fails, retain evidence and stop. No runs are performed here.

After accepted source/control evidence, propose one **F work45/whole60, lane1/retry0**, identity **P2-s53-exhausted-cold-a-01-attempt-02**:
- Fixed public prefix: `E:\AI-Projects\kilogram-p2p-messenger\ai-docs\tasks\long-lived-dialog-a3-migration-s5-handoff-evidence\P2-s53-exhausted-cold-a-01-attempt-02`; distinct D1 and five reports.
- Fresh E: `E:\AI-Projects\kilogram-p2p-messenger\.tmp\migration-s5-preparation-evidence\P2-s53-exhausted-cold-a-01-attempt-02`; same exact O-lock-only/S/BKP/inventory roles and optional-parent rules. Attempt01/private evidence is never inspected/reused/deleted.
- Later source edits must bind the same closed attempt02 in launcher, Core.ProductionAttempt and production output literal; no path/command override or fallback attempt selector. New source/review pins and D1/native plan require review before activation. This document creates none of those files/directories.

Budget is **proposal only**. Actual-review snapshot: inactive C689.1175091/F23.9217433. C30 fits; F60 needs another36.0782567 of internal F headroom (ceiling at least1678.0782567 at unchanged spend), set only by root. No reservation/charge/ceiling edit or cross-category borrowing here. Standing user test-account authorization covers necessary reviewed local tests with these identified copies/keys; no renewed user-permission gate solely for test-key bytes.

## Delivery boundary

Public source pins unchanged: launcher71513/8cb43e3e6dfba31b21f014ea4f82be1451417a15424ed046d87d2d455d6f3557; Core82999/59787bd263052b12cc77ab524ce48b27f18e2e7a83e0039d9b40f8c3066573af; output21721/331bfdde271d6386f78e2198da2ef9480b3c16f5e45dc3180747601d8cac9b98; deadline6038/0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45.

Only this proposal is authored after the accepted12-file publication. No source implementation, import/compile/test/runtime/private query, new D1/claim/E, nav/ledger or further commit/push. Callback root and **STOP before implementation**.
