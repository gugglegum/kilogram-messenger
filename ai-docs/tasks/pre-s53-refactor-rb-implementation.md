# R-B: durable assertion snapshot before the tail — implementation submission

2026-10-26. Submitted for root implementation acceptance against published
`9016b578bed8b8b5e43dd0e2e94edc376804e4f0`, sole parent
`eafd0f873eb31b64377e8e65e06bc84a911f6dc6`. The
[assignment](pre-s53-refactor-rb-task.md) and
[source clearance](pre-s53-refactor-rb-source-review.md) control this cut.
No R-B commit/push or S5.3 work has begun.

## Result and boundary

The test-only `attempt_result::persist_before_tail` receives an explicit
case/run/output/provenance envelope. It creates a new file, writes pretty JSON,
flushes, syncs and explicitly closes the writer before calling the fallible tail.
Open/write/flush/sync errors prevent the tail. A tail error propagates unchanged;
success returns the actual tail result. Existing files are never overwritten.
There is no retry, recovery framework or crash-atomic publication claim.

The existing exhausted controller now captures exactly its22 observations after
successful `checkpoint.info`, persists `assertions-before-tail.json` in the
controller output area, then calls its original preservation tail. The final
parent JSON reuses that same value and appends only the actual `checkpoint`,
`phase_trace` and `phase_records` after the tail. Both sources retain their public
projections; no private SourceNative object is serialized.

[Source audit](pre-s53-refactor-rb-evidence/S-rb-source-audit.json) compares the
original25-field aggregate with22 before-tail expressions plus3 post-tail fields.
The entire prefix through `checkpoint.info`, later phase journal and terminal
phase behavior match the published controller. Its case03 guard, functional
body,760-second allowance, original preservation call, phase order and cleanup
remain unchanged. The module file adds only the helper declaration.

The snapshot states `functional_assertions_completed` and
`whole_attempt_verdict: not_established`. The helper reads no environment and
discovers no evidence. The legacy controller explicitly uses
`unavailable_legacy_runner`, retaining its old caller contract. Managed component
tests read the newly saved actual inventory, use its path/identity and compute
its actual BLAKE3 digest. Partial or invalid supplied bindings fail; only the
all-absent standalone mode uses `opaque_unbound_standalone_fixture`.

No product Rust source, format, protocol, Native/DB algorithm or CLI behavior
changed. The historical runner, preservation helper, obligations module, owned-job
deadline helper, stable runner, published R-A adapters and all92 R-A publication
paths remain unchanged.

## Actual verification

One executable attempt was authorized and performed. NativeToken was requested
through ordinary `require_escalated` review under the standing human permission;
the adapter records the declared mode rather than claiming token introspection.
Only opaque temporary data was used, without Account/Session/Native/DB/network
fixtures. The stable wrapper launches its four fixed harness paths with the same
prefix filter; three harnesses execute zero tests and the integration harness
executes exactly the intended three tests.

| Attempt | Whole outcome | Kernel and observed assertions |
|---|---|---|
| C-result-lifetime-01, work45/whole60 | **PASS, process exit0,17.7590631s** | Exact three names once each;3 passed/0 failed in0.08s |

The failure-tail test exclusively reopens and reads the exact closed file inside
the tail, then deliberately fails. It verifies the original typed error and
unchanged snapshot bytes. The collision/error test preserves existing bytes and
uses a real file-as-parent invalid destination, with no tail call in either case.
The success test verifies the actual returned tail value and the snapshot's
limited classification without a whole verdict.

The [case record](pre-s53-refactor-rb-evidence/C-result-lifetime-01.json),
[untrimmed log](pre-s53-refactor-rb-evidence/C-result-lifetime-01.log), started
marker and actual input inventory are retained. Stable-wrapper exit0 and its
completion marker, exact3/0 summary, unchanged sources/images, unexpired work
deadline and cleanup/query0 all passed. The outer tool independently observed
process exit0. A read-only post-run process query found no Kilogram processes.

