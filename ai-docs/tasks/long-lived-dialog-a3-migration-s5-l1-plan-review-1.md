# A3-S5.2 plan review 1 — two corrections before implementation

2026-10-26. Root reviewed the proposal with two independent read-only GPT-6.1
Sol Max reviewers (state/history and protocol/continuation). Decision: the core
implementation direction is approved, but **plan corrections are required before
product implementation or test execution**. No new human permission is needed;
this is the existing architecture-review gate.

Baseline: `73c499818ba9b699c331aa842e64e8432e90c59a`.
[Submitted plan](long-lived-dialog-a3-migration-s5-l1-plan.md): 38467 bytes,
SHA256 `155387113ed699d4e1fa85e2314770dfccf1ff8f9271a748d7adf3ff027d23be`.
[Assignment](long-lived-dialog-a3-migration-s5-l1-task.md) and
[accepted S5 contract](long-lived-dialog-a3-migration-s5-contract.md) remain normative.
All review work was source/document inspection; no build, test, fixture DB open,
EXE staging or product edit was performed by root or reviewers.

## Approved implementation direction

The new closed Legacy1 purpose/facade and sibling Owner7 are appropriate. Keep
Component5, fresh Runtime6/Origin::Fresh/Native2 and selected H2 bindings separate.
The live facade must not instantiate Inventory or expose generic path mutations.
Immutable migration audit/Ready witnesses and mutable current Head1/Native1 are
different records with explicit correspondence. The existing Ready output digest
exclusion is correctly understood; it is not an S5.1 bug.

Bounded, paid, resumable PrepareL1Indexes7 and InitializeHistory7 are allowed as
closed per-C runtime preparations. Imported queue tickets already contain request/
order, committed request rows contain Event IDs, and HVI1 has real scope without Q.
Do not reopen migration or create new LegacySnapshot after Ready. The separate
version7 imported/full-L1 history reader, immutable snapshot tags, actual later
F/R for pending and exact delivery overlays are approved. Keep per-C escrow,
source backing ownership and one-time Ready funding without resets/refills.

The partial phase/constructor table, final lease gate before Native commit,
ALPN1/direct-only/d9b path and future pre-Q eligibility/first P-or-Prepared cutoff
are consistent with the contract. Existing handoff/ALPN2/mixed work stays deferred.
Exact typed encodings/lowering invariants remain implementation work within these
boundaries; there is no requirement to repeat the entire architecture document.

## R1 — authority scope must include every referencing context

Plan line 133 says own Root changes may affect both C while a peer/C-specific
change does not. That is too narrow for a peer account shared by two contexts.
The accepted contract at lines 303–306 requires an account revocation to invalidate
all affected C leases, independent of whether the account is own or peer.
[Engine::pin_authority](../../apps/kilogram-cli/src/ordered_dialog/engine.rs)
lines 245–263 already computes this affected set by account membership.

Correct the plan explicitly: every account-pin update atomically advances the
appropriate policy/high-water state and invalidates leases/cache for **all C that
reference that account** before the next dispatch/final commit. Own-versus-peer
is not the invalidation boundary. Per-C Consent1/Locator1 updates affect their C;
genuinely unrelated contexts remain independently usable. Equal-revision exact
retry, conflicting same revision and lower revision keep their closed semantics.

Add a targeted assertion for two C sharing one peer account: an authority advance/
revocation closes both old leases and neither can dispatch using old policy.
Retain the complementary different-peer case proving unrelated C progress. This
belongs inside the proposed policy group, not an additional whole test suite.

## R2 — Consent1 renewal needs a usable offline producer

Plan lines 238 and 250–256 define tag11/consent1-import but no producing command.
The existing `context-create` always chooses a new random C
([admin.rs](../../apps/kilogram-cli/src/ordered_dialog/admin.rs), line 236), and
Consent2Create produces a different protocol. Existing context-import can update
the current consent for an existing C with the same certs/membership (lines 329–354),
but cannot create the initial renewal proposal itself. An expired existing dialog
therefore lacks a complete public renewal path as currently planned.

Specify a minimal offline Root-admin producer/cosigning flow for **the same C and
exact certificate tuple**, using existing public scope/membership and current
credential/authority artifacts. For example, one-party propose and opposite-party
cosign commands can emit the existing wire1 consent and a compatible completed
invitation. Each party signs with its own explicitly supplied Root; do not require
Root secrets in the runtime owner, expose a new runtime writer, create a new C,
or recreate Native. Reuse existing verification/signing primitives where possible.

