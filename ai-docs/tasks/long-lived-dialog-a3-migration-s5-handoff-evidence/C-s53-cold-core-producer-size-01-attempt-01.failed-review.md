# C2-R attempt01 failed closure and minimal completion repair

2026-10-31. OVERALL FAILED, consumed, no retry in place. Baseline0cbc322fe3ae06632af7ea4c1538f19b060d22ac. Worker positive evidence is retained, not promoted to whole-attempt acceptance.

Actual app exec-a8b43f42-9556-46cf-9ea4-6578a9d77467 confirms approved wrapper/allocation, outer exit1,138136ms, child_return diagnostic. One session74456/five chunks. Request2026-10-31T05:25:45.916Z to final05:28:06.341Z=140.425s. Root219 data assertions confirm worker positive facts and outer failed state. Rootd53d40 rehashed8 sources and4 report/run files, HEAD/index, then froze exact current Core81411 and output15663 in .core-source.ps1/.output-source.ps1. Existing W/S/helper/reports remain immutable.

Worker14096 exit0: full6956rows/45597399B per mirror,6locks,13912 capture notifications/13934inputs+E before hashing; fourpasses182389596B/five expected H, final0/peak13938, independent4receipts/10571092B, exact helper cleanup41749files/21dirs/32enums/41784streamchecks/41753searches, output ancestry/mask/transfer/cleanup all pass. Core comparison10.3468519s, preparation122.9475081s; fixture aggregate probes7.5894112s are included, not added. No whole-positive claim because parent never finished report validation.

Parent reports child_return; later cleanup observes child0/reaped/EOF/known cleanup/no force/no kill. Publicinputs9 and empty source_after confirm failure before worker report open (expected10). Relevant source locations: Start success gate line414 or final one-shot WaitForExit(0)/combined gate427-428. Worker demonstrably started; monotonic Forced/Unknown remain false. Initial reap and initial child-exit values were not independently recorded. One-shot reap failure is the supported explanation, not an instrumented branch observation.

Official .NET v10.0.0 Windows source distinguishes ProcessManager.HasExited (exit-code-first compatibility path) from Process.WaitForExitCore (wait for kernel process signal). That explains why indication/pipe EOF must not be assumed to imply an immediately true zero-time wait. This is source-level supporting evidence, not a claim that exact installed10.0.11 binaries or the unrecorded initial value were instrumented:
https://github.com/dotnet/runtime/blob/v10.0.0/src/libraries/System.Diagnostics.Process/src/System/Diagnostics/ProcessManager.Windows.cs
https://github.com/dotnet/runtime/blob/v10.0.0/src/libraries/System.Diagnostics.Process/src/System/Diagnostics/Process.Windows.cs

C480 CLOSED charge140.425/release339.575. S5.3 C1775.0684909/F1562.2882567/P0; combinedC2362.5171018/F6278.6084056/P0; ceilings2150/1642/0; active0/inactiveC374.9315091/F79.7117433/P0. No private grant.

## Accepted narrow repair contract; source-only attempt02

Fresh prefix C-s53-cold-core-producer-size-01-attempt-02/fixed root, closed selector producer-size-r02 and matching output binding; copied fresh helper/type binding only where necessary. Preserve original mappings and attempt01 sources/reports/notes; snapshots already created by root. Regenerate inventory pin for changed absolute root; relative payload/H/counts/resource model unchanged only if independently confirmed.

The substantive supervisor change: continue bounded normal wait/drain loop until actual same retained Process.WaitForExit(...) returns true AND both pipes EOF. HasExited only indicates exit and stops sampling; it cannot substitute for kernel reap. Preserve a successful reap observation monotonically; no subsequent zero-time query may erase it. Read/check exit code after actual reap. All checks/pumps/finite waits remain within original work420/common start, no deadline reset or silent extension. Existing cleanup450/460/470 and guardian455/475 remain.

Record first exit indication, first real successful reap, EOF QPC separately; preserve pre-cleanup normal-gate values and first failure reason so finally cannot overwrite diagnostic evidence. Normal gate still requires actual exit0/reaped/bothEOF/no force/unknown/observer failure, worker report/schema/receipt proof/source rehash and final native0. Distinguish nonzero child exit, unreaped timeout and report/assertion failures. No skip/reclassification of failed attempts.

Source-review must trace false-first-wait then true, never-signaled-to-deadline, nonzero exit, missing EOF and force/unknown cases against the actual loop; no native reproduction required now. This is a small repair, not a new test framework/matrix. Preserve all accepted Core/receipt/helper/observer predicates and remaining timing. Do not mix setup-probe/performance optimization into this correction; measured overhead can be considered later.

Allowed edits: fresh attempt02 W/S/helper and short repair-source-review; minimum additional closed Core/output mapping. No runtime/compile/dot-source/fixture/event/.tmp/private/network/Rust, allocation/ledger/navigation edits, commit/push or S5.4. Prospective420/480 remains inactive. Callback and STOP for focused delta review, then root may allocate one new run. Worker component evidence from01 remains valid as evidence but does not replace new whole-attempt acceptance.