The frozen [binding](pre-s53-refactor-rb-evidence/R-B-pre-run-binding.json) is
164525 bytes, SHA256
`0a5bcf701ddd31acaf2c83a6f53fc921adf3f7aa138f4320879b31081479aab1`.
Root independently checked all775 inputs and7 images before clearance. The adapter
durably created the actual pre-run inventory before the harness and supplied its
absolute path/identity to the tests. Its SHA256 stayed
`8f849a6d894244916a59252e6ca5dd90040365dc5b90e8390a35c760347f386a`.
Final source/image equality was checked before sealing the whole result.

The owned-job watchdog remained armed through result/ledger writes. Explicit
PSCustomObject rows and scalar totals retain the actual charge; absent, empty and
nonempty process-environment states are restored using the reviewed helper.
The six reused PowerShell functions are exact source copies from accepted R-A.
The separate adapters add no general runner framework or legacy-runner contract.
The recorded17.7590631s is sampled after the initial result/ledger publication,
immediately before the final terminal writes and watchdog Finish. Those final
operations have no separately measured duration; the late work check and owned-job
deadline remain armed through them. The numeric sample is not an exact duration
to the last process instruction. The outer process exit0 is separately observed.

## Static lane and images

All static commands were serial, locked/offline, Jobs12/BelowNormal. No source
edits occurred during source-bound commands. Compile-only staging was last.

| Static record | Exit | Seconds |
|---|---:|---:|
| S-rb-fmt-01 |0|4.8210577|
| S-rb-fmt-check-01 |0|5.1199768|
| S-rb-clippy-01, CLI all-targets `-- -D warnings` |0|4.7127142|
| S-rb-compile-01, stable compile-only |0|25.7224967|

Each static record retains758 before/after source rows and complete unchanged
product-image inventories; formatting is the declared source-changing action.
The final integration harness is14094848 bytes, SHA256
`a193ef59c9755589b9a953e34bb7233913d4d8821904353be01500747477eeab`.
The other three harnesses and published R-A product/control/frozen images are
unchanged. Before staging, all four previously executed harness contents were
preserved byte-for-byte in `.tmp/pre-s53-result-baseline-harnesses`; the public
`S-rb-baseline-harnesses.json` records their hashes. Archived images were not run.
No product CLI build or invocation was needed. The unchanged stable runner emitted
its existing target-cache size warning; no pruning or system-setting change ran.

## Accounting, reuse and limits

The independent [R-B ledger](pre-s53-refactor-rb-evidence/contributing-cases.json)
contains one complete case: **C17.7590631/F0/P0**, within60/0/0. There was no retry
or F/P spending. Closed R-A remains C118.3327995/F148.7417507/P0, including
conservative C60; its revised150/210/0 allocation is not transferred to R-B.
Combined refactor charge is C136.0918626/F148.7417507/P0 under the allocated
210/210/0; the C total includes the same conservative60. Static-only checks are
recorded separately, not treated as executable-case consumption.

[Reuse assessment](pre-s53-refactor-rb-evidence/S-rb-reuse-assessment.json)
binds the final source, actual case, unchanged prior evidence and narrow scope.
The three component tests establish helper lifecycle behavior. Source review
establishes the controller's22+3 integration seam. They do not establish a new
whole exhausted-controller PASS or certify a future S5.3 run. No long controller,
511 generation,100k workload, exhausted checkpoint/backup or original Native
state was run, traversed, restored or opened during R-B. Boundary03 remains whole
FAIL649.592232s and its missing detailed parent result remains missing.

The three navigation files now link the actual S5.2/R-A publications, this R-B
submission and the required order before S5.3. Historical submissions, decisions,
raw outcomes and ledgers retain their original status and bytes. The queued
S5.3 task remains unchanged, SHA256
`75cbc0f9950971820beffb08edbd7c6ccb4465d8be44aea331c51687f1807f7c`,
and outside this candidate.

## Review handoff

[Exact candidate](pre-s53-refactor-rb-candidate-files.json) lists raw sizes/SHA256
and normalized Git blobs for source, adapters, task/source clearance, new evidence,
report and navigation. The manifest excludes its own digest to avoid a cycle;
its exact digest is supplied in the callback. Private `.tmp`, target outputs and
the queued S5.3 task are excluded. Index and baseline HEAD remain unchanged.

Stop for root implementation acceptance. A separate explicit publication
assignment follows acceptance of this exact diff. S5.3 remains queued until
root independently verifies R-B publication.
