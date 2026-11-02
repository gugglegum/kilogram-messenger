# P11A8a: bounded memory query and ledger core

2026-11-02. Baseline `546487d9c8001c6d110444a05acb3c0d1c381772`. A8 task/proposal/root-decision hashes were verified; all three documents accompany this candidate. Implemented only A8a query/ledger primitives and focused verification. **A8b Controller/Node/Pair/normal/relay lifecycle/status taps are not connected.** No production Rust/Cargo/dependency/unsafe changes, messenger/network/private fixture/store/key access, whole runtime/observer activation, memory-peak/whole-fit claim, commit or push.

## Implementation

Seven source/fixture paths: root test-module declaration, two narrow A7 private transport/capture changes, three new memory/query/test modules and one benign resident fixture. Exact hashes and normalized Git blobs are in executor-review.json and the handoff manifest. Existing shared lifecycle files and product Stats/diagnostics remain unchanged. The module's staged dead-code allowance is explicit until A8b; no executable whole selector is added.

Ledger retains run identity/unique instance, bounded relative clock, original-profile/image hints, per-lifetime ordinal/PID, provisional then verified owner/profile/source/nonce, attempted/successful boundaries, samples, query intervals, failures, stop request and actual waited outcome. Foreign tokens, original-profile switches/overlap, duplicate/reused identities, invalid bounds/time, missing coverage, query failure and malformed/regressing Stats poison functional coverage. Original frozen setup can bind after wait; a finished ledger seals against further work. Failure still permits recording stop/cleanup outcomes without restoring PASS. Finish requires all registered lifetimes closed once, start/pre-stop success, verified binding and accepted current Stats where required.

First progress is born+5s; later progress attempts are at least5s from the previous attempt, no catch-up. The row-free due check returns false for not-due/closing/closed profiles without a query. At most36 lifetimes/two owners/2400s and1032 attempts; overflow fails, no trimming/reset. Pure tests reach the conservative1032 upper envelope, plus36 actual-positive-duration sub5s logical restarts with boundaries only. Counts are bookkeeping, not additional workload permission. Profiles retain sampled maxima, query cost/intervals, missed slots and conservative max gaps including closing tail. These are sampled coverage, never an unsampled peak claim.

Every Stats candidate uses actual version3/Ready/indexed/source schema and numeric/range checks before aggregation; native/commit regressions fail permanently even if a later candidate would be valid. One bounded pending aggregate per active owner retains first/last offsets and maxima; it cannot close successfully without the owned checkpoint. In A8a, real pending acceptance is tied to the actual query bracket; injected checkpoints are explicitly test-only. A8b will supply the reviewed shared status/lifecycle binding without per-message RAM queries. Reported Stats are observed high waters, not terminal values. Caps are384MiB sampled PrivateBytes,128KiB native serialization and192MiB reported commit app-buffer bound; existing code/storage/resource caps are not recast as RAM.

Query adapter requires the exact validated x64 System32 shell descriptor (regular/non-reparse path, actual size/SHA, no local shadows). Its closed .NET command uses only the actual owned PID, retains Process.Handle while reading StartTime/PrivateMemorySize64, and brackets with the concrete Child's live try_wait before/after. First valid reading fixes creation ticks; mismatches/death/reuse fail. Descriptor is rechecked after query. Query work ceiling2s is intersected with original A7 effective bound; cleanup keeps the existing reserve/outer Job, never a new absolute deadline.

A7 now has only two closed internal kinds: existing Status retains1MiB/copy/sanitizer/original-Value behavior; Memory uses4KiB per stream and exact numeric PID/ticks/bytes schema. Both use the same original sequence/create_new files, overlapped readers, known query-child owner, actual wait/EOF and safe error metadata. There is no arbitrary script/executor API, raw stderr echo, spool or blocking reader. All A7 status regression selectors passed; routing and public_fingerprints are unchanged.

## Fixture premises and actual checks

Immutable raw base `.tmp/s54-block3e-p11a8a-01/`. Fixture premises and12 validation/fixture inputs were frozen before the first test run. Resident is fixed bundled pwsh, SHA256362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139, with source-checked ready-file/30s-sleep script, null streams, no grandchildren/network/profiles. Memory query is actual System32 powershell.exe495616B/SHA25660c9b29843624dd8af6b6a7b26147753be6aeef65098b9a503cc50850afe410f. Application profile/source labels in this benign fixture are injected, not authenticated original S54 identity. Process PID/ticks/PrivateBytes are actual OS readings. Future whole-run resolver/no-shadow/command inventory binding remains mandatory; this is not its activation.

