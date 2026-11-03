# Block4AP: first-A and late-B storage preflight report

2026-11-03. Review candidate on published AO HEAD `266989ec653ad6b5471fbc1ea8c779499d2a2534`. Implements the accepted AP contract and storage task only. No commit/push; no production Prepare/Run, F11 or WholeP. P controller and P1 offline prerequisite are unchanged.

## Result

Both new exact selectors passed, each on separate fresh real Engine/Native pairs with C1 physical slots 0 and 1 and fixed logical roles. The late-B expectation is now observed storage evidence: **nine exact B16–B24 debts**, with B1–B15 retired by genuine opposite signed receipts. The one debt at A is A24, while B retains its authentic signed R1(A24). This does not establish the current-A/legacy-B runtime scenario.

Early A: real pre-traffic genesis capture after dropping both vault leases; real P Text traffic with withheld B16; A17/A18 queue request IDs and sealed bytes/body binding verified. Queueing does not change durable Native state. Only A migrates: C1 [17,16], C2 [1,1], exact cold contents and absent current-owner metadata; B remains legacy, its inventory unchanged, and its memory source unbound.

Independent late B: both traffic objects remain legacy Engines in a new pair. Existing queued A17/A18 are genuinely materialized; A/B traffic continues through24. A accepts exact signed B17–B24 as pending behind missing B16 with readable=0 and no receipts; B reads A traffic, sends genuine receipts through A23, and saves R1(A24) without A applying it. Exact old outbox states/events/receipts are observed before migration. Only B migrates: C1 [25,25], C2 [1,1], empty queue; cold exact debt_page, event and receipt bytes match the retained witness, including R1(A24). A remains physically legacy and unchanged by B conversion.

Both cases prove stable enrollment/context/Native session identity with changed genesis-to-traffic source digest, strict cold after_migration verification, zero migration/additional-read Native calls and no cold writes. Verified witness/Ready/endpoint facts feed AN MigrationSources and AO checked binding for only the actually migrated owner. Lifecycle records used to inspect binding are **injected**; no actual process, Stats, mixed runtime, lease admission, TLS or memory measurement is claimed by these two tests.

Eight new in-memory substitutions exercise the same exact verifier used by positive cases: missing/extra/wrong-ID/wrong-bytes queue; missing/extra/wrong-ID debt and a purported receipt at an actual debt slot. None is persisted. Existing refusal suites remain unchanged.

## Changed source and scope

- `migration_s54_cold_witness_fixture.rs`: opt-in isolated AP genesis roots, owner-specific migration, actual late legacy traffic and exact pending checks. Existing AF/AO constructors retain their root/default-slot/workload behavior. AP fresh directories retain data even on failure; no migrated store is reopened as legacy.
- `migration_s54_p_storage_tests.rs`: exact real outbox/receipt capture, signature verification, cold comparator, one-owner checked binding, two positive tests/both slots and eight substitutions.
- `migration_s54_source.rs`: test-module registration only.

No production runtime/storage/crypto or memory equality/ledger changes. The three pre-existing dirty navigation files are byte-identical to the baseline; index remains empty. The publication candidate contains these three source paths, the accepted AP contract, this report and its companion pins only. Root-owned task records are not added by this executor.

## Verification and actual receipts

Pinned process-local Rust1.98.0 and pwsh; offline/locked, verification profile, Jobs2/BelowNormal; isolated diagnostic target `target/s54-diagnostics-source-checks`. Compile-only then exact execution through `scripts/run-cargo-tests-stable.ps1`, fixed executable `target/s54-diagnostics-source-checks/verification/deps/kilogram-cli-test-migration_s5_process-tests.exe`. No direct network cargo-test execution. Actual stable image pin and Cargo arrays are in companion evidence.

| Step | Tool / child exit | Seconds | Work/whole/cleanup seconds |
| --- | --- | --- | --- |
| compile-02 | 0 / 0 | 35.149 | 3600/3660/60 |
| rust-early-01 | 0 / 0 | 148.582 | 570/600/30 |
| rust-late-01 | 0 / 0 | 223.115 | 570/600/30 |
| rust-01 | 0 / 0 | 330.699 | 570/600/30 |
| adapter-01 | 0 / 0 | 35.457 | 570/600/30 |
| clippy-01 | 0 / 0 | 18.709 | 3600/3660/60 |
| seal-01 | 0 / 0 | 1.295 | 570/600/30 |

Exact new selectors:

- `live_l1::handoff::final_p5::source::p_storage_tests::ap_real_p_first_a_storage_roles_and_slots`
- `live_l1::handoff::final_p5::source::p_storage_tests::ap_real_p_late_b_storage_roles_and_slots`

Direct shared-fixture regressions: AF F/P cold + strict warm restart, 20 exact witness/cardinality refusals and AF genuine F/P Ready-before-metadata baseline; AO real F growth/both slots and seven independent deferred-binding refusals. Six exact selectors passed. Separate AO actual Windows adapter selector passed with its actual owned-child/admission protocol; its decoder/Stats injection and non-Owner7 coverage limit remain unchanged. Strict CLI all-target Clippy passed with the literal `-- -D warnings`; scoped rustfmt and diff checks passed. No unrelated matrix or historical validation was repeated.

The first compile failed due to a missing StateDirectoryLock import and an unused import (17.968 seconds). Both were corrected; its immutable failed logs/receipt remain alongside successful compile-02. No test failed and no successful test was rerun. Compile-02's owned Job cleanup forced residual process52180 after the launcher exited; membership was verified, the residual's application identity was not established, and cleanup ended with no remaining members. No expiry occurred. Every required completed invocation has actual handle Job membership, child wait/exit and bounded stdout/stderr EOF receipts. Exact application tool chunk IDs, timestamps, limits, stdout/stderr hashes and cleanup records are retained under `.tmp/s54-block4ap-storage-01` and pinned by the companion JSON; these are diagnostic execution receipts, not a WholeP PASS.

## Remaining gate

Next reviewed work must reuse these assertions in existing P and close P1's passive Backoff30s prerequisite. Actual staggered current-A/legacy-B transport/process evidence remains a fresh authorized WholeP obligation. Production images and historical preparations were untouched; F01–F10 remain consumed and AK stale. No expired AM16/AD16 raw logs or historical private account material were accessed. Stop here for root review; publication requires root acceptance and a separate commit/push assignment.
