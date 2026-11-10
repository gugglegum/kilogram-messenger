# Block4BB: assignable F held-ACK release contract

2026-11-10; HEAD `447451e819d3d0b97f1a55ecd0877a1571f48fa4`. Design only. Recommend the seven-file fixture/diagnostic change below, scheduling-only checks plus one separately allocated next full F. No runtime7/transport change or new process fixture. BA's actual negotiated timeout, closure/drop timing and guaranteed benefit of60s remain unknown. F01–F12 consumed; S5.3 closed, S5.4 open, full F/WholeP unproved.

## Exact clocks and budget policy

`Controls::new` writes `max_pause_ms=120000`; it does not start a timer (`migration_s54_control.rs:93–109`). `arm_ack` enables the targeted outgoing ACK while OD1 ACK remains suppressed, then enables OD1 (`feasibility.rs:232–253`). The timer starts **when `Observer::pause` constructs that particular Pause**, after loading the applicable schedule (`runtime7/verification.rs:132–150`). It stores an absolute `Instant +120s`. `Pause::wait_until` reloads controls but never renews that deadline (`:269–284`). Rewriting the same controls or rearming a decoder does not extend an existing Pause. Replay after old closure constructs a fresh Pause with a fresh120s lifetime; B remains warm.

Capture `A=Instant::now()` immediately before the first `arm_ack`. Verify hold was previously false, disable_ack true, CatchUp/B/from7 and max_pause_ms120000; then the genuine first Pause cannot predate A. Thus **H=A+120s is a conservative earliest expiry**, not the measured creation time. Never refresh A while the same ACK is held.

The original clock is `admission_runtime::original` (`:99–123`): earlier request Instant + sampled remaining milliseconds gives `until`; `origin=until-450s`. Guard uses that origin; Controller receives that same until. Cache D=`min(s.until, observer_context.whole)` **before** entering a local Phase, and check equality for this F. Do not call `observer_context()` after expiry: it calls `check()` first. Nested Phase only shortens and restores a bound (`migration_s53_deadline.rs:126–151`). Work480/outer510 and a supervisor's larger limit provide no extra controller time.

Recommended explicit **experimental allocation**, not a measured completion guarantee:

- After the held ACK/receipt/reserve32 gates and disable_ack write, let K=now immediately before abrupt stop. Require D-K >=150s and H-K >=70s. Expired D has priority; otherwise fail *before kill* as tail_budget or pause_budget respectively.
- Set R=`min(K+60s, D-90s, H-10s)`. Install one named `f_old_ack_release` Phase through R **around stop(A), wait/reconcile and the existing release poll**. Its up-to60s includes kill/reconciliation; those operations cannot consume an unbudgeted interval before a fresh60s clock. Polling may therefore get less than60s. Retain the existing inner Controller::wait(60); its Phase inherits the earlier R. Require success before R, not a predicate returned after R.
- This leaves at least90s of original F budget and10s before earliest old-Pause expiry at successful release. If any precondition fails, emit the exact diagnostic below and return Err; no kill/reconnect, shortened-history workaround or clock renewal. A later error still fails the run.

