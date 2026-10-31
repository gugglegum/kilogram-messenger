# Attempt02 cooperative cleanup: source/control review

2026-10-31. SOURCE ONLY under [accepted contract](P2-s53-exhausted-cold-a-01-attempt-02.timeout-cleanup-accepted.md), baseline **a20dcb3dea0b83d0b2cda07ad6b1b8d7de5f9078**. No control/test/compilation/private execution or allocation occurred.

## Minimal production delta

[Launcher](../../../scripts/invoke-migration-s53-cold-preservation.ps1):
- **Complete-ChildCleanup:755** waits/pumps using the original common start through51. Only a still-live retained child receives the single main-path fallback Kill; successful wait/reap remains required by52, EOF by54. Independent guardian behavior is unchanged.
- **Check-DiagnosticBindings:774 / Collect-FailedReports:813** collect closed-schema claim/P2/Parent only after actual reap/bothEOF. Verify D1/attempt/common clock through Read-Report; retained PID/UTC, claim→started and Parent→P2 raw SHA; exact source maps; P2 QPC/capture-time/Parent elapsed bounds; consistent remaining-handle facts. A claimed known cleanup additionally requires released Core inputs/zero owners/no unknown, actual child-report cleanup flags and complete/known wrapper observations. Process exit never substitutes for these reports.
- Missing/partial/malformed reports yield bounded secondary `diagnostics={state,cause}`; states not_collected/normal_validated/bound/unknown are closed by Validate-Report. Primary first failure/pre-cleanup reason stays unchanged. Bound means diagnostic linkage, **not** successful preservation or known Core cleanup.
- Collection finally closes all reopened public streams. Unknown closure invalidates bindings, sets control.Unknown and prevents cleanup success. Public handles still reserve before acquire within24, records within40. Worst record count27 (D1+18 sources+started+normal3+failed3+supervisor); old reads close before failed3, so ownership never exceeds existing22/24 peak.
- Normal success function Check-ChildReports, work45 Check, normal gate, final good expression, Parent cleanup/job/report code and independent guardians are byte-identical. Cleanup reap/late exit0 cannot promote timeout.

Outside launcher, only **one fixed attempt01→attempt02 literal each** changed: Core.ProductionAttempt and the production output path. Core/output full text inverse restores published files; no algorithm/ownership/fixture/deadline-helper change. Launcher binds attempt02 and updated closed Core/output SHA pins. No active attempt02 D1/E/receipt was created; old attempt01 documents remain immutable.

## Source/control pins

| File | Bytes | SHA256 |
| --- | ---: | --- |
| invoke-migration-s53-cold-preservation.ps1 | 76869 | 620753ecddd1ff1b1d07fa5d3868c8fdfa7170d7770f407165e112ade8551fad |
| migration-s53-cold-preservation.ps1 | 82999 | eb166bce9b53da51863b605a06041e7100aca0d053f21c2b2ece09cf191b259f |
| migration-s53-cold-output.ps1 | 21721 | ced08d88656e7bad142d1e23a826c42e7be4f75da8380c838167610504db7ae0 |
| C-s53-cold-timeout-controls-01-attempt-01.preflight.ps1 | 26131 | 3dbdc5c973fa287096ff794473af0ca057c3ec71e7074a5d70f98b8a00ecebac |

Shared deadline remains6038B/SHA256 `0c6b3cd61b8641756f9343c7e663cce59fabbb0d052a3d54ba07ad5898e89d45`. Its work thread excludes owner, whole job cutoff50 remains until Finish disarms it; independent Parent51 remains through reporting. Ordering45<50<51<52<54<55<57<59<60 is unchanged. No unchanged job-kill/post-Finish scenario rerun is requested.

## Exactly three dormant public controls

[Control source](C-s53-cold-timeout-controls-01-attempt-01.preflight.ps1) extracts **43 actual function definitions +7 AST fragments**, including normal loop/gate/final-good/catch, cleanup collector/binding validators, public IO and retained waits. It does not import the launcher entrypoint, Core, owner initializers, private paths or process enumeration. Three dummy pwsh children, sequential, fixed case names only:

| Case | Planned assertions, not execution results |
| --- | --- |
| normal | Child targets age1; actual normal loop/gate/wait/exit0/EOF and final-good expression pass. Full preservation/normal Check-ChildReports is not exercised; its unchanged source is separately verified. |
| grace | Work cutoff2, child writes actual-schema failed claim/P2/Parent at age3 then exits0. Actual failed collector binds hashes/identity/clock, returns bound; Core=null/cleanup_known=false remains unknown. Timeout/failed gate stays sticky despite exit0. |
| forced | Infinite dummy wait; main fallback only after grace5, real cleanup wait/reap6/EOF7, nonzero exit. Missing P2 gives unknown diagnostics, never success. |

Five pure data refusals inside grace cover wrong PID, QPC, P2 hash, source hash and extra field, requiring exact InvalidOperationException/HResult/code. No extra processes.

Closed test mappings: work45→2; diagnostic Parent50→4; grace51→5; reap52→6; drain54→7. Runtime case clock starts **before** context/spawn, so startup is included. Existing started schema fields45/60 and permission_consumed are fixtures, not grants. No other validation/body change; every mapped text is inverted byte-for-byte. Actual production guardian body is embedded only to arm **before source IO/bootstrap**, then compared to its mapped AST body: test root child20/self29, single root clock/outer30. No Add-Type/native compilation.

