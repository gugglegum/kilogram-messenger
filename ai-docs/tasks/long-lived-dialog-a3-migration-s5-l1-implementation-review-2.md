# A3-S5.2 implementation review 2 — corrected source ready for focused F

2026-10-26. Root response to
[review1 fixes](long-lived-dialog-a3-migration-s5-l1-review1-fixes.md),9026 bytes,
SHA256`a34af34bec4b7301616ba40ef799f9a28fb3c9ca6f2e2fbff3a5b30ee2bb69ef`.
The unchanged [contract](long-lived-dialog-a3-migration-s5-contract.md), accepted
plan and [implementation review1](long-lived-dialog-a3-migration-s5-l1-implementation-review-1.md)
remain controlling. Baseline is73c499818ba9b699c331aa842e64e8432e90c59a.
This accepts the corrections for field verification, not the whole implementation
or publication. Standing human authorization covers this review/fix continuation.

## Review outcome and binding

Root and the read-only protocol/state reviewers found the three reported defects
closed in the corrected production code. No further confirmed blocker was found
in the focused correction or new field-controller scope. No reviewer executed
tests, opened a DB, modified source or contacted the executor independently.

- R1: actual current own F is captured separately from original O, atomically with
  own genesis/Text. Bounds, lowering and both full-proof/history readers require
  applicable F; peer captures and immutable LegacySnapshot behavior remain intact.
- R2: eligibility is min(queued,4,128-debt) before Native. At128 no packet work;
  above128 fails closed. Closed batch and debt-insertion guards independently
  enforce the bound. Outer operation and occupied-context checks protect indexing.
- R3: actual Ready shape/scope/role/suite/format admission precedes fork
  classification; a prior peer Ready is retained as the first conflict witness.
  Signed malformed controls remain ordinary refusals. New debug hooks use the
  existing external debug-only scheduler, not an IPC signer or state writer.

The field controllers exercise genuine partial origins/authority advance,
127-debt+4-queue through one actual exact receipt, and signed malformed Ready
through the real dispatch before a separate genuine scoped fork. The R2 hold,
single-query and post-receipt actor checkpoint ordering was reviewed for races.
These are executable verification plans; no field PASS is claimed yet.

Root verified all18 source-delta entries against current bytes and all49 after
inputs in build10, compile15 and strict Clippy18 against the working tree, with
no mismatch. Delta SHA256:
`3f5c5c85718c7ef2007da9c3d896e12bf2011e98c93f63d5d08a3aa23388aaca`.
It is a hash delta, not a reconstructed patch for old untracked files.

Both fixed CLI/control-peer images are70222848 bytes, SHA256
`a3ff9d2ee67e5b01db8f4c922247737d0dee7bb4c639477be3e02d4999b13c56`.
The corrected controller SHA256 is
`3c682b63971480a0fe3e93ff49f36dd734424c0a4ed0438bc7b1b474fcc93ed2`.
Root verified C-review1-01 SHA256
`d2c60176228326ae4ade5ceff84e5312e4211a2723d4d5498589d5848829e543`:
five actual matching tests (CLI2/state3),20.9155434s, exit0, no guard expiry and
zero owned children after cleanup. Component/negative evidence is not substituted
for genuine Native/Event integration. Historical Clippy17 failure remains recorded.

## Explicit budget amendment 3

Authorize S5.2 **F2400 -> F2900**, an additional500 seconds of contributing local
verification. C500 and P0 do not change. Append this amendment and its hash to the
live ledger, without rewriting existing case records or resetting consumption.

Current actuals: C126.2916571/F979.0484649/P0. The new F remaining balance is
1920.9515351. Requested conservative remaining work:
120+240+160+100+45+850+140+225=1880 seconds. The forecast total2859.0484649 leaves
40.9515351 below2900. The added allocation funds review regressions; it does not
buy retries, weaken assertions, erase failures or borrow later P time.

