# A3-S5.3 — queued existing-dialog protocol handoff

2026-10-26. The human explicitly asked root to agree the next stage now and send
it automatically when the executor finishes. Root selects accepted S5 implementation
cut3: existing Consent2/control stages, durable handoff and mixed continuation.
Use the same GPT-6.1 Sol Max executor, normal speed, no Fast. This is a root-owned
forward assignment; S5.2 remains the executor's current task.

## Automatic activation and baseline

The authorized sequence is:

1. Executor submits S5.2 implementation and evidence; root reviews and runs focused
   correction loops as needed. A completion message alone is not acceptance.
2. After explicit root acceptance, executor commits/pushes the exact S5.2 allowlist.
   Root independently verifies published SHA/parent/path/blob correspondence.
3. Root immediately dispatches this S5.3 task, specifying that actual published
   S5.2 SHA and accepted source/evidence/checkpoint paths. Do not invent that future
   SHA or start from the present dirty candidate. No new human confirmation is needed.
4. Executor supplies the narrow mapping described below. Root reviews it promptly,
   resolves remaining contract/implementation decisions and grants the exact new
   verification tranche; then dispatches implementation. The user-facing scope is
   agreed now, while these technical gates remain between root and executor.
5. Root reviews the completed S5.3 implementation, requests fixes if needed, then
   assigns exact commit/push and independently verifies publication.

Until step3, no S5.3 product edits, builds, tests, fixture opens or staging are
authorized by this document. Preserve it as root-owned work outside the current
S5.2 candidate; do not silently stage it in that publication. Do not alter active
S5.2 root task/review/navigation files to describe future work as started.

## Product outcome and existing contract

An existing, already migrated L1 dialog can explicitly switch to the long-lived
protocol and continue messaging with the same account, devices, dialog, session
and keys. Existing history, request identities, queues, pending messages, receipt
debts and blocks survive. The old per-lane L1 lifetime remains unchanged; actual
format2 continuation removes that L1 boundary for the upgraded dialog.

The [accepted S5 contract](long-lived-dialog-a3-migration-s5-contract.md), SHA256
`f87c5c8b0b307863078cf3e2e85f3d13a05392d1d654affd5f2a40a3943ad2ad`, already decides
the protocol, stages, scope, captures and budgets/review procedure. In particular,
use sections3,6–8,10–12 and implementation cut3. Reuse the accepted S3 existing
proof/mixed protocol and S4 infrastructure without weakening fresh-only checks.
S5.2's [mapping](long-lived-dialog-a3-migration-s5-l1-plan.md), overriding
[revision2](long-lived-dialog-a3-migration-s5-l1-plan-revision-2.md) and eventual
implementation acceptance provide the concrete starting point. Do not write a
replacement architecture contract or repeat settled review without a contradiction.

## Scope of the first deliverable

Write `long-lived-dialog-a3-migration-s5-handoff-plan.md` against the actually
accepted/published S5.2 source. Keep it a concise implementation and verification
mapping: exact reusable modules, new typed purposes/write sets, operator/IPC surface,
changed shared branches, observable tests and measured budget anchors. Resolve:

1. Public existing Consent2 creation/import and current policy, explicit per-C
   upgrade intent/cancel, exact original C/D/E/SID/cert roles and real completed
   Ready1/genesis prerequisites. An incomplete session stays on genuine L1.
   Account-pin advances close all referencing leases; unrelated C remain usable.
2. Durable Propose/Prepare/Select/CommitB/ActivateA and exact P/A/K/Committed bytes.
   Publish controls only after commit. Exact retry returns saved bytes, not a new
   signature or Native operation. Before durable P/Prepared an intent is cancellable;
   after that boundary there is no downgrade/rollback. Freeze own materialization,
   preserve allowed bounded queue/old receive/repair/ACK, and close old ALPN1 leases
   at that first boundary, not only at Selected.
3. Fresh ALPN2 existing admission before Q with actual current Consent2/Root/locator/
   endpoint/route/lease. Support exactly the reachable asymmetric stages and hints
   in contract7.1.1. In particular Selected A can reconnect to Prepared B without
   B already having Q. No fake selection, Q adoption from a hint, symmetric-selected
   prerequisite, ALPN1 fallback after P/Prepared or weakened fresh-S4 verifier.
4. Closed pre-Q Head/Resume/Prefix/Data1/Receipt1/ACK handlers, actual immutable
   source and opposite-Device receipt verifier, current final gate and durable
   watermarks. Possession/hints do not release debt. Event2 is refused before local
   selection, before pending/possession/Native. Once selected, mixed verification
   uses the real local proof and never falls back after an error.
