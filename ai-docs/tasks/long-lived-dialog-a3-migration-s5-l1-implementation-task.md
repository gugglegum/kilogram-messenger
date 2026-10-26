# A3-S5.2 — implement and verify indexed L1 continuation

2026-10-26. Root implementation dispatch to the existing GPT-6.1 Sol Max executor,
normal speed, no Fast. The user's continuing S5 and delegation authorization applies.
No additional human approval is required to carry out this assignment.

## Accepted inputs and outcome

Start at `73c499818ba9b699c331aa842e64e8432e90c59a`. Root has independently verified
its S5.1 publication. Read [plan acceptance](long-lived-dialog-a3-migration-s5-l1-plan-review-2.md),
[submitted mapping](long-lived-dialog-a3-migration-s5-l1-plan.md) and its overriding
[revision 2](long-lived-dialog-a3-migration-s5-l1-plan-revision-2.md). The unchanged
[S5 contract](long-lived-dialog-a3-migration-s5-contract.md) remains normative.
Their exact hashes are recorded in the acceptance. Do not reopen settled design
questions unless implementation reveals a concrete contradiction; report that
contradiction and a proposed minimal resolution before changing the contract.

The product outcome is continuing the same real L1 dialog after storage migration,
first against the actual frozen d9b peer and then with both indexed owners, including
duplex messages, requests, history, offline delivery and restart. Keep the original
Account/Session/Native/nonce/keys/D/E/SID and actual queues, pending, receipts and
blocks. Ready storage alone is not live permission. This is implementation cut2;
handoff/Selected/ALPN2 and whole P5 are not part of this assignment.

## Authorized implementation

Implement the accepted closed Legacy1 operations and typed indexed facade, sibling
Owner7/runtime7, live policy/admission/transport, bounded history/request access,
strict Profile3/IPC3/CLI and the small offline Consent1 renewal producer. Add the
focused tests/controllers and evidence needed for this cut. Concrete module names
may be adjusted for maintainability without changing the accepted boundaries.

Retain these acceptance conditions:

1. One lifetime lock, held core, serialized owner and bounded mailbox. Live loading
   and access use typed bounded point/range reads, never a mutable legacy Engine,
   migration Inventory reconstruction, generic path writes or full-history cache.
   Separate immutable migration audit/Ready witnesses from mutable current state.
   PrepareL1Indexes7 and InitializeHistory7 are paid, bounded, resumable closed
   operations; never reopen Copy/Verify or create LegacySnapshot after Ready.
2. Continue each real partial phase at its first legal next step. Constructors are
   allowed only for genuinely absent initial objects. Preserve existing objects,
   queue/request identity and signed-fork/quarantine blocks. No fake session/view
   for incomplete bootstrap, synthetic positive Native/Event origins or raised L1
   lifetime. State stays independent of protocol/ratchet authorization.
3. Native/Guard1/Seen/current pointers/projection/captures/credits/debt commit
   atomically, with current lease/policy checked at final commit. Preserve applicable
   post-Ready O/A/F/R, immutable imported snapshots and actual later receipt overlays.
   Delayed exact receipts release only their debt once. Keep per-C LegacyHold,
   global source backing ownership and one-time Ready funding; no reset/refill or
   borrowing another C's escrow. Purpose checks precede even exact retry.
4. Use actual ALPN1/Locator1/Hello1/Proof1/Consent1 and real d9b compatibility with
   DirectOnly. Publish fresh endpoint/monotone locator only after durable metadata
   and owned-file checks. Preserve mailbox16, Native batch<=4, two global pre-spawn
   permits and128 refs with64/64 shares for two C even if one is blocked. Historical
   witnesses never grant current permission. All referencing C lose old leases on
   an account-pin advance; unrelated C keep independent valid policy tokens.
5. Implement revision2's public offline propose/cosign flow, exact known authority
   floors, one own Root per invocation and same original C/cert tuple/membership.
   The completed legacy invitation must work through the actual stopped frozen d9b
   authority-update/context-import sequence; indexed current consent uses tag11 CAS.
   A renewed interval never clears revocation, fork or quarantine. No Root secret
   enters the runtime owner or generic IPC signer. Avoid creating a messenger vault
   for these offline commands. The actual public commands must be covered.
6. Keep IPC3 exact arities/caps, canonical options, tag11 and bounded tag35 config.
   Tags7–10/handoff/family2 stay unsupported; tags32–34 stay MigrationAdmin-only.
   Preserve frozen wire1/IPC2 and Component5/fresh Runtime6/Native2 checks. The future
   pre-Q control seam must remain possible; do not embed fake Q/Selected prerequisites
   in common Root7 loading. No placeholder handoff success or compatibility bypass.
7. History pages are C-scoped and bounded, with real HVI1, explicit snapshot/full-L1
   proof variants and exact cursor suffix after append/reopen. Check actual escaped
   JSON byte limits before cursor consumption. No Native restore/source scan per row.
   Distinguish live indexed history from the frozen V4 storage-history membership;
   old queue/committed retries and changed-body conflict keep their exact semantics.

