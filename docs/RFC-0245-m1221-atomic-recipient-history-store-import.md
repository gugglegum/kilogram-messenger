# RFC-0245: M1.2.21 atomic recipient history-store import

Status: implemented and locally verified on 2026-10-20.

## Purpose

M1.2.20 deliberately stopped before the recipient's ordinary history stores.
It produced a committed, source-signed recovery chunk plus recipient-signed
checkpoints and encrypted projections in a separate execution journal. This
stage consumes exactly that durable result and publishes one bounded page into
the recipient's real event and encrypted local-message stores.

It does not add a network transfer, a multi-source scheduler, active runtime
IPC, desktop controls, or a new public default.

## Import authority

An import accepts only an exact committed M1.2.20 execution record. Before the
first import it requires that record to be the current head for the exact
consent selection. It then re-verifies the source-signed bundle, both signed
checkpoints, encrypted projections, Account and Device binding, range, and
conversation membership. The importing identity must be the current recipient
Device.

An unimported stale execution record is rejected. An exact record which already
has a valid recipient import receipt may be replayed even after the source
execution head advances; this replay is a no-op and re-verifies every imported
event, projection, checkpoint, and receipt byte instead of repairing anything.

## Atomic publication

The page is preflighted through the event and projection command overlays.
Then one state-directory transaction publishes the immutable authorized
events, encrypted projections, before/after checkpoints, and a recipient
Device-signed import receipt. Every created path is registered with the same
transaction. The state-directory lock serializes writers. If an encrypted
state vault exists, its DB-primary shadow is recovered first and its dual-write
protocol surrounds the same transaction.

An error after events, projections, checkpoints, or receipt creation rolls the
whole page back. A simulated process crash intentionally leaves the active
transaction marker; privacy-safe status refuses to describe that partial state,
and the next mutating retry performs existing transaction/vault recovery before
reapplying the exact page. There is no partially successful page claim.

Checkpoint filenames are short content-addressed IDs under
`history-recovery/import-checkpoints`. The signed checkpoint still contains the
full consent, selection, conversation, range and Device binding. This keeps the
same validation while avoiding Windows path-length failure in deeply nested
state directories.

## Durable receipt chain

Each receipt is bounded to 64 KiB and signed by the recipient Device. It binds:

- recipient Account and source/recipient Devices;
- consent selection and exact execution record/generation;
- conversation, bundle and both checkpoint IDs;
- approved and imported ranges;
- ordered event IDs and encrypted projection digests;
- the previous receipt ID and commit time.

At most 16,384 receipts are accepted. Generations and checkpoint ranges must be
linear, one execution record can be consumed only once, and corruption,
forking, orphaning, unexpected directory entries, symlinked store roots, or an
existing conflicting event/projection/checkpoint fail closed.

## Observation and privacy

Read-only status has no-create semantics and reports only an opaque selection
token, imported chunk/event counts, next range, approved end, and completion.
It exposes no message body, plaintext, raw identity, path, endpoint, or network
coordinate and grants neither execution nor network authority.

Message bodies in the ordinary event store remain E2EE protocol ciphertext;
local readable history is represented by the existing encrypted projection
format. No plaintext is written by the import journal or receipt.

## Verification and limits

Four focused import tests cover atomic publication and reopen, exact replay,
stale head/current Device/sequential-chain enforcement, conflicts, corruption,
explicit rollback, and crash recovery. Together with the six M1.2.20 execution
tests, the stable-path Windows harness passes 10/10. The boundary verifier
rejects ten negative mutations. Aggregate accepted-tag readiness contains 147
closed gates.

This remains a local, offline, single-source mechanism. It does not claim
whole-database rollback detection without an external monotonic witness. It
does not prove network delivery, multi-source recovery, active IPC/desktop
authority, public rollout eligibility, or independent security review. Active
IPC remains Postcard v33 / descriptor domain v14, and the public default remains
`legacy-only`.
