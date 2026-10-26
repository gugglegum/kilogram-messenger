# A3-S5.2 implementation review 1 — changes required

2026-10-26. Root review against published baseline
`73c499818ba9b699c331aa842e64e8432e90c59a`, the unchanged
[S5 contract](long-lived-dialog-a3-migration-s5-contract.md), accepted
[plan revision 2](long-lived-dialog-a3-migration-s5-l1-plan-revision-2.md), and
[implementation assignment](long-lived-dialog-a3-migration-s5-l1-implementation-task.md).
This is a correction assignment within cut2, not implementation acceptance.
The user's standing delegation and review/fix authorization applies.

Root independently checked the three findings below, following parallel read-only
protocol and state reviews. Reviewers made no edits, builds, test runs or DB opens;
their assigned source hashes were unchanged during review. The candidate field
image is build09 SHA256
`4f4c240f10e90f86aaae5bf087bb4056280e7da762d52c68ebfeb24432542d98`.

## R1 — preserve applicable own finalization captures

`apps/kilogram-cli/src/ordered_dialog/runtime7/bootstrap.rs:30` emits
`(admission, final_auth, receiver) = (None, None, None)` for every own readable
event, including Text through `runtime7/data.rs:132`.
`crates/kilogram-state/src/vault/indexed_dialog/migration7/live1/data.rs:196`
also rejects own `final_auth=Some`, and `live1/history.rs:322` requires complete
captures only for received events. Consequently an own post-Ready event is
reported as FullL1 without its applicable F link.

Concrete distinguishing case: migrate genuine Offered A or Granted B, advance
authority legally, then complete bootstrap. Genesis O still comes from the
persisted InitialPair (`bootstrap.rs:302,402`), while the actual current F can
differ. The current writer loses that distinction. Even when O and F bytes are
equal for an ordinary own Text, an absent F is not an exact persisted F link.
S5 section 3.3, especially the own Text/genesis rows and its post-Ready rule,
requires this capture.

Persist the actual current own F atomically with the transition; deduplicate
identical bundle bytes normally, while retaining the separate role link. Keep
own A and not-yet-applicable R semantics intact. Update the closed state shape
and full proof reader to require own F, without weakening peer O/A/F/R checks.
Do not infer F later from O, pins, time or a remaining bundle. Do not edit old
candidate fixtures to manufacture F or relabel their new rows LegacySnapshot.

Regression coverage must distinguish genuine partial-bootstrap Oold/Fcurrent
after authority advance, ordinary own Text F, and rejection of a new full proof
with missing F. Cover both affected own-genesis roles, using focused component
coverage where sufficient and genuine public field origin for the integration
claim. Imported immutable LegacySnapshot records remain unchanged.

## R2 — enforce the L1 unacknowledged-message bound before Native

`runtime7/data.rs:101-146` materializes up to four queued rows without considering
current debt. `migration7/live1/data.rs:384-408` likewise accepts the batch and
creates Undelivered rows without checking the 128 limit. Queue128 and the total
L1 lifetime do not enforce this independent bound.

An admitted peer can withhold receipts while the sender keeps accepting and
materializing requests. In particular, debt127 plus queue4 currently permits
four encryptions and debt131. S5 section 6 explicitly retains unacked<=128 for
L1; only later Active2 may expand the limits. The old outbox stopped at128.

Before restoring/forking Native or invoking packet encryption, limit eligibility
to `min(4, queued, 128 - debt)` for the same C. At the limit leave the accepted
queue durable and return without packet work. Independently enforce the final
debt bound in the closed state materialization purpose, including multi-row
atomic operations. Do not change the lifetime, refill escrow or count transport
Complete/possession as a receipt.

Regression: debt127/queue4 gives exactly one new packet and debt128/queue3;
another turn and cold reopen preserve the remainder without extra Native work;
one actual exact receipt frees one slot and permits one further materialization.
Include a closed-state over-limit rejection and unchanged durable state on
refusal. Positive Native/Event integration claims require genuine lineage;
synthetic negative unit inputs must be identified as such, not field evidence.

## R3 — distinguish malformed Ready refusal from authenticated fork evidence

`runtime7/bootstrap.rs:537` treats any signed Ready body unequal to the expected
body as a permanent signed fork. This includes wrong arity, D/E/SID, role or
suite. The generic bounded Block purpose does not restore the missing check.
An admitted peer's wrong-scope or malformed Ready can therefore permanently
block an otherwise valid C, even without a same-scope genesis contradiction.