The corrected plan must give concrete command inputs/outputs and artifact validation,
including wrong C/certs/membership/peer/Root refusal and rollback-safe current
authority inputs. Identify the way the frozen d9b side imports the compatible
invitation while stopped through its existing command; the indexed side uses
tag11 and current-ref CAS. Do not silently rewrite immutable original Spec/initial
pair or historical captures. Expiry renewal is not permission to undo revocation
or clear a fork/quarantine block. Add a public-producer -> import -> renewed live
permission assertion inside the policy group, with no raw test-only renewal
factory standing in for the operator path.

This is a small completion of the planned consent administration, not an onboarding
redesign. A different minimal existing public path is acceptable if its exact
same-C behavior is demonstrated by source pointers.

## IPC3 clarification and approved tag allocation

Tag11 for current Consent1 CAS is approved in principle; tags7–10 remain reserved
for the later accepted Consent2/handoff/cancel operations. Keep tags32–34
MigrationAdmin-only and tag35 on the held same-owner capacity core.

Make the proposed exact tuples unambiguous. The existing nested wire Value codec
has uint/bytes/array, with `[]` for None and `[value]` for Some; it has no literal
null. Use that codec unchanged rather than broadening frozen wire1/IPC2 parsers.
In the plan table, spell optional cursor/expected-ref/locator payload as canonical
`[] | [bytes]`: expected-ref Some has exactly32 bytes, locator import requires
Some and export requires None. Required raw consent/authority remain byte strings.
Define the bounded tag35 candidate-config representation explicitly. No literal
JSON/CBOR null or codec fallback is permitted inside these nested tuples.

The new explicit version3 CLI surface is otherwise acceptable. Keep exact arity,
version/tag/cap checks and UnsupportedInCut2 behavior; no raw purpose/Native/mode
setters. History byte-limit checking must precede cursor consumption.

## Root budget and exhausted-origin decisions for the corrected plan

Select the recommended **one real exhausted lane A511/B16**, not duplex511/511.
M02 needs genuine own.next512; the opposite cut is its actual value, not an invented
512. Preserve the same stopped Native domain for the later cutA512/Event2 n512
handoff proof. Current script caps128, so the proposed asymmetric driver must be
implemented explicitly; no false claim that an existing511 command already works.

Accept the proposed planning envelopes: new S5.2 **C<=500 seconds, F<=1800 seconds,
P=0**. These become executable only after root accepts the corrected plan and sends
the separate implementation assignment. S5.1 totals stay immutable at
C279.3257298/F1236.2886628/P0. Resulting cumulative maxima would be
C779.3257298/F3036.2886628/P0. This is an explicit new tranche proposal rather than
a reset or quiet reuse of old balances. Later C350–550/F650–1100 are estimates,
not allocations or a promise; final whole P still requires one separate continuous
run within2400 seconds and its own root decision.

The F matrix arithmetic is correct: named ranges945–1575 plus225 contingency gives
1800. The500–850-second exhausted-origin estimate is plausible from the cited old
runtime measurements, but remains an estimate. Small infrastructure/interop gates
must pass first. Before long generation, require remaining budget for the seed
upper bound, gate140 and contingency; the900-second seed-attempt stop includes
cleanup. Failed/overrun attempts remain charged and trigger root before a retry.
Do not spend contingency on unnecessary repeated already-covered suites.

Correct the plan's section9 preparation classification to match the established
contract: separately executed compile-only/field build/stage/Clippy/fmt are recorded
with argv/source hashes/elapsed as **static-only**, outside C/F. All cached
compilation, staging, inventory, setup, fixture generation, copies, waits, kills,
recovery and cleanup occurring inside a contributing runner are charged once in
that runner. No hidden free fixture work and no double charge of the same preparation.
The source/build binding and Windows stable paths/Jobs12/BelowNormal remain mandatory.

## Correction deliverable and stop

Preserve the submitted plan as the review input. Write the corrected proposal as
`long-lived-dialog-a3-migration-s5-l1-plan-revision-2.md`, explicitly resolving R1,
R2 and the IPC/budget clarifications above. Do not modify the accepted contract,
S5.1 evidence/ledger or root task/navigation/review files. Keep the correction
focused; no new full architecture document is needed.

Only document work is authorized in this correction. No product/script edits,
builds/tests/fixtures/EXE staging/commit/push yet. Callback root chat
`01a164ec-589c-7b41-bc34-3ba158d1f22b` with exact path/hash and decisions resolved,
then stop for review. Root will promptly review this narrow correction and issue
the implementation assignment with its explicit budget once resolved.
