# Block4AU: F11 failed on busy history admission

2026-11-10. Read-only diagnosis on HEAD `8a3ba3be34c7a2697e6f48ca086d6b190fa1bdcc`. F11 remains consumed and failed: wrapper1/native101/launcher124, `harness_nonzero`, exit/EOF at original T0+184843ms, before450/480/510s. No replay, new purpose, activation, build/test, product/source change, WholeP, commit or push. S5.3 remains closed; S5.4/WholeP remain unproved.

The newly established CLI error is **SchedulingWindowBusy**. A's last observed C1 reservation was32 of64; history IPC requires34 more. The immediate failure is the fixture treating this bounded admission rejection as fatal. This supports a narrowly bounded S54 history-reader correction, not increasing the product window or weakening history assertions. Exact network permit ownership/release timing and eventual liveness were not retained and remain unproved.

## Evidence and last successful operation

Administrative evidence: `.tmp/s54-block4at-f-runtime-11/{postcheck-01.json,result-facts.json,root-review.json}` and F11 `outer-result.json`. All five bindings and join1/permit1 passed. Runtime forced owned cleanup ended empty; supervisor cleanup was unforced/empty. No functional/assertions/native-capture/wrapper-completed marker. The inherited postcheck key `actual_native_exit=1` denotes the Run-wrapper exit; actual harness101 is in process_outcomes. Original durable desktop-return=pending was preserved.

AU inspected only numeric pairs306..313 already listed by AT. Before content access: inspected recorder/schema, checked canonical direct-child containment, regular files and all ancestors for symlink/reparse attributes, and checked size bounds. No directory descent into f-origin/profiles/keys/DBs or any other attempt. Sixteen files,27679 unique bytes; three bounded passes read80031 bytes in total, below2MiB, every file below64KiB. No backward extension was needed. Original hashes matched on the second pass and the third-pass status subset. Only whitelisted numeric/enum/schema findings were saved. No raw copies, errors, private argv or message bodies were emitted.

Recorder sources: tests/support/migration_s53_deadline.rs:170-186,216-287 allocate the shared sequence and file-backed ordinary CLI output; migration_s54_capture.rs:87-104,108-163,240-285 intercept status/memory queries, sanitize status and record closed capture facts; migration_s53_handoff.rs:269 transforms keys into fingerprints. These are under apps/kilogram-cli/. Capture command ordinals are shared with ordinary CLI and memory queries; they are not workload ordinals.

| Command | Publicly extracted observation |
| --- | --- |
|306|A current status: C1 next[7,31],pending4; window current[0,0].|
|307|Owned memory query succeeded, PID39004,8466432bytes; capture code0,waited/EOF.|
|308|A status: C1[7,35],queued/debt/pending0; window current[0,32].|
|309|B status: C1[7,35],debt8,pending/queued0; window current[0,0].|
|310|Owned memory query succeeded, PID48132,9015296bytes; capture code0,waited/EOF.|
|311|A status: C1[7,35],queued/debt/pending0; window current[0,32],peak[0,64].|
|312|Last actual successful command: B status, C1[7,35],queued/debt/pending0; window current[0,0],peak[0,34].|
|313|stdout empty; stderr28bytes. SHA256 exactly matches source-defined `Error: SchedulingWindowBusy` followed by LF. Ordinary recorder does not save the numeric child exit code.|

Status captures306/308/309/311/312 all record code0,exit_observed/waited/stdout_eof/stderr_eof=true,kill_requested/cleanup_pending=false. Both contexts have head_pending=null. On both owners logical C1 is physical slot1, Active/family2 with cuts[5,3]; logical C2 is physical slot0,L1/family1,next[1,1],all counts0. Leases current=true. Source identities are stable per role and distinct between roles; context order agrees, without exporting identity values. Actual cumulative A stats at311 encrypt4/decrypt32/attempts36, B at312 encrypt32/decrypt4/attempts36; all aborted/ambiguous/account/outbound/inbound counters0.

