# RFC-0183: M1.1.57 dormant history-compatible sync runtime adapter

Status: accepted locally; state machine verified, active transport unchanged.

## Outcome

`kilogram-session` now contains a transport-independent client/server state
machine for the exact `kilogram/m1/sync/cbor/2` frame generation. The adapter
signs typed inventories and diffs, binds the conversation, transport session,
requester and responder, reconciles full `(namespace, Event ID)` keys, and
requires exact event and completion sets.

The regression passes every state transition through the closed canonical frame
encoder and strict decoder. Both exact active Postcard history carriers and
native canonical-CBOR carriers converge in both transfer directions. Duplicate
store inventory, incomplete store reads and authorization-policy rejection fail
closed.

## Store and policy boundaries

The dormant adapter intentionally does not implement the active `EventStore`.
`CanonicalSyncV2Store` requires an eventual implementation to preserve the
full typed key, persist a batch atomically, accept a byte-identical replay
idempotently and reject an immutable-key conflict.

Wire verification authenticates the signatures embedded in each closed event
carrier, but signature validity alone does not establish current conversation
membership. `CanonicalSyncV2EventPolicy` is therefore mandatory and runs before
any received carrier reaches storage. A future active policy must verify legacy
Postcard membership with the active snapshot and native canonical membership
with the canonical snapshot; it may not infer the format or fall back after a
parse error.

## Non-activation guarantee

This is a dormant runtime adapter, not a dual-reader activation. The module is
compiled and covered by unit regression but is absent from the active CLI and
Iroh transport. `/cbor/2` is not registered or dialed,
`canonical_dial_enabled` remains false, and Postcard v6 over
`kilogram/m0/sync/8` remains the only active synchronization wire.

No format sniffing, parse-error fallback, automatic downgrade or active wire
change is introduced. The adapter also does not yet connect the typed durable
session checkpoint to a runtime transaction.

## Activation work that remains

Activation remains blocked on all of the following:

1. an active typed store supporting both event namespaces;
2. concrete legacy and canonical membership-policy verification;
3. transactional integration of the typed durable session checkpoint;
4. mutual authenticated capability selection with no error-triggered fallback;
5. mixed-version, downgrade and rollback regression;
6. sustained fuzzing plus fresh hosted and physical-device evidence.

Keeping these boundaries separate lets the state machine receive focused
review without creating an accidental network activation path.
