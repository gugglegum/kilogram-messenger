# A3-S5.2 — indexed L1 continuation: implementation mapping and verification plan

2026-10-26. User asked root to continue S5 after S5.1 acceptance/publication.
Executor: the existing GPT-6.1 Sol Max chat, normal speed, no Fast.

## Verified starting point

S5.1 commit/HEAD/master/actual GitHub master:
`73c499818ba9b699c331aa842e64e8432e90c59a`, sole parent
`3547cf528f15c15a21cdb9e30b1d7ca35a01297a`.
Root independently checked exact 230 committed paths/blobs, accepted raw hashes,
the sole parent and clean worktree/index. GitHub compare to master is identical,
ahead0/behind0. Executor independently reported successful ordinary push and exact
remote `ls-remote`. Publication is complete.

Normative inputs:

- [Accepted S5 contract](long-lived-dialog-a3-migration-s5-contract.md), particularly
  sections 2–3, 6–8, 10–12 and implementation cut 2. Its SHA256 remains
  `f87c5c8b0b307863078cf3e2e85f3d13a05392d1d654affd5f2a40a3943ad2ad`.
- [S5.1 acceptance](long-lived-dialog-a3-migration-s5-storage-implementation-review-3.md)
  and [revision-3 implementation](long-lived-dialog-a3-migration-s5-storage-implementation-revision-3.md).
- Existing accepted A3/S3/S4 protocol, Native, storage and runtime boundaries
  referenced in those documents. Contract acceptance need not be repeated wholesale.

S5.1 implements storage Ready and deliberately refuses live indexed startup. The
next product outcome is continuing the same real L1 dialog through an indexed
owner, including communication with the actual frozen d9b remote. No new account,
replacement dialog, lost history or recreated saved Native domain is acceptable.

## First deliverable: bounded plan for root review

Read the current implementation and produce one working document:
`long-lived-dialog-a3-migration-s5-l1-plan.md`. Keep it concrete: code pointers,
operation/state tables, test commands and cost estimates rather than a copy of
the full architecture contract. Distinguish existing code, required implementation,
deferred cut-3 work and genuine design contradictions.

This dispatch authorizes source/document inspection and that proposal only.
Do not yet edit product code/scripts, build/run tests, create/open/migrate fixture
DBs, stage EXEs or commit/push. Root must first accept the proposed implementation
boundary and explicitly allocate the new verification budget. The reason for this
gate is the nearly spent S5 F allowance and the additional live-owner surface,
not missing human authorization to continue the project.

The plan must resolve these implementation points:

1. One lifetime lock and dedicated owner/mailbox, with typed indexed point/range
   access. Map concrete modules and changes needed for L1 bootstrap, queue,
   materialize, pending, readable and acknowledgement purposes. No old mutable
   Engine as a parallel writer, generic imported write API, hot-path legacy
   `paths()` scan or full-history cache. State remains independent of protocol/
   ratchet dependencies; owner and structural validation responsibilities stay
   explicit. Preserve Component5/fresh Runtime6 behavior.
2. Continue original Head1/Native1 with the original nonce, keys and self-HPKE
   bindings. Specify each real partial phase's first legal next operation and
   exactly when a new Native object is legitimate for a never-completed bootstrap.
   Existing objects must never be recreated. Imported queue/pending/quarantine,
   signed-fork blocks, requests, readable history and receipt debt must retain
   identity and behavior across crash/reopen. S5.1's archived conflict proof remains
   evidence, not a readable message or permission to unblock the C.
3. Specify the atomic Native/Guard1/Seen/current-pointer/history/provenance/credit
   changes for each live purpose. Migration-only LegacySnapshot construction stays
   closed after Ready, including exact retry; post-Ready records need the accepted
   actual captures. Explain how imported and newly created L1 records coexist and
   how delayed old receipts release precisely the covered debt once. Keep escrow,
   global source backing, C2 isolation and one-time Ready funding unchanged.
4. Enable the actual L1 transport path with current policy and authenticated lease:
   exact Locator1/ALPN1/Hello1/Proof1/Consent1, direct-only, real d9b compatibility,
   fresh endpoint and persisted monotone locator revision before publication.
   Expired/revoked archived witnesses alone never grant current admission. Preserve
   bounded owner/mailbox16, Native batch<=4, two global pre-spawn permits and the
   accepted per-C work/ref bounds. Decide which S4 runtime infrastructure can be
   reused without weakening its fresh owner/purpose checks.
