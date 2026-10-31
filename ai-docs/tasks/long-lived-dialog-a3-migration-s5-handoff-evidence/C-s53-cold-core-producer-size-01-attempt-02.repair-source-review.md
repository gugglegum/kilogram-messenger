# C2-R attempt02: bounded completion repair, source review

2026-10-31. **Source only; STOP for focused review.** Follows [attempt01 failed review](C-s53-cold-core-producer-size-01-attempt-01.failed-review.md). Attempt01 remains consumed/overall failed; its accepted worker evidence does not replace a new whole-attempt result. No runtime or allocation here.

| Source | Bytes | SHA256 |
| --- | ---: | --- |
| [worker](C-s53-cold-core-producer-size-01-attempt-02.preflight.ps1) |43583 |1847f1ea879cb3e3cb06db7b7f2761bc8045d8561b918daa4f931fa61c48c2b0 |
| [supervisor](C-s53-cold-core-producer-size-01-attempt-02.watchdog.ps1) |47280 |9d36eb48fa86e71759098f2e4bdafaf6c34377490bf42fac15dcd89ba1363ae3 |
| [helper](C-s53-cold-core-producer-size-01-attempt-02.fixture-support.ps1) |56156 |adb6f2ffe7cc45d2d4eee41bf7e0d97114a7edc9a4213bf5d3af8dcf56e3c039 |
| [Core](../../../scripts/migration-s53-cold-preservation.ps1) |81937 |ed56b2c3b632d81b8a775db45da93bd6657aef6ca47cd1d81f237605c89b2f43 |
| [output](../../../scripts/migration-s53-cold-output.ps1) |16100 |40710b0a2993e768a0f0b52d9949947258a3d44281f9f1aa7d10417efb7571c2 |

## Narrow change and preserved evidence

Normal completion now requires a real successful same-retained-Process WaitForExit return plus both observed EOFs, inside the original work420. HasExited only records an indication and stops metrics; indication/EOF cannot break the loop by themselves. Wait-ForReap requests1..20ms, clamped to floor((absoluteLimit-Age)*1000). False leaves state unchanged; Record-Wait alone changes reaped from false to true, once. Once true, neither normal nor cleanup waits again; no zero-time query can erase success. Read ExitCode only after successful reap, reject nonzero explicitly. A reaped child with missing EOF is pumped with bounded<=5ms sleeps until EOF or the same420 cutoff.

Normal completion gate additionally requires nonzero indication/reap/EOF QPCs within the common start/work interval, exit0, no force/unknown; observer/worker schema/Core/receipt/source validation and final native0 remain mandatory. The good predicate still requires normalCandidate, observed completion gate, no first failure, known reap/EOF/cleanup/worker0 and all prior safeguards. Cleanup cannot promote a failed normal path.

