# A3-S5.1 storage implementation — review 2

2026-10-26. Root decision: **changes requested**, one remaining compatibility
finding. Revision 2 is not accepted or authorized for publication. This document
authorizes the bounded R4 repair and verification below in the existing GPT-6.1
Sol Max executor chat, normal speed, no Fast. S5.2 remains outside this assignment.

## Reviewed package and closed findings

Baseline/HEAD: `3547cf528f15c15a21cdb9e30b1d7ca35a01297a`; index empty.
[Revision-2 report](long-lived-dialog-a3-migration-s5-storage-implementation-revision-2.md)
SHA256 `d21473a5c0efdf01338d691165916065bb2070c6f6135e7b967befbc3f5b60b9`.
[Revision-2 manifest](long-lived-dialog-a3-migration-s5-storage-candidate-files-2.json)
70347 bytes, SHA256
`240e13d9e3a7d7f73782d28b5bdbd1e3a7bf5c8e60634254ed0bc7a560d2bc79`.

Root independently checked all 203 listed sizes/raw hashes/normalized Git blobs
and exact 204 changed paths including the manifest. All 124 protected files match;
the original 29 ledger contributors remain identical and the submission-1 ledger
snapshot matches its original manifest hash. All 93 evidence JSON files parse.

- R1 closed: `bounded_ordered_profile` checks borrowed manifest/generation/index
  lengths before copy/decrypt. Recovery reuses its authenticated index. Root read
  the new prepared-journal regression and the unchanged extracted generation
  authentication body. Current-source recovery tests pass 4/4.
- Original R2 closed: local/peer locators now verify static signatures and pins;
  peer authority verifies signature/encoding/account/path/registration. Coherent
  cross-C and foreign-account substitutions fail before Fence and on resumed
  source before Ready. Expired locator/revoked authority positive controls retain
  exact historical bytes and zero Native packet/constructor calls.
- R3 closed for revision 2: storage invariants rerun 2/2 on final source, owner/core
  7/7. Root independently matched all five final case source manifests (33 files
  each), all 439 field-build inputs before/after/current and six fixed/frozen images.
  Strict Clippy evidence and static checks pass.

The process evidence is accepted as coverage of five independent scenarios on
the same candidate, not a single passing aggregate: the full attempt passed three
and failed two at the missing FixtureDirectory precondition; the focused attempt
then passed those two using the same source/image and three explicit skips. Root
read both complete outcomes and the saved runner. The failed attempt is retained
and charged. Six parent-alive/kill/wait boundaries and capacity/version controls
are present. No whole-P prefix/suffix stitching is claimed or accepted.

The post-power-loss seed check uses new copies and verifies exact source IDs,
Native/fold/history and unchanged original database hashes. Root did not itself
open any fixture database or run tests/builds. This review remains source/evidence
inspection plus independent integrity checks.

## R4 — P2: static fork verification rejects a real old-writer artifact

At [provenance.rs](../../apps/kilogram-cli/src/ordered_dialog/storage_migration/provenance.rs)
lines 168–179, the new `Init`/`Confirm` fork-witness branch requires the expected
`core.role` for both old and conflicting proofs.

The old production capture boundary is less restrictive for the conflicting proof.
With genuine A0 already saved on B, A can change that signed Event's `core.role`
from 0 to 1 and re-sign it while retaining the Native header/ciphertext. The frame
handler at [runtime.rs](../../apps/kilogram-cli/src/ordered_dialog/runtime.rs)
lines 678–686 checks the outer C/D/E and calls `Engine::confirm`. At
[engine.rs](../../apps/kilogram-cli/src/ordered_dialog/engine.rs) lines 668–680,
the signature, C/D/E and genesis kind/ordinal/request/parents pass, then the changed
Event ID causes a durable `Init` signed-fork witness before a Guard role check.
`Core::parse` allows either role independently of genesis kind, and `check_scope`
does not check role. The analogous `Confirm` path is engine.rs lines 760–771.

