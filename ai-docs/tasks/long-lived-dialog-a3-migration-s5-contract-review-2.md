# A3-S5 contract review 2 — accepted for exact documentation publication

2026-10-25. Baseline/expected publication parent:
`0c1baa887008f784a86fd7ebabdcd5145fb4d1c0` on master.

Executor revision reviewed: 109621 bytes, SHA256
`a459a1aac6254ec7ad4473b12897b0864a72c8574f4a5709d7ac22316fc46719`,
Git blob `a92a55eb2530b24981c92c980733748464397315`.

Root made two editorial changes before acceptance: updated the document's status
to this acceptance and made the pseudocode tie explicitly choose from the filtered
`continuations` set. It must never select a non-continuation merely because it is
the own-lane candidate.

Accepted [contract](long-lived-dialog-a3-migration-s5-contract.md): **109805 bytes**,
SHA256 `f87c5c8b0b307863078cf3e2e85f3d13a05392d1d654affd5f2a40a3943ad2ad`,
Git blob `23d7d6d094345d80352e7342d0f12848859fb295`.

## R1–R4 disposition

- R1 closed: intent is separate from LocalMode7; named projection preserves wire
  stages1–5 and role/Q/cut shapes. Incomplete migrated bootstrap cannot impersonate
  fresh Runtime6 or advertise an invented existing scope.
- R2 closed: VerifiedExistingScope1 and current Consent2/lease evidence support
  pre-Q admission without a fabricated ModeProof. Receipt1 and source-head dispatch
  have explicit pre-Q branches. Selected/Prepared reconnect can retransmit K while
  the peer has no Q; hints do not select a proof or authorize Event2 at Prepared.
  ALPN1 eligibility ends at durable P/Prepared, independently of receive/ACK repair.
- R3 closed: LegacyBudget leaves23/47 contribute actual summary reservations;
  Root7 load and every commit validate correspondence. Source backing is charged
  once to global owner0 and cannot cover context1 escrow. Ready moves the prepaid
  endowment into the pool once. Capacity changes use an identity-preserving,
  authenticated monotone CAS and a revision-bound publication plan.
- R4 closed: the two-frontier fold has bounded causal/preflight selection, explicit
  continuation priority and exact final Guard/Seen comparison. It is a summary
  validation operation, with no Native replay or reconstruction of packet keys.

Root independently checked these changes against the current Guard2/import_l1,
ModeView2/Receipt1, selected-only runtime head path, Root/Ledger summaries and
leaf-charge constants. This is architectural acceptance, not a claim that the
new implementation or its fault cases have passed.

## Scope and verification obligations retained

D1–D4 are accepted as specified in the contract. LegacySnapshot remains a closed
migration-only provenance variant; unknown old A/F never become a full proof or
current permission. Component5/Runtime6 and Native config1 remain separate and
unchanged. The source is a verified V4 primary under a single lifetime lock; the
same-DB fence is forward-only. Existing Native identity, history and delivery debts
must survive, and current authorization is independently checked during live work.

D5 remains C<=600s, F<=1200s and whole P<=2400s. The F estimate790–1260s is a known
planning risk, not a waiver. Charge fixture generation, short feasibility, copies,
kills and runner overhead; return measured feasibility before a long511/full run
if the remaining bound cannot be met. No silent repetitions, free setup, stitched
whole-run success or inherited S4 3000s exception.

This contract acceptance does not complete S5, A3 or MVP. Indexed L1 interoperability,
existing handoff, mixed history/cursors and the whole resumed P5 still require
production implementation and independent review. Deferred M1.2.63, GUI/groups/
multi-device and the A2 crafted relay regression are not included.

## Exact publication boundary

Publish only the paths/blobs in
[accepted manifest](long-lived-dialog-a3-migration-s5-contract-accepted-files.json)
plus that manifest itself, as one ordinary commit and fast-forward push to
origin/master. Check the raw hashes before staging, exact normalized staged blobs,
sole parent and exact committed path set. Do not amend files/evidence, broaden
staging, force-push or start implementation in the publication turn. Report commit
SHA, push result, remote tip and callback to root.

This uses the user's standing instruction for ordinary commit/push of an explicitly
accepted package; no renewed human approval is required. Root independently verifies
the actual remote commit and accepted diff before assigning implementation.

The next intended bounded slice is contract section12 cut1: infrastructure controls,
production V4 preflight/origin verification, typed Root7 fence/copy/Ready and the
offline migration/read-only administration path, with focused integrity/quota/
version/kill evidence. Its exact assignment and baseline follow remote verification.
Do not begin that work on this documentation publication instruction alone.