Preserve the existing `Engine::ready` admission boundary (`engine.rs:839`):
first check exact shape, scope, peer role and suite; only then classify conflicting
genesis IDs as fork evidence. If a prior peer Ready exists, preserve it as the
first witness, falling back to the appropriate saved genesis only when absent.
Keep genuine same-scope conflicts blocked across renewal/reopen.

Regression: correctly signed wrong-scope/shape controls refuse without changing
block/Native; valid Ready remains usable afterwards. A genuinely same-scope
conflicting genesis still creates the correct durable fork evidence. Exercise
the public dispatch/owner path, not only a duplicated parsing helper.

## Preserved results, scope and exact reviewed inputs

The successful `F-obligations-resume-03` remains an immutable PASS for its actual
resumed assertions. It did not test own F or the 128-debt boundary and is not
acceptance of this whole implementation. Case SHA256:
`15c9ffdadd714f513807c09eb9b6d09ee9ec3cdc4728812dbc4d6e392372bd2c`;
parent-result SHA256:
`b58f053dd76028e612c7177f2a74349e6e41457f4002ab5ab38aefa58d7c918e`.
The exact default8 phases were pending64 in46.2018257s and shared drain65/ACK
in54.3150724s. These correctness waits do not establish acceptable user latency.
All earlier failures and their charges remain unchanged.

Reviewed source SHA256 anchors:

| File | SHA256 |
|---|---|
| runtime7/bootstrap.rs | ee7a6a2d1d72e0ad8ace0a7b33292af25c2dd510c1b92012695d66c5edb96332 |
| runtime7/data.rs | 0b39c92c14dffff2c9ef80d8eee2cd2864b69fb6b6a063ac82e05e1ab404be94 |
| migration7/live1/data.rs | 80d1ec4affd7e071fffbb4d968bb6dd69056d6d6ee9deb18fe5e2ac147c5ee1a |
| migration7/live1/history.rs | 341f6bc6609c1702db5adbe28eea57b6c06a92f96bd8168c79a5a7227f6e074c |

No further confirmed defects were found in the assigned remaining protocol,
renewal, IPC, policy-token, pending/head, quarantine, receipt, credit, bounded
history and loading seams. This is bounded static review, not a security proof
or a claim that unexecuted regressions passed.

## Authorized correction and verification order

1. Finish and account for any already-running contributing attempt before source
   edits. Defer the two previously authorized small gates if not yet started;
   execute them later against the corrected candidate. No new F is authorized by
   this review. Keep a full record of any attempt that already started.
2. Implement R1-R3 and their focused regression controllers. The contract and
   cut2 scope are unchanged. Preserve original fixtures and previous evidence;
   use genuine unchanged origins for new positive runs, with one active isolated
   branch per Native domain. A rejected candidate's incomplete FullL1 rows cannot
   be silently repaired or presented as accepted history.
3. Run recorded static compile-only/build/Clippy/fmt as needed, keeping one serial
   source/build/staging lane and fixed EXE paths. Existing build09 is not evidence
   for changed production functions. Preserve all predecessor images.
4. Within the existing C500 ceiling, one initial focused C regression group with
   a planned whole upper bound120s is authorized after static checks. Record exact
   selectors and require actual matching tests. Windows tests use the stable
   wrapper, locked/offline verification, Jobs12/BelowNormal. Preserve failures;
   a failure or overrun requires callback with a proposed focused continuation,
   not automatic repeated suites. Do not relabel F fixture generation as C or
   static work to fit this allocation.
5. Callback root with fixes/source hashes, static/C results, exact genuine field
   regression selectors and provenance, affected-check reuse assessment, and a
   revised whole-run forecast for remaining F. Include isolation, affected S5.1
   capacity, new regressions, origin850/boundary140 and contingency225. Do not run
   F or long511 until root reviews that concrete package and budget. If the sum
   does not fit, state the necessary additional allocation explicitly; do not
   reduce assertions or borrow later P time.

At this review's ledger read, S5.2 actual C105.3761137/F925.0234269/P0; current
ceilings C500/F2400/P0. This document does not reset charges or raise ceilings.
Any already-started concurrent case must be added before the next decision.
Long511 still requires separate readiness approval and its existing exact
runner/ledger binding, work850/whole900 guard and minimum remaining1215.

No commit/push, cut3 or whole-P run yet. After corrected implementation/evidence
acceptance, the standing exact allowlisted publication procedure and the queued
S5.3 assignment still apply. Preserve this root-owned review as a new input;
do not rewrite earlier reviews, historical evidence or the future S5.3 task.
