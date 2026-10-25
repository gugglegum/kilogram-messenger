# A3-S5.1 storage implementation — revision 3

2026-10-26. R4 compatibility repair, submitted for implementation review.
Baseline/HEAD remains `3547cf528f15c15a21cdb9e30b1d7ca35a01297a`; no publication.
Authority: [root review2](long-lived-dialog-a3-migration-s5-storage-implementation-review-2.md),
9006 bytes, SHA256
`bb07f171f3a82b8bf6d864cac40af286374b33461be91697cd1b634aa31299d1`.
R1 and original R2/R3 are closed by that review. Their
[revision2 report](long-lived-dialog-a3-migration-s5-storage-implementation-revision-2.md),
manifest and evidence remain unchanged. The accepted product contract is unchanged.

## R4: distinguish saved genesis from conflicting evidence

The previous archive helper required the expected `core.role` on both proofs in
an Init/Confirm fork. The real old Engine records a distinct, correctly signed
genesis conflict before its Guard role check. A conflicting role is therefore a
valid captured witness even though it is not a readable Event origin.

Only the `Init`/`Confirm` part of `archived_context`'s kind27 branch changes:

- Both proofs still pass `Event::parse` with the actual expected peer Device.
  Signatures, canonical Core, public Native header and packet-to-outer binding
  retain the existing parser's checks.
- Both proofs must match the receiver's actual phase3/context, D/E, genesis
  kind/ordinal, empty parents and selected Offer/Grant-derived request ID. Confirm
  also requires the selected SID, as the old `receive_confirm` capture does.
- The old proof requires the expected role and exact ID/raw bytes of the genuine
  saved lane genesis. Ordinary readable/Native verification remains unchanged.
- The new proof must have a distinct Event ID and satisfy those actual old capture
  conditions. Its conflicting signed role is retained as evidence. It is never
  imported as a readable Event, pending delivery or Native replay.

The old live Engine, current policy/lease checks and every other verifier branch
are unchanged. The role allowance applies only to captured Init/Confirm fork
evidence, not to readable messages or other scopes.

## Focused genuine-source regression

`migration_s5_r4_old_engine_genesis_role_forks_remain_archivable` uses new isolated
copies of both sides of the already verified stopped d9b F-feasibility-01 seed.
It pins the exact receiver source ID and checks actual phase3/readable3/3 before
capture. Original seed databases are only raw-hashed/copied, never opened or
repaired; their hashes are compared again after the group.

For both Init and Confirm it reads the actual saved peer genesis, changes only
the signed outer role, and re-signs with the existing peer Device. The packet's
native kind, key, index and ciphertext remain byte-for-byte unchanged. The test
calls the unchanged production `Engine::confirm` / `Engine::receive_confirm`
under their real current-policy check and a test admission lease. It does not
claim a new network-admission run or create any new positive Native/Head/Event.

Assertions cover:

- The Engine returns its actual signed-fork Blocked error and durably writes the
  exact old/new proof envelope. `Engine::policy` refuses the blocked context.
- Every old primary row except the expected Head metadata update keeps its cipher
  hash, content hash and length. Head equals precisely `old_head.bump(false)`;
  Native bytes/generation/blob and genuine readable genesis stay unchanged.
- Static full-source and C-scoped verification accept the genuine captured
  conflict; it is absent from the readable lanes.
- Fresh genuine-source copies receive only a deliberately bad witness through the
  ordinary authenticated primary writer. Both wrong-signature and re-signed
  foreign-C proofs are rejected by full-source and scoped history verification.
  Append-only genuine fork records are not overwritten or cleared.
- The real migration owner reaches storage Ready. Actual Text IDs, requests,
  sequence/lane metadata, text and Delivered state match the pre-capture source.
  The original block marker/proofs and Native bytes remain exact through history
  and two further cold repository opens. No Event row is created for the conflict.
- Native constructor/packet-crypto counters remain unchanged for Engine capture,
  migration, history and reopen.

Storage Ready is not live messaging permission. The retained per-C blocked-fork
marker remains the block witness; S5.2 has not been implemented or enabled.

## Verification, budget and reuse

Before updating the ledger, revision2 was snapshotted as
[contributing-cases-revision-2.json](long-lived-dialog-a3-migration-s5-storage-evidence/contributing-cases-revision-2.json),
SHA256 `cc17fa755698e791e1d2bf59fccdea6c2004aaf515926ed6d495911787a851c9`.
The explicit root amendment adds 120 seconds to cumulative F for R4 only:
**F ceiling1320**, with 1181.4272386 seconds already charged and 138.5727614
available before this repair. C600/P2400 are unchanged. No contributor is reset,
reclassified or erased, and no earlier failed attempt becomes a PASS.