`static-error-comparison-02.json` compares36 finite encodings of six source-defined errors (actual Display, prefix, LF/CRLF/none); exactly one matches command313 stderr SHA256 `e0b59099227dbb01ecd7ccc5824a20910213db9dcac8265ce38cda8adb5ce753`,28B. These candidates do not match the outer retained271B harness-stderr prefix hash. The harness error chain/private CLI argv cannot be reconstructed from that prefix and were not guessed. The first administrative comparison used enum spelling ReopenRequired;02 corrects it to actual Display and leaves the busy match unchanged.

## Source path and attribution

All following paths are repository-relative, with current lines:

- `apps/kilogram-cli/tests/support/migration_s54_feasibility.rs:137` waits for next/debt/pending/queued/head_pending; it does not require a free scheduling window. Lines425-427 complete cuts[7,35] and native delta checks; line429 next calls Frozen40 capture. No intervening CLI operation exists.
- `apps/kilogram-cli/tests/support/migration_s54_feasibility_data.rs:80` first requests16 rows without a cursor; suffix32 follows only after first success. `migration_s5_l1.rs:336` sends history3 with the resolved physical slot. `migration_s54_control.rs:433` resolves logical contexts through retained Ready mapping. This maps command313 to the first A40 history request by source/control-flow inference, corroborated by the final A/B status pair and unique static error. No per-command argv or operation tag was retained.
- `apps/kilogram-cli/src/ordered_dialog/runtime7/runtime.rs:417` parses history; line427 reserves34 refs **before** line428 enters repository.history. `runtime2/window.rs:18` grants64 per enabled context; lines29-36 reject an over-cap reservation with SchedulingWindowBusy.32+34=66>64 even when the requested page limit is16. This rejects before reading a history page; it does not demonstrate a history-data/format/cursor failure.
- `runtime7/runtime.rs:683`, `apps/kilogram-cli/src/ordered3_cli.rs:147`, and `tests/migration_s5_process.rs:128-138` propagate the error through IPC/CLI and turn any nonzero CLI into a test error. There is no busy retry. Native101 and the completed failed-test summary are consistent with this path; the numeric command313 exit code is not independently retained.
- `runtime7/protocol2.rs:750,887,999` holds32-reference permits in outgoing delivery/ACK dispatches. `runtime7/network2.rs:627-668` keeps dispatches across RPC/completion. This supplies a plausible legitimate source of A's observed32 refs. The particular dispatch, whether it was ACK/query/reply/delivery, and its final release time are **not established**. A at311 and command313 are different sampling instants; the failure proves insufficient room at admission, not a same-instant32 measurement.

Established attribution: a fixture admission-handling assumption is incompatible with the existing bounded product API under actual concurrent traffic. The evidence does not establish a product permit leak, starvation, corrupted migration, a memory-binding defect or an administrative deadline failure. Busy alone is not a functional history result; an implementation must not count it as a passed query. Increasing64, reducing the fixed34 reservation without a memory proof, stopping peers, bypassing admission, or using a different context/cursor would change the tested behavior and is outside this repair contract.

## Minimum correction for architectural review; not implemented

Use one S54-specific typed per-page history reader: `Page(actual_json)`, `Busy`, or terminal failure. Retain the real CLI path, physical slot, supplied cursor and limits. Classify Busy only from an actual unsuccessful command with the exact source-defined closed SchedulingWindowBusy response; never substring-match arbitrary errors, treat malformed success JSON as Busy, or retry other errors. Do not broaden the shared legacy Node.history/cli behavior.

For the F history family (Frozen40 initial/suffix/verification and continuous F history reads), retry only Busy inside one operation deadline fixed at entry: min(original controller deadline,existing phase deadline,entry+30s). The first16 and suffix24 share that deadline. Between attempts service Controller.check, current child liveness/decoder/memory progress and wait at most25ms; every CLI remains bounded by that same effective deadline. No deadline reset per attempt/page, nested unbounded wait, extra process alias/source binding or native work. A free-window status may explain pressure but cannot replace retrying actual admission: a check-then-query race remains possible.