**Why90s is only a planning floor:** the tail has no successful F measurement. Its serial wait caps alone include three starts15s, four locator waits15s, two live waits30s, replay ACK30s and two cuts waits60s:315s, before histories, terminal reconciliation and two normal pairs (each owner's normal lifetime15s). These maxima do not fit the likely remaining budget; neither90 nor any smaller number is proved sufficient. F12's exit328.250s is not a kill timestamp or tail measurement. The guard is a defensible resource allocation for a fallible experiment, not a guarantee of whole-F completion. If root requires measured tail sufficiency before another F, that prerequisite is currently unavailable: the smallest missing observation is actual inclusive release-success→final-normal/memory completion on this workload. Recommend collecting it in the next allocated F rather than building a second migration fixture. Do not allocate/run merely on this contract.

On successful F, retain numeric original-origin timestamps for arm lower bound, K, actual kill-wait completion, first release observation and final completion; remaining-controller/earliest-pause budgets; actual mapped slot; and whether the late-open witness below occurred. Use existing `remember`, `timing` and assertion persistence, with no IDs/bodies in these added timing facts. Failure survival is the closed line below, not an assertion file promised on Err.

## Failure diagnostic that is actually retained

Add exactly five typed codes to `migration_s54_admission_diagnostic.rs` and the matching `admissionCodes` HashSet in `scripts/migration-s54-outer.ps1`:

```text
S54_ADMISSION_DIAGNOSTIC v1 stage=f_ack_release reason=tail_budget
S54_ADMISSION_DIAGNOSTIC v1 stage=f_ack_release reason=pause_budget
S54_ADMISSION_DIAGNOSTIC v1 stage=f_ack_release reason=controller_deadline
S54_ADMISSION_DIAGNOSTIC v1 stage=f_ack_release reason=phase_deadline
S54_ADMISSION_DIAGNOSTIC v1 stage=f_ack_release reason=operation_failed
```

Keep the existing prefix and advisory/non-authoritative classification; its historical name does not make these runtime observations admission failures. Add one narrow typed `report_code(code, result)` beside existing `diagnostic::report`, using its locked-stderr/static-line pattern and returning Result unchanged; re-export only it and Code to feasibility. At the wrapper around `feasibility.rs:693–705`, capture Result instead of letting `?` bypass reporting. After dropping Phase, classify Err using cached clocks: now>=D → controller_deadline; otherwise now>=R → phase_deadline; otherwise operation_failed. Budget refusals have explicit codes. Emit the selected boundary code directly: existing `at()` preserves older typed contexts and must not accidentally hide this stage. No anyhow-text classification, arbitrary error formatting, post-expiry query or Ok report. One line before unwinding; sink failure never replaces the original error chain.

The actual filter (`outer.ps1:83–115`) requires the entire exact stage:reason pair, <=128 ASCII characters, valid UTF-8 and complete LF/CRLF, with four lines/512 bytes per stream. The five lines above are under the limit; adding the exact five HashSet pairs makes them survive the existing native forwarder and `Write-S54Transcript` on failure. Unknown reason, extra field/tail, doubled CR, partial line and invalid UTF-8 remain rejected. Update the two vocabulary counts29→34 (`admission_tests.rs:106`, `test-migration-s54-diagnostics.ps1:39,46`). Diagnostics cannot satisfy any functional/wrapper marker or one-pass summary.

## Real regression and honest delayed-closure evidence

Reuse **`live_l1::handoff::final_p5::feasibility::s54_short_mixed_hooks_feasibility_public`**, Controller, existing outgoing `arm_ack/ack`, actual `stop`, reconnect, receipt/history oracles and owned cleanup. It requires a new root-allocated/prepared/activated attempt; current F12 cannot run again. No new selector that bypasses admission or pretends to be full F.

Before kill: actual originals/ready mapping; B role1, C1 actual slot from ready (F12 slot1), C1 Active/family2 at[185,35], A debt178/B debt0, pending/queued0; C2 L1 at[5,5]. Validate held **outgoing push** (`query_reply=false`, from7), current old binding/generation, receipt set/digest, reserve32, B live incarnation; timing guard as above. Never force B close, drop its permit manually, release hold early, fake status, replace abrupt kill with graceful shutdown or alter transport/system timers.

Poll immediately after actual A kill/wait; retain **request-start and response-end original-clock offsets** for the live status observations. The existing lease_current predicate requires `connection.close_reason().is_none()` (`runtime7/owner.rs:37–61`, runtime.rs:322–365). A fresh request **started >=30s after kill-wait completion**, returning same old generation/current lease and reserve32, is a genuine *late-open witness*: the connection was still open at the later sample. A subsequent real gate leasefalse/gen null/mapped-window0, with hold still armed and before H-10s, proves release following that late-open witness. Preserve the same warm B and no new admission during this interval. This establishes closure/release after30s without confusing observation latency with transport delay; it does not measure the exact close instant or prove the negotiated timer was30s. If no such fresh late-open sample occurs, report `late_open_witness=false`: the real path is proved if all gates pass, but reproduction of >30s closure is **unproved**. Delaying the first poll until31s is not a substitute.

Only after release: cold A reopen/zero replay; fresh binding/request, same B incarnation, same saved receipt multiset/digest; release hold only after replay verification; debt0, immutable histories, normal termination and final reopen remain mandatory. Keep ACK/prefix/live ordering. Current `Pause::wait(connection)` and `established` release real dispatch resources on connection/task termination; the existing `s54_pressure_wait_releases_actual_selection_on_control_release_and_connection_loss` injects a oneshot into `wait_until` and is **not** a real QUIC/process-loss regression.

No existing cheap seam guarantees that natural QUIC closure lands after30s. A deterministic real delayed-closure fixture would additionally need a test-only transport configuration seam and a genuine migrated process pair with ACK/source/receipt/history setup and owned admission/cleanup. A longer configured idle timeout alone does not exclude earlier path/RPC closure. That scope is larger and less faithful than reusing F, so **do not add it now**. Root can accept the real path separately from the conditional late-open reproduction claim.

## Exact small diff and verification allocation

Source allowlist for the next implementation, only:

1. `apps/kilogram-cli/tests/support/migration_s54_feasibility.rs`: arm lower bound, fixed local budget/Result wrapper, existing Phase/Controller wait, late-open/timing facts; unchanged gates and workload.
2. `apps/kilogram-cli/tests/support/migration_s54_feasibility_tests.rs`: budget/clock/classifier tests and bounded scoped-driver rejection checks.
3. `apps/kilogram-cli/tests/support/migration_s54_admission.rs`: narrow sibling re-export of Code/report_code only.
4. `apps/kilogram-cli/tests/support/migration_s54_admission_diagnostic.rs`: exactly five codes, report_code static emitter/required visibility.
5. `apps/kilogram-cli/tests/support/migration_s54_admission_tests.rs`: count34 and one benign typed-release diagnostic child.
6. `scripts/migration-s54-outer.ps1`: exactly five allowed pairs, unchanged caps/success authority.
7. `scripts/test-migration-s54-diagnostics.ps1`: count34; focused `-HeldAckOnly` mode using existing forwarder/Protocol/cleanup and a fresh implementation-owned evidence prefix. Return before old matrix; no new runner.

| Check / selector | Limit and evidence | What it proves |
| --- | --- | --- |
| Proposed `feasibility::tests::s54_f_held_ack_release_budget_and_pause_bounds` | stable harness60/90s work/outer; test clock inputs around150s/70s, original-expired priority, unchanged A/H across rewrites | Budget ownership/arithmetic only |
| Proposed `feasibility::tests::s54_f_held_ack_release_scoped_wait_and_nonrelease` | same command; real short Phase/never-release callback and local scope cancellation; delayed-observation case separately labelled; no profile/process pair | Bounded driver failure, no reconnect on Err; **not QUIC closure** |
| Existing admission closed-codes test + proposed `admission::tests::s54_f_held_ack_release_diagnostic_child` through focused script | focused script90/120s; actual child<=30s/forwarder40s; exit101, wait/EOF/owned cleanup; five exact lines, unknown/tail/partial rejection, no success authority | Actual producer→forwarder→production sanitizer retention; no F activation |
| Existing full-F exact selector above, separately allocated | original450/480/510s only; actual kill outcome, live same B, release samples, cold A replay/receipts/histories/terminal, timing facts and marker/cardinality evidence | Real production/IPC/transport path; >30s reproduction only with late-open witness |

Selector prefixes are `live_l1::handoff::final_p5::` for the table's Rust tests; full F selector is already exact above. Compile-only via existing stable harness, package kilogram-cli, profile verification, jobs2/offline/locked, fixed nonproduction `target/s54-diagnostics-source-checks/verification/deps` and its constant `kilogram-cli-test-migration_s5_process-tests.exe`; pin content, never encode revision in EXE name. Compilation budget600/630s, BelowNormal; no execution by direct cargo test. Execute only those source-check selectors with stable harness/filter/expected artifact pins and existing bounded ownership helpers. Proposed focused script adds only the fresh implementation evidence prefix, retaining existing constant harness path; per-case leaf/parent bounds above remain clipped to its original90/120s owner clock. No aggregate or old diagnostics matrix. Final source-check and runtime commands still require root assignment after implementation review; no tests were run in BB.

P's `migration_s54_catchup.rs:140–150`30s wait stays outside this diff and must be reviewed before WholeP. F from7/debt178/frontier[185,35] must not replace P from385/debt256/frontier[641,640]. Production transport/Owner7/lease/pause behavior and all attempt-purpose bindings remain unchanged.

## Handoff

Fresh admin `.tmp/s54-block4bb-held-ack-contract-01`; companion pins cover the contract and preservation documents, not raw graphs. All536 source/nine images, six accepted AY/AZ/BA documents, HEAD/index/navigation/foreign bytes preserved. Raw scopes stayed closed throughout BB. No implementation/build/test/new F/Prepare/Run/WholeP/commit/push. Root can assign the seven-file implementation with this explicit experimental budget and proof distinction, or reject the90s floor without mistaking it for measured tail sufficiency. One callback, then stop.
