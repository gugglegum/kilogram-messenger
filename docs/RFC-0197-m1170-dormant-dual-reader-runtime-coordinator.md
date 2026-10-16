# RFC-0197: M1.1.70 dormant dual-reader runtime coordinator

Status: accepted locally; runtime dispatch verified, transport inactive.

## Purpose

M1.1.69 can decide whether one authenticated session is allowed to use legacy
Postcard v6 or canonical `/cbor/2`, but it did not turn that decision into the
exact restart action an endpoint must take. M1.1.70 adds the missing
transport-independent coordinator without connecting it to Iroh or the CLI.

## Legacy dispatch

A legacy plan returns only the symbolic `LegacyPostcardV6` dispatch. The
coordinator does not reimplement, wrap or modify the installed legacy runtime,
and it does not inspect a canonical checkpoint to infer the format. The active
Postcard path remains outside this dormant module.

## Canonical dispatch

A canonical plan must already contain the mutual authenticated selection from
M1.1.68 and must have passed the M1.1.69 downgrade matrix. The coordinator then
requires the corresponding verified durable runtime. It exact-matches the
conversation, requester/listener Device roles, session binding and canonical
protocol identifier before exposing an action.

Client and responder actions come from the persisted checkpoint/frame pair.
Pending Inventory, EventBatch, Diff or Complete is returned byte-equivalent to
the durable record; it is never rebuilt from current local state. Missing
runtime, swapped role, cross-session identity or checkpoint/frame mismatch fail
closed.

## Deliberate non-activation

The coordinator returns a dispatch description, not a transport permit. Its
`active_transport_authorized()` result is always false. It does not register an
ALPN, dial a peer, accept a connection, read or write a network frame, or alter
the active Postcard path. The new symbols remain absent from active wire, Iroh
and CLI code.

The next bounded stage is a deterministic rollback drill over persisted
migration epoch and selected session evidence. Sustained fuzzing and fresh
hosted/physical-device evidence remain separate gates. This stage performs no
network run, release build, archive creation or stable executable replacement.