| Final check | Actual child exit | Seconds |
| --- | --- | --- |
| fmt-03 | 0 | 4.655 |
| clippy-03 | 0 | 11.319 |
| compile-01 | 0 | 98.615 |
| pure-01 | 0 | 49.025 |

Format -> strict Clippy -> stable compile-only -> isolated checks, Jobs2/BelowNormal/offline/locked/x64 verification-scale. Existing watchdogs compile1190/1200, Clippy590/600, format/tests230/240 seconds. All20 exact isolated selectors PASS/0fail/0ignored; six aggregate harnesses select0. Seven new tests cover cadence/closure, identities/late setup binding, exact limits/many short restarts, failed-not-zero/cleanup, every-candidate Stats validation, closed wire/shell rejection and one real resident/three-query case. Thirteen directly affected A7/deadline selectors verify reused transport/status behavior. No broad accepted-stage replay.

Real resident PID44740, query PIDs[10468, 61164, 70020], same creation ticks639291843866855497:

| Reason | Start ms | End ms | PrivateBytes |
| --- | --- | --- | --- |
| Start | 117 | 446 | 27353088 |
| Progress | 5117 | 5349 | 27652096 |
| BeforeStop | 5349 | 5594 | 27652096 |

First progress is at least5000ms after registered birth; costs are[329, 232, 245]ms, all<=2s. Target and all three queries actually exit/wait; query streams confirm both EOFs. The finalized/waited-target API call is deliberately rejected and creates no fourth query/log. Valid coverage was finalized before that expected negative; no failed OS query was relabelled as success. Exact closed command bytes are independently reconstructed from the pinned format literal and match the per-PID SHA in the case record. Three readings do not establish whole-run fit, whole-profile coverage or a physical peak.

Existing verified Job wrapper independently checked9 actual live harness process handles (one memory case/eight A7 synthetic cases), bound image/hash/start/attempt/nonce and granted permits before child spawn. Actual test wrapper returned0, work_expired=false, Job empty before/after cleanup. Inherited non-breakaway containment covers the resident/query children; JSON/marker alone is not proof. Eight A7 synthetic children plus one resident/three measurement children =12 local synthetic child launches, not messenger work. An additional stalled memory query was unnecessary: shared deadline/cleanup control flow is unchanged and the actual A7 stall/cap cases passed through that reused code.

Compile/run before/after inventories agree;501 units, all501 fresh at run,593 current compiler source inputs, six stable artifacts and independent frozen guard verified. Extra ps1/tool/job inputs are independently pinned because generic compiler enumeration omits arbitrary ps1. Only migration harness image changed: 18026496B/SHA256 `b2a82f5bdfd96362bc3e2707e440a19135d445d162f25477ec8373b49cbeeea4`. Nine other preserved compiler images and four field images remain unchanged; normal messenger/field/state binaries were not executed.

## Preservation and handoff

Thirteen preimages (3 sources/10 images), failed Clippy01's14 source/helper inputs and intermediate Clippy02's14 inputs remain pinned. First Clippy rejected a collapsible if; no functional test failed. After Clippy02 passed, final cadence due API/short-restart coverage was completed before the first compile; fmt03/Clippy03/compile01/pure01 are the final source. All failed/intermediate receipts are retained. No source/fixture/execution helper changed after the actual tests. Documentation generation is not a historical execution input and causes no retest.

Evidence archive reuses the existing receipt/helpers: one executor review, eight original command receipts/logs, actual memory case, compiler source inventory and frozen fixture/selection inputs. Full raw captures/expected manifest/test inventory, command files, EXE preimages and revisions stay at their pinned ignored paths. Raw logs are never rewritten for whitespace. Exact candidate manifest follows final archive/report generation and includes original A8 task, proposal and decision. Index empty; unrelated navigation/historical work preserved.

Reproduce in a fresh ignored base with updated immutable helper step/capture paths: format/lint, compile-only, wait for capture, freeze manifest/fixtures, run exact isolated selectors under the existing verified Job/rendezvous. The audit generator checks this retained baseline attempt, not a standalone whole-run benchmark. Stop at root A8a implementation review; A8b must still wire/audit setup/provisional lifetimes, all shared progress/stop/error paths, status candidates and the original whole-run clock/inventory. No commit/push before acceptance.
