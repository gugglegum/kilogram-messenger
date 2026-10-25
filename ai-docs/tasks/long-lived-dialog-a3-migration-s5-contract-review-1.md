# A3-S5 contract review 1

2026-10-25. Root review of the contract against accepted S4
`0c1baa887008f784a86fd7ebabdcd5145fb4d1c0`.

Reviewed input: [S5 contract](long-lived-dialog-a3-migration-s5-contract.md),
72768 bytes, SHA256
`c99ad8f15810348257ea9d5ef0eebe173cbc32c11e5175700ba3cb0d7c96b495`,
Git blob `f6a6081d094cc2080c553728ee2e277d72327b10`.
Line references to the contract below describe this input revision.

**Decision: changes requested. No implementation/build/migration/fixture generation
or commit/push is authorized by this review.** Revise the contract only and return
it for another review. The findings concern concrete implementation seams; there
is no request to rewrite the accepted A3/S3/S4 architecture or broaden S5.

## Decisions already resolved at this review

- D1: accept the direction of a distinct authenticated MigratedRuntime7 discriminator
  inside schema5, separate purpose and marker, same-DB forward-only fence, and
  unchanged Component5/Runtime6 encodings. The final contract is not accepted yet.
- D2: the explicit LegacySnapshot applicability table correctly distinguishes
  unavailable historical A/F from a receipt that has not happened yet. Preserve
  the closed migration lifecycle, exact source membership, mandatory O/receipt
  checks, and actual post-Ready captures from
  [provenance decision 1](long-lived-dialog-a3-migration-s5-provenance-decision-1.md).