Prospective one C **work20/whole30/lane1/retry0**, unallocated. Expected case targets total1+3+5=9s; conservative successful case bounds<2+5+7=14s leave about3s for source pins/AST/runspace/fixture preparation and2s for data assertions/serialization, about19s. These are planning estimates, not measured results or hard scheduler promises. Root guardian stops active child at20/self29; overrun fails without retry. No 13k-file fixture/real-size comparison matrix.

All outputs are fixed CreateNew under the public evidence directory with prefix **C-s53-cold-timeout-controls-01-attempt-01**: child.ps1, cases.json and normal/grace/forced context/started/claim/p2/parent JSON names (17-name whitelist, at most64KiB each). No directory creation. Contexts<=16KiB, generated child **27746B**,31 actual public definitions; parsed only in memory. Failed fixtures are retained, never repaired/reused.

Public reader peak reservations expected **1/3/2** for normal/grace/forced, remaining0 and every registered managed handle closed. Forced peak2 includes the reserved failing P2 open. Managed closure does not independently prove native CloseHandle success. Case source uses actual Read-Report/validator/wait functions, not replacement predicates.

## Focused initialization audit after root review

Root identified two missing prerequisites in the dormant harness; both are fixed without altering production functions. Every scenario creates a fresh waitTrace List before waits and resets child/ID/UTC/retained-handle state. After successful Start(), it captures actual Id, StartTime and Handle.ToInt64() from the same retained Process before New-Drain/sampling. The production Sample-Child equality check is unchanged.

A one-off AST variable inventory of all43 selected functions found **44 external names** after excluding parameters/local assignments/loop variables/automatic constants. Manual ordering review covers those names plus five decision-fragment inputs (**49 slots**); no reusable analyzer or extra test scenario was added.

| Prerequisite group | Initialization before first reachable read |
| --- | --- |
| Eight public path variables: entry/corePath/outputPath/rootNative/deadlinePath/fixedShell/taskRoot/e | Host header and childSetup before imported definitions are called. |
| StartTimestamp/attempt/DecisionSha256/cleanupReserve/cleanupLimit/guard | Fresh scenario setup before started writer/loop; child gets fixed context and initial cleanup policy before Await-Start. |
| sourcePins/self/startedHash | Host pins validated and actual current Process retained; startedHash assigned by actual Write-Report before loop/diagnostic binding. Child uses its own actual Process and Await-Start result; it does not import the host binding collector. |
| child/childId/childStartedUtc/retainedProcessHandle | Reset per case; actual Start at170, real capture171, loop178. No stale or dummy identity used by Sample-Child. |
| reaped/childExit/exitQpc/reapQpc/eofQpc/waitTrace | Reset per case; fresh list at160, before Start170 and first wait through loop178. Script-scoped updates and setup share the harness script scope. |
| out/err | Null before spawn, actual stream drains assigned immediately after retained identity and before loop. |
| metrics/nextSampleQpc | Actual production metrics initializer plus fresh zero sample clock before loop. |
| completion/normalStage | Actual production completion initializer and explicit stage before loop/first-failure/Record-Wait. |
| control | Synchronized object before guardian; actual Process registered before Start, released only after real reap. |
| outputOwner/parentGuards/preparedHandles/ownerDisposed/guardsDisposed/coreFinal | Explicit neutral initialization in both scenario and child setup. No owner/guard/Core is constructed; null facts branches remain the fixture's only path. |
| normalCandidate/registeredChildReleased/stdioDisposed/cleanupKillAttempted/cleanupFirstCause | Candidate false before loop; actual disposal and cleanup-state results assigned before evaluating the extracted final-good expression. |

Guardian's five parameters are bound before BeginInvoke. Failure fragment runs only within catch, with actual $_; its local failureCause/failure assignments precede reads. Locals in sampled API/error paths and conditional wait branches were read in source order: counters/API selector are initialized before try, and throwing refusal branches do not continue into missing locals. Child Kind is the supplied closed parameter; context/sourcePins/public lists/self/neutral facts are initialized before Await-Start or record writing.

Static AST order checks and child assembly parse passed (6dd63d). The initialization-only changes invert exactly to the previously reviewed25725B preflight/SHA8f5c54b775a280fdfc235cafa9884bd8fbba556b100f8c05c4fd4011efac3d38. Generated child now27746B; only neutral facts variables were added to its setup. No control execution or semantic compilation; production hashes and50 function/fragment mappings stay unchanged.

## Static results / stop gate

Four PS files plus generated child,43 mapped definitions and7 fragments parse with0 errors. All50 inverses pass. Aggregate original function text SHA256 **4286c7c287382b0d6baf07563bcb14769a7de0700dccd844ef74ebebc257c79a**, mapped **a48b48d7f42566d3033b5cb022e604289ab9886ba3790e2dd7e6e306bb937475**. Generated child SHA256 **8e0a6ee95f10155ceb5fc82508c16e0969284c699ae642e4fe1570dfebb23e2f**. Production guardian AST SHA256 **c238bde453f0702da3a8fc28471c98e2e95a13ec2b4de9939f6260bafa2090ee** is unchanged; early test copy is exact after its two fixed limits. Core/output identity inverse and source diff whitespace pass. No semantic compilation or runtime result is inferred.

The original45s comparison phase/cause remains unobserved. Existing failed P2 Core facts are the first diagnostic source; no checkpoint telemetry added. Standing test-data authorization applies to later reviewed runs, but this slice allocates/executes nothing. No private existence/ACL/inventory read, process probe, E/D1 activation, ledger/nav/commit/push. Return these pins and **STOP for root source review**.
