# Block4BC: bounded F held-ACK release implementation

2026-11-10. **Implementation/source checks submitted for root review; no publication or real F runtime.** Baseline HEAD `447451e819d3d0b97f1a55ecd0877a1571f48fa4`, index empty. BB contract/pins are byte-identical. Exactly its seven source files changed; expected publication candidate is those7 + BC report/pins + BB contract/pins =11 paths. Admin `.tmp/s54-block4bc-held-ack-implementation-01`.

## Behavior

`migration_s54_feasibility.rs:274–460,873` now captures the original D before a local Phase and checks its equality with the Controller deadline. A is recorded immediately before first ACK arm after checking prior holdfalse, CatchUp/B and120000ms schedule. H=A+120s remains a conservative earliest creation-based expiry; it is never renewed for that ACK.

Before the scenario stop, controller remaining>=150s and earliest-pause remaining>=70s are required. Refusal returns Err before the stop/reconnect work closure. R=min(K+60,D-90,H-10); one Phase includes actual stop/wait/reconciliation and polling. Existing Controller::wait inherits R, and the post-work check rejects a predicate that returns after R. Original450/480/510 limits remain unchanged. Tail90 is an **experimental planning floor, not measured sufficiency**. Failure teardown retains ordinary owned cleanup; zero scenario stop/reconnect on budget refusal does not prohibit cleanup of owned processes on failure.

Each release observation uses one actual status response for mapped role/context/slot, lease/generation, counters and window. B incarnation and old Head binding/generation are retained. A late-open witness requires request-start >=30s after stop's wait/reconciliation completion, same old current lease/generation and reserve32 in that response. This completion timestamp conservatively follows the actual kill/wait; it is not claimed as the exact process-exit instant. Without such a fresh observation the witness is false. Leasefalse/generationnull/mapped-window0 must pass before cold A reconnect; warm B, ACK suppression/hold ordering, fresh binding/request and identical receipt multiset/digest/history/normal-reopen gates remain intact.

Numeric assertion rows retain arm/earliest-expiry, kill-start/wait-reconciliation end, request/response/release offsets, controller/pause margins, mapped slot and optional late-open request/response offsets. The final assertion includes inclusive release→memory-completion duration. No new timing identifier/body/key is exported.

`migration_s54_admission.rs:10`, `migration_s54_admission_diagnostic.rs:48–52,80–97` expose only typed Code/report_code for this sibling use. The selected static code is emitted directly to locked stderr on Err, with unchanged Result/error chain; older typed context cannot mask the release stage. Sink errors do not replace it; Ok emits nothing. After local Phase drop, cached clocks select original expiry first, then phase expiry, otherwise operation_failed. Preparation failures are also caught/reported; budget refusals select their own exact code. No post-expiry scope query or error-string classification is used.

`scripts/migration-s54-outer.ps1:82–83` admits exactly the five BB pairs: f_ack_release/{tail_budget,pause_budget,controller_deadline,phase_deadline,operation_failed}. Existing128-byte/four-line/512-byte bounds and no-success authority remain unchanged. `test-migration-s54-diagnostics.ps1 -HeldAckOnly` uses the existing fixed harness, actual benign producer, forwarder/filter and cleanup, returns around the old matrix, and accepts only the fresh BC evidence prefix for this mode.

## Actual verification

Rust1.98.0; offline/locked/jobs2/BelowNormal; fixed diagnostic cache. Cargo compiled without execution first. Both test runs used `scripts/run-cargo-tests-stable.ps1`; all other target harnesses matched zero tests under these filters. No direct cargo test execution, full-F selector or historical matrix replay.

