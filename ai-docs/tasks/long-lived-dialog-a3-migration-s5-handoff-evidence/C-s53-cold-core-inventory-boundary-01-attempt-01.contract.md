# C2-P: inventory raw-length boundary and arithmetic reachability

2026-10-31. **Contract only; STOP for root review.** Baseline 753b5683c6e97120a210afaa33f5b5a90a5611c6. Actual [Core](../../../scripts/migration-s53-cold-preservation.ps1):79709B/SHA256 ec3cd2190c4b67c201c02eeda6c68e5832e95ac114a2e570954e4a25a05f3bb9. Read against [r42](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-contract-r42.md) as amended by [r43](../long-lived-dialog-a3-migration-s5-handoff-cold-preservation-amendment-r43.md). No implementation, fixture probe or execution is authorized by this proposal.

## One reachable inventory negative

Let I=16,777,216 (16MiB). Source chain, at this baseline:

1. Spec.Production pins historical InventoryBytes6765525. Spec.Fixture83–102 rejects bytes>I with ArgumentException **before constructing Core**. That is not a runtime Core refusal.
2. CaptureAndCompare1016–1019 acquires six locks/ancestors, sets phase/role inventory, then retains the inventory through Fixed/Acquire. Acquire checks actual held-file kind/path/ID/single-link, nonnegative size<=64MiB and strict stream shape; control-file length comes from GetFileInformationByHandle, not the caller's claim.
3. ReadInventory795–806 gets fresh Info on that retained handle. Its first guard is **info.Length!=spec.InventoryBytes || info.Length>Limits.Inventory**; failure is **inventory_binding**, not inventory_cap. It precedes InventoryTake, the inventory-sized byte array, Seek/ReadFile, SHA256 and ParseInventory. Fixed constructor tables/scratch and lock EOF checks have already occurred.
4. Only admitted equal lengths<=I reach Accounting.InventoryTake (inventory initially0, one call under one-use capture), then allocate/read. Thus its separate inventory_cap diagnostic is unreachable under current guards. A mismatched overlong file alone short-circuits the first operand: it cannot isolate the I guard.

**Proposed sole case:** family C-s53-cold-core-inventory-boundary-01-attempt-01, leaf inventory-size, selector boundary-inventory-size. Fixed O/S/BKP/proof roles follow accepted small-fixture layout. Keep the ordinary five-row/52B payload mirrors, safe-empty-S and six empty locks; the control file alone is I+1=16,777,217 zero bytes. Its SHA256 is **1003b1b5dc078189799a1216ce0f9fbcebb94e8b6b83c58c4b03345f07f94ced** (independent in-memory64KiB-chunk calculation). This is a raw length-boundary control, not a producer-valid JSON inventory or a payload-limit test.

A minimal **future closed test-only seam** is necessary: append that selector/fixed family binding and permit exactly this selector's fixed I+1/above hash/5files/52B metadata through Spec.Fixture's upper-length precondition. Keep the bytes>0/hash/other argument guards and every other selector's<=I condition. Do not change Limits.Inventory, Spec.Production, ReadInventory, Accounting or add a cap/path argument. This admits one fixture declaration so the unchanged production reader guard can reject it; it does not permit an oversized buffer/read. No general cap increase or injected exception.

Future fixture setup must CreateNew only fixed files, stream the raw control with one64KiB zero buffer, flush/close and verify actual length/content hash/ACL/no-follow/identity/strict streams through bounded reads. Do not use a17MiB byte array/DOM, SetLength-only content assumption, sparse-file setting or relaxed common premise checker. A dedicated closed control-file branch may allow I+1 for this path only; other small setup files keep existing bounds. Preserve the raw-control label rather than claiming healthy inventory validation. No output owner/E or receipt creation is needed.

The oracle must prove factory **returned** and CaptureAndCompare **entered**, Spec and held-file length both I+1 (therefore mismatch operand false), and hooks exactly locks/inventory. Expected refusal: **inventory_binding / inventory / inventory**, native0, bounded_refusal, HRESULT -2146233079; shell chain MethodInvocationException(-2146233087) -> normalized InvalidOperationException -> original marked InvalidOperationException, both latter HRESULT -2146233079. Require exact normalized code/phase/role and original colda_code, no deeper/native cause. ArgumentException, inventory_digest/schema, timeout or setup error is not PASS.

At refusal/after Dispose use complete38-field observations: locks6, expected5/52, expected-file acquisitions0, captureQPC0/capturedfalse, completedfalse/observedsetsfalse, hashpasses/bytes0, all five tuple digests null, **peak_inventory_bytes0**, receipts/bytes/digests0, remaining0/input_handles_releasedtrue/cleanup_unknownfalse/native cleanup0. Source model:26 retained fixed input handles, transient stream slot gives peak27; records12073840+26*384=12083824, no row credit. Finally closes current/search/retained inputs and hash state; parent requires actual exit0/reap/both EOF/known cleanup. No observed inventory_cap claim: this tests the real raw-size guard with its current inventory_binding classification.

Prospective **one work20/whole30, lane1/retry0** batch through the accepted guardian/job pattern;16MiB+1 raw writes and bounded setup readbacks are additional to small-case setup, not Core hashing. Source/report caps and actual-return collection remain as accepted. Timing remains unmeasured; source review and separate allocation precede execution. No reservation or ceiling request is activated here.

## All five executable checked expressions