On success retain all existing F assertions: first16,real cursor,exact suffix24,same view/F[7,35],40 genuine sources (A1..6+B1..34),and later frozen continuity against current169/218. A permanent busy result must fail within the bound. P helpers/frontiers remain unchanged pending explicit review; sharing this reader with P would require separately demonstrating its cold/warm and held-window behavior.

Add only a closed diagnostic at this boundary if needed: stage=`f_history`,operation=`first|suffix|resume|continuous`,reason=`busy_exhausted|command_failed|invalid_page`,numeric owner/logical-context/physical-slot/limit/retry-count. Preserve the original failure category, emit no raw error/argv/cursor/body, and use the existing bounded protocol channel. No generic error-export framework is needed. For F11, the exact missing fact is dispatch-kind/release timing; proving a product leak would require an authorized fresh deterministic held-dispatch regression, not more profile/DB/raw-log access.

## Required failing-path regression and adjacent assumptions

Before running tests, review these explicit prerequisites; no tests were run in AU.

1. Real imported Ready data, two owners and both contexts; migrate cold via existing FirstCurrent lane-root verification, then start Owner7 so metadata/history identity are initialized. Keep strict warm reads thereafter. `migration_s54_source.rs:786-790` and runtime7/owner.rs:184-218 distinguish these states. A fresh standalone repository or toy Node page closure alone cannot cover this failure.
2. Reproduce F's warm Active C1 cuts[5,3],both next[7,35],40 sources,A role0/B role1; C1 physical1,C2 physical0 exactly as F11. Control the actual32-ref dispatch/permit lifetime through Owner7 and call the actual history IPC/CLI with limit16. Confirm busy before release, then successful admission after release under the same operation deadline. Exercise an intervening busy on the supplied-cursor suffix too. Test free-window and opposite-slot mapping as focused sensitivity controls, not a full matrix.
3. Assert returned16+24 with identical view/F and genuine IDs, zero additional encrypt/decrypt/constructors, refs released to baseline, no cross-C2 effects and no source/nonce rebinding. Include persistent busy until expiry and a non-busy terminal error that is not retried. A fake parser-only retry test is insufficient; a small parser negative may supplement the real failing path.
4. Preserve subsequent F checkpoints: frozen verification occurs **before** arming source64 (feasibility.rs:500-506); same-context history cannot fit during a64 hold and must not be silently bypassed. C2 history during that hold uses its independent physical0 share. Offline129 yields current A169 at[136,35],while retained A40 remains historical; later current218 does not replace it. Those stages were not reached by F11.
5. Do not transplant F frontiers into P. P cold A migration is[17,16],P warm mixed boundary[25,16],C2[1,1]; P frozen39/suffix23 and other preserved acceptance assertions remain separate. Existing migration source IDs, source-bound memory aliases, interval transitions and physical-slot mapping stay intact. Successful queries310-312 and source flow show progress past the earlier startup/binding boundary, not final memory/interval coverage or WholeP acceptance.

Future verification must follow AGENTS.md: compile without running, then stable Windows harness paths with Jobs2/BelowNormal and explicit limits; no direct network cargo test. A new published F attempt requires root allocation and the usual fresh preparation/admission. F11 cannot be replayed. No historical closed matrices are requested.

## Preservation and review handoff

Supporting public pins/findings are in `.tmp/s54-block4au-f11-diagnosis-01`; the companion `.pins.json` records current before/after closure and sanitized evidence.533 source files,nine images,AS/AT four accepted documents,current navigation,index and HEAD were checked unchanged. Only this report,its manifest and fresh AU administrative artifacts were created.

**Raw command access expires with AU.** Raw before/after pins are separately scoped administrative files; future historical auditors must exclude their command targets before hashing/traversal. Sanitized findings and comparison hashes suffice for architectural review. Do not rerun inspect.py/window.py/context.py after AU without a new explicit scope. Original AT failure evidence remains immutable and is not relabelled as success. Root must accept the repair contract before implementation; AS/AT/AU publication remains pending root review/instruction.