Prior S5.1 actual C279.3257298/F1236.2886628/P0 remains immutable. Combined maxima
are now C779.3257298/F4136.2886628/P0. This is local test-runtime accounting, not a
model-token or subscription-limit claim.

## Authorized next execution

One attempt per group, serially, using the corrected bound images and sources:

| Exact case ID | Exact focused selector | Whole upper / work cutoff seconds |
|---|---|---:|
| F-review1-own-f-01 | migration_s5_l1_review1_genuine_partial_own_f_both_genesis_roles |120/100|
| F-review1-unacked-01 | migration_s5_l1_review1_unacked128_before_native_and_one_exact_receipt |240/220|
| F-review1-ready-01 | migration_s5_l1_review1_signed_ready_refusal_then_scoped_fork |160/140|
| F-isolation-02 | migration_s5_l1_different_peer_c2_survives_signed_fork_and_account_pin |100/85|
| F-capacity-01 | migration_s5_process_capacity_crash_and_predecessor_fences |45/40|

Reuse the already-verified dedicated-shell Windows-job deadline mechanism for
these cases. A narrow runner extension binding each ID, F/Tests classification,
exact selector, this decision hash and the table's work/whole limits is authorized.
Keep the whole watchdog armed through cleanup/finalization. Record immutable
started markers; reconcile an interrupted attempt before further runs. No process
name kill, live EXE recopy, or broad replacement of the tested deadline mechanism.
Parse/check the extended runner; a policy-table extension alone does not require
another full mechanism probe. Record its new hash. These script changes do not
justify silently claiming a new product build; explicitly bind unchanged product
inputs and the existing compiled harness.

Use locked/offline verification, Jobs12/BelowNormal and the stable test wrapper.
Require the intended matching test, not merely a zero-test executable exit.
Whole accounting includes setup/copies, cached work inside the runner, waits,
kills/recovery and cleanup. Keep the previously disclosed terminal publication
timing boundary explicit. Do not edit source/controller files while a source-bound
case is running, even when preparing the future exhausted boundary controller.

The R2 controller's100-second wait for its distinct127-debt workload is approved
as a bounded correctness allowance. It does not change production timeouts or
approve user-facing latency. R3 ordinary waits remain25 seconds. Do not change
default batching, window shares, lifetime or assertions to force a PASS.

Retain fresh/genuine origin rules from the submission: no missing-F candidate
fixture repair, no inferred captures, no manufactured positive Native/Event
provenance, no reopening the frozen original, and only one active isolated branch
per Native domain. Preserve all prior candidate results as historical evidence.

On any failure, work/whole expiry, integrity/source-binding issue or budget
overrun: retain actual artifacts, finish owned-child cleanup, account for the
attempt and callback root before another F. No automatic repeat or compensating
scenario. After each PASS the next listed group is authorized, provided remaining
funds still cover the remaining listed upper bounds plus the separate long-entry
reserve1215. If that precondition fails, callback rather than spend the reserve.

After all five PASS, send one compact results/readiness callback with exact cases,
matching assertions, source/image bindings, actual charges and remaining balance.
Assess reuse of unaffected prior branches by exact source/function comparison;
do not relabel prior candidate PASS as evidence for changed production paths.

## Separate long and publication boundaries

Long511 is still **not authorized**. Finish the exhausted boundary controller and
its compile/source binding while no contributing run is active, then provide
actual readiness: one genuine frozen d9b A511/B16 origin, work850/whole900,
boundary140, remaining>=1215, source/producer/provenance and exact runner/ledger
hashes. Root must separately review and grant the first-long approval. No silent
second origin, P substitution, cut3 or whole-P run.

No commit/push yet. Full corrected implementation/evidence acceptance must precede
the standing exact allowlisted publication procedure, followed by the already
queued S5.3 assignment. Preserve this new root-owned decision and all earlier
reviews, plans, published artifacts and unrelated working-tree files.