[F-fork-final-r3-01](long-lived-dialog-a3-migration-s5-storage-evidence/F-fork-final-r3-01.json)
passes **1/1 targeted Rust test**, covering Init and Confirm and four coherent
bad-signature/foreign-context witness cases. Rust test time is 37.17 seconds;
the complete charged runner is **54.8614242 seconds**, including cached compile,
stable staging, all source copies/capture/migration/reopen/cleanup and inventory.
It passed on its first execution. No full owner/process or extra field case ran.

Updated cumulative budget: **C279.3257298/600 s, F1236.2886628/1320 s, P0/2400 s**.
Remaining amended F is **83.7113372 seconds**. The
[ledger](long-lived-dialog-a3-migration-s5-storage-evidence/contributing-cases.json)
retains all 38 revision2 contributors unchanged and adds only this one case.
Its amendment records the root review2 hash and original1200+120 allowance.

The refreshed [field build/stage](long-lived-dialog-a3-migration-s5-storage-evidence/static-field-images-r3-01.json)
records the same 439 input paths with equal before/after/current hashes, baseline,
literal effective `cargo build --jobs 12 --locked --offline --profile verification
-p kilogram-cli --bin kilogram-cli` and empty live-owner inventory. Both fixed
current CLI/control-peer copies are **67,720,192 bytes**, SHA256
`5685e34704c39edb3d6c7531f0c3c78d590915600201e52b9b9114516a231926`.
d9b/S3/exact S4/relay remain unchanged. These images were refreshed for provenance;
the focused check executes the matching final-source stable unit harness, not a
new network/process-origin run.

Affected strict Clippy for state/CLI all targets passes with exact literal
`-- -D warnings` forwarding:
[static-clippy-r3-02](long-lived-dialog-a3-migration-s5-storage-evidence/static-clippy-r3-02.json).
Locked/offline verification build and compile-only harness preparation pass;
final fmt/diff checks pass. Actual execution uses the stable Windows wrapper,
fixed EXE paths, Jobs12/BelowNormal.

[Final source correspondence](long-lived-dialog-a3-migration-s5-storage-evidence/final-source-correspondence-revision-3.json)
has zero differences for all 33 targeted-case source files and all 439 final
build inputs. Its mechanical comparison proves identical production prefix and
suffix outside the fork region, unchanged previous test bodies, and identical
case-runner execution/timing body outside budget metadata. Exact
[fork diff](long-lived-dialog-a3-migration-s5-storage-evidence/revision-3-fork-branch.diff)
and [budget diff](long-lived-dialog-a3-migration-s5-storage-evidence/revision-3-budget-runner.diff)
are retained. The new test does not replace or revise previous evidence.

[Validation](long-lived-dialog-a3-migration-s5-storage-evidence/validation-revision-3.json)
and [preservation](long-lived-dialog-a3-migration-s5-storage-evidence/protected-files-revision-3.json)
confirm all 180 protected prior files, exact previous contributors, recomputed
amended budget, unchanged HEAD/empty index and no remaining cargo/rustc/kilogram
process. A static-only finalization assertion initially compared against the
earliest shared snapshots rather than the closed final revision2 snapshots; its
diagnostic is retained. Correcting that comparison required no product changes,
fixture work or extra test run.

The source/branch comparison against revision2 is explicit. Production changes
are confined to the Init/Confirm fork-evidence region. The rest of
`provenance.rs`, `VerifiedSource::verify_scope`, readable Event/Native/Head/fold,
per-C source dispatch and state migration/copy/history/ledger/runtime files retain
their exact earlier bytes. The new test is appended to the existing test module.
The only other source/tool change records the authorized F1320 limit and its
review2 authority in the case runner; its test execution/timing behavior is intact.

Unblocked origins do not enter kind27's Init/Confirm branch. Therefore the closed
revision2 owner/core and five distinct process scenarios remain usable evidence
for those unchanged paths. This includes the genuine partial phases, renewal/
pending/debt, capacity/version/parent-kill and real two-C escrow/history cases.
It does not convert their earlier split 3+2 process coverage into an aggregate PASS.

C2 isolation is reused with the same explicit branch argument: `archived_context`
still filters each entry by the selected `spec.c`; every new genesis lookup uses
only that same C. A blocked C1 adds no C2 read, mutation, quota or permission rule.
The per-C caller/iteration, C-scoped history decoder, state copy and escrow code
are unchanged. The prior actual two-C and C2-body sentinel evidence covers those
unchanged paths; no new large C2 origin is claimed or created.

No full owner/process rerun, separate two-C repetition, full511, 100k or whole P5
was performed. Earlier protected reports/manifests/per-case/static artifacts,
both root reviews, root-owned navigation/assignment files and accepted documents
remain byte-for-byte preserved. The new manifest inventories exact raw sizes,
SHA256 and normalized Git blobs, excluding its own checksum cycle.

Stop for implementation review. Commit/push requires acceptance and an explicit
publication assignment; neither has been performed. S5.2 remains outside this cut.