5. Atomic Guard1/Seen-to-Guard2 and Native1-to-Mixed7 encoding conversion with exact
   counters, original SessionKeys/SID and zero constructor/packet calls for the
   handoff itself. Native generation bumps once for actual encoding conversion;
   duplicate controls do not reseal. Validate cuts/previous IDs against genuine
   readable/pending/quarantined rows and observed source watermarks.
6. Selected A is receive-only until durable signed Committed; B may already send
   Event2 after its Active commit. Lost Committed and restart must preserve this
   asymmetry. The same queued request materializes once after activation. Late old
   packets retain IDs/bytes, correct Receipt1/Receipt2 semantics and exact debt.
   Renewal changes current permission without rewriting selected consent/proof/Q.
7. Per-C paid transition, pending/head and old receipt debt; release only genuinely
   unused legacy escrow once while preserving current obligations and independent
   C2 funding. Unified bounded history keeps view identity and the old cursor's
   exact frozen suffix across handoff, append and restart, including ordinal512+.
8. Two C can use different stages/protocols with existing global permits/mailbox/
  64–64 shares and current policy. Keep DirectOnly while any enabled C requires
   L1 direct. Existing own-relay route behavior becomes available only after all
   relevant enabled contexts have completed the protocol transition; no new route
   redesign or relay fallback. Specify the accepted IPC3 tags7–10/locator2 shapes
   and explicit status/errors rather than permissive placeholder success.

The initial dispatch is source/document-only until root accepts that mapping and
its verification allocation. Implementation must follow the existing contract;
if the actual S5.2 source exposes a contradiction, report exact source/contract
pointers and a minimal resolution, not a silent protocol change.

## Verification plan and reuse

Plan focused component and real-process checks for the new transitions and shared
branches. The minimum observable scenarios are:

- Lost P, A, K and Committed at actual durable-before-publication windows, fresh
  authenticated reconnect, crash/reopen and exact retry. Include Selected/Prepared
  without symmetric Q and Selected receiving B's Event2 before A may send.
- Nonempty old receipt debt and old gap/pending/head delivery through pre-Q and
  selected stages, exact-once credits, no fabricated receipt or cleared block.
- Current permission expiry/revocation/renewal, stale lease/final gate, invalid
  scope/cut/format/role/known-Q, prohibited post-P ALPN1 and cancellation boundaries.
- Real S5.2 stopped exhausted checkpoint: preserve Account/Session/keys/D/E/SID,
  actual cutA512 and actual cutB, then first genuine Event2 at A.n512. Prove old
  request retries/history/receipts survive. This is continuation of the existing
  domain, not a fresh-session throughput test.
- C1 transition/failure with independently permitted C2 progress; mixed modes and
  bounded resource shares, then already-supported route/restart checks as affected.
- History cursor suffix across transition, append and restart; typed state,
  Native encoding, escrow/debt atomicity and changed fresh-path regression gates.

Use exact S5.2 accepted checkpoint provenance and stopped isolated copies. Never
open its frozen original, run concurrent counterfactual copies of one Native domain,
regenerate511 as free setup or claim the handoff seed is itself whole P5. If no
usable accepted checkpoint exists, root resolves the dependency before spending
another long generation. Preserve all historical attempts/evidence/ledgers.

The prior plan's later C350–550/F650–1100 values are estimates, not a budget
allocation. Re-estimate against actual S5.2 consumption, changed code and seed reuse;
root grants an explicit incremental/cumulative tranche before execution. Separately
executed compile-only/build/stage/Clippy/fmt are recorded static-only; every fixture,
copy, wait, failure, recovery and cleanup belongs to its contributing runner.
Use locked/offline verification, Jobs12/BelowNormal, stable Windows harness/field
EXE paths and source/image hashes. Parallelize prepared independent cases only.
Do not rerun unchanged bulk100k or already-covered suites without a changed branch.

## Submission, callback and exclusions

For mapping review, callback root `01a164ec-589c-7b41-bc34-3ba158d1f22b` with the
plan path/hash, actual baseline/checkpoint, scope decisions and budget proposal,
then stop. Root handles acceptance/corrections and implementation dispatch without
asking the human to copy responses or reconfirm the agreed stage.

Implementation later submits `long-lived-dialog-a3-migration-s5-handoff-implementation.md`,
source-bound evidence/ledger, exact candidate allowlist and unresolved limitations,
then stops before commit/push. Follow the same accepted publication procedure.

S5.3 is not whole S5/MVP acceptance. S5.4 remains the next distinct validation cut:
one continuous whole P5 plus required final focused/static checks and exact review.
This forward agreement selects S5.3; it does not start/fund S5.4 or public rollout.
GUI, groups, multi-device, mailbox/onboarding redesign, M1.2.63, dependency upgrades
and deferred crafted A2 relay DoS are outside this assignment.
