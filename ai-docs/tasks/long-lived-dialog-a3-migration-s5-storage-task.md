# A3-S5.1 — production storage migration core

2026-10-25. User requested S5; root has accepted and independently verified the
published architecture contract. This is the first bounded implementation assignment
to the existing GPT-6.1 Sol Max executor, normal speed, no Fast.

Baseline/HEAD/master/actual GitHub master:
`3547cf528f15c15a21cdb9e30b1d7ca35a01297a`, sole parent
`0c1baa887008f784a86fd7ebabdcd5145fb4d1c0`.
Root checked all9 accepted publication paths/blobs, raw hashes, sole parent and clean
worktree/index; independent GitHub compare to master returned identical/ahead0/behind0.

Normative inputs:

- [Accepted S5 contract](long-lived-dialog-a3-migration-s5-contract.md), especially
  sections2–6, the migration/admin parts of8, and the storage portions of10–12.
  SHA256 `f87c5c8b0b307863078cf3e2e85f3d13a05392d1d654affd5f2a40a3943ad2ad`.
- [Review2 acceptance](long-lived-dialog-a3-migration-s5-contract-review-2.md),
  [review1 findings](long-lived-dialog-a3-migration-s5-contract-review-1.md),
  [historical provenance decision](long-lived-dialog-a3-migration-s5-provenance-decision-1.md).
- Existing accepted A3/S3/S4 contracts and source boundaries referenced there.

## Authorized implementation

Implement section12 cut1 as a coherent storage/admin change. The contract is already
accepted: a further planning-only document is not the requested output. Begin the
necessary product code and focused checks. Report a substantive contradiction before
changing accepted wire/storage semantics or replacing the design.

1. Implement production V4 preflight and bounded source verification under one
   lifetime state-directory lock. Resolve supported old recovery before freezing
   the source; preserve primary authority, existing owner/keys, optional phases,
   selected controls and actual Native provenance. Build the deterministic readable
   source fold and exact Guard/Seen verification now, since they protect migration
   readiness; do not defer this validation to the later live handoff slice.
2. Add closed MigratedRuntime7/purpose/namespace handling, immutable source witness
   and bounded V4Ref access. Preserve Component5/Runtime6 codecs and fresh runtime
   semantics. Fence+Intent+mode replacement are one redb commit; source bytes stay
   immutable except the specified saved-mode override. No second writable domain.
3. Implement bounded resumable copy/verification/Ready, current-pointer retirement,
   migration-only LegacySnapshot creation and its lifecycle closure before retry.
   Preserve IDs, sealed bytes, requests, queue/pending/receipts and historical
   provenance without manufacturing A/F links or Delivered. Zero packet crypto or
   Account/session/OTK constructors during copy/conversion.
4. Implement authenticated per-C LegacyBudget, source-backing accounting and
   effective-config capacity CAS, including one-time Ready funding and exact
   publication recovery. No refill on reopen, extra endowment on retry or silently
   changed owner/contexts/paths. Caller-provided totals/trusted-origin booleans are
   not verification. State stays independent of protocol/ratchet dependencies;
   private owner and structural state responsibilities stay explicit.
5. Expose the offline storage check/resume/status/pause/read-only old-history and
   capacity administration path with bounded typed IPC as necessary. A live peer
   runtime must not start on Copying/Paused/Blocked or a partially implemented
   Runtime7. Ready is a storage state; until S5.2 provides the owner, clearly report
   the unavailable live runtime. Do not route Root7 to the old mutable Engine or
   fresh Runtime6 as a temporary compatibility fallback.
6. Repair and verify the Windows writable-marker/sync/atomic pause hooks before
   relying on them. Keep sync and atomicity. Do not modify the frozen d9b image;
   real partial d9b origins use the control-peer method in the contract. Parent-side
   evidence must prove the child was alive/paused and then actually killed.

Deferred to subsequent separately assigned/reviewed slices: live indexed L1 owner,
ALPN1/2 interoperation, completion of initial handshakes, existing Consent2 P/A/K/
Committed machine, mixed live ingress/receipts, full exhausted511 continuation and
whole P5. Storage types needed for those later operations may be defined, but do
not expose a permissive placeholder or implement the whole handoff in this slice.

## Focused verification and budget

