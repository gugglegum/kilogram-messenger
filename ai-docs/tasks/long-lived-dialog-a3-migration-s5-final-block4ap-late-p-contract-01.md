# Block4AP: late P migration contract

2026-11-03. Source baseline: AO `266989ec653ad6b5471fbc1ea8c779499d2a2534`. Contract only: no implementation, compilation, test or production run. S5.3 is closed; this does not accept S5.4.

## Recommendation and evidence boundary

Add a small P storage preflight and narrow witness assertions to the **existing P scenario**. Keep actual current-A/legacy-B execution as an explicit WholeP obligation. A second process scenario runner is disproportionate; the existing component network helper cannot establish this mixed-generation path. Repair the P1 offline prerequisite under a separate reviewed fixture change before WholeP.

The proposed “one B24 debt” is incorrect. The existing single-debt assertion concerns **A24 at A**. Before late B migration, **B16–B24 (nine debts)** is the source-derived expectation; fresh evidence must confirm exact IDs and receipt absence. This document reports no new observed runtime result.

## Source references and milestones

Paths below are repository-relative. Support files are under `apps/kilogram-cli/tests/support/` (S); runtime/storage files are under `apps/kilogram-cli/src/ordered_dialog/` (R).

| Milestone | Required state and evidence | Source |
| --- | --- | --- |
| Original enrollment | Retain both genesis witnesses. Hold B16 and suppress A prefix requests; C2 has no Text traffic. | S `migration_s54_early.rs:134–160`; `migration_s54_control.rs:94–98` |
| P0 complete | A C1 next=[17,16], own debt=0; B next=[17,17], own debt=1 (B16). | S `migration_s54_early.rs:178–196` |
| P1 queue, then A migration | Stop/wait B; establish A offline; enqueue A17/A18 without advancing A send=17 or receive=16. Stop/wait A, migrate only A, suspend materialization, start actual current A; first indexed history has 31 Texts. C2 remains [1,1]. | S `migration_s54_early.rs:226–285` |
| P2 mixed execution | Restart legacy B and import ALPN1 locators. Current A materializes queued A17/A18: A next=[19,16], B receive=19. A19–23/B17–23 then yield A next=[24,16], debt=0; B next=[24,24]. | S `migration_s54_early.rs:288–312` |
| P2 late checkpoint | With ACK controls armed, actual A24/B24 yield A next=[25,16], debt=1, pending=8; B next=[25,25]. A24 delivered=false. Pending A records are B17–B24; B16 is still missing. | S `migration_s54_early.rs:313–342` |
| P3 late B migration | Stop both; reconcile current A interval; capture/migrate stopped legacy B. B migration source C1=[25,25], C2=[1,1], queue empty. A's retained migration source remains [17,16], distinct from its live [25,16]. Retain and verify B's saved R1(A24) against A's exact debt. | S `migration_s54_transition.rs:47–98`; `migration_s54_source.rs:650–688` |

B16 is released only later (`migration_s54_transition.rs:264`); A prefix suppression is cleared at line 322. Do not release these controls early to manufacture a smaller debt set.

### Why B debt is not one

Legacy dispatch excludes held B16 (R `runtime.rs:1028–1031`). Current A persists later arrivals as Pending (R `runtime7/data.rs:185–280`); receipt creation follows contiguous readable drain, blocked by missing B16 (`data.rs:341–457`). ACK queries return stored receipts, not delivery acknowledgements for pending ciphertext (`runtime7/protocol.rs:369–400`). B17–B24 therefore cannot retire through A's readable receipts at this checkpoint.

Legacy `disable_ack` suppresses saved-receipt pushes but does **not** suppress its unacked queries (`runtime.rs:989–1017`). Current A's query suppression is explicit (`runtime7/protocol.rs:340–347`). Do not describe both controls as disabling all queries.

Verify nine B debts from authenticated legacy outbox entries and absence of opposite signed receipts, then compare migrated `debt_page` with those exact ordinal/ID pairs. `outbox.rs:166–187,287–343` governs retirement; `storage_migration/provenance.rs:632–651` rejects delivered-without-receipt and undelivered-with-saved-receipt. By contrast, `migration_s54_source.rs:650–688` explicitly proves **A** debt=[(24,A24-ID)] and B's authentic R1(A24). Existing P2 has no corresponding B debt-count assertion.

## Smallest implementation slice after review

1. **Real storage/crypto preflight.** Extend the existing staged fresh fixture narrowly: permit P genesis capture and owner-specific migration instead of immediately migrating both. Reuse `migration_s54_cold_witness_fixture.rs:25–194,209–239`, real Engine/Native traffic, queue request IDs, `Legacy::capture`, `same_identity`, and `after_migration`. First case proves genuine A genesis → [17,16], C2=[1,1], queue A17/A18, zero migration Native calls, exact cold indexed contents and deferred A binding with B unbound. Exercise both physical C1 slots without interchanging owner roles.
2. **Separate late-B storage case.** Grow a fresh legacy pair through real Engine/Native operations to B [25,25], preserving withheld B16, A pending B17–B24 and exact receipts. It is permissible to keep **both engines legacy in this independent case**. Migrate B and prove nine exact debts, queue empty, C2=[1,1], unchanged identity and changed inventory digest. This case proves late-source conversion only; it does not prove mixed execution or continuity with the first case. Never reopen a migrated A store as a legacy Engine. Component leases are test observations, not TLS/process evidence.
3. **Existing P assertions.** Factor/reuse exact source/debt verifiers and call them at the existing P1/P2/P3 boundaries, before ceremony. Record A's verified migration source unchanged while B grows, B unbound until its own successful migration, exact queue/pending/debt IDs and final AN lookup. Use existing `Controller::memory_diagnostics` (`migration_s54_control.rs:262`), owned child lifetimes and interval records. No alternative PASS selector, partial production admission or second full runner. The actual mixed traffic, leases, restarts, process memory and transport evidence still require a later authorized fresh WholeP run.

