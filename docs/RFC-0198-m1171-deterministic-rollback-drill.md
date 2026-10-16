# RFC-0198: M1.1.71 deterministic dormant rollback drill

Status: accepted locally; deterministic restart drill verified, transport inactive.

## Purpose

M1.1.70 can dispatch an authenticated canonical session from its exact durable
checkpoint, but migration still lacked an executable rollback contract. M1.1.71
adds a dormant, deterministic drill that proves a persisted migration state can
move exactly one step toward legacy without silently crossing canonical session
evidence.

## Persisted evidence

`CanonicalSyncV2RollbackSnapshot` is a strict fixed-width 107-byte record. It
contains the migration state and epoch plus optional content identifiers for an
authenticated selected session, durable client progress and durable responder
progress. Unknown versions, flags, zero identifiers and unflagged bytes fail
closed. The snapshot identifier is derived with BLAKE3 over the exact bytes.

`CanonicalSyncV2RollbackDrillRecord` binds the previous snapshot identifier,
the replacement snapshot, an explicit trigger and a bounded reason. Its strict
codec rejects non-canonical length, invalid UTF-8, malformed reason and record
identifier mismatch. The test writes this record to a temporary file, restores
it after a simulated restart and applies it only to the exact previous
snapshot. This verifies a persistence contract; it does not integrate a new
active runtime store.

## Rollback policy

Rollback is allowed only for an explicit local action or release policy. A
parse error or network error can never trigger automatic fallback. The reason
must be non-empty, trimmed, control-free and at most 256 bytes. The migration
epoch advances exactly once even though the migration state moves one step
toward legacy; a stale or skipped epoch fails closed.

Only the immediately previous migration state is accepted. `canonical-required`
can move to `dual-reader-prefer-canonical`, then to
`dual-reader-prefer-legacy`, then to `advertise-cbor-v2`, and finally to
`legacy-only`. A state may not be skipped, and `legacy-only` has no rollback.
Identical input produces the same exact record and content identifier.

## Canonical evidence boundary

Rollback is blocked once an authenticated canonical session has been selected.
It is also blocked after durable client or responder progress exists. The
capture operation verifies persisted checkpoint/frame consistency and exact
dual-reader identity before accepting their evidence. This prevents rollback
from turning a selected or partially progressed canonical exchange into an
implicit legacy retry.

The replacement state can produce only a symbolic legacy dispatch description.
It does not reimplement or invoke the installed Postcard runtime. The drill's
`active_transport_authorized()` result is always false.

## Deliberate non-activation

The rollback types remain absent from active wire, Iroh and CLI code. This
stage does not register an ALPN, dial or accept a peer, read or write a network
frame, alter endpoint publication, or change the active Postcard path. It also
performs no network run, release build, archive creation or stable executable
replacement.

The remaining bounded work is sustained coverage-guided fuzzing followed by
fresh hosted and physical-device evidence before any canonical transport
activation decision.