Bounds below apply to the pinned Core and accepted closed caps; fixture reductions only lower Rows/Entries/Handles. No direct field mutation/reflection bypass, raised safety caps or synthetic OverflowException is proposed.

| Core site / expression | Strict bound and reason | Reachable overflow |
| --- | --- | --- |
|394: checked constructor table charge |60000*64 +40126*72 +8000*104 +20000*224 +32768 = **12073840**, all long arithmetic, positive caps; checked before table allocation. |Excluded. A constructor failure would not be runtime Capture evidence. |
|447: checked aligned Intern charge (32L+2L*value.Length+7L)&~7L |Even String.Length's Int32 maximum gives pre-mask4294967333/aligned4294967328, far below Int64.MaxValue. Actual row/parent strings<=1024; fixed strings are closed literals. Path accounting refuses before retention when charge exceeds96MiB. |Excluded. |
|553: checked (int)r.TokenStartIndex |The reader is over one admitted raw byte array length<=I; offset is nonnegative and<=I<Int32.MaxValue. No sequence/unbounded reader. |Excluded. |
|554: checked (int)r.BytesConsumed-offsets[...] |Cast operand<=I; after Skip, consumed>=saved start. Difference lies0..I. Malformed JSON refuses separately; the new I+1 fixture never reaches this parser. |Excluded. |
|949: checked facts.HashBytes+rows[i].Bytes |Number/per-row/aggregate gates admit one mirror total<=64MiB. Exactly four HashTree calls under one-use Capture; each prefix nonnegative, total<=**268435456**. Internal facts start0; returned observations are copies. |Excluded. |

Supporting pre-addition proofs are not new runtime negatives. Accounting.Add249 requires n/value>=0, value<=positive cap and n<=cap-value before value+=n; subtraction/addition remain0..cap (largest path cap96MiB). The already accepted [C2-C proof](../long-lived-dialog-a3-migration-s5-handoff-cold-payload-limits-readiness-r63.md) keeps payload t in0..64MiB, rejects v>64MiB-t before t+=v, and Number rejects values above Int64.MaxValue before signed cast. **C2-C integer-range is accepted inventory_schema coverage, not observed arithmetic_overflow.** Its [root closure](C-s53-cold-core-payload-limits-01-attempt-01.root-closure.json) and [actual review](S5.3-r64-cold-payload-limits.root-review.json) remain valid.

Adjacent indexing/allocation stays bounded: reader offsets<=I, canonical path<=1024 UTF-16 units (UTF-8<=3072), fixed tables Rows8000/Entries20000/Handles40126; fixed receipt record<=32768 and per-file/aggregate output checked by remaining capacity before increments. Identity.GetHashCode272 is deliberately **unchecked** hash mixing, not a sixth checked failure site. External CLR/OS failures are not certified by this proof.

**Proposed coverage for root decision:** accept a source-bound static proof for these five checked sites plus unchanged Number/accounting/aggregate guards, with lexical checked-site recount and exact source pins in the later source review. No reachable accepted-input arithmetic_overflow scenario was found; raising caps, corrupting facts or injecting that exception would test a different premise. The OverflowException mapper remains unchanged. This proposal does **not** independently close r42/r56's arithmetic requirement or label it runtime tested.

## Finite remainder of this synthetic block

Keep the [accepted small positive](C-s53-cold-core-small-01-attempt-02.root-closure.json) and accepted C2-A–O outcomes in their scopes. In particular, do not repeat/reopen C2-C file/total/integer-range, [C2-I rows/entries/handles](C-s53-cold-core-resource-caps-01-attempt-01.accepted.md), [C2-J depth/paths](C-s53-cold-core-path-boundaries-01-attempt-01.accepted.md), [C2-M output cap/collision](C-s53-cold-core-output-boundaries-01-attempt-01.accepted.md), [C2-L native receipt-close](C-s53-cold-core-receipt-close-01-attempt-01.accepted.md), [C2-N native read/unsupported](C-s53-cold-core-native-read-errors-01-attempt-01.accepted.md) or [C2-O ACL refusal](C-s53-cold-core-output-acl-01-attempt-01.accepted.md).

Pending required block outcomes from r42/r43 and [r56 checklist](../long-lived-dialog-a3-migration-s5-handoff-cold-negative-a-task-r56.md):

- Inventory raw-size boundary: this proposed single case; not executed.
- Arithmetic safety: root decision on the above static coverage; no claim of an observed OverflowException.
- Blocked-worker deadline/whole cutoff with owned-process EOF/reap/cleanup and actual final return; ordinary successful lifecycle checks do not establish the blocked case.
- Producer-size positive:6956 files/45597399B per mirror, full three-array provenance/four retained-handle passes, independent restricted receipts, phase/capture timing, exact owned peaks, sampled OS handles+interval, OS peak working set and arena/scratch metrics required by r43. Small positive does not substitute.

The frozen launcher's failing-path consumption inference remains the existing later integration issue identified in C2-O; no new fault matrix is added here. After synthetic acceptance, private preservation and typed-open/ownership-gap/write prerequisites remain separate r42/r43 stages, without a grant. Whole S5.3/G1/S5.4 remain open.

Only this contract is added. Public code/evidence reads, AST parse and in-memory public bound/hash calculations only; no script/fixture/harness changes, runtime/compile/dot-source/.tmp/private operation, allocation/ledger change, commit/push or test repeat. **Callback and STOP for contract review.**
