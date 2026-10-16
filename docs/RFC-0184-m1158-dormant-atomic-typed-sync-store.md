# RFC-0184: M1.1.58 dormant atomic typed sync store

Status: accepted locally; durable store verified, active runtime unchanged.

## Outcome

`kilogram-store` now contains `CanonicalSyncV2CarrierStore`, a dormant durable
store for exact `kilogram/m1/sync/cbor/2` carriers. A database key contains the
conversation ID, the Event namespace and the Event ID; legacy Postcard and
native canonical events therefore cannot collide even if their 32-byte IDs are
equal.

The store uses a separately versioned redb database and an exact schema marker.
Every value is strictly decoded and its embedded conversation and derived typed
key must match the database key. A byte-identical replay is idempotent, while a
different value under an immutable key fails closed.

## Atomicity boundary

`put_batch` canonicalizes and validates the complete batch before opening the
write set. Inside one redb transaction it preflights every existing key before
inserting any missing value, then commits with immediate durability. A conflict
in any element therefore aborts the whole batch; no earlier element becomes
visible. redb also serializes concurrent writers and provides crash-atomic
commit semantics.

The storage format is local state, not a third wire encoding. Values remain the
exact deterministic-CBOR closed carriers defined for `/cbor/2`; the database
does not sniff formats or fall back after a decode error.

## Integration and non-activation

`kilogram-session` implements its dormant `CanonicalSyncV2Store` contract for
the new database. Regression covers both Event namespaces, reopen persistence,
trait round trips, idempotent replay, batch rollback on conflict, corruption and
cross-conversation rejection.

This does not make the database the active runtime store. Installed Postcard
history still lives in `EventStore`; no import or shadow-read path copies it
into the typed database. The concrete legacy/canonical membership policy and
the typed checkpoint transaction are also absent. `/cbor/2` remains
unregistered and undialed, and Postcard v6 remains the only active sync wire.

## Remaining activation work

Activation remains blocked on:

1. bounded, resumable import of verified installed legacy history;
2. concrete legacy and canonical membership-policy verification;
3. transactional coupling to the typed durable checkpoint;
4. mutual authenticated capability selection with no error fallback;
5. mixed-version, downgrade and rollback regression;
6. sustained fuzzing and fresh hosted plus physical-device evidence.

Keeping the database dormant permits storage durability and corruption behavior
to be reviewed independently from network activation.