The mechanism is supported by [.NET v10 ProcessManager.HasExited](https://github.com/dotnet/runtime/blob/v10.0.0/src/libraries/System.Diagnostics.Process/src/System/Diagnostics/ProcessManager.Windows.cs) (exit-code-first compatibility path) versus [Process.WaitForExitCore](https://github.com/dotnet/runtime/blob/v10.0.0/src/libraries/System.Diagnostics.Process/src/System/Diagnostics/Process.Windows.cs) (process signal wait). The same retained process handle remains owned throughout. This is supporting source evidence; neither installed10.0.11 binaries nor attempt01's missing initial wait result were instrumented.

Fresh fixed family/attempt02 and producer-size-r02 are isolated from consumed01. Worker/helper changes are binding-only: namespace/initializer R02, fixed root/attempt/classification, new inventory pin, closed output factory and dependency pins/lengths. Core adds only the r02 closed selector/exact binding/root, preserving r01 and production algorithms/caps. Output adds only a closed CreateProducerSizeR02 factory/private checked constructor selecting the02 literal; old constructors/factories/paths/checks stay intact. No arbitrary path surface. The legacy reflection wrapper remains separately open before private; direct binding/pending-reference cleanup is unchanged.

Core/output inverse transformations reproduce the frozen01 source exactly. Helper binding inverse reproduces accepted R1; worker is exactly its01 source under the declared binding substitution. No setup-probe/performance change or helper redesign. Fresh helper gets the same complete lifecycle and independent validators.

Original01 W/S/helper, frozen Core/output snapshots, started/producer/watchdog reports and run note were rehashed before/after: all9 byte counts/hashes unchanged. HEAD remains0cbc322fe3ae06632af7ea4c1538f19b060d22ac/index empty. Other01 notes were not edited.

## Separate observations and stable failure evidence

Supervisor exports exit_indication_qpc, reap_observed_qpc and both_eof_qpc separately. Note-ExitIndication records the first confirmed observation only, including already accepted sampler exit/race branches or a successful signal wait; its basis is explicit. Pump records first actual both-EOF observation. Observer predicates/identity/API failure policy/interval/scratch/limits are unchanged; normalizing only these timestamp notifications reproduces its prior body.

Completion records normal_gate and a separate pre_cleanup_gate, copied before kill/wait/drain/disposal in finally. They preserve indication/reap/exit code/EOF/force/unknown/observer flags, stage and first reason. pre_cleanup_first_reason is captured before cleanup; Set-FirstReason only fills null. Cleanup progress updates final observations but cannot overwrite these snapshots. Final reporting/guardian failures also preserve the first reason and emit bounded aggregate diagnostics if the earlier report is already immutable.

Wait evidence is bounded: normal/cleanup attempt/false/true counters, last result/QPC, first successful reap stage, and at most8 transition records (at most6 kinds reachable): first normal wait, first normal false after indication, first normal false after both EOF, normal reap success, first cleanup wait, cleanup reap success. Each includes requested milliseconds, before/after QPC and observed gate values. No per-poll log or unbounded collection.

### Static source trace, not executed tests

| Input sequence | Actual source path / required result |
| --- | --- |
| First normal wait false, later true, EOF present |False increments counters only; if indication/EOF already observed, their first-false transitions are retained. Loop continues. True sets reaped/reapQPC once, then reads exit code. No later wait resets it; exit0+EOF proceeds to unchanged observer/report/source gates. |
| Never signaled, even if indication+EOF true |No indication/EOF shortcut. Wait false repeats within420. Common Check or remaining-time guard refuses; first reason unreaped_work_timeout, pre-cleanup reaped=false/child_exit=null. Cleanup may later reap, but normalCandidate stays false and reason/snapshot remain. |
| True with nonzero exit |Record-Wait observes actual reap, then reads code; normal loop refuses child_exit_nonzero before acceptance. Snapshot retains code/reap, later cleanup cannot relabel success. |
| True/exit0, missing EOF |No additional wait once reaped. Pump/sleep remains under420; no EOF by cutoff gives eof_work_timeout. Later cleanup EOF cannot replace the failed normal gate. |
| Force/unknown |Immediate completion_forced_or_unknown inside loop, or failed explicit gate if flags change at gate boundary. Original final no-force/no-unknown predicate remains; no passing path. |
| Worker report/schema/receipt assertion failure |Completion can be healthy, but stage worker_report records worker_report_validation_failed plus original exception/code; no normalCandidate. |
| Source-after integrity failure |Stage source_after records source_integrity_failed, preserving original source_pin cause. No success. |

All checks remain on the same attempt start. Cleanup targets450/460/470, hard guardian455/475 and outer480 requirement unchanged; no reset/extension. Normal work includes bootstrap/compiler/setup/verification/report validation. New bounded waits do not open a new budget. No native reproduction, fake process fixture or negative matrix was run.

## Fresh-root oracle, scope and allocation

Two independent public in-memory algorithms regenerated the changed-root compact producer JSON and agree byte-for-byte. Inventory **3465145B/SHA256 6962b9986b7055d7bceb5e0ea15bfeb70ebec43c3db4d7de2cf0e5cbc693ea12**. Payload map/H remains2af20fb9e80ec86463e8430a54ab5700b1cc7f851be23441577a00fb87fa9fcf:6956 rows (6954 nonempty+2 exact zero locks)/45597399B per mirror,13915 physical files/94659943B. New root has the same width:26 fixed paths/6988 interned entries/path563128/records18537840; same dirs/IDs/owner counters and peak13938. Signed-FILETIME bounds remain1651/row comparison,637/row namespace, conservative11556901 aggregate.

All accepted worker/Core/receipts/observer predicates stay: fresh5 types/once initializers, real6 fixture locks, captured13934 inputs+E before hashing/13912 notifications, four passes182389596B/fiveH, output ancestry held/rechecked, independent four streamed receipts/provisional2/3 versus final0/4, actual helper41749/21/32/41784/41753 cleanup/no remaining or unknown, source-after integrity and actual native exit/EOF. Earlier worker positive evidence supplies no bypass.

Future canonical prefix02.root-allocation.json keeps exact24-field schema1 and all type/duplicate/output/cap checks. Binding changes only: attempt/family02, fixture_cases=[producer-size-r02], classification=root-approved-cold-core-producer-size-c2-r02, operation_set=cold-producer-size-one-only-r02, current W/S/five-source pins. Six fixed dependencies remain Core/output/helper pins above plus frozen Root25596/a3eefcc9dd4da8bdd9141d8731fc60feaf496d5d357047c0283b53dd8e6c3382, deadline6038/0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45, shell301368/362a356ce7f0940ec74f73a8fc2c990a2cc24a38a11c90bbd8eca947110ad139. Self/paired script hashes are allocation-only, no circular self-pin. No allocation file is created.

Work420/whole480/lane1/retry0, exact started/producer-size/watchdog outputs65536 each/196608 aggregate unchanged. All current sources fit131072/524288 source caps. Worker report schema/value-width bound remains<=65200; new bounded completion/trace metadata adds<6144B to the earlier normal supervisor<16384B, still below65536. Actual serialized sizes remain checked before write. JSON timestamp decode stays -DateKind String. Public evidence remains aggregate-only.

## Validation and stop

ParseFile0 errors: worker28/supervisor31/Core3/output3/helper1; all22 shared functions byte-identical. Supervisor has one reaped=false initialization and one conditional true assignment, no WaitForExit(0); all real normal/cleanup waits route through the bounded recorder. Core/output/helper C# masked delimiters balance; this is not compilation/ABI/native proof. Inverse source checks, independent inventory arithmetic, original01 frozen hashes, LF/UTF-8/no-BOM/whitespace/source pins are checked. No tests mirroring implementation were added.

Ledger untouched: S5.3 C1775.0684909/F1562.2882567/P0; combined C2362.5171018/F6278.6084056/P0; ceilings2150/1642/0; active0/inactiveC374.9315091/F79.7117433/P0.420/480 prospective only; root controls any new allocation.

Only fresh02 W/S/helper/review and minimal live Core/output mappings written. No compile/dot-source/helper invocation/fixture/event/.tmp/private/original/network/Rust runtime, allocation/ledger/navigation edit, commit/push or S5.4. Callback and **STOP**; new whole-attempt success remains unmeasured.