Preserve root-owned navigation/task/review files, the submitted plans and all
published S5.1 contracts/manifests/evidence/ledgers. Do not revise historical PASS
claims to describe the new source. Product changes to common S5.1 branches are
allowed only as required by the accepted live mapping, with affected checks below.
Keep unrelated work untouched, including development-environment.md. No dependency
upgrade, GUI, groups, multi-device, new onboarding ceremony, M1.2.63, public rollout
or deferred crafted A2 reproducer is included.

## Verification allocation and execution order

This dispatch activates **new S5.2 C<=500 seconds, F<=1800 seconds, P=0**.
Record a separate S5.2 ledger starting at0 and bind it to the accepted plan/review
and this assignment. Prior S5.1 actual C279.3257298/F1236.2886628/P0 is immutable;
cumulative maxima are C779.3257298/F3036.2886628/P0. This explicitly adds a tranche;
it does not erase old consumption or allocate the later estimates/whole P.

Use the accepted five C and seven F groups, with revision2's policy/renewal/IPC
assertions integrated. No broad repeated suites or100k rerun for unchanged storage
branches. Run meaningful targeted shared Root6/Component5/S5.1 checks where the
actual diff affects their gates, readers or accounting; identify reuse by exact
source/function comparison. Existing S5.1 or fresh S4 PASS does not cover new Root7
runtime paths. Do not label mixed/partial attempts as a single aggregate PASS.

Implement the asymmetric real-origin driver explicitly: the current old driver
caps128/symmetric and cannot already generate511. The chosen origin is actual
frozen d9b A511/B16 Text, with complete receipts, A.next512 and actual B counter.
First run small infrastructure/interop/partial/obligation/policy/atomic gates.
Before long generation, recompute the remaining F balance; it must cover the
seed upper850 + boundary gate140 + contingency. Whole seed attempt stop is900s,
including cleanup. A failure/overrun remains charged and requires a root decision
before a retry. No second generation hidden as setup or borrowing P time.

Freeze the stopped genuine V4 original with producer/source/script/image and
logical Native/Event/request provenance. Never open a frozen original; use a
single active isolated copy per Native role/session/counter domain. Preserve the
stopped indexed exhausted checkpoint after the boundary gate for later cut3.
Current cut proves L1 UpgradeRequired/retries/history/receive/ACK; cutA512/first
Event2 n512 and actual handoff will be verified only in cut3. Real fixture creation,
copy/setup, waits, kills/recovery and cleanup belong to the contributing runner.

Separately executed compile-only, field build/stage, Clippy and fmt are static-only
outside C/F, with exact argv/source/features/lock/image hashes and elapsed recorded.
Everything inside a contributing runner, including cached compile/stage/inventory,
is charged once in its whole elapsed. Retain every failed/interrupted attempt and
the actual source/image binding. Do not subtract repeated setup or double charge
already recorded preparation. If scope/cost no longer fits, submit the concrete
cause and remaining balance to root before further execution, not to the human.

On Windows use locked/offline verification, Jobs12/BelowNormal and the stable
wrapper. Compile harnesses with --no-run; never run network cargo test directly.
Use fixed field EXE paths, compare content hashes and never recopy a live image.
Preserve frozen d9b/S3/exact S4/relay images. If an accepted S5.1 predecessor image
is needed, save/verify it at a separate stable predecessor path before staging over
its current path. Source freeze/build/staging form one serial lane. Parallelize
only independent cases on prepared images and distinct fixtures/Native domains;
elapsed remains charged per contributor, not only overlapping wall time.

## Submission and review boundary

Write `long-lived-dialog-a3-migration-s5-l1-implementation.md` plus a focused
`long-lived-dialog-a3-migration-s5-l1-evidence/` package. Report concrete changed
behavior, operation/state mapping, actual commands/results and missing coverage;
do not turn an estimate or planned assertion into PASS. Include:

- exact candidate path allowlist with raw size/SHA256 and normalized Git blob,
  baseline/HEAD/index state, before/after source and protected-input inventories;
- final-source build/feature/lock bindings, fixed EXE hashes, static results and
  each C/F contributor with attempts/elapsed/classification/cumulative totals;
- genuine origin and stopped-checkpoint provenance/ownership, actual public renewal
  commands, same IDs/keys/history and no-Native-before-refusal assertions;
- failures/corrections, affected-check reruns versus justified source reuse,
  explicit deferred cut3/whole-P/MVP limits and any unresolved findings.

Evidence may include public keys/fingerprints and hashes, never private roots,
Native plaintext/pickles, credentials containing secrets or IPC bearer tokens.
Keep protected fixture DBs local. Source changes after final verification need
explicit affected-check assessment; do not silently reuse an old executable.

Then callback root chat `01a164ec-589c-7b41-bc34-3ba158d1f22b` with document/manifest
paths and hashes, tests/totals, preserved evidence and unresolved issues, and stop.
The human already authorized direct callback and review/fix loops. No commit/push
yet: root independently reviews implementation/evidence and may request focused
fixes. Only after explicit acceptance will root assign exact allowlisted commit/push,
then verify the published diff against the accepted candidate.