Consequently this source is an authentic production-created blocked profile with
genuine readable Events/Native, yet migration refuses the whole profile with
`fork Event scope`. Scoped C1 history also fails. The
[accepted contract](long-lived-dialog-a3-migration-s5-contract.md), section 2
source matrix, explicitly preserves valid signed-fork blocked C, its evidence and
history, and permits C2 after storage Ready. A conflicting signed artifact need
not be a valid readable message to be valid evidence of the conflict.

This finding was independently checked by the read-only reviewer and root against
the real caller/capture/parser paths; it is not a claim that root ran a reproducer.
No additional actionable finding arose in the remaining reviewed locator,
authority, current-auth, extra-control, catalog or sentinel changes.

### Required narrow repair

Separate the authentic saved proof from the newly captured conflicting proof for
`Init` and `Confirm`. Validate the old proof against the genuine saved genesis;
validate the new proof's signature and the static scope/genesis conditions actually
enforced at the old capture boundary. Retain the conflicting role as evidence.
Do not relax readable Event/Native/Head verification, current live authorization,
or signature/context checks globally. Do not clear the blocked state or promote a
fork proof into readable history, pending delivery or a Native replay.

Add a focused regression using the supported old Engine capture path and a copy
of a genuine stopped source, covering both Init and Confirm role conflicts. Assert
the writer really persisted the signed-fork witness, no readable/Native mutation,
exact retained evidence/history through migration and reopen, and preserved block.
Include a negative wrong-signature or foreign-context witness so accepting an
authentic conflicting role does not become opaque archival acceptance. Reuse the
existing C2 isolation coverage with a precise branch argument, or include C2 in
this focused regression if it fits the allowance without a new long origin run.
No product fix to the old writer is requested in this migration task.

## Explicit verification budget amendment for this repair only

Root independently recomputed 38 contributors: C = 279.3257298/600 seconds,
F = 1181.4272386/1200 seconds, P = 0/2400 seconds. Revision 2 stayed within the
original ceiling, leaving 18.5727614 seconds of F.

That remainder is too small to promise a meaningful new real-source regression
including the stable wrapper. Root therefore explicitly adds **120 seconds to
the cumulative S5 F allowance for R4**, before any new run: the revised cumulative
ceiling is **F <= 1320 seconds**. Current available time is 138.5727614 seconds.
C <= 600 and P <= 2400 remain unchanged. This is an amendment to the verification
budget only, not a retroactive PASS, ledger reset, product-contract change, or a
budget for S5.2/whole P5. The prior normative contract and accepted documents stay
byte-for-byte unchanged; this decision records the exception separately.

Use one targeted final-source fork regression group, with all fixture work,
failures and complete runner overhead charged. No repeated full owner/process
suite, separate two-C field repetition, full511, 100k or whole P5. If the forecast
cannot fit, callback root before running. All previous contributors remain.

For unaffected results, provide a function/branch diff showing the repair changes
only the fork-evidence branch and its tests; the earlier unblocked-origin process
scenarios may be reused on that basis. Refresh final source/build correspondence
and fixed field-image provenance, affected strict Clippy/fmt/diff/static checks.
Use locked/offline verification, Jobs12/BelowNormal, compile-only Cargo plus the
stable Windows runner and fixed image paths. Never recopy a live EXE. Root review
will not require a full expensive rerun merely because one archival branch changed.

## Resubmission boundary

Preserve revision-1/2 report/manifests, all prior per-case/static evidence, both
root reviews and the five earlier root-owned task/navigation paths. Before changing
the cumulative ledger, preserve its revision-2 snapshot under a new filename.
Use new case IDs and new evidence names; do not rewrite earlier correspondence or
validation reports. No unrelated work, accepted document or frozen image changes.

Submit `long-lived-dialog-a3-migration-s5-storage-implementation-revision-3.md`
and `long-lived-dialog-a3-migration-s5-storage-candidate-files-3.json`, with the
same exact raw-size/SHA256/normalized-blob inventory convention. Include this
root-owned review. Callback root chat `01a164ec-589c-7b41-bc34-3ba158d1f22b` with
R4 behavior, evidence, reuse justification, cumulative budgets and manifest hash.
Stop for review. Commit/push requires root acceptance and the exact publication
assignment; neither is given by this document.
