# AQ2: retained proof at P migration boundaries

2026-11-10. Review candidate on AQ1 HEAD `6516f485d73d3bff21f5178a30e89d87e53b3857`. Only retained migration integration is implemented. Offline restart, warm-current collection, P2 statuses, descriptor-target/frozen-image/lifetime qualification and production runtime/storage/crypto/memory/ledger changes are deferred. No attempt allocation, Prepare/Run/WholeP or commit/push.

## Result and shared operation

P1 now explicitly calls migrate_p(0,FirstA); P3 calls migrate_p(1,LateB). Each route performs one admission-checked capture_boundary under the existing exclusive lease, releases it, then consumes the retained Proof into a Checked value before conversion. Typed Phase has only FirstA/LateB. Checked owns immutable retained evidence; no optional proof/fallback can skip P verification in this route.

Proof.prepare validates owner/source/logical context mapping and actual captured contiguous source frontiers, including genesis, for both contexts. FirstA requires role0 C1=[17,16], C2=[1,1]/zeroText, exact A17/A18 queue IDs with sealed body provenance from the shared admitted capture, and no own debt. LateB requires role1 C1=[25,25], C2=[1,1]/empty queue, exact authenticated B16–B24 ordinal/event debt pairs, genuine saved receipts retiring B1–B15 and signed R1(A24) via AQ1's signature verifier. Test-only phase oracles were moved into this shared operation, not duplicated. Original genesis identity remains checked before conversion.

The common Controller body accepts a closed RetainedMigration enum: Legacy for F, Checked P for P. It preserves stopped-owner check, original identity, preview source-ID agreement and one conversion. Its complete()? returns the retained Legacy witness and endpoint only after verification. P complete calls Checked.after_migration: first original after_migration fully verifies/closes its repo, then opens a new cold repo, invokes shared Exact with physical slot derived from Ready, closes it, and returns. Strict cold !initialized/missing-metadata behavior is unchanged; no source/repository handles overlap or survive conversion.

Only after complete()? succeeds does the Controller register accounting, migration interval, remembered Ready/witness and finally AN lookup/AO binding. A cold failure propagates before those registrations/start; converted failure evidence is retained, with no rollback, second conversion or successful partial proof. B conversion writes only owner1's migration slot; original A17/16 source and both genesis witnesses stay intact. Existing71/95 accounting, separate saved-R1-versus-current-A debt verification and ceremony order are preserved.

F's migrate(owner) retains Legacy::capture and original after_migration-only semantics through the common body. It does not require P-specific queues/debts or new Exact checks. Its migration measurement still wraps capture and the common body. P1/P3 stop sequences, P2 body and pending comparator are unchanged; no extra lifetime, new timing deadline or clock reset was introduced.

## Focused evidence

Both AP real Engine/Native storage selectors now invoke the exact Proof.prepare / Checked.after_migration operations used by P, both C1 physical slots. Repository stamps are taken in closed scopes before/after the shared cold operation; no competing handle can turn a mapping refusal into a lock-contention result. Existing eight queue/debt/receipt and eight pending substitutions still pass their intended refusal checks.

New pre-conversion in-memory refusals: opposite intent, wrong owner, swapped logical mapping and missing actual own frontier source for each phase; lateB also lacks saved R1 or a retired B1 receipt. These10 refusals occur while fixture owner is still legacy and Ready is null. Two further physical Ready mapping substitutions (one per phase) invoke the same shared cold operation after conversion. They are immutable component checks, not process execution. Total28 targeted refusals:16 preserved+12new. No forged durable packets/counters/receipt state.

The directly affected existing AF real F cold/strict-warm regression passed for both owners, preserving the original storage verifier behavior. It does not execute the actual Controller F wrapper. Static inspection of the actual Controller call sites and complete()? propagation is retained in static-order.json: stopped/identity/preview/conversion/complete precede accounting/interval/Ready/witness/bind; F and P branches are explicit, source capture is once, P1/P3 differ only at the typed migration call, P2 and pending comparator match the parent. This source inspection is not runtime coverage.

Late AP is an independent fixture with two legacy Engines, still proving B debts16–24=9, A24's exact debt and saved R1, pending B17–B24 and cold source preservation. DeferredModel lifecycle records remain injected. Actual current-A/legacy-B transport, full Controller execution, fresh failed admission, warm L1 collector and planned24lifetimes are not established here; they remain later reviewed/fresh WholeP obligations. No unchanged broad AF/AN/AO matrix or adapter repeat.

## Verification and scope

Fresh `.tmp/s54-block4aq2-retained-migration-01`; current pinned tools verified before and after: process-local Rust1.98.0/pwsh, offline/locked verification profile, Jobs2/BelowNormal. Compile-only then exact stable Windows selectors via scripts/run-cargo-tests-stable.ps1; no direct network cargo-test execution. Admission environment identifier stayed KILOGRAM_S54_CAPTURE_ADMISSION; administration renaming was limited to owned paths/classification. Fixed diagnostic harness `E:\AI-Projects\kilogram-p2p-messenger\target\s54-diagnostics-source-checks\verification\deps\kilogram-cli-test-migration_s5_process-tests.exe`, 35692032B SHA256 `ab13e1c3bcba8cdd09fc9ae977c2d4b0690eae899df5e1ebd4ce5376651b3991`.

| Step | Tool/child exit | Seconds | Work/whole/cleanup seconds |
| --- | --- | --- | --- |
| format-01 | 0/0 | 1.117 | 570/600/30 |
| compile-01 | 0/0 | 34.198 | 3600/3660/60 |
| rust-early-01 | 0/0 | 140.074 | 570/600/30 |
| rust-late-01 | 0/0 | 209.563 | 570/600/30 |
| rust-01 | 0/0 | 30.118 | 570/600/30 |
| clippy-01 | 0/0 | 17.057 | 3600/3660/60 |
| seal-01 | 0/0 | 1.141 | 570/600/30 |

Exact selectors: source::p_storage_tests::ap_real_p_first_a_storage_roles_and_slots; source::p_storage_tests::ap_real_p_late_b_storage_roles_and_slots; source::cold_witness_tests::af_real_f_cold_witness_first_start_and_strict_warm_restart (full names in pinned selector arrays). Strict CLI all-target Clippy passed with literal -- -D warnings; scoped formatting/diff and final diagnostic process inventory passed. Source/tool/image pins were unchanged throughout focused execution. No compile/test failures or retries occurred, and no successful checks were repeated after changes.

Seven actual application owner receipts record child wait/exit, actual Job membership, bounded stdout/stderr EOF, no work expiry and empty post-cleanup memberships. Forced owned residual cleanup after launcher completion: `{'compile-01': [49492]}`; residual application identity is not established, membership/cleanup after=[] are verified. All other cleanup was unforced/empty. Actual tool chunks, logs, Cargo argv and image pins accompany the report. These are diagnostic receipts, not a WholeP verdict.

Changed sources only: migration_s54_boundary.rs (typed phase/checked shared cold operation), migration_s54_early.rs (explicit retained common body/P1 call), migration_s54_transition.rs (P3 typed call only), migration_s54_p_storage_tests.rs (shared operations/new focused refusals). Candidate adds only this report/pins to those four sources; accepted AQ contract is already in the parent and is not restaged. Existing navigation files are byte-identical to the fresh baseline, index empty, no remaining diagnostic processes. Production images/preparations, historical private accounts and expired AM16/AD16 raw evidence were untouched; no historical manifest graph was traversed. Stop for root review before publication or the next slice.