Use new fixtures only. Start with short infrastructure and real d9b source smoke,
then storage cases; do not launch full511, whole P5, S2 100k or blanket workspace
stress runs in this slice. Frozen S1/S2/P/F DBs are not fixtures and must not be
opened. Small stopped-seed branches may be reused only as specified in the contract,
with one writable branch of a Native domain at a time and actual setup charged.

Cover the storage portions of M01/M04/M05/M08/M09/M10/M11/M12 with explicit case IDs:

- Actual supported old origins (including incomplete phase boundaries), exact
  key/Native/ID/history/queue/pending/receipt preservation, source versus stale
  shadow, invalid/corrupt source rejection before fence, zero migration constructors
  and packet calls. Small active seeds suffice here; large511/live drain remain
  later gates and must not be marked passed by a small storage fixture.
- Real process kills before/after fence, middle copy, staged-before-Ready and
  Ready-before-profile; exact resume outcome, identity/incarnation/checkpoint,
  immutable source and closed lifecycle. Selected copy/config/backend IO ambiguity
  must produce a whole old/new outcome and poison/reopen handling, not partial Ready.
- Quota need-1/need, exact per-C escrow/retained backing and funding, clean reopen,
  capacity CAS before/after crash, stale/lower/retarget refusal, required budget
  payload/owner/summary integrity, no second endowment. Live receipt/C2 transitions
  not yet implemented remain explicitly pending.
- Actual d9b/S3/exact S4 cold and live writer fences with correct-owner/config
  positive controls, purpose-before-retry, header/profile relabel and malformed
  Root7 refusal. The predecessor helper must use the accepted archived source,
  not current code compiled with a predecessor name.
- Honest LegacySnapshot membership/applicability, unknown A/F versus required O/R,
  pending admission preservation, corrupted full-proof rejection, no post-Ready
  copy/import/generic creation, bounded read-only history and exact existing IDs.
- Source-scoped fresh Runtime6/component regression checks for shared code touched.
  Do not claim an old PASS proves a changed branch; avoid repeating unchanged heavy
  paths. If a common append/page/allocator change would require100k, callback root
  with that concrete reason before running it.

Maintain one explicit contributing-case/evidence ledger with source hashes and
runner elapsed including fixture generation/copies/recovery/overhead. Contract
C600/F1200/P2400 remains in force for the final contributing sets; do not hide this
slice's reused seed/check cost outside the final accounting. Start with a short
charged feasibility measurement and report the remaining estimate. If required
coverage cannot fit, preserve evidence and request a root scope/budget decision
before the long run. Failed attempts remain recorded; no silent resets/repeats.

Windows: verification profile, locked/offline, CargoJobs12, BelowNormal. Compile
network harnesses with --no-run and execute through scripts/run-cargo-tests-stable.ps1
only. Use the contract's fixed EXE paths and content hashes. Stage images only after
exact-path process inventory shows no live owner. The wrapper recopies EXEs even
in Run mode: shared CLI compile/stage/run/cleanup stays serial. Independent unit
fixtures/prepared distinct harnesses can run concurrently without shared Native
domains, identities, writer locks or EXE replacement. Preserve accepted old CLI,
S3 predecessor, S4 CLI and relay hashes.

Run fmt, locked builds, strict Clippy for affected packages/all targets with exact
argument forwarding, focused tests, diff/link and source-to-evidence checks. No
extra product scope merely to satisfy an overbroad test command.

## Review, publication and callback

Return an implementation report, exact changed-file manifest with raw SHA256/Git
blobs, source/evidence correspondence, test results/timings and explicitly remaining
S5 gates. Preserve failed logs. Keep accepted contract/reviews/manifests and
historical evidence immutable. Do not change development-environment.md (protected
SHA256 `53074ff3734dacd9906ee03cc5c7ded952dedd99d207581f692ff900b9a79f8f`).
Root-owned dirty files from this assignment are this task and README/next-stage/
coordination updates. Preserve them; include them transparently in the eventual
candidate inventory, but do not edit them. No unrelated worktree changes.

Do not commit/push before implementation review. After acceptance root will provide
the exact publication allowlist; the user's standing commit/push instruction then
applies. No new chat, model or Fast switch is needed.

Send substantive completion/blocker callback to root chat
`01a164ec-589c-7b41-bc34-3ba158d1f22b` under the user's direct standing authorization.
Executor chat is `01a1653c-0154-7611-8e8c-044fbe1e4d7a`. Stop at review; the root will
check the diff/evidence, request fixes if necessary and authorize publication.