5. Define the exposed profile/CLI/IPC operations and strict unsupported boundaries.
   Storage Ready alone must not bypass live initialization/permission checks.
   For this cut only L1 continuation is enabled; do not expose fake ALPN2/Q or
   permissive handoff placeholders. Consent2/P/A/K/Committed, Selected/mixed writer,
   durable P/Prepared transport cutover and whole P5 remain cut 3/final work.
   Preserve the contract's later eligibility boundary so cut 2 cannot require an
   unsafe compatibility fallback in cut 3.
6. Plan indexed history and request/status semantics for imported and new records,
   including bounded C-scoped reads, stable snapshot cursor suffix after appends,
   and the distinct frozen V4 offset dataset. No fake SID/view for an incomplete
   session. Current authorization/blocking of C1 must not globally disable C2's
   independent allowed operations.

## Verification and explicit budget proposal

S5.1 consumed C279.3257298/600 seconds and F1236.2886628/1320 seconds;
P0/2400. The +120 F amendment was only for R4 and is not an allowance for new
live work. Keep the published S5.1 ledger immutable. The plan must show both prior
consumption and proposed incremental/cumulative limits for S5.2, including how
later handoff/final acceptance is reserved. No new run is authorized by an estimate.

Use existing timing evidence for the proposal; no new empirical feasibility run
in this planning dispatch. Address the old full511 seed estimate of500–850 seconds
and decide where its one-time generation belongs. Separate the exhausted L1
boundary evidence required in this cut from eventual handoff cut512/Event2
continuation evidence that cannot be claimed until cut 3. Do not replace a real
exhausted origin with fabricated Native/Head/Event rows or an increased lifetime.

Propose a minimal named matrix with production entrypoints and observable assertions:

- Small real d9b origin -> storage migration -> actual indexed L1 versus frozen
  d9b peer: duplex messages, exact same IDs/keys/history, requests/retries and
  restart/offline reconnect. Then current indexed owners on both sides as needed.
- Real incomplete Offered/Granted/genesis/Ready origins from the accepted public
  control-peer technique: complete the remaining initial handshake once, preserve
  queue and existing Native, and refuse fake complete-session views.
- Imported ordinary/head pending, late/gap delivery and own-unacked receipt debt:
  lossless drain, exactly-once accounting and no erased quarantine/fork block.
- Current permission/expiry/revocation, malformed input, stale lease, purpose-before-
  retry and per-C isolation; historical static acceptance is not live admission.
- Typed live transaction crash/reopen boundaries, exact retry and native/history/
  credit atomicity; use targeted shared checks rather than broad repeated suites.
- L1 next512 rejection and old request retry/reopen, preserving the source for the
  later separately accepted handoff test. Explain the cost/reuse and ownership.

Every attempt, setup, copies, kills/recovery and runner overhead must be charged
and retained. No quiet repeats, new free setup, reset clocks or prefix+suffix whole
P5 claim. Source/function reuse arguments must identify changed shared branches;
do not rerun100k when normal append/page/allocator is unchanged. Full whole P5
remains one later continuous run with its own2400-second ceiling unless root
explicitly decides otherwise before execution.

Maintain Windows compile-only Cargo and stable fixed-path harnesses/images,
locked/offline verification, Jobs12/BelowNormal. Plan independent parallel work
only on distinct fixtures/Native domains and prepared EXEs; wrapper recopy of a
live image remains prohibited. Frozen d9b/S3/exact S4/relay and accepted historical
DBs/evidence remain protected. Proposed reuse of a stopped seed needs its exact
provenance and one active branch at a time; never open the frozen original.

## Review handoff

Preserve this task and root's README/next-stage/coordination updates. Produce only
the named plan unless a short directly referenced design appendix is necessary.
Report unresolved contradictions, specific required new APIs, source scope,
matrix/estimated incremental budget and exact plan hash in one callback to root
chat `01a164ec-589c-7b41-bc34-3ba158d1f22b`. Then stop for review.

Root will accept/correct the mapping and budget, then separately authorize product
implementation. Implementation is reviewed before exact commit/push under the
user's standing authorization. No GUI/groups/multi-device/mailbox/M1.2.63, public
rollout, deferred crafted A2 reproducer or claim that whole S5/MVP is complete.