Storage tests must generate packets/receipts through the real APIs; setting target counters, copying heads or synthesizing signatures/receipt state is not coverage. AO's injected Stats/deferred ledger tests may check observation rules separately and must retain that label; the Windows child adapter proves owned-process query/lifetime behavior, not Owner7 networking.

### Reusable APIs and missing seam

Current `Owner7::new(repo, enabled_ids)` is the real entry (`runtime7/owner.rs:118`). Materialize/receive/drain are available (`runtime7/data.rs:123,185,341`), but require a valid lease containing an actual Iroh connection, selected permitted route and deadlines (`owner.rs:17–59`). A fabricated Lease1 cannot stand in for admission.

`runtime7/handoff_integration_tests.rs:253–267` accepts **[Owner7;2]** and starts two current actors; its `legacy()` at line 203 means ALPN1 between current owners. `handoff_tests.rs:279–348` also converts both owners first. The old `repair_tests.rs:19,37,163,179` pair exposes component admission/activation but is two legacy Engines. Old runtime admission/established functions are private (`runtime.rs:314,402,1106`); its public entry is `run(Profile)` at line 1127.

There is no reusable one-sided actual legacy-runtime admission/lifetime adapter in those helpers. A standalone genuine mixed component test would need that separate seam. Existing P already owns the correct legacy/current CLIs, locators, stops and clock; its constructor selects WholeP (`migration_s54_control.rs:306–330`). Calling only P0–P3 is not an already authorized component admission. Prefer the preflight plus existing-P obligation above.

## Preconditions, focused failures and verification

Keep genesis and migration witnesses separate: stable enrollment secrets, context/spec/Native keys and D/E/SID must match (`migration_s54_source.rs:615–638`), while traffic changes inventory source IDs. Resolve each baseline through AN `MigrationSources::source` (`migration_s54_control.rs:33–65`), validating owner, context, physical slot, Ready and interval endpoint. Preserve AO order: source capture → identity/check/Ready → accounting/interval/retained rows/witness → bind (`migration_s54_early.rs:199–224`). No rebinding A to live state or B/genesis IDs. Preserve fresh current restart nonces and the unchanged memory clock, ledger, coverage, wait and EOF rules.

Targeted refusal cases: missing/foreign/wrong-slot Ready or endpoint; binding genesis; premature B binding; A baseline drift after B growth; reused current nonce; missing queue member; absent/extra/wrong-ID B debt; falsely delivered pending event; forged/mismatched A24 receipt. Reuse existing AN/AO negatives where unchanged. Direct regressions are the new P storage cases and affected AF P cold conversion/source-binding consumers; rerun F/adapter cases only if a shared fixture constructor or observation code changes. Do not broaden the unrelated matrix.

Use fresh isolated test directories and the existing stable Windows harness: compile without execution, then `scripts/run-cargo-tests-stable.ps1` with fixed executable paths, CargoJobs2/BelowNormal and reviewed bounds. No direct network `cargo test`. Exact selectors/bounds belong to the next implementation assignment. This contract ran no tests. Preserve failed evidence, unrelated changes and existing production images; no AM16/AD16 raw-log access, historical re-audit, stale AK reuse, F11, Prepare or Run.

## P1 offline prerequisite: must close before WholeP

P1 stops B then passively waits **30 seconds** for A `ConnectionBackoff` (`migration_s54_early.rs:227–241`). After P0, A has no own unacked/queue/pending work. Legacy idle Resume is every 30 seconds (`runtime.rs:849,871–891`), an RPC may take 15 seconds (`runtime.rs:165`), and Backoff is published only when the dialer/admission/established call returns an error (`runtime.rs:1236–1238`). Source does not guarantee prompt remote-stop detection; the next idle probe plus RPC can exceed this barrier. B's held debt does not make A's local work-waiting predicate true. This is a source-derived prerequisite gap, not a measured new failure.

Before WholeP, separately review a deterministic offline setup or demonstrate a bounded detection mechanism. The existing F fixture stops/waits/reopens A after B is stopped, requiring fresh failed admission before queueing (`migration_s54_feasibility.rs:350–363`). A P analogue should additionally prove A [17,16], unacked/queue/pending=0, stable identity and unchanged Native state across reopening, then queue17/18 without committed advance. Keep the absolute run clock and lifetime/interval ledger intact. Do not merely extend the passive deadline or change production heartbeats/timeouts. No timing fix is implemented here.

Delivery boundary: root reviews this contract and assigns implementation. No commit/push or polling follows this handoff.
