# S4 review 2 — measured repair and one bounded full verification run

2026-10-25. Root/orchestrator decision after [review1](long-lived-dialog-a3-runtime-s4-implementation-review-1.md).
**Implementation is still not accepted. No commit/push is authorized here.**

## Measured outcome

Root independently read both short diagnostic results, their actual PASS lines,
the numerical attribution and [exact narrow delta](long-lived-dialog-a3-runtime-s4-evidence/owner-root-counter-r1-diff.json).
The diagnostic controller remained byte-identical. Each fixture exercised98 Text
and4 genesis; these are diagnostics, not substitutes for P.

| Measure | Before | After |
|---|---:|---:|
| Data elapsed, seconds |58.240759|57.562526|
| Data process CPU sum, seconds |88.343750|70.031250|
| Full diagnostic controller, seconds |75.553578|76.568639|
| Diagnostic runner, seconds |119.4238763|122.8123687|

Removing the repeated installed-record read saves about20.7% CPU, but only about1.2%
data elapsed. It is a valid resource reduction, **not a demonstrated repair of the
original full-P time budget**. The remaining measured costs include ordinary owner
jobs, authenticated reads and closed durable operations. Nested timers are not
added as independent wall time; no single unmeasured cause is asserted.

Root inspected guard preservation and the R1 correction. Candidate production CLI
SHA256 is `d5d8c8c118925dfde67e37fdb32aa5a120293cde258e93843dd1939fddb15560`,
identified by [image record](long-lived-dialog-a3-runtime-s4-evidence/diagnostic-after-image-1.json)
and its source manifest. Actual F/route verification remains necessary.

## Explicit, narrowly scoped verification exception

The frozen original contract section11.3 requires P runner<=2400s. P4 failed its
2350s controller before the substantive relay/final-history tail; its result is
unchanged. It reached3265 of3288 committed packet operations and had112 outstanding
C1 receipt debts. The completed640-row history read took8.237s; the omitted work
includes further catch-up, rebinds, both peers' final histories and old-cursor checks.
There is insufficient evidence that the narrow CPU repair alone makes that tail
fit the original ceiling.

Root therefore authorizes **one new complete S4 P attempt with runner<=3000s and
controller2950s**, leaving runner overhead margin. This is an explicit one-run
exception to section11.3, not a reinterpretation of2400s or a retroactive PASS.
If the new run exceeds2400s but passes within3000s, final acceptance/reporting must
state that the original verification-time target remains unmet and that this
specific exception was used. If it finishes within2400s, record the actual result
without claiming an unmeasured general performance improvement.

The exception permits the remaining substantive checks to execute on the reviewed
candidate. It does not establish release-build throughput or MVP usability. Further
performance work can use the saved measurements independently of this functional
stage. No additional deep storage/crypto/ACK changes are authorized by this decision.

All other requirements stay exact: verification/unoptimized/locked/offline,
Jobs12/BelowNormal, P's1640 Text/4 genesis/3288 committed packet calls, every restart,
DH/late-tail/offline/full-pending/repair/C2/relay/B-only/history assertion, two-context
64/64 fairness, packet batch<=4, memory/capacity bounds, policy checks and atomicity.
TLS15s, admission30s, stream round15s and lease<=300s are unchanged. No prefix/suffix
assembly, count reduction, release profile, synthetic crypto or quiet retry.

F remains<=1200s; selected components remain<=600s. Both short diagnostics have their
own explicitly authorized<=300s investigative bounds and belong in the development
ledger; they do not replace required component/P/F gates or silently consume/reset
their final contributing budgets. Preserve every earlier source/log/result file.

## Executor instructions and remaining acceptance

Finish already-running guard/R1 checks. If they pass, freeze the reviewed production
candidate, change only P's aggregate controller/declared budget as above, record
new source and executable hashes, and run one full P on a fresh isolated fixture.
Run the complete F selection including R1 on that same production image, within
its unchanged budget. Independent F/static work may run alongside P using already
prepared stable executable paths and separate fixtures; never overwrite a live EXE.

On any new failure, retain it and report; do not silently rerun, increase another
deadline or change product code during the frozen runs. On success, return exact
P/F/component/static evidence, source correspondence, limits and an explicit candidate
publication allowlist. Send a substantive callback to the root chat, including if
the run fails or needs attention. Root will review the implementation and final
diff before separately authorizing commit/push and verifying the remote commit.
