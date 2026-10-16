# RFC-0191: M1.1.65 atomic runtime transition transaction

Status: accepted locally; inactive storage contract.

## Purpose

A canonical sync transition can change four durable facts at once: received
event carriers, the immutable signed successor checkpoint, the session head and
the exact pending client frame for the successor. Committing those facts in
separate operations would allow interruption to expose a carrier without its
checkpoint or a new checkpoint with the preceding frame.

M1.1.65 adds one immediate-durability redb transaction for that complete local
state change. It remains a dormant store primitive and changes no active wire
path.

## Optimistic transition token

`commit_session_runtime_transition` takes the previously restored
`CanonicalSyncV2StoredSessionRuntime` as an optimistic concurrency token. The
durable signed checkpoint, head and runtime record must still match it exactly.
The proposed successor is independently signature-verified and checked as a
valid successor before any write begins.

Every received carrier is canonicalized through the closed typed EventBatch
codec. Its sorted typed keys must exactly equal the newly added
`received_event_keys` delta in the signed successor; neither omitted nor extra
carriers are accepted. Existing immutable carrier bytes and an unexpected
successor record are preflighted before the transaction inserts anything. The
transaction then commits all missing carriers, the immutable successor, the
replacement head and the successor-bound runtime frame together.

## Retry and rollback rules

An exact retry after a lost response accepts only the same successor, the same
replacement runtime frame and the same signed carrier delta. Before the first
commit, the complete expected predecessor runtime must still equal durable
state. Any different successor, replacement frame, stale expectation or
immutable carrier fails closed. A corrupt existing carrier prevents every
other requested carrier, the successor, head and runtime record from being
committed.

Regression tests prove successful restore after reopen, exact retry
idempotence, and complete rollback on a carrier-key conflict.

## Deliberate boundary

This stage does not let the dormant session adapter choose or execute
transitions. It does not yet restore and dispatch a pending action, select the
candidate through mutual authenticated capabilities, register its endpoint or
dial it. `kilogram/m1/sync/cbor/2` therefore remains unregistered and undialed.

The next bounded slice integrates this atomic primitive with the dormant
adapter's Inventory, Diff, EventBatch and Complete transitions and proves exact
restart continuation without changing active Postcard v6.