| Check | Actual outcome | Owner work/whole/cleanup seconds |
| --- | --- | --- |
| Scoped rustfmt, PS parser, seven-file diff check (`static-01`) | exit0, wait/EOF | 570/600/30 |
| Compile-only (`compile-01`) | exit0; Cargo40.51s; wait/EOF | 3600/3660/60 |
| Affected test strict Clippy (`clippy-01`) | exit0, literal `-- -D warnings`, wait/EOF | 3600/3660/60 |
| `s54_f_held_ack_release_budget_and_pause_bounds` and `s54_f_held_ack_release_scoped_wait_and_nonrelease` | **2 passed**,0 failed; benign producer1 ignored | 570/600/30 |
| Existing `s54_admission_diagnostics_closed_codes_preserve_original_unknown_and_nested_context` | **1 passed**,0 failed | 570/600/30 |
| `HeldAckOnly` (`focused-diagnostics-01`) | enclosing verifier exit0; **10 checks**, nine separate cases | 570/600/30; internal90/120, leaf30/parent40 |

Rust selector prefix is `live_l1::handoff::final_p5::feasibility::tests::` for the two scheduling tests and `live_l1::handoff::final_p5::admission::tests::` for vocabulary/benign child. Scheduling tests cover threshold equality/one-nanosecond refusal, unchanged A/H/D, original-expiry priority, zero stop/reconnect on prekill refusal, real short-Phase nonrelease rejection, late-success rejection and nested clipping. The vocabulary test checks34 closed unique lines, original error chain and no output for Ok. These tests are explicitly **not QUIC/process-loss proof**.

Focused actual cases:

| Cases | Actual child/forwarder exit | Retained / dropped diagnostics |
| --- | --- | --- |
| Five separate `held-{reason}` cases | **101 each**, expected; enclosing verifier0 | 1 /0 each |
| `held-ok` | 0 | 0 /0 |
| `forged` | 7, expected | 1 /6 |
| `partial` | 7, expected | 0 /1 |
| `count-bound` | 7, expected | 4 /4 |

Five nonzero producers use the real ignored `s54_f_held_ack_release_diagnostic_child`, closed reason input and report_code. Each has actual native capture with both EOFs, no overflow/read failure, exact wait/job verification and original synthetic error redacted. All cases reject diagnostic-only functional success. No five-line stream is treated as valid. The tenth check is complete34-code producer/filter vocabulary parity. Focused internal cleanup is verified/nonforced with zero remaining children.

Stable harness: `target/s54-diagnostics-source-checks/verification/deps/kilogram-cli-test-migration_s5_process-tests.exe`,36833792B, SHA256 `940833ab9ebd32cbfb3a1fc252ddd2e107586f502dffb729e7768bdacb4a2409`. Source7 and actual tool pins are bound before/after each owner step; harness pins match before/after Clippy, both stable runs and focused diagnostics. The initial compile stages this fixed path; its precompile cache executable is not claimed as a preserved baseline image.

**Compile cleanup qualification:** its primary leaf exited0 with both EOFs and unchanged source binding, but the cleanup-entry Job list contained owned PID49108. Bounded cleanup ended empty. Executable identity and individual termination receipt were not retained; the entry list alone does not establish a particular TerminateProcess call. Do not label that process or claim unforced compile cleanup. Other five outer steps had empty cleanup-entry lists and all six ended empty. No check was repeated to erase this observation.

Two final administrative-summary attempts failed (Python token syntax, then an incorrect assumption that every cleanup-entry list was empty); corrected locally, receipts/reasons retained in `admin-corrections.json`. No product verification failed or was retried; successful checks were reused because the seven source hashes remained unchanged.

## Review boundary and preservation

Final seal: source536 total, exactly7 changed/529 preserved; nine production images, six accepted AY/AZ/BA documents, both accepted BB documents, HEAD/index and navigation/foreign bytes preserved. Current sanitized test evidence, diff, exact source/tool/harness bindings and candidate are pinned. Root task/reviews/navigation/admin scripts are excluded from the expected11 publication paths.

All historical BA/AU/F01–F12/AM16/AD16 raw scopes remained closed; no raw/private origin traversal or old command hashing. New BC evidence is public synthetic source-check evidence only. No commit/push, new F allocation, production Prepare/activation/Run or WholeP. Runtime7/transport/purpose and P's separate30s barrier are untouched. Real outgoing-held-ACK process-loss/replay proof and natural closure beyond30s remain deferred to a separately assigned next F; this source-check result establishes neither. S5.3 closed; S5.4 open. One callback to root, then stop.