- D3: B4=141.125MiB/C matches the actual old reservation at
  [ordered.rs](../../crates/kilogram-state/src/vault/ordered.rs#L630).
  Keeping unused legacy capacity and separately funding the new repair pool is
  accepted in principle; R3 below must close its durable enforcement seam.
- D4: accept DB-wide offline copy followed by independent per-C handoff, one owner,
  explicit ALPN families and DirectOnly until all enabled contexts support L2.
  The asymmetric reconnect rules still need R2 below.
- D5: retain C600/F1200/P2400. No 3000s exception and no uncharged fixture setup.
  The short charged feasibility step is authorized only as part of a later
  implementation assignment. Its measurements may motivate a separate root
  budget decision before a long run; the present estimate is not a test result.

## R1 — wire Mode2 numbering must not inherit local Intent numbering (P1)

Contract lines296–297 say that Mode7 corresponds to wire Mode2, then assign
`0=L1Active,1=Intent`. The accepted
[Mode2 encoding](long-lived-dialog-a3-contract.md#L471) instead assigns
`0=FreshHandshake,1=L1,2=Offering,3=Prepared,4=Selected,5=Active2`.
[ModeView2::parse](../../crates/kilogram-protocol/src/ordered_dialog_v2/mode.rs#L557)
accepts numerical stages, so this is not caught by the wire parser.

Define an explicit total local-to-wire mapping. Prefer a separate local intent
record/flag with both L1Active and Intent projecting to wire stage1; retaining
different local enum numbers is also possible with a closed conversion. Native
phase0–3 remains separate. A migrated existing context must not advertise
FreshHandshake merely because no upgrade intent exists. Specify what is advertised
for incomplete L1 bootstrap and which operations are unavailable then.

Add positive stage-encoding assertions and wrong-stage/shape negatives to the
component matrix. Never serialize a local discriminant by a raw cast.

## R2 — pre-Q and asymmetric reconnect need a complete typed dispatch (P1)

Contract lines429–434 correctly avoid calling the S3 Event verifier without a
selection, but only describe the data verifier. The same issue affects receipts
and the source-head/control loop:

- [ParsedReceipt1::verify](../../crates/kilogram-protocol/src/ordered_dialog_v2/authority.rs#L439)
  also requires StaticSelection2 and cannot verify a pre-Q receipt by itself.
- [runtime2 accept_head](../../apps/kilogram-cli/src/ordered_dialog/runtime2/network.rs#L307)
  unconditionally requires a selected proof and exact peer Q/cuts.
- [ModeView2::check_selected](../../crates/kilogram-protocol/src/ordered_dialog_v2/mode.rs#L570)
  rejects a Prepared peer's absent Q when the local A is already Selected.

After a lost K or a crash between A Selected and B processing K, fresh admission
and control retry must work while these durable states differ. Blindly reusing
the current head check would deadlock the accepted recovery trace. Passing a
dummy/peer-chosen Q would weaken the selection boundary.

Add one compact dispatch/compatibility table covering local L1/Intent, Offering,
Prepared, Selected and Active versus the actually reachable peer stages. Specify:

1. The current Consent2/Root/lease admission evidence that is sufficient before Q,
   independently of an installed ModeProof. Complete Ready1/genesis and the existing
   scope must still be verified; this does not confer materialization permission.
2. Typed pre-Q handling of Resume/Head/Prefix/Data1/Receipt1, not just Event1.
   Preserve source/receiver signatures, exact immutable source binding, capture
   rules, old ordinal limits, and durable monotonic legacy watermarks.
3. Local Selected + peer Prepared/absent Q: which hints/control/old data are
   accepted to finish K/Committed, which claimed Q/cuts cause rejection, and why
   no peer hint installs local Q or enables Event2 at Prepared. In particular,
   admission/control recovery must not require symmetric selected hints first.
4. The exact ALPN1 refusal boundary after local P/Prepared. Lines425–427 and
   486 currently emphasize different boundaries. Make transport eligibility
   and the irreversible send freeze separate explicit rules.

Exercise lost P/A/K/Committed with real reconnects, including a nonempty
Receipt1 debt and source-head exchange before Q. Existing M06/P5 may carry these
assertions; do not add an unnecessary independent long suite.

## R3 — make LegacyHold part of authenticated per-C quota enforcement (P1)

The new Root7 deliberately preserves the common ledger encoding. The current
[Ledger::check](../../crates/kilogram-state/src/vault/indexed_dialog/credits.rs#L157)
accounts for used/reserved/pool/head, while
[Root::totals/check](../../crates/kilogram-state/src/vault/indexed_dialog.rs#L316)
derives used/reserved from tree summaries. There is no separate LegacyHold field
in that ledger. Contract lines206 and358–380 name LegacyBudget7 and a scalar
formula, but do not yet state how its escrow becomes an enforced authenticated
summary liability on every load and commit.

Define the small LegacyBudget7 record, its owner/key/lifecycle and exact
used/reserved contribution. State the per-C equations explicitly, with a mapping
from physical retained bytes and per-context obligations to charged owners.
In particular, source backing charged to slot0 must not silently stand in for
slot1 escrow, and the same source byte/ticket must not be subtracted twice.
The sum protected by Root7 checks must include LegacyHold; storing a number
only in an unchecked payload/status calculator is insufficient.

Specify the atomic adjustments for old queue/pending/receipt transitions and
Selected/Active escrow release, plus the funded/never-refilled distinction for
occupied versus unused slots. Cover quota just below need, clean reopen, ordinary
L1 append, delayed receipt and C2 activity with unchanged outstanding obligations.

Also resolve the configuration seam: Intent7 stores an immutable requested-config
digest while section6 permits capacity increases and exact resume. State which
fields are identity-pinned and which capacity fields can change, how the change
is authenticated/CAS-bound, and which effective config the final publication
spec contains. No new migration ID, second repair endowment or silent retargeting
of owner/paths/contexts is permitted on a capacity retry.

## R4 — specify the deterministic source fold, not a search requirement (P2)

The two-frontier direction in lines124–131 is reasonable, but the proposed
production origin verifier still lacks an executable ordering rule. The existing
[import_l1](../../crates/kilogram-ratchet/src/ordered_v2.rs#L505) only checks an
already-ordered list; it does not recover that order from the two stored lanes.
The source contains no durable global chronology. A wrong merge may reject a
valid old state or change Active/Inactive provenance despite valid per-lane rows.

Add bounded pseudocode for choosing the next event from the two authenticated
readable frontiers using previous/parent/ClosedTail and Guard preflight. State
the tie rule, why choosing a known-chain continuation does not skip a necessary
causal dependency, the no-progress rejection, and the final exact Guard1/seen
comparison. This is a deterministic validation fold, not Native replay, DFS,
timestamp reconstruction or an opportunity to manufacture source rows.

Add small cases for multiple own continuations before a new peer first,
late peer continuation after an own new first, pending rows excluded, incomplete
genesis prefixes and equal-looking per-lane summaries with an invalid dependency.
Keep this within the charged component budget; no full511 test is needed merely
to specify the algorithm.

## Return boundary

Revise only the contract; preserve this review and all root-owned task/navigation/
provenance files and historical evidence. Return the revised byte count/SHA256,
an R1–R4 disposition and local-link/diff checks. No product code or builds now.
After the final contract is accepted, the root will provide an exact documentation
publication allowlist, verify the resulting remote commit and assign the first
bounded implementation slice. S5 remains incomplete until the whole migration,
indexed L1 compatibility, existing handoff and final P/F/C acceptance are done.
